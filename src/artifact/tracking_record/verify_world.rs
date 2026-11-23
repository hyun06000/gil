//! **Experiment 안의 개별 닫힌 Verify 가 확정한 세계와 그 확정 정책** 을 건넨 기록에서 함께 유도하는 순수 선택 — 계약 §6.1 의
//! 「Experiment 의 Verify·Exit → 그 실험을 열 때의 적용 정책」 가운데 **지정한 Verify** 한 줄, 제품 미연결.
//!
//! 이 모듈은 §6.1 의 확정 정책 표에서 Experiment 의 **Verify** 만 옮긴다. 닫힌 Experiment 의 Exit 은 [`super::experiment_exit`] 가 맡는다.
//! **§6.1 전체(열린 사이클의 마지막 확정 세계·가장 가까운 Verify 를 찾는 계보 선택, Interview 의 Entry 계승), B(T)·S(N) 의
//! `ExitPolicyNotDerived` 대체, 검증 ①·②, T8·T9, 3n/3o 시작점, 제품 연결의 완료가 아니다.** 정본은
//! [공유 저장·전이 계약](../../../spec/GIL_Storage_Transition_Contract_v0.1.md) §6.1(확정 정책 표, 「정책은 사이클 사이에서만 바뀌므로 한
//! 실험의 생애에는 정책이 하나다」, Verify 의 `snapshot_ref` 는 한 번 쓰면 바꾸지 않는다)과 §6.2 의 P(N), §5.1(판 이름과 정체성의 구분),
//! 그리고 [Artifact Model](../../../spec/GIL_Artifact_Model_v0.1.md) §7.2–§7.4(닫힌 Verify 는 구조 필드 `snapshot_ref` 를 지니고, 바꾸지
//! 않은 Verify 도 기존 세계를 다시 가리킨다)다.
//!
//! ```text
//! 확정 정책 — Experiment 의 Verify    그 실험을 열 때의 적용 정책 (§6.2 의 P)
//!
//! 대상 StepRef(그 Cycle N) + (PolicyLedger, EventPolicyLinks, EventCycleFacts*, &[Chain], 등록부)
//!     ─ select_opening_policy (P(N), 그대로 한 번 — 그 안의 3j 와 대상 위치 확인) ────────────▶ Inputs / Target 갈래 · OpeningPolicy
//! 대상 StepRef + &[Chain] ─ experiment_exit::owned_cycle (같은 소유 규칙) · Cycle::kind · Cycle::steps · Cycle::step_ref
//!                           · StepNode::kind · StepNode::is_closed · StepNode::snapshot (읽기 전용, P(N) 의 결과와 무관하게)
//!     ──────────────────────────────────────────────────────────────────────────────────────▶ Target / Interview / Step 갈래
//!                                   │ 실재하는 Experiment 안의 실재하는 닫힌 Verify 이고 snapshot_ref 칸이 있을 때만
//!                                   ▼
//! 그 Verify 의 snapshot_ref ─ 건넨 등록부에서 이름 조회 ──────────────────────────────────────▶ SnapshotNotInRegistry 갈래
//!                                   │ 모든 갈래가 성립할 때만
//!                                   ▼
//! VerifyWorld { 대상 Step, 그 Cycle, 소유 Chain, 그 Verify 의 세계, P(N) 그대로 }
//! ```
//!
//! # 무엇을 고르는가
//!
//! - **세계는 지정한 Verify 자신의 `snapshot_ref` 그대로다.** 같은 Cycle 의 다른 Verify, 계보에서 가장 가까운 Verify, 그 Cycle 의 Entry·Exit,
//!   지금 서 있는 자리의 현재 세계([`Cycle::world_snapshot`](crate::Cycle::world_snapshot)), B(T)·S(N), 나중 사건 기준점으로 대신하지 않는다.
//!   바꾸지 않은 Verify 는 기존 세계를 다시 가리키며(§7.3), 같은 Snapshot 을 가리키는 서로 다른 Verify 는 서로 다른 결과다 — 대상 Step 이
//!   결과에 남는다.
//! - **역사적 조회다.** 지정한 닫힌 Verify 를 그대로 조회한다. 되돌아가기로 현재 계보에서 떨어진 가지의 Verify 도 그 자체의 세계·정책 출처를
//!   돌려준다. 그것이 현재 세계나 Cycle Exit 으로 채택됐다는 뜻은 아니다. 가장 가까운 Verify 를 찾는 계보 선택은 이 모듈에 없다.
//! - **확정 정책은 같은 호출의 P(N) 결과 그대로다.** N 은 대상 Step 이 담긴 Cycle 이다. [`select_opening_policy`] 를 같은 입력으로 **한 번**
//!   부르고, 그 선택([`OpeningSelection`] — 사건 이름·종류·판 이름·승인 당시 `next_cycle_id`, 또는 건넨 이력 안의 정책 채택 전)을 바꾸지
//!   않고 담는다. 한 실험의 생애에는 정책이 하나이므로(§6.1) 같은 Experiment 의 Verify 들은 같은 P(N) 을 지닌다. 현재 원장의 마지막 판, 보관
//!   발급기(`baseline.next_cycle_id`), 사건의 기준점, Snapshot 이름, 원문 지문으로 정책을 다시 고르지 않는다. 같은 원문의 `P1`·`P3` 은 다른
//!   판이다(§5.1).
//! - **소유 Experiment 의 열림·닫힘은 묻지 않는다.** 열린 Experiment 안의 닫힌 Verify 도 이미 세계를 확정했다.
//!
//! # 입력과 검사
//!
//! 3j([`check_event_inputs`](super::check_event_inputs))는 [`select_opening_policy`] **안에서만** 같은 호출의 입력에 한 번 실행된다. 이 모듈은
//! 3j 를 따로 부르지 않고, 다른 입력에서 얻은 성공 값([`OpeningPolicy`]·`EventInputsAgree`)을 받는 문도 없다. P(N) 의 실패는 갈래를 그대로
//! 옮긴다 — `Inputs` 는 [`VerifyWorldProblem::Inputs`], 대상 위치(`Target`)는 [`VerifyWorldProblem::Target`].
//!
//! 대상은 **스스로도** 건넨 `Chain` 목록에서 찾는다 — P(N)·[`select_experiment_exit`](super::select_experiment_exit) 와 같은 소유 조회
//! (`owned_cycle`)로 Cycle 의 실재·유일한 소유를 확인하고, 실제 [`CycleKind`]·Step 의 실재·종류·닫힘·`snapshot_ref` 칸을 읽은 뒤 그 이름을 건넨
//! 등록부에서 조회한다. 이 조회는 P(N) 의 결과와 무관하게 평가하고, 실패는 입력 갈래와 **함께** 돌려준다. 소유 문제는 P(N) 이 같은 규칙으로
//! 이미 돌려준 것과 같은 사실이므로 한 번만 담는다.
//!
//! - Interview 는 **명시적 범위 밖**이다([`VerifyWorldProblem::InterviewOutOfScope`]) — Interview 는 세계를 확정하지 않고, §6.1 의
//!   「Interview 의 Exit → 그 Entry 의 확정 정책」은 이 선택이 유도하지 않는다. 지정한 Step 의 실재와 무관하게 같다.
//! - Experiment 안에서 지정한 Step 이 없으면 [`StepProblem::NotFound`], Verify 가 아니면 [`StepProblem::NotVerifyOutOfScope`], 열린 Verify 면
//!   [`StepProblem::OpenVerifyOutOfScope`] 다. 앞의 둘·셋째는 **범위 밖이며 세계 조회를 하지 않는다** — 그 Step 에 `snapshot_ref` 칸이 적혀
//!   있어도(기존 복원 문이 받아도 현행 Project load 의 세계 검사 ④가 거절할 값) 확정 세계처럼 조회하거나 돌려주지 않는다.
//! - 닫힌 Verify 에 `snapshot_ref` 가 없으면 [`StepProblem::ClosedVerifyLacksSnapshot`] 이다(§7.3 이 허용하지 않는 손상). 다른 Verify·Entry 로
//!   대신하지 않는다.
//! - 그 이름이 건넨 등록부에 없으면 [`VerifyWorldProblem::SnapshotNotInRegistry`] 다.
//!
//! # 결과가 말하는 것
//!
//! [`VerifyWorld`] 는 대상 Step·그 Cycle·소유 Chain·그 Verify 의 세계, 그리고 같은 호출의 [`OpeningPolicy`] 를 그대로 지닌다 — 그 사건·종류·판·
//! 승인 발급기 출처와 `inputs()` 의 3j 판단하지 않음(`not_judged`·`chains_not_looked_up`)이 숨지 않는다. [`VerifyWorld::policy`] 의
//! [`OpeningSelection::BeforeAdoptionInGivenHistory`] 는 **건넨 이력 안에서** N 을 열 때 적용된 정책이 없었다는 뜻이며, B(T)·S(N) 의
//! `ExitPolicyNotDerived` 와도, 입력 오류·대상 오류·범위 밖과도 다른 값이다. 어떤 오류·범위 밖도 정책 채택 전으로 바뀌지 않는다.
//!
//! 성공은 **건넨 기록 안의 유도**다. 그 Verify 가 실제로 관측·보관됐는지, N 의 개방이 검사 ② 를 만족했는지, 실제 정책 승인이 있었는지,
//! 이력이 완전한지, 등록부의 이름이 실제 manifest·blob 으로 실재하는지는 증명하지 않는다. 보이지 않는 것은 [`VERIFY_WORLD_DOES_NOT_SHOW`] 에 있다.

