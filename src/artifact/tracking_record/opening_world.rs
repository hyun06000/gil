//! **비뿌리 사이클 N 의 세계 출처 S(N)** 을 건넨 기록에서 고르고, 같은 N 의 실제 Entry 와 견주는 **검증 ①** — 계약 §6.2 의 두 줄, 제품 미연결.
//!
//! 이 모듈은 §6.2 의 S(N) 정의와 검증 ① 만 옮긴다. **검증 ②(실험의 확정 정책 = P(N)), 실제 개방 허가, T8·T9, 뿌리·후속 Chain 의
//! Entry, Interview 의 정책 계승, 제품 연결의 완료가 아니다.** 정본은 [공유 저장·전이 계약](../../../spec/GIL_Storage_Transition_Contract_v0.1.md)
//! §6.2 와 §5.1(사건 칸 표의 `baseline.after_cycle_ref`·`baseline.next_cycle_id`·`baseline.snapshot_ref`·`policy_after_ref`), §5.5,
//! §6.1, §6.3, §6.5, §6.6, 그리고 [Artifact Model](../../../spec/GIL_Artifact_Model_v0.1.md) §7.1 의 구조적 parent·`revisit_from` 구분이다.
//!
//! ```text
//! 사이클 N 의 구조적 부모 T     N.parent (revisit_from 이 아니다)
//! N 이전의 기준점               baseline.next_cycle_id ≤ N 인 보관 완료 기준점
//! S(N)                          N 이전의 기준점 중 T 에서 보관한(baseline.after_cycle_ref = T) 것 가운데 사건 이름이 가장 큰 것
//!                               그런 기준점이 없으면 T 의 Exit
//! 검증 ①                        N.entry_snapshot_ref = S(N) 의 세계
//!
//! (PolicyLedger, EventPolicyLinks, EventCycleFacts*, &[Chain], 등록부) ─ check_event_inputs (3j, 그대로) ─────────▶ Inputs 갈래
//! 대상 CycleRef + &[Chain] ─ Chain::id · Cycles::nodes · Cycle::parent · Cycles::node · Cycle::is_closed ·
//!                            Cycle::exit_snapshot · Cycle::entry_snapshot (읽기 전용) ──────────────────────▶ Target / RootOutOfScope / Parent 갈래
//!                                   │ 둘 다 성립할 때만
//!                                   ▼
//! 대응이 확인된 자리마다 (사건 이름·종류·policy_after, baseline 의 snapshot_ref·after_cycle_ref·next_cycle_id)
//!     ─ snapshot_ref 있음 · baseline.after_cycle_ref = T · baseline.next_cycle_id ≤ N, 이름 최대 ─▶ EventBaseline
//!     ─ 그런 사건이 없으면 T 의 Exit, 등록부에 있을 때만 ─────────────────────────────────────▶ ParentExit | ParentExitNotInRegistry
//!                                   │
//!                                   ▼
//! OpeningWorld::check_entry ─ 같은 값 안에서 N 의 entry_snapshot_ref 와 고른 세계를 견준다 ──▶ EntryVerdict::Matches | Differs
//! ```
//!
//! # 무엇으로 고르는가
//!
//! - **구조적 부모 T** — 건넨 목록에서 N 을 소유한 체인의 사이클 목록에서 `N.parent` 를 읽는다. 되돌아간 뒤 연 갈래의 부모는 되돌아간
//!   대상이고 `revisit_from`(버린 실패)이 아니다(Artifact Model §7.1, §6.2 「구조적 부모와 세계 출처가 갈린다」). 다른 체인에서 이름만
//!   같은 사이클을 부모로 가져오지 않는다.
//! - **보관 경계 `baseline.after_cycle_ref` 와 보관 발급기 `baseline.next_cycle_id`.** 승인 경계 `after_cycle_ref` 와 승인 당시
//!   `next_cycle_id` 로 대신하지 않는다. 승인은 N 보다 먼저였어도 보관이 N 보다 늦었으면 그 기준점은 N 의 출처가 아니다(§6.2 「보관이 N
//!   보다 먼저 끝났다」). 늦게 보관한 기준점은 이미 열린 N 의 과거 세계를 바꾸지 않는다.
//! - **보관된 기준점만.** `baseline.snapshot_ref` 가 없는 사건(보관 대기·대체됨)은 후보가 아니고, 이름이 더 커도 앞선 후보를 가리지
//!   않는다. 다른 경계에서 보관한 사건도 T 의 후보를 가리지 않는다.
//! - **견주는 수는 둘뿐이다.** `baseline.next_cycle_id` 와 N 의 번호. 사건 이름의 수를 사이클 번호와 시간 조건으로 견주지 않는다.
//! - **`restore_result` 도 사건의 기준점이다.** 조건을 만족하면 그 사건 자체가 출처이고, 같은 판을 앞선 사건이 도입했다는 이유로
//!   출처를 그 사건으로 바꾸지 않는다.
//! - **합치지 않는다.** 같은 세계 이름을 다른 사건이나 T 의 Exit 이 가리켜도 출처는 고른 자리 하나다(§5.5). 같은 원문의 다른 판은
//!   판 이름으로 남긴다(§5.1).
//! - **P(N)·B(T) 를 쓰지 않는다.** S(N) 은 P(N)(승인 당시 `next_cycle_id` 로 고른 적용 정책)도, B(T)(지금 건넨 기록에서 T 의 경계 세계
//!   — 과거 개방 시점 필터가 없다)도 아니다. 두 선택을 부르지 않고 그 결과를 받지도 않는다.
//!
//! # 입력과 검사
//!
//! 3j 의 [`check_event_inputs`] 를 **같은 호출에서 받은 입력에** 한 번 실행하고 실패를 그대로 담는다. 다른 입력에서 얻은 성공 값을 받아
//! 검사를 건너뛰는 문도, 호출자가 부모·기대 Entry·성공 표식을 건네는 문도 없다. 3j 가 성공한 뒤에만, 3j 가 자리·이름·종류의 대응을
//! 확인한 바로 그 두 목록에서 사건의 정책 연결(3c 입력)과 `baseline` 칸(3h 입력 안의 3e 입력)을 읽는다. 고른 기준점 이름이 등록부에
//! 있다는 것은 같은 3j 안의 3e 가 확인했다. 입력을 정렬·잘라내기·보정하지 않는다.
//!
//! 대상·부모의 위치는 3j 의 결과와 무관하게 평가하고, 두 갈래의 실패를 함께 돌려준다. 이 검사가 **스스로 확인하는 것**:
//!
//! - N 이 건넨 목록에서 실재하고, 한 체인 항목만 소유하며, 그 체인의 이름이 목록에 한 번 있다([`OpeningTargetProblem`]).
//!   N 은 열렸거나 닫혔을 수 있다.
//! - N 이 뿌리가 아니다. 뿌리는 「정책 채택 전」도 성공도 아닌 **범위 밖** 결과다([`OpeningWorldProblem::RootOutOfScope`]) — 뿌리·
//!   후속 Chain 의 첫 사이클의 Entry(§6.2 「뿌리 사이클」, §6.6)는 이 선택이 정하지 않는다.
//! - T 의 이름을 건넨 목록의 다른 체인이 함께 소유하지 않는다([`ParentProblem::SeveralOwners`]).
//! - T 가 닫혔다고 적혔으면 `exit_snapshot_ref` 가 있다([`ParentProblem::ClosedWithoutExit`]). [`crate::Cycles`] 의 복원은 이 짝을 보지
//!   않고 `Project` 의 load 가 본다. 그래서 건넨 `Chain` 목록에서는 닿을 수 있는 입력이다.
//! - 고른 출처가 T 의 Exit 이면 그 이름이 건넨 등록부에 있다([`OpeningWorldProblem::ParentExitNotInRegistry`]). 고르지 않은 Exit 은
//!   조회하지 않는다.
//!
//! **기존 불변식이 보장하는 것** — 다음 두 갈래는 방어용이며 지금의 생성·복원 경로로는 닿을 수 없다. 시험이 그것을 거절 성공으로
//! 세지 않는다.
//!
//! - T 가 N 과 같은 사이클 목록에 있다([`ParentProblem::NotInOwnerChain`]) — `Cycles::open_child`·`open_after_revisit` 은 지금 서 있는
//!   자리를 부모로 적고, `Cycles::restore` 는 부모가 같은 목록의 먼저 난 사이클인지 잰다(`CycleParentNotEarlier`).
//! - T 가 닫혔다([`ParentProblem::NotClosed`]) — 두 여는 문은 닫힌 자리에서만 열고, `Cycles::restore` 는 부모가 `open_child` 를
//!   적었는지(닫힌 Report 의 방향) 재며 열린 사이클은 서 있는 자리 하나뿐이다.
//!
//! # 결과가 말하는 것
//!
//! [`OpeningWorld`] 는 N·T·N 의 실제 Entry·고른 출처·같은 호출의 3j 성공 값을 함께 지닌다. [`WorldSource::EventBaseline`] 은 사건 이름·
//! 종류·세계·`policy_after` 판 이름·보관 경계·보관 발급기를, [`WorldSource::ParentExit`] 는 T 의 Exit 이름만 지닌다. Exit 의 확정 정책은
//! **유도하지 않았다**([`WorldSourcePolicy::ExitPolicyNotDerived`]) — 현재 적용 정책도, P(N)·P(T) 도, 정책 채택 전도 아니다.
//!
//! [`OpeningWorld::check_entry`] 는 같은 값 안의 두 이름을 견준다 — 3j 를 다시 실행하지 않는다. 불일치([`EntryVerdict::Differs`])도
//! 오류가 아니라 명시적 판정이며, 입력·대상·부모의 실패는 처음부터 [`OpeningWorld`] 가 되지 않으므로 불일치나 후보 없음으로 바뀌지
//! 않는다. 일치는 **건넨 기록 안의 검증 ①** 뿐이며 ②, 실제 개방 허가, 실제 승인·보관, 이력 완전성의 증명이 아니다. 보이지 않는 것은
//! [`OPENING_WORLD_DOES_NOT_SHOW`] 에 있다.

