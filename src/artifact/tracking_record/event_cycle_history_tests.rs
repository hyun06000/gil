//! 관리 사건의 승인 당시 세 칸(`chain_ref`·`after_cycle_ref`·`next_cycle_id`)의 두 시점 보존 검사(「3i」) 시험 — **합성 입력**과
//! **실제 전이로 만든 체인**을 쓴다.
//!
//! 사건 칸·시도 결과·등록부는 시험이 만든 값이다. 체인은 3h 시험과 같은 도움 함수(기존 전이 함수로 걸어 만든다)로 만든다. 승인은
//! 시험 자료의 `approved: yes` 이며 모니터 승인이 아니다. `state.yaml`·`gil start`·파일 시스템을 쓰지 않는다.
//!
//! 두 관측 시점의 세계:
//!
//! ```text
//! 앞 시점  chain:C1 (닫힘) C1 ─▶ C2 ─▶ C3     모두 닫힘
//!          chain:C2 (열림) C4 ─▶ C5           C4 닫힘, C5(대기 중 인터뷰) 열림
//! 뒤 시점  chain:C1 그대로
//!          chain:C2 (열림) C4 ─▶ C5 ─▶ C6     C4·C5 닫힘, C6 열림
//! ```
//!
//! 기본 사건(앞 시점): M1 `start`(chain:C1, 경계 `null`, 발급 1, 보관 A1), M2(chain:C1, C2 에서 승인·보관, 발급 3, A2),
//! M3(chain:C2, C4 에서 승인, 발급 5, 보관 실패 하나로 대기).

use super::event_cycles_tests::{a, c, ch, close_interview, m, open_interview, registry, rules, world, x1};
use super::*;
use crate::Chain;
use crate::artifact::SnapshotRegistry;
use crate::cycles::Cycles;
use crate::refs::CycleRef;

use AttemptSeen::{Archived as A, NotArchived as F};
use BaselineSlot::{Unwritten, Written};
use EventKind::{RestoreResult, ScopeChange, Start};

// ── 세계 ───────────────────────────────────────────────────────────────────

/// 앞 시점의 세계 — 뒤 시점(`world()`)의 `chain:C2` 에서 C5 가 아직 열려 있다.
fn earlier_world() -> Vec<Chain> {
    let first = world().remove(0);
    let entry = first.exit_snapshot().expect("닫힌 체인의 Exit");
    let mut second = Cycles::start_at(rules(), x1(), entry, first.cycles().next_name());
    close_interview(&mut second, "open_child");
    open_interview(&mut second);
    let second = Chain::restore(ch(2), x1(), entry, Some(ch(1)), second);
    vec![first, second]
}

#[test]
fn the_two_worlds_are_what_the_fixture_says() {
    let shape = |chains: &[Chain]| -> Vec<(u32, u32, bool)> {
        chains
            .iter()
            .flat_map(|chain| {
                chain.cycles().nodes().iter().map(move |cycle| (chain.id().number(), cycle.id().to_ref().number(), cycle.is_closed()))
            })
            .collect()
    };
    assert_eq!(shape(&earlier_world()), [(1, 1, true), (1, 2, true), (1, 3, true), (2, 4, true), (2, 5, false)]);
    assert_eq!(shape(&world()), [(1, 1, true), (1, 2, true), (1, 3, true), (2, 4, true), (2, 5, true), (2, 6, false)]);
}

// ── 사건: 합성 입력 ─────────────────────────────────────────────────────────

/// 사건의 `baseline` 쪽.
#[derive(Clone, Copy)]
enum Base {
    /// 기준점 없음, 경계·발급기 칸 없음, 실패 시도 `failures` 개.
    Waiting { failures: usize },
    /// 실패 시도 `failures` 개 뒤 `archived` 하나, 기준점·경계(`None` = `null`)·발급기를 함께 적음.
    Fixed { failures: usize, snapshot: u32, boundary: Option<u32>, issuer: u32 },
}

fn event(number: u32, kind: EventKind, chain: u32, after: BaselineSlot<Option<CycleRef>>, issuer: u32, base: Base) -> EventCycleFacts {
    let baseline = match base {
        Base::Waiting { failures } => BaselineFacts::new(m(number), kind, None, Unwritten, Unwritten, vec![F; failures]),
        Base::Fixed { failures, snapshot, boundary, issuer } => {
            let mut attempts = vec![F; failures];
            attempts.push(A);
            BaselineFacts::new(m(number), kind, Some(a(snapshot)), Written(boundary.map(c)), Written(issuer), attempts)
        }
    };
    EventCycleFacts::new(baseline, ch(chain), after, issuer)
}

