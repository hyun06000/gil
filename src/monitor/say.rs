//! Monitor 가 사람에게 쓰는 **낱말** — 두 renderer 가 함께 읽는 한 자리.
//!
//! plain text 와 HTML 은 모양이 다르지만 **같은 사실을 같은 말로** 불러야 한다. 상태와
//! 관계의 문장을 renderer 마다 따로 적으면, 같은 `verdict` 가 화면에 따라 다른 뜻으로
//! 읽히기 시작하고 그때부터 두 화면은 서로 다른 진실원이 된다.
//!
//! 여기 있는 것은 **낱말뿐이다.** 어느 절에 무엇을 싣고 어떻게 배치할지는 renderer 가
//! 정한다 — 그것이 두 표현이 갈라져도 되는 자리다(Monitor Model §7.1).

use crate::cycle::CycleKind;
use crate::node::NodeStatus;

use super::{ActionKind, CycleRelation, WorldMark};

// ── 절의 이름 ──────────────────────────────────────────────────────────────
//
// Every word comes from the language tables (`mon.*`) in the current locale; the two
// renderers call these functions instead of constants so one table change reaches both.

use crate::locale::text;
use crate::tr;

pub(super) fn active_path() -> &'static str { text("mon.active_path") }
pub(super) fn left_behind() -> &'static str { text("mon.left_behind") }
pub(super) fn doing() -> &'static str { text("mon.doing") }
pub(super) fn world() -> &'static str { text("mon.world") }
pub(super) fn next() -> &'static str { text("mon.next") }

/// 지금 자리의 절 이름 — **Interview 를 「현재 실험」이라 부르지 않는다.**
pub(super) fn here_title(kind: CycleKind) -> &'static str {
    match kind {
        CycleKind::Experiment => text("mon.here_experiment"),
        CycleKind::Interview => text("mon.here_interview"),
    }
}

// ── 이름표 ─────────────────────────────────────────────────────────────────

pub(super) const CYCLE: &str = "Cycle";
pub(super) const STEP: &str = "Step";
pub(super) fn problem() -> &'static str { text("mon.problem") }
pub(super) fn success_condition() -> &'static str { text("mon.success_condition") }
pub(super) fn verdict() -> &'static str { text("mon.verdict") }
pub(super) fn lesson() -> &'static str { text("mon.lesson") }
pub(super) fn handoff() -> &'static str { text("mon.handoff") }
pub(super) fn next_direction() -> &'static str { text("mon.next_direction") }
pub(super) fn why() -> &'static str { text("mon.why") }
pub(super) fn parent() -> &'static str { text("mon.parent") }
pub(super) fn branch_source() -> &'static str { text("mon.branch_source") }
pub(super) fn relation() -> &'static str { text("mon.relation") }
pub(super) fn objective() -> &'static str { text("mon.objective") }
pub(super) fn next_action() -> &'static str { text("mon.next_action") }
pub(super) fn done_when() -> &'static str { text("mon.done_when") }
pub(super) fn baseline() -> &'static str { text("mon.baseline") }
pub(super) fn world_state() -> &'static str { text("mon.world_state") }
pub(super) fn why_unknown() -> &'static str { text("mon.why_unknown") }
pub(super) fn reason() -> &'static str { text("mon.reason") }
pub(super) fn run() -> &'static str { text("mon.run") }
pub(super) fn help() -> &'static str { text("mon.help") }

/// 아직 아무 자리도 열지 않았다는 사실 — 다음에 무엇을 할지와 곧바로 이어진다.
pub(super) fn no_step_yet() -> &'static str { text("mon.no_step_yet") }
/// 밟을 수 있는 GIL 명령이 하나도 없다는 사실.
pub(super) fn no_move() -> &'static str { text("mon.no_move") }

/// 아직 실험이 정의되지 않았다 — **없는 질문을 지어내지 않는다.**
///
/// **Experiment 에만 쓴다.** Interview 에 이 말을 붙이면 이미 닫힌 Question 이 있는데도
/// 화면이 「질문이 없다」고 말하게 된다 — 실제로 그렇게 말한 적이 있다.
pub(super) fn no_definition() -> &'static str { text("mon.no_definition") }

/// Interview 가 아직 Question 을 열지 않았다.
pub(super) fn not_asked() -> &'static str { text("mon.not_asked") }
/// Question 이 열려 있고 질문 글은 닫을 때 적힌다.
pub(super) fn asking() -> &'static str { text("mon.asking") }
/// 사람이 준 답. 질문 아래 붙는다.
pub(super) fn response() -> &'static str { text("mon.response") }