use std::fmt;

use super::super::SnapshotRegistry;
use super::baseline_record::BaselineSlot;
use super::event_cycles::EventCycleFacts;
use super::event_inputs::{EventInputsAgree, EventInputsProblem, check_event_inputs};
use super::event_links::{EventKind, EventName, EventPolicyLinks};
use super::{PolicyLedger, PolicyName};
use crate::cycles::CycleId;
use crate::refs::{ChainRef, CycleRef, SnapshotRef};
use crate::{Chain, Cycle};

/// 이 선택과 검증 ① 이 성공해도 **보이지 않는** 것 — 결과를 들고 있어도 이것들은 성립하지 않는다.
pub(crate) const OPENING_WORLD_DOES_NOT_SHOW: [&str; 12] = [
    "입력의 출처와 완전성 — 건넨 원장·사건 입력·Chain 목록·등록부가 실제 프로젝트의 같은 시점에서 빠짐없이 뽑혔는지 보지 않는다. 원장과 두 사건 입력을 함께 잘라낸 입력은 알아내지 못하며 잘린 이력 안에서 고른다. 3j 의 EVENT_INPUTS_DO_NOT_SHOW 그대로다",
    "실제 승인·보관 — 고른 사건의 모니터 승인 응답과 승인 저장·기준점 보관의 실제 성공, 보관 실행 권한을 확인하지 않는다. 성공은 건넨 기록 안의 선택이다",
    "3j 의 판단하지 않음 — after_cycle_ref·baseline.after_cycle_ref 의 null·칸 없음(not_judged)과 chain_ref 미조회(chains_not_looked_up)는 판단하지 않은 채 성공 값의 inputs() 에 그대로 남는다. baseline.after_cycle_ref 가 null 이거나 칸이 없는 사건은 어느 T 의 후보도 아니다",
    "검증 ② 와 개방 허가 — S(N) 의 확정 정책과 P(N) 을 견주지 않는다. Experiment 를 열어도 되는지(T9), 보관 대기 뒤의 실험, 실제 개방 전이를 판단하지 않는다. 검증 ① 의 일치는 개방 허가가 아니다",
    "Exit 의 확정 정책 — S(N) 이 T 의 Exit 이면 그 확정 정책을 유도하지 않는다(ExitPolicyNotDerived). 현재 적용 정책·P(N)·P(T)·정책 채택 전 어느 것도 아니며, Experiment 의 P·Interview 의 Entry 계승(§6.1)은 후속이다",
    "뿌리와 후속 Chain — 뿌리 사이클(start 사건의 출처, 후속 Chain 의 첫 사이클 = 부모 Chain 의 Exit)은 범위 밖 결과로 돌려준다. Chain revisit·병렬 Chain·merge 의 Entry 도 정하지 않는다",
    "T8 과 world_before_ref — 사건의 world_before_ref 가 승인 경계·보관 경계의 마지막 확정 세계와 같은지, 기준점이 적용 정책으로 올바르게 관측됐는지(§5.4) 보지 않는다",
    "열린 사이클의 계보 — N 안의 Verify 계보와 지금 서 있는 경계, 되돌아가기의 복원 목표 B(T) 를 유도하지 않는다. N 이 열렸는지 닫혔는지는 선택에 쓰지 않는다",
    "건넨 Chain 목록의 나머지 불변식 — 직렬 계보·Entry/Exit 인계·발급기 경계는 Project 의 기존 검사 몫이다. 현행 Project load 는 일반 자식의 Entry 가 부모 Exit 과 다르면 거절한다(사건 기준점과 이어지는 제품 연결은 구현 대기). 이 선택은 그 load 를 대신하거나 바꾸지 않는다",
    "세계의 실재 — 고른 이름이 건넨 등록부에서 조회되는지만 본다(사건의 기준점은 같은 3j 안의 3e, T 의 Exit 은 골랐을 때만). N 의 Entry 의 등록, manifest·blob 의 실재와 내용, 고르지 않은 Exit 의 등록 여부는 보지 않는다",
    "3n·3o·보관·재시도·복원·Monitor·CLI·MCP — 결과를 LedgerStart·ConfirmedStart·AppliedSpan 으로 바꾸는 문이 없고, 이 선택을 부르는 제품 경로가 없으며 파일·state.yaml·객체 창고를 읽거나 쓰지 않는다. 기준점을 만들거나 고치지 않는다. 형식 10/11 은 활성화되지 않았다",
    "미결 결정 — 즉시 보관 중단의 요청·해제(O-2), 보관 대기 중 Chain 종료와 미완료 사건의 이관(O-3), BaseUndecided 의 나머지 조합에 아무 의미도 정하지 않는다",
];

