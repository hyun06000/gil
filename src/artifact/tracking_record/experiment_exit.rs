//! **닫힌 Experiment T 의 Exit 세계와 그 확정 정책** 을 건넨 기록에서 함께 유도하는 순수 선택 — 계약 §6.1 의 「Experiment 의 Verify·Exit
//! → 그 실험을 열 때의 적용 정책」 가운데 **Exit** 한 줄, 제품 미연결.
//!
//! 이 모듈은 §6.1 의 확정 정책 표에서 닫힌 Experiment 의 Exit 만 옮긴다. **§6.1 전체(Verify 의 확정 정책, 열린 사이클의 Verify 계보,
//! Interview 의 Entry 계승, 마지막 확정 세계), B(T)·S(N) 의 `ExitPolicyNotDerived` 대체, 검증 ①·②, T8·T9, 제품 연결의 완료가 아니다.**
//! 정본은 [공유 저장·전이 계약](../../../spec/GIL_Storage_Transition_Contract_v0.1.md) §6.1(확정 정책 표, 「정책은 사이클 사이에서만
//! 바뀌므로 한 실험의 생애에는 정책이 하나다」, `exit_snapshot_ref` 는 한 번 쓰면 바꾸지 않는다)과 §6.2 의 P(N), §5.1(판 이름과 정체성의
//! 구분), §5.5(정책을 세계 등록부에 적지 않는다 — 관측 근거는 확정한 자리가 지닌다), 그리고
//! [Artifact Model](../../../spec/GIL_Artifact_Model_v0.1.md) §7.2·§7.5(Cycle 의 Exit 필드)다.
//!
//! ```text
//! 확정 정책 — Experiment 의 Exit    그 실험을 열 때의 적용 정책 (§6.2 의 P)
//!
//! 대상 CycleRef + (PolicyLedger, EventPolicyLinks, EventCycleFacts*, &[Chain], 등록부)
//!     ─ select_opening_policy (P(T), 그대로 한 번 — 그 안의 3j 와 대상 위치 확인) ───────────▶ Inputs / Target 갈래 · OpeningPolicy
//! 대상 CycleRef + &[Chain] ─ Chain::id · Cycles::nodes · Cycle::kind · Cycle::is_closed · Cycle::exit_snapshot (읽기 전용)
//!     ──────────────────────────────────────────────────────────────────────────────────────▶ Target / InterviewOutOfScope / Experiment 갈래
//!                                   │ 유일한 닫힌 Experiment 이고 Exit 칸이 있을 때만 (P(T) 의 결과와 무관하게)
//!                                   ▼
//! T 의 exit_snapshot_ref ─ 건넨 등록부에서 이름 조회 ───────────────────────────────────────────▶ ExitNotInRegistry 갈래
//!                                   │ 모든 갈래가 성립할 때만
//!                                   ▼
//! ExperimentExit { T, 소유 Chain, Exit 세계, P(T) 그대로 }
//! ```
//!
//! # 무엇을 고르는가
//!
//! - **세계는 T 의 `exit_snapshot_ref` 그대로다.** B(T)(T 경계에서 나중에 보관한 사건 기준점), S(N), 지금 서 있는 경계의 현재 세계,
//!   T 의 Entry, 부모·자식·`revisit_from` 의 Exit 으로 대신하지 않는다. 사건은 과거의 Exit 을 고치지 않고 그 옆에 선다(§6.1). 그래서 T 를
//!   닫은 뒤 T 에서 보관한 기준점이 있어도 이 결과의 세계와 정책은 바뀌지 않는다.
//! - **확정 정책은 같은 호출의 P(T) 결과 그대로다.** [`select_opening_policy`] 를 같은 입력으로 **한 번** 부르고, 그 선택
//!   ([`OpeningSelection`] — 사건 이름·종류·판 이름·승인 당시 `next_cycle_id`, 또는 건넨 이력 안의 정책 채택 전)을 바꾸지 않고 담는다.
//!   P 를 고르는 규칙을 여기서 다시 쓰지 않는다. 현재 원장의 마지막 판, 보관 발급기(`baseline.next_cycle_id`), 사건의 기준점, 세계 이름,
//!   원문 지문으로 정책을 다시 고르지 않는다. 같은 원문의 `P1`·`P3` 은 다른 판이다(§5.1).
//! - **판정으로 거르지 않고 저장된 Exit 을 읽는다.** `success`·`failure` 어느 쪽으로 닫혔든 T 에 적힌 `exit_snapshot_ref` 를 그대로 쓴다.
//!   Exit 은 Artifact Model §7.5 의 계보 규칙을 따르며 그 계보에 Verify 가 없으면 Entry 를 계승한다. **이 함수는 그 유도의 정합성을
//!   검사하지 않는다.** 한 실험의 생애에는 정책이 하나이므로(§6.1) 확정 정책은 P(T) 다. 실패한 실험의 Exit 이 현재 세계로 채택됐다는
//!   뜻은 아니다.
//!
//! # 입력과 검사
//!
//! 3j([`check_event_inputs`](super::check_event_inputs))는 [`select_opening_policy`] **안에서만** 같은 호출의 입력에 한 번 실행된다. 이 모듈은
//! 3j 를 따로 부르지 않고, 다른 입력에서 얻은 성공 값([`OpeningPolicy`]·`EventInputsAgree`)을 받는 문도 없다. P(T) 의 실패는 갈래를 그대로
//! 옮긴다 — `Inputs` 는 [`ExperimentExitProblem::Inputs`], 대상 위치(`Target`)는 [`ExperimentExitProblem::Target`].
//!
//! 이 선택은 대상을 **스스로도** 건넨 `Chain` 목록에서 찾는다 — P(T) 와 같은 소유 규칙([`Chain::owns`] 와 같은 사이클 목록 조회, 첫 항목을
//! 고르지 않음)으로 실재·유일한 소유를 확인한 뒤 실제 [`CycleKind`]·닫힘·Exit 을 읽고, 그 Exit 이름을 건넨 등록부에서 조회한다. 이 조회는
//! P(T) 의 결과와 무관하게 평가하고, 실패는 입력 갈래와 **함께** 돌려준다. 소유 문제는 P(T) 가 같은 규칙으로 이미 돌려준 것과 같은
//! 사실이므로 한 번만 담는다.
//!
//! - Interview 는 **명시적 범위 밖**이다([`ExperimentExitProblem::InterviewOutOfScope`]) — §6.1 의 「Interview 의 Exit → 그 Entry 의 확정
//!   정책을 물려받는다」는 이 선택이 유도하지 않는다. 정책 없음·정책 채택 전·성공 어느 것도 아니다. 열렸든 닫혔든 같다.
//! - 열린 Experiment 는 Exit 이 없다([`ExperimentTargetProblem::NotClosed`]). 닫혔다고 적혔는데 Exit 이 없는 값
//!   ([`ExperimentTargetProblem::ClosedWithoutExit`])은 기존 전이와 `Project` load 가 지키는 짝이 깨진 입력이며 대신할 세계를 고르지 않는다.
//! - Exit 이름은 건넨 등록부에서 조회되어야 한다([`ExperimentExitProblem::ExitNotInRegistry`]). 대상이 유일한 닫힌 Experiment 이고 Exit
//!   칸이 있을 때만 조회하며, 입력 검사(3j)의 실패와 무관하게 평가해 함께 돌려준다 — 입력 오류를 Exit 누락으로 바꾸지도, Exit 누락을
//!   입력 오류 뒤에 숨기지도 않는다. 없는·모호한 대상, Interview, 열린 Experiment, Exit 칸이 없는 대상에서는 세계 이름을 추정하거나 다른
//!   이름을 대신 조회하지 않는다.
//!
//! # 결과가 말하는 것
//!
//! [`ExperimentExit`] 는 T·소유 Chain·Exit 세계, 그리고 같은 호출의 [`OpeningPolicy`] 를 그대로 지닌다 — 그 사건·종류·판·승인 발급기
//! 출처와 `inputs()` 의 3j 판단하지 않음(`not_judged`·`chains_not_looked_up`)이 숨지 않는다. [`ExperimentExit::policy`] 의
//! [`OpeningSelection::BeforeAdoptionInGivenHistory`] 는 **건넨 이력 안에서** T 를 열 때 적용된 정책이 없었다는 뜻이며, 확정 정책을
//! 유도하지 않았다는 B(T)·S(N) 의 `ExitPolicyNotDerived` 와도, 입력 오류·대상 오류와도 다른 값이다.
//!
//! 성공은 **건넨 기록 안의 유도**다. Exit 이 올바른 Verify 계보에서 계산됐는지, T 의 개방이 검사 ② 를 만족했는지, 전체 상태가
//! 유효한지, 이력이 완전한지, 실제 승인·보관이 성공했는지는 증명하지 않는다. 보이지 않는 것은 [`EXPERIMENT_EXIT_DOES_NOT_SHOW`] 에 있다.