/// 지금 걸린 행동이 없다.
pub(super) fn no_will() -> &'static str { text("mon.no_will") }

/// Cycle Report 에 적힌 **다음 방향의 낱말**을 사람의 말로.
///
/// `open_child` 나 `revisit` 은 저장된 값의 이름이지 사람의 말이 아니다. 판독 실험 1 에서
/// 사람은 바로 이 낱말을 보고 **지금 할 일로 오인했다.** 모르는 낱말은 지어내지 않고
/// 그대로 둔다 — 없는 뜻을 붙이는 것이 더 나쁘다.
pub(super) fn direction_of(action: &str) -> &str {
    match action {
        "open_child" => text("mon.direction.open_child"),
        "revisit" => text("mon.direction.revisit"),
        "close_chain" => text("mon.direction.close_chain"),
        "close_cycle" => text("mon.direction.close_cycle"),
        other => other,
    }
}
/// 활성 경로가 무엇인지 한 줄로.
pub(super) fn active_path_is() -> &'static str { text("mon.active_path_is") }
/// 지나온 갈래가 무엇인지 한 줄로.
pub(super) fn left_behind_is() -> &'static str { text("mon.left_behind_is") }
/// 갈래의 출처가 계보의 변이 아니라는 사실 — 두 화면 모두 이것을 말한다.
pub(super) fn not_a_lineage_edge() -> &'static str { text("mon.not_a_lineage_edge") }
/// 지금 Cycle 과 부모가 같다는 사실.
pub(super) fn same_parent() -> &'static str { text("mon.same_parent") }
/// 지금 열린 자리가 세계를 확정할 수 있다는 사실.
pub(super) fn verify_can_confirm() -> &'static str { text("mon.verify_can_confirm") }

// ── 상태와 관계 ────────────────────────────────────────────────────────────
//
// **색으로 가르지 않는다.** 어느 renderer 에서도 이 낱말이 화면에 그대로 있어야 하고,
// 그래서 상태를 아는 데 색이나 도형이 필요하지 않다.

pub(super) fn state_of(state: NodeStatus) -> &'static str {
    match state {
        NodeStatus::Open => text("mon.state.open"),
        NodeStatus::Closed => text("mon.state.closed"),
    }
}

pub(super) fn world_of(state: WorldMark) -> &'static str {
    match state {
        WorldMark::Clean => text("mon.world.clean"),
        WorldMark::Dirty => text("mon.world.dirty"),
        WorldMark::Unknown => text("mon.world.unknown"),
    }
}

/// 계보 밖 Cycle 이 지금 자리와 맺는 관계 — **셋을 서로 다른 말로.**
///
/// enum 이름을 그대로 내보이지 않는다. 사람이 읽는 것은 관계이지 Rust 의 variant 가 아니다.
pub(super) fn relation_of(relation: CycleRelation) -> &'static str {
    match relation {
        CycleRelation::RevisitSource => text("mon.relation.revisit_source"),
        CycleRelation::Abandoned => text("mon.relation.abandoned"),
        // **Graph 가 끊겼다는 뜻이 아니다.** 두고 온 가지의 자손처럼, 같은 Graph 안에 있되
        // 지금 걷는 길과 곧바로 맞닿지 않는 자리다.
        CycleRelation::Other => text("mon.relation.other"),
    }
}

/// 그 수가 무엇을 하는가 — **variant 이름이 아니라 사람의 말로.**
pub(super) fn what_it_does(kind: ActionKind) -> String {
    match kind {
        ActionKind::CloseStep(step) => tr!("mon.does.close_step", step = step),
        ActionKind::OpenStep(step) => tr!("mon.does.open_step", step = step),
        ActionKind::OpenChain => tr!("mon.does.open_chain"),
        ActionKind::CloseChain => tr!("mon.does.close_chain"),
        ActionKind::CloseCycle(cycle) => tr!("mon.does.close_cycle", cycle = cycle),
        ActionKind::OpenCycle(cycle) => tr!("mon.does.open_cycle", cycle = cycle),
        ActionKind::OpenBranch(cycle) => tr!("mon.does.open_branch", cycle = cycle),
        ActionKind::Revisit => tr!("mon.does.revisit"),
        ActionKind::Restore => tr!("mon.does.restore"),
    }
}