// ── 대상과 부모 ────────────────────────────────────────────────────────────

/// 대상 사이클 N 을 건넨 `Chain` 목록에서 하나로 정할 수 없는 이유. **「기준점 없음」도 Entry 불일치도 아니다.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OpeningTargetProblem {
    /// 건넨 목록의 어느 체인도 그 사이클을 소유하지 않는다.
    NotFound { cycle: CycleRef },
    /// 둘 이상의 체인 항목이 그 사이클을 소유한다(목록 순서).
    SeveralOwners { cycle: CycleRef, chains: Vec<ChainRef> },
    /// 그 사이클을 소유한 체인의 이름이 목록에 `count` 번 있다 — 어느 기록의 사이클인지 정할 수 없다.
    OwnerChainRepeated { cycle: CycleRef, chain: ChainRef, count: usize },
}

impl fmt::Display for OpeningTargetProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpeningTargetProblem::NotFound { cycle } => write!(
                f,
                "대상 {cycle} 을 건넨 체인 목록의 어느 체인도 소유하지 않는다. 기준점 없음도 Entry 불일치도 아니다"
            ),
            OpeningTargetProblem::SeveralOwners { cycle, chains } => write!(
                f,
                "대상 {cycle} 을 여러 체인 항목({})이 소유한다. 대상을 하나로 정할 수 없다",
                chains.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")
            ),
            OpeningTargetProblem::OwnerChainRepeated { cycle, chain, count } => write!(
                f,
                "대상 {cycle} 을 소유한 {chain} 이 건넨 체인 목록에 {count}번 있다. 대상을 하나로 정할 수 없다"
            ),
        }
    }
}

