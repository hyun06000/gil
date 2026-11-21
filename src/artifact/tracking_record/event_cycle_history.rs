//! 관리 사건에 처음 기록한 **체인·승인 경계·발급 번호**의 두 시점 보존 검사 — 구현 순서 3번 안의 작은 후속(편의상 「3i」).
//!
//! 「3i」는 이 PR 의 편의상 이름이다. **사건 기록 전체의 보존도, T1·T7 전체의 완료도, 3번 전체의 완료도 아니다.** 정본은
//! [공유 저장·전이 계약](../../../spec/GIL_Storage_Transition_Contract_v0.1.md) §5.1 의 `events` 「추가만 한다」와 사건 칸 표,
//! §5.3 의 저장 1·저장 2, §5.7 의 T1·T6·T7, §6.3 의 재시도, §6.6 이다. 범위 설명은
//! [구현 순서](../../../spec/GIL_Implementation_Order_2026-11-19.md)의 「3i」 기록에 있다.
//!
//! ```text
//! (앞 EventCycleFacts*, 앞 &[Chain], 앞 등록부) ─┬─ check_cycle_refs (3h, 그대로) ────────────▶ BeforeRefs 갈래
//! (뒤 EventCycleFacts*, 뒤 &[Chain], 뒤 등록부) ─┼─ check_cycle_refs (3h, 그대로) ────────────▶ AfterRefs 갈래
//!                                                ├─ check_replaced_baselines (3g → 3f, 그대로) ─▶ Baselines 갈래
//!                                                └─ 3f 의 입력 대응이 섰을 때만:
//!                                                   앞 사건마다 chain_ref·after_cycle_ref·next_cycle_id ─▶ PastApproval 갈래
//!                                                             │
//!                                     check_event_cycle_history ─▶ EventCycleHistoryKept | [EventCycleHistoryProblem]
//! ```
//!
//! # 왜 두 시점인가
//!
//! 3h([`check_cycle_refs`])는 한 시점에서 실제 사이클 참조의 존재·닫힘·소속과 발급 번호 관계를 본다. 두 시점이 각각 그 검사를
//! 통과해도, 이미 기록된 사건의 승인 당시 값이 **다른 유효한 값**으로 바뀌었는지는 보이지 않는다 — 예컨대 `cycle:C4` 에서 승인한
//! 사건의 경계를 나중에 닫힌 `cycle:C5` 로 고친 기록은 뒤 시점에서도 T7 을 지킨다. 이 검사는 그 차이만 본다.
//!
//! # 계약에 적힌 조건과 도출한 조건
//!
//! - 적힌 것: 사건 원장은 추가만 한다(§5.1, T1). 저장 1 이 승인 시점의 사건을 만든다 — `after_cycle_ref` 는 승인을 **저장한**
//!   경계, `next_cycle_id` 는 승인을 저장할 **때의** 발급기 값이다(§5.1 칸 표, §5.3). 저장 2 는 나중에 `baseline` 을 적는다(§5.3).
//!   보관을 재시도하면 보관 경계는 지금 서 있는 경계라 승인 경계와 다를 수 있고, 그 값은 `baseline` 에 적히며 사건은 승인 경계에
//!   「고정」된 채 남는다(§6.3, §6.5 (가), §5.8).
//! - 도출한 것: 앞선 사건을 고치는 정본의 전이는 `baseline` 에 대한 둘(실패 시도 기록 추가, 저장 2 의 최초 확정)뿐이고, 어느 것도
//!   사건의 세 칸을 채우거나 지우거나 바꾸지 않는다. 그래서 앞 시점에 있던 사건마다 세 칸은 뒤 시점에서 같다. `after_cycle_ref` 는
//!   칸 없음 / `null` / 실제 참조를 구분한 채 견준다.
//!
//! # 새 규칙을 만들지 않는다
//!
//! 한 시점의 참조·발급 번호 관계는 3h, `baseline` 의 허용된 변경(실패 시도 추가, 최초 확정, 다른 경계에서의 보관)과 이미 대체된
//! 사건의 기준점 추가 금지는 3g [`check_replaced_baselines`](그 안의 3f [`check_baseline_history`](super::check_baseline_history))를
//! 그대로 부른다. `baseline` 을 이 검사가 다시 비교하지 않으므로 전체 [`EventCycleFacts`] 의 같음을 요구하지 않는다. 사건의 대응은
//! 3f 의 대응 규칙([`match_events`])을 그대로 쓰고, 대응이 서지 않으면 세 칸을 비교하지 않는다 — 그 실패는 3g 갈래 안의 3f 대응
//! 실패로 드러나므로 결과는 성공이 될 수 없다. 새 사건이 몇 개 붙든, 새 사건이 과거 경계를 가리키거나 같은 경계를 되풀이하든 이
//! 검사는 막지 않는다(그 사건의 한 시점 관계는 뒤 시점의 3h 가 본다). `next_cycle_id` 의 보존은 **같은 사건**의 과거 기록에 대한
//! 것이며, 새 사건의 발급 번호가 커질 수 있다는 규칙(3h 의 비감소)과 다른 것이다.
//!
//! # 「판단하지 않음」은 그대로 남는다
//!
//! 성공 값은 앞·뒤 시점의 [`CycleRefsAgree`] 와 [`ReplacedBaselinesKept`] 를 그대로 지닌다 — 3h 의 `null`·칸 없음과 `chain_ref`
//! 미조회, 3g 의 대체된 사건에 뒤늦게 붙은 실패 기록이 숨지 않는다. 세 칸의 `null`·칸 없음이 두 시점에서 같다는 것은 **보존**일
//! 뿐이며 그 값이 그 종류의 사건에서 허용된다는 판단이 아니다. 검사하지 않은 것은 [`APPROVAL_HISTORY_DOES_NOT_SHOW`] 에 있다.

