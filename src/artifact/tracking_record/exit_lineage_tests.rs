//! 닫힌 Experiment T 의 저장 `exit_snapshot_ref` 와 마지막 Outcome 의 구조적 계보로 유도한 기대 세계의 비교(Artifact Model §7.5) 시험 —
//! **기존 Cycles 전이로 만든 메모리 체인**과 이름을 밝힌 **복원 구조**, 그리고 한 곳의 **합성 문법**을 쓴다.
//!
//! 등록부는 시험이 만든 합성 값이다. `state.yaml`·`gil start`·파일 시스템을 쓰지 않는다. 기대값은 구현에서 옮기지 않고 정본에서
//! 도출했다 — 「마지막 Outcome 의 구조적 lineage 에서 가장 최근에 확정된 Verify Snapshot, 없으면 Entry 계승」, 「버려진 가지의 Verify
//! Snapshot·ID 가 가장 큰 Snapshot 을 고르지 않는다」(§7.2·§7.5), 「Snapshot 을 가리키지 않는 Closed Verify 를 허용하지 않는다」(§7.3),
//! 「Entry 와 Exit 이 같은 SnapshotRef 일 수 있다」(§7.2), Cycle Model §9 의 「Cycle Exit」.
//!
//! ```text
//! tree()        experiment_exit_tests 의 체인 그대로
//!   cycle:C1 Interview (닫힘)            Entry A1 · Exit A1
//!   cycle:C2 Experiment (닫힘·success)   Entry A1 · S3 Verify A2 → Exit A2
//!   cycle:C3 Experiment (닫힘·failure)   Entry A2 · S3 Verify A3 → Exit A3
//!   cycle:C4 Experiment (열림)           parent C2 · revisit_from C3 · Entry A2
//!
//! branching()   Cycles 층의 실제 Step 되돌아가기를 지난 체인
//!   cycle:C1 Interview (닫힘)            Entry A1 · Exit A1
//!   cycle:C2 Experiment (닫힘·success)   Entry A1
//!       S1 Define ─ S2 Hypothesis ─ S3 Verify A2 ─ S4 Analysis ─┬─ S5 Hypothesis ─ S6 Verify A4 ─ S7 Analysis ─ S8 Outcome failure (revisit → S4)
//!                                                               └─ S9 Hypothesis (revisit_from S8) ─ S10 Verify A3 ─ S11 Analysis ─ S12 Outcome success
//!       → Exit A3   (버려진 가지 S6 의 A4 는 가장 큰 이름, S3 의 A2 는 더 오래된 Verify, A1 은 Entry)
//!   cycle:C3 Experiment (닫힘·failure)   Entry A3 · S3 Verify A3(바꾸지 않은 세계를 다시 가리킴) → Exit A3
//! ```
//!
//! 현재 문법의 Experiment 는 Outcome ← Analysis ← Verify 로만 이어지므로 Verify 없는 Experiment 종료를 만들 수 없다. Entry 계승 갈래는
//! Experiment 의 Define 뒤에 Outcome 을 허락하는 **합성 문법**(현재 문법이 아니다)으로만 걷는다. 복원 구조는 `Cycles::restore` 의 그래프
//! 불변식을 통과한 값이며, 저장 Exit 을 바꾼 값·Verify 의 snapshot 을 지운 값은 현행 `Project` load 의 세계 검사(③·④)가 거절할 값이다.
//! 기존 복원 문이 먼저 거절하는 값은 그 사실을 적고 불변식을 풀지 않는다. 어떤 시험도 실제 관측·보관·승인·제품 load 성공을 검증했다고
//! 말하지 않는다.

use super::event_cycles_tests::{a, c, ch, close_interview, registry, rules, x1};
use super::experiment_exit_tests::{at, close_experiment, restored, restored_under, tree};
use super::*;
use crate::artifact::SnapshotRegistry;
use crate::cycles::Cycles;
use crate::refs::{JourneyRef, StepRef};
use crate::walk::{NodeId, RestoreError};
use crate::{Chain, Cycle, CycleKind, NodeKind, RuleSet};

use ExitLineageVerdict::{Differs, Matches};

#[path = "../../../tests/common/mod.rs"]
mod common;

// ── 체인 ───────────────────────────────────────────────────────────────────

