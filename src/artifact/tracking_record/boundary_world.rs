//! **닫힌 사이클 T 의 경계에서 마지막 확정 세계의 출처 B(T)** 를 건넨 기록에서 고르는 순수 선택 — 계약 §6.1 의 B(T) 한 줄, 제품 미연결.
//!
//! 이 모듈은 §6.1 의 정의 하나만 옮긴다. **§6.1 전체(열린 사이클의 Verify 계보, Interview 의 정책 계승), 세계 출처 S(N), 검증 ①·②,
//! T8, 제품 연결의 완료가 아니다.** 정본은 [공유 저장·전이 계약](../../../spec/GIL_Storage_Transition_Contract_v0.1.md) §6.1 과
//! §5.1(사건 칸 표의 `baseline.after_cycle_ref`·`baseline.next_cycle_id`·`baseline.snapshot_ref`·`policy_after_ref`), §5.5, §6.3, §6.4,
//! §5.7 의 T5·T6·T7 이다.
//!
//! ```text
//! 닫힌 사이클 T 의 경계    B(T) = 기준점을 T 에서 보관한(baseline.after_cycle_ref = T) 사건 가운데 이름이 가장 큰 것의 기준점
//!                                그런 사건이 없으면 T 의 exit_snapshot_ref
//! 확정 정책               사건의 기준점 → 그 사건의 policy_after
//!                         Exit → (이 선택은 유도하지 않는다)
//!
//! (PolicyLedger, EventPolicyLinks, EventCycleFacts*, &[Chain], 등록부) ─ check_event_inputs (3j, 그대로) ─▶ Inputs 갈래
//! 대상 CycleRef + &[Chain] ─ Chain::id · Cycles::nodes · Cycle::is_closed · Cycle::exit_snapshot (읽기 전용) ──▶ Target 갈래
//!                                   │ 둘 다 성립할 때만
//!                                   ▼
//! 대응이 확인된 자리마다 (사건 이름·종류·policy_after, baseline 의 snapshot_ref·after_cycle_ref·next_cycle_id)
//!     ─ snapshot_ref 있음 · baseline.after_cycle_ref = T, 이름 최대 ─▶ EventBaseline
//!     ─ 그런 사건이 없으면 T 의 Exit, 등록부에 있을 때만 ─────────────▶ ExitOfTarget | ExitNotInRegistry
//! ```
//!
//! # 무엇으로 고르는가
//!
//! - **보관 경계 `baseline.after_cycle_ref`** — 기준점을 보관한 경계다(§5.1). 승인 경계 `after_cycle_ref` 로 대신하지 않는다. 보관
//!   대기 중 인터뷰가 닫힌 뒤 다른 경계에서 보관한 사건은 두 값이 다르며, 그 기준점은 보관한 경계의 B 다(§6.3·§6.5 (가)).
//! - **보관된 기준점만.** `baseline.snapshot_ref` 가 없는 사건(보관 대기·대체됨)은 후보가 아니다. 그런 사건이 이름이 더 커도 같은
//!   경계의 앞선 보관 기준점을 가리지 않는다(§5.2 「보관 대기는 유도값」, §6.3 마지막 확정 세계).
//! - **발급기로 거르지 않는다.** 이 선택은 **지금 건넨 기록**에서의 B(T) 다. S(N) 의 「N 이전의 기준점」(`baseline.next_cycle_id ≤ N`)
//!   같은 과거 개방 시점 필터를 섞지 않고, 사건의 승인 당시 `next_cycle_id` 로 기준점을 고르지 않는다. 입력에 N 이 없다.
//! - **`restore_result` 도 사건의 기준점이다.** 그 기준점은 적용 정책으로 투영한 세계이고(§6.4), 확정 정책은 그 사건의 `policy_after`
//!   다. 같은 판을 앞선 사건이 도입했다는 이유로 출처를 그 사건으로 바꾸지 않는다.
//! - **합치지 않는다.** 같은 세계 이름을 다른 사건이나 T 의 Exit 이 가리켜도 출처는 고른 자리 하나다(§5.5 — 정책을 세계 등록부에
//!   적지 않는다). 같은 원문의 다른 판(`P1`·`P3`)은 판 이름으로 남긴다(§5.1).
//! - **구조를 보지 않는다.** 사건의 `chain_ref`·승인 경계, 대상의 종류·부모·`revisit_from` 은 선택에 들지 않는다. 보관 경계가 T 이면
//!   3h 가 그 사건의 `chain_ref` 와 T 의 소속이 같음을 이미 확인했다.
//!
//! # 입력과 검사
//!
//! 3j 의 [`check_event_inputs`] 를 **같은 호출에서 받은 입력에** 실행하고 실패를 그대로 담는다. 다른 입력에서 얻은 성공 값만 받아
//! 검사를 건너뛰는 문은 없다. 3j 가 성공한 뒤에만, 3j 가 자리·이름·종류의 대응을 확인한 바로 그 두 목록에서 사건의 정책 연결
//! (3c 입력)과 `baseline` 칸(3h 입력 안의 3e 입력)을 읽는다. 고른 기준점 이름이 등록부에 있다는 것은 같은 3j 안의 3e 가 확인했다.
//!
//! 대상은 건넨 `Chain` 목록에서 **실재하고 하나로 정해지며 닫힌** 사이클이다. 없는 대상·여러 체인 항목이 소유한 대상·소유 체인의
//! 이름이 목록에 여러 번 있는 대상·열린 대상은 [`BoundaryTargetProblem`] 으로 돌려주며 「기준점 없음」이나 「정책 없음」으로 바꾸지
//! 않는다. 대상의 위치는 3j 의 결과와 무관하게 평가하고, 두 갈래의 실패를 함께 돌려준다. T 의 Exit 이 출처로 골라졌을 때만 그 이름을
//! 건넨 등록부에서 조회하고, 없으면 [`BoundaryWorldProblem::ExitNotInRegistry`] 로 거절한다.
//!
//! # 결과가 말하는 것
//!
//! [`BoundarySource::EventBaseline`] 은 사건 이름·종류·세계·`policy_after` 판 이름·보관 경계·보관 발급기를 함께 지닌다.
//! [`BoundarySource::ExitOfTarget`] 은 세계 이름만 지니며 **Exit 의 확정 정책을 아직 유도하지 않았다**
//! ([`BoundaryPolicy::ExitPolicyNotDerived`]). 그것은 현재 적용 정책도, P(T) 도, 정책 채택 전도 아니다 — §6.1 에서 Experiment 의 Exit 은
//! 그 실험을 열 때의 적용 정책, Interview 의 Exit 은 Entry 의 확정 정책을 물려받으며, 그 유도는 이 선택의 범위 밖이다.
//!
//! 성공 값은 3j 의 성공 값을 그대로 지닌다 — 3h 의 `null`·칸 없음(`not_judged`)과 `chain_ref` 미조회(`chains_not_looked_up`)가 숨지
//! 않는다. 성공은 **건넨 기록 안의 선택**이며 실제 승인·보관, 원장의 완전성, T8, 전체 저장 상태의 유효성을 증명하지 않는다. 보이지
//! 않는 것은 [`BOUNDARY_WORLD_DOES_NOT_SHOW`] 에 있다.

