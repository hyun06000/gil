//! 건네받은 실제 사이클 참조의 존재·닫힘·체인 소속과 발급 번호 관계 검사(「3h」) 시험 — **합성 입력**과 **실제 전이로 만든
//! 체인**을 쓴다.
//!
//! 사건 칸·시도 결과·등록부는 시험이 만든 값이다. 체인과 사이클은 기존 전이 함수(`Cycle::open_step`·`Cycle::close_step`·
//! `Cycles::close_current`·`Cycles::open_child`·`Chain::close`)로 걸어 만든다 — 존재·닫힘·소속을 따로 적은 bool 목록이 아니다.
//! 승인은 시험 자료의 `approved: yes` 이며 모니터 승인이 아니다. `state.yaml`·`gil start`·파일 시스템을 쓰지 않는다.
//!
//! 세계(모든 시험 공통):
//!
//! ```text
//! chain:C1 (닫힘)  cycle:C1 ─▶ cycle:C2 ─▶ cycle:C3(종료 인터뷰)          모두 닫힘
//! chain:C2 (열림)  cycle:C4 ─▶ cycle:C5 ─▶ cycle:C6                        C4·C5 닫힘, C6 열림
//! ```

use super::*;
use crate::artifact::{ManifestAddress, SnapshotRegistry};
use crate::cycles::Cycles;
use crate::refs::{ChainRef, CycleRef, ExistenceRef, JourneyRef, SnapshotRef, StepRef};
use crate::{Chain, CycleKind, NodeKind, Report, RuleSet};

use AttemptSeen::{Archived as A, NotArchived as F};
use BaselineSlot::{Unwritten, Written};
use EventKind::{RestoreResult, ScopeChange, Start};

#[path = "../../../tests/common/mod.rs"]
mod common;

// ── 체인: 실제 전이로 만든다 ───────────────────────────────────────────────

fn x1() -> ExistenceRef {
    ExistenceRef::new(1).expect("X1")
}

fn rules() -> RuleSet {
    common::spec().for_chain()
}

/// 지금 서 있는 열린 Interview 를 Question → Interpretation → 승인된 Synthesis → Outcome 으로 걷고 `action` 방향으로 닫는다.
/// 돌려주는 것은 그 Synthesis 의 주소.
fn close_interview(cycles: &mut Cycles, action: &str) -> StepRef {
    let (synthesis, report) = {
        let cycle = cycles.current_mut();
        for kind in [NodeKind::Question, NodeKind::Interpretation, NodeKind::Synthesis] {
            cycle.open_step(kind).unwrap_or_else(|err| panic!("{kind} 를 열지 못했다: {err}"));
            let mut report = common::full_report(cycle.rules(), CycleKind::Interview, kind);
            if kind == NodeKind::Synthesis {
                report.insert(common::APPROVED, common::APPROVAL);
                let basis: Vec<String> = cycle
                    .steps()
                    .nodes()
                    .iter()
                    .filter(|node| matches!(node.kind, NodeKind::Question | NodeKind::Interpretation))
                    .map(|node| cycle.step_ref(node.id).to_string())
                    .collect();
                report.insert(common::BASIS_REFS, basis.join("\n"));
                if action == "close_chain" {
                    report.insert("chain_verdict", "success");
                }
            }
            cycle.close_step(report).unwrap_or_else(|err| panic!("{kind} 를 닫지 못했다: {err}"));
        }
        let synthesis = cycle.step_ref(cycle.steps().current().expect("Synthesis 에 서 있다"));
        cycle.open_step(NodeKind::Outcome).expect("Outcome 을 연다");
        let outcome = cycle.steps().current().expect("Outcome 에 서 있다");
        let report = common::full_report(cycle.rules(), CycleKind::Interview, NodeKind::Outcome)
            .with(common::SYNTHESIS_REF, synthesis.to_string())
            .with(common::REASON, "승인된 Synthesis 를 얻었다");
        cycle.close_step(report).expect("Outcome 을 닫는다");
        (synthesis, common::cycle_report(cycle, "success", outcome).with(common::ACTION, action))
    };
    cycles.close_current(report, JourneyRef::new(x1(), 0)).expect("Interview 를 닫는다");
    synthesis
}