/// 지금 서 있는 자리에서 `kind` 를 열고 닫는다(Verify·Outcome 이 아닌 Step).
fn walk(cycle: &mut Cycle, kind: NodeKind) -> NodeId {
    cycle.open_step(kind).unwrap_or_else(|err| panic!("{kind} 를 열지 못했다: {err}"));
    let report = common::full_report(cycle.rules(), cycle.kind(), kind);
    cycle.close_step(report).unwrap_or_else(|err| panic!("{kind} 를 닫지 못했다: {err}"));
    cycle.steps().current().expect("방금 닫은 자리에 서 있다")
}

/// Verify 를 열고 세계 `A<world>` 를 확정하며 닫는다.
fn verify(cycle: &mut Cycle, world: u32) -> NodeId {
    cycle.open_step(NodeKind::Verify).expect("Verify 를 연다");
    let report = common::full_report(cycle.rules(), cycle.kind(), NodeKind::Verify);
    cycle.close_verify_step(report, a(world)).expect("Verify 를 닫는다");
    cycle.steps().current().expect("방금 닫은 Verify 에 서 있다")
}

/// Outcome 을 `verdict` 로 닫는다. `revisit` 이 있으면 그 자리로 되돌아간다고 적고, 없으면 Cycle 경계로 간다고 적는다.
fn outcome(cycle: &mut Cycle, verdict: &str, revisit: Option<NodeId>) -> NodeId {
    cycle.open_step(NodeKind::Outcome).expect("Outcome 을 연다");
    let mut report = common::full_report(cycle.rules(), cycle.kind(), NodeKind::Outcome)
        .with("verdict", verdict)
        .with("lesson", "이 갈래가 남긴 것 — 시험 자료")
        .with(common::REASON, "판정을 다음 자리로 넘긴다");
    match revisit {
        Some(target) => {
            report.insert(common::ACTION, "revisit");
            report.insert(common::TARGET, cycle.step_ref(target).to_string());
        }
        None => report.insert(common::ACTION, "close_cycle"),
    }
    cycle.close_step(report).expect("Outcome 을 닫는다");
    cycle.steps().current().expect("방금 닫은 Outcome 에 서 있다")
}

/// 서 있는 Experiment 를 `success` 로 닫고 자식을 열겠다고 적는다.
fn close_success(cycles: &mut Cycles, last: NodeId) {
    let mut report = common::cycle_report(cycles.current(), "success", last);
    report.insert(common::ACTION, "open_child");
    cycles.close_current(report, JourneyRef::new(x1(), 0)).expect("Experiment 를 닫는다");
}

/// 위 그림의 `branching` — 모두 기존 전이 함수로 걸어 만든다.
fn branching() -> Vec<Chain> {
    let mut cycles = Cycles::start_at(rules(), x1(), a(1), 1);
    close_interview(&mut cycles, "open_child");
    cycles.open_child(CycleKind::Experiment, x1()).expect("승인된 인터뷰 뒤 C2 를 연다");
    let last = {
        let cycle = cycles.current_mut();
        walk(cycle, NodeKind::Define);
        walk(cycle, NodeKind::Hypothesis);
        verify(cycle, 2);
        let fork = walk(cycle, NodeKind::Analysis);
        walk(cycle, NodeKind::Hypothesis);
        verify(cycle, 4);
        walk(cycle, NodeKind::Analysis);
        outcome(cycle, "failure", Some(fork));
        cycle.revisit_step().expect("적어 둔 대로 S4 로 되돌아간다");
        walk(cycle, NodeKind::Hypothesis);
        verify(cycle, 3);
        walk(cycle, NodeKind::Analysis);
        outcome(cycle, "success", None)
    };
    close_success(&mut cycles, last);
    cycles.open_child(CycleKind::Experiment, x1()).expect("성공한 C2 뒤 C3 을 연다");
    close_experiment(&mut cycles, 3, "failure");
    vec![Chain::restore(ch(1), x1(), a(1), None, cycles)]
}

/// Experiment 의 Define 뒤에 Outcome 을 허락하는 **합성 문법** — 현재 문법이 아니다. 그 밖의 규칙은 현재 문법 그대로다.
fn without_verify_rules() -> RuleSet {
    let text = std::fs::read_to_string(common::SPEC_PATH).expect("현재 문법을 읽는다");
    let define = "          - cycle_entry\n        allowed_children:\n          - hypothesis\n";
    let outcome = "판정하고 Cycle 경계로 갈 방향을 정한다\n        allowed_parents:\n          - analysis\n";
    assert_eq!((text.matches(define).count(), text.matches(outcome).count()), (1, 1), "바꿀 자리가 하나씩이다");
    let text = text
        .replace(define, &format!("{define}          - outcome\n"))
        .replace(outcome, &format!("{outcome}          - define\n"));
    RuleSet::from_yaml_str(&text).expect("합성 문법을 읽는다").for_chain()
}

