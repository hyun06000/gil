//! **닫힌 Experiment T 의 저장 `exit_snapshot_ref` 가 마지막 Outcome 의 구조적 계보로 유도한 기대 세계와 같은가** 를 건넨 기록에서
//! 보는 순수 검사 — 제품 미연결.
//!
//! 정본은 [Artifact Model](../../../spec/GIL_Artifact_Model_v0.1.md) §7.2·§7.5(Exit 은 마지막 Outcome 의 구조적 lineage 에서 가장
//! 가까운 Verify Snapshot, 없으면 Entry 계승. 버려진 가지·이름이 가장 큰 Snapshot 을 쓰지 않는다)와 §7.3(닫힌 Verify 는 언제나
//! `snapshot_ref` 를 지닌다), [Cycle Model](../../../spec/GIL_Cycle_Model_v0.1.md) §9 의 「Cycle Exit」, 그리고
//! [공유 저장·전이 계약](../../../spec/GIL_Storage_Transition_Contract_v0.1.md) §6.1 의 「`exit_snapshot_ref` 는 한 번 쓰면 바꾸지
//! 않는다. 사건은 그 값들을 덮어쓰지 않고 옆에 선다」다. **§6.1 전체·검증 ①·②·T8·T9·제품 close/load 연결의 완료가 아니다.**
//!
//! ```text
//! 대상 CycleRef + &[Chain] ─ experiment_exit::owned_cycle (P(T)·select_experiment_exit 와 같은 소유 규칙) ─▶ Target 갈래
//!                          ─ Cycle::kind · Cycle::is_closed ─────────────────────────────────▶ Interview / 열린 Experiment 범위 밖
//!                                   │ 유일한 닫힌 Experiment 일 때만
//!                                   ├─ Cycle::exit_snapshot ─ 등록부 이름 조회 ─────────────▶ StoredExitMissing / StoredExitNotInRegistry
//!                                   └─ Walk::current (Cycle 을 닫은 자리 = 마지막 Outcome)
//!                                      ─ Walk::lineage (parent 사슬만) ─ 뒤에서부터 첫 닫힌 Verify
//!                                         ├ 있음 · snapshot 있음 → 기대 세계 = 그 snapshot_ref, 출처 = 그 Verify
//!                                         ├ 있음 · snapshot 없음 → LineageVerifyLacksSnapshot (더 오래된 Verify·Entry 로 우회하지 않음)
//!                                         └ 없음                → 기대 세계 = T 의 Entry, 출처 = Entry
//!                                      ─ 기대 세계의 등록부 이름 조회 ─────────────────────────▶ ExpectedNotInRegistry
//!                                   │ 모든 갈래가 성립할 때만
//!                                   ▼
//! ExitLineageCheck { T, 소유 Chain, 마지막 Outcome, 저장 Exit, 기대 세계, 그 출처, 일치 | 불일치 }
//! ```
//!
//! # 무엇과 견주는가
//!
//! - **기대 세계는 마지막 Outcome 의 `parent` 사슬에서 가장 가까운 닫힌 Verify 의 `snapshot_ref` 다.** 사슬은 기존
//!   [`Walk::lineage`](crate::walk::Walk::lineage) 로 읽는다 — 만든 순서도, 형제 가지도, `revisit_from` 도 타지 않는다. 그래서 되돌아가기로
//!   버려진 가지의 Verify, 이름(ID)이 가장 큰 Snapshot·Step, 목록의 마지막 항목, `revisit_from` 이 가리키는 실패 Outcome 의 계보는 출처가
//!   되지 않는다. 같은 Snapshot 을 여러 Verify 가 가리켜도(바꾸지 않은 Verify, §7.3) 출처는 계보에서 가장 가까운 그 Verify 하나다.
//! - **그 계보에 닫힌 Verify 가 없으면 T 의 Entry 를 계승한다.** 현재 문법의 Experiment 는 Outcome 의 부모가 Analysis, Analysis 의 부모가
//!   Verify 뿐이라 닫힌 Experiment 의 마지막 Outcome 계보에는 언제나 Verify 가 있다. Entry 갈래는 정본 규칙을 그대로 옮긴 것이며 현재 문법의
//!   기록에서는 나지 않는다.
//! - **닫힌 Verify 에 Snapshot 이 없는 손상은 손상으로 돌려준다.** 「Verify 없음」으로 바꾸지 않고, 더 오래된 Verify 나 Entry 로 조용히
//!   우회하지 않는다([`ExitLineageProblem::LineageVerifyLacksSnapshot`]). 기존 `Cycle::exit_would_be` 는 Cycle 이 **닫을 때** 쓰는 비공개
//!   규칙(`verify_in_lineage`)을 지나며, 그 규칙은 이 손상을 만나면 더 오래된 Verify·Entry 로 넘어가고 계보 조회 실패도 Entry 로 바꾼다.
//!   그래서 이 검사는 그 함수를 부르지 않고 같은 공개 계보 API 위에서 첫 닫힌 Verify 를 읽는다. 손상이 없는 기록에서 두 결과가 같다는 것은
//!   시험이 확인한다. 제품의 close·load 동작은 바꾸지 않았다.
//! - **판정으로 거르지 않는다.** `success`·`failure` 어느 쪽으로 닫혔든 같은 규칙이다.
//! - **나중의 사건·경계는 보지 않는다.** 이 검사는 정책 원장·사건 입력을 받지 않는다. 현재 경계 B(T), T 에서 나중에 보관한 사건 기준점,
//!   S(N), 지금 서 있는 경계의 현재 세계는 기대 세계가 아니다 — 사건은 과거의 Exit 을 고치지 않고 그 옆에 선다(§6.1).
//!
//! # 대상과 결과
//!
//! 대상은 [`select_experiment_exit`](super::select_experiment_exit) 와 **같은 소유 조회**([`super::experiment_exit`] 의 `owned_cycle`)로
//! 건넨 `Chain` 목록에서 하나로 정한다. 없는·여러 체인이 소유한·소유 체인 이름이 반복된 대상은 [`ExitLineageProblem::Target`] 이다.
//! Interview 와 열린 Experiment 는 **이 검사의 범위 밖**이며([`ExitLineageProblem::InterviewOutOfScope`]·
//! [`ExitLineageProblem::OpenExperimentOutOfScope`]) 일치도 불일치도 아니다.
//!
//! 유일한 닫힌 Experiment 에서는 저장 Exit 의 존재와 등록부 조회, 기대 세계의 유도와 그 등록부 조회를 **서로 무관하게** 평가해 성립하지
//! 않는 것을 함께 돌려준다 — 저장 Exit 이 없어도 계보의 손상을 숨기지 않고, 계보가 손상돼도 저장 Exit 의 등록부 누락을 숨기지 않는다.
//! 어느 것이든 있으면 **검사 불능**이며(`Err`) 일치·불일치를 말하지 않는다. 모두 성립하면 [`ExitLineageCheck`] 가 대상·소유 체인·마지막
//! Outcome·저장 Exit·기대 세계·그 출처(Verify 의 주소 또는 Entry)와 일치/불일치([`ExitLineageVerdict`])를 지닌다. **불일치는 검사 불능이
//! 아니라 정상적으로 끝난 검사의 결과다.**
//!
//! 등록부 조회는 이름이 건넨 등록부에 있는지만 본다. manifest·blob 의 실재와 내용은 증명하지 않는다. 일치는 저장 Exit 이 계보 규칙과
//! 같은 이름이라는 것뿐이며, P(T)·확정 정책의 선택, 승인, 이력의 완전성, T 개방의 검증 ①·②, 전체 상태의 유효성을 증명하지 않는다.
//! 보이지 않는 것은 [`EXIT_LINEAGE_DOES_NOT_SHOW`] 에 있다.

