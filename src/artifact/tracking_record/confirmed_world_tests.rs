//! 범위 변경 사건의 세 값 읽기(`read_scope_change`) 시험 — **합성 사건 입력**과 **기존 Cycles 전이로 만든 체인**, 이름을 밝힌 **복원 구조**를
//! 쓴다.
//!
//! 정책 판·사건 칸·보충 칸(`review_cycle_ref`·`world_before_ref`)·시도 결과·등록부는 시험이 만든 값이다. 모니터 승인도, 실제 보관도
//! 아니다. `state.yaml`·`gil start`·파일 시스템·객체 창고를 쓰지 않는다. 기대값은 구현에서 옮기지 않고 계약에서 도출했다 — §6.1 의 확정
//! 정책 표(사건의 기준점 → 그 사건의 policy_after · Experiment 의 Exit → 그 실험을 열 때의 적용 정책 · Interview 의 Exit → Entry 의 확정
//! 정책을 물려받는다), §6.2 의 P(N)·S(N)·검증 ①·②, §6.3 의 보관 대기 세 값, §5.5 의 같은 세계, §5.7 의 T5·T8·T9, §6.5 의 사례.
//!
//! ```text
//! story(k)         기존 전이 — open_child · close_current
//!   chain:C1  cycle:C1 Interview  (뿌리)            Entry A1 · Exit A1
//!             └ cycle:C2 Experiment (success)       Entry A1 · Verify A2 → Exit A2
//!                 └ cycle:C3 Interview              Entry A2 · Exit A2
//!                     └ … cycle:C<2+k> Interview    Entry A2 · Exit A2      모두 닫혔고 마지막 사이클에 서 있다
//! restored(..)     위 체인의 상태를 기존 `Cycles::restore` 로 다시 세운 것 — 적은 칸만 바꾼다
//! ```
//!
//! `story` 는 Cycles 층의 메모리 전이다. 프로젝트 세션의 관측·보관·저장·잠금을 거치지 않는다. `restored` 는 `Cycles::restore` 의 그래프
//! 불변식을 통과한 값이지만 현행 `Project` load 의 세계 검사를 거친 값이 아니다. 어떤 시험도 실제 승인·보관·제품 load·개방 허가의 성공을
//! 검증했다고 말하지 않는다. 같은 값을 새로 만들어 다시 읽는 시험은 제품 재시작 시험이 아니다.

use super::event_cycles_tests::{a, c, ch, close_interview, m, open_interview, registry, rules, world, x1};
use super::experiment_exit_tests::{Base, Records, at, change, close_experiment, kept, p, records, records_with, restored, start};
use super::*;
use crate::artifact::SnapshotRegistry;
use crate::cycles::Cycles;
use crate::{Chain, CycleKind};

use AttemptSeen::Archived as A;
use BaselineSlot::{Unwritten, Written};
use EventKind::{ScopeChange, Start};

// ── 체인 ───────────────────────────────────────────────────────────────────

/// 위 그림의 `story(k)` 를 걷고, 같은 `Cycles` 에 `more` 를 이어 적용한 뒤 열린 체인 하나로 싼다.
fn story_then(interviews: u32, more: impl FnOnce(&mut Cycles)) -> Vec<Chain> {
    let mut cycles = Cycles::start_at(rules(), x1(), a(1), 1);
    close_interview(&mut cycles, "open_child");
    cycles.open_child(CycleKind::Experiment, x1()).expect("승인된 인터뷰 뒤 C2 를 연다");
    close_experiment(&mut cycles, 2, "success");
    for _ in 0..interviews {
        open_interview(&mut cycles);
        close_interview(&mut cycles, "open_child");
    }
    more(&mut cycles);
    vec![Chain::restore(ch(1), x1(), a(1), None, cycles)]
}

fn story(interviews: u32) -> Vec<Chain> {
    story_then(interviews, |_| {})
}

/// 사이클마다 (번호, 종류, 닫힘, 부모, Entry, Exit).
type Shape = (u32, CycleKind, bool, Option<u32>, u32, Option<u32>);

fn shape(chain: &Chain) -> Vec<Shape> {
    chain
        .cycles()
        .nodes()
        .iter()
        .map(|cycle| {
            (
                cycle.id().to_ref().number(),
                cycle.kind(),
                cycle.is_closed(),
                cycle.parent().map(|id| id.to_ref().number()),
                cycle.entry_snapshot().number(),
                cycle.exit_snapshot().map(|world| world.number()),
            )
        })
        .collect()
}

#[test]
fn the_story_is_what_the_picture_says() {
    use CycleKind::{Experiment, Interview};
    let chains = story(3);
    assert_eq!(
        shape(&chains[0]),
        [
            (1, Interview, true, None, 1, Some(1)),
            (2, Experiment, true, Some(1), 1, Some(2)),
            (3, Interview, true, Some(2), 2, Some(2)),
            (4, Interview, true, Some(3), 2, Some(2)),
            (5, Interview, true, Some(4), 2, Some(2)),
        ]
    );
    let chain = &chains[0];
    assert!(!chain.is_closed() && !chain.awaiting_report() && chain.parent().is_none());
    assert_eq!((chain.current_cycle(), chain.cycles().next_name()), (c(5), 6), "닫힌 C5 에 서 있고 다음 이름은 6 이다");
    assert_eq!(chain.cycles().pending_revisit(), None);
}

// ── 한 시점의 입력 ─────────────────────────────────────────────────────────

/// 사건마다 보충 칸 — `start` 는 둘 다 `null`, 범위 변경은 건넨 (review_cycle_ref, world_before_ref) 를 순서대로.
fn worlds_of(records: &Records, changes: &[(Option<u32>, Option<u32>)]) -> Vec<EventWorldFacts> {
    let mut given = changes.iter();
    records
        .links
        .events()
        .iter()
        .map(|link| match link.kind() {
            ScopeChange => {
                let (review, before) = given.next().expect("범위 변경 사건마다 보충 칸을 건넨다");
                EventWorldFacts::new(link.name(), ScopeChange, Written(review.map(c)), Written(before.map(a)))
            }
            kind => EventWorldFacts::new(link.name(), kind, Written(None), Written(None)),
        })
        .collect()
}