use std::fmt;

use super::super::SnapshotRegistry;
use super::event_cycles::EventCycleFacts;
use super::event_inputs::{EventInputsAgree, EventInputsProblem};
use super::event_links::EventPolicyLinks;
use super::opening_policy::{OpeningPolicy, OpeningPolicyProblem, OpeningSelection, TargetProblem, select_opening_policy};
use super::PolicyLedger;
use crate::refs::{ChainRef, CycleRef, SnapshotRef};
use crate::{Chain, Cycle, CycleKind};

/// 이 유도가 성공해도 **보이지 않는** 것 — 결과를 들고 있어도 이것들은 성립하지 않는다.
pub(crate) const EXPERIMENT_EXIT_DOES_NOT_SHOW: [&str; 13] = [
    "입력의 출처와 완전성 — 건넨 원장·사건 입력·Chain 목록·등록부가 실제 프로젝트의 같은 시점에서 빠짐없이 뽑혔는지 보지 않는다. 원장과 두 사건 입력을 함께 잘라낸 입력은 알아내지 못하며 잘린 이력 안에서 P(T) 를 고른다. 3j 의 EVENT_INPUTS_DO_NOT_SHOW 와 P(T) 의 OPENING_POLICY_DOES_NOT_SHOW 그대로다",
    "실제 승인·적용·보관 — P(T) 사건의 모니터 승인 응답과 승인 저장의 실제 성공, T 의 Verify 닫기(세계 확정)의 실제 성공을 확인하지 않는다. 성공은 건넨 기록 안의 유도다",
    "3j 의 판단하지 않음 — after_cycle_ref·baseline.after_cycle_ref 의 null·칸 없음(not_judged)과 chain_ref 미조회(chains_not_looked_up)는 판단하지 않은 채 성공 값의 inputs() 에 그대로 남는다",
    "Exit 의 계보 — T 의 exit_snapshot_ref 가 마지막 Outcome 의 구조적 lineage 에서 가장 가까운 Verify 로 계산됐는지(Artifact Model §7.5), 버려진 가지의 세계가 아닌지 보지 않는다. Verify 노드의 snapshot_ref 를 읽지 않는다",
    "T 의 개방 — T 를 열 때 검증 ①(Entry = S(T) 의 세계)·②(S(T) 의 확정 정책 = P(T))를 만족했는지, 보관 대기 뒤에 연 실험인지(T9) 보지 않는다. Exit 의 확정 정책 = P(T) 는 §6.1 의 정의를 옮긴 것이며 T 가 올바르게 열렸다는 증명이 아니다",
    "Verify 의 확정 정책과 열린 사이클 — 닫힌 T 의 Exit 만 다룬다. T 안의 개별 Verify 세계, 열린 Experiment 의 Verify 계보, 열린 사이클의 마지막 확정 세계는 유도하지 않는다",
    "B(T)·S(N)·지금 서 있는 경계 — 결과의 세계는 T 의 Exit 이며 B(T) 가 아니다. T 에서 나중에 보관한 기준점, 현재 세계, 되돌아가기의 복원 목표로 대신하지 않는다. B(T)·S(N) 의 ExitPolicyNotDerived 를 이 결과로 바꾸는 문은 없다",
    "Interview 의 Exit — 범위 밖 결과로 돌려준다. Entry 의 확정 정책을 물려받는 §6.1 의 줄은 후속이며, 정책 없음·정책 채택 전·성공 어느 것도 아니다",
    "판정과 채택 — success·failure 로 거르지 않는다. 실패한 실험의 Exit 이 현재 세계·다음 실험의 출처로 채택됐다는 뜻이 아니다",
    "세계의 실재 — Exit 이름이 건넨 등록부에서 조회되는지만 본다. manifest·blob 의 실재와 내용, T 의 Entry 의 등록은 보지 않는다",
    "전체 상태의 유효성 — 건넨 Chain 목록의 직렬 계보·Entry/Exit 인계·발급기 경계, T8(world_before_ref), 저장 load 의 다른 불변식을 대신 검사하지 않는다",
    "3n·3o·보관·재시도·복원·Monitor·CLI·MCP — 결과를 LedgerStart·ConfirmedStart·AppliedSpan 으로 바꾸는 문이 없고, 이 유도를 부르는 제품 경로가 없으며 파일·state.yaml·객체 창고를 읽거나 쓰지 않는다. 형식 10/11 은 활성화되지 않았다",
    "미결 결정 — 즉시 보관 중단의 요청·해제(O-2), 보관 대기 중 Chain 종료와 미완료 사건의 이관(O-3), BaseUndecided 의 나머지 조합에 아무 의미도 정하지 않는다",
];