use std::fmt;

use super::super::SnapshotRegistry;
use super::experiment_exit::owned_cycle;
use super::opening_policy::TargetProblem;
use crate::refs::{ChainRef, CycleRef, SnapshotRef, StepRef};
use crate::{Chain, CycleKind, NodeKind};

/// 이 검사가 성공해도 **보이지 않는** 것 — 결과를 들고 있어도 이것들은 성립하지 않는다.
pub(crate) const EXIT_LINEAGE_DOES_NOT_SHOW: [&str; 10] = [
    "입력의 출처와 완전성 — 건넨 Chain 목록·등록부가 실제 프로젝트의 같은 시점에서 빠짐없이 뽑혔는지 보지 않는다",
    "세계의 실재 — 저장 Exit 과 기대 세계의 이름이 건넨 등록부에서 조회되는지만 본다. manifest·blob 의 실재와 내용, 관측이 실제로 그 세계였는지는 보지 않는다",
    "확정 정책과 승인 — P(T)·Exit 의 확정 정책을 고르지 않고 select_experiment_exit 의 결과를 바꾸지 않는다. 정책 선택·모니터 승인·보관의 실제 성공, 이력의 완전성을 증명하지 않는다",
    "계보 밖의 손상 — 마지막 Outcome 의 계보에서 가장 가까운 닫힌 Verify 까지만 읽는다. 버려진 가지·더 오래된 Verify 의 snapshot 누락, Verify 가 아닌 Step 의 snapshot, 열린 Verify 의 snapshot 은 보지 않는다(현행 Project load 의 세계 검사 ④가 본다)",
    "T 의 개방과 Entry — T 의 Entry 가 부모 Exit·S(T) 의 세계인지(검증 ①), S(T) 의 확정 정책 = P(T) 인지(검증 ②), 보관 대기 뒤에 연 실험인지(T9) 보지 않는다. Entry 를 계승할 때도 Entry 의 출처는 검사하지 않는다",
    "B(T)·S(N)·지금 서 있는 경계·나중 사건 — 사건 입력을 받지 않는다. T 에서 나중에 보관한 기준점, 현재 세계, 되돌아가기의 복원 목표와 견주지 않으며 B(T)·S(N) 의 ExitPolicyNotDerived 를 바꾸지 않는다",
    "Interview 와 열린 Experiment — 범위 밖 결과로 돌려준다. Interview Exit 의 Entry 계승·정책 상속, 열린 Experiment 의 현재 세계는 검사하지 않는다",
    "전체 상태의 유효성 — 건넨 Chain 목록의 직렬 계보·Entry/Exit 인계·발급기 경계, T8(world_before_ref), Cycle Report 의 outcome_ref 와 닫은 자리의 짝, 저장 load 의 다른 불변식을 대신 검사하지 않는다. 기존 복원 문이 지키는 불변식을 전제로 하며 어긋난 값은 검사 불능으로 돌려줄 뿐이다",
    "제품 동작 — 제품의 Cycle close·load 가 이 검사를 부르지 않고, 불일치를 고치거나 다른 세계로 바꾸는 문이 없다. 파일·state.yaml·객체 창고를 읽거나 쓰지 않으며 Monitor·CLI·MCP 에 연결되지 않았다. 형식 10/11 은 활성화되지 않았다",
    "미결 결정 — 즉시 보관 중단의 요청·해제(O-2), 보관 대기 중 Chain 종료와 미완료 사건의 이관(O-3), BaseUndecided 의 나머지 조합, 승인 이후 변경의 결속·보존·재적용에 아무 의미도 정하지 않는다",
];

