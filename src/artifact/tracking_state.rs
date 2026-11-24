//! 형식 10/11 의 **`tracking` 블록** — 공유 계약 §5.1 의 꼴을 실제 `state.yaml` 에 연결한다.
//!
//! 이 빌드(`tracking-policy`, 개발 구성)가 만들고 읽는 기록은 다음뿐이다.
//!
//! ```yaml
//! tracking:
//!   next_policy_id: 3
//!   next_event_id: 3
//!   policies: [{id: P1, …}, {id: P2, engine: 1, rules: {algorithm: sha256, digest: <원문 주소>}, proposal_digest: <Monitor 가 보여 준 제안>}]
//!   policy_file: {sync_base: {state: present|absent|unconfirmed, digest: ...}}
//!   events:
//!   - {id: M1, kind: start, chain_ref: chain:C1, after_cycle_ref: null, review_cycle_ref: null, next_cycle_id: 1,
//!      policy_before_ref: null, policy_after_ref: policy:P1, world_before_ref: null,
//!      baseline: {after_cycle_ref: null, next_cycle_id: 1, snapshot_ref: snapshot:A1, attempts: [{result: archived}]}}
//!   - {id: M2, kind: scope_change, chain_ref: chain:C1, after_cycle_ref: cycle:C4, review_cycle_ref: cycle:C4, next_cycle_id: 5,
//!      policy_before_ref: policy:P1, policy_after_ref: policy:P2, world_before_ref: snapshot:A5,
//!      baseline: {snapshot_ref: null, attempts: [{result: failed, cause: external_change}]}}     # 보관 대기
//!   archive_halt: null            # 또는 {at: step:C5/S3 | cycle:C4 | event:M2}
//! ```
//!
//! 최초 시작(구현 순서 4번)의 `P1`·`M1` 에 더해, **같은 최초 Chain 안의 범위 변경 사건**(5번: 승인 저장 = 저장 1, 보관 대기, 실패 시도,
//! 기준점 확정 = 저장 2)과 즉시 보관 중단 표식을 읽고 쓴다. 복원 결과 사건(6번)·정책 미채택(`policies: []`, 8→10 전환)·후속 Chain 의
//! 사건·사이클 없는 경계의 첫 채택은 이 빌드가 만들지 않으므로 **지원하지 않는 기록**으로 거절한다 — 손상과 다른 거절이다. 읽을 수 없는
//! 기록을 「정책 미설정」으로 바꿔 기존 전체 관측으로 우회하지 않는다(Artifact Model §3.6).
//!
//! 검사 순서: 꼴(이 모듈) → **같은 상태에서 빠짐없이 뽑은 입력**([`TrackingState::extract`])으로 기록 전체의 연결
//! ([`check_record`] — 3j, start 기준점 = 뿌리 Entry, 모든 사이클의 §6.2 ①·②, 모든 범위 변경 사건의 T8) → 이 빌드의 모양(중단 표식의
//! 자리, 닫힌 Chain 에 남은 대기 없음) → 객체(원문 blob 의 실재·주소·엔진 해석, **확정한 자리마다 그때의 확정 정책**으로 본 manifest 의
//! 범위). Monitor 승인 응답과의 연결은 `start_prep`(P1)과 `scope_change`(P2 이후)가 본다.
#![cfg(feature = "tracking-policy")]

use serde::{Deserialize, Serialize};

use super::gilignore::{ApprovedPolicy, EngineVersion, PathKind};
use super::tracking_record::{
    AttemptSeen, BaselineFacts, BaselineSlot, EventCycleFacts, EventKind, EventName, EventPolicyLink, EventPolicyLinks,
    EventWorldFacts, MarkSeen, OpeningSelection, PolicyLedger, PolicyName, PolicyRecord, RecordInputs, RecordStanding,
    ScopeMarks, StandingAt, StoredPolicy, StoredPolicyFile, StoredSyncBase, SyncBase, check_record, select_opening_policy,
};
use super::{ContentDigest, ObjectStore, SnapshotRegistry};
use crate::node::NodeKind;
use crate::refs::{CycleRef, StepRef};
use crate::{ChainRef, CycleKind, SnapshotRef};

/// 보관 시도가 실패한 까닭의 저장 이름(§5.3 「`cause` 는 기존 오류 분류의 이름을 쓴다」 — 미확정 변경, 불안정한 관측, 관측 불가, 저장
/// 실패, 중단). 닫힌 집합이며 모르는 값은 거절한다.
pub(crate) const ATTEMPT_CAUSES: [&str; 5] =
    ["external_change", "unstable_observation", "unobservable", "store_failure", "interrupted"];

/// 실제 `state.yaml` 에서 읽은(또는 시작 확정·승인 저장·기준점 확정이 만든) 추적 정책 기록.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TrackingState {
    ledger: PolicyLedger,
    sync_base: SyncBase,
    /// 자리 n 의 사건 이름이 `M<n+1>` 이다(T1). 첫 사건은 `start` 다.
    events: Vec<TrackedEvent>,
    halt: Option<HaltAt>,
}