fn open_interview(cycles: &mut Cycles) {
    cycles.open_child(CycleKind::Interview, x1()).expect("적어 둔 방향대로 연다");
}

fn ch(number: u32) -> ChainRef {
    ChainRef::new(number).expect("0 이 아니다")
}

/// 모든 시험의 세계 — 위 그림 그대로.
fn world() -> Vec<Chain> {
    let mut first = Cycles::start_at(rules(), x1(), a(1), 1);
    close_interview(&mut first, "open_child");
    open_interview(&mut first);
    close_interview(&mut first, "open_child");
    open_interview(&mut first);
    let closing = close_interview(&mut first, "close_chain");
    let mut chain1 = Chain::restore(ch(1), x1(), a(1), None, first);
    chain1
        .close(
            Report::new()
                .with("verdict", "success")
                .with("closing_synthesis_ref", closing.to_string())
                .with("handoff_summary", "합성 인계 — 시험 자료"),
        )
        .expect("종료 인터뷰를 닫은 체인을 닫는다");

    let entry = chain1.exit_snapshot().expect("닫힌 체인의 Exit");
    let mut second = Cycles::start_at(rules(), x1(), entry, chain1.cycles().next_name());
    close_interview(&mut second, "open_child");
    open_interview(&mut second);
    close_interview(&mut second, "open_child");
    open_interview(&mut second);
    let chain2 = Chain::restore(ch(2), x1(), entry, Some(ch(1)), second);
    vec![chain1, chain2]
}

#[test]
fn the_world_is_what_the_fixture_says() {
    let chains = world();
    assert!(chains[0].is_closed() && !chains[1].is_closed());
    let seen: Vec<(u32, u32, bool)> = chains
        .iter()
        .flat_map(|chain| {
            chain.cycles().nodes().iter().map(move |cycle| (chain.id().number(), cycle.id().to_ref().number(), cycle.is_closed()))
        })
        .collect();
    assert_eq!(
        seen,
        [(1, 1, true), (1, 2, true), (1, 3, true), (2, 4, true), (2, 5, true), (2, 6, false)],
        "체인 소유·닫힘이 그림과 같아야 한다"
    );
    assert!(chains[0].owns(c(2)) && !chains[1].owns(c(2)) && chains[1].owns(c(6)));
}

// ── 사건: 합성 입력 ─────────────────────────────────────────────────────────

fn m(number: u32) -> EventName {
    EventName::new(number).expect("0 이 아니다")
}

fn a(number: u32) -> SnapshotRef {
    SnapshotRef::new(number).expect("0 이 아니다")
}

fn c(number: u32) -> CycleRef {
    CycleRef::new(number).expect("0 이 아니다")
}

/// `A1`…`A<count>` 를 아는 합성 등록부. 주소는 아무 객체도 가리키지 않는 합성 값이다.
fn registry(count: u32) -> SnapshotRegistry {
    let records = (1..=count)
        .map(|n| (n, ManifestAddress::from_parts("sha256", &[u8::try_from(n).unwrap(); 32]).unwrap()))
        .collect();
    SnapshotRegistry::restore(count + 1, records).expect("시험 등록부는 꼴이 맞다")
}

/// 사건의 `baseline` 쪽.
#[derive(Clone, Copy)]
enum Base {
    /// 기준점 없음, 경계·발급기 칸 없음, 실패 시도 하나.
    Waiting,
    /// 기준점·경계(`None` = `null`)·발급기를 함께 적고 `archived` 하나.
    Fixed { snapshot: u32, boundary: Option<u32>, issuer: u32 },
}

fn facts(event: u32, kind: EventKind, chain: u32, after: Option<u32>, issuer: u32, base: Base) -> EventCycleFacts {
    let baseline = match base {
        Base::Waiting => BaselineFacts::new(m(event), kind, None, Unwritten, Unwritten, vec![F]),
        Base::Fixed { snapshot, boundary, issuer } => BaselineFacts::new(
            m(event),
            kind,
            Some(a(snapshot)),
            Written(boundary.map(c)),
            Written(issuer),
            vec![A],
        ),
    };
    EventCycleFacts::new(baseline, ch(chain), Written(after.map(c)), issuer)
}