use std::fmt;

use super::super::SnapshotRegistry;
use super::baseline_history::{boundary_text, match_events};
use super::baseline_position::{ReplacedBaselineProblem, ReplacedBaselinesKept, check_replaced_baselines};
use super::baseline_record::{BaselineFacts, BaselineSlot};
use super::event_cycles::{CycleRefProblem, CycleRefsAgree, EventCycleFacts, check_cycle_refs};
use super::event_links::EventName;
use crate::Chain;
use crate::refs::{ChainRef, CycleRef};

/// 이 검사가 성공해도 **보이지 않은** 것.
pub(crate) const APPROVAL_HISTORY_DOES_NOT_SHOW: [&str; 11] = [
    "두 관측 사이에 바꿨다가 같은 값으로 되돌린 중간 수정 — 두 입력의 차이만 본다",
    "입력이 실제 사건 원장에서 빠짐없이 뽑혔는가 — 특히 앞 시점 목록의 끝이 잘려 건네지면 잘린 사건은 비교되지 않는다. 다른 검사(3c–3h)의 입력과 같은 상태에서 뽑혔는가도 보지 않는다. 후속 연결 계층의 책임",
    "같은 이름·같은 종류의 다른 사건으로 바꿔 넣었는가 — 세 칸이 같고 baseline 이 허용된 갱신의 꼴이면 구별하지 못한다",
    "사건 기록 전체의 보존 — review_cycle_ref·world_before_ref 는 입력에 없다. 정책 연결 칸의 보존은 3d(link_history), baseline 의 보존·허용된 갱신은 3f·3g 의 규칙 그대로다",
    "null·칸 없음의 종류별 허용 — 두 시점에서 같다는 것은 보존일 뿐 그 값의 유효성 승인이 아니다. 각 시점의 판단하지 않음은 성공 값의 before()·after()·baselines() 에 그대로 남는다",
    "Chain 목록 전체와 SnapshotRegistry 매핑의 보존 — 각 시점의 읽기 전용 조회만 쓴다(3h·3e)",
    "사건 저장 당시의 닫힘, 사건 발급 번호와 현재 프로젝트 발급기의 관계 — 3h 의 한계 그대로다",
    "보관 경계가 승인 경계와 다른 기록이 실제로 대기 중 인터뷰 뒤의 재시도였는가(§6.3 세부 전이), 두 관측 사이의 실제 실행 순서",
    "world_before_ref 와 마지막 확정 세계(T8), 현재 세계와 다음 Entry(§6), 다음 실험을 열 자격(T9)",
    "실제 승인·보관·복원과 그 권한, 저장 전환 동의(O-1), 즉시 보관 중단 해제(O-2)와 T12, 수동 편집의 BaseUndecided, Chain 종료와 미완료 사건의 이관(O-3) — 현재 계약 안의 과거 기록 보존만 보며 이관 정책을 고르지 않는다",
    "형식 전환·활성화·제품 연결 — 이 검사를 부르는 제품 경로가 없다",
];

// ── 승인 당시 칸의 변경 ─────────────────────────────────────────────────────

/// 저장 1 이 사건과 함께 적고 그 뒤 어떤 전이도 고치지 않는 세 칸.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ApprovalField {
    ChainRef,
    AfterCycleRef,
    NextCycleId,
}

impl ApprovalField {
    /// 계약 §5.1 의 칸 이름.
    pub(crate) fn name(self) -> &'static str {
        match self {
            ApprovalField::ChainRef => "chain_ref",
            ApprovalField::AfterCycleRef => "after_cycle_ref",
            ApprovalField::NextCycleId => "next_cycle_id",
        }
    }
}

