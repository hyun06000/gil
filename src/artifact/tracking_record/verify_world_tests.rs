//! Experiment 안의 지정한 닫힌 Verify 가 확정한 세계와 그 확정 정책 유도(§6.1 의 「Experiment 의 Verify → 그 실험을 열 때의 적용 정책」)
//! 시험 — **합성 사건 입력**과 **기존 Cycles 전이로 만든 메모리 체인**, 그리고 이름을 밝힌 **복원 구조**를 쓴다.
//!
//! 정책 판·사건 칸·시도 결과·등록부는 시험이 만든 값이다. 승인은 시험 자료의 `approved: yes` 이며 모니터 승인이 아니다. `state.yaml`·
//! `gil start`·파일 시스템을 쓰지 않는다.
//!
//! 기대값은 구현에서 옮기지 않고 정본에서 도출했다 — §6.1 의 확정 정책 표(「Experiment 의 Verify·Exit → 그 실험을 열 때의 적용 정책
//! (§6.2 의 P)」), 「정책은 사이클 사이에서만 바뀌므로 한 실험의 생애에는 정책이 하나다」, Verify 의 `snapshot_ref` 는 한 번 쓰면 바꾸지
//! 않는다, §6.2 의 P(N), §5.1 의 판 이름 규칙, Artifact Model §7.2–§7.4(닫힌 Verify 는 구조 필드 `snapshot_ref` 를 지닌다, 바꾸지 않은
//! Verify 는 기존 세계를 다시 가리킨다, 같은 세계를 여러 Step 이 가리킬 수 있다).
//!
//! ```text
//! tree()        experiment_exit_tests 의 체인 그대로
//! branching()   exit_lineage_tests 의 체인 그대로 — C2 의 S6 Verify A4 는 Step 되돌아가기로 버려진 가지, S10 Verify A3 이 Exit 의 계보
//! live()        tree() 와 같은 C1–C3 위에서 열린 C4 를 더 걷는다 — 모두 기존 전이 함수
//!   chain:C1  cycle:C1 Interview (닫힘)            Entry A1 · Exit A1
//!             cycle:C2 Experiment (닫힘·success)   Entry A1 · S3 Verify A2 → Exit A2
//!             cycle:C3 Experiment (닫힘·failure)   Entry A2 · S3 Verify A3 → Exit A3
//!             cycle:C4 Experiment (열림)           parent C2 · revisit_from C3 · Entry A2
//!               S1 Define ─ S2 Hypothesis ─ S3 Verify A2(바꾸지 않은 세계) ─ S4 Analysis ─┬─ S5 Hypothesis ─ S6 Verify A8 ─ S7 Analysis ─ S8 Outcome failure (revisit → S4)
//!                                                                                         └─ S9 Hypothesis (revisit_from S8) ─ S10 Verify (열림)
//!               지금 서 있는 자리 S10 — 현재 세계는 그 계보의 가장 가까운 닫힌 Verify S3 의 A2 (S6 의 A8 이 아니다)
//! ```
//!
//! 체인은 Cycles 층의 메모리 전이다. 프로젝트 세션의 Verify 관측·보관·작업물 복원·저장·잠금을 검증하지 않는다. `restored` 는
//! `Cycles::restore` 의 그래프 불변식을 통과한 합성 값이다 — 닫힌 Verify 의 snapshot 을 지운 값, Verify 가 아닌 Step·열린 Verify 에
//! snapshot 을 적은 값은 현행 `Project` load 의 세계 검사 ④가 거절할 구조이며 Project load 는 실행하지 않았다. 어떤 시험도 실제 관측·
//! 승인·보관·관문 ②·제품 load 성공을 검증했다고 말하지 않는다.

use super::event_cycles_tests::{a, c, ch, close_interview, m, registry, rules, world, x1};
use super::exit_lineage_tests::{branching, outcome, verify, walk};
use super::experiment_exit_tests::{
    Records, applied, at, change, changed_after_opening, close_experiment, kept, no_records, p, records, records_with, restored,
    start, tree,
};
use super::*;
use crate::artifact::SnapshotRegistry;
use crate::cycles::Cycles;
use crate::refs::{SnapshotRef, StepRef};
use crate::walk::NodeId;
use crate::{Chain, Cycle, CycleKind, NodeKind};

use EventKind::{ScopeChange, Start};
use OpeningSelection::BeforeAdoptionInGivenHistory as Before;

// ── 체인 ───────────────────────────────────────────────────────────────────