// ── 결과 ───────────────────────────────────────────────────────────────────

/// 기대 세계를 어디서 얻었는가.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExpectedSource {
    /// 마지막 Outcome 의 계보에서 가장 가까운 닫힌 Verify — 그 Step 의 주소.
    Verify(StepRef),
    /// 그 계보에 닫힌 Verify 가 없어 T 의 Entry 를 계승했다.
    Entry,
}

impl fmt::Display for ExpectedSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExpectedSource::Verify(step) => write!(f, "Verify {step}"),
            ExpectedSource::Entry => f.write_str("Entry 계승"),
        }
    }
}

/// 저장 Exit 과 기대 세계의 비교. 둘 다 정상적으로 끝난 검사의 결과다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExitLineageVerdict {
    /// 저장 Exit 이 기대 세계와 같은 이름이다.
    Matches,
    /// 저장 Exit 이 기대 세계와 다른 이름이다 — 손상 판정·수정·대체 세계 선택이 아니다.
    Differs,
}

/// 건넨 기록 안에서 본 닫힌 Experiment T 의 저장 Exit 과 계보 기대 세계의 비교 — **그것뿐이다.** 보이지 않는 것은
/// [`EXIT_LINEAGE_DOES_NOT_SHOW`] 에 있다. 이 값을 만드는 문은 [`check_experiment_exit_lineage`] 하나뿐이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExitLineageCheck {
    target: CycleRef,
    chain: ChainRef,
    last_outcome: StepRef,
    stored: SnapshotRef,
    expected: SnapshotRef,
    source: ExpectedSource,
}

impl ExitLineageCheck {
    /// 대상 Experiment T.
    pub(crate) fn target(&self) -> CycleRef {
        self.target
    }

    /// 건넨 목록에서 T 를 소유한 체인.
    pub(crate) fn chain(&self) -> ChainRef {
        self.chain
    }

    /// T 를 닫은 자리 — 계보를 읽기 시작한 마지막 Outcome.
    pub(crate) fn last_outcome(&self) -> StepRef {
        self.last_outcome
    }