/// 한 시점에 함께 건네는 값 전부 — 시험이 만든 합성 자료다.
struct Case {
    records: Records,
    worlds: Vec<EventWorldFacts>,
    chains: Vec<Chain>,
    registry: SnapshotRegistry,
    marks: ScopeMarks,
}

impl Case {
    fn new(records: Records, changes: &[(Option<u32>, Option<u32>)], chains: Vec<Chain>) -> Case {
        let worlds = worlds_of(&records, changes);
        Case { records, worlds, chains, registry: registry(9), marks: ScopeMarks::not_provided() }
    }

    fn inputs(&self) -> RecordInputs<'_> {
        RecordInputs {
            ledger: &self.records.ledger,
            links: &self.records.links,
            facts: &self.records.facts,
            worlds: &self.worlds,
            chains: &self.chains,
            registry: &self.registry,
            marks: self.marks,
        }
    }

    fn read(&self, event: u32) -> Result<ScopeChangeReading, Vec<ReadingProblem>> {
        read_scope_change(m(event), &self.inputs())
    }

    fn reading(&self, event: u32) -> ScopeChangeReading {
        self.read(event).unwrap_or_else(|problems| panic!("읽혀야 한다: {problems:#?}"))
    }

    fn refused(&self, event: u32) -> Vec<ReadingProblem> {
        self.read(event).expect_err("읽히지 않아야 한다")
    }
}

/// §6.5 의 대기: C3(인터뷰)을 닫은 경계에서 M2(P1→P2) 를 승인 저장했고 기준점이 없다. 그 뒤 인터뷰 `later` 개가 닫혔다.
fn waiting_after(later: u32) -> Case {
    Case::new(records(&[start(), change(2, 3, 4, Base::Waiting)]), &[(Some(3), Some(2))], story(1 + later))
}

/// 지금 경계의 유도 경로.
fn trail(reading: &ScopeChangeReading) -> Vec<(&'static str, u32, u32, Option<u32>)> {
    trail_of(reading.confirmed())
}

/// (경계, 세계, 확정 정책)의 번호.
fn basis(world: &ConfirmedWorld) -> (u32, u32, u32) {
    (world.boundary().number(), world.world().number(), world.policy().number())
}

/// 유도 경로를 견주기 쉬운 꼴로 — (걸음의 종류, 사이클 또는 사건의 번호, 세계, 그 걸음이 지닌 판).
fn trail_of(world: &ConfirmedWorld) -> Vec<(&'static str, u32, u32, Option<u32>)> {
    world
        .path()
        .iter()
        .map(|step| match *step {
            PolicyStep::EventBaseline { event, world, policy, .. } => {
                ("baseline", event.number(), world.number(), Some(policy.number()))
            }
            PolicyStep::ExperimentExit { cycle, world, policy, .. } => {
                ("experiment", cycle.number(), world.number(), Some(policy.number()))
            }
            PolicyStep::InterviewInherits { cycle, world } => ("interview", cycle.number(), world.number(), None),
            PolicyStep::StartRoot { cycle, world, policy, .. } => ("start", cycle.number(), world.number(), Some(policy.number())),
        })
        .collect()
}

// ── 세 값: 적용 정책 · 마지막 확정 세계와 확정 정책 · 기준점 ─────────────────

#[test]
fn a_waiting_event_reads_as_applied_p2_confirmed_a2_under_p1_and_no_baseline() {
    let case = waiting_after(0);
    let reading = case.reading(2);

    // 적용 정책 — 원장의 마지막 판과 그 출처. 미승인 후보로 되돌리지 않는다.
    assert_eq!(reading.applied(), AppliedPolicy { policy: p(2), introduced_by: m(2), last_event: m(2) });
    assert_eq!((reading.policy_before(), reading.policy_after()), (Some(p(1)), p(2)));

    // 마지막 확정 세계 — 지금 경계 C3 의 Exit A2 이고, 확정 정책은 P2 가 아니라 P1 이다.
    let confirmed = reading.confirmed();
    assert_eq!((confirmed.boundary(), confirmed.world(), confirmed.policy()), (c(3), a(2), p(1)));
    assert!(!reading.basis_agrees(), "보관 대기에서는 확정 정책과 적용 정책이 다르다 — 손상이 아니다");

    // 유도 경로 — Interview C3 은 Entry 를 계승, Experiment C2 의 Exit 은 P(C2) = P1, 그 Entry 는 뿌리 Interview 를 거쳐 start 기준점.
    assert_eq!(
        trail(&reading),
        [("interview", 3, 2, None), ("experiment", 2, 2, Some(1)), ("interview", 1, 1, None), ("start", 1, 1, Some(1))]
    );
    assert!(matches!(
        confirmed.path()[1],
        PolicyStep::ExperimentExit { opened_under, lineage: ExpectedSource::Verify(_), .. } if opened_under == m(1)
    ));
    assert_eq!(
        confirmed.path()[3],
        PolicyStep::StartRoot { cycle: c(1), chain: ch(1), event: m(1), world: a(1), policy: p(1) }
    );

    // 사건의 기준점 — 없다. 유효한 대기다.
    assert_eq!(reading.standing(), EventStanding::Waiting { attempts: 1 });
    // world_before_ref 의 근거 — 승인 경계 C3 에서 앞선 기록(M1 뿐)으로 한 유도도 지금 경계와 같은 세계·같은 확정 정책·같은 경로다.
    let before = reading.world_before();
    assert_eq!((before.world, &before.second), (a(2), &SecondBasis::CurrentBoundary { boundary: c(3) }));
    assert_eq!(basis(&before.approval), (3, 2, 1));
    assert_eq!(trail_of(&before.approval), trail(&reading));
    assert_eq!((reading.event(), reading.chain(), reading.issuer()), (m(2), ch(1), 4));
}