/// 최초 시작 사건 `M1` 의 값 가운데 기록마다 달라지는 둘. 나머지 칸은 이 모양에서 고정이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StartEvent {
    pub(crate) chain: ChainRef,
    pub(crate) snapshot: SnapshotRef,
}

/// 관리 사건 하나(§5.1). 승인 저장 때의 칸은 만들어진 뒤 바뀌지 않고, `archived`·`attempts` 만 나중에 채워진다(T6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TrackedEvent {
    pub(crate) kind: EventKind,
    pub(crate) chain: ChainRef,
    /// 승인을 저장한 경계. `start` 는 `None`(적힌 `null`).
    pub(crate) after_cycle: Option<CycleRef>,
    /// `scope_change` 의 검토 인터뷰. `start` 는 `None`.
    pub(crate) review_cycle: Option<CycleRef>,
    pub(crate) next_cycle_id: u32,
    pub(crate) before: Option<PolicyName>,
    pub(crate) after: PolicyName,
    pub(crate) world_before: Option<SnapshotRef>,
    /// 확정된 기준점. 없으면 (마지막 사건일 때) 보관 대기다.
    pub(crate) archived: Option<Archived>,
    /// 실패한 시도들의 `cause`. 기준점이 있으면 그 뒤에 `archived` 시도 하나가 저장 꼴에 덧붙는다.
    pub(crate) failed: Vec<String>,
}

/// 기준점 확정의 세 칸 — 한 번만 적는다(T6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Archived {
    /// 보관한 경계. `start` 는 `None`.
    pub(crate) after_cycle: Option<CycleRef>,
    pub(crate) next_cycle_id: u32,
    pub(crate) snapshot: SnapshotRef,
}

/// 즉시 보관 중단 표식이 가리키는 자리(§5.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HaltAt {
    Step(StepRef),
    Cycle(CycleRef),
    Event(EventName),
}

impl std::fmt::Display for HaltAt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HaltAt::Step(step) => write!(f, "{step}"),
            HaltAt::Cycle(cycle) => write!(f, "{cycle}"),
            HaltAt::Event(event) => write!(f, "event:{}", event.id()),
        }
    }
}

/// 기록을 읽지 못한 까닭. `Unsupported` 는 손상이 아니라 이 빌드가 아직 다루지 않는 모양이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TrackingError {
    Shape(String),
    Unsupported(String),
    Links(String),
    Objects(String),
}

impl std::fmt::Display for TrackingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrackingError::Shape(why) => write!(f, "tracking 기록의 꼴이 맞지 않는다: {why}"),
            TrackingError::Unsupported(why) => write!(f, "이 빌드가 지원하지 않는 tracking 기록이다: {why}"),
            TrackingError::Links(why) => write!(f, "정책·사건·사이클·세계의 연결이 맞지 않는다: {why}"),
            TrackingError::Objects(why) => write!(f, "정책 원문·보관 세계 객체가 기록과 맞지 않는다: {why}"),
        }
    }
}

/// 같은 실제 상태에서 **빠짐없이** 뽑은 기존 검사들의 입력(3c·3e·3h·보충 칸·표식). 호출자가 고른 값이 들어갈 자리가 없다.
#[derive(Debug, Clone)]
pub(crate) struct Extracted {
    pub(crate) ledger: PolicyLedger,
    pub(crate) links: EventPolicyLinks,
    pub(crate) facts: Vec<EventCycleFacts>,
    pub(crate) worlds: Vec<EventWorldFacts>,
    pub(crate) marks: ScopeMarks,
}

impl Extracted {
    pub(crate) fn inputs<'a>(&'a self, chains: &'a [crate::Chain], registry: &'a SnapshotRegistry) -> RecordInputs<'a> {
        RecordInputs {
            ledger: &self.ledger,
            links: &self.links,
            facts: &self.facts,
            worlds: &self.worlds,
            chains,
            registry,
            marks: self.marks,
        }
    }
}

fn event_name(position: usize) -> EventName {
    u32::try_from(position).ok().and_then(|at| at.checked_add(1)).and_then(EventName::new).expect("사건 수는 이름의 범위 안이다")
}

impl TrackingState {
    /// 정식 시작의 한 번 저장이 쓰는 기록 — 최초 정책 P1, 시작 사건 M1, 기준점 = 최초 Snapshot.
    pub(crate) fn started(policy: PolicyRecord, sync_base: SyncBase, start: StartEvent) -> Result<Self, TrackingError> {
        let after = policy.name();
        let ledger = PolicyLedger::from_records(2, vec![policy]).map_err(|e| TrackingError::Shape(format!("{e:?}")))?;
        let event = TrackedEvent {
            kind: EventKind::Start,
            chain: start.chain,
            after_cycle: None,
            review_cycle: None,
            next_cycle_id: 1,
            before: None,
            after,
            world_before: None,
            archived: Some(Archived { after_cycle: None, next_cycle_id: 1, snapshot: start.snapshot }),
            failed: Vec::new(),
        };
        Ok(TrackingState { ledger, sync_base, events: vec![event], halt: None })
    }