fn at(cycle: u32) -> BaselineSlot<Option<CycleRef>> {
    Written(Some(c(cycle)))
}

fn start() -> EventCycleFacts {
    event(1, Start, 1, Written(None), 1, Base::Fixed { failures: 0, snapshot: 1, boundary: None, issuer: 1 })
}

fn m2() -> EventCycleFacts {
    event(2, ScopeChange, 1, at(2), 3, Base::Fixed { failures: 0, snapshot: 2, boundary: Some(2), issuer: 3 })
}

/// C4 에서 승인한 M3 — `baseline` 만 바꿔 쓴다.
fn m3(base: Base) -> EventCycleFacts {
    event(3, ScopeChange, 2, at(4), 5, base)
}

fn before_events() -> Vec<EventCycleFacts> {
    vec![start(), m2(), m3(Base::Waiting { failures: 1 })]
}

type Point<'a> = (&'a [EventCycleFacts], &'a [Chain], &'a SnapshotRegistry);

fn check(before: &[EventCycleFacts], after: &[EventCycleFacts]) -> Result<EventCycleHistoryKept, Vec<EventCycleHistoryProblem>> {
    let (earlier, later, registry) = (earlier_world(), world(), registry(9));
    let before: Point<'_> = (before, &earlier, &registry);
    let after: Point<'_> = (after, &later, &registry);
    check_event_cycle_history(before, after)
}

fn kept(before: &[EventCycleFacts], after: &[EventCycleFacts]) -> EventCycleHistoryKept {
    check(before, after).unwrap_or_else(|problems| panic!("받아들여야 한다: {problems:#?}"))
}

fn refused(before: &[EventCycleFacts], after: &[EventCycleFacts]) -> Vec<EventCycleHistoryProblem> {
    check(before, after).expect_err("거절해야 한다")
}

/// 두 시점이 각각 3h 를 통과한다 — 「각 시점은 유효하지만」의 전제를 먼저 확인한다.
fn each_point_valid(before: &[EventCycleFacts], after: &[EventCycleFacts]) {
    let registry = registry(9);
    check_cycle_refs(before, &earlier_world(), &registry).unwrap_or_else(|found| panic!("앞 시점이 3h 를 통과해야 한다: {found:#?}"));
    check_cycle_refs(after, &world(), &registry).unwrap_or_else(|found| panic!("뒤 시점이 3h 를 통과해야 한다: {found:#?}"));
}

/// 실패가 세 칸의 변경 갈래 하나뿐이면 그 목록을 돌려준다.
fn only_past_approval(problems: Vec<EventCycleHistoryProblem>) -> Vec<ApprovalChange> {
    match problems.as_slice() {
        [EventCycleHistoryProblem::PastApproval(changes)] => changes.clone(),
        other => panic!("승인 당시 칸의 변경 하나만 나와야 한다: {other:#?}"),
    }
}

fn fields(changes: &[ApprovalChange]) -> Vec<(EventName, ApprovalField)> {
    changes.iter().map(|change| (change.event(), change.field())).collect()
}

// ── 받아들이는 것 ─────────────────────────────────────────────────────────

#[test]
fn unchanged_records_are_kept() {
    let events = before_events();
    let found = kept(&events, &events);
    assert_eq!(found.compared(), [m(1), m(2), m(3)]);
    assert!(found.added().is_empty());
}

#[test]
fn empty_lists_and_a_first_observation_after_nothing_are_kept() {
    let found = kept(&[], &[]);
    assert!(found.compared().is_empty() && found.added().is_empty());
    let found = kept(&[], &before_events());
    assert!(found.compared().is_empty());
    assert_eq!(found.added(), [m(1), m(2), m(3)], "앞 시점에 없던 사건은 뒤 시점의 3h 만 받는다");
}