/// 합성 문법에서 Interview C1 뒤 Experiment C2 를 Define → Outcome(success) 으로 닫은 체인 — C2 의 계보에 Verify 가 없다.
fn entry_only() -> Vec<Chain> {
    let mut cycles = Cycles::start_at(without_verify_rules(), x1(), a(1), 1);
    close_interview(&mut cycles, "open_child");
    cycles.open_child(CycleKind::Experiment, x1()).expect("C2 를 연다");
    let last = {
        let cycle = cycles.current_mut();
        walk(cycle, NodeKind::Define);
        outcome(cycle, "success", None)
    };
    close_success(&mut cycles, last);
    vec![Chain::restore(ch(1), x1(), a(1), None, cycles)]
}

fn step(cycle: u32, number: u32) -> StepRef {
    StepRef::new(c(cycle), number).expect("1 부터 센다")
}

fn checked(target: u32, chains: &[Chain], known: &SnapshotRegistry) -> ExitLineageCheck {
    check_experiment_exit_lineage(c(target), chains, known)
        .unwrap_or_else(|problems| panic!("cycle:C{target} 을 검사하지 못했다: {problems:#?}"))
}

fn refused(target: u32, chains: &[Chain], known: &SnapshotRegistry) -> Vec<ExitLineageProblem> {
    check_experiment_exit_lineage(c(target), chains, known).expect_err("검사 불능이어야 한다")
}

/// (기대 세계 번호, 출처, 저장 Exit 번호, 판정, 마지막 Outcome).
fn summary(check: &ExitLineageCheck) -> (u32, ExpectedSource, u32, ExitLineageVerdict, StepRef) {
    (check.expected().number(), check.source(), check.stored().number(), check.verdict(), check.last_outcome())
}

fn cycle_of(chains: &[Chain], number: u32) -> &Cycle {
    chains[0].cycles().nodes().iter().find(|cycle| cycle.id().to_ref() == c(number)).expect("그 사이클이 있다")
}

#[test]
fn the_branching_fixture_is_what_the_picture_says() {
    let chains = branching();
    let c2 = cycle_of(&chains, 2);
    let steps_of = |cycle: &Cycle| -> Vec<(u32, NodeKind, Option<u32>, Option<u32>, Option<u32>)> {
        cycle
            .steps()
            .nodes()
            .iter()
            .map(|node| {
                (
                    node.id.raw(),
                    node.kind,
                    node.parent.map(NodeId::raw),
                    node.revisit_from.map(NodeId::raw),
                    node.snapshot.map(|world| world.number()),
                )
            })
            .collect()
    };
    use NodeKind::{Analysis, Define, Hypothesis, Outcome, Verify};
    assert_eq!(
        steps_of(c2),
        [
            (1, Define, None, None, None),
            (2, Hypothesis, Some(1), None, None),
            (3, Verify, Some(2), None, Some(2)),
            (4, Analysis, Some(3), None, None),
            (5, Hypothesis, Some(4), None, None),
            (6, Verify, Some(5), None, Some(4)),
            (7, Analysis, Some(6), None, None),
            (8, Outcome, Some(7), None, None),
            (9, Hypothesis, Some(4), Some(8), None),
            (10, Verify, Some(9), None, Some(3)),
            (11, Analysis, Some(10), None, None),
            (12, Outcome, Some(11), None, None),
        ]
    );
    let c3 = cycle_of(&chains, 3);
    let worlds = |cycle: &Cycle| (cycle.entry_snapshot().number(), cycle.exit_snapshot().map(|world| world.number()));
    assert_eq!((worlds(c2), worlds(c3)), ((1, Some(3)), (3, Some(3))));
    assert_eq!(c3.steps().node(NodeId::from_raw(3)).and_then(|node| node.snapshot), Some(a(3)));
    // 바꾸지 않고 다시 세우면 같은 구조다(복원 도움 함수는 닫힌 Step 의 Journey 칸만 채운다).
    let again = vec![restored(&chains[0], |_| {}).expect("그대로 복원된다")];
    assert_eq!((steps_of(cycle_of(&again, 2)), worlds(cycle_of(&again, 2)), worlds(cycle_of(&again, 3))), (steps_of(c2), worlds(c2), worlds(c3)));
}

// ── 일치 ───────────────────────────────────────────────────────────────────