// ── 대상 ───────────────────────────────────────────────────────────────────

/// 실재하고 하나로 정해진 Experiment 대상에서 Exit 을 읽을 수 없는 이유. **「정책 채택 전」도 「정책 없음」도 아니다.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExperimentTargetProblem {
    /// 그 Experiment 가 열려 있다 — Exit 이 아직 없다.
    NotClosed { cycle: CycleRef, chain: ChainRef },
    /// 그 Experiment 가 닫혔다고 적혔는데 `exit_snapshot_ref` 가 없다. 기존 전이와 저장 load 가 지키는 짝이 깨진 입력이며, 대신할 세계를
    /// 고르지 않는다.
    ClosedWithoutExit { cycle: CycleRef, chain: ChainRef },
}

impl fmt::Display for ExperimentTargetProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExperimentTargetProblem::NotClosed { cycle, chain } => {
                write!(f, "Experiment {cycle}({chain}) 가 열려 있다. 닫힌 Experiment 의 Exit 만 유도한다")
            }
            ExperimentTargetProblem::ClosedWithoutExit { cycle, chain } => write!(
                f,
                "Experiment {cycle}({chain}) 는 닫혔는데 exit_snapshot_ref 가 없다. 대신할 세계를 고르지 않는다"
            ),
        }
    }
}