    /// 적용 정책 = 원장의 마지막 판(§5.2). 승인 저장이 성공한 순간부터 그 판이다 — 기준점 보관을 기다리지 않는다.
    pub(crate) fn applied(&self) -> &PolicyRecord {
        self.ledger.policies().last().expect("이 모양의 원장에는 P1 이 있다")
    }

    pub(crate) fn ledger(&self) -> &PolicyLedger {
        &self.ledger
    }

    /// 이름으로 판을 찾는다.
    pub(crate) fn policy(&self, name: PolicyName) -> Option<&PolicyRecord> {
        self.ledger.policies().iter().find(|record| record.name() == name)
    }

    pub(crate) fn sync_base(&self) -> &SyncBase {
        &self.sync_base
    }

    pub(crate) fn start(&self) -> StartEvent {
        let first = &self.events[0];
        let archived = first.archived.expect("start 사건에는 기준점이 있다(§5.3)");
        StartEvent { chain: first.chain, snapshot: archived.snapshot }
    }

    /// 사건들과 그 이름 — 저장 순서.
    pub(crate) fn events(&self) -> impl Iterator<Item = (EventName, &TrackedEvent)> {
        self.events.iter().enumerate().map(|(position, event)| (event_name(position), event))
    }

    /// 보관 대기인 사건 — 기준점이 없는 마지막 사건(§5.2 유도값. 별도 플래그가 없다).
    pub(crate) fn waiting(&self) -> Option<(EventName, &TrackedEvent)> {
        let last = self.events.last().filter(|event| event.archived.is_none())?;
        Some((event_name(self.events.len() - 1), last))
    }

    /// 즉시 보관 중단 표식(§5.6).
    pub(crate) fn halt(&self) -> Option<HaltAt> {
        self.halt
    }

    /// 닫힌 사이클 `cycle` 에서 보관한 기준점 가운데 사건 이름이 가장 큰 것 — B(T) 의 사건 갈래(§6.1). 없으면 그 경계의 세계는 T 의 Exit 이다.
    pub(crate) fn baseline_at(&self, cycle: CycleRef) -> Option<SnapshotRef> {
        self.events
            .iter()
            .rev()
            .filter_map(|event| event.archived)
            .find(|archived| archived.after_cycle == Some(cycle))
            .map(|archived| archived.snapshot)
    }

    // ── 전이(순수) — 저장은 부르는 쪽이 한 번 한다 ─────────────────────────

    /// **저장 1** 의 값(§5.3) — 새 정책 판 + 그 판을 after 로 하는 `scope_change` 사건(기준점 없음) + `sync_base`.
    /// 앞선 판·사건을 바꾸지 않고 끝에만 붙인다.
    pub(crate) fn with_scope_change(&self, policy: PolicyRecord, sync_base: SyncBase, event: TrackedEvent) -> Result<Self, TrackingError> {
        let shape = |why: &str| TrackingError::Shape(why.into());
        if event.kind != EventKind::ScopeChange || event.archived.is_some() || !event.failed.is_empty() {
            return Err(shape("승인 저장의 사건은 기준점·시도가 없는 scope_change 다"));
        }
        if event.before != Some(self.applied().name()) || event.after != policy.name() {
            return Err(shape("새 사건의 정책 연결이 원장의 마지막 판에서 새 판으로 이어지지 않는다"));
        }
        if self.halt.is_some() {
            return Err(shape("보관 중단 표식이 있는 동안 승인을 저장하지 않는다(§5.3 사전 조건)"));
        }
        let mut records = self.ledger.policies().to_vec();
        records.push(policy);
        let next = self.ledger.next_policy_id().checked_add(1).ok_or_else(|| shape("정책 이름이 소진됐다"))?;
        let ledger = PolicyLedger::from_records(next, records).map_err(|e| TrackingError::Shape(format!("{e:?}")))?;
        let mut events = self.events.clone();
        events.push(event);
        Ok(TrackingState { ledger, sync_base, events, halt: None })
    }

    /// **저장 2** 의 값(§5.3) — 보관 대기인 마지막 사건 `event` 에 기준점·보관 경계·발급기와 `archived` 시도를 한 번에 적는다.
    pub(crate) fn with_baseline(&self, event: EventName, archived: Archived) -> Result<Self, TrackingError> {
        let mut next = self.clone();
        next.waiting_mut(event)?.archived = Some(archived);
        Ok(next)
    }

    /// 보관 대기인 마지막 사건 `event` 에 실패 시도 하나를 덧붙인다. 정책·사건·기준점을 바꾸지 않는다.
    pub(crate) fn with_failed_attempt(&self, event: EventName, cause: &str) -> Result<Self, TrackingError> {
        if !ATTEMPT_CAUSES.contains(&cause) {
            return Err(TrackingError::Shape(format!("모르는 실패 사유 {cause:?}")));
        }
        let mut next = self.clone();
        next.waiting_mut(event)?.failed.push(cause.to_string());
        Ok(next)
    }

