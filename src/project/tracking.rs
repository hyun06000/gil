//! 형식 10/11 프로젝트의 **추적 정책 전이** — 범위 변경 사건(구현 순서 5번)을 도메인에 잇는다.
//!
//! 정본은 [공유 저장·전이 계약](../../spec/GIL_Storage_Transition_Contract_v0.1.md) §5.3·§5.6·§6.1–§6.3·§6.7·§12(O-2·O-3)다.
//!
//! 여기 있는 것은 **순수 전이**다. 폴더 관측·객체 확정·잠금·상태 저장은 [`crate::scope_change`] 와 `ProjectSession` 이 한다.
//! 전이마다 복제본 위에서 값을 바꾼 뒤 load 와 **같은 검사**([`TrackingState::standing`])를 통과했을 때만 갈아 끼운다 — 열 때와 읽을 때
//! 규칙이 갈리지 않는다(§6.2).
//!
//! | 묻는 것 | 답하는 자리 |
//! |---|---|
//! | 지금 자리의 적용 정책·마지막 확정 세계와 확정 정책·대기 | [`Project::tracking_standing`] |
//! | 다음 사이클의 Entry(S(N))와 Experiment 관문(검증 ②) | [`Project::tracked_entry`] |
//! | 중단 표식이 있는 동안의 진행 거절(T12) | [`Project::refuse_while_halted`] |
//! | 보관 대기 중 종료 Synthesis 승인 거절(O-3) | [`Project::refuse_closing_approval_while_waiting`] |
use super::Project;
use crate::artifact::start_policy::{PolicyRecord, SyncBase};
use crate::artifact::tracking::{Archived, EventName, HaltAt, RecordStanding, StandingAt, TrackedEvent};
use crate::artifact::{ManifestAddress, TrackingError};
use crate::node::NodeKind;
use crate::report::Report;
use crate::{CycleKind, SnapshotRef};

impl Project {
    /// 지금 자리의 세 값(§6.1) — 추적 정책이 없는 형식이면 `None`.
    pub(crate) fn tracking_standing(&self) -> Option<Result<RecordStanding, TrackingError>> {
        let tracking = self.tracking.as_ref()?;
        Some(tracking.standing(self.chains(), &self.artifacts))
    }

    /// 보관 중단 표식이 있으면 진행 전이를 거절한다(T12·§5.6). 조회는 이 문을 지나지 않는다.
    pub(crate) fn refuse_while_halted(&self) -> Result<(), String> {
        match self.tracking.as_ref().and_then(|tracking| tracking.halt()) {
            Some(at) => Err(crate::tr!("tracking.halted", at = at)),
            None => Ok(()),
        }
    }

    /// 기준점 보관을 기다리는 사건.
    pub(crate) fn waiting_event(&self) -> Option<EventName> {
        self.tracking.as_ref().and_then(|tracking| tracking.waiting()).map(|(name, _)| name)
    }

    /// **보관 확정 전에는 Chain 최종 종료 승인으로 진행하지 않는다**(2026-11-24 사용자 결정, 계약 §12 O-3).
    ///
    /// `chain_verdict` 가 있는 Synthesis 를 `approved: yes` 로 닫으면 정상 전진이 Outcome → `close_chain` Cycle Report → Chain Report 로
    /// 좁혀지고, 그 인터뷰를 닫은 경계는 재시도를 받지 않는 종료 경계다. 그래서 마지막 닫기가 아니라 **그 승인 기록**에서 멈춘다.
    /// 일반 Interview 의 Synthesis·Outcome·닫기는 이 문이 막지 않는다.
    pub(crate) fn refuse_closing_approval_while_waiting(&self, kind: NodeKind, report: &Report) -> Result<(), String> {
        let Some(event) = self.waiting_event() else { return Ok(()) };
        let closing = kind == NodeKind::Synthesis
            && self.cycles().current().kind() == CycleKind::Interview
            && report.has("chain_verdict")
            && report.get("approved") == Some("yes");
        match closing {
            true => Err(crate::tr!("tracking.closing_while_waiting", event = event)),
            false => Ok(()),
        }
    }