/// 건넨 목록에서 읽은 닫힌 Experiment T.
struct Located {
    chain: ChainRef,
    exit: SnapshotRef,
}

/// 건넨 목록에서 대상을 소유한 유일한 체인과 그 사이클 — 체인 이름([`Chain::id`])과 사이클 목록만 읽는다. 소유 규칙은 P(T) 의 것과
/// 같고 첫 항목을 고르지 않는다. 같은 디렉터리의 Exit 계보 검사([`super::exit_lineage`])도 이 조회를 그대로 쓴다.
pub(super) fn owned_cycle(target: CycleRef, chains: &[Chain]) -> Result<(ChainRef, &Cycle), TargetProblem> {
    let owners: Vec<(ChainRef, &Cycle)> = chains
        .iter()
        .filter_map(|chain| {
            chain.cycles().nodes().iter().find(|cycle| cycle.id().to_ref() == target).map(|cycle| (chain.id(), cycle))
        })
        .collect();
    let (chain, cycle) = match owners.as_slice() {
        [] => return Err(TargetProblem::NotFound { cycle: target }),
        [(owner, cycle)] => (*owner, *cycle),
        _ => {
            let chains = owners.iter().map(|(chain, _)| *chain).collect();
            return Err(TargetProblem::SeveralOwners { cycle: target, chains });
        }
    };
    let count = chains.iter().filter(|other| other.id() == chain).count();
    if count != 1 {
        return Err(TargetProblem::OwnerChainRepeated { cycle: target, chain, count });
    }
    Ok((chain, cycle))
}