#[test]
fn success_and_failure_are_checked_by_the_same_rule_and_agree_with_the_close_rule() {
    let tree = tree();
    assert_eq!(summary(&checked(2, &tree, &registry(3))), (2, ExpectedSource::Verify(step(2, 3)), 2, Matches, step(2, 5)));
    assert_eq!(summary(&checked(3, &tree, &registry(3))), (3, ExpectedSource::Verify(step(3, 3)), 3, Matches, step(3, 5)));
    let check = checked(3, &tree, &registry(3));
    assert_eq!((check.target(), check.chain()), (c(3), ch(1)));

    // 손상이 없는 기록에서는 Cycle 을 닫을 때의 규칙(exit_would_be)과 같은 세계를 기대한다.
    for (chains, known) in [(tree, registry(3)), (branching(), registry(4))] {
        for cycle in chains[0].cycles().nodes().iter().filter(|cycle| cycle.kind() == CycleKind::Experiment && cycle.is_closed()) {
            let check = checked(cycle.id().to_ref().number(), &chains, &known);
            assert_eq!((check.expected(), Some(check.stored())), (cycle.exit_would_be(), cycle.exit_snapshot()));
        }
    }
}

#[test]
fn the_nearest_verify_of_the_last_outcome_wins_over_abandoned_older_entry_and_largest() {
    let chains = branching();
    let check = checked(2, &chains, &registry(4));
    assert_eq!(summary(&check), (3, ExpectedSource::Verify(step(2, 10)), 3, Matches, step(2, 12)));
    // 버려진 가지 S6 의 A4(가장 큰 이름이자 revisit_from 이 가리키는 S8 의 계보), 더 오래된 S3 의 A2, Entry A1 이 아니다.
    assert!(![a(4), a(2), a(1)].contains(&check.expected()));
}

#[test]
fn a_verify_that_points_again_at_an_unchanged_world_is_still_the_source() {
    let chains = branching();
    let check = checked(3, &chains, &registry(4));
    assert_eq!(summary(&check), (3, ExpectedSource::Verify(step(3, 3)), 3, Matches, step(3, 5)));
    assert_eq!(cycle_of(&chains, 3).entry_snapshot(), check.expected(), "Entry 와 같은 세계지만 출처는 Verify 다");
}

#[test]
fn entry_is_inherited_only_where_the_lineage_has_no_verify() {
    // 현재 문법: Experiment 의 Outcome 은 Analysis 뒤에만, Analysis 는 Verify 뒤에만 열린다 — Verify 없는 Experiment 종료는 만들 수 없다.
    let current = rules();
    let parents = |kind| current.rules(CycleKind::Experiment, kind).expect("선언된 Step").allowed_parents.clone();
    assert_eq!((parents(NodeKind::Outcome), parents(NodeKind::Analysis)), (vec![NodeKind::Analysis], vec![NodeKind::Verify]));

    // 합성 문법(현재 문법이 아니다)의 Define → Outcome 종료: 저장 Exit 은 Entry 를 계승했고 검사도 Entry 를 기대한다.
    let chains = entry_only();
    assert_eq!(summary(&checked(2, &chains, &registry(2))), (1, ExpectedSource::Entry, 1, Matches, step(2, 2)));
    let moved = vec![
        restored_under(without_verify_rules(), &chains[0], |states| states[at(2)].exit = Some(a(2))).expect("복원 문이 받는다"),
    ];
    assert_eq!(summary(&checked(2, &moved, &registry(2))), (1, ExpectedSource::Entry, 2, Differs, step(2, 2)));
    let unknown = SnapshotRegistry::restore(1, Vec::new()).expect("빈 등록부");
    assert_eq!(
        refused(2, &chains, &unknown),
        [
            ExitLineageProblem::StoredExitNotInRegistry { cycle: c(2), chain: ch(1), world: a(1) },
            ExitLineageProblem::ExpectedNotInRegistry { cycle: c(2), chain: ch(1), world: a(1), source: ExpectedSource::Entry },
        ]
    );
}

// ── 불일치 ─────────────────────────────────────────────────────────────────