use std::fmt;

use super::super::SnapshotRegistry;
use super::PolicyLedger;
use super::event_cycles::EventCycleFacts;
use super::event_inputs::{EventInputsAgree, EventInputsProblem};
use super::event_links::EventPolicyLinks;
use super::experiment_exit::owned_cycle;
use super::opening_policy::{OpeningPolicy, OpeningPolicyProblem, OpeningSelection, TargetProblem, select_opening_policy};
use crate::refs::{ChainRef, CycleRef, SnapshotRef, StepRef};
use crate::{Chain, CycleKind, NodeKind};

/// 이 유도가 성공해도 **보이지 않는** 것 — 결과를 들고 있어도 이것들은 성립하지 않는다.
pub(crate) const VERIFY_WORLD_DOES_NOT_SHOW: [&str; 12] = [
    "입력의 출처와 완전성 — 건넨 원장·사건 입력·Chain 목록·등록부가 실제 프로젝트의 같은 시점에서 빠짐없이 뽑혔는지 보지 않는다. 원장과 두 사건 입력을 함께 잘라낸 입력은 알아내지 못하며 잘린 이력 안에서 P(N) 을 고른다. 3j 의 EVENT_INPUTS_DO_NOT_SHOW 와 P(N) 의 OPENING_POLICY_DOES_NOT_SHOW 그대로다",
    "실제 관측·보관·승인 — 그 Verify 의 세계 확정(관측·manifest 생성·보관)과 P(N) 사건의 모니터 승인 응답·승인 저장의 실제 성공을 확인하지 않는다. 성공은 건넨 기록 안의 유도다",
    "3j 의 판단하지 않음 — after_cycle_ref·baseline.after_cycle_ref 의 null·칸 없음(not_judged)과 chain_ref 미조회(chains_not_looked_up)는 판단하지 않은 채 성공 값의 inputs() 에 그대로 남는다",
    "세계의 실재 — 그 Verify 의 snapshot_ref 이름이 건넨 등록부에서 조회되는지만 본다. manifest·blob 의 실재와 내용, 관측이 실제로 그 세계였는지, N 의 Entry 의 등록은 보지 않는다",
    "N 의 개방 — N 을 열 때 검증 ①(Entry = S(N) 의 세계)·②(S(N) 의 확정 정책 = P(N))를 만족했는지, 보관 대기 뒤에 연 실험인지(T9) 보지 않는다. Verify 의 확정 정책 = P(N) 은 §6.1 의 정의를 옮긴 것이며 N 이 올바르게 열렸다거나 실제 정책 승인이 있었다는 증명이 아니다",
    "계보와 채택 — 지정한 Verify 를 그대로 조회한다. 그 Verify 가 지금 서 있는 자리의 계보에 있는지, 현재 세계(Cycle::world_snapshot)·Cycle Exit·다음 실험의 출처로 채택됐는지 보지 않는다. 가장 가까운 Verify 를 찾는 계보 선택(열린 사이클의 마지막 확정 세계)은 하지 않는다",
    "다른 자리의 세계 — 같은 Cycle 의 다른 Verify, Entry·Exit, B(T)·S(N)·나중 사건 기준점·되돌아가기의 복원 목표와 견주거나 그것으로 대신하지 않는다. B(T)·S(N) 의 ExitPolicyNotDerived 를 이 결과로 바꾸는 문은 없다",
    "Interview 와 범위 밖 Step — Interview, 없는 Step, Verify 가 아닌 Step, 열린 Verify 는 범위 밖 결과로 돌려준다. Interview 의 Entry 계승·정책 상속은 후속이며, 범위 밖 Step 에 적힌 snapshot_ref 는 조회하지 않는다",
    "Verify 밖의 손상 — 지정한 Step 하나만 읽는다. 다른 Verify 의 snapshot 누락, Verify 가 아닌 Step 의 snapshot, 열린 Verify 의 snapshot 은 검사하지 않는다(현행 Project load 의 세계 검사 ④가 본다)",
    "전체 상태의 유효성 — 건넨 Chain 목록의 직렬 계보·Entry/Exit 인계·발급기 경계, T8(world_before_ref), 저장 load 의 다른 불변식을 대신 검사하지 않는다",
    "3n·3o·보관·재시도·복원·Monitor·CLI·MCP — 결과를 LedgerStart·ConfirmedStart·AppliedSpan 으로 바꾸는 문이 없고, 이 유도를 부르는 제품 경로가 없으며 파일·state.yaml·객체 창고를 읽거나 쓰지 않는다. 형식 10/11 은 활성화되지 않았다",
    "미결 결정 — 즉시 보관 중단의 요청·해제(O-2), 보관 대기 중 Chain 종료와 미완료 사건의 이관(O-3), BaseUndecided 의 나머지 조합, 승인 이후 변경의 결속·보존·재적용에 아무 의미도 정하지 않는다",
];

