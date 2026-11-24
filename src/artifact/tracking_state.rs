//! 형식 10/11 의 **`tracking` 블록** — 공유 계약 §5.1 의 꼴을 실제 `state.yaml` 에 연결한다.
//!
//! 이 빌드(`tracking-policy`, 개발 구성)가 만들고 읽는 기록은 **최초 시작의 한 모양뿐**이다.
//!
//! ```yaml
//! tracking:
//!   next_policy_id: 2
//!   next_event_id: 2
//!   policies: [{id: P1, engine: 1, rules: {algorithm: sha256, digest: <원문 주소>}, proposal_digest: <Monitor 가 보여 준 제안>}]
//!   policy_file: {sync_base: {state: present|absent|unconfirmed, digest: ...}}
//!   events:
//!   - {id: M1, kind: start, chain_ref: chain:C1, after_cycle_ref: null, review_cycle_ref: null, next_cycle_id: 1,
//!      policy_before_ref: null, policy_after_ref: policy:P1, world_before_ref: null,
//!      baseline: {after_cycle_ref: null, next_cycle_id: 1, snapshot_ref: snapshot:A1, attempts: [{result: archived}]}}
//!   archive_halt: null
//! ```
//!
//! 범위 변경 사건(구현 순서 5번)·복원 결과 사건(6번)·정책 미채택(`policies: []`, 8→10 전환)·보관 중단 표식은
//! 이 빌드가 만들지 않으므로 **지원하지 않는 기록**으로 거절한다 — 손상과 다른 거절이다. 읽을 수 없는 기록을
//! 「정책 미설정」으로 바꿔 기존 전체 관측으로 우회하지 않는다(Artifact Model §3.6 「기존 프로젝트의 추적 범위 설정
//! 안내」).
//!
//! 검사 순서: 꼴(이 모듈) → 정책·사건·사이클·등록부의 연결(3j [`check_event_inputs`], 그 안의 3c·3h·3e) →
//! 이 모양의 고정 값(시작 사건의 경계·발급기·기준점 = 뿌리 Entry) → 객체(원문 blob 의 실재·주소·엔진 해석, 모든
//! 등록 세계의 manifest 가 P1 범위 안인가). Monitor 승인 응답과의 연결은 준비 기록을 읽는 [`crate::start_prep`] 이 본다.
#![cfg(feature = "tracking-policy")]

use serde::{Deserialize, Serialize};

use super::gilignore::{ApprovedPolicy, EngineVersion, PathKind};
use super::tracking_record::{
    AttemptSeen, BaselineFacts, BaselineSlot, EventCycleFacts, EventKind, EventName, EventPolicyLink,
    EventPolicyLinks, PolicyLedger, PolicyName, PolicyRecord, StoredPolicy, StoredPolicyFile, StoredSyncBase,
    SyncBase, check_event_inputs,
};
use super::{ContentDigest, ObjectStore, SnapshotRegistry};
use crate::{ChainRef, SnapshotRef};

/// 실제 `state.yaml` 에서 읽은(또는 시작 확정이 만든) 추적 정책 기록.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TrackingState {
    ledger: PolicyLedger,
    sync_base: SyncBase,
    start: StartEvent,
}

/// 최초 시작 사건 `M1` 의 값 가운데 기록마다 달라지는 둘. 나머지 칸은 이 모양에서 고정이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StartEvent {
    pub(crate) chain: ChainRef,
    pub(crate) snapshot: SnapshotRef,
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

impl TrackingState {
    /// 정식 시작의 한 번 저장이 쓰는 기록 — 최초 정책 P1, 시작 사건 M1, 기준점 = 최초 Snapshot.
    pub(crate) fn started(policy: PolicyRecord, sync_base: SyncBase, start: StartEvent) -> Result<Self, TrackingError> {
        let ledger = PolicyLedger::from_records(2, vec![policy]).map_err(|e| TrackingError::Shape(format!("{e:?}")))?;
        Ok(TrackingState { ledger, sync_base, start })
    }