    /// T 에 저장된 `exit_snapshot_ref` — 건넨 등록부에서 이름이 조회됐다.
    pub(crate) fn stored(&self) -> SnapshotRef {
        self.stored
    }

    /// 계보 규칙으로 유도한 기대 세계 — 건넨 등록부에서 이름이 조회됐다.
    pub(crate) fn expected(&self) -> SnapshotRef {
        self.expected
    }

    /// 기대 세계의 출처 — 가장 가까운 닫힌 Verify 또는 Entry 계승.
    pub(crate) fn source(&self) -> ExpectedSource {
        self.source
    }

    /// 일치 / 불일치.
    pub(crate) fn verdict(&self) -> ExitLineageVerdict {
        match self.stored == self.expected {
            true => ExitLineageVerdict::Matches,
            false => ExitLineageVerdict::Differs,
        }
    }
}

/// 검사할 수 없는 이유. 갈래 순서는 대상(소유·범위)과, 유일한 닫힌 Experiment 에서의 저장 Exit·계보·기대 세계 조회다. 뒤의 갈래들은
/// 서로 무관하게 평가하므로 함께 올 수 있다. **어느 것도 일치나 불일치가 아니다.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExitLineageProblem {
    /// 대상 사이클을 건넨 목록에서 하나로 정할 수 없다 — `select_experiment_exit` 와 같은 소유 규칙의 문제 그대로.
    Target(TargetProblem),
    /// 대상이 Interview 다 — **범위 밖**이다. 열렸든 닫혔든 같다.
    InterviewOutOfScope { cycle: CycleRef, chain: ChainRef },
    /// 대상 Experiment 가 열려 있다 — **범위 밖**이다. 저장 Exit 이 아직 없다.
    OpenExperimentOutOfScope { cycle: CycleRef, chain: ChainRef },
    /// 닫힌 Experiment 에 `exit_snapshot_ref` 가 없다 — 견줄 저장값이 없다. 대신할 세계를 고르지 않는다.
    StoredExitMissing { cycle: CycleRef, chain: ChainRef },
    /// 저장 Exit 의 이름이 건넨 등록부에 없다.
    StoredExitNotInRegistry { cycle: CycleRef, chain: ChainRef, world: SnapshotRef },
    /// 닫힌 Experiment 의 서 있는 자리가 닫힌 Outcome 이 아니거나 그 계보를 읽을 수 없다. 기존 복원 문이 지키는 짝이 깨진 값이며
    /// 다른 자리나 Entry 로 대신하지 않는다.
    LastOutcomeUnreadable { cycle: CycleRef, chain: ChainRef },
    /// 마지막 Outcome 의 계보에서 가장 가까운 닫힌 Verify 에 `snapshot_ref` 가 없다. 「Verify 없음」이 아니며, 더 오래된 Verify 나 Entry 로
    /// 우회하지 않는다.
    LineageVerifyLacksSnapshot { cycle: CycleRef, chain: ChainRef, step: StepRef },
    /// 유도한 기대 세계의 이름이 건넨 등록부에 없다.
    ExpectedNotInRegistry { cycle: CycleRef, chain: ChainRef, world: SnapshotRef, source: ExpectedSource },
}

impl fmt::Display for ExitLineageProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExitLineageProblem::Target(problem) => write!(f, "Experiment 의 Exit 계보를 검사하지 않았다: {problem}"),
            ExitLineageProblem::InterviewOutOfScope { cycle, chain } => write!(
                f,
                "{cycle}({chain}) 은 Interview 다. Interview 의 Exit 은 이 검사의 범위 밖이며 일치도 불일치도 아니다"
            ),
            ExitLineageProblem::OpenExperimentOutOfScope { cycle, chain } => write!(
                f,
                "Experiment {cycle}({chain}) 가 열려 있다. 열린 Experiment 는 이 검사의 범위 밖이며 일치도 불일치도 아니다"
            ),
            ExitLineageProblem::StoredExitMissing { cycle, chain } => write!(
                f,
                "Experiment {cycle}({chain}) 는 닫혔는데 exit_snapshot_ref 가 없다. 견줄 저장값이 없어 검사하지 않았다"
            ),
            ExitLineageProblem::StoredExitNotInRegistry { cycle, chain, world } => {
                write!(f, "Experiment {cycle}({chain}) 의 저장 Exit {world} 이 건넨 등록부에 없다")
            }
            ExitLineageProblem::LastOutcomeUnreadable { cycle, chain } => write!(
                f,
                "Experiment {cycle}({chain}) 의 닫은 자리가 닫힌 Outcome 이 아니거나 그 계보를 읽을 수 없다. 다른 자리로 대신하지 않는다"
            ),
            ExitLineageProblem::LineageVerifyLacksSnapshot { cycle, chain, step } => write!(
                f,
                "Experiment {cycle}({chain}) 의 마지막 Outcome 계보에서 가장 가까운 닫힌 Verify {step} 에 snapshot_ref 가 없다. \
                 더 오래된 Verify 나 Entry 로 대신하지 않는다"
            ),
            ExitLineageProblem::ExpectedNotInRegistry { cycle, chain, world, source } => {
                write!(f, "Experiment {cycle}({chain}) 의 기대 세계 {world}({source}) 이 건넨 등록부에 없다")
            }
        }
    }
}