// ── 대상 ───────────────────────────────────────────────────────────────────

/// 실재하고 하나로 정해진 Experiment 안에서 지정한 Step 의 세계를 읽을 수 없는 이유. **「정책 채택 전」도 「정책 없음」도 아니다.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StepProblem {
    /// 그 Experiment 에 지정한 Step 이 없다.
    NotFound { step: StepRef, chain: ChainRef },
    /// 지정한 Step 이 Verify 가 아니다 — **범위 밖이며** 그 Step 에 적힌 값을 조회하지 않는다.
    NotVerifyOutOfScope { step: StepRef, chain: ChainRef, kind: NodeKind },
    /// 지정한 Verify 가 열려 있다 — **범위 밖이며** 아직 세계를 확정하지 않았다. 적힌 값을 조회하지 않는다.
    OpenVerifyOutOfScope { step: StepRef, chain: ChainRef },
    /// 지정한 Verify 가 닫혔는데 `snapshot_ref` 가 없다. Artifact Model §7.3 이 허용하지 않는 손상이며, 대신할 세계를 고르지 않는다.
    ClosedVerifyLacksSnapshot { step: StepRef, chain: ChainRef },
}

impl fmt::Display for StepProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StepProblem::NotFound { step, chain } => {
                write!(f, "{step}({chain}) 이 그 Experiment 에 없다. 다른 Step 으로 대신하지 않는다")
            }
            StepProblem::NotVerifyOutOfScope { step, chain, kind } => write!(
                f,
                "{step}({chain}) 은 {kind} 다. Verify 가 아닌 Step 은 이 유도의 범위 밖이며 그 세계를 조회하지 않는다"
            ),
            StepProblem::OpenVerifyOutOfScope { step, chain } => write!(
                f,
                "Verify {step}({chain}) 가 열려 있다. 열린 Verify 는 이 유도의 범위 밖이며 그 세계를 조회하지 않는다"
            ),
            StepProblem::ClosedVerifyLacksSnapshot { step, chain } => write!(
                f,
                "Verify {step}({chain}) 는 닫혔는데 snapshot_ref 가 없다. 다른 Verify 나 Entry 로 대신하지 않는다"
            ),
        }
    }
}