/// N 의 구조적 부모 T 를 하나의 닫힌 경계로 읽을 수 없는 이유. 다른 체인의 같은 이름이나 `revisit_from` 으로 대신하지 않는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ParentProblem {
    /// N 이 적은 부모가 N 을 소유한 체인의 사이클 목록에 없다. **방어용** — 지금의 `Cycles` 생성·복원 경로로는 닿을 수 없다.
    NotInOwnerChain { cycle: CycleRef, parent: CycleRef, chain: ChainRef },
    /// 부모의 이름을 둘 이상의 체인 항목이 소유한다(목록 순서). 어느 기록의 사이클인지 정할 수 없다.
    SeveralOwners { cycle: CycleRef, parent: CycleRef, chains: Vec<ChainRef> },
    /// 부모가 열려 있다. **방어용** — 지금의 `Cycles` 생성·복원 경로로는 닿을 수 없다.
    NotClosed { cycle: CycleRef, parent: CycleRef, chain: ChainRef },
    /// 부모가 닫혔다고 적혔는데 `exit_snapshot_ref` 가 없다. `Project` load 가 지키는 짝이 깨진 입력이며, 대신할 세계를 고르지 않는다.
    ClosedWithoutExit { cycle: CycleRef, parent: CycleRef, chain: ChainRef },
}