use std::fmt;

use super::super::SnapshotRegistry;
use super::baseline_record::BaselineSlot;
use super::event_cycles::EventCycleFacts;
use super::event_inputs::{EventInputsAgree, EventInputsProblem, check_event_inputs};
use super::event_links::{EventKind, EventName, EventPolicyLinks};
use super::{PolicyLedger, PolicyName};
use crate::refs::{ChainRef, CycleRef, SnapshotRef};
use crate::{Chain, Cycle};

/// 이 선택이 성공해도 **보이지 않는** 것 — 결과를 들고 있어도 이것들은 성립하지 않는다.
pub(crate) const BOUNDARY_WORLD_DOES_NOT_SHOW: [&str; 11] = [
    "입력의 출처와 완전성 — 건넨 원장·사건 입력·Chain 목록·등록부가 실제 프로젝트의 같은 시점에서 빠짐없이 뽑혔는지 보지 않는다. 원장과 두 사건 입력을 함께 잘라낸 입력은 알아내지 못하며 잘린 이력 안에서 고른다. 3j 의 EVENT_INPUTS_DO_NOT_SHOW 그대로다",
    "실제 승인·보관 — 고른 사건의 모니터 승인 응답과 승인 저장·기준점 보관(저장 2)의 실제 성공, 보관 실행 권한을 확인하지 않는다. 성공은 건넨 기록 안의 선택이다",
    "3j 의 판단하지 않음 — after_cycle_ref·baseline.after_cycle_ref 의 null·칸 없음(not_judged)과 chain_ref 미조회(chains_not_looked_up)는 판단하지 않은 채 성공 값의 inputs() 에 그대로 남는다. baseline.after_cycle_ref 가 null 이거나 칸이 없는 사건은 어느 T 의 후보도 아니다",
    "Exit 의 확정 정책 — 기준점을 T 에서 보관한 사건이 없어 T 의 Exit 을 고르면 그 확정 정책을 유도하지 않는다(ExitPolicyNotDerived). 현재 적용 정책·P(T)·정책 채택 전 어느 것도 아니며, Experiment 의 P·Interview 의 Entry 계승(§6.1)은 후속이다",
    "T8 과 world_before_ref — 사건의 world_before_ref 가 승인 경계·보관 경계의 마지막 확정 세계와 같은지, 기준점이 적용 정책으로 올바르게 관측됐는지(§5.4) 보지 않는다",
    "S(N) 과 다음 Entry — N 이전의 기준점(baseline.next_cycle_id ≤ N), 구조적 부모, 검증 ①·②, 실험을 열 자격(T9)을 보지 않는다. 이 선택에는 N 이 없다",
    "지금 서 있는 경계 — T 가 방금 닫은 자리인지, 되돌아와서 서 있는 자리인지, 되돌아가기의 대상인지 보지 않는다. 열린 사이클의 마지막 확정 세계(Verify 계보)도 유도하지 않는다",
    "세계의 실재 — 고른 이름이 건넨 등록부에서 조회되는지만 본다(사건의 기준점은 같은 3j 안의 3e, T 의 Exit 은 골랐을 때만). manifest·blob 의 실재와 내용, 고르지 않은 Exit 의 등록 여부는 보지 않는다",
    "3n·3o 와의 연결 — 결과를 LedgerStart·ConfirmedStart·AppliedSpan 으로 바꾸는 문이 없다. 경로 판정·구간 선택의 시작점은 호출자가 따로 정한다",
    "보관·재시도·복원·Monitor·CLI·MCP — 이 선택을 부르는 제품 경로가 없고 파일·state.yaml·객체 창고를 읽거나 쓰지 않는다. 기준점을 만들거나 고치지 않는다. 형식 10/11 은 활성화되지 않았다",
    "미결 결정 — 즉시 보관 중단의 요청·해제(O-2), 보관 대기 중 Chain 종료와 미완료 사건의 이관(O-3), BaseUndecided 의 나머지 조합에 아무 의미도 정하지 않는다",
];