    /// 적용 정책 = 원장의 마지막 판(§5.2). 이 모양에서는 P1 하나다.
    pub(crate) fn applied(&self) -> &PolicyRecord {
        self.ledger.policies().last().expect("이 모양의 원장에는 P1 이 있다")
    }

    pub(crate) fn sync_base(&self) -> &SyncBase {
        &self.sync_base
    }

    pub(crate) fn start(&self) -> StartEvent {
        self.start
    }

    pub(crate) fn to_stored(&self) -> StoredTracking {
        let (next_policy_id, policies) = self.ledger.to_stored();
        StoredTracking {
            next_policy_id,
            next_event_id: 2,
            policies,
            policy_file: StoredPolicyFile { sync_base: StoredSyncBase::of(&self.sync_base) },
            events: vec![StoredEvent {
                id: "M1".into(),
                kind: EventKind::Start.name().into(),
                chain_ref: self.start.chain.to_string(),
                after_cycle_ref: None,
                review_cycle_ref: None,
                next_cycle_id: 1,
                policy_before_ref: None,
                policy_after_ref: Some(format!("policy:{}", self.applied().name().id())),
                world_before_ref: None,
                baseline: StoredBaseline {
                    after_cycle_ref: Some(None),
                    next_cycle_id: Some(1),
                    snapshot_ref: Some(self.start.snapshot.to_string()),
                    attempts: vec![StoredAttempt { result: "archived".into(), cause: None }],
                },
            }],
            archive_halt: None,
        }
    }

    /// 꼴과 이 빌드가 다루는 모양을 확인하며 읽는다. 연결·객체 검사는 [`check_links`](Self::check_links)·
    /// [`check_objects`](Self::check_objects) 가 따로 한다.
    pub(crate) fn from_stored(stored: StoredTracking) -> Result<Self, TrackingError> {
        let shape = |why: String| TrackingError::Shape(why);
        let unsupported = |why: &str| TrackingError::Unsupported(why.into());
        if stored.archive_halt.is_some() {
            return Err(unsupported("즉시 보관 중단 표식(archive_halt)은 이 빌드에 없다"));
        }
        if stored.policies.is_empty() && stored.events.is_empty() {
            return Err(unsupported("정책을 아직 채택하지 않은 형식 10/11 기록(8→10 전환 결과)은 이 빌드가 만들지도 읽지도 않는다"));
        }
        for event in &stored.events {
            match EventKind::from_name(&event.kind) {
                Some(EventKind::Start) => {}
                Some(_) => return Err(unsupported("범위 변경·복원 결과 사건은 이 빌드가 아직 지원하지 않는다(구현 순서 5·6번)")),
                None => return Err(shape(format!("모르는 사건 종류 {:?}", event.kind))),
            }
        }
        if stored.policies.len() != 1 || stored.events.len() != 1 || stored.next_event_id != 2 {
            return Err(unsupported("이 빌드는 최초 정책 P1 과 시작 사건 M1 하나뿐인 기록만 읽는다"));
        }
        let ledger = PolicyLedger::from_stored(stored.next_policy_id, stored.policies)
            .map_err(|e| shape(format!("정책 원장: {e:?}")))?;
        let sync_base = stored.policy_file.sync_base.into_sync_base()
            .map_err(|e| shape(format!("sync_base: {e:?}")))?;
        let event = stored.events.into_iter().next().expect("길이를 확인했다");
        let fixed = event.id == "M1"
            && event.after_cycle_ref.is_none()
            && event.review_cycle_ref.is_none()
            && event.next_cycle_id == 1
            && event.policy_before_ref.is_none()
            && event.policy_after_ref.as_deref() == Some("policy:P1")
            && event.world_before_ref.is_none()
            && event.baseline.after_cycle_ref == Some(None)
            && event.baseline.next_cycle_id == Some(1)
            && event.baseline.attempts == [StoredAttempt { result: "archived".into(), cause: None }];
        if !fixed {
            return Err(shape("시작 사건 M1 의 경계·발급기·정책·보관 시도 칸이 최초 시작의 값과 다르다".into()));
        }
        let chain: ChainRef = event.chain_ref.parse().map_err(|e| shape(format!("chain_ref: {e}")))?;
        let snapshot: SnapshotRef = event.baseline.snapshot_ref.as_deref()
            .ok_or_else(|| shape("시작 사건에 기준점이 없다 — 보관 대기 상태의 start 는 없다(§5.3)".into()))?
            .parse().map_err(|e| shape(format!("baseline.snapshot_ref: {e}")))?;
        Ok(TrackingState { ledger, sync_base, start: StartEvent { chain, snapshot } })
    }

