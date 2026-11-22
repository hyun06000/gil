//! **사이클 N 을 열 당시의 적용 정책 P(N)** 을 건넨 기록에서 고르는 순수 선택 — 계약 §6.2 의 P(N) 한 줄, 제품 미연결.
//!
//! 이 모듈은 §6.2 의 정의 하나만 옮긴다. **§6.1 전체, 마지막 확정 세계·확정 정책의 유도, 세계 출처 S(N)·B(T), 검증 ①·②,
//! 제품 연결의 완료가 아니다.** 정본은 [공유 저장·전이 계약](../../../spec/GIL_Storage_Transition_Contract_v0.1.md) §6.2 와
//! §5.1(사건 칸 표의 `next_cycle_id`·`policy_after_ref`), §5.7 의 T1·T3·T4·T7 이다.
//!
//! ```text
//! N 이전의 사건          next_cycle_id ≤ N 인 사건                     (승인이 N 보다 먼저 저장됐다)
//! P(N)                  N 이전의 사건 가운데 이름이 가장 큰 것의 policy_after (없으면 정책 없음)
//!
//! (PolicyLedger, EventPolicyLinks, EventCycleFacts*, &[Chain], 등록부) ─ check_event_inputs (3j, 그대로) ─▶ Inputs 갈래
//! 대상 CycleRef + &[Chain] ─ Chain::owns · Chain::id (읽기 전용, 모호하면 고르지 않음) ──────────────────▶ Target 갈래
//!                                   │ 둘 다 성립할 때만
//!                                   ▼
//! 대응이 확인된 자리마다 (사건 이름·종류·policy_after, 사건의 next_cycle_id) ─ next_cycle_id ≤ N, 이름 최대 ─▶ OpeningPolicy
//! ```
//!
//! # 무엇으로 고르는가
//!
//! - **사건의 `next_cycle_id`** — 승인을 저장할 때의 Cycle 발급기 값이다(§5.1). 「정책이 언제 바뀌었는지」를 시각 없이 말한다.
//!   `baseline.next_cycle_id` 는 **보관** 시점의 값이므로 이 선택의 기준이 아니다. 보관 대기 중 인터뷰가 닫힌 뒤 다른 경계에서
//!   보관한 사건은 두 값이 다르다(§6.3·§6.5 (가)).
//! - **기준점 보관 여부로 거르지 않는다.** 보관 대기 사건·뒤 사건으로 대체된 사건도 승인 저장으로 이미 적용된 사건이다(§5.2
//!   「승인 기록 저장 = 정책 적용」, §6.2 는 사건과 기준점을 따로 정의한다).
//! - **`restore_result` 도 사건이다.** 새 판을 도입하지 않아도 선택된 사건 이름은 그 사건으로 남는다. 같은 판을 앞선 사건이
//!   도입했다는 이유로 출처를 그 사건으로 바꾸지 않는다.
//! - **판 이름으로 남긴다.** 같은 원문을 다시 승인한 판(`P1`·`P3`)은 별개 판이다(§5.1). 원문 지문으로 판을 합치지 않는다.
//! - **현재 원장의 마지막 판을 대입하지 않는다.** 과거의 N 에는 그 뒤에 저장된 사건이 닿지 않는다.
//! - **발급기를 섞지 않는다.** `next_event_id`(사건 발급기)·사건 이름의 수를 사이클 번호와 견주지 않는다. 견주는 것은 사건의
//!   `next_cycle_id` 와 대상 사이클의 번호뿐이다. 사이클 번호는 Chain 사이에서도 한 발급기가 이어 준다(`Project::open_child_chain`).
//! - **구조를 보지 않는다.** 대상의 부모·`revisit_from`·Chain 은 P(N) 의 정의에 들지 않는다. 사건의 `chain_ref` 로 거르지도 않는다.
//!
//! # 입력과 검사
//!
//! 3j 의 [`check_event_inputs`] 를 **같은 호출에서 받은 입력에** 실행하고 실패를 그대로 담는다. 다른 입력에서 얻은 성공 값만 받아
//! 검사를 건너뛰는 문은 없다. 3j 가 성공한 뒤에만, 3j 가 자리·이름·종류의 대응을 확인한 바로 그 두 목록에서 사건의 정책 연결
//! (3c 입력)과 `next_cycle_id`(3h 입력)를 읽는다.
//!
//! 대상은 건넨 `Chain` 목록에서 **실재하고 하나로 정해지는** 사이클이다. 없는 대상·여러 체인이 소유한 대상·소유 체인의 이름이
//! 목록에 여러 번 있는 대상은 [`TargetProblem`] 으로 돌려주며 「정책 없음」으로 바꾸지 않는다. 대상의 위치는 3j 의 결과와 무관하게
//! 평가하고, 두 갈래의 실패를 함께 돌려준다.
//!
//! # 결과가 말하는 것
//!
//! 「정책 채택 전」([`OpeningSelection::BeforeAdoptionInGivenHistory`])과 입력 오류(`Err`)는 다른 값이다. 정책 채택 전은 **건넨 이력
//! 안에서** N 이전의 사건이 없다는 뜻일 뿐이다. 성공 값은 3j 의 성공 값을 그대로 지닌다 — 3h 의 `null`·칸 없음(`not_judged`)과
//! `chain_ref` 미조회(`chains_not_looked_up`)가 숨지 않는다.
//!
//! P(N) 은 Interview 의 확정 정책이 아니다(§6.1 — Interview 의 Exit 은 Entry 의 확정 정책을 물려받는다). 보관 대기 중에 연 인터뷰는
//! P(N) 이 새 판이어도 그 세계는 앞 판으로 확정됐다. 그래서 이 결과를 3o 의 시작점·3n 의 구간으로 바꾸는 문이 없다. 보이지 않는
//! 것은 [`OPENING_POLICY_DOES_NOT_SHOW`] 에 있다.