/// 위 그림의 `live` — 모두 기존 전이 함수로 걸어 만든다.
fn live() -> Vec<Chain> {
    let mut cycles = Cycles::start_at(rules(), x1(), a(1), 1);
    close_interview(&mut cycles, "open_child");
    cycles.open_child(CycleKind::Experiment, x1()).expect("승인된 인터뷰 뒤 C2 를 연다");
    close_experiment(&mut cycles, 2, "success");
    cycles.open_child(CycleKind::Experiment, x1()).expect("성공한 C2 뒤 C3 을 연다");
    close_experiment(&mut cycles, 3, "failure");
    cycles.revisit().expect("적어 둔 대로 C2 로 되돌아간다");
    cycles.open_after_revisit(CycleKind::Experiment, x1()).expect("되돌아간 C2 에서 새 갈래를 연다");
    {
        let cycle = cycles.current_mut();
        walk(cycle, NodeKind::Define);
        walk(cycle, NodeKind::Hypothesis);
        verify(cycle, 2);
        let fork = walk(cycle, NodeKind::Analysis);
        walk(cycle, NodeKind::Hypothesis);
        verify(cycle, 8);
        walk(cycle, NodeKind::Analysis);
        outcome(cycle, "failure", Some(fork));
        cycle.revisit_step().expect("적어 둔 대로 S4 로 되돌아간다");
        walk(cycle, NodeKind::Hypothesis);
        cycle.open_step(NodeKind::Verify).expect("Verify 를 열어 둔다");
    }
    vec![Chain::restore(ch(1), x1(), a(1), None, cycles)]
}

fn step(cycle: u32, number: u32) -> StepRef {
    StepRef::new(c(cycle), number).expect("1 부터 센다")
}

fn cycle_of(chains: &[Chain], number: u32) -> &Cycle {
    chains[0].cycles().nodes().iter().find(|cycle| cycle.id().to_ref() == c(number)).expect("그 사이클이 있다")
}

#[test]
fn the_live_fixture_is_what_the_picture_says() {
    let chains = live();
    // C1–C3 은 tree() 와 같다.
    let first_three = |chains: &[Chain]| format!("{:?}", &chains[0].cycles().nodes()[..3]);
    assert_eq!(first_three(&chains), first_three(&tree()));
    let c4 = cycle_of(&chains, 4);
    assert_eq!((c4.kind(), c4.is_closed(), c4.entry_snapshot(), c4.exit_snapshot()), (CycleKind::Experiment, false, a(2), None));
    assert_eq!((c4.parent().map(|id| id.to_ref()), c4.revisit_from().map(|id| id.to_ref())), (Some(c(2)), Some(c(3))));
    use NodeKind::{Analysis, Define, Hypothesis, Outcome, Verify};
    let seen: Vec<(u32, NodeKind, Option<u32>, Option<u32>, bool, Option<u32>)> = c4
        .steps()
        .nodes()
        .iter()
        .map(|node| {
            (
                node.id.raw(),
                node.kind,
                node.parent.map(NodeId::raw),
                node.revisit_from.map(NodeId::raw),
                node.is_closed(),
                node.snapshot.map(SnapshotRef::number),
            )
        })
        .collect();
    assert_eq!(
        seen,
        [
            (1, Define, None, None, true, None),
            (2, Hypothesis, Some(1), None, true, None),
            (3, Verify, Some(2), None, true, Some(2)),
            (4, Analysis, Some(3), None, true, None),
            (5, Hypothesis, Some(4), None, true, None),
            (6, Verify, Some(5), None, true, Some(8)),
            (7, Analysis, Some(6), None, true, None),
            (8, Outcome, Some(7), None, true, None),
            (9, Hypothesis, Some(4), Some(8), true, None),
            (10, Verify, Some(9), None, false, None),
        ]
    );
    assert_eq!(c4.steps().current().map(NodeId::raw), Some(10));
    assert_eq!(c4.world_snapshot(), a(2), "지금 서 있는 자리의 현재 세계는 S3 의 A2 다");
}

// ── 유도 도움 ──────────────────────────────────────────────────────────────

fn select_with(
    target: StepRef,
    records: &Records,
    chains: &[Chain],
    registry: &SnapshotRegistry,
) -> Result<VerifyWorld, Vec<VerifyWorldProblem>> {
    select_verify_world(target, &records.ledger, &records.links, &records.facts, chains, registry)
}

fn derived(target: StepRef, records: &Records, chains: &[Chain]) -> VerifyWorld {
    select_with(target, records, chains, &registry(12)).unwrap_or_else(|problems| panic!("{target} 를 유도해야 한다: {problems:#?}"))
}

fn refused(target: StepRef, records: &Records, chains: &[Chain]) -> Vec<VerifyWorldProblem> {
    select_with(target, records, chains, &registry(12)).expect_err("거절해야 한다")
}

/// (세계 번호, 확정 정책).
fn world_and_policy(target: StepRef, records: &Records, chains: &[Chain]) -> (u32, OpeningSelection) {
    let found = derived(target, records, chains);
    (found.world().number(), found.policy())
}

/// 같은 입력의 P(N) — 비교용. 제품 코드가 아니라 시험이 따로 부른다.
fn opening_of(cycle: u32, records: &Records, chains: &[Chain]) -> OpeningPolicy {
    select_opening_policy(c(cycle), &records.ledger, &records.links, &records.facts, chains, &registry(12)).expect("P(N) 을 고른다")
}