/// 앞 시점에 있던 사건의 승인 당시 칸이 뒤 시점에서 다르다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ApprovalChange {
    /// 처음 기록한 체인이 다르다.
    Chain { event: EventName, before: ChainRef, after: ChainRef },
    /// 처음 기록한 승인 경계 칸이 다르다 — 칸 없음 / `null` / 실제 참조 사이의 모든 차이를 포함한다.
    Boundary { event: EventName, before: BaselineSlot<Option<CycleRef>>, after: BaselineSlot<Option<CycleRef>> },
    /// 승인을 저장할 때의 발급 번호가 다르다.
    Issuer { event: EventName, before: u32, after: u32 },
}

impl ApprovalChange {
    pub(crate) fn event(&self) -> EventName {
        match self {
            ApprovalChange::Chain { event, .. }
            | ApprovalChange::Boundary { event, .. }
            | ApprovalChange::Issuer { event, .. } => *event,
        }
    }

    pub(crate) fn field(&self) -> ApprovalField {
        match self {
            ApprovalChange::Chain { .. } => ApprovalField::ChainRef,
            ApprovalChange::Boundary { .. } => ApprovalField::AfterCycleRef,
            ApprovalChange::Issuer { .. } => ApprovalField::NextCycleId,
        }
    }
}

impl fmt::Display for ApprovalChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (event, field) = (self.event(), self.field().name());
        let (before, after) = match self {
            ApprovalChange::Chain { before, after, .. } => (before.to_string(), after.to_string()),
            ApprovalChange::Boundary { before, after, .. } => (boundary_text(before), boundary_text(after)),
            ApprovalChange::Issuer { before, after, .. } => (before.to_string(), after.to_string()),
        };
        write!(f, "{event} 의 {field} 가 {before} 에서 {after} 로 바뀌었다. 승인 당시의 기록은 고치지 않는다")
    }
}

/// 앞선 사건 하나의 세 칸을 견준다. 짝은 3f 의 대응 규칙이 정했다.
fn compare(earlier: &EventCycleFacts, later: &EventCycleFacts, changes: &mut Vec<ApprovalChange>) {
    let event = earlier.event();
    if later.chain() != earlier.chain() {
        changes.push(ApprovalChange::Chain { event, before: earlier.chain(), after: later.chain() });
    }
    if later.boundary() != earlier.boundary() {
        changes.push(ApprovalChange::Boundary { event, before: earlier.boundary(), after: later.boundary() });
    }
    if later.issuer() != earlier.issuer() {
        changes.push(ApprovalChange::Issuer { event, before: earlier.issuer(), after: later.issuer() });
    }
}

// ── 두 시점 비교 ───────────────────────────────────────────────────────────

/// 두 시점 비교가 실패한 갈래. 앞의 세 갈래는 기존 검사의 결과를 그대로 담는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EventCycleHistoryProblem {
    /// 변경 전 입력이 한 시점 검사(3h)를 통과하지 못했다.
    BeforeRefs(Vec<CycleRefProblem>),
    /// 변경 후 입력이 한 시점 검사(3h)를 통과하지 못했다.
    AfterRefs(Vec<CycleRefProblem>),
    /// `baseline` 의 두 시점 비교(3g, 그 안의 3f — 입력 대응 포함)가 맞지 않는다.
    Baselines(Vec<ReplacedBaselineProblem>),
    /// 앞선 사건의 승인 당시 칸이 바뀌었다. 사건 순서, 사건 안에서는 `chain_ref`·`after_cycle_ref`·`next_cycle_id` 순.
    PastApproval(Vec<ApprovalChange>),
}

impl fmt::Display for EventCycleHistoryProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn listed<T: ToString>(items: &[T]) -> String {
            items.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
        }
        match self {
            EventCycleHistoryProblem::BeforeRefs(found) => write!(f, "변경 전 입력의 사이클 참조가 맞지 않는다: {}", listed(found)),
            EventCycleHistoryProblem::AfterRefs(found) => write!(f, "변경 후 입력의 사이클 참조가 맞지 않는다: {}", listed(found)),
            EventCycleHistoryProblem::Baselines(found) => write!(f, "기준점 기록의 두 시점 비교가 맞지 않는다: {}", listed(found)),
            EventCycleHistoryProblem::PastApproval(found) => write!(f, "앞선 사건의 승인 당시 기록이 바뀌었다: {}", listed(found)),
        }
    }
}

/// 건넨 두 시점이 각각 3h 를 통과하고, `baseline` 의 두 시점 비교(3g·3f)를 통과하며, 앞 시점에 있던 사건마다 `chain_ref`·
/// `after_cycle_ref`·`next_cycle_id` 가 뒤 시점에서 같다 — **그것뿐이다.** 보이지 않은 것은 [`APPROVAL_HISTORY_DOES_NOT_SHOW`] 에
/// 있다. 이 값을 만드는 문은 [`check_event_cycle_history`] 하나뿐이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EventCycleHistoryKept {
    before: CycleRefsAgree,
    after: CycleRefsAgree,
    baselines: ReplacedBaselinesKept,
    compared: Vec<EventName>,
}