/// 대상을 소유한 체인에서 T 를 읽는다 — [`owned_cycle`] 의 소유 조회 뒤 종류·닫힘·Exit 만 읽는다.
fn locate(target: CycleRef, chains: &[Chain]) -> Result<Located, ExperimentExitProblem> {
    let (chain, cycle) = owned_cycle(target, chains).map_err(ExperimentExitProblem::Target)?;
    match cycle.kind() {
        CycleKind::Interview => return Err(ExperimentExitProblem::InterviewOutOfScope { cycle: target, chain }),
        CycleKind::Experiment => {}
    }
    if !cycle.is_closed() {
        return Err(ExperimentExitProblem::Experiment(ExperimentTargetProblem::NotClosed { cycle: target, chain }));
    }
    match cycle.exit_snapshot() {
        Some(exit) => Ok(Located { chain, exit }),
        None => Err(ExperimentExitProblem::Experiment(ExperimentTargetProblem::ClosedWithoutExit { cycle: target, chain })),
    }
}

// ── 결과 ───────────────────────────────────────────────────────────────────

/// 건넨 기록 안에서 유도한 닫힌 Experiment T 의 Exit 세계와 그 확정 정책 — **그것뿐이다.** 보이지 않는 것은
/// [`EXPERIMENT_EXIT_DOES_NOT_SHOW`] 에 있다. 이 값을 만드는 문은 [`select_experiment_exit`] 하나뿐이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExperimentExit {
    world: SnapshotRef,
    opening: OpeningPolicy,
}

impl ExperimentExit {
    /// 대상 Experiment T.
    pub(crate) fn target(&self) -> CycleRef {
        self.opening.target()
    }

    /// 건넨 목록에서 T 를 소유한 체인 — P(T) 의 소유 조회와 이 선택의 조회가 같은 체인을 가리킨다.
    pub(crate) fn chain(&self) -> ChainRef {
        self.opening.chain()
    }

    /// T 의 `exit_snapshot_ref` — 건넨 등록부에서 이름이 조회됐다. B(T) 나 현재 세계가 아니다.
    pub(crate) fn world(&self) -> SnapshotRef {
        self.world
    }

    /// Exit 의 확정 정책 — 같은 호출의 P(T) 선택 그대로(§6.1). 사건·종류·판 이름·승인 당시 `next_cycle_id`, 또는 건넨 이력 안의 정책
    /// 채택 전.
    pub(crate) fn policy(&self) -> OpeningSelection {
        self.opening.selection()
    }

    /// 같은 호출에서 고른 P(T) 그대로.
    pub(crate) fn opening(&self) -> &OpeningPolicy {
        &self.opening
    }

    /// P(T) 안에서 같은 호출에 실행한 3j 의 성공 값 그대로 — 그 판단하지 않음(`not_judged`·`chains_not_looked_up`)을 지닌다.
    pub(crate) fn inputs(&self) -> &EventInputsAgree {
        self.opening.inputs()
    }
}

/// 유도가 실패한 갈래. 갈래 순서는 입력 검사, 대상(소유·종류·닫힘·Exit 과 그 Exit 의 등록부 조회)이다. 대상 갈래는 입력 검사와 무관하게
/// 평가하므로 둘이 함께 올 수 있다. **어느 것도 정책 채택 전이나 Exit 정책 미유도가 아니다.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExperimentExitProblem {
    /// 같은 호출의 P(T) 안에서 건넨 입력이 3j 를 통과하지 못했다 — 그 실패 그대로.
    Inputs(Vec<EventInputsProblem>),
    /// 대상 사이클을 건넨 목록에서 하나로 정할 수 없다 — P(T) 와 같은 소유 규칙의 문제 그대로이며 한 번만 담는다.
    Target(TargetProblem),
    /// 대상이 Interview 다 — **범위 밖이며** 성공도 정책 채택 전도 아니다. 열렸든 닫혔든 같다.
    InterviewOutOfScope { cycle: CycleRef, chain: ChainRef },
    /// 대상 Experiment 에서 Exit 을 읽을 수 없다.
    Experiment(ExperimentTargetProblem),
    /// 대상이 유일한 닫힌 Experiment 이고 Exit 칸이 있는데 그 이름이 건넨 등록부에 없다. 입력 검사의 결과와 무관하게 평가한다.
    ExitNotInRegistry { cycle: CycleRef, chain: ChainRef, world: SnapshotRef },
}