#[test]
fn interviews_closed_while_waiting_keep_the_confirmed_world_and_its_policy() {
    // C4·C5 는 M2 의 승인 뒤에 연 인터뷰다. 그것들을 열 때의 적용 정책 P(N) 은 P2 지만 세계는 P1 로 확정됐다.
    let case = waiting_after(2);
    for interview in [4, 5] {
        let opened = select_opening_policy(
            c(interview),
            &case.records.ledger,
            &case.records.links,
            &case.records.facts,
            &case.chains,
            &case.registry,
        )
        .expect("P(N) 이 골라진다");
        assert!(
            matches!(opened.selection(), OpeningSelection::Applied { policy, .. } if policy == p(2)),
            "C{interview} 를 열 때의 적용 정책은 P2 다"
        );
    }

    let reading = case.reading(2);
    let confirmed = reading.confirmed();
    assert_eq!((confirmed.boundary(), confirmed.world(), confirmed.policy()), (c(5), a(2), p(1)));
    assert_eq!(
        trail(&reading),
        [
            ("interview", 5, 2, None),
            ("interview", 4, 2, None),
            ("interview", 3, 2, None),
            ("experiment", 2, 2, Some(1)),
            ("interview", 1, 1, None),
            ("start", 1, 1, Some(1)),
        ]
    );
    assert_eq!(reading.applied().policy, p(2));
    assert_eq!(reading.standing(), EventStanding::Waiting { attempts: 1 });
    assert_eq!(reading.issuer(), 6, "지금 보관한다면 baseline.next_cycle_id 는 6 이다");
    assert_eq!(reading.world_before().second, SecondBasis::CurrentBoundary { boundary: c(5) });
    assert_eq!(basis(&reading.world_before().approval), (3, 2, 1), "승인 경계 C3 의 근거도 A2·P1 이다");
}

#[test]
fn an_event_archived_at_a_later_boundary_reads_as_confirmed_with_its_own_source() {
    // §6.5 (가): 대기 중 인터뷰 C4·C5 가 닫힌 뒤 C5 에서 보관했다 — baseline = {after C5, 발급기 6, A3}.
    let case = Case::new(records(&[start(), change(2, 3, 4, kept(3, 5, 6))]), &[(Some(3), Some(2))], story(3));
    let reading = case.reading(2);
    assert_eq!(
        reading.standing(),
        EventStanding::AlreadyConfirmed { snapshot: a(3), archived_at: c(5), archive_issuer: 6, attempts: 1 }
    );
    let confirmed = reading.confirmed();
    assert_eq!((confirmed.boundary(), confirmed.world(), confirmed.policy()), (c(5), a(3), p(2)));
    assert_eq!(
        confirmed.path(),
        [PolicyStep::EventBaseline { event: m(2), kind: ScopeChange, world: a(3), policy: p(2), archived_at: c(5), archive_issuer: 6 }]
    );
    assert!(reading.basis_agrees());
    // world_before_ref 는 승인 경계 C3 과 보관 경계 C5 의 **앞선 기록**(M1 뿐)으로 유도한 세계 A2 와 같다 — M2 자신의 기준점 A3 이 아니다.
    // 지금 경계가 M2 의 기준점(P2)을 골랐어도 두 역사 경계의 근거는 같은 유도로 따로 섰고, 그 확정 정책은 P1 이다.
    let before = reading.world_before();
    assert_eq!(before.world, a(2));
    assert_eq!(basis(&before.approval), (3, 2, 1));
    assert_eq!(
        trail_of(&before.approval),
        [("interview", 3, 2, None), ("experiment", 2, 2, Some(1)), ("interview", 1, 1, None), ("start", 1, 1, Some(1))]
    );
    let SecondBasis::ArchiveBoundary { basis: archive } = &before.second else {
        panic!("이미 확정된 사건의 두 번째 근거는 보관 경계다: {:?}", before.second);
    };
    assert_eq!(basis(archive), (5, 2, 1));
    assert_eq!(
        trail_of(archive),
        [
            ("interview", 5, 2, None),
            ("interview", 4, 2, None),
            ("interview", 3, 2, None),
            ("experiment", 2, 2, Some(1)),
            ("interview", 1, 1, None),
            ("start", 1, 1, Some(1)),
        ]
    );
}

#[test]
fn a_baseline_archived_later_does_not_become_the_source_of_an_earlier_entry() {
    // M2 를 C4 에서 보관했다고 적혀 있지만 그 보관 발급기는 6 이다 — C5(번호 5)를 연 뒤의 보관이다. C5 의 Entry 출처는 여전히
    // 부모 C4 의 Exit 이고, C5 의 Exit 이 물려받는 확정 정책은 P1 이다.
    let case = Case::new(records(&[start(), change(2, 3, 4, kept(3, 4, 6))]), &[(Some(3), Some(2))], story(3));
    let reading = case.reading(2);
    assert_eq!(
        reading.standing(),
        EventStanding::AlreadyConfirmed { snapshot: a(3), archived_at: c(4), archive_issuer: 6, attempts: 1 }
    );
    let confirmed = reading.confirmed();
    assert_eq!((confirmed.boundary(), confirmed.world(), confirmed.policy()), (c(5), a(2), p(1)));
    assert!(
        trail(&reading).iter().all(|(kind, ..)| *kind != "baseline"),
        "나중에 보관한 기준점이 과거 Entry 의 근거가 됐다: {:?}",
        trail(&reading)
    );
    assert!(!reading.basis_agrees());
}

#[test]
fn the_next_cycle_after_the_baseline_takes_the_baseline_as_its_source() {
    // M2 를 C5 에서 보관한 뒤(A3, 발급기 6) 연 Experiment C6 — Entry A3, Verify A4 → Exit A4.
    let walked = story_then(3, |cycles| {
        cycles.open_child(CycleKind::Experiment, x1()).expect("C5 뒤 C6 을 연다");
        close_experiment(cycles, 4, "success");
    });
    let rows = [start(), change(2, 3, 4, kept(3, 5, 6))];

    // 기존 전이는 자식의 Entry 를 부모 Exit(A2)으로 적는다. 기준점이 있는 기록에서 그 Entry 는 검증 ① 을 어긴다.
    let stale = Case::new(records(&rows), &[(Some(3), Some(2))], walked.clone());
    assert_eq!(stale.refused(2), [ReadingProblem::EntryDiffers { cycle: c(6), entry: a(2), expected: a(3) }]);

    // Entry 가 기준점 A3 인 기록(복원 구조)에서는 Exit A4 의 확정 정책이 P(C6) = P2 이고, 그 Entry 의 출처는 M2 의 기준점이다.
    let entered = vec![restored(&walked[0], |nodes| nodes[at(6)].entry = a(3)).expect("Cycles::restore 가 받아들인다")];
    let case = Case::new(records(&rows), &[(Some(3), Some(2))], entered);
    let reading = case.reading(2);
    let confirmed = reading.confirmed();
    assert_eq!((confirmed.boundary(), confirmed.world(), confirmed.policy()), (c(6), a(4), p(2)));
    assert_eq!(trail(&reading), [("experiment", 6, 4, Some(2)), ("baseline", 2, 3, Some(2))]);
    assert!(matches!(confirmed.path()[0], PolicyStep::ExperimentExit { opened_under, .. } if opened_under == m(2)));
}