#[test]
fn a_stored_exit_other_than_the_lineage_world_is_a_finished_mismatch() {
    // tree 의 C3 은 자식·되돌아온 갈래의 Entry 가 기대지 않는 Exit 이다. 그 Entry(A2 = 부모 C2 의 Exit)와 A1 으로 바꿔 둔다.
    for stored in [2, 1] {
        let chains = vec![restored(&tree()[0], |states| states[at(3)].exit = Some(a(stored))).expect("복원 문이 받는다")];
        assert_eq!(summary(&checked(3, &chains, &registry(3))), (3, ExpectedSource::Verify(step(3, 3)), stored, Differs, step(3, 5)));
    }
    // branching 의 C2 를 버려진 가지의 A4, 더 오래된 Verify 의 A2, Entry A1 로 바꿔 둔다 — 기대는 그대로 S10 의 A3 이다.
    for stored in [4, 2, 1] {
        let chains = vec![restored(&branching()[0], |states| states[at(2)].exit = Some(a(stored))).expect("복원 문이 받는다")];
        assert_eq!(summary(&checked(2, &chains, &registry(4))), (3, ExpectedSource::Verify(step(2, 10)), stored, Differs, step(2, 12)));
    }
}

// ── 검사 불능 ──────────────────────────────────────────────────────────────

#[test]
fn a_missing_stored_exit_gives_no_verdict_and_hides_no_lineage_damage() {
    let missing = vec![restored(&tree()[0], |states| states[at(3)].exit = None).expect("복원 문이 받는다")];
    assert_eq!(refused(3, &missing, &registry(3)), [ExitLineageProblem::StoredExitMissing { cycle: c(3), chain: ch(1) }]);
    let both = vec![
        restored(&tree()[0], |states| {
            states[at(3)].exit = None;
            states[at(3)].steps.nodes[2].snapshot = None;
        })
        .expect("복원 문이 받는다"),
    ];
    assert_eq!(
        refused(3, &both, &registry(3)),
        [
            ExitLineageProblem::StoredExitMissing { cycle: c(3), chain: ch(1) },
            ExitLineageProblem::LineageVerifyLacksSnapshot { cycle: c(3), chain: ch(1), step: step(3, 3) },
        ]
    );
}

#[test]
fn a_nearest_verify_without_its_snapshot_is_damage_not_a_detour() {
    // C2 의 S10 이 비면 S3 의 A2·Entry A1 로 넘어가지 않는다.
    let chains = vec![restored(&branching()[0], |states| states[at(2)].steps.nodes[9].snapshot = None).expect("복원 문이 받는다")];
    let lacking = ExitLineageProblem::LineageVerifyLacksSnapshot { cycle: c(2), chain: ch(1), step: step(2, 10) };
    assert_eq!(refused(2, &chains, &registry(4)), [lacking]);
    // C3 의 S3 가 비면 Entry 로 넘어가 저장 Exit(A3)과 「일치」하게 되는 우회도 하지 않는다.
    let chains = vec![restored(&branching()[0], |states| states[at(3)].steps.nodes[2].snapshot = None).expect("복원 문이 받는다")];
    assert_eq!(
        refused(3, &chains, &registry(4)),
        [ExitLineageProblem::LineageVerifyLacksSnapshot { cycle: c(3), chain: ch(1), step: step(3, 3) }]
    );
}

#[test]
fn registry_misses_are_each_named_and_give_no_verdict() {
    // 저장 Exit 과 기대 세계가 같은 이름 A3 이고 등록부에 없다 — 두 자리를 각각 말한다.
    assert_eq!(
        refused(3, &tree(), &registry(2)),
        [
            ExitLineageProblem::StoredExitNotInRegistry { cycle: c(3), chain: ch(1), world: a(3) },
            ExitLineageProblem::ExpectedNotInRegistry {
                cycle: c(3),
                chain: ch(1),
                world: a(3),
                source: ExpectedSource::Verify(step(3, 3)),
            },
        ]
    );
    // 저장 Exit 만 없다 — 이름이 달라도 불일치 판정을 내지 않는다.
    let far = vec![restored(&tree()[0], |states| states[at(3)].exit = Some(a(9))).expect("복원 문이 받는다")];
    assert_eq!(
        refused(3, &far, &registry(3)),
        [ExitLineageProblem::StoredExitNotInRegistry { cycle: c(3), chain: ch(1), world: a(9) }]
    );
    // 기대 세계만 없다.
    let older = vec![restored(&tree()[0], |states| states[at(3)].exit = Some(a(1))).expect("복원 문이 받는다")];
    assert_eq!(
        refused(3, &older, &registry(2)),
        [ExitLineageProblem::ExpectedNotInRegistry {
            cycle: c(3),
            chain: ch(1),
            world: a(3),
            source: ExpectedSource::Verify(step(3, 3)),
        }]
    );
}