#[test]
fn several_events_appended_at_once_are_kept() {
    // 두 관측 사이에 사건이 몇 개 붙어도 된다. M4 는 C5 에서, M5 는 같은 C5 경계에서 다시, M6 은 과거 경계 C4 에서 승인했다.
    // 발급 번호는 같은 값이 이어진다(사이클이 새로 열리지 않았다).
    let mut after = before_events();
    after.push(event(4, ScopeChange, 2, at(5), 6, Base::Waiting { failures: 2 }));
    after.push(event(5, ScopeChange, 2, at(5), 6, Base::Waiting { failures: 0 }));
    after.push(event(6, ScopeChange, 2, at(4), 6, Base::Fixed { failures: 0, snapshot: 3, boundary: Some(5), issuer: 6 }));
    let found = kept(&before_events(), &after);
    assert_eq!(found.compared(), [m(1), m(2), m(3)]);
    assert_eq!(found.added(), [m(4), m(5), m(6)]);
}

#[test]
fn failed_attempts_added_to_the_waiting_event_are_kept() {
    let after = [start(), m2(), m3(Base::Waiting { failures: 4 })];
    kept(&before_events(), &after);
}

#[test]
fn archiving_at_another_boundary_after_the_interview_keeps_the_approval_boundary() {
    // §6.5 (가): M3 는 C4 에 고정된 채, 대기 중 인터뷰 C5 를 닫은 뒤 C5 에서 보관했다. 보관 경계·발급 번호(C5·6)가 승인 경계·발급
    // 번호(C4·5)와 다른 것은 과거 승인 경계의 수정이 아니다.
    let after = [start(), m2(), m3(Base::Fixed { failures: 1, snapshot: 3, boundary: Some(5), issuer: 6 })];
    let found = kept(&before_events(), &after);
    let fixed = found.baselines().history().compared().last().copied();
    assert_eq!(fixed, Some((m(3), BaselineUpdate::Fixed { added_before_archived: 0 })));
}

#[test]
fn the_last_waiting_event_may_be_archived_and_then_replaced_by_new_events() {
    // 앞 시점의 마지막 대기 사건이 최초 확정되고 뒤에 새 사건이 붙었다 — 3g 가 받아들이는 꼴이며 세 칸도 그대로다.
    let mut after = vec![start(), m2(), m3(Base::Fixed { failures: 1, snapshot: 3, boundary: Some(4), issuer: 5 })];
    after.push(event(4, ScopeChange, 2, at(5), 6, Base::Waiting { failures: 0 }));
    let found = kept(&before_events(), &after);
    assert_eq!(found.added(), [m(4)]);
}

#[test]
fn a_new_event_with_a_larger_issuer_does_not_make_the_old_issuer_movable() {
    // 새 사건의 발급 번호는 커질 수 있다(3h 의 비감소). 같은 사건의 발급 번호는 과거 기록이다 — 받아들이는 것은 새 사건뿐이다.
    let mut after = before_events();
    after.push(event(4, ScopeChange, 2, at(5), 6, Base::Waiting { failures: 0 }));
    kept(&before_events(), &after);

    let mut moved = after.clone();
    moved[2] = event(3, ScopeChange, 2, at(4), 6, Base::Waiting { failures: 1 });
    each_point_valid(&before_events(), &moved);
    let changes = only_past_approval(refused(&before_events(), &moved));
    assert_eq!(changes, [ApprovalChange::Issuer { event: m(3), before: 5, after: 6 }]);
}

// ── 각 시점은 유효하지만 과거 값이 바뀐 것 ──────────────────────────────────

#[test]
fn a_past_boundary_moved_to_another_closed_boundary_is_refused() {
    // M2 의 승인 경계 C2 를 같은 체인의 닫힌 C1 로 — 두 시점 모두 T7 을 지킨다(C1 < 3, 보관 경계 C2 와 발급 3 그대로).
    let mut after = before_events();
    after[1] = event(2, ScopeChange, 1, at(1), 3, Base::Fixed { failures: 0, snapshot: 2, boundary: Some(2), issuer: 3 });
    each_point_valid(&before_events(), &after);
    let changes = only_past_approval(refused(&before_events(), &after));
    assert_eq!(changes, [ApprovalChange::Boundary { event: m(2), before: at(2), after: at(1) }]);
}

#[test]
fn a_past_issuer_changed_to_another_valid_value_is_refused() {
    // 대기 중인 M3 의 발급 번호 5 를 6 으로 — C4 < 6, 앞 사건의 3 보다 작지 않다. 뒤 시점은 유효하다.
    let mut after = before_events();
    after[2] = event(3, ScopeChange, 2, at(4), 6, Base::Waiting { failures: 1 });
    each_point_valid(&before_events(), &after);
    let changes = only_past_approval(refused(&before_events(), &after));
    assert_eq!(fields(&changes), [(m(3), ApprovalField::NextCycleId)]);
}