#[test]
fn the_same_world_name_keeps_separate_sources_and_policies() {
    // §5.8 「같은 세계」: P2 로 보관한 세계가 A2 그대로다. 같은 이름 A2 를 C3 의 Exit(P1 로 확정)과 M2 의 기준점(P2)이 함께 가리킨다.
    let same = Case::new(records(&[start(), change(2, 3, 4, kept(2, 3, 4))]), &[(Some(3), Some(2))], story(1));
    let archived = same.reading(2);
    let waiting = waiting_after(0).reading(2);

    assert_eq!(archived.confirmed().world(), waiting.confirmed().world(), "두 읽기의 세계 이름은 같은 A2 다");
    assert_eq!((archived.confirmed().policy(), waiting.confirmed().policy()), (p(2), p(1)));
    assert_eq!(trail(&archived), [("baseline", 2, 2, Some(2))]);
    assert_eq!(trail(&waiting)[0], ("interview", 3, 2, None));
    assert_eq!(
        archived.standing(),
        EventStanding::AlreadyConfirmed { snapshot: a(2), archived_at: c(3), archive_issuer: 4, attempts: 1 }
    );

    // 같은 원문을 다시 승인한 판도 판 이름으로 남는다 — P1 과 P2 의 원문이 같아도 확정 정책은 P1, 적용 정책은 P2 다.
    let twice = Case::new(
        records_with(&["same/\n", "same/\n"], &[start(), change(2, 3, 4, Base::Waiting)]),
        &[(Some(3), Some(2))],
        story(1),
    );
    let reading = twice.reading(2);
    assert_eq!((reading.confirmed().policy(), reading.applied().policy), (p(1), p(2)));
}

#[test]
fn a_replaced_event_is_read_as_replaced_and_the_later_event_as_waiting() {
    // §6.5 (나): M2 대기 중 인터뷰 C4 에서 정리한 새 변경안 M3(P2→P3)을 C4 에서 승인했다. M2 는 기준점 없이 대체됐다.
    let rows = [start(), change(2, 3, 4, Base::Waiting), change(3, 4, 5, Base::Waiting)];
    let case = Case::new(records(&rows), &[(Some(3), Some(2)), (Some(4), Some(2))], story(2));

    let replaced = case.reading(2);
    assert_eq!(replaced.standing(), EventStanding::Replaced { by: m(3) });
    assert_eq!(replaced.applied(), AppliedPolicy { policy: p(3), introduced_by: m(3), last_event: m(3) });
    assert_eq!((replaced.policy_before(), replaced.policy_after()), (Some(p(1)), p(2)));
    assert_eq!(replaced.world_before().second, SecondBasis::NoneForReplaced);

    let waiting = case.reading(3);
    assert_eq!(waiting.standing(), EventStanding::Waiting { attempts: 1 });
    assert_eq!(basis(&waiting.world_before().approval), (4, 2, 1), "M3 의 승인 경계 C4 의 근거 — 대체된 M2 의 판 P2 가 아니다");

    // 두 사건 모두 마지막 확정 세계는 A2, 확정 정책은 두 판 앞의 P1 이다.
    for reading in [&replaced, &waiting] {
        let confirmed = reading.confirmed();
        assert_eq!((confirmed.boundary(), confirmed.world(), confirmed.policy()), (c(4), a(2), p(1)));
    }
}

#[test]
fn reading_again_or_from_rebuilt_values_is_the_same_reading_and_changes_nothing() {
    let case = waiting_after(1);
    let before = (case.records.ledger.clone(), case.records.links.clone(), case.records.facts.clone(), case.registry.clone());
    let first = case.reading(2);
    let again = case.reading(2);
    // 같은 값을 처음부터 다시 만들어 읽는다 — 새 판정 인스턴스다. 제품 재시작 시험이 아니다.
    let rebuilt = waiting_after(1).reading(2);
    assert_eq!(first, again);
    assert_eq!(first, rebuilt);
    assert_eq!(first.standing(), EventStanding::Waiting { attempts: 1 }, "몇 번을 읽어도 시도가 생기지 않는다");
    assert_eq!(
        (case.records.ledger.clone(), case.records.links.clone(), case.records.facts.clone(), case.registry.clone()),
        before,
        "읽기가 입력을 바꿨다"
    );
}

// ── 판단하지 않은 것과 한계 ────────────────────────────────────────────────