use std::fmt;

use super::super::SnapshotRegistry;
use super::event_cycles::EventCycleFacts;
use super::event_inputs::{EventInputsAgree, EventInputsProblem, check_event_inputs};
use super::event_links::{EventKind, EventName, EventPolicyLinks};
use super::{PolicyLedger, PolicyName};
use crate::Chain;
use crate::refs::{ChainRef, CycleRef};

/// 이 선택이 성공해도 **보이지 않는** 것 — 결과를 들고 있어도 이것들은 성립하지 않는다.
pub(crate) const OPENING_POLICY_DOES_NOT_SHOW: [&str; 10] = [
    "입력의 출처와 완전성 — 건넨 원장·사건 입력·Chain 목록·등록부가 실제 프로젝트의 같은 시점에서 빠짐없이 뽑혔는지 보지 않는다. 원장과 두 사건 입력을 함께 잘라낸 입력은 알아내지 못하며 잘린 이력 안에서 고른다. 3j 의 EVENT_INPUTS_DO_NOT_SHOW 그대로다",
    "실제 승인·적용 — 고른 사건의 모니터 승인 응답(proposal_digest)과 승인 저장의 실제 성공을 확인하지 않는다. 성공은 건넨 기록 안의 선택이다",
    "3j 의 판단하지 않음 — after_cycle_ref·baseline.after_cycle_ref 의 null·칸 없음(not_judged)과 chain_ref 미조회(chains_not_looked_up)는 판단하지 않은 채 성공 값의 inputs() 에 그대로 남는다",
    "확정 정책과 마지막 확정 세계 — P(N) 은 N 을 열 당시의 적용 정책이다. Interview 의 확정 정책(Entry 에서 물려받음)이 아니며, 보관 대기 중에 연 인터뷰의 세계를 확정한 정책도 아니다. §6.1 의 유도는 하지 않는다",
    "세계 출처 — S(N)·B(T)·N 이전의 기준점, 검증 ①(Entry = S(N) 의 세계)·②(실험의 확정 정책 = P(N)), 실험을 열 자격(T9)을 보지 않는다",
    "3n·3o 와의 연결 — 결과를 LedgerStart·ConfirmedStart·AppliedSpan 으로 바꾸는 문이 없다. 경로 판정·구간 선택의 시작점은 호출자가 §6.1 로 따로 정한다",
    "대상의 계보 — 대상의 부모·revisit_from·종류·열림/닫힘과 사건의 chain_ref 는 선택에 쓰지 않는다. 대상이 건넨 목록에서 실재하고 하나로 정해지는지만 본다",
    "제안 정책과 디스크의 .gilignore — 입력이 아니다. 승인 대기 제안이나 파일 내용으로 P(N) 을 정하지 않는다",
    "보관·재시도·복원·Monitor·CLI·MCP — 이 선택을 부르는 제품 경로가 없고 파일·state.yaml·객체 창고를 읽거나 쓰지 않는다. 형식 10/11 은 활성화되지 않았다",
    "미결 결정 — 즉시 보관 중단의 요청·해제(O-2), 보관 대기 중 Chain 종료와 미완료 사건의 이관(O-3), BaseUndecided 의 나머지 조합에 아무 의미도 정하지 않는다",
];