#[test]
fn rewriting_the_approval_boundary_to_match_the_later_archive_is_refused() {
    // C5 에서 보관한 뒤 M3 의 승인 경계·발급 번호도 C5·6 으로 「맞춰」 고쳤다. 뒤 시점만 보면 보관 경계와 같은 평범한 사건이다.
    let after_kept = [start(), m2(), m3(Base::Fixed { failures: 1, snapshot: 3, boundary: Some(5), issuer: 6 })];
    let mut after = after_kept.to_vec();
    after[2] = event(3, ScopeChange, 2, at(5), 6, Base::Fixed { failures: 1, snapshot: 3, boundary: Some(5), issuer: 6 });
    each_point_valid(&before_events(), &after);
    let changes = only_past_approval(refused(&before_events(), &after));
    assert_eq!(
        changes,
        [
            ApprovalChange::Boundary { event: m(3), before: at(4), after: at(5) },
            ApprovalChange::Issuer { event: m(3), before: 5, after: 6 },
        ]
    );
}

#[test]
fn moving_chain_and_boundary_together_is_refused_though_each_point_owns_its_cycle() {
    // M3 를 chain:C2 의 C4 에서 chain:C1 의 C2 로 — 각 시점에서 경계는 사건 체인의 닫힌 사이클이므로 소속 검사로는 드러나지 않는다.
    let mut after = before_events();
    after[2] = event(3, ScopeChange, 1, at(2), 5, Base::Waiting { failures: 1 });
    each_point_valid(&before_events(), &after);
    let changes = only_past_approval(refused(&before_events(), &after));
    assert_eq!(
        changes,
        [
            ApprovalChange::Chain { event: m(3), before: ch(2), after: ch(1) },
            ApprovalChange::Boundary { event: m(3), before: at(4), after: at(2) },
        ]
    );
}

#[test]
fn a_chain_changed_where_no_cycle_reference_is_looked_up_is_still_refused() {
    // M1 은 실제 사이클 참조가 없어 3h 가 chain_ref 를 조회하지 않는다. 다른 체인이든 목록에 없는 체인이든 각 시점은 통과한다.
    for chain in [2, 9] {
        let mut after = before_events();
        after[0] = event(1, Start, chain, Written(None), 1, Base::Fixed { failures: 0, snapshot: 1, boundary: None, issuer: 1 });
        each_point_valid(&before_events(), &after);
        let changes = only_past_approval(refused(&before_events(), &after));
        assert_eq!(changes, [ApprovalChange::Chain { event: m(1), before: ch(1), after: ch(chain) }]);
    }
}

#[test]
fn each_field_and_event_is_told_apart() {
    // 세 사건의 세 칸을 하나씩 바꾼다 — 각 시점은 유효하다. 바뀐 칸만, 그 사건 이름으로 나온다.
    let cases: [(usize, EventCycleFacts, ApprovalField); 4] = [
        (0, event(1, Start, 2, Written(None), 1, Base::Fixed { failures: 0, snapshot: 1, boundary: None, issuer: 1 }), ApprovalField::ChainRef),
        (1, event(2, ScopeChange, 1, at(1), 3, Base::Fixed { failures: 0, snapshot: 2, boundary: Some(2), issuer: 3 }), ApprovalField::AfterCycleRef),
        (2, event(3, ScopeChange, 2, at(4), 7, Base::Waiting { failures: 1 }), ApprovalField::NextCycleId),
        (2, event(3, ScopeChange, 1, at(4), 5, Base::Waiting { failures: 1 }), ApprovalField::ChainRef),
    ];
    for (place, changed, field) in cases {
        let mut after = before_events();
        after[place] = changed;
        let found = refused(&before_events(), &after);
        if field != ApprovalField::ChainRef || place == 0 {
            each_point_valid(&before_events(), &after);
            assert_eq!(fields(&only_past_approval(found)), [(m(place as u32 + 1), field)]);
        } else {
            // 경계 C4 는 chain:C2 의 사이클이므로 뒤 시점의 3h 도 다른 체인 소속으로 거절한다. 두 갈래가 함께 나온다.
            assert!(matches!(found.first(), Some(EventCycleHistoryProblem::AfterRefs(_))), "{found:#?}");
            assert!(
                matches!(found.last(), Some(EventCycleHistoryProblem::PastApproval(changes)) if fields(changes) == [(m(3), field)]),
                "{found:#?}"
            );
        }
    }
}