/// 건넨 목록에서 읽은 닫힌 Verify.
struct Located {
    chain: ChainRef,
    world: SnapshotRef,
}

/// 대상 Step 의 Cycle 을 [`owned_cycle`] 로 정한 뒤 그 Cycle 의 종류와 Step 의 실재·종류·닫힘·`snapshot_ref` 칸만 읽는다.
fn locate(target: StepRef, chains: &[Chain]) -> Result<Located, VerifyWorldProblem> {
    let (chain, cycle) = owned_cycle(target.cycle(), chains).map_err(VerifyWorldProblem::Target)?;
    match cycle.kind() {
        CycleKind::Interview => return Err(VerifyWorldProblem::InterviewOutOfScope { step: target, chain }),
        CycleKind::Experiment => {}
    }
    let step = |problem| Err(VerifyWorldProblem::Step(problem));
    let Some(node) = cycle.steps().nodes().iter().find(|node| cycle.step_ref(node.id) == target) else {
        return step(StepProblem::NotFound { step: target, chain });
    };
    if node.kind != NodeKind::Verify {
        return step(StepProblem::NotVerifyOutOfScope { step: target, chain, kind: node.kind });
    }
    if !node.is_closed() {
        return step(StepProblem::OpenVerifyOutOfScope { step: target, chain });
    }
    match node.snapshot {
        Some(world) => Ok(Located { chain, world }),
        None => step(StepProblem::ClosedVerifyLacksSnapshot { step: target, chain }),
    }
}