// ── 대상 ───────────────────────────────────────────────────────────────────

/// 대상 사이클을 건넨 `Chain` 목록에서 하나의 닫힌 경계로 정할 수 없는 이유. **「기준점 없음」도 「정책 없음」도 아니다.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BoundaryTargetProblem {
    /// 건넨 목록의 어느 체인도 그 사이클을 소유하지 않는다.
    NotFound { cycle: CycleRef },
    /// 둘 이상의 체인 항목이 그 사이클을 소유한다(목록 순서).
    SeveralOwners { cycle: CycleRef, chains: Vec<ChainRef> },
    /// 그 사이클을 소유한 체인의 이름이 목록에 `count` 번 있다 — 어느 기록의 사이클인지 정할 수 없다.
    OwnerChainRepeated { cycle: CycleRef, chain: ChainRef, count: usize },
    /// 그 사이클이 열려 있다 — 닫힌 사이클의 경계가 아니다.
    TargetNotClosed { cycle: CycleRef, chain: ChainRef },
    /// 그 사이클이 닫혔다고 적혔는데 `exit_snapshot_ref` 가 없다. 기존 전이와 저장 load 가 지키는 짝이 깨진 입력이며, 대신할 세계를
    /// 고르지 않는다.
    ClosedWithoutExit { cycle: CycleRef, chain: ChainRef },
}