    /// 같은 실제 상태에서 빠짐없이 뽑은 입력으로 3j(그 안의 3c·3h·3e)를 부르고, 이 모양의 고정 관계를 본다.
    ///
    /// 3j 의 미판정 값(`null` 경계 등)은 성공으로 지우지 않는다: 시작 사건의 `null` 은 위 [`from_stored`] 가 이
    /// 모양의 정해진 값으로 따로 확인했고, 그 밖의 미판정은 이 모양에서 생기지 않는다.
    pub(crate) fn check_links(&self, chains: &[crate::Chain], registry: &SnapshotRegistry) -> Result<(), TrackingError> {
        let name = EventName::new(1).expect("M1");
        let policy = PolicyName::new(1).expect("P1");
        let links = EventPolicyLinks::from_links(2, vec![EventPolicyLink::new(name, EventKind::Start, None, Some(policy))])
            .map_err(|e| TrackingError::Links(format!("{e:?}")))?;
        let baseline = BaselineFacts::new(
            name,
            EventKind::Start,
            Some(self.start.snapshot),
            BaselineSlot::Written(None),
            BaselineSlot::Written(1),
            vec![AttemptSeen::Archived],
        );
        let facts = [EventCycleFacts::new(baseline, self.start.chain, BaselineSlot::Written(None), 1)];
        check_event_inputs(&self.ledger, &links, &facts, chains, registry)
            .map_err(|problems| TrackingError::Links(format!("{problems:?}")))?;
        let first = chains.first().ok_or_else(|| TrackingError::Links("Chain 이 없다".into()))?;
        let root_entry = first.cycles().nodes().first().map(crate::Cycle::entry_snapshot);
        if first.id() != self.start.chain || first.entry_snapshot() != self.start.snapshot
            || root_entry != Some(self.start.snapshot) {
            return Err(TrackingError::Links(format!(
                "시작 사건의 기준점 {} 이 첫 Chain·첫 Interview 의 Entry 와 다르다", self.start.snapshot)));
        }
        Ok(())
    }

    /// 원문 객체(T2)와 보관 세계: 원문 blob 이 실재하고 주소와 맞으며 기록된 엔진으로 해석되는가, 그리고
    /// **등록된 모든 세계의 manifest 가 그 정책의 추적 범위 안인가**(이 모양에는 정책이 하나뿐이다).
    /// 성공하면 그 정책을 돌려준다 — 이후 관측·보관이 같은 판을 쓴다.
    pub(crate) fn check_objects(&self, store: &ObjectStore, registry: &SnapshotRegistry) -> Result<ApprovedPolicy, TrackingError> {
        let record = self.applied();
        let policy = load_policy(store, record.engine(), record.rules())?;
        for world in registry.records() {
            let manifest = store.read_manifest(world.manifest())
                .map_err(|e| TrackingError::Objects(format!("{}: {e}", world.id())))?;
            if let Some(path) = manifest.paths().find(|path| !policy.decide(path, PathKind::File).is_tracked()) {
                return Err(TrackingError::Objects(format!(
                    "{} 의 {path} 는 적용 정책 {} 의 추적 범위 밖이다", world.id(), record.name().id())));
            }
        }
        Ok(policy)
    }
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