// ── 대응 ───────────────────────────────────────────────────────────────────

/// 대응 실패는 3g 갈래 안의 3f 대응 실패로만 나오고, 세 칸은 비교하지 않았다.
fn unmatched(problems: &[EventCycleHistoryProblem]) -> Vec<MatchProblem> {
    assert!(
        !problems.iter().any(|problem| matches!(problem, EventCycleHistoryProblem::PastApproval(_))),
        "대응이 서지 않으면 세 칸을 비교하지 않는다: {problems:#?}"
    );
    problems
        .iter()
        .find_map(|problem| match problem {
            EventCycleHistoryProblem::Baselines(found) => found.iter().find_map(|inner| match inner {
                ReplacedBaselineProblem::History(history) => history.iter().find_map(|branch| match branch {
                    BaselineHistoryProblem::Unmatched(found) => Some(found.clone()),
                    _ => None,
                }),
                _ => None,
            }),
            _ => None,
        })
        .unwrap_or_else(|| panic!("3f 의 대응 실패가 있어야 한다: {problems:#?}"))
}

#[test]
fn a_removed_or_truncated_past_event_is_refused_without_comparing() {
    let before = before_events();
    let missing = |events: &[u32]| events.iter().map(|&n| MatchProblem::Missing { event: m(n) }).collect::<Vec<_>>();
    assert_eq!(unmatched(&refused(&before, &before[..2])), missing(&[3]), "마지막 사건 삭제");
    assert_eq!(unmatched(&refused(&before, &[])), missing(&[1, 2, 3]), "목록 비우기");
    // 지운 자리에 새 사건을 붙여도 같은 자리의 이름·종류가 맞지 않으면 다른 사건이다.
    let mut after = before[..2].to_vec();
    after.push(event(4, ScopeChange, 2, at(4), 5, Base::Waiting { failures: 1 }));
    let found = refused(&before, &after);
    assert!(matches!(unmatched(&found).first(), Some(MatchProblem::NameOutOfPlace { side: Side::After, .. })), "{found:#?}");
}

#[test]
fn reordered_events_are_not_compared_as_the_same_events() {
    // M2 와 M3 의 자리를 바꾸고 이름도 자리에 맞게 바꿔 적은 목록이 아니라, 원래 이름 그대로 순서만 바꾼 목록이다.
    let before = before_events();
    let after = [before[0].clone(), before[2].clone(), before[1].clone()];
    let found = refused(&before, &after);
    assert!(matches!(unmatched(&found).first(), Some(MatchProblem::NameOutOfPlace { side: Side::After, .. })));
    assert!(matches!(found.first(), Some(EventCycleHistoryProblem::AfterRefs(_))), "뒤 시점의 3h 도 이름 자리로 거절한다: {found:#?}");
}

#[test]
fn a_changed_kind_is_not_compared_even_when_its_three_fields_differ() {
    // 같은 이름·다른 종류는 같은 사건이 아니다. 세 칸이 달라도 「바뀐 칸」으로 견주지 않고, 성공도 아니다.
    let before = before_events();
    let mut after = before.clone();
    after[2] = event(3, RestoreResult, 2, at(5), 6, Base::Waiting { failures: 1 });
    let found = refused(&before, &after);
    assert_eq!(unmatched(&found), [MatchProblem::KindDiffers { event: m(3), before: ScopeChange, after: RestoreResult }]);
}

#[test]
fn names_out_of_place_before_are_not_compared() {
    let before = [start(), m3(Base::Waiting { failures: 1 })];
    let found = refused(&before, &before);
    assert!(matches!(unmatched(&found).first(), Some(MatchProblem::NameOutOfPlace { side: Side::Before, .. })));
    assert!(matches!(found.first(), Some(EventCycleHistoryProblem::BeforeRefs(_))));
}

// ── 기존 검사의 판정은 그대로 ───────────────────────────────────────────────