impl fmt::Display for ParentProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParentProblem::NotInOwnerChain { cycle, parent, chain } => write!(
                f,
                "{cycle} 이 적은 부모 {parent} 가 {chain} 의 사이클 목록에 없다. 다른 체인의 같은 이름을 부모로 쓰지 않는다"
            ),
            ParentProblem::SeveralOwners { cycle, parent, chains } => write!(
                f,
                "{cycle} 의 부모 {parent} 를 여러 체인 항목({})이 소유한다. 부모를 하나로 정할 수 없다",
                chains.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")
            ),
            ParentProblem::NotClosed { cycle, parent, chain } => {
                write!(f, "{cycle} 의 부모 {parent}({chain}) 가 열려 있다. 닫힌 경계에서만 S(N) 을 고른다")
            }
            ParentProblem::ClosedWithoutExit { cycle, parent, chain } => write!(
                f,
                "{cycle} 의 부모 {parent}({chain}) 는 닫혔는데 exit_snapshot_ref 가 없다. 대신할 세계를 고르지 않는다"
            ),
        }
    }
}

/// 건넨 목록에서 읽은 N 과 그 구조적 부모 T.
struct Located {
    chain: ChainRef,
    parent: CycleRef,
    parent_exit: SnapshotRef,
    entry: SnapshotRef,
}

/// N 을 소유한 체인에서 N 과 그 구조적 부모를 읽는다 — 체인 이름([`Chain::id`])·사이클 목록·`parent`·닫힘·Exit·Entry 만 읽는다. 첫
/// 항목을 고르지 않는다.
fn locate(target: CycleRef, chains: &[Chain]) -> Result<Located, OpeningWorldProblem> {
    let owners: Vec<(&Chain, &Cycle)> = chains
        .iter()
        .filter_map(|chain| chain.cycles().nodes().iter().find(|cycle| cycle.id().to_ref() == target).map(|cycle| (chain, cycle)))
        .collect();
    let (owner, cycle) = match owners.as_slice() {
        [] => return Err(OpeningWorldProblem::Target(OpeningTargetProblem::NotFound { cycle: target })),
        [(owner, cycle)] => (*owner, *cycle),
        _ => {
            let chains = owners.iter().map(|(chain, _)| chain.id()).collect();
            return Err(OpeningWorldProblem::Target(OpeningTargetProblem::SeveralOwners { cycle: target, chains }));
        }
    };
    let chain = owner.id();
    let count = chains.iter().filter(|other| other.id() == chain).count();
    if count != 1 {
        return Err(OpeningWorldProblem::Target(OpeningTargetProblem::OwnerChainRepeated { cycle: target, chain, count }));
    }

    let Some(parent_id) = cycle.parent() else {
        return Err(OpeningWorldProblem::RootOutOfScope { cycle: target, chain });
    };
    let parent = parent_id.to_ref();
    let parent_problem = |problem| Err(OpeningWorldProblem::Parent(problem));
    let Some(node) = owner.cycles().node(CycleId::from(parent)) else {
        return parent_problem(ParentProblem::NotInOwnerChain { cycle: target, parent, chain });
    };
    let parent_owners: Vec<ChainRef> = chains.iter().filter(|other| other.owns(parent)).map(Chain::id).collect();
    if parent_owners.len() != 1 {
        return parent_problem(ParentProblem::SeveralOwners { cycle: target, parent, chains: parent_owners });
    }
    if !node.is_closed() {
        return parent_problem(ParentProblem::NotClosed { cycle: target, parent, chain });
    }
    let Some(parent_exit) = node.exit_snapshot() else {
        return parent_problem(ParentProblem::ClosedWithoutExit { cycle: target, parent, chain });
    };
    Ok(Located { chain, parent, parent_exit, entry: cycle.entry_snapshot() })
}