    fn waiting_mut(&mut self, event: EventName) -> Result<&mut TrackedEvent, TrackingError> {
        let refuse = || TrackingError::Shape(format!("{event} 은 보관 대기인 마지막 사건이 아니다"));
        if self.halt.is_some() {
            return Err(TrackingError::Shape("보관 중단 표식이 있는 동안 기준점·시도를 적지 않는다(T12)".into()));
        }
        if event_name(self.events.len() - 1) != event {
            return Err(refuse());
        }
        self.events.last_mut().filter(|last| last.archived.is_none() && last.kind == EventKind::ScopeChange).ok_or_else(refuse)
    }

    /// 정책 파일 동기화의 기준만 바꾼 값(§5.2). 판·사건을 만들지 않는다.
    pub(crate) fn with_sync_base(&self, sync_base: SyncBase) -> Self {
        TrackingState { sync_base, ..self.clone() }
    }

    /// 중단 표식을 적은 값. 이미 있으면 그대로 둔다(같은 요청의 재전송이 표식을 옮기지 않는다).
    pub(crate) fn with_halt(&self, at: HaltAt) -> Self {
        TrackingState { halt: Some(self.halt.unwrap_or(at)), ..self.clone() }
    }

    /// 중단 표식을 지운 값. 그 밖의 어떤 칸도 바꾸지 않는다 — 해제는 재시도·보관·복원·종료가 아니다.
    pub(crate) fn without_halt(&self) -> Self {
        TrackingState { halt: None, ..self.clone() }
    }

    // ── 저장 꼴 ────────────────────────────────────────────────────────────

    pub(crate) fn to_stored(&self) -> StoredTracking {
        let (next_policy_id, policies) = self.ledger.to_stored();
        let policy_ref = |name: PolicyName| format!("policy:{}", name.id());
        let events = self
            .events()
            .map(|(name, event)| {
                let mut attempts: Vec<StoredAttempt> =
                    event.failed.iter().map(|cause| StoredAttempt { result: "failed".into(), cause: Some(cause.clone()) }).collect();
                if event.archived.is_some() {
                    attempts.push(StoredAttempt { result: "archived".into(), cause: None });
                }
                StoredEvent {
                    id: name.id(),
                    kind: event.kind.name().into(),
                    chain_ref: event.chain.to_string(),
                    after_cycle_ref: event.after_cycle.map(|cycle| cycle.to_string()),
                    review_cycle_ref: event.review_cycle.map(|cycle| cycle.to_string()),
                    next_cycle_id: event.next_cycle_id,
                    policy_before_ref: event.before.map(policy_ref),
                    policy_after_ref: Some(policy_ref(event.after)),
                    world_before_ref: event.world_before.map(|world| world.to_string()),
                    baseline: StoredBaseline {
                        after_cycle_ref: event.archived.map(|archived| archived.after_cycle.map(|cycle| cycle.to_string())),
                        next_cycle_id: event.archived.map(|archived| archived.next_cycle_id),
                        snapshot_ref: event.archived.map(|archived| archived.snapshot.to_string()),
                        attempts,
                    },
                }
            })
            .collect();
        StoredTracking {
            next_policy_id,
            next_event_id: u32::try_from(self.events.len()).ok().and_then(|count| count.checked_add(1)).expect("사건 수는 이름의 범위 안이다"),
            policies,
            policy_file: StoredPolicyFile { sync_base: StoredSyncBase::of(&self.sync_base) },
            events,
            archive_halt: self.halt.map(|at| StoredHalt { at: at.to_string() }),
        }
    }