/// §5.1 예시의 `start` — `chain_ref: chain:C1`, 경계 `null`, `next_cycle_id: 1`, 기준점 A1(경계 `null`, 발급기 1).
fn start() -> EventCycleFacts {
    facts(1, Start, 1, None, 1, Base::Fixed { snapshot: 1, boundary: None, issuer: 1 })
}

/// 사이클 경계에서 승인하고 그 경계에서 바로 보관한 `scope_change`.
fn change(event: u32, chain: u32, after: u32, issuer: u32, snapshot: u32) -> EventCycleFacts {
    facts(event, ScopeChange, chain, Some(after), issuer, Base::Fixed { snapshot, boundary: Some(after), issuer })
}

/// 보관 대기인 `scope_change`.
fn waiting(event: u32, chain: u32, after: u32, issuer: u32) -> EventCycleFacts {
    facts(event, ScopeChange, chain, Some(after), issuer, Base::Waiting)
}

fn agree(events: &[EventCycleFacts]) -> CycleRefsAgree {
    check_cycle_refs(events, &world(), &registry(9)).unwrap_or_else(|problems| panic!("받아들여야 한다: {problems:#?}"))
}

fn refused(events: &[EventCycleFacts]) -> Vec<CycleRefProblem> {
    check_cycle_refs(events, &world(), &registry(9)).expect_err("거절해야 한다")
}

fn checked(events: &[EventCycleFacts]) -> Vec<(EventName, RefReading, RefReading)> {
    agree(events).events().iter().map(|refs| (refs.event(), refs.after(), refs.baseline_after())).collect()
}

use RefReading::{Checked, NullNotJudged, UnwrittenNotJudged};

// ── 정상 ───────────────────────────────────────────────────────────────────

#[test]
fn an_empty_event_list_has_nothing_to_check() {
    let found = agree(&[]);
    assert!(found.events().is_empty() && found.not_judged().is_empty() && found.chains_not_looked_up().is_empty());
}

#[test]
fn an_event_pointing_at_a_closed_cycle_of_its_own_chain_is_accepted() {
    let events = [start(), change(2, 1, 1, 2, 2)];
    assert_eq!(
        checked(&events),
        [(m(1), NullNotJudged, NullNotJudged), (m(2), Checked(c(1)), Checked(c(1)))]
    );
}

#[test]
fn consecutive_events_at_the_same_boundary_may_share_the_issuer() {
    // §5.3: 보관 대기 중 다른 변경안 승인 — 사이클이 새로 열리지 않은 같은 경계, 같은 발급 번호.
    let events = [start(), waiting(2, 1, 2, 3), waiting(3, 1, 2, 3), change(4, 1, 2, 3, 2)];
    assert_eq!(agree(&events).events().len(), 4);
}

#[test]
fn the_issuer_may_jump_by_more_than_one() {
    let events = [start(), change(2, 1, 1, 2, 2), change(3, 1, 3, 4, 3)];
    agree(&events);
}

#[test]
fn a_past_boundary_is_accepted_while_the_issuer_does_not_decrease() {
    // §6.5 「되돌아가기」: M3 은 C7, 그 뒤 M4 는 되돌아간 C4. 경계 번호는 내려가도 발급 번호는 내려가지 않는다.
    let events = [
        start(),
        change(2, 1, 2, 3, 2),
        facts(3, RestoreResult, 1, Some(1), 3, Base::Fixed { snapshot: 3, boundary: Some(1), issuer: 3 }),
    ];
    assert_eq!(checked(&events)[2], (m(3), Checked(c(1)), Checked(c(1))));
}

#[test]
fn archiving_after_an_interview_at_another_boundary_with_a_larger_issuer_is_accepted() {
    // §6.3·§6.5 (가): C4 에서 승인(발급 번호 5), 대기 중 인터뷰 C5 가 닫힌 뒤 C5 에서 보관(발급 번호 6).
    let events = [
        start(),
        change(2, 1, 1, 2, 2),
        facts(3, ScopeChange, 2, Some(4), 5, Base::Fixed { snapshot: 4, boundary: Some(5), issuer: 6 }),
    ];
    assert_eq!(checked(&events)[2], (m(3), Checked(c(4)), Checked(c(5))));
}