// ── 결과 ───────────────────────────────────────────────────────────────────

/// 건넨 기록 안에서 고른 S(N) 의 출처.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorldSource {
    /// N 이전의 기준점 중 T 에서 보관한 것 가운데 사건 이름이 가장 큰 사건의 기준점.
    EventBaseline {
        /// 고른 사건의 이름 — `restore_result` 라도 그 사건이다.
        event: EventName,
        /// 고른 사건의 종류(3j 가 두 입력에서 같다고 확인한 값).
        kind: EventKind,
        /// 그 사건의 `baseline.snapshot_ref`.
        world: SnapshotRef,
        /// 그 사건의 `policy_after_ref` — 판 이름이다. 원문 지문이 아니다.
        policy: PolicyName,
        /// 그 사건의 `baseline.after_cycle_ref` — 보관 경계이며 언제나 N 의 구조적 부모 T 다. 승인 경계 `after_cycle_ref` 가 아니다.
        archived_at: CycleRef,
        /// 그 사건의 `baseline.next_cycle_id` — 보관 시점의 발급기 값이며 N 의 번호보다 크지 않다. 승인 당시 `next_cycle_id` 가 아니다.
        archive_issuer: u32,
    },
    /// N 이전에 T 에서 보관한 기준점이 건넨 기록에 없다 — T 의 `exit_snapshot_ref`. **Exit 의 확정 정책은 유도하지 않았다.**
    ParentExit {
        /// T 의 `exit_snapshot_ref`. 건넨 등록부에서 이름이 조회됐다.
        world: SnapshotRef,
    },
}

impl WorldSource {
    /// 고른 세계의 이름.
    pub(crate) fn world(&self) -> SnapshotRef {
        match self {
            WorldSource::EventBaseline { world, .. } | WorldSource::ParentExit { world } => *world,
        }
    }

    /// 고른 세계의 확정 정책에 대해 이 선택이 말할 수 있는 것.
    pub(crate) fn policy(&self) -> WorldSourcePolicy {
        match self {
            WorldSource::EventBaseline { event, policy, .. } => WorldSourcePolicy::PolicyAfter { event: *event, policy: *policy },
            WorldSource::ParentExit { .. } => WorldSourcePolicy::ExitPolicyNotDerived,
        }
    }
}

/// 고른 세계의 확정 정책에 대해 이 선택이 말하는 것 — §6.1 의 표 가운데 「사건의 기준점」 줄만 옮겼다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorldSourcePolicy {
    /// 사건의 기준점 — 그 사건의 `policy_after`(§6.1). 건넨 기록 안의 값이며 실제 승인·적용의 증명이 아니다.
    PolicyAfter { event: EventName, policy: PolicyName },
    /// T 의 Exit — 확정 정책을 **아직 유도하지 않았다.** 정책 채택 전이라는 뜻이 아니고, 현재 적용 정책·P(N)·P(T) 를 뜻하지도 않는다.
    ExitPolicyNotDerived,
}

/// 건넨 기록 안에서 고른 S(N) 과, 같은 N 에서 읽은 실제 Entry — **그것뿐이다.** 보이지 않는 것은 [`OPENING_WORLD_DOES_NOT_SHOW`] 에
/// 있다. 이 값을 만드는 문은 [`select_opening_world`] 하나뿐이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpeningWorld {
    target: CycleRef,
    chain: ChainRef,
    parent: CycleRef,
    entry: SnapshotRef,
    source: WorldSource,
    inputs: EventInputsAgree,
}

impl OpeningWorld {
    /// 대상 사이클 N.
    pub(crate) fn target(&self) -> CycleRef {
        self.target
    }

    /// 건넨 목록에서 N 을 소유한 체인 — T 도 이 체인의 사이클이다.
    pub(crate) fn chain(&self) -> ChainRef {
        self.chain
    }

    /// N 의 구조적 부모 T(`N.parent`). `revisit_from` 이 아니다.
    pub(crate) fn parent(&self) -> CycleRef {
        self.parent
    }

    /// 같은 N 에서 읽은 실제 `entry_snapshot_ref`.
    pub(crate) fn entry(&self) -> SnapshotRef {
        self.entry
    }