    /// 꼴과 이 빌드가 다루는 모양을 확인하며 읽는다. 연결·객체 검사는 [`check_links`](Self::check_links)·
    /// [`check_objects`](Self::check_objects) 가 따로 한다.
    pub(crate) fn from_stored(stored: StoredTracking) -> Result<Self, TrackingError> {
        let shape = |why: String| TrackingError::Shape(why);
        let unsupported = |why: &str| TrackingError::Unsupported(why.into());
        if stored.policies.is_empty() && stored.events.is_empty() {
            return Err(unsupported("정책을 아직 채택하지 않은 형식 10/11 기록(8→10 전환 결과)은 이 빌드가 만들지도 읽지도 않는다"));
        }
        let wanted = u32::try_from(stored.events.len()).ok().and_then(|count| count.checked_add(1));
        if wanted != Some(stored.next_event_id) {
            return Err(shape(format!("next_event_id {} 가 사건 수 {} 와 맞지 않는다(T1)", stored.next_event_id, stored.events.len())));
        }
        let ledger = PolicyLedger::from_stored(stored.next_policy_id, stored.policies)
            .map_err(|e| shape(format!("정책 원장: {e:?}")))?;
        let sync_base = stored.policy_file.sync_base.into_sync_base()
            .map_err(|e| shape(format!("sync_base: {e:?}")))?;
        let mut events = Vec::new();
        for (position, event) in stored.events.into_iter().enumerate() {
            let name = event_name(position);
            if event.id != name.id() {
                return Err(shape(format!("{}번째 사건의 이름이 {} 이 아니라 {:?} 다(T1)", position + 1, name.id(), event.id)));
            }
            let kind = EventKind::from_name(&event.kind).ok_or_else(|| shape(format!("모르는 사건 종류 {:?}", event.kind)))?;
            events.push(match kind {
                EventKind::Start if position == 0 => read_start(event)?,
                EventKind::Start => return Err(shape(format!("{} — start 는 첫 사건에서만 나온다", name.id()))),
                EventKind::ScopeChange if position == 0 => {
                    return Err(unsupported("기존 프로젝트의 첫 채택(start 없는 scope_change)은 이 빌드가 만들지도 읽지도 않는다"));
                }
                EventKind::ScopeChange => read_scope_change_event(&name.id(), event)?,
                EventKind::RestoreResult => return Err(unsupported("복원 결과 사건은 이 빌드가 아직 지원하지 않는다(구현 순서 6번)")),
            });
        }
        if events.is_empty() {
            return Err(shape("정책은 있는데 사건이 없다(T4)".into()));
        }
        let halt = stored.archive_halt.map(|halt| read_halt(&halt.at)).transpose()?;
        Ok(TrackingState { ledger, sync_base, events, halt })
    }

    /// **같은 상태에서 빠짐없이** 기존 검사들의 입력을 뽑는다 — 원장 전체, 모든 사건의 정책 연결·사이클 칸·보충 칸, 중단 표식.
    ///
    /// 미완 복원 흔적은 `state.yaml` 밖(`.gil/restore/`)의 사실이라 이 값이 알지 못한다. 부르는 쪽이 실제로 본 값을 건네고, 보지 않았으면
    /// [`MarkSeen::NotProvided`] 다 — 「없다」로 채우지 않는다.
    pub(crate) fn extract(&self, unfinished_restore: MarkSeen) -> Result<Extracted, TrackingError> {
        let mut links = Vec::new();
        let mut facts = Vec::new();
        let mut worlds = Vec::new();
        for (name, event) in self.events() {
            links.push(EventPolicyLink::new(name, event.kind, event.before, Some(event.after)));
            let mut attempts = vec![AttemptSeen::NotArchived; event.failed.len()];
            let (snapshot, boundary, issuer) = match event.archived {
                Some(archived) => {
                    attempts.push(AttemptSeen::Archived);
                    (Some(archived.snapshot), BaselineSlot::Written(archived.after_cycle), BaselineSlot::Written(archived.next_cycle_id))
                }
                None => (None, BaselineSlot::Unwritten, BaselineSlot::Unwritten),
            };
            let baseline = BaselineFacts::new(name, event.kind, snapshot, boundary, issuer, attempts);
            facts.push(EventCycleFacts::new(baseline, event.chain, BaselineSlot::Written(event.after_cycle), event.next_cycle_id));
            worlds.push(EventWorldFacts::new(
                name,
                event.kind,
                BaselineSlot::Written(event.review_cycle),
                BaselineSlot::Written(event.world_before),
            ));
        }
        let next = self.to_stored().next_event_id;
        let links = EventPolicyLinks::from_links(next, links).map_err(|e| TrackingError::Links(format!("{e:?}")))?;
        let archive_halt = match self.halt {
            Some(_) => MarkSeen::Present,
            None => MarkSeen::Absent,
        };
        Ok(Extracted { ledger: self.ledger.clone(), links, facts, worlds, marks: ScopeMarks { archive_halt, unfinished_restore } })
    }