impl fmt::Display for ExperimentExitProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExperimentExitProblem::Inputs(found) => write!(
                f,
                "건넨 사건 입력이 맞지 않아 Experiment 의 Exit 정책을 유도하지 않았다: {}",
                found.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
            ),
            ExperimentExitProblem::Target(problem) => write!(f, "Experiment 의 Exit 을 유도하지 않았다: {problem}"),
            ExperimentExitProblem::InterviewOutOfScope { cycle, chain } => write!(
                f,
                "{cycle}({chain}) 은 Interview 다. Interview 의 Exit 은 이 유도의 범위 밖이며 정책 없음도 정책 채택 전도 성공도 아니다"
            ),
            ExperimentExitProblem::Experiment(problem) => write!(f, "Experiment 의 Exit 을 유도하지 않았다: {problem}"),
            ExperimentExitProblem::ExitNotInRegistry { cycle, chain, world } => write!(
                f,
                "Experiment {cycle}({chain}) 의 Exit {world} 이 건넨 등록부에 없다. 대신할 세계를 고르지 않는다"
            ),
        }
    }
}

// ── 유도 ───────────────────────────────────────────────────────────────────

/// 건넨 기록에서 닫힌 Experiment `target` 의 Exit 세계와 그 확정 정책을 함께 유도한다. **순수 함수**다.
///
/// 같은 입력으로 [`select_opening_policy`] 를 한 번 부르고(그 안에서 3j 와 대상 위치를 확인한다), 대상의 소유·종류·닫힘·Exit 과 그 Exit 의
/// 등록부 조회를 그 결과와 무관하게 평가한다. 모두 성립할 때만 T 의 `exit_snapshot_ref` 와 P(T) 를 그대로 담는다. 입력·체인·등록부를
/// 바꾸지 않고 파일·상태·창고를 읽거나 쓰지 않는다.
pub(crate) fn select_experiment_exit(
    target: CycleRef,
    ledger: &PolicyLedger,
    links: &EventPolicyLinks,
    facts: &[EventCycleFacts],
    chains: &[Chain],
    registry: &SnapshotRegistry,
) -> Result<ExperimentExit, Vec<ExperimentExitProblem>> {
    let opening = select_opening_policy(target, ledger, links, facts, chains, registry);
    let located = locate(target, chains).and_then(|located| {
        if registry.resolve(located.exit).is_none() {
            Err(ExperimentExitProblem::ExitNotInRegistry {
                cycle: target,
                chain: located.chain,
                world: located.exit,
            })
        } else {
            Ok(located)
        }
    });

    let (opening, located) = match (opening, located) {
        (Ok(opening), Ok(located)) => (opening, located),
        (opening, located) => {
            let mut problems = Vec::new();
            if let Err(found) = opening {
                problems.extend(found.into_iter().map(|problem| match problem {
                    OpeningPolicyProblem::Inputs(found) => ExperimentExitProblem::Inputs(found),
                    OpeningPolicyProblem::Target(problem) => ExperimentExitProblem::Target(problem),
                }));
            }
            if let Err(problem) = located {
                // 같은 소유 규칙이므로 P(T) 가 돌려준 소유 문제와 같은 값이면 한 번만 담는다.
                if !problems.contains(&problem) {
                    problems.push(problem);
                }
            }
            return Err(problems);
        }
    };
    debug_assert_eq!(opening.chain(), located.chain, "P(T) 와 이 선택은 같은 소유 규칙으로 같은 체인을 읽는다");
    Ok(ExperimentExit { world: located.exit, opening })
}