    /// 고른 출처 — 사건의 기준점 또는 T 의 Exit.
    pub(crate) fn source(&self) -> WorldSource {
        self.source
    }

    /// S(N) 의 세계 — 검증 ① 의 기대 세계.
    pub(crate) fn expected(&self) -> SnapshotRef {
        self.source.world()
    }

    /// 고른 세계의 확정 정책에 대해 이 선택이 말하는 것.
    pub(crate) fn policy(&self) -> WorldSourcePolicy {
        self.source.policy()
    }

    /// 같은 호출에서 실행한 3j 의 성공 값 그대로 — 그 판단하지 않음(`not_judged`·`chains_not_looked_up`)을 지닌다.
    pub(crate) fn inputs(&self) -> &EventInputsAgree {
        &self.inputs
    }

    /// 검증 ① — 같은 값 안의 N 의 실제 Entry 와 S(N) 의 세계를 견준다. **순수**하며 3j 를 다시 실행하지 않는다.
    pub(crate) fn check_entry(&self) -> EntryCheck<'_> {
        let verdict = match self.entry == self.expected() {
            true => EntryVerdict::Matches,
            false => EntryVerdict::Differs,
        };
        EntryCheck { opening: self, verdict }
    }
}

/// 검증 ① 의 판정.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntryVerdict {
    /// `N.entry_snapshot_ref = S(N) 의 세계` — 건넨 기록 안의 검증 ① 일치. ②·개방 허가·이력 완전성이 아니다.
    Matches,
    /// N 의 실제 Entry 가 S(N) 의 세계와 다르다. 입력·대상·부모의 실패가 아니라 고른 출처와 견준 결과다.
    Differs,
}

/// 검증 ① 의 결과 — 판정과 그것을 낸 [`OpeningWorld`] 를 함께 지닌다. 이 값을 만드는 문은 [`OpeningWorld::check_entry`] 하나뿐이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EntryCheck<'a> {
    opening: &'a OpeningWorld,
    verdict: EntryVerdict,
}

impl<'a> EntryCheck<'a> {
    /// 일치 또는 불일치.
    pub(crate) fn verdict(&self) -> EntryVerdict {
        self.verdict
    }

    /// N 의 실제 Entry.
    pub(crate) fn entry(&self) -> SnapshotRef {
        self.opening.entry()
    }

    /// 기대 세계 — S(N) 의 세계.
    pub(crate) fn expected(&self) -> SnapshotRef {
        self.opening.expected()
    }

    /// 판정을 낸 선택 그대로 — N·T·출처·3j 의 판단하지 않음을 불일치에서도 읽을 수 있다.
    pub(crate) fn opening(&self) -> &'a OpeningWorld {
        self.opening
    }
}

/// 선택이 실패한 갈래. 갈래 순서는 입력 검사, 대상·뿌리·부모, 고른 Exit 의 등록부 조회다. **어느 것도 Entry 불일치나 후보 없음이 아니다.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OpeningWorldProblem {
    /// 같은 호출에 건넨 입력이 3j 를 통과하지 못했다 — 그 실패 그대로.
    Inputs(Vec<EventInputsProblem>),
    /// 대상 사이클을 건넨 목록에서 하나로 정할 수 없다.
    Target(OpeningTargetProblem),
    /// 대상이 체인의 뿌리 사이클이다 — 구조적 부모가 없다. **범위 밖이며** 성공도 정책 채택 전도 아니다.
    RootOutOfScope { cycle: CycleRef, chain: ChainRef },
    /// 대상의 구조적 부모를 하나의 닫힌 경계로 읽을 수 없다.
    Parent(ParentProblem),
    /// N 이전에 T 에서 보관한 기준점이 없어 T 의 Exit 을 골랐는데 그 이름이 건넨 등록부에 없다.
    ParentExitNotInRegistry { cycle: CycleRef, parent: CycleRef, world: SnapshotRef },
}