    /// 다음에 여는 사이클의 Entry — S(N)(§6.2). 추적 정책이 없는 형식이면 `None`(기존 규칙: 부모의 Exit).
    ///
    /// 최초 `start` 의 Chain 안에서는 지금 경계의 마지막 확정 세계(B(T): 그 경계에서 보관한 기준점, 없으면 T 의 Exit)이고,
    /// **Experiment 는 그 확정 정책이 적용 정책과 같을 때만** 연다(검증 ②) — 보관 대기 중에는 거절한다. Interview 는 ①만 지키므로 열린다.
    /// 후속 Chain 안에서는 사건이 없으므로 기존대로 부모의 Exit 이다.
    pub(crate) fn tracked_entry(&self, kind: CycleKind) -> Result<Option<SnapshotRef>, String> {
        let Some(standing) = self.tracking_standing() else { return Ok(None) };
        let standing = standing.map_err(|problem| problem.to_string())?;
        self.refuse_while_halted()?;
        if kind == CycleKind::Experiment
            && let Some(event) = standing.waiting
        {
            return Err(crate::tr!("tracking.experiment_waits_for_baseline", event = event));
        }
        let (StandingAt::ClosedBoundary { .. }, Some(confirmed)) = (standing.at, standing.confirmed.as_ref()) else {
            // 열린 사이클이면 기존 문이 「아직 열려 있다」로 거절한다. 후속 Chain 이면 기존 규칙이다.
            return Ok(None);
        };
        if kind == CycleKind::Experiment && confirmed.policy != standing.applied.policy {
            return Err(crate::tr!(
                "tracking.experiment_policy_differs",
                confirmed = confirmed.policy,
                applied = standing.applied.policy
            ));
        }
        Ok(Some(confirmed.world))
    }

    /// status·context 가 함께 싣는 안내 — **보관 대기이거나 중단 표식이 있을 때만** 말한다(그 밖에는 빈 문자열이라 기존 출력이 그대로다).
    ///
    /// 세 값을 따로 말한다(§6.3): 적용 정책, 마지막 확정 세계와 그 확정 정책, 보관 대기. 새 정책이 이미 보관에 반영된 것처럼도, 이전
    /// 정책이 아직 적용 중인 것처럼도 말하지 않는다.
    pub(crate) fn tracking_note(&self) -> String {
        let Some(tracking) = self.tracking.as_ref() else { return String::new() };
        let (waiting, halt) = (tracking.waiting().map(|(name, _)| name), tracking.halt());
        if waiting.is_none() && halt.is_none() {
            return String::new();
        }
        let mut out = match tracking.standing(self.chains(), &self.artifacts) {
            Ok(standing) => match standing.confirmed {
                Some(confirmed) => crate::tr!(
                    "tracking.note_values",
                    applied = standing.applied.policy,
                    world = confirmed.world,
                    confirmed = confirmed.policy
                ),
                None => crate::tr!("tracking.note_applied", applied = standing.applied.policy),
            },
            Err(problem) => crate::tr!("tracking.note_unread", why = problem),
        };
        if let Some(event) = waiting {
            out.push_str(&crate::tr!("tracking.note_waiting", event = event));
        }
        if let Some(at) = halt {
            out.push_str(&crate::tr!("tracking.note_halted", at = at));
        }
        out
    }

    /// 전이 뒤의 값이 load 와 같은 검사를 통과하는가.
    fn checked(next: Project) -> Result<Project, TrackingError> {
        if let Some(tracking) = next.tracking.as_ref() {
            tracking.check_links(next.chains(), &next.artifacts)?;
        }
        Ok(next)
    }

    /// 사이클을 연 뒤의 값을 같은 규칙으로 다시 본다(검증 ①·②, T9).
    pub(crate) fn check_tracking_after_open(&self) -> Result<(), String> {
        match self.tracking.as_ref() {
            Some(tracking) => tracking.check_links(self.chains(), &self.artifacts).map_err(|problem| problem.to_string()),
            None => Ok(()),
        }
    }