    /// 같은 실제 상태에서 뽑은 입력으로 기록 전체를 검사하고, 지금 자리의 세 값(적용 정책·마지막 확정 세계와 확정 정책·대기)을 읽는다.
    ///
    /// **열 때와 load 때 같은 규칙**(§6.2)이 이 한 함수다 — [`check_record`] 가 모든 사이클의 ①·②와 모든 범위 변경 사건의 T8 을 보고,
    /// 여기서 이 빌드의 모양을 더 본다. 3j 의 미판정 가운데 시작 사건의 `null` 경계는 `from_stored` 가 이 모양의 정해진 값으로 확인했고,
    /// 기준점이 없는 사건의 `baseline` 칸 부재는 정상 부재다.
    pub(crate) fn standing(&self, chains: &[crate::Chain], registry: &SnapshotRegistry) -> Result<RecordStanding, TrackingError> {
        let extracted = self.extract(MarkSeen::NotProvided)?;
        let standing = check_record(&extracted.inputs(chains, registry)).map_err(|problems| {
            TrackingError::Links(problems.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "))
        })?;
        // 이 빌드의 모양 ── 닫힌 Chain 에 남은 대기 사건은 만들지 않는다(종료 Synthesis 승인 전에 막는다, 2026-11-24 O-3 결정).
        if standing.waiting.is_some() && (standing.chain != standing.start_chain || standing.chain_closed || standing.closing_boundary) {
            return Err(TrackingError::Unsupported(
                "기준점 보관을 기다리는 사건이 있는데 그 Chain 이 종료 경계이거나 닫혔다 — 이 빌드는 보관 확정 전에 Chain 종료 승인으로 나아가지 않는다".into(),
            ));
        }
        if let Some(at) = self.halt {
            let expected = self.halt_place(chains, &standing);
            if expected != Some(at) {
                return Err(TrackingError::Links(format!(
                    "보관 중단 표식 {at} 이 지금 자리({})와 다르다 — 표식이 있는 동안에는 자리가 움직이지 않는다(§5.6)",
                    expected.map_or_else(|| "알 수 없음".to_string(), |place| place.to_string()),
                )));
            }
        }
        Ok(standing)
    }

    /// load 가 부르는 연결 검사 — [`standing`](Self::standing) 과 같은 검사이고 값만 버린다.
    pub(crate) fn check_links(&self, chains: &[crate::Chain], registry: &SnapshotRegistry) -> Result<(), TrackingError> {
        self.standing(chains, registry).map(drop)
    }

    /// 지금 중단을 요청하면 표식이 가리킬 자리(§5.6): 열린 단계가 있으면 그 단계, 없고 보관 대기면 그 사건, 그 밖에는 서 있는 사이클.
    pub(crate) fn halt_place(&self, chains: &[crate::Chain], standing: &RecordStanding) -> Option<HaltAt> {
        let cycle = chains.last()?.cycles().current();
        if let Some(open) = cycle.step_now_open() {
            return Some(HaltAt::Step(cycle.step_ref(open)));
        }
        if let Some(event) = standing.waiting {
            return Some(HaltAt::Event(event));
        }
        Some(HaltAt::Cycle(match standing.at {
            StandingAt::ClosedBoundary { cycle } | StandingAt::OpenInterview { cycle } | StandingAt::OpenExperiment { cycle } => cycle,
        }))
    }

    /// 원문 객체(T2)와 보관 세계.
    ///
    /// 모든 판의 원문 blob 이 실재하고 주소와 맞으며 기록된 엔진으로 해석되는가. 그리고 **확정한 자리마다 그때의 확정 정책으로**
    /// manifest 가 추적 범위 안인가 — 사건의 기준점은 그 사건의 `policy_after`, Experiment 의 Verify 는 그 실험을 열 때의 적용 정책
    /// P(N)(§6.1). 과거 보관본을 최신 정책으로 일괄 판정하지 않는다. 같은 세계 이름을 다른 정책의 자리가 함께 가리키면 자리마다 따로 본다
    /// (§5.5). 어느 확정 자리도 가리키지 않는 등록 세계는 판정할 정책이 없으므로 통과시키지 않는다.
    /// 성공하면 적용 정책을 돌려준다 — 이후 관측·보관이 그 판을 쓴다.
    pub(crate) fn check_objects(&self, store: &ObjectStore, chains: &[crate::Chain], registry: &SnapshotRegistry) -> Result<ApprovedPolicy, TrackingError> {
        let mut policies: Vec<(PolicyName, ApprovedPolicy)> = Vec::new();
        for record in self.ledger.policies() {
            policies.push((record.name(), load_policy(store, record.engine(), record.rules())?));
        }
        // (세계, 그 자리의 확정 정책, 자리 이름)
        let mut places: Vec<(SnapshotRef, PolicyName, String)> = Vec::new();
        for (name, event) in self.events() {
            if let Some(archived) = event.archived {
                places.push((archived.snapshot, event.after, format!("event:{} 의 기준점", name.id())));
            }
        }
        let extracted = self.extract(MarkSeen::NotProvided)?;
        for cycle in chains.iter().flat_map(|chain| chain.cycles().nodes()) {
            if cycle.kind() != CycleKind::Experiment {
                continue;
            }
            let target = cycle.id().to_ref();
            let opened = select_opening_policy(target, &extracted.ledger, &extracted.links, &extracted.facts, chains, registry)
                .map_err(|problems| TrackingError::Links(format!("{target} 을 열 때의 적용 정책: {problems:?}")))?;
            let OpeningSelection::Applied { policy, .. } = opened.selection() else {
                return Err(TrackingError::Links(format!("{target} 을 열 때 적용된 정책이 없다")));
            };
            for node in cycle.steps().nodes() {
                if let (NodeKind::Verify, true, Some(world)) = (node.kind, node.is_closed(), node.snapshot) {
                    places.push((world, policy, format!("{} 의 Verify", cycle.step_ref(node.id))));
                }
            }
        }
        for world in registry.records() {
            let manifest = store.read_manifest(world.manifest())
                .map_err(|e| TrackingError::Objects(format!("{}: {e}", world.id())))?;
            let mut judged = false;
            for (_, name, place) in places.iter().filter(|(at, _, _)| *at == world.id()) {
                judged = true;
                let policy = policies.iter().find(|(known, _)| known == name).map(|(_, policy)| policy).ok_or_else(|| TrackingError::Links(format!("{place} 의 확정 정책 {name} 이 원장에 없다")))?;
                if let Some(path) = manifest.paths().find(|path| !policy.decide(path, PathKind::File).is_tracked()) {
                    return Err(TrackingError::Objects(format!(
                        "{} 의 {path} 는 그 세계를 확정한 {place} 의 정책 {} 의 추적 범위 밖이다", world.id(), name.id())));
                }
            }
            if !judged {
                // 정책이 하나뿐인 기록(최초 시작의 모양)은 기존 검사 그대로다 — 모든 등록 세계가 그 한 판의 범위 안이다.
                let [(name, policy)] = policies.as_slice() else {
                    return Err(TrackingError::Objects(format!(
                        "{} 을 확정한 자리(사건의 기준점·Verify)를 기록에서 찾지 못했다 — 어느 정책으로 판정할지 알 수 없다", world.id())));
                };
                if let Some(path) = manifest.paths().find(|path| !policy.decide(path, PathKind::File).is_tracked()) {
                    return Err(TrackingError::Objects(format!(
                        "{} 의 {path} 는 적용 정책 {} 의 추적 범위 밖이다", world.id(), name.id())));
                }
            }
        }
        let applied = self.applied().name();
        policies.into_iter().find(|(name, _)| *name == applied).map(|(_, policy)| policy)
            .ok_or_else(|| TrackingError::Links("적용 정책이 원장에 없다".into()))
    }
}

fn parse_policy(text: &str, field: &str) -> Result<PolicyName, TrackingError> {
    text.strip_prefix("policy:")
        .and_then(|bare| PolicyName::parse_bare(bare).ok())
        .ok_or_else(|| TrackingError::Shape(format!("{field}: {text:?} 는 policy:P<n> 꼴이 아니다")))
}

fn parse_ref<T: std::str::FromStr>(text: &str, field: &str) -> Result<T, TrackingError>
where
    T::Err: std::fmt::Display,
{
    text.parse().map_err(|e| TrackingError::Shape(format!("{field}: {e}")))
}

fn read_start(event: StoredEvent) -> Result<TrackedEvent, TrackingError> {
    let shape = |why: &str| TrackingError::Shape(why.into());
    let fixed = event.after_cycle_ref.is_none()
        && event.review_cycle_ref.is_none()
        && event.next_cycle_id == 1
        && event.policy_before_ref.is_none()
        && event.policy_after_ref.as_deref() == Some("policy:P1")
        && event.world_before_ref.is_none()
        && event.baseline.after_cycle_ref == Some(None)
        && event.baseline.next_cycle_id == Some(1)
        && event.baseline.attempts == [StoredAttempt { result: "archived".into(), cause: None }];
    if !fixed {
        return Err(shape("시작 사건 M1 의 경계·발급기·정책·보관 시도 칸이 최초 시작의 값과 다르다"));
    }
    let chain: ChainRef = parse_ref(&event.chain_ref, "chain_ref")?;
    let snapshot: SnapshotRef = parse_ref(
        event.baseline.snapshot_ref.as_deref().ok_or_else(|| shape("시작 사건에 기준점이 없다 — 보관 대기 상태의 start 는 없다(§5.3)"))?,
        "baseline.snapshot_ref",
    )?;
    Ok(TrackedEvent {
        kind: EventKind::Start,
        chain,
        after_cycle: None,
        review_cycle: None,
        next_cycle_id: 1,
        before: None,
        after: PolicyName::new(1).expect("P1"),
        world_before: None,
        archived: Some(Archived { after_cycle: None, next_cycle_id: 1, snapshot }),
        failed: Vec::new(),
    })
}

fn read_scope_change_event(id: &str, event: StoredEvent) -> Result<TrackedEvent, TrackingError> {
    let shape = |why: String| TrackingError::Shape(format!("{id}: {why}"));
    let unsupported = |why: &str| TrackingError::Unsupported(format!("{id}: {why}"));
    let after_cycle: CycleRef = parse_ref(
        event.after_cycle_ref.as_deref().ok_or_else(|| unsupported("사이클 없는 경계의 범위 변경은 이 빌드가 만들지도 읽지도 않는다"))?,
        "after_cycle_ref",
    )?;
    let review_cycle: CycleRef = parse_ref(
        event.review_cycle_ref.as_deref().ok_or_else(|| shape("scope_change 에는 검토 인터뷰(review_cycle_ref)가 있다(§5.1)".into()))?,
        "review_cycle_ref",
    )?;
    let before = parse_policy(
        event.policy_before_ref.as_deref().ok_or_else(|| unsupported("앞 정책이 없는 범위 변경(첫 채택)은 이 빌드가 만들지도 읽지도 않는다"))?,
        "policy_before_ref",
    )?;
    let after = parse_policy(
        event.policy_after_ref.as_deref().ok_or_else(|| shape("policy_after_ref 가 없다(T4)".into()))?,
        "policy_after_ref",
    )?;
    let world_before: SnapshotRef = parse_ref(
        event.world_before_ref.as_deref().ok_or_else(|| shape("world_before_ref 가 없다(T8)".into()))?,
        "world_before_ref",
    )?;
    let mut failed = Vec::new();
    let mut archived_attempts = 0usize;
    let last = event.baseline.attempts.len().saturating_sub(1);
    for (position, attempt) in event.baseline.attempts.iter().enumerate() {
        match (attempt.result.as_str(), attempt.cause.as_deref()) {
            ("failed", Some(cause)) if ATTEMPT_CAUSES.contains(&cause) => failed.push(cause.to_string()),
            ("failed", cause) => return Err(shape(format!("실패 시도의 cause {cause:?} 를 모른다"))),
            ("archived", None) if position == last => archived_attempts += 1,
            ("archived", _) => return Err(shape("archived 시도는 cause 없이 마지막에 한 번만 나온다(§5.1)".into())),
            (other, _) => return Err(shape(format!("모르는 시도 결과 {other:?}"))),
        }
    }
    let baseline = event.baseline;
    let archived = match (baseline.snapshot_ref, baseline.after_cycle_ref, baseline.next_cycle_id) {
        (None, None, None) if archived_attempts == 0 => None,
        (Some(snapshot), Some(Some(cycle)), Some(next_cycle_id)) if archived_attempts == 1 => Some(Archived {
            after_cycle: Some(parse_ref(&cycle, "baseline.after_cycle_ref")?),
            next_cycle_id,
            snapshot: parse_ref(&snapshot, "baseline.snapshot_ref")?,
        }),
        _ => return Err(shape("baseline 의 기준점·보관 경계·발급기·archived 시도는 함께 있거나 함께 없다(T6)".into())),
    };
    Ok(TrackedEvent {
        kind: EventKind::ScopeChange,
        chain: parse_ref(&event.chain_ref, "chain_ref")?,
        after_cycle: Some(after_cycle),
        review_cycle: Some(review_cycle),
        next_cycle_id: event.next_cycle_id,
        before: Some(before),
        after,
        world_before: Some(world_before),
        archived,
        failed,
    })
}

fn read_halt(at: &str) -> Result<HaltAt, TrackingError> {
    let bad = || TrackingError::Shape(format!("archive_halt.at {at:?} 는 step:·cycle:·event: 참조가 아니다(§5.6)"));
    if let Some(bare) = at.strip_prefix("event:") {
        return EventName::parse_bare(bare).map(HaltAt::Event).map_err(|_| bad());
    }
    if at.starts_with("step:") {
        return at.parse().map(HaltAt::Step).map_err(|_| bad());
    }
    at.parse().map(HaltAt::Cycle).map_err(|_| bad())
}

/// 원문 객체를 읽어 주소를 다시 재고 그 엔진으로 해석한다. 원문을 정규화하지 않는다.
pub(crate) fn load_policy(store: &ObjectStore, engine: EngineVersion, rules: &ContentDigest) -> Result<ApprovedPolicy, TrackingError> {
    let mut bytes = Vec::new();
    store.read_blob(rules, &mut bytes).map_err(|e| TrackingError::Objects(format!("정책 원문 {}: {e}", rules.hex())))?;
    let policy = ApprovedPolicy::compile(engine, &bytes)
        .map_err(|e| TrackingError::Objects(format!("정책 원문을 엔진이 해석하지 못한다: {e}")))?;
    if policy.identity().rules_digest() != rules {
        return Err(TrackingError::Objects("정책 원문의 주소가 기록과 다르다".into()));
    }
    Ok(policy)
}

// ── 저장 꼴 ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StoredTracking {
    next_policy_id: u32,
    next_event_id: u32,
    policies: Vec<StoredPolicy>,
    policy_file: StoredPolicyFile,
    events: Vec<StoredEvent>,
    archive_halt: Option<StoredHalt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredHalt {
    at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredEvent {
    id: String,
    kind: String,
    chain_ref: String,
    after_cycle_ref: Option<String>,
    review_cycle_ref: Option<String>,
    next_cycle_id: u32,
    policy_before_ref: Option<String>,
    policy_after_ref: Option<String>,
    world_before_ref: Option<String>,
    baseline: StoredBaseline,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredBaseline {
    /// 칸 없음(`None`)과 적힌 `null`(`Some(None)`)을 가른다 — 보관한 경계는 기준점과 함께 한 번만 적는다(T6).
    #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
    after_cycle_ref: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    next_cycle_id: Option<u32>,
    snapshot_ref: Option<String>,
    attempts: Vec<StoredAttempt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredAttempt {
    result: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cause: Option<String>,
}

fn present<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(deserializer: D) -> Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(deserializer).map(Some)
}