// ── 검사 ───────────────────────────────────────────────────────────────────

/// 건넨 기록에서 닫힌 Experiment `target` 의 저장 Exit 을 마지막 Outcome 의 구조적 계보로 유도한 기대 세계와 견준다. **순수 함수**다.
///
/// 대상을 하나로 정하고 종류·닫힘을 확인한 뒤, 저장 Exit 의 존재·등록부 조회와 기대 세계의 유도·등록부 조회를 서로 무관하게 평가한다.
/// 모두 성립할 때만 비교 결과를 돌려준다. 체인·등록부를 바꾸지 않고 파일·상태·창고를 읽거나 쓰지 않는다.
pub(crate) fn check_experiment_exit_lineage(
    target: CycleRef,
    chains: &[Chain],
    registry: &SnapshotRegistry,
) -> Result<ExitLineageCheck, Vec<ExitLineageProblem>> {
    let (chain, cycle) = owned_cycle(target, chains).map_err(|problem| vec![ExitLineageProblem::Target(problem)])?;
    match cycle.kind() {
        CycleKind::Interview => return Err(vec![ExitLineageProblem::InterviewOutOfScope { cycle: target, chain }]),
        CycleKind::Experiment => {}
    }
    if !cycle.is_closed() {
        return Err(vec![ExitLineageProblem::OpenExperimentOutOfScope { cycle: target, chain }]);
    }

    let mut problems = Vec::new();

    // 저장 Exit — 계보와 무관하게 본다.
    let stored = cycle.exit_snapshot();
    match stored {
        None => problems.push(ExitLineageProblem::StoredExitMissing { cycle: target, chain }),
        Some(world) if registry.resolve(world).is_none() => {
            problems.push(ExitLineageProblem::StoredExitNotInRegistry { cycle: target, chain, world });
        }
        Some(_) => {}
    }

    // 기대 세계 — 닫은 자리(마지막 Outcome)의 parent 사슬에서 가장 가까운 닫힌 Verify, 없으면 Entry.
    let steps = cycle.steps();
    let last = steps
        .current()
        .and_then(|here| steps.node(here))
        .filter(|node| node.kind == NodeKind::Outcome && node.is_closed())
        .and_then(|node| steps.lineage(node.id).ok().map(|lineage| (node.id, lineage)));
    let derived = match last {
        None => {
            problems.push(ExitLineageProblem::LastOutcomeUnreadable { cycle: target, chain });
            None
        }
        Some((outcome, lineage)) => {
            let nearest = lineage.iter().rev().find(|node| node.kind == NodeKind::Verify && node.is_closed());
            let expected = match nearest {
                None => Some((cycle.entry_snapshot(), ExpectedSource::Entry)),
                Some(verify) => match verify.snapshot {
                    Some(world) => Some((world, ExpectedSource::Verify(cycle.step_ref(verify.id)))),
                    None => {
                        let step = cycle.step_ref(verify.id);
                        problems.push(ExitLineageProblem::LineageVerifyLacksSnapshot { cycle: target, chain, step });
                        None
                    }
                },
            };
            expected.map(|(world, source)| (cycle.step_ref(outcome), world, source))
        }
    };
    if let Some((_, world, source)) = derived
        && registry.resolve(world).is_none()
    {
        problems.push(ExitLineageProblem::ExpectedNotInRegistry { cycle: target, chain, world, source });
    }

    match (stored, derived) {
        (Some(stored), Some((last_outcome, expected, source))) if problems.is_empty() => {
            Ok(ExitLineageCheck { target, chain, last_outcome, stored, expected, source })
        }
        _ => Err(problems),
    }
}