fn in_step(problem: StepProblem) -> Vec<VerifyWorldProblem> {
    vec![VerifyWorldProblem::Step(problem)]
}

// ── 열린·닫힌 Experiment 안의 닫힌 Verify ──────────────────────────────────

#[test]
fn a_closed_verify_gives_its_own_world_in_a_closed_or_an_open_experiment() {
    // 사건이 없으면 정책은 건넨 이력 안의 정책 채택 전이다. 세계는 각 Verify 자신의 snapshot_ref 다 — 판정(success·failure)으로, 소유
    // Experiment 의 열림·닫힘으로 거르지 않는다.
    let chains = live();
    assert_eq!(world_and_policy(step(2, 3), &no_records(), &chains), (2, Before));
    assert_eq!(world_and_policy(step(3, 3), &no_records(), &chains), (3, Before));
    assert_eq!(world_and_policy(step(4, 3), &no_records(), &chains), (2, Before), "열린 C4 안의 닫힌 Verify");
    assert_eq!(world_and_policy(step(4, 6), &no_records(), &chains), (8, Before), "열린 C4 안의 버려진 가지의 닫힌 Verify");
    let found = derived(step(4, 6), &no_records(), &chains);
    assert_eq!((found.step(), found.cycle(), found.chain(), found.world()), (step(4, 6), c(4), ch(1), a(8)));
    assert!(found.inputs().events().is_empty());
}

// ── 확정 정책은 소유 Experiment 를 열 때의 P(N), 나중 변경에 흔들리지 않는다 ────

#[test]
fn the_policy_is_the_opening_policy_of_the_owning_experiment_and_survives_later_changes() {
    // changed_after_opening: M1 start(P1, 발급기 1) · M2 C2 경계(P2, 3) · M3 C3 경계(P3, 4) · M4 C2 경계 restore_result(P3, 5).
    let chains = live();
    let rows = changed_after_opening();
    let full = records(&rows);
    // C2 는 M1 뒤에 열렸다 — P1. M2·M3·M4 는 C2 가 열린 뒤에 저장됐다.
    assert_eq!(world_and_policy(step(2, 3), &full, &chains), (2, applied(1, Start, 1, 1)));
    // C3 은 M2 뒤에 열렸다 — P2. C3 을 닫은 경계의 M3(P3)·원장의 마지막 판 P3 이 아니다.
    assert_eq!(world_and_policy(step(3, 3), &full, &chains), (3, applied(2, ScopeChange, 2, 3)));
    // C4 는 M3 뒤에 열렸다 — P3, 사건 M3. 그 뒤에 저장된 M4(restore_result, 발급기 5)는 C4 의 정책 출처가 아니다.
    for number in [3, 6] {
        assert_eq!(world_and_policy(step(4, number), &full, &chains).1, applied(3, ScopeChange, 3, 4));
    }
    assert_eq!(full.ledger.policies().last().map(PolicyRecord::name), Some(p(3)), "원장의 마지막 판은 P3 이다");
    // 사건을 덧붙이기 전의 기록에서도 같다 — 나중 사건은 과거 Verify 의 세계·정책을 고치지 않는다.
    let earlier = records_with(&["rule-1/\n"], &rows[..1]);
    assert_eq!(derived(step(2, 3), &earlier, &chains).policy(), derived(step(2, 3), &full, &chains).policy());
    let before_m4 = records_with(&["rule-1/\n", "rule-2/\n", "rule-3/\n"], &rows[..3]);
    assert_eq!(derived(step(4, 6), &before_m4, &chains).policy(), derived(step(4, 6), &full, &chains).policy());
}

#[test]
fn the_result_carries_the_same_call_opening_policy_unchanged() {
    let chains = live();
    let full = records(&changed_after_opening());
    for (cycle, number) in [(2, 3), (3, 3), (4, 3), (4, 6)] {
        let found = derived(step(cycle, number), &full, &chains);
        let opening = opening_of(cycle, &full, &chains);
        assert_eq!(found.opening(), &opening);
        assert_eq!(found.policy(), opening.selection());
        assert_eq!((found.cycle(), found.chain()), (opening.target(), opening.chain()));
        assert_eq!(found.inputs(), opening.inputs());
    }
}

#[test]
fn a_closed_experiment_and_its_verify_carry_one_policy_and_existing_selections_are_unchanged() {
    // 한 실험의 생애에는 정책이 하나다 — 닫힌 C2·C3 의 Exit 유도와 그 Verify 의 정책이 같다. 기존 두 선택의 결과는 이 유도와 무관하게 그대로다.
    let chains = tree();
    let full = records(&changed_after_opening());
    for (cycle, world) in [(2, 2), (3, 3)] {
        let exit = select_experiment_exit(c(cycle), &full.ledger, &full.links, &full.facts, &chains, &registry(12)).expect("Exit 을 유도한다");
        let found = derived(step(cycle, 3), &full, &chains);
        assert_eq!((found.world(), found.policy()), (a(world), exit.policy()));
        assert_eq!(found.opening(), exit.opening());
    }
}