#[test]
fn what_was_not_judged_stays_visible_in_a_successful_reading() {
    let mut case = waiting_after(0);
    let reading = case.reading(2);
    let not_judged = reading.not_judged();
    assert!(not_judged.contains(&Unjudged::ArchiveHaltNotProvided) && not_judged.contains(&Unjudged::UnfinishedRestoreNotProvided));
    // 대기 사건의 baseline.after_cycle_ref 칸은 없다 — 3h 는 그것을 판단하지 않았고 그 사실이 남는다. 다만 그것은 기준점이 없는 사건의
    // 정상 부재로 따로 적히며, 경계를 확인하지 못한 칸(CycleRefAbsent)과 섞이지 않는다.
    assert!(not_judged.contains(&Unjudged::BaselineSlotsNotWritten { event: m(2) }));
    assert!(!not_judged.iter().any(|entry| matches!(entry, Unjudged::CycleRefAbsent { .. })), "{not_judged:?}");
    // 시작 사건의 null 경계와 chain_ref 는 이 읽기가 따로 확인했으므로 판단하지 않은 목록에 없다.
    assert!(
        !not_judged.iter().any(|entry| matches!(
            entry,
            Unjudged::CycleRefAbsent { event, .. } | Unjudged::ChainNotLookedUp { event } if *event == m(1)
        )),
        "{not_judged:?}"
    );

    // 표식이 없다고 건네면 그 두 줄만 사라진다. 그 말의 진위는 보지 않는다.
    case.marks = ScopeMarks { archive_halt: MarkSeen::Absent, unfinished_restore: MarkSeen::Absent };
    let told = case.reading(2);
    assert!(!told.not_judged().contains(&Unjudged::ArchiveHaltNotProvided));
    assert!(!told.not_judged().contains(&Unjudged::UnfinishedRestoreNotProvided));

    // review_cycle_ref 가 null 이면 거절하지도 승인하지도 않고 남긴다.
    let unreviewed = Case::new(records(&[start(), change(2, 3, 4, Base::Waiting)]), &[(None, Some(2))], story(1));
    assert!(unreviewed.reading(2).not_judged().contains(&Unjudged::ReviewCycleAbsent { event: m(2), written_null: true }));
}

#[test]
fn a_jointly_truncated_history_is_read_inside_what_was_given_and_the_limit_stays_stated() {
    // 실제로는 M3 이 M2 를 대체한 기록에서 원장·사건·보충 입력을 함께 M2 까지만 잘라 건넸다. 이 읽기는 알아내지 못하고 M2 를 대기로 읽는다.
    let truncated = Case::new(records(&[start(), change(2, 3, 4, Base::Waiting)]), &[(Some(3), Some(2))], story(2));
    assert_eq!(truncated.reading(2).standing(), EventStanding::Waiting { attempts: 1 });
    assert!(CONFIRMED_WORLD_DOES_NOT_SHOW[0].contains("완전성") && CONFIRMED_WORLD_DOES_NOT_SHOW[0].contains("잘린 이력"));
    assert!(CONFIRMED_WORLD_DOES_NOT_SHOW[1].contains("실제 승인"));

    // 한쪽만 자른 입력은 거절된다 — 잘린 쪽을 성공으로 채우지 않는다.
    let full = records(&[start(), change(2, 3, 4, Base::Waiting), change(3, 4, 5, Base::Waiting)]);
    let mut lopsided = Case::new(full, &[(Some(3), Some(2)), (Some(4), Some(2))], story(2));
    lopsided.records.facts.truncate(2);
    assert!(matches!(lopsided.refused(2).as_slice(), [ReadingProblem::Inputs(_)]));
    assert_eq!(truncated.refused(3), [ReadingProblem::UnknownEvent { event: m(3), events: 2 }]);
}

// ── 범위 밖 ────────────────────────────────────────────────────────────────

fn limit(case: &Case, event: u32) -> ScopeLimit {
    match case.refused(event).as_slice() {
        [ReadingProblem::OutOfScope(limit)] => limit.clone(),
        other => panic!("범위 밖 하나여야 한다: {other:#?}"),
    }
}

#[test]
fn records_this_reading_does_not_judge_come_back_as_out_of_scope() {
    // 대상이 start 사건이다.
    assert_eq!(limit(&waiting_after(0), 1), ScopeLimit::EventKind { event: m(1), kind: Start });

    // 첫 사건이 start 가 아니다 — 기존 프로젝트의 첫 채택에서 이어진 기록.
    let adopted = Case::new(
        records(&[change(1, 1, 2, kept(1, 1, 2)), change(2, 3, 4, Base::Waiting)]),
        &[(Some(1), Some(1)), (Some(3), Some(2))],
        story(1),
    );
    assert_eq!(limit(&adopted, 2), ScopeLimit::NoStartOrigin { first: m(1), kind: ScopeChange });

    // 열린 사이클 안에 서 있다.
    let open = Case::new(
        records(&[start(), change(2, 3, 4, Base::Waiting)]),
        &[(Some(3), Some(2))],
        story_then(1, |cycles| open_interview(cycles)),
    );
    assert_eq!(limit(&open, 2), ScopeLimit::OpenCycle { chain: ch(1), cycle: c(4) });

    // 종료 인터뷰를 닫고 Chain 보고서를 기다리는 경계다.
    let closing = Case::new(
        records(&[start(), change(2, 3, 4, Base::Waiting)]),
        &[(Some(3), Some(2))],
        story_then(1, |cycles| {
            open_interview(cycles);
            close_interview(cycles, "close_chain");
        }),
    );
    assert!(closing.chains[0].awaiting_report());
    assert_eq!(limit(&closing, 2), ScopeLimit::ChainClosingBoundary { chain: ch(1) });

    // 닫힌 Chain 이다(기존 시험 세계의 chain:C1 — 인터뷰 C1·C2·C3 뒤 닫혔다).
    let closed = Case::new(records(&[start(), change(2, 1, 2, Base::Waiting)]), &[(Some(1), Some(1))], world());
    assert_eq!(limit(&closed, 2), ScopeLimit::ChainClosed { chain: ch(1) });
}

#[test]
fn a_halt_or_an_unfinished_restore_handed_in_as_present_is_out_of_scope() {
    let mut case = waiting_after(0);
    case.marks = ScopeMarks { archive_halt: MarkSeen::Present, unfinished_restore: MarkSeen::Present };
    assert_eq!(
        case.refused(2),
        [
            ReadingProblem::OutOfScope(ScopeLimit::ArchiveHaltPresent),
            ReadingProblem::OutOfScope(ScopeLimit::UnfinishedRestorePresent),
        ]
    );
}

// ── 거절: 유도 경로 ────────────────────────────────────────────────────────

#[test]
fn an_experiment_opened_while_the_event_waits_breaks_check_two() {
    // 대기 사건 뒤에 연 Experiment C4 — P(C4) = P2 인데 그 Entry 의 출처(C3 의 Exit)는 P1 로 확정됐다(§6.5 「위조 거절」).
    let chains = story_then(1, |cycles| {
        cycles.open_child(CycleKind::Experiment, x1()).expect("C3 뒤 C4 를 연다");
        close_experiment(cycles, 3, "success");
    });
    let case = Case::new(records(&[start(), change(2, 3, 4, Base::Waiting)]), &[(Some(3), Some(2))], chains);
    assert_eq!(
        case.refused(2),
        [ReadingProblem::ExperimentPolicyDiffers { cycle: c(4), opened_under: p(2), entry_confirmed: p(1) }]
    );
}