impl fmt::Display for OpeningWorldProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpeningWorldProblem::Inputs(found) => write!(
                f,
                "건넨 사건 입력이 맞지 않아 S(N) 을 고르지 않았다: {}",
                found.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
            ),
            OpeningWorldProblem::Target(problem) => write!(f, "S(N) 을 고르지 않았다: {problem}"),
            OpeningWorldProblem::RootOutOfScope { cycle, chain } => write!(
                f,
                "{cycle} 은 {chain} 의 뿌리 사이클이다. 뿌리·후속 Chain 의 Entry 는 이 선택의 범위 밖이며 정책 채택 전도 성공도 아니다"
            ),
            OpeningWorldProblem::Parent(problem) => write!(f, "S(N) 을 고르지 않았다: {problem}"),
            OpeningWorldProblem::ParentExitNotInRegistry { cycle, parent, world } => write!(
                f,
                "{cycle} 이전에 {parent} 에서 보관한 기준점이 없어 Exit {world} 을 골랐는데 그 이름이 건넨 등록부에 없다"
            ),
        }
    }
}

// ── 선택 ───────────────────────────────────────────────────────────────────

/// 건넨 기록에서 비뿌리 사이클 `target` 의 세계 출처 S(N) 을 고르고 같은 N 의 실제 Entry 를 함께 담는다. **순수 함수**다.
///
/// 같은 입력에 3j 의 [`check_event_inputs`] 를 한 번 실행하고, 대상·부모의 위치를 그 결과와 무관하게 평가한다. 둘 다 성립할 때만 고른다
/// — 대응이 확인된 자리마다 `baseline.snapshot_ref` 가 있고, `baseline.after_cycle_ref` 가 N 의 구조적 부모 T 이며,
/// `baseline.next_cycle_id` 가 N 의 번호보다 크지 않은 사건 가운데 이름이 가장 큰 사건의 기준점. 그런 사건이 없으면 T 의
/// `exit_snapshot_ref` 이며, 그 이름이 등록부에 있어야 한다. 검증 ① 은 결과의 [`OpeningWorld::check_entry`] 가 한다. 입력·체인·등록부를
/// 바꾸지 않고 파일·상태·창고를 읽거나 쓰지 않는다.
pub(crate) fn select_opening_world(
    target: CycleRef,
    ledger: &PolicyLedger,
    links: &EventPolicyLinks,
    facts: &[EventCycleFacts],
    chains: &[Chain],
    registry: &SnapshotRegistry,
) -> Result<OpeningWorld, Vec<OpeningWorldProblem>> {
    let inputs = check_event_inputs(ledger, links, facts, chains, registry);
    let located = locate(target, chains);

    let (inputs, located) = match (inputs, located) {
        (Ok(inputs), Ok(located)) => (inputs, located),
        (inputs, located) => {
            let mut problems = Vec::new();
            if let Err(found) = inputs {
                problems.push(OpeningWorldProblem::Inputs(found));
            }
            if let Err(problem) = located {
                problems.push(problem);
            }
            return Err(problems);
        }
    };
    let Located { chain, parent, parent_exit, entry } = located;

    // 3j 가 성공했으므로 두 목록은 같은 수이고, 같은 자리의 이름과 종류가 같다.
    let before_target = links
        .events()
        .iter()
        .zip(facts)
        .filter_map(|(link, fact)| {
            let baseline = fact.baseline();
            let world = baseline.snapshot()?;
            let BaselineSlot::Written(Some(archived_at)) = baseline.boundary() else {
                return None;
            };
            if archived_at != parent {
                return None;
            }
            let BaselineSlot::Written(archive_issuer) = baseline.issuer() else {
                unreachable!("3e 의 성공은 기준점이 있는 사건에 baseline.next_cycle_id 칸이 있음을 보장한다")
            };
            (archive_issuer <= target.number()).then_some((link, world, archived_at, archive_issuer))
        })
        .max_by_key(|(link, ..)| link.name());

    let source = match before_target {
        Some((link, world, archived_at, archive_issuer)) => WorldSource::EventBaseline {
            event: link.name(),
            kind: link.kind(),
            world,
            policy: link.after().expect("3c 의 성공은 모든 사건의 after 에 판이 있음을 보장한다"),
            archived_at,
            archive_issuer,
        },
        None => {
            if registry.resolve(parent_exit).is_none() {
                return Err(vec![OpeningWorldProblem::ParentExitNotInRegistry { cycle: target, parent, world: parent_exit }]);
            }
            WorldSource::ParentExit { world: parent_exit }
        }
    };

    Ok(OpeningWorld { target, chain, parent, entry, source, inputs })
}