impl EventCycleHistoryKept {
    /// 변경 전 시점의 3h 성공 값 그대로 — 그 시점의 판단하지 않음(`not_judged`·`chains_not_looked_up`)을 지닌다.
    pub(crate) fn before(&self) -> &CycleRefsAgree {
        &self.before
    }

    /// 변경 후 시점의 3h 성공 값 그대로.
    pub(crate) fn after(&self) -> &CycleRefsAgree {
        &self.after
    }

    /// 3g 의 성공 값 그대로 — 대체된 사건에 뒤늦게 붙은 실패 기록(`unjudged_late_attempts`)을 판단하지 않은 채 지닌다.
    pub(crate) fn baselines(&self) -> &ReplacedBaselinesKept {
        &self.baselines
    }

    /// 세 칸이 같음을 확인한 앞선 사건, 사건 순서대로. 그 값이 그 종류에서 허용되는지는 판단하지 않았다.
    pub(crate) fn compared(&self) -> &[EventName] {
        &self.compared
    }

    /// 뒤 시점의 끝에 덧붙은 사건(3f 의 값). 세 칸은 뒤 시점의 3h 만 받았다.
    pub(crate) fn added(&self) -> &[EventName] {
        self.baselines.history().added()
    }
}

/// 변경 전후의 (사건의 사이클 칸 목록, 그 시점의 Chain 목록, 그 시점의 등록부) 두 쌍을 비교한다. **순수 함수**다.
///
/// 각 시점에 3h 의 [`check_cycle_refs`] 를, 두 시점의 `baseline` 에 3g 의 [`check_replaced_baselines`] 를 그대로 부르고 그 실패를
/// 갈래마다 담는다. 3f 의 대응 규칙으로 앞 사건과 뒤 사건의 짝이 서면 — 앞·뒤 시점 검사의 실패와 무관하게 — 앞 사건마다 세 칸을
/// 견주고 바뀐 칸을 [`EventCycleHistoryProblem::PastApproval`] 로 돌려준다. 짝이 서지 않으면 세 칸을 비교하지 않으며, 그 사실은
/// [`EventCycleHistoryProblem::Baselines`] 안의 3f 대응 실패로 드러난다. 갈래 순서는 변경 전, 변경 후, 기준점, 승인 당시 칸이다.
/// 입력·체인·등록부를 바꾸지 않고 파일·상태를 읽거나 쓰지 않는다.
pub(crate) fn check_event_cycle_history(
    before: (&[EventCycleFacts], &[Chain], &SnapshotRegistry),
    after: (&[EventCycleFacts], &[Chain], &SnapshotRegistry),
) -> Result<EventCycleHistoryKept, Vec<EventCycleHistoryProblem>> {
    let (before_events, before_chains, before_registry) = before;
    let (after_events, after_chains, after_registry) = after;
    let mut problems = Vec::new();

    let before_refs = check_cycle_refs(before_events, before_chains, before_registry);
    if let Err(found) = &before_refs {
        problems.push(EventCycleHistoryProblem::BeforeRefs(found.clone()));
    }
    let after_refs = check_cycle_refs(after_events, after_chains, after_registry);
    if let Err(found) = &after_refs {
        problems.push(EventCycleHistoryProblem::AfterRefs(found.clone()));
    }

    let before_baselines: Vec<BaselineFacts> = before_events.iter().map(|facts| facts.baseline().clone()).collect();
    let after_baselines: Vec<BaselineFacts> = after_events.iter().map(|facts| facts.baseline().clone()).collect();
    let baselines = check_replaced_baselines((&before_baselines, before_registry), (&after_baselines, after_registry));
    if let Err(found) = &baselines {
        problems.push(EventCycleHistoryProblem::Baselines(found.clone()));
    }

    // 짝은 3f 의 규칙 그대로 — 서면 앞 사건의 자리마다 같은 자리의 뒤 사건이 같은 이름·종류로 있다.
    let mut compared = Vec::new();
    if match_events(&before_baselines, &after_baselines).is_ok() {
        let mut changes = Vec::new();
        for (earlier, later) in before_events.iter().zip(after_events) {
            compare(earlier, later, &mut changes);
            compared.push(earlier.event());
        }
        if !changes.is_empty() {
            problems.push(EventCycleHistoryProblem::PastApproval(changes));
        }
    }

    match (problems.is_empty(), before_refs, after_refs, baselines) {
        (true, Ok(before), Ok(after), Ok(baselines)) => Ok(EventCycleHistoryKept { before, after, baselines, compared }),
        _ => Err(problems),
    }
}