#[test]
fn an_experiment_left_behind_while_waiting_is_found_even_off_the_derivation_path() {
    // 대기 중에 연 Experiment C4 가 실패해 C3 으로 되돌아왔다. 유도 경로(C3 → C2 → C1)에는 C4 가 없지만 T9 와 되돌아온 자리가 걸린다.
    let chains = story_then(1, |cycles| {
        cycles.open_child(CycleKind::Experiment, x1()).expect("C3 뒤 C4 를 연다");
        close_experiment(cycles, 3, "failure");
        let moved = cycles.revisit().expect("적어 둔 대로 C3 으로 되돌아간다");
        assert_eq!(moved.target, c(3));
    });
    let case = Case::new(records(&[start(), change(2, 3, 4, Base::Waiting)]), &[(Some(3), Some(2))], chains);
    assert_eq!(
        case.refused(2),
        [
            ReadingProblem::ExperimentWhileWaiting { event: m(2), cycle: c(4) },
            ReadingProblem::OutOfScope(ScopeLimit::RevisitWhileWaiting { event: m(2), boundary: c(3) }),
        ]
    );
}

#[test]
fn a_stored_exit_that_breaks_its_lineage_is_refused() {
    // C2 의 저장 Exit 을 A1 로 바꾸고 뒤따르는 인터뷰의 Entry·Exit 도 그에 맞췄다. 계보의 Verify 는 A2 를 확정했다.
    let bent = restored(&story(1)[0], |nodes| {
        nodes[at(2)].exit = Some(a(1));
        nodes[at(3)].entry = a(1);
        nodes[at(3)].exit = Some(a(1));
    })
    .expect("Cycles::restore 가 받아들인다");
    let case = Case::new(records(&[start(), change(2, 3, 4, Base::Waiting)]), &[(Some(3), Some(1))], vec![bent]);
    let problems = case.refused(2);
    assert!(
        matches!(
            problems.as_slice(),
            [ReadingProblem::ExitLineageDiffers { cycle, stored, expected, source: ExpectedSource::Verify(_) }]
                if (*cycle, *stored, *expected) == (c(2), a(1), a(2))
        ),
        "{problems:#?}"
    );
}

#[test]
fn an_entry_or_an_interview_exit_that_does_not_follow_its_source_is_refused() {
    let rows = [start(), change(2, 3, 4, Base::Waiting)];
    // Interview C4 의 Entry·Exit 이 모두 A1 이다 — 부모 C3 의 Exit 은 A2 다(검증 ①).
    let entry = restored(&story(2)[0], |nodes| {
        nodes[at(4)].entry = a(1);
        nodes[at(4)].exit = Some(a(1));
    })
    .expect("Cycles::restore 가 받아들인다");
    let case = Case::new(records(&rows), &[(Some(3), Some(2))], vec![entry]);
    assert_eq!(case.refused(2), [ReadingProblem::EntryDiffers { cycle: c(4), entry: a(1), expected: a(2) }]);

    // Interview C4 의 저장 Exit 만 A3 이다 — Interview 는 세계를 확정하지 않으므로 Exit 은 Entry 여야 한다.
    let exit = restored(&story(2)[0], |nodes| nodes[at(4)].exit = Some(a(3))).expect("Cycles::restore 가 받아들인다");
    let case = Case::new(records(&rows), &[(Some(3), Some(2))], vec![exit]);
    assert_eq!(case.refused(2), [ReadingProblem::InterviewExitNotEntry { cycle: c(4), entry: a(2), exit: a(3) }]);
}

#[test]
fn a_start_baseline_that_is_not_the_chain_and_root_entry_is_refused() {
    let rows = [start(), change(2, 3, 4, Base::Waiting)];
    // Chain Entry 가 A9 다 — 시작 사건의 기준점은 A1.
    let mut cycles = Cycles::start_at(rules(), x1(), a(1), 1);
    close_interview(&mut cycles, "open_child");
    open_interview(&mut cycles);
    close_interview(&mut cycles, "open_child");
    open_interview(&mut cycles);
    close_interview(&mut cycles, "open_child");
    let other_entry = vec![Chain::restore(ch(1), x1(), a(9), None, cycles)];
    let case = Case::new(records(&rows), &[(Some(3), Some(1))], other_entry);
    assert_eq!(
        case.refused(2),
        [ReadingProblem::Start(StartProblem::NotChainEntry { event: m(1), baseline: a(1), chain_entry: a(9) })]
    );

    // 뿌리 사이클의 Entry 가 A3 이다.
    let root = restored(&story(1)[0], |nodes| nodes[at(1)].entry = a(3)).expect("Cycles::restore 가 받아들인다");
    let case = Case::new(records(&rows), &[(Some(3), Some(2))], vec![root]);
    assert_eq!(
        case.refused(2),
        [ReadingProblem::Start(StartProblem::NotRootEntry { event: m(1), baseline: a(1), root: c(1), root_entry: a(3) })]
    );
}

// ── 거절: world_before_ref · review_cycle_ref · 입력 ───────────────────────