    /// **저장 1 의 값**(§5.3) — 새 정책 판, 그 판을 after 로 하는 `scope_change` 사건(기준점 없음), `sync_base`.
    /// 사건은 약속·Journey 판·보고서·단계를 만들지 않는다(T11). 저장은 부르는 쪽이 한 번 한다.
    pub(crate) fn with_scope_change(&self, policy: PolicyRecord, sync_base: SyncBase, event: TrackedEvent) -> Result<Project, TrackingError> {
        let tracking = self.tracking.as_ref().ok_or_else(|| TrackingError::Unsupported("추적 정책이 없는 형식이다".into()))?;
        let mut next = self.clone();
        next.tracking = Some(tracking.with_scope_change(policy, sync_base, event)?);
        Project::checked(next)
    }

    /// **저장 2 의 값**(§5.3) — 세계 이름 등록 + `baseline` 의 경계·발급기·`snapshot_ref` + `archived` 시도. 한 저장 단위다.
    /// 돌려주는 이름은 등록부가 그 manifest 에 준 이름이다(같은 세계면 기존 이름 — §5.5).
    pub(crate) fn with_baseline(&self, event: EventName, world: ManifestAddress) -> Result<(Project, SnapshotRef), TrackingError> {
        let tracking = self.tracking.as_ref().ok_or_else(|| TrackingError::Unsupported("추적 정책이 없는 형식이다".into()))?;
        let standing = tracking.standing(self.chains(), &self.artifacts)?;
        let StandingAt::ClosedBoundary { cycle } = standing.at else {
            return Err(TrackingError::Links("기준점은 닫힌 경계에서만 보관한다".into()));
        };
        let mut next = self.clone();
        let snapshot = next.artifacts.intern(world).map_err(|e| TrackingError::Objects(e.to_string()))?;
        let archived = Archived { after_cycle: Some(cycle), next_cycle_id: standing.issuer, snapshot };
        next.tracking = Some(tracking.with_baseline(event, archived)?);
        Ok((Project::checked(next)?, snapshot))
    }

    /// 보관 대기 사건에 실패 시도 하나를 덧붙인 값.
    pub(crate) fn with_failed_attempt(&self, event: EventName, cause: &str) -> Result<Project, TrackingError> {
        let tracking = self.tracking.as_ref().ok_or_else(|| TrackingError::Unsupported("추적 정책이 없는 형식이다".into()))?;
        let mut next = self.clone();
        next.tracking = Some(tracking.with_failed_attempt(event, cause)?);
        Project::checked(next)
    }

    /// 정책 파일 동기화의 기준만 바꾼 값(§5.2).
    pub(crate) fn with_sync_base(&self, sync_base: SyncBase) -> Option<Project> {
        let tracking = self.tracking.as_ref()?;
        let mut next = self.clone();
        next.tracking = Some(tracking.with_sync_base(sync_base));
        Some(next)
    }

    /// **즉시 보관 중단**(§5.6) — 표식만 적는다. 열린 단계를 닫지 않고, 실패로 판정하지 않고, 복원·삭제·범위 변경을 하지 않는다.
    /// 이미 표식이 있으면 값이 같다(재전송이 표식을 옮기지 않는다).
    pub(crate) fn with_halt(&self) -> Result<(Project, HaltAt), TrackingError> {
        let tracking = self.tracking.as_ref().ok_or_else(|| TrackingError::Unsupported("추적 정책이 없는 형식이다".into()))?;
        if let Some(at) = tracking.halt() {
            return Ok((self.clone(), at));
        }
        let standing = tracking.standing(self.chains(), &self.artifacts)?;
        let at = tracking
            .halt_place(self.chains(), &standing)
            .ok_or_else(|| TrackingError::Links("중단 표식이 가리킬 자리를 정하지 못했다".into()))?;
        let mut next = self.clone();
        next.tracking = Some(tracking.with_halt(at));
        Ok((Project::checked(next)?, at))
    }

    /// 중단 표식을 지운 값 — 그 밖의 어떤 기록도 바꾸지 않는다. 해제는 보관·재시도·복원·종료 승인이 아니다.
    pub(crate) fn without_halt(&self) -> Result<Project, TrackingError> {
        let tracking = self.tracking.as_ref().ok_or_else(|| TrackingError::Unsupported("추적 정책이 없는 형식이다".into()))?;
        let mut next = self.clone();
        next.tracking = Some(tracking.without_halt());
        Project::checked(next)
    }
}