#[test]
fn targets_outside_the_check_are_named_and_not_substituted() {
    let tree = tree();
    let known = registry(3);
    assert_eq!(refused(1, &tree, &known), [ExitLineageProblem::InterviewOutOfScope { cycle: c(1), chain: ch(1) }]);
    assert_eq!(refused(4, &tree, &known), [ExitLineageProblem::OpenExperimentOutOfScope { cycle: c(4), chain: ch(1) }]);
    assert_eq!(refused(9, &tree, &known), [ExitLineageProblem::Target(TargetProblem::NotFound { cycle: c(9) })]);

    let renamed = Chain::restore(ch(2), x1(), a(1), None, tree[0].cycles().clone());
    let two = vec![tree[0].clone(), renamed];
    assert_eq!(
        refused(3, &two, &known),
        [ExitLineageProblem::Target(TargetProblem::SeveralOwners { cycle: c(3), chains: vec![ch(1), ch(2)] })]
    );
    // 소유 체인의 이름 ch(1) 이 C3 을 갖지 않은 다른 항목에도 쓰였다 — 어느 기록의 C3 인지 정하지 않는다.
    let other = Chain::restore(ch(1), x1(), a(1), None, Cycles::start_at(rules(), x1(), a(1), 10));
    assert_eq!(
        refused(3, &[tree[0].clone(), other], &known),
        [ExitLineageProblem::Target(TargetProblem::OwnerChainRepeated { cycle: c(3), chain: ch(1), count: 2 })]
    );
}

#[test]
fn fixtures_the_existing_restore_refuses_are_recorded_not_forced() {
    // 닫은 자리를 Outcome 이 아닌 Verify 로 옮긴 닫힌 Experiment — 복원 문이 먼저 거절하므로 LastOutcomeUnreadable 은 건넨 Chain 에서 나지 않는다.
    let moved = restored(&tree()[0], |states| states[at(3)].steps.current = Some(NodeId::from_raw(3)));
    assert!(matches!(moved, Err(RestoreError::CycleClosedTooEarly)), "{moved:?}");
    // 되돌아온 갈래 C4 의 Entry 가 기대는 C2 의 Exit 은 바꿀 수 없다 — 복원 문이 먼저 거절한다.
    let parent = restored(&tree()[0], |states| states[at(2)].exit = Some(a(1)));
    assert!(matches!(parent, Err(RestoreError::CycleRevisitEntryIsNotTheTargetExit { .. })), "{parent:?}");
}

#[test]
fn inputs_are_left_unchanged_and_the_answer_is_repeatable() {
    let chains = branching();
    let known = registry(4);
    let before = format!("{chains:?}{known:?}");
    let first = check_experiment_exit_lineage(c(2), &chains, &known);
    let second = check_experiment_exit_lineage(c(2), &chains, &known);
    assert_eq!((format!("{chains:?}{known:?}"), &first), (before, &second));
}

#[test]
fn every_problem_reads_and_the_list_names_what_it_does_not_show() {
    let source = ExpectedSource::Verify(step(3, 3));
    let problems = [
        ExitLineageProblem::Target(TargetProblem::NotFound { cycle: c(9) }),
        ExitLineageProblem::InterviewOutOfScope { cycle: c(1), chain: ch(1) },
        ExitLineageProblem::OpenExperimentOutOfScope { cycle: c(4), chain: ch(1) },
        ExitLineageProblem::StoredExitMissing { cycle: c(3), chain: ch(1) },
        ExitLineageProblem::StoredExitNotInRegistry { cycle: c(3), chain: ch(1), world: a(9) },
        ExitLineageProblem::LastOutcomeUnreadable { cycle: c(3), chain: ch(1) },
        ExitLineageProblem::LineageVerifyLacksSnapshot { cycle: c(3), chain: ch(1), step: step(3, 3) },
        ExitLineageProblem::ExpectedNotInRegistry { cycle: c(3), chain: ch(1), world: a(3), source },
    ];
    let texts: Vec<String> = problems.iter().map(ToString::to_string).collect();
    for (text, needle) in texts.iter().zip(["cycle:C9", "범위 밖", "범위 밖", "검사하지 않았다", "등록부", "대신하지 않는다", "대신하지 않는다", "Verify step:C3/S3"]) {
        assert!(text.contains(needle), "{text}");
    }
    assert!(ExpectedSource::Entry.to_string().contains("Entry"));
    for needle in ["manifest", "P(T)", "B(T)", "Interview", "제품", "O-2"] {
        assert!(EXIT_LINEAGE_DOES_NOT_SHOW.iter().any(|line| line.contains(needle)), "{needle}");
    }
}