// ── 같은 세계를 가리키는 여러 Verify, 같은 원문의 다른 판 ────────────────────

#[test]
fn verifies_pointing_at_the_same_world_stay_separate_and_same_text_boards_are_not_merged() {
    // P1·P3 은 같은 원문이다. C2 의 S3 와 C4 의 S3(바꾸지 않은 Verify — C4 의 Entry A2 를 다시 가리킨다)는 같은 세계 A2 를 가리킨다.
    let chains = live();
    let same = records_with(&["same/\n", "other/\n", "same/\n"], &changed_after_opening()[..3]);
    assert_eq!(same.ledger.policies()[0].rules(), same.ledger.policies()[2].rules(), "P1 과 P3 은 같은 원문이다");
    let older = derived(step(2, 3), &same, &chains);
    let unchanged = derived(step(4, 3), &same, &chains);
    assert_eq!((older.world(), unchanged.world()), (a(2), a(2)));
    assert_eq!(unchanged.world(), cycle_of(&chains, 4).entry_snapshot(), "바꾸지 않은 Verify 는 기존 세계를 다시 가리킨다");
    assert_eq!((older.step(), older.cycle(), older.policy()), (step(2, 3), c(2), applied(1, Start, 1, 1)));
    assert_eq!((unchanged.step(), unchanged.cycle(), unchanged.policy()), (step(4, 3), c(4), applied(3, ScopeChange, 3, 4)));
    assert_ne!(older, unchanged, "같은 세계를 가리켜도 서로 다른 Verify 의 결과다");
    // branching: C2 의 S10 과 C3 의 S3(바꾸지 않은 Verify)도 같은 세계 A3 을 가리키며 각자의 출처로 남는다.
    let chains = branching();
    let (first, second) = (derived(step(2, 10), &no_records(), &chains), derived(step(3, 3), &no_records(), &chains));
    assert_eq!((first.world(), second.world()), (a(3), a(3)));
    assert_eq!((first.step(), second.step()), (step(2, 10), step(3, 3)));
    assert_eq!((first.cycle(), second.cycle()), (c(2), c(3)));
}

// ── 역사적 조회와 현재 세계·Exit 선택의 구분 ───────────────────────────────

#[test]
fn a_verify_off_the_current_lineage_is_looked_up_but_not_taken_as_the_current_world_or_the_exit() {
    // 열린 C4: 버려진 가지의 S6 는 자신의 A8 을 돌려준다. 지금 서 있는 자리의 현재 세계는 여전히 S3 의 A2 이고 C4 에는 Exit 이 없다.
    let chains = live();
    let off = derived(step(4, 6), &no_records(), &chains);
    assert_eq!(off.world(), a(8));
    let c4 = cycle_of(&chains, 4);
    assert_eq!((c4.world_snapshot(), c4.exit_snapshot()), (a(2), None));
    // 닫힌 C2(branching): 버려진 가지의 S6 는 A4, 더 오래된 S3 는 A2 를 그대로 돌려준다. Exit 과 계보 기대 세계는 S10 의 A3 그대로다.
    let chains = branching();
    assert_eq!(derived(step(2, 6), &no_records(), &chains).world(), a(4));
    assert_eq!(derived(step(2, 3), &no_records(), &chains).world(), a(2));
    let lineage = check_experiment_exit_lineage(c(2), &chains, &registry(12)).expect("계보를 검사한다");
    assert_eq!((lineage.expected(), lineage.source(), lineage.stored()), (a(3), ExpectedSource::Verify(step(2, 10)), a(3)));
    let exit = select_experiment_exit(c(2), &PolicyLedger::empty(), &EventPolicyLinks::empty(), &[], &chains, &registry(12))
        .expect("Exit 을 유도한다");
    assert_eq!(exit.world(), a(3));
}

// ── 건넨 이력 안의 정책 채택 전과 오류의 구분 ──────────────────────────────