impl fmt::Display for BoundaryTargetProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BoundaryTargetProblem::NotFound { cycle } => write!(
                f,
                "대상 {cycle} 을 건넨 체인 목록의 어느 체인도 소유하지 않는다. 기준점 없음도 정책 없음도 아니다"
            ),
            BoundaryTargetProblem::SeveralOwners { cycle, chains } => write!(
                f,
                "대상 {cycle} 을 여러 체인 항목({})이 소유한다. 대상을 하나로 정할 수 없다",
                chains.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")
            ),
            BoundaryTargetProblem::OwnerChainRepeated { cycle, chain, count } => write!(
                f,
                "대상 {cycle} 을 소유한 {chain} 이 건넨 체인 목록에 {count}번 있다. 대상을 하나로 정할 수 없다"
            ),
            BoundaryTargetProblem::TargetNotClosed { cycle, chain } => write!(
                f,
                "대상 {cycle}({chain}) 이 열려 있다. 닫힌 사이클의 경계에서만 B(T) 를 고른다"
            ),
            BoundaryTargetProblem::ClosedWithoutExit { cycle, chain } => write!(
                f,
                "대상 {cycle}({chain}) 은 닫혔는데 exit_snapshot_ref 가 없다. 대신할 세계를 고르지 않는다"
            ),
        }
    }
}

/// 대상을 소유한 체인과 그 닫힌 사이클의 Exit — 체인 이름([`Chain::id`])과 사이클 목록만 읽는다. 첫 항목을 고르지 않는다.
fn locate(target: CycleRef, chains: &[Chain]) -> Result<(ChainRef, SnapshotRef), BoundaryTargetProblem> {
    let owners: Vec<(ChainRef, &Cycle)> = chains
        .iter()
        .filter_map(|chain| {
            chain.cycles().nodes().iter().find(|cycle| cycle.id().to_ref() == target).map(|cycle| (chain.id(), cycle))
        })
        .collect();
    match owners.as_slice() {
        [] => Err(BoundaryTargetProblem::NotFound { cycle: target }),
        [(owner, cycle)] => {
            let count = chains.iter().filter(|chain| chain.id() == *owner).count();
            if count != 1 {
                return Err(BoundaryTargetProblem::OwnerChainRepeated { cycle: target, chain: *owner, count });
            }
            if !cycle.is_closed() {
                return Err(BoundaryTargetProblem::TargetNotClosed { cycle: target, chain: *owner });
            }
            match cycle.exit_snapshot() {
                Some(exit) => Ok((*owner, exit)),
                None => Err(BoundaryTargetProblem::ClosedWithoutExit { cycle: target, chain: *owner }),
            }
        }
        _ => Err(BoundaryTargetProblem::SeveralOwners {
            cycle: target,
            chains: owners.iter().map(|(chain, _)| *chain).collect(),
        }),
    }
}

// ── 결과 ───────────────────────────────────────────────────────────────────

/// 건넨 기록 안에서 고른 B(T) 의 출처.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BoundarySource {
    /// 기준점을 T 에서 보관한 사건 가운데 이름이 가장 큰 사건의 기준점.
    EventBaseline {
        /// 고른 사건의 이름 — `restore_result` 라도 그 사건이다.
        event: EventName,
        /// 고른 사건의 종류(3j 가 두 입력에서 같다고 확인한 값).
        kind: EventKind,
        /// 그 사건의 `baseline.snapshot_ref`.
        world: SnapshotRef,
        /// 그 사건의 `policy_after_ref` — 판 이름이다. 원문 지문이 아니다.
        policy: PolicyName,
        /// 그 사건의 `baseline.after_cycle_ref` — 보관 경계이며 언제나 대상 T 다. 승인 경계 `after_cycle_ref` 가 아니다.
        archived_at: CycleRef,
        /// 그 사건의 `baseline.next_cycle_id` — 보관 시점의 발급기 값. 승인 당시 `next_cycle_id` 가 아니다.
        archive_issuer: u32,
    },
    /// 기준점을 T 에서 보관한 사건이 건넨 기록에 없다 — T 의 `exit_snapshot_ref`. **Exit 의 확정 정책은 유도하지 않았다.**
    ExitOfTarget {
        /// T 의 `exit_snapshot_ref`. 건넨 등록부에서 이름이 조회됐다.
        world: SnapshotRef,
    },
}