// ── 결과 ───────────────────────────────────────────────────────────────────

/// 건넨 기록 안에서 유도한 지정 Verify 의 세계와 그 확정 정책 — **그것뿐이다.** 보이지 않는 것은 [`VERIFY_WORLD_DOES_NOT_SHOW`] 에 있다.
/// 이 값을 만드는 문은 [`select_verify_world`] 하나뿐이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VerifyWorld {
    step: StepRef,
    world: SnapshotRef,
    opening: OpeningPolicy,
}

impl VerifyWorld {
    /// 대상 Verify.
    pub(crate) fn step(&self) -> StepRef {
        self.step
    }

    /// 대상 Verify 가 담긴 Experiment N — P(N) 의 대상과 같다.
    pub(crate) fn cycle(&self) -> CycleRef {
        self.opening.target()
    }

    /// 건넨 목록에서 N 을 소유한 체인 — P(N) 의 소유 조회와 이 선택의 조회가 같은 체인을 가리킨다.
    pub(crate) fn chain(&self) -> ChainRef {
        self.opening.chain()
    }

    /// 대상 Verify 자신의 `snapshot_ref` — 건넨 등록부에서 이름이 조회됐다. 현재 세계·Cycle Exit 이 아니다.
    pub(crate) fn world(&self) -> SnapshotRef {
        self.world
    }

    /// 그 세계의 확정 정책 — 같은 호출의 P(N) 선택 그대로(§6.1). 사건·종류·판 이름·승인 당시 `next_cycle_id`, 또는 건넨 이력 안의 정책
    /// 채택 전.
    pub(crate) fn policy(&self) -> OpeningSelection {
        self.opening.selection()
    }

    /// 같은 호출에서 고른 P(N) 그대로.
    pub(crate) fn opening(&self) -> &OpeningPolicy {
        &self.opening
    }

    /// P(N) 안에서 같은 호출에 실행한 3j 의 성공 값 그대로 — 그 판단하지 않음(`not_judged`·`chains_not_looked_up`)을 지닌다.
    pub(crate) fn inputs(&self) -> &EventInputsAgree {
        self.opening.inputs()
    }
}

/// 유도가 실패한 갈래. 갈래 순서는 입력 검사, 대상(Cycle 의 소유·종류, Step 의 실재·종류·닫힘·`snapshot_ref`, 그 이름의 등록부 조회)이다.
/// 대상 갈래는 입력 검사와 무관하게 평가하므로 둘이 함께 올 수 있다. **어느 것도 정책 채택 전이나 Exit 정책 미유도가 아니다.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum VerifyWorldProblem {
    /// 같은 호출의 P(N) 안에서 건넨 입력이 3j 를 통과하지 못했다 — 그 실패 그대로.
    Inputs(Vec<EventInputsProblem>),
    /// 대상 Step 의 Cycle 을 건넨 목록에서 하나로 정할 수 없다 — P(N) 과 같은 소유 규칙의 문제 그대로이며 한 번만 담는다.
    Target(TargetProblem),
    /// 대상 Step 의 Cycle 이 Interview 다 — **범위 밖이며** 성공도 정책 채택 전도 아니다. Step 의 실재·열림·닫힘과 무관하게 같다.
    InterviewOutOfScope { step: StepRef, chain: ChainRef },
    /// 실재하는 Experiment 안에서 지정한 Step 의 세계를 읽을 수 없다.
    Step(StepProblem),
    /// 대상이 실재하는 닫힌 Verify 이고 `snapshot_ref` 칸이 있는데 그 이름이 건넨 등록부에 없다. 입력 검사의 결과와 무관하게 평가한다.
    SnapshotNotInRegistry { step: StepRef, chain: ChainRef, world: SnapshotRef },
}