// ── 대상 ───────────────────────────────────────────────────────────────────

/// 대상 사이클을 건넨 `Chain` 목록에서 하나로 정할 수 없는 이유. **「정책 없음」이 아니다.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TargetProblem {
    /// 건넨 목록의 어느 체인도 그 사이클을 소유하지 않는다.
    NotFound { cycle: CycleRef },
    /// 둘 이상의 체인 항목이 그 사이클을 소유한다(목록 순서).
    SeveralOwners { cycle: CycleRef, chains: Vec<ChainRef> },
    /// 그 사이클을 소유한 체인의 이름이 목록에 `count` 번 있다 — 어느 기록의 사이클인지 정할 수 없다.
    OwnerChainRepeated { cycle: CycleRef, chain: ChainRef, count: usize },
}

impl fmt::Display for TargetProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TargetProblem::NotFound { cycle } => {
                write!(f, "대상 {cycle} 을 건넨 체인 목록의 어느 체인도 소유하지 않는다. 정책 없음이 아니다")
            }
            TargetProblem::SeveralOwners { cycle, chains } => write!(
                f,
                "대상 {cycle} 을 여러 체인 항목({})이 소유한다. 대상을 하나로 정할 수 없다",
                chains.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")
            ),
            TargetProblem::OwnerChainRepeated { cycle, chain, count } => write!(
                f,
                "대상 {cycle} 을 소유한 {chain} 이 건넨 체인 목록에 {count}번 있다. 대상을 하나로 정할 수 없다"
            ),
        }
    }
}

/// 대상을 소유한 체인 — 기존 소유 관계([`Chain::owns`])와 체인 이름([`Chain::id`])만 읽는다. 첫 항목을 고르지 않는다.
fn locate(target: CycleRef, chains: &[Chain]) -> Result<ChainRef, TargetProblem> {
    let owners: Vec<ChainRef> = chains.iter().filter(|chain| chain.owns(target)).map(Chain::id).collect();
    match owners.as_slice() {
        [] => Err(TargetProblem::NotFound { cycle: target }),
        [owner] => match chains.iter().filter(|chain| chain.id() == *owner).count() {
            1 => Ok(*owner),
            count => Err(TargetProblem::OwnerChainRepeated { cycle: target, chain: *owner, count }),
        },
        _ => Err(TargetProblem::SeveralOwners { cycle: target, chains: owners }),
    }
}

// ── 결과 ───────────────────────────────────────────────────────────────────

/// 건넨 이력 안에서 N 을 열 당시의 적용 정책.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpeningSelection {
    /// 건넨 이력에 `next_cycle_id ≤ N` 인 사건이 없다 — **건넨 이력 안에서** 정책 채택 전이다. 입력 오류의 대용이 아니며,
    /// 잘린 이력이 아니라는 증명도 아니다.
    BeforeAdoptionInGivenHistory,
    /// `next_cycle_id ≤ N` 인 사건 가운데 이름이 가장 큰 사건과 그 `policy_after`.
    Applied {
        /// 고른 사건의 이름 — `restore_result` 라도 그 사건이다.
        event: EventName,
        /// 고른 사건의 종류(3j 가 두 입력에서 같다고 확인한 값).
        kind: EventKind,
        /// 고른 사건의 `policy_after_ref` — 판 이름이다. 원문 지문이 아니다.
        policy: PolicyName,
        /// 고른 사건의 승인 당시 `next_cycle_id`(보관 시점의 `baseline.next_cycle_id` 가 아니다).
        next_cycle_id: u32,
    },
}