#[test]
fn ownership_is_read_per_chain_when_several_chains_are_given() {
    let events = [start(), change(2, 1, 2, 3, 2), change(3, 1, 3, 4, 3), change(4, 2, 4, 5, 4), change(5, 2, 5, 6, 5)];
    let found = agree(&events);
    let chains: Vec<(EventName, ChainRef)> = found.events().iter().map(|refs| (refs.event(), refs.chain())).collect();
    assert_eq!(chains, [(m(1), ch(1)), (m(2), ch(1)), (m(3), ch(1)), (m(4), ch(2)), (m(5), ch(2))]);
}

#[test]
fn history_of_a_chain_that_is_closed_now_is_not_refused() {
    let chains = world();
    assert!(chains[0].is_closed(), "chain:C1 은 지금 닫혀 있다");
    let events = [start(), change(2, 1, 2, 3, 2)];
    assert!(check_cycle_refs(&events, &chains, &registry(9)).is_ok());
}

#[test]
fn the_null_boundary_of_start_is_not_a_missing_or_wrong_reference() {
    let found = agree(&[start()]);
    assert_eq!(
        found.not_judged(),
        [(m(1), CycleField::AfterCycleRef, NullNotJudged), (m(1), CycleField::BaselineAfterCycleRef, NullNotJudged)]
    );
    assert_eq!(found.chains_not_looked_up(), [m(1)], "참조가 없으면 chain_ref 를 조회하지 않았음을 드러낸다");
}

#[test]
fn a_waiting_event_is_not_refused_by_the_reference_check() {
    let found = agree(&[start(), change(2, 1, 1, 2, 2), waiting(3, 1, 2, 3)]);
    assert_eq!(found.events()[2].after(), Checked(c(2)));
    assert_eq!(found.events()[2].baseline_after(), UnwrittenNotJudged);
    assert!(found.not_judged().contains(&(m(3), CycleField::BaselineAfterCycleRef, UnwrittenNotJudged)));
    assert_eq!(found.baselines().without_snapshot(), [m(3)], "3e 의 성공 값이 그대로 실린다");
}

#[test]
fn equal_numbers_of_chain_and_cycle_do_not_link_them() {
    // chain:C1 과 cycle:C1 은 숫자가 같을 뿐이다. 소속은 Chain::owns 로 읽는다.
    agree(&[start(), change(2, 1, 1, 2, 2)]);
    // chain:C2 의 사건이 cycle:C2(chain:C1 의 사이클)를 가리키면 번호가 같아도 거절한다.
    let found = refused(&[start(), change(2, 2, 2, 3, 2)]);
    assert_eq!(
        found,
        [
            CycleRefProblem::OtherChain { event: m(2), field: CycleField::AfterCycleRef, cycle: c(2), chain: ch(2), owner: ch(1) },
            CycleRefProblem::OtherChain {
                event: m(2),
                field: CycleField::BaselineAfterCycleRef,
                cycle: c(2),
                chain: ch(2),
                owner: ch(1)
            },
        ]
    );
}

// ── 거절 ───────────────────────────────────────────────────────────────────

#[test]
fn an_unknown_chain_is_refused_when_a_real_reference_needs_it() {
    let found = refused(&[start(), change(2, 9, 1, 2, 2)]);
    assert_eq!(found, [CycleRefProblem::UnknownChain { event: m(2), chain: ch(9) }]);
}

#[test]
fn an_unknown_cycle_is_refused() {
    let found = refused(&[start(), facts(2, ScopeChange, 1, Some(9), 10, Base::Waiting)]);
    assert_eq!(found, [CycleRefProblem::UnknownCycle { event: m(2), field: CycleField::AfterCycleRef, cycle: c(9) }]);
}