impl fmt::Display for VerifyWorldProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VerifyWorldProblem::Inputs(found) => write!(
                f,
                "건넨 사건 입력이 맞지 않아 Verify 의 확정 정책을 유도하지 않았다: {}",
                found.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
            ),
            VerifyWorldProblem::Target(problem) => write!(f, "Verify 의 세계를 유도하지 않았다: {problem}"),
            VerifyWorldProblem::InterviewOutOfScope { step, chain } => write!(
                f,
                "{step}({chain}) 은 Interview 안의 Step 이다. Interview 는 이 유도의 범위 밖이며 정책 없음도 정책 채택 전도 성공도 아니다"
            ),
            VerifyWorldProblem::Step(problem) => write!(f, "Verify 의 세계를 유도하지 않았다: {problem}"),
            VerifyWorldProblem::SnapshotNotInRegistry { step, chain, world } => write!(
                f,
                "Verify {step}({chain}) 의 세계 {world} 이 건넨 등록부에 없다. 대신할 세계를 고르지 않는다"
            ),
        }
    }
}

// ── 유도 ───────────────────────────────────────────────────────────────────

/// 건넨 기록에서 Experiment 안의 지정한 닫힌 Verify `target` 이 확정한 세계와 그 확정 정책을 함께 유도한다. **순수 함수**다.
///
/// 대상 Step 의 Cycle N 으로 같은 입력의 [`select_opening_policy`] 를 한 번 부르고(그 안에서 3j 와 대상 위치를 확인한다), 대상의 소유·종류와
/// Step 의 실재·종류·닫힘·`snapshot_ref` 와 그 이름의 등록부 조회를 그 결과와 무관하게 평가한다. 모두 성립할 때만 그 Verify 의 `snapshot_ref` 와
/// P(N) 을 그대로 담는다. 입력·체인·등록부를 바꾸지 않고 파일·상태·창고를 읽거나 쓰지 않는다.
pub(crate) fn select_verify_world(
    target: StepRef,
    ledger: &PolicyLedger,
    links: &EventPolicyLinks,
    facts: &[EventCycleFacts],
    chains: &[Chain],
    registry: &SnapshotRegistry,
) -> Result<VerifyWorld, Vec<VerifyWorldProblem>> {
    let opening = select_opening_policy(target.cycle(), ledger, links, facts, chains, registry);
    let located = locate(target, chains).and_then(|located| match registry.resolve(located.world) {
        None => Err(VerifyWorldProblem::SnapshotNotInRegistry { step: target, chain: located.chain, world: located.world }),
        Some(_) => Ok(located),
    });

    let (opening, located) = match (opening, located) {
        (Ok(opening), Ok(located)) => (opening, located),
        (opening, located) => {
            let mut problems = Vec::new();
            if let Err(found) = opening {
                problems.extend(found.into_iter().map(|problem| match problem {
                    OpeningPolicyProblem::Inputs(found) => VerifyWorldProblem::Inputs(found),
                    OpeningPolicyProblem::Target(problem) => VerifyWorldProblem::Target(problem),
                }));
            }
            if let Err(problem) = located {
                // 같은 소유 규칙이므로 P(N) 이 돌려준 소유 문제와 같은 값이면 한 번만 담는다.
                if !problems.contains(&problem) {
                    problems.push(problem);
                }
            }
            return Err(problems);
        }
    };
    debug_assert_eq!(opening.chain(), located.chain, "P(N) 과 이 선택은 같은 소유 규칙으로 같은 체인을 읽는다");
    Ok(VerifyWorld { step: target, world: located.world, opening })
}