#[test]
fn a_world_before_that_differs_from_the_derived_world_is_refused_at_both_bases() {
    // 대기 사건: 승인 경계 C3 의 앞선 기록과 지금 경계 C4 의 마지막 확정 세계는 모두 A2 인데 A1 이 적혀 있다.
    let waiting = Case::new(records(&[start(), change(2, 3, 4, Base::Waiting)]), &[(Some(3), Some(1))], story(2));
    assert_eq!(
        waiting.refused(2),
        [
            ReadingProblem::WorldBeforeDiffers { event: m(2), at: BasisAt::Approval, boundary: c(3), recorded: a(1), derived: a(2) },
            ReadingProblem::WorldBeforeDiffers {
                event: m(2),
                at: BasisAt::CurrentBoundary,
                boundary: c(4),
                recorded: a(1),
                derived: a(2),
            },
        ]
    );

    // 이미 확정된 사건에 자기 기준점 A3 을 world_before_ref 로 적었다 — 자기 기준점은 자기 보관 전 근거가 되지 못한다.
    let circular = Case::new(records(&[start(), change(2, 3, 4, kept(3, 3, 4))]), &[(Some(3), Some(3))], story(1));
    assert_eq!(
        circular.refused(2),
        [
            ReadingProblem::WorldBeforeDiffers { event: m(2), at: BasisAt::Approval, boundary: c(3), recorded: a(3), derived: a(2) },
            ReadingProblem::WorldBeforeDiffers { event: m(2), at: BasisAt::Archive, boundary: c(3), recorded: a(3), derived: a(2) },
        ]
    );
}

#[test]
fn a_missing_or_unregistered_world_before_is_refused() {
    let rows = [start(), change(2, 3, 4, Base::Waiting)];
    let null = Case::new(records(&rows), &[(Some(3), None)], story(1));
    assert_eq!(null.refused(2), [ReadingProblem::WorldBeforeMissing { event: m(2), written_null: true }]);

    let mut unwritten = waiting_after(0);
    unwritten.worlds[1] = EventWorldFacts::new(m(2), ScopeChange, Written(Some(c(3))), Unwritten);
    assert_eq!(unwritten.refused(2), [ReadingProblem::WorldBeforeMissing { event: m(2), written_null: false }]);

    let mut unknown = waiting_after(0);
    unknown.registry = registry(2);
    unknown.worlds[1] = EventWorldFacts::new(m(2), ScopeChange, Written(Some(c(3))), Written(Some(a(7))));
    let problems = unknown.refused(2);
    assert!(problems.contains(&ReadingProblem::WorldBeforeNotInRegistry { event: m(2), world: a(7) }), "{problems:#?}");
}

#[test]
fn an_earlier_baseline_recorded_as_archived_after_the_approval_cannot_be_the_prior_basis() {
    // M2 는 C3 에서 보관했다고 적혀 있지만 보관 발급기가 5 다. M3 은 같은 C3 에서 발급기 4 일 때 승인됐다 — M2 의 기준점이 M3 의 승인보다
    // 늦다고 적혀 있으므로 M3 의 승인 근거로 쓸 수 없다.
    let rows = [start(), change(2, 3, 4, kept(3, 3, 5)), change(3, 3, 4, Base::Waiting)];
    let case = Case::new(records(&rows), &[(Some(3), Some(2)), (Some(3), Some(3))], story(2));
    let problems = case.refused(3);
    assert!(
        problems.contains(&ReadingProblem::PriorBaselineNotBefore {
            event: m(3),
            at: BasisAt::Approval,
            basis: m(2),
            archive_issuer: 5,
            bound: 4,
        }),
        "{problems:#?}"
    );
}

#[test]
fn a_review_cycle_that_is_not_a_closed_interview_before_the_approval_is_refused() {
    let rows = [start(), change(2, 3, 4, Base::Waiting)];
    let experiment = Case::new(records(&rows), &[(Some(2), Some(2))], story(1));
    assert_eq!(
        experiment.refused(2),
        [ReadingProblem::ReviewCycle { event: m(2), cycle: c(2), why: ReviewProblem::NotInterview }]
    );
    // C4 는 승인(발급기 4) 뒤에 연 인터뷰다.
    let later = Case::new(records(&rows), &[(Some(4), Some(2))], story(2));
    assert_eq!(
        later.refused(2),
        [ReadingProblem::ReviewCycle { event: m(2), cycle: c(4), why: ReviewProblem::NotBeforeApproval { issuer: 4 } }]
    );
    let missing = Case::new(records(&rows), &[(Some(8), Some(2))], story(1));
    assert!(matches!(
        missing.refused(2).as_slice(),
        [ReadingProblem::ReviewCycle { why: ReviewProblem::Target(TargetProblem::NotFound { .. }), .. }]
    ));
}

#[test]
fn inputs_that_do_not_describe_one_event_list_are_refused() {
    // 보충 입력의 사건 수·이름·종류가 사건 목록과 다르다.
    let mut short = waiting_after(0);
    short.worlds.truncate(1);
    assert_eq!(
        short.refused(2),
        [ReadingProblem::WorldFacts(vec![WorldFactsMismatch::CountDiffers { events: 2, worlds: 1 }])]
    );
    let mut renamed = waiting_after(0);
    renamed.worlds[1] = EventWorldFacts::new(m(3), ScopeChange, Written(Some(c(3))), Written(Some(a(2))));
    assert_eq!(
        renamed.refused(2),
        [ReadingProblem::WorldFacts(vec![WorldFactsMismatch::NameDiffers { position: 1, event: m(2), found: m(3) }])]
    );
    let mut rekinded = waiting_after(0);
    rekinded.worlds[1] = EventWorldFacts::new(m(2), Start, Written(Some(c(3))), Written(Some(a(2))));
    assert_eq!(
        rekinded.refused(2),
        [ReadingProblem::WorldFacts(vec![WorldFactsMismatch::KindDiffers { event: m(2), kind: ScopeChange, found: Start }])]
    );

    // 원장에 M2 가 도입한 판이 없다 — 3c 의 실패 그대로.
    let ledgerless = Case::new(
        records_with(&["rule-1/\n"], &[start(), change(2, 3, 4, Base::Waiting)]),
        &[(Some(3), Some(2))],
        story(1),
    );
    assert!(matches!(
        ledgerless.refused(2).as_slice(),
        [ReadingProblem::Inputs(found)] if matches!(found.as_slice(), [EventInputsProblem::PolicyLinks(_)])
    ));

    // 기준점은 적혀 있는데 보관 경계 칸이 없는 부분 기록 — 3e 의 실패 그대로이며 「이미 확정」으로 읽지 않는다.
    let mut partial = waiting_after(0);
    partial.records.facts[1] = EventCycleFacts::new(
        BaselineFacts::new(m(2), ScopeChange, Some(a(3)), Unwritten, Written(4), vec![A]),
        ch(1),
        Written(Some(c(3))),
        4,
    );
    assert!(matches!(
        partial.refused(2).as_slice(),
        [ReadingProblem::Inputs(found)] if matches!(found.as_slice(), [EventInputsProblem::CycleRefs(_)])
    ));
}