#[test]
fn a_baseline_boundary_in_another_chain_is_refused() {
    // 승인은 chain:C1 의 C3 에서, 보관 경계는 chain:C2 의 C4 — 승인과 보관은 한 체인 안에서 끝난다(§6.6, T7).
    let events = [
        start(),
        facts(2, ScopeChange, 1, Some(3), 4, Base::Fixed { snapshot: 2, boundary: Some(4), issuer: 5 }),
    ];
    assert_eq!(
        refused(&events),
        [CycleRefProblem::OtherChain {
            event: m(2),
            field: CycleField::BaselineAfterCycleRef,
            cycle: c(4),
            chain: ch(1),
            owner: ch(2)
        }]
    );
}

#[test]
fn an_open_cycle_is_refused() {
    let found = refused(&[start(), waiting(2, 2, 6, 7)]);
    assert_eq!(found, [CycleRefProblem::OpenCycle { event: m(2), field: CycleField::AfterCycleRef, cycle: c(6) }]);
    let found = refused(&[
        start(),
        facts(2, ScopeChange, 2, Some(5), 6, Base::Fixed { snapshot: 2, boundary: Some(6), issuer: 7 }),
    ]);
    assert_eq!(
        found,
        [CycleRefProblem::OpenCycle { event: m(2), field: CycleField::BaselineAfterCycleRef, cycle: c(6) }]
    );
}

#[test]
fn a_cycle_number_equal_to_or_above_the_issuer_is_refused() {
    for (cycle, issuer) in [(2, 2), (3, 2)] {
        let found = refused(&[start(), waiting(2, 1, cycle, issuer)]);
        assert_eq!(
            found,
            [CycleRefProblem::CycleNotBeforeIssuer { event: m(2), field: CycleField::AfterCycleRef, cycle: c(cycle), issuer }],
            "C{cycle} · next_cycle_id {issuer}"
        );
    }
    for (cycle, issuer) in [(2, 2), (3, 2)] {
        let events = [
            start(),
            facts(2, ScopeChange, 1, Some(1), 2, Base::Fixed { snapshot: 2, boundary: Some(cycle), issuer }),
        ];
        assert_eq!(
            refused(&events),
            [CycleRefProblem::CycleNotBeforeIssuer {
                event: m(2),
                field: CycleField::BaselineAfterCycleRef,
                cycle: c(cycle),
                issuer
            }],
            "baseline C{cycle} · baseline.next_cycle_id {issuer}"
        );
    }
}

#[test]
fn a_decreasing_issuer_in_event_order_is_refused() {
    let found = refused(&[start(), change(2, 1, 2, 3, 2), change(3, 1, 1, 2, 3)]);
    assert_eq!(
        found,
        [CycleRefProblem::IssuerDecreased { event: m(3), issuer: 2, previous: m(2), previous_issuer: 3 }]
    );
}

#[test]
fn a_baseline_issuer_below_the_event_issuer_is_refused() {
    let events = [
        start(),
        facts(2, ScopeChange, 1, Some(2), 3, Base::Fixed { snapshot: 2, boundary: Some(1), issuer: 2 }),
    ];
    assert_eq!(
        refused(&events),
        [CycleRefProblem::BaselineIssuerBelowEvent { event: m(2), baseline_issuer: 2, issuer: 3 }]
    );
}

#[test]
fn names_out_of_place_are_refused_and_the_decrease_is_not_judged() {
    // 재배열: 자리를 무시하고 비교하면 M1 의 1 이 M2 의 3 보다 작아 「감소」로 보인다 — 그 판정을 하지 않는다.
    let reordered = [change(2, 1, 2, 3, 2), start()];
    assert_eq!(refused(&reordered), [CycleRefProblem::NameOutOfPlace { position: 0, found: m(2), repeated: false }]);

    let gap = [start(), change(3, 1, 2, 3, 2)];
    assert_eq!(refused(&gap), [CycleRefProblem::NameOutOfPlace { position: 1, found: m(3), repeated: false }]);

    let mut again = change(2, 1, 1, 2, 2);
    again = EventCycleFacts::new(
        BaselineFacts::new(m(1), ScopeChange, again.baseline().snapshot(), again.baseline().boundary(), again.baseline().issuer(), vec![A]),
        again.chain(),
        again.boundary(),
        again.issuer(),
    );
    assert_eq!(refused(&[start(), again]), [CycleRefProblem::NameOutOfPlace { position: 1, found: m(1), repeated: true }]);

    // 순서와 무관한 사건별 조건은 그대로 본다.
    let found = refused(&[waiting(2, 1, 3, 2), start()]);
    assert!(found.contains(&CycleRefProblem::NameOutOfPlace { position: 0, found: m(2), repeated: false }));
    assert!(found.iter().any(|problem| matches!(problem, CycleRefProblem::CycleNotBeforeIssuer { .. })), "{found:#?}");
    assert!(!found.iter().any(|problem| matches!(problem, CycleRefProblem::IssuerDecreased { .. })), "{found:#?}");
}