#[test]
fn before_adoption_in_the_given_history_is_kept_apart_from_errors_and_from_not_derived() {
    // 기존 프로젝트의 첫 채택(없음 → P1)을 C2 경계에서 저장했다(발급기 3). C2 를 열 때는 사건이 없었다.
    let chains = live();
    let first = records(&[change(1, 2, 3, kept(5, 2, 3))]);
    assert_eq!(world_and_policy(step(2, 3), &first, &chains), (2, Before));
    assert_eq!(world_and_policy(step(3, 3), &first, &chains), (3, applied(1, ScopeChange, 1, 3)));
    assert_eq!(world_and_policy(step(4, 3), &first, &chains), (2, applied(1, ScopeChange, 1, 3)));
    // 사건이 없을 때 B(C2) 의 확정 정책은 미유도다 — 이 유도의 정책 채택 전과 다른 값이고 이 유도가 바꾸지 않는다.
    let none = no_records();
    let boundary = select_boundary_world(c(2), &none.ledger, &none.links, &none.facts, &chains, &registry(12)).expect("B(C2) 를 고른다");
    assert_eq!(boundary.policy(), BoundaryPolicy::ExitPolicyNotDerived);
    assert_eq!(derived(step(2, 3), &none, &chains).policy(), Before);
    // 범위 밖·오류는 Err 이며 정책 채택 전 성공이 아니다.
    for target in [step(1, 1), step(4, 1), step(4, 10), step(4, 99), step(9, 1)] {
        assert!(select_with(target, &none, &chains, &registry(12)).is_err(), "{target}");
    }
}

// ── 대상: Cycle ────────────────────────────────────────────────────────────

#[test]
fn an_interview_step_is_out_of_scope_whether_or_not_the_step_exists() {
    let out = |number| [VerifyWorldProblem::InterviewOutOfScope { step: step(1, number), chain: ch(1) }];
    let chains = live();
    // P(C1) 은 고를 수 있지만 그것을 Interview 의 Step 의 확정 정책·성공으로 쓰지 않는다.
    assert!(select_opening_policy(c(1), &PolicyLedger::empty(), &EventPolicyLinks::empty(), &[], &chains, &registry(12)).is_ok());
    assert_eq!(refused(step(1, 1), &no_records(), &chains), out(1));
    assert_eq!(refused(step(1, 1), &records(&[start()]), &chains), out(1));
    assert_eq!(refused(step(1, 99), &no_records(), &chains), out(99), "없는 Step 이어도 Interview 는 범위 밖이다");
    // 다른 체인의 닫힌 Interview 도 같다.
    assert_eq!(
        refused(step(5, 1), &no_records(), &world()),
        [VerifyWorldProblem::InterviewOutOfScope { step: step(5, 1), chain: ch(2) }]
    );
}

#[test]
fn a_missing_cycle_is_reported_once() {
    // P(N) 과 이 선택의 조회가 같은 소유 규칙으로 같은 사실을 찾는다 — 한 번만 담는다.
    let not_found = |cycle| [VerifyWorldProblem::Target(TargetProblem::NotFound { cycle: c(cycle) })];
    assert_eq!(refused(step(9, 1), &no_records(), &live()), not_found(9));
    assert_eq!(refused(step(9, 1), &records(&[start()]), &live()), not_found(9));
    assert_eq!(refused(step(2, 3), &no_records(), &[]), not_found(2));
}

fn ambiguous_lookup(problem: &VerifyWorldProblem) -> bool {
    matches!(problem, VerifyWorldProblem::Inputs(found)
        if matches!(found.as_slice(), [EventInputsProblem::CycleRefs(refs)]
            if refs.iter().any(|problem| matches!(problem, CycleRefProblem::AmbiguousLookup(_)))))
}

#[test]
fn an_ambiguous_cycle_is_refused_and_3j_is_reported_too() {
    // 같은 체인 항목이 두 번 — 여러 소유자.
    let mut chains = live();
    chains.push(chains[0].clone());
    let problems = refused(step(2, 3), &no_records(), &chains);
    assert_eq!(problems.len(), 2, "{problems:#?}");
    assert!(ambiguous_lookup(&problems[0]), "{problems:#?}");
    assert_eq!(problems[1], VerifyWorldProblem::Target(TargetProblem::SeveralOwners { cycle: c(2), chains: vec![ch(1), ch(1)] }));
    // chain:C1 이라는 이름의 다른 항목이 C7 을 지닌다 — 소유 항목은 하나지만 그 이름의 기록이 둘이다.
    let mut chains = live();
    chains.push(Chain::restore(ch(1), x1(), a(1), None, Cycles::start_at(rules(), x1(), a(1), 7)));
    let problems = refused(step(2, 3), &no_records(), &chains);
    assert_eq!(problems.len(), 2, "{problems:#?}");
    assert!(ambiguous_lookup(&problems[0]), "{problems:#?}");
    assert_eq!(problems[1], VerifyWorldProblem::Target(TargetProblem::OwnerChainRepeated { cycle: c(2), chain: ch(1), count: 2 }));
}

// ── 대상: Step ─────────────────────────────────────────────────────────────

#[test]
fn a_missing_step_is_not_replaced() {
    let chains = live();
    for (cycle, number) in [(4, 99), (4, 11), (2, 6)] {
        assert_eq!(
            refused(step(cycle, number), &no_records(), &chains),
            in_step(StepProblem::NotFound { step: step(cycle, number), chain: ch(1) })
        );
    }
}