/// 건넨 기록 안에서 고른 P(N) — **그것뿐이다.** 보이지 않는 것은 [`OPENING_POLICY_DOES_NOT_SHOW`] 에 있다. 이 값을 만드는 문은
/// [`select_opening_policy`] 하나뿐이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpeningPolicy {
    target: CycleRef,
    chain: ChainRef,
    selection: OpeningSelection,
    inputs: EventInputsAgree,
}

impl OpeningPolicy {
    /// 대상 사이클 N.
    pub(crate) fn target(&self) -> CycleRef {
        self.target
    }

    /// 건넨 목록에서 N 을 소유한 체인. 선택에는 쓰지 않았다.
    pub(crate) fn chain(&self) -> ChainRef {
        self.chain
    }

    /// 고른 결과 — 정책 채택 전 또는 사건·판.
    pub(crate) fn selection(&self) -> OpeningSelection {
        self.selection
    }

    /// 같은 호출에서 실행한 3j 의 성공 값 그대로 — 그 판단하지 않음(`not_judged`·`chains_not_looked_up`)을 지닌다.
    pub(crate) fn inputs(&self) -> &EventInputsAgree {
        &self.inputs
    }
}

/// 선택이 실패한 갈래. 갈래 순서는 입력 검사, 대상이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OpeningPolicyProblem {
    /// 같은 호출에 건넨 입력이 3j 를 통과하지 못했다 — 그 실패 그대로.
    Inputs(Vec<EventInputsProblem>),
    /// 대상 사이클을 건넨 목록에서 하나로 정할 수 없다.
    Target(TargetProblem),
}

impl fmt::Display for OpeningPolicyProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpeningPolicyProblem::Inputs(found) => write!(
                f,
                "건넨 사건 입력이 맞지 않아 P(N) 을 고르지 않았다: {}",
                found.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
            ),
            OpeningPolicyProblem::Target(problem) => write!(f, "P(N) 을 고르지 않았다: {problem}"),
        }
    }
}

// ── 선택 ───────────────────────────────────────────────────────────────────

/// 건넨 기록에서 사이클 `target` 을 열 당시의 적용 정책 P(N) 을 고른다. **순수 함수**다.
///
/// 같은 입력에 3j 의 [`check_event_inputs`] 를 실행하고, 대상의 위치를 그 결과와 무관하게 평가한다. 둘 다 성립할 때만 고른다 —
/// 대응이 확인된 자리마다 사건의 `next_cycle_id` 가 대상의 번호보다 크지 않은 사건 가운데 이름이 가장 큰 사건의 `policy_after`.
/// 그런 사건이 없으면 건넨 이력 안에서 정책 채택 전이다. 입력·체인·등록부를 바꾸지 않고 파일·상태·창고를 읽거나 쓰지 않는다.
pub(crate) fn select_opening_policy(
    target: CycleRef,
    ledger: &PolicyLedger,
    links: &EventPolicyLinks,
    facts: &[EventCycleFacts],
    chains: &[Chain],
    registry: &SnapshotRegistry,
) -> Result<OpeningPolicy, Vec<OpeningPolicyProblem>> {
    let inputs = check_event_inputs(ledger, links, facts, chains, registry);
    let located = locate(target, chains);

    let (inputs, chain) = match (inputs, located) {
        (Ok(inputs), Ok(chain)) => (inputs, chain),
        (inputs, located) => {
            let mut problems = Vec::new();
            if let Err(found) = inputs {
                problems.push(OpeningPolicyProblem::Inputs(found));
            }
            if let Err(problem) = located {
                problems.push(OpeningPolicyProblem::Target(problem));
            }
            return Err(problems);
        }
    };

    // 3j 가 성공했으므로 두 목록은 같은 수이고, 같은 자리의 이름과 종류가 같다.
    let selection = links
        .events()
        .iter()
        .zip(facts)
        .filter(|(_, fact)| fact.issuer() <= target.number())
        .max_by_key(|(link, _)| link.name())
        .map_or(OpeningSelection::BeforeAdoptionInGivenHistory, |(link, fact)| OpeningSelection::Applied {
            event: link.name(),
            kind: link.kind(),
            policy: link.after().expect("3c 의 성공은 모든 사건의 after 에 판이 있음을 보장한다"),
            next_cycle_id: fact.issuer(),
        });

    Ok(OpeningPolicy { target, chain, selection, inputs })
}