#[test]
fn a_partial_record_refused_by_the_point_check_is_not_turned_into_success() {
    let registry = registry(9);
    let chains = world();
    // 기준점은 있는데 발급기 칸이 없다 — 3e 가 거절한다. 참조 자체는 맞다.
    let partial = EventCycleFacts::new(
        BaselineFacts::new(m(2), ScopeChange, Some(a(2)), Written(Some(c(1))), Unwritten, vec![A]),
        ch(1),
        Written(Some(c(1))),
        2,
    );
    // 기준점이 없는데 보관 경계가 적혔다 — 역시 3e 의 일이다.
    let early = EventCycleFacts::new(
        BaselineFacts::new(m(3), ScopeChange, None, Written(Some(c(2))), Unwritten, vec![F]),
        ch(1),
        Written(Some(c(2))),
        3,
    );
    let events = [start(), partial, early];
    let baselines: Vec<BaselineFacts> = events.iter().map(|facts| facts.baseline().clone()).collect();
    let point = check_baselines(&baselines, &registry).expect_err("3e 가 거절한다");
    let found = check_cycle_refs(&events, &chains, &registry).expect_err("3e 의 거절을 성공으로 덮지 않는다");
    assert_eq!(found, [CycleRefProblem::Point(point)]);
}

#[test]
fn an_ambiguous_chain_list_is_refused_without_picking_the_first_entry() {
    let events = [start(), change(2, 1, 1, 2, 2)];
    let [first, second] = <[Chain; 2]>::try_from(world()).ok().unwrap();

    let duplicated = [first.clone(), first.clone(), second.clone()];
    let found = check_cycle_refs(&events, &duplicated, &registry(9)).expect_err("같은 체인 이름이 둘이다");
    let [CycleRefProblem::AmbiguousLookup(lookup)] = found.as_slice() else { panic!("모호함만 돌려준다: {found:#?}") };
    assert_eq!(lookup[0], LookupProblem::DuplicateChain { chain: ch(1), count: 2 });
    assert_eq!(lookup.len(), 4, "C1·C2·C3 을 두 항목이 소유한다는 사실도 함께: {lookup:#?}");

    // 다른 이름의 체인이 같은 사이클을 소유하면 소속을 정할 수 없다.
    let shared = Chain::restore(ch(3), x1(), second.entry_snapshot(), Some(ch(2)), second.cycles().clone());
    let found = check_cycle_refs(&events, &[first, second, shared], &registry(9)).expect_err("사이클 소유가 겹친다");
    assert_eq!(
        found,
        [CycleRefProblem::AmbiguousLookup(
            (4..=6).map(|n| LookupProblem::CycleInSeveralChains { cycle: c(n), chains: vec![ch(2), ch(3)] }).collect()
        )]
    );
}