impl BoundarySource {
    /// 고른 세계의 이름.
    pub(crate) fn world(&self) -> SnapshotRef {
        match self {
            BoundarySource::EventBaseline { world, .. } | BoundarySource::ExitOfTarget { world } => *world,
        }
    }

    /// 고른 세계의 확정 정책에 대해 이 선택이 말할 수 있는 것.
    pub(crate) fn policy(&self) -> BoundaryPolicy {
        match self {
            BoundarySource::EventBaseline { event, policy, .. } => BoundaryPolicy::PolicyAfter { event: *event, policy: *policy },
            BoundarySource::ExitOfTarget { .. } => BoundaryPolicy::ExitPolicyNotDerived,
        }
    }
}

/// 고른 세계의 확정 정책에 대해 이 선택이 말하는 것 — §6.1 의 표 가운데 「사건의 기준점」 줄만 옮겼다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BoundaryPolicy {
    /// 사건의 기준점 — 그 사건의 `policy_after`(§6.1). 건넨 기록 안의 값이며 실제 승인·적용의 증명이 아니다.
    PolicyAfter { event: EventName, policy: PolicyName },
    /// T 의 Exit — 확정 정책을 **아직 유도하지 않았다.** 정책 채택 전이라는 뜻이 아니고, 현재 적용 정책이나 P(T) 를 뜻하지도 않는다.
    ExitPolicyNotDerived,
}

/// 건넨 기록 안에서 고른 B(T) — **그것뿐이다.** 보이지 않는 것은 [`BOUNDARY_WORLD_DOES_NOT_SHOW`] 에 있다. 이 값을 만드는 문은
/// [`select_boundary_world`] 하나뿐이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BoundaryWorld {
    target: CycleRef,
    chain: ChainRef,
    source: BoundarySource,
    inputs: EventInputsAgree,
}

impl BoundaryWorld {
    /// 대상 사이클 T.
    pub(crate) fn target(&self) -> CycleRef {
        self.target
    }

    /// 건넨 목록에서 T 를 소유한 체인. 선택에는 쓰지 않았다.
    pub(crate) fn chain(&self) -> ChainRef {
        self.chain
    }

    /// 고른 출처 — 사건의 기준점 또는 T 의 Exit.
    pub(crate) fn source(&self) -> BoundarySource {
        self.source
    }

    /// 고른 세계의 이름.
    pub(crate) fn world(&self) -> SnapshotRef {
        self.source.world()
    }

    /// 고른 세계의 확정 정책에 대해 이 선택이 말하는 것.
    pub(crate) fn policy(&self) -> BoundaryPolicy {
        self.source.policy()
    }

    /// 같은 호출에서 실행한 3j 의 성공 값 그대로 — 그 판단하지 않음(`not_judged`·`chains_not_looked_up`)을 지닌다.
    pub(crate) fn inputs(&self) -> &EventInputsAgree {
        &self.inputs
    }
}

/// 선택이 실패한 갈래. 갈래 순서는 입력 검사, 대상, 고른 Exit 의 등록부 조회다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BoundaryWorldProblem {
    /// 같은 호출에 건넨 입력이 3j 를 통과하지 못했다 — 그 실패 그대로.
    Inputs(Vec<EventInputsProblem>),
    /// 대상 사이클을 건넨 목록에서 하나의 닫힌 경계로 정할 수 없다.
    Target(BoundaryTargetProblem),
    /// 기준점을 T 에서 보관한 사건이 없어 T 의 Exit 을 골랐는데 그 이름이 건넨 등록부에 없다.
    ExitNotInRegistry { cycle: CycleRef, world: SnapshotRef },
}