#[test]
fn a_changed_fixed_baseline_is_still_refused_by_the_existing_comparison() {
    // M2 의 확정 기준점 A2 를 A4 로, 또는 보관 경계를 C1 로 — 세 칸은 그대로이고 각 시점은 유효하다. 3f 가 거절한다.
    for changed in [
        event(2, ScopeChange, 1, at(2), 3, Base::Fixed { failures: 0, snapshot: 4, boundary: Some(2), issuer: 3 }),
        event(2, ScopeChange, 1, at(2), 3, Base::Fixed { failures: 0, snapshot: 2, boundary: Some(1), issuer: 3 }),
    ] {
        let mut after = before_events();
        after[1] = changed;
        each_point_valid(&before_events(), &after);
        let found = refused(&before_events(), &after);
        let baselines = |events: &[EventCycleFacts]| events.iter().map(|facts| facts.baseline().clone()).collect::<Vec<_>>();
        let expected = check_replaced_baselines((&baselines(&before_events()), &registry(9)), (&baselines(&after), &registry(9)))
            .expect_err("3g 가 거절한다");
        assert_eq!(found, [EventCycleHistoryProblem::Baselines(expected)], "3g 의 결과를 그대로 담는다");
    }
}

#[test]
fn a_baseline_added_to_an_already_replaced_event_is_still_refused() {
    // 앞 시점에 M3 는 M4 로 이미 대체됐다. 뒤 시점에 M3 에 기준점이 적혔다 — 세 칸은 그대로이고 3h 는 두 시점 모두 통과한다.
    let mut before = before_events();
    before.push(event(4, ScopeChange, 2, at(4), 5, Base::Waiting { failures: 0 }));
    let mut after = before.clone();
    after[2] = m3(Base::Fixed { failures: 1, snapshot: 3, boundary: Some(5), issuer: 6 });
    each_point_valid(&before, &after);
    let found = refused(&before, &after);
    assert!(
        matches!(found.as_slice(), [EventCycleHistoryProblem::Baselines(inner)]
            if inner == &[ReplacedBaselineProblem::ReplacedGainedBaseline { event: m(3), last_before: m(4), snapshot: a(3) }]),
        "{found:#?}"
    );
}

#[test]
fn a_late_failure_on_a_replaced_event_stays_unjudged_in_success() {
    // 3g 는 대체된 사건에 실패 기록만 뒤늦게 붙은 것을 판단하지 않는다 — 이 검사의 성공 값에도 그대로 남는다.
    let mut before = before_events();
    before.push(event(4, ScopeChange, 2, at(4), 5, Base::Waiting { failures: 0 }));
    let mut after = before.clone();
    after[2] = m3(Base::Waiting { failures: 3 });
    let found = kept(&before, &after);
    assert_eq!(found.baselines().unjudged_late_attempts(), [(m(3), 2)]);
}

#[test]
fn failures_of_either_point_are_kept_apart_and_never_turned_into_success() {
    let before = before_events();
    // 앞 시점만 틀렸다 — M3 가 그때 열려 있던 C5 를 가리킨다. 뒤 시점과 세 칸의 보존은 맞다.
    let mut broken = before.clone();
    broken[2] = event(3, ScopeChange, 2, at(5), 6, Base::Waiting { failures: 1 });
    let found = refused(&broken, &broken);
    let expected = check_cycle_refs(&broken, &earlier_world(), &registry(9)).expect_err("앞 시점은 C5 가 열려 있다");
    assert_eq!(found, [EventCycleHistoryProblem::BeforeRefs(expected)]);
    check_cycle_refs(&broken, &world(), &registry(9)).expect("뒤 시점에는 C5 가 닫혀 있다");

    // 뒤 시점만 틀렸다 — 새 사건이 열린 C6 을 가리킨다.
    let mut after = before.clone();
    after.push(event(4, ScopeChange, 2, at(6), 7, Base::Waiting { failures: 0 }));
    let found = refused(&before, &after);
    let expected = check_cycle_refs(&after, &world(), &registry(9)).expect_err("C6 은 열려 있다");
    assert_eq!(found, [EventCycleHistoryProblem::AfterRefs(expected)]);
}