#[test]
fn problems_from_independent_conditions_are_all_returned() {
    let events = [
        start(),
        change(2, 1, 2, 3, 2),
        facts(3, ScopeChange, 2, Some(2), 2, Base::Fixed { snapshot: 3, boundary: Some(6), issuer: 1 }),
    ];
    let found = refused(&events);
    assert_eq!(
        found,
        [
            CycleRefProblem::IssuerDecreased { event: m(3), issuer: 2, previous: m(2), previous_issuer: 3 },
            CycleRefProblem::BaselineIssuerBelowEvent { event: m(3), baseline_issuer: 1, issuer: 2 },
            CycleRefProblem::OtherChain { event: m(3), field: CycleField::AfterCycleRef, cycle: c(2), chain: ch(2), owner: ch(1) },
            CycleRefProblem::CycleNotBeforeIssuer { event: m(3), field: CycleField::AfterCycleRef, cycle: c(2), issuer: 2 },
            CycleRefProblem::OpenCycle { event: m(3), field: CycleField::BaselineAfterCycleRef, cycle: c(6) },
            CycleRefProblem::CycleNotBeforeIssuer {
                event: m(3),
                field: CycleField::BaselineAfterCycleRef,
                cycle: c(6),
                issuer: 1
            },
        ]
    );
    for problem in &found {
        assert!(problem.to_string().contains("event:M3"), "{problem}");
    }
}

#[test]
fn the_check_changes_neither_input_nor_chains_nor_registry() {
    let events = vec![start(), change(2, 9, 6, 2, 2)];
    let chains = world();
    let registry = registry(9);
    let (events_before, chains_before, registry_before) =
        (events.clone(), format!("{chains:?}"), format!("{registry:?}"));
    let _ = check_cycle_refs(&events, &chains, &registry);
    assert_eq!(events, events_before);
    assert_eq!(format!("{chains:?}"), chains_before);
    assert_eq!(format!("{registry:?}"), registry_before);
}

// ── 한계 ───────────────────────────────────────────────────────────────────

#[test]
fn absent_references_of_other_kinds_are_neither_refused_nor_approved() {
    // 기존 프로젝트 첫 채택이나 일반 사건의 null 경계, 칸이 아예 없는 경계 — 허용 여부를 정하지 않았다. 거절하지 않고, 성공 값에
    // 「판단하지 않음」으로 남긴다.
    let null_change = facts(1, ScopeChange, 1, None, 1, Base::Waiting);
    let found = agree(&[null_change]);
    assert_eq!(
        found.not_judged(),
        [(m(1), CycleField::AfterCycleRef, NullNotJudged), (m(1), CycleField::BaselineAfterCycleRef, UnwrittenNotJudged)]
    );
    let unwritten = EventCycleFacts::new(BaselineFacts::new(m(1), ScopeChange, None, Unwritten, Unwritten, vec![]), ch(9), Unwritten, 1);
    let found = agree(&[unwritten]);
    assert_eq!(found.events()[0].after(), UnwrittenNotJudged, "칸 없음은 null 과 따로 남는다");
    assert_eq!(found.chains_not_looked_up(), [m(1)], "chain:C9 는 조회하지 않았다 — 실재를 말하지 않는다");
    assert!(CYCLE_REFS_DO_NOT_SHOW[0].contains("null") && CYCLE_REFS_DO_NOT_SHOW[1].contains("review_cycle_ref"));
}

#[test]
fn a_truncated_or_differently_extracted_list_still_passes() {
    // 원장의 끝이 잘려 건네져도, 실제 원장에 없는 사건을 지어 넣어도, 참조와 발급 번호가 맞으면 통과한다 — 완전성은 보장하지 않는다.
    let full = [start(), change(2, 1, 2, 3, 2), waiting(3, 1, 3, 4)];
    agree(&full);
    agree(&full[..2]);
    agree(&[start(), change(2, 2, 5, 6, 7)]);
    assert!(CYCLE_REFS_DO_NOT_SHOW[4].contains("빠짐없이"));
}

#[test]
fn success_says_nothing_about_approval_archiving_world_or_next_experiment() {
    // 마지막 사건이 보관 대기여도 성공한다. 그 성공은 승인·보관 실행·현재 세계·다음 실험 허용·저장 당시의 닫힘을 말하지 않는다.
    agree(&[start(), waiting(2, 2, 5, 6)]);
    let said = CYCLE_REFS_DO_NOT_SHOW.join("\n");
    for topic in ["저장 당시의 닫힘", "T8", "다음 실험", "실제 승인", "O-3", "제품 연결"] {
        assert!(said.contains(topic), "{topic}");
    }
}