#[test]
fn a_step_that_is_not_a_verify_is_out_of_scope() {
    let chains = live();
    for (cycle, number, kind) in [
        (4, 1, NodeKind::Define),
        (4, 4, NodeKind::Analysis),
        (4, 9, NodeKind::Hypothesis),
        (2, 5, NodeKind::Outcome),
        (3, 4, NodeKind::Analysis),
    ] {
        assert_eq!(
            refused(step(cycle, number), &no_records(), &chains),
            in_step(StepProblem::NotVerifyOutOfScope { step: step(cycle, number), chain: ch(1), kind })
        );
    }
}

#[test]
fn an_open_verify_is_out_of_scope() {
    assert_eq!(
        refused(step(4, 10), &no_records(), &live()),
        in_step(StepProblem::OpenVerifyOutOfScope { step: step(4, 10), chain: ch(1) })
    );
}

#[test]
fn a_closed_verify_without_its_snapshot_is_refused_without_a_substitute() {
    // 복원 구조: 닫힌 C3 의 S3 Verify 에서 snapshot 을 지웠다. Cycles::restore 는 받고, 현행 Project load 의 ④는 거절할 값이다.
    let chains = vec![restored(&tree()[0], |states| states[at(3)].steps.nodes[2].snapshot = None).expect("복원 문이 받는다")];
    assert_eq!(
        refused(step(3, 3), &no_records(), &chains),
        in_step(StepProblem::ClosedVerifyLacksSnapshot { step: step(3, 3), chain: ch(1) })
    );
    // C3 의 Exit A3·Entry A2 는 그대로 있다 — 그것으로 Verify 의 세계를 대신하지 않는다.
    let c3 = cycle_of(&chains, 3);
    assert_eq!((c3.exit_snapshot(), c3.entry_snapshot()), (Some(a(3)), a(2)));
    // 같은 구조의 C2 의 Verify 는 그대로 유도한다.
    assert_eq!(world_and_policy(step(2, 3), &no_records(), &chains), (2, Before));
}

#[test]
fn a_snapshot_written_on_a_step_out_of_scope_is_neither_looked_up_nor_returned() {
    // 복원 구조: Verify 가 아닌 C4 의 S4(Analysis)에 A9, 열린 C4 의 S10 Verify 에 A10 을 적었다. Cycles::restore 는 받고, 현행 Project load
    // 의 ④는 거절할 값이다. 범위 밖 결과이며 그 이름을 등록부에서 조회하지 않는다 — A1 만 아는 등록부로도 같은 범위 밖이다.
    let chains = vec![
        restored(&live()[0], |states| {
            states[at(4)].steps.nodes[3].snapshot = Some(a(9));
            states[at(4)].steps.nodes[9].snapshot = Some(a(10));
        })
        .expect("복원 문이 받는다"),
    ];
    let c4 = cycle_of(&chains, 4);
    assert_eq!((c4.steps().nodes()[3].snapshot, c4.steps().nodes()[9].snapshot), (Some(a(9)), Some(a(10))));
    for known in [registry(12), registry(1)] {
        assert_eq!(
            select_with(step(4, 4), &no_records(), &chains, &known).expect_err("범위 밖이다"),
            in_step(StepProblem::NotVerifyOutOfScope { step: step(4, 4), chain: ch(1), kind: NodeKind::Analysis })
        );
        assert_eq!(
            select_with(step(4, 10), &no_records(), &chains, &known).expect_err("범위 밖이다"),
            in_step(StepProblem::OpenVerifyOutOfScope { step: step(4, 10), chain: ch(1) })
        );
    }
}

#[test]
fn a_world_missing_from_the_registry_is_refused() {
    let chains = live();
    // 등록부는 A1 만 안다. C2 의 S3 의 A2 가 조회되지 않는다.
    assert_eq!(
        select_with(step(2, 3), &no_records(), &chains, &registry(1)).expect_err("거절해야 한다"),
        [VerifyWorldProblem::SnapshotNotInRegistry { step: step(2, 3), chain: ch(1), world: a(2) }]
    );
    // A1–A7 을 아는 등록부 — C4 의 S3(A2)는 유도하고 S6(A8)은 거절한다.
    assert_eq!(select_with(step(4, 3), &no_records(), &chains, &registry(7)).expect("유도한다").world(), a(2));
    assert_eq!(
        select_with(step(4, 6), &no_records(), &chains, &registry(7)).expect_err("거절해야 한다"),
        [VerifyWorldProblem::SnapshotNotInRegistry { step: step(4, 6), chain: ch(1), world: a(8) }]
    );
}

// ── 입력 오류와 대상 오류의 보존 ───────────────────────────────────────────

fn only_inputs(problems: &[VerifyWorldProblem]) -> &[EventInputsProblem] {
    match problems {
        [VerifyWorldProblem::Inputs(found)] => found,
        other => panic!("입력 갈래 하나만 와야 한다: {other:#?}"),
    }
}