#[test]
fn all_branches_are_returned_in_order() {
    // 앞 시점 3h 실패, 뒤 시점 3h 실패, 3g 실패, 세 칸의 변경이 한 번에 — 갈래 순서대로 모두 나온다.
    let mut before = before_events();
    before[2] = event(3, ScopeChange, 2, at(5), 6, Base::Waiting { failures: 1 });
    let mut after = before.clone();
    after[1] = event(2, ScopeChange, 1, at(1), 3, Base::Fixed { failures: 0, snapshot: 4, boundary: Some(2), issuer: 3 });
    after.push(event(4, ScopeChange, 2, at(6), 7, Base::Waiting { failures: 0 }));
    let found = refused(&before, &after);
    let branches: Vec<&str> = found
        .iter()
        .map(|problem| match problem {
            EventCycleHistoryProblem::BeforeRefs(_) => "before",
            EventCycleHistoryProblem::AfterRefs(_) => "after",
            EventCycleHistoryProblem::Baselines(_) => "baselines",
            EventCycleHistoryProblem::PastApproval(_) => "approval",
        })
        .collect();
    assert_eq!(branches, ["before", "after", "baselines", "approval"]);
    assert!(matches!(found.last(), Some(EventCycleHistoryProblem::PastApproval(changes)) if fields(changes) == [(m(2), ApprovalField::AfterCycleRef)]));
}

// ── null·칸 없음 ─────────────────────────────────────────────────────────────

#[test]
fn null_and_unwritten_are_compared_as_written_values_without_being_approved() {
    // 기존 프로젝트 첫 채택처럼 null 경계로 적힌 scope_change, 경계 칸이 없는 사건 — 그대로면 보존이다. 그러나 그 값이 그 종류에서
    // 허용된다는 판단은 아니다: 두 시점의 3h 성공 값에 「판단하지 않음」이 그대로 남는다.
    let null_change = event(2, ScopeChange, 2, Written(None), 5, Base::Waiting { failures: 0 });
    let unwritten = event(2, ScopeChange, 2, Unwritten, 5, Base::Waiting { failures: 0 });
    for second in [null_change.clone(), unwritten.clone()] {
        let events = [start(), second.clone()];
        let found = kept(&events, &events);
        assert_eq!(found.compared(), [m(1), m(2)]);
        let reading = match second.boundary() {
            Written(None) => RefReading::NullNotJudged,
            _ => RefReading::UnwrittenNotJudged,
        };
        for point in [found.before(), found.after()] {
            assert!(point.not_judged().contains(&(m(1), CycleField::AfterCycleRef, RefReading::NullNotJudged)), "start 의 null");
            assert!(point.not_judged().contains(&(m(2), CycleField::AfterCycleRef, reading)), "{reading:?}");
            assert_eq!(point.chains_not_looked_up(), [m(1), m(2)]);
        }
    }

    // 칸 없음·null·실제 참조 사이의 어떤 바뀜도 승인 당시 칸의 변경이다.
    for (earlier, later) in [(&null_change, &unwritten), (&unwritten, &null_change)] {
        let found = only_past_approval(refused(&[start(), earlier.clone()], &[start(), later.clone()]));
        assert_eq!(found, [ApprovalChange::Boundary { event: m(2), before: earlier.boundary(), after: later.boundary() }]);
    }
    let filled = event(2, ScopeChange, 2, at(4), 5, Base::Waiting { failures: 0 });
    for earlier in [&null_change, &unwritten] {
        each_point_valid(&[start(), earlier.clone()], &[start(), filled.clone()]);
        let found = only_past_approval(refused(&[start(), earlier.clone()], &[start(), filled.clone()]));
        assert_eq!(found, [ApprovalChange::Boundary { event: m(2), before: earlier.boundary(), after: at(4) }]);
    }
    let found = only_past_approval(refused(&[start(), filled.clone()], &[start(), null_change.clone()]));
    assert_eq!(fields(&found), [(m(2), ApprovalField::AfterCycleRef)]);
}

#[test]
fn each_point_keeps_its_own_unjudged_readings() {
    // 뒤 시점에만 null 경계의 새 사건(M4)이 붙었다. 뒤 시점의 「판단하지 않음」은 뒤 시점 값에만 있고, 앞 시점 값으로 바뀌어 사라지지
    // 않는다.
    let mut after = before_events();
    after.push(event(4, ScopeChange, 2, Written(None), 5, Base::Waiting { failures: 0 }));
    let found = kept(&before_events(), &after);
    let unjudged_m4 = (m(4), CycleField::AfterCycleRef, RefReading::NullNotJudged);
    assert!(found.after().not_judged().contains(&unjudged_m4), "{:#?}", found.after().not_judged());
    assert!(!found.before().not_judged().iter().any(|(event, _, _)| *event == m(4)));
    assert_eq!(found.after().chains_not_looked_up(), [m(1), m(4)]);
    assert_eq!(found.before().chains_not_looked_up(), [m(1)]);
    let registry = registry(9);
    assert_eq!(found.before(), &check_cycle_refs(&before_events(), &earlier_world(), &registry).unwrap(), "3h 의 성공 값 그대로");
    assert_eq!(found.after(), &check_cycle_refs(&after, &world(), &registry).unwrap(), "3h 의 성공 값 그대로");
}