impl fmt::Display for BoundaryWorldProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BoundaryWorldProblem::Inputs(found) => write!(
                f,
                "건넨 사건 입력이 맞지 않아 B(T) 를 고르지 않았다: {}",
                found.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
            ),
            BoundaryWorldProblem::Target(problem) => write!(f, "B(T) 를 고르지 않았다: {problem}"),
            BoundaryWorldProblem::ExitNotInRegistry { cycle, world } => write!(
                f,
                "{cycle} 에서 기준점을 보관한 사건이 없어 Exit {world} 을 골랐는데 그 이름이 건넨 등록부에 없다"
            ),
        }
    }
}

// ── 선택 ───────────────────────────────────────────────────────────────────

/// 건넨 기록에서 닫힌 사이클 `target` 의 경계 세계 B(T) 와 그 출처를 고른다. **순수 함수**다.
///
/// 같은 입력에 3j 의 [`check_event_inputs`] 를 실행하고, 대상의 위치·닫힘을 그 결과와 무관하게 평가한다. 둘 다 성립할 때만 고른다 —
/// 대응이 확인된 자리마다 `baseline.snapshot_ref` 가 있고 `baseline.after_cycle_ref` 가 대상인 사건 가운데 이름이 가장 큰 사건의
/// 기준점. 그런 사건이 없으면 대상의 `exit_snapshot_ref` 이며, 그 이름이 등록부에 있어야 한다. 입력·체인·등록부를 바꾸지 않고
/// 파일·상태·창고를 읽거나 쓰지 않는다.
pub(crate) fn select_boundary_world(
    target: CycleRef,
    ledger: &PolicyLedger,
    links: &EventPolicyLinks,
    facts: &[EventCycleFacts],
    chains: &[Chain],
    registry: &SnapshotRegistry,
) -> Result<BoundaryWorld, Vec<BoundaryWorldProblem>> {
    let inputs = check_event_inputs(ledger, links, facts, chains, registry);
    let located = locate(target, chains);

    let (inputs, (chain, exit)) = match (inputs, located) {
        (Ok(inputs), Ok(located)) => (inputs, located),
        (inputs, located) => {
            let mut problems = Vec::new();
            if let Err(found) = inputs {
                problems.push(BoundaryWorldProblem::Inputs(found));
            }
            if let Err(problem) = located {
                problems.push(BoundaryWorldProblem::Target(problem));
            }
            return Err(problems);
        }
    };

    // 3j 가 성공했으므로 두 목록은 같은 수이고, 같은 자리의 이름과 종류가 같다.
    let archived_here = links
        .events()
        .iter()
        .zip(facts)
        .filter_map(|(link, fact)| {
            let baseline = fact.baseline();
            let world = baseline.snapshot()?;
            match baseline.boundary() {
                BaselineSlot::Written(Some(archived_at)) if archived_at == target => {
                    Some((link, world, archived_at, baseline.issuer()))
                }
                _ => None,
            }
        })
        .max_by_key(|(link, ..)| link.name());

    let source = match archived_here {
        Some((link, world, archived_at, issuer)) => {
            let BaselineSlot::Written(archive_issuer) = issuer else {
                unreachable!("3e 의 성공은 기준점이 있는 사건에 baseline.next_cycle_id 칸이 있음을 보장한다")
            };
            BoundarySource::EventBaseline {
                event: link.name(),
                kind: link.kind(),
                world,
                policy: link.after().expect("3c 의 성공은 모든 사건의 after 에 판이 있음을 보장한다"),
                archived_at,
                archive_issuer,
            }
        }
        None => {
            if registry.resolve(exit).is_none() {
                return Err(vec![BoundaryWorldProblem::ExitNotInRegistry { cycle: target, world: exit }]);
            }
            BoundarySource::ExitOfTarget { world: exit }
        }
    };

    Ok(BoundaryWorld { target, chain, source, inputs })
}