#[test]
fn input_problems_are_kept_verbatim_and_with_every_target_problem() {
    let chains = live();
    let mut cut = records(&[start(), change(2, 2, 3, kept(5, 2, 3))]);
    cut.facts.truncate(1);
    // 대상이 정상이면 입력 갈래 하나뿐이며 P(N) 의 입력 실패 그대로다.
    let problems = refused(step(2, 3), &cut, &chains);
    assert!(matches!(only_inputs(&problems), [EventInputsProblem::Correspondence(_)]), "{problems:#?}");
    let from_opening = select_opening_policy(c(2), &cut.ledger, &cut.links, &cut.facts, &chains, &registry(12)).expect_err("P(N) 도 거절한다");
    assert_eq!(from_opening, [OpeningPolicyProblem::Inputs(only_inputs(&problems).to_vec())]);
    // 대상 문제와 함께 오면 둘 다 순서대로 온다. 대상 문제는 한 번이다.
    let second = |target, chains: &[Chain]| {
        let problems = refused(target, &cut, chains);
        assert_eq!(problems.len(), 2, "{problems:#?}");
        assert!(matches!(&problems[0], VerifyWorldProblem::Inputs(_)), "{problems:#?}");
        problems[1].clone()
    };
    assert_eq!(second(step(1, 1), &chains), VerifyWorldProblem::InterviewOutOfScope { step: step(1, 1), chain: ch(1) });
    assert_eq!(second(step(9, 1), &chains), VerifyWorldProblem::Target(TargetProblem::NotFound { cycle: c(9) }));
    assert_eq!(second(step(4, 99), &chains), VerifyWorldProblem::Step(StepProblem::NotFound { step: step(4, 99), chain: ch(1) }));
    assert_eq!(
        second(step(4, 1), &chains),
        VerifyWorldProblem::Step(StepProblem::NotVerifyOutOfScope { step: step(4, 1), chain: ch(1), kind: NodeKind::Define })
    );
    assert_eq!(second(step(4, 10), &chains), VerifyWorldProblem::Step(StepProblem::OpenVerifyOutOfScope { step: step(4, 10), chain: ch(1) }));
    let lacking = vec![restored(&tree()[0], |states| states[at(3)].steps.nodes[2].snapshot = None).expect("복원 문이 받는다")];
    assert_eq!(
        second(step(3, 3), &lacking),
        VerifyWorldProblem::Step(StepProblem::ClosedVerifyLacksSnapshot { step: step(3, 3), chain: ch(1) })
    );
    // 대상 세계의 등록부 누락도 입력 검사와 무관하게 평가해 함께 돌려준다. 등록부는 A1 만 알고 cut 의 기준점은 A1 뿐이므로 입력 실패는
    // 대응(Correspondence) 하나 그대로다.
    let problems = select_with(step(2, 3), &cut, &chains, &registry(1)).expect_err("거절해야 한다");
    let from_opening = select_opening_policy(c(2), &cut.ledger, &cut.links, &cut.facts, &chains, &registry(1)).expect_err("P(N) 도 거절한다");
    let [OpeningPolicyProblem::Inputs(inputs)] = from_opening.as_slice() else { panic!("P(N) 은 입력 갈래 하나: {from_opening:#?}") };
    assert!(matches!(inputs.as_slice(), [EventInputsProblem::Correspondence(_)]), "{inputs:#?}");
    assert_eq!(
        problems,
        [
            VerifyWorldProblem::Inputs(inputs.clone()),
            VerifyWorldProblem::SnapshotNotInRegistry { step: step(2, 3), chain: ch(1), world: a(2) },
        ]
    );
}

#[test]
fn a_baseline_and_the_verify_world_both_missing_from_the_registry_are_both_kept() {
    // 등록부는 A1 만 안다. M2 의 기준점 A5 누락은 P(N) 안의 3j(3e)가, C3 의 S3 의 A3 누락은 이 선택의 대상 조회가 찾는다.
    let chains = live();
    let two = records(&[start(), change(2, 2, 3, kept(5, 2, 3))]);
    let problems = select_with(step(3, 3), &two, &chains, &registry(1)).expect_err("거절해야 한다");
    let from_opening =
        select_opening_policy(c(3), &two.ledger, &two.links, &two.facts, &chains, &registry(1)).expect_err("P(N) 도 거절한다");
    let [OpeningPolicyProblem::Inputs(inputs)] = from_opening.as_slice() else { panic!("P(N) 은 입력 갈래 하나: {from_opening:#?}") };
    match inputs.as_slice() {
        [EventInputsProblem::CycleRefs(found)] => {
            assert!(found.iter().any(|problem| matches!(problem, CycleRefProblem::Point(_))), "{found:#?}")
        }
        other => panic!("{other:#?}"),
    }
    assert_eq!(
        problems,
        [
            VerifyWorldProblem::Inputs(inputs.clone()),
            VerifyWorldProblem::SnapshotNotInRegistry { step: step(3, 3), chain: ch(1), world: a(3) },
        ]
    );
}

// ── 3j 의 판단하지 않음과 잘린 이력의 한계 ─────────────────────────────────