// ── 입력·한계 ───────────────────────────────────────────────────────────────

#[test]
fn the_check_changes_neither_inputs_nor_chains_nor_registries() {
    let before = before_events();
    let mut after = before.clone();
    after[1] = event(2, ScopeChange, 1, at(1), 3, Base::Fixed { failures: 0, snapshot: 2, boundary: Some(2), issuer: 3 });
    let (earlier, later, first, second) = (earlier_world(), world(), registry(9), registry(5));
    let seen = (before.clone(), after.clone(), format!("{earlier:?}"), format!("{later:?}"), first.clone(), second.clone());
    for pair in [(&before, &after), (&before, &before)] {
        let _ = check_event_cycle_history((pair.0, &earlier, &first), (pair.1, &later, &second));
    }
    assert_eq!((before, after), (seen.0, seen.1));
    assert_eq!((format!("{earlier:?}"), format!("{later:?}")), (seen.2, seen.3));
    assert_eq!((first, second), (seen.4, seen.5));
}

#[test]
fn what_two_observations_cannot_see_is_accepted_and_written_down() {
    // ① 두 관측 사이에 바꿨다가 같은 값으로 되돌렸다 — 두 입력이 같으면 보존으로 보인다.
    kept(&before_events(), &before_events());
    // ② 앞 시점 목록의 끝이 잘려 건네졌다 — 잘린 M3 은 「새 사건」으로 보여 그 세 칸이 실제 과거와 달라도 비교되지 않는다.
    let truncated = &before_events()[..2];
    let mut after = before_events();
    after[2] = event(3, ScopeChange, 2, at(4), 6, Base::Waiting { failures: 1 });
    let found = kept(truncated, &after);
    assert_eq!(found.added(), [m(3)]);
    // ③ 같은 이름·종류의 다른 사건으로 바꿔 넣었다 — 세 칸이 같고 baseline 이 허용된 갱신이면 구별하지 못한다.
    let said = APPROVAL_HISTORY_DOES_NOT_SHOW.join("\n");
    for topic in ["되돌린", "끝이 잘려", "바꿔 넣었는가", "review_cycle_ref", "null", "SnapshotRegistry", "O-3", "제품 연결"] {
        assert!(said.contains(topic), "{topic}");
    }
}

#[test]
fn messages_name_the_event_and_the_field() {
    let change = ApprovalChange::Boundary { event: m(3), before: at(4), after: Written(None) };
    let text = EventCycleHistoryProblem::PastApproval(vec![change]).to_string();
    assert!(text.contains("event:M3") && text.contains("after_cycle_ref") && text.contains("cycle:C4") && text.contains("null"), "{text}");
    let text = ApprovalChange::Chain { event: m(1), before: ch(1), after: ch(2) }.to_string();
    assert!(text.contains("chain_ref") && text.contains("chain:C1") && text.contains("chain:C2"), "{text}");
    let text = ApprovalChange::Issuer { event: m(2), before: 3, after: 4 }.to_string();
    assert!(text.contains("next_cycle_id") && text.contains('3') && text.contains('4'), "{text}");
}

#[test]
fn the_module_list_names_this_check_without_widening_the_others() {
    assert!(UNCHECKED_CONDITIONS.iter().any(|line| line.starts_with("T1") && line.contains("event_cycle_history")));
    assert!(UNCHECKED_CONDITIONS.iter().any(|line| line.starts_with("T7") && line.contains("event_cycle_history")));
    // 기존 모듈 자신의 한계 문장은 그대로다 — 3d 는 여전히 사이클 칸을 입력으로 받지 않는다고 말한다.
    assert!(HISTORY_DOES_NOT_SHOW.iter().any(|line| line.contains("chain_ref·after_cycle_ref")));
}