// ── 역사 경계의 근거 ───────────────────────────────────────────────────────

#[test]
fn a_historical_boundary_whose_basis_does_not_stand_is_refused_even_when_the_current_boundary_picks_the_events_own_baseline() {
    // M2 는 C3 에서 승인하고 바로 보관했다(A3). 지금 경계 C3 의 B(T) 는 M2 자신의 기준점이라 C3 의 Exit 을 따라가지 않는다. 그러나 승인
    // 경계와 보관 경계의 앞선 근거는 C3 의 Exit 이고, 그 근거의 Experiment C2 는 저장 Exit(A1)이 계보의 Verify(A2)와 다르다. 세계 이름만
    // 보면 world_before_ref A1 = C3 의 Exit A1 이지만, 근거가 서지 않으므로 일치로 치지 않는다.
    let bent = restored(&story(1)[0], |nodes| {
        nodes[at(2)].exit = Some(a(1));
        nodes[at(3)].entry = a(1);
        nodes[at(3)].exit = Some(a(1));
    })
    .expect("Cycles::restore 가 받아들인다");
    let case = Case::new(records(&[start(), change(2, 3, 4, kept(3, 3, 4))]), &[(Some(3), Some(1))], vec![bent]);
    let problems = case.refused(2);
    let unbased = |at: BasisAt| {
        problems.iter().any(|problem| {
            matches!(
                problem,
                ReadingProblem::PriorBasis { event, at: found_at, boundary, found }
                    if (*event, *found_at, *boundary) == (m(2), at, c(3))
                        && matches!(found.as_slice(), [ReadingProblem::ExitLineageDiffers { cycle, .. }] if *cycle == c(2))
            )
        })
    };
    assert!(unbased(BasisAt::Approval) && unbased(BasisAt::Archive), "{problems:#?}");
    assert_eq!(problems.len(), 2, "{problems:#?}");
}

#[test]
fn a_historical_basis_is_derived_from_earlier_records_only_and_keeps_its_own_confirmed_policy() {
    // M2(P1→P2)는 C3 에서 승인, 대기 중 인터뷰 C4 가 닫힌 뒤 C4 에서 보관했다(A3, 발급기 5). 그 뒤 C5 가 그 기준점에서 열려 닫혔고,
    // M3(P2→P3)을 C5 에서 승인했다(발급기 6, 대기).
    let walked = restored(&story(3)[0], |nodes| {
        nodes[at(5)].entry = a(3);
        nodes[at(5)].exit = Some(a(3));
    })
    .expect("Cycles::restore 가 받아들인다");
    let rows = [start(), change(2, 3, 4, kept(3, 4, 5)), change(3, 5, 6, Base::Waiting)];
    let case = Case::new(records(&rows), &[(Some(3), Some(2)), (Some(5), Some(3))], vec![walked]);

    // M2: 지금 경계 C5 의 확정 세계는 A3(P2)지만, M2 의 승인 경계 C3·보관 경계 C4 의 근거는 M2 보다 앞의 기록으로만 유도한 A2(P1)다 —
    // M2 자신의 기준점도, 뒤의 M3 의 판 P3 도 과거의 근거가 되지 않는다.
    let second = case.reading(2);
    assert_eq!(basis(second.confirmed()), (5, 3, 2));
    assert_eq!(trail(&second), [("interview", 5, 3, None), ("baseline", 2, 3, Some(2))]);
    let before = second.world_before();
    assert_eq!(basis(&before.approval), (3, 2, 1));
    let SecondBasis::ArchiveBoundary { basis: archive } = &before.second else {
        panic!("이미 확정된 사건의 두 번째 근거는 보관 경계다: {:?}", before.second);
    };
    assert_eq!(basis(archive), (4, 2, 1));
    assert!(trail_of(archive).iter().all(|(kind, ..)| *kind != "baseline"), "{:?}", trail_of(archive));

    // M3: 승인 경계 C5 의 근거는 앞선 M2 의 기준점에서 이어진 A3(P2)이고, 적용 정책 P3 이 아니다.
    let third = case.reading(3);
    assert_eq!(third.applied().policy, p(3));
    assert_eq!(third.standing(), EventStanding::Waiting { attempts: 1 });
    assert_eq!(basis(third.confirmed()), (5, 3, 2));
    assert_eq!(basis(&third.world_before().approval), (5, 3, 2));
    assert_eq!(trail_of(&third.world_before().approval), [("interview", 5, 3, None), ("baseline", 2, 3, Some(2))]);
}

#[test]
fn the_same_world_name_under_another_confirmed_policy_is_not_one_basis() {
    // M2 는 C3 에서 승인했고 인터뷰 C4 가 닫힌 뒤 C4 에서 보관했다 — 추적 세계가 같아 이름은 A2 그대로, 확정 정책은 P2 다. M3 은 그 뒤
    // (발급기 5) C3 에서 승인됐다고 적혀 있고 world_before_ref 는 A2 다. 승인 경계 C3 의 근거는 A2(P1), 지금 경계 C4 의 근거는 A2(P2) —
    // 세계 이름은 같지만 한 비교 근거가 아니다. 이 기록은 승인 경계보다 뒤의 경계에 서 있지도 않다.
    let rows = [start(), change(2, 3, 4, kept(2, 4, 5)), change(3, 3, 5, Base::Waiting)];
    let case = Case::new(records(&rows), &[(Some(3), Some(2)), (Some(3), Some(2))], story(2));
    let problems = case.refused(3);
    assert!(
        problems.contains(&ReadingProblem::BasisPolicyDiffers {
            event: m(3),
            at: BasisAt::CurrentBoundary,
            approval: p(1),
            second: p(2),
        }),
        "{problems:#?}"
    );
    assert!(
        !problems.iter().any(|problem| matches!(problem, ReadingProblem::WorldBeforeDiffers { .. })),
        "세계 이름은 두 근거 모두 A2 다: {problems:#?}"
    );
}