#[test]
fn the_unjudged_parts_of_3j_stay_visible() {
    let chains = live();
    let two = records(&[start(), change(2, 2, 3, kept(5, 2, 3))]);
    let agreed = check_event_inputs(&two.ledger, &two.links, &two.facts, &chains, &registry(12)).expect("3j 가 받아들인다");
    let found = derived(step(4, 6), &two, &chains);
    assert_eq!(found.inputs(), &agreed);
    assert_eq!(
        found.inputs().cycle_refs().not_judged(),
        [(m(1), CycleField::AfterCycleRef, RefReading::NullNotJudged), (m(1), CycleField::BaselineAfterCycleRef, RefReading::NullNotJudged)]
    );
    assert_eq!(found.inputs().cycle_refs().chains_not_looked_up(), [m(1)]);
}

#[test]
fn inputs_cut_together_are_not_detected_and_are_chosen_within() {
    // 원장·정책 연결·사이클 입력을 함께 잘라낸 입력은 3j 가 알아내지 못한다 — 잘린 이력 안에서 P(N) 을 고르며 완전하다고 주장하지 않는다.
    let chains = live();
    let rows = [start(), change(2, 1, 2, kept(5, 1, 2))];
    assert_eq!(world_and_policy(step(2, 3), &records(&rows), &chains), (2, applied(2, ScopeChange, 2, 2)));
    assert_eq!(world_and_policy(step(2, 3), &records(&rows[..1]), &chains), (2, applied(1, Start, 1, 1)));
    assert_eq!(world_and_policy(step(2, 3), &no_records(), &chains), (2, Before), "잘린 이력 안의 정책 채택 전이다");
}

// ── 순수성과 한계 ──────────────────────────────────────────────────────────

#[test]
fn inputs_are_left_unchanged_and_the_answer_is_repeatable() {
    let chains = live();
    let full = records(&changed_after_opening());
    let known = registry(12);
    let snapshot = || format!("{:?}{:?}{:?}{:?}{:?}", full.ledger, full.links, full.facts, chains, known);
    let before = snapshot();
    for target in [step(4, 6), step(4, 10)] {
        let first = select_with(target, &full, &chains, &known);
        let second = select_with(target, &full, &chains, &known);
        assert_eq!(first, second, "{target}");
    }
    assert_eq!(snapshot(), before, "입력을 바꿨다");
    let found = select_with(step(4, 6), &full, &chains, &known).expect("유도한다");
    assert_eq!((found.world(), found.policy()), (a(8), applied(3, ScopeChange, 3, 4)));
    // 지금 서 있는 자리도 그대로다.
    assert_eq!(cycle_of(&chains, 4).steps().current().map(NodeId::raw), Some(10));
}

#[test]
fn every_problem_reads_and_the_list_names_what_it_does_not_show() {
    let problems = [
        VerifyWorldProblem::Inputs(Vec::new()),
        VerifyWorldProblem::Target(TargetProblem::NotFound { cycle: c(9) }),
        VerifyWorldProblem::Target(TargetProblem::SeveralOwners { cycle: c(2), chains: vec![ch(1), ch(1)] }),
        VerifyWorldProblem::Target(TargetProblem::OwnerChainRepeated { cycle: c(2), chain: ch(1), count: 2 }),
        VerifyWorldProblem::InterviewOutOfScope { step: step(1, 1), chain: ch(1) },
        VerifyWorldProblem::Step(StepProblem::NotFound { step: step(4, 99), chain: ch(1) }),
        VerifyWorldProblem::Step(StepProblem::NotVerifyOutOfScope { step: step(4, 1), chain: ch(1), kind: NodeKind::Define }),
        VerifyWorldProblem::Step(StepProblem::OpenVerifyOutOfScope { step: step(4, 10), chain: ch(1) }),
        VerifyWorldProblem::Step(StepProblem::ClosedVerifyLacksSnapshot { step: step(3, 3), chain: ch(1) }),
        VerifyWorldProblem::SnapshotNotInRegistry { step: step(2, 3), chain: ch(1), world: a(2) },
    ];
    assert!(problems.iter().all(|problem| !problem.to_string().is_empty()));
    for problem in &problems[4..] {
        assert!(problem.to_string().contains("step:C"), "대상 Step 을 말해야 한다: {problem}");
    }
    for needle in [
        "함께 잘라낸",
        "not_judged",
        "chains_not_looked_up",
        "manifest",
        "검증 ①",
        "②",
        "T9",
        "world_snapshot",
        "Cycle Exit",
        "B(T)",
        "ExitPolicyNotDerived",
        "Interview",
        "④",
        "T8",
        "LedgerStart",
        "O-2",
        "O-3",
        "승인 이후 변경",
    ] {
        assert!(VERIFY_WORLD_DOES_NOT_SHOW.iter().any(|line| line.contains(needle)), "{needle}");
    }
}
