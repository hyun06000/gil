//! 비뿌리 사이클 N 의 세계 출처 S(N) 선택과 검증 ①(§6.2) 시험 — **합성 사건 입력**과 **기존 전이로 만든 체인**, 그리고 이름을 밝힌
//! **복원 구조**를 쓴다.
//!
//! 정책 판·사건 칸·시도 결과·등록부는 시험이 만든 값이다. 승인은 시험 자료의 `approved: yes` 이며 모니터 승인이 아니다. `state.yaml`·
//! `gil start`·파일 시스템을 쓰지 않는다.
//!
//! 기대값은 구현에서 옮기지 않고 계약에서 도출했다 — §6.2 의 정의(「N 이전의 기준점 = `baseline.next_cycle_id ≤ N` 인 보관 완료 기준점」,
//! 「S(N) = N 이전의 기준점 중 T 에서 보관한 것 가운데 사건 이름이 가장 큰 것, 없으면 T 의 Exit」, 「검증 ① N.entry_snapshot_ref = S(N) 의
//! 세계」, 「구조적 부모와 세계 출처가 갈린다」), §6.5 의 사례(보관 대기와 인터뷰 (가), 되돌아가기의 M4, 위조 거절), §6.1 의 「사건의
//! 기준점 → 그 사건의 `policy_after`」, §5.1 의 판 이름 규칙, §5.5 의 같은 세계, Artifact Model §7.1 의 parent·`revisit_from` 구분.
//!
//! 체인 세 가지:
//!
//! ```text
//! revisit_tree()   Cycles 층의 실제 전이 — open_child · close_current · revisit · open_after_revisit
//!   chain:C1  cycle:C1 Interview   (닫힘·open_child)     Entry A1 · Exit A1
//!             └ cycle:C2 Experiment (닫힘·success)       Entry A1 · Verify A2 → Exit A2
//!                 ├ cycle:C3 Experiment (닫힘·failure → revisit cycle:C2)   Entry A2 · Verify A3 → Exit A3
//!                 └ cycle:C4 Experiment (열림)  parent C2 · revisit_from C3   Entry A2
//! line(n)          Interview 만의 직선 — cycle:C1 ─▶ … ─▶ cycle:C<n>, C<n> 만 열림, 모든 Entry·Exit A1
//! restored(..)     위 체인 하나의 상태를 기존 `Cycles::restore` 로 다시 세운 것 — 한 칸만 바꾼다
//! ```
//!
//! `revisit_tree` 는 Cycles 층의 실제 되돌아가기 전이로 만든 **메모리 구조**다. 프로젝트 세션의 revisit 실행(작업물 복원·저장·잠금)을
//! 검증하지 않는다. `restored` 는 `Cycles::restore` 의 그래프 불변식을 통과한 값이지만 현행 `Project` load 의 세계 검사(일반 자식의
//! Entry = 부모 Exit, 닫힌 사이클의 Exit 실재)는 거칠 수 없을 수 있다 — 그런 시험은 그 사실을 적는다. 어떤 시험도 실제 Experiment 의
//! 확정·관문 ②·제품 load 성공을 검증했다고 말하지 않는다.

use super::event_cycles_tests::{a, c, ch, close_interview, m, open_interview, registry, rules, world, x1};
use super::*;
use crate::artifact::gilignore::EngineVersion;
use crate::artifact::{SnapshotRegistry, digest_from_raw, digest_of};
use crate::cycle::CycleState;
use crate::cycles::{Cycles, CyclesState};
use crate::refs::{JourneyRef, SnapshotRef};
use crate::walk::{RestoreError, WalkState};
use crate::{Chain, Cycle, CycleKind, NodeKind, NodeStatus};

use AttemptSeen::{Archived as A, NotArchived as F};
use BaselineSlot::{Unwritten, Written};
use EntryVerdict::{Differs, Matches};
use EventKind::{RestoreResult, ScopeChange, Start};
use WorldSource::{EventBaseline, ParentExit};

#[path = "../../../tests/common/mod.rs"]
mod common;

// ── 체인 ───────────────────────────────────────────────────────────────────

/// 지금 서 있는 열린 Experiment 를 Define → Hypothesis → Verify(세계 `A<verify>` 확정) → Analysis → Outcome 으로 걷고 닫는다.
/// `success` 는 `open_child`, `failure` 는 부모로의 `revisit` 을 적는다(공통 도움 함수 `cycle_report` 가 부모를 대상으로 고른다).
fn close_experiment(cycles: &mut Cycles, verify: u32, verdict: &str) {
    let report = {
        let cycle = cycles.current_mut();
        for kind in [NodeKind::Define, NodeKind::Hypothesis, NodeKind::Verify, NodeKind::Analysis] {
            cycle.open_step(kind).unwrap_or_else(|err| panic!("{kind} 를 열지 못했다: {err}"));
            let report = common::full_report(cycle.rules(), CycleKind::Experiment, kind);
            match kind == NodeKind::Verify {
                true => cycle.close_verify_step(report, a(verify)),
                false => cycle.close_step(report),
            }
            .unwrap_or_else(|err| panic!("{kind} 를 닫지 못했다: {err}"));
        }
        cycle.open_step(NodeKind::Outcome).expect("Outcome 을 연다");
        let outcome = cycle.steps().current().expect("Outcome 에 서 있다");
        let report = common::full_report(cycle.rules(), CycleKind::Experiment, NodeKind::Outcome)
            .with("verdict", verdict)
            .with("lesson", "이 갈래가 남긴 것 — 시험 자료")
            .with(common::ACTION, "close_cycle")
            .with(common::REASON, "판정을 Cycle 경계로 넘긴다");
        cycle.close_step(report).expect("Outcome 을 닫는다");
        let mut report = common::cycle_report(cycle, verdict, outcome);
        match verdict {
            "success" => {
                report.insert(common::ACTION, "open_child");
            }
            _ => assert_eq!(report.get(common::ACTION), Some("revisit"), "실패는 부모로 되돌아간다고 적는다"),
        }
        report
    };
    cycles.close_current(report, JourneyRef::new(x1(), 0)).expect("Experiment 를 닫는다");
}

/// 위 그림의 `revisit_tree` — 모두 기존 전이 함수로 걸어 만든다.
fn revisit_tree() -> Vec<Chain> {
    let mut cycles = Cycles::start_at(rules(), x1(), a(1), 1);
    close_interview(&mut cycles, "open_child");
    cycles.open_child(CycleKind::Experiment, x1()).expect("승인된 인터뷰 뒤 C2 를 연다");
    close_experiment(&mut cycles, 2, "success");
    cycles.open_child(CycleKind::Experiment, x1()).expect("성공한 C2 뒤 C3 을 연다");
    close_experiment(&mut cycles, 3, "failure");
    let moved = cycles.revisit().expect("적어 둔 대로 C2 로 되돌아간다");
    assert_eq!((moved.target, moved.target_world), (c(2), a(2)));
    cycles.open_after_revisit(CycleKind::Experiment, x1()).expect("되돌아간 C2 에서 새 갈래를 연다");
    vec![Chain::restore(ch(1), x1(), a(1), None, cycles)]
}

/// 열린 체인 하나에 Interview `count` 개 — 마지막만 열려 있고, 모든 Entry·Exit 은 A1 이다.
fn line(count: u32) -> Vec<Chain> {
    let mut cycles = Cycles::start_at(rules(), x1(), a(1), 1);
    for _ in 1..count {
        close_interview(&mut cycles, "open_child");
        open_interview(&mut cycles);
    }
    vec![Chain::restore(ch(1), x1(), a(1), None, cycles)]
}

/// 사이클 하나를 기존 복원 입구의 상태로 옮긴다 — 접근자로 읽은 값 그대로이고, 한 칸만 채운다.
///
/// `Project` 의 단계 닫기는 닫은 Journey 판을 `Walk::record_journey` 로 적는다. 이 시험의 Cycles 층 메모리 전이는 그 칸을 비워 두므로,
/// 복원 입구(`Walk::restore`)가 닫힌 단계에 요구하는 그 칸에만 사이클을 닫을 때 쓴 것과 같은 합성 판 `X1` 의 0번 판을 적는다.
fn state_of(cycle: &Cycle) -> CycleState {
    let walk = cycle.steps();
    let mut nodes = walk.nodes().to_vec();
    for node in nodes.iter_mut().filter(|node| node.is_closed() && node.journey.is_none()) {
        node.journey = Some(JourneyRef::new(x1(), 0));
    }
    CycleState {
        id: cycle.id(),
        parent: cycle.parent(),
        revisit_from: cycle.revisit_from(),
        kind: cycle.kind(),
        status: cycle.status(),
        existence: cycle.existence(),
        journey: cycle.journey(),
        report: cycle.report().cloned(),
        entry: cycle.entry_snapshot(),
        exit: cycle.exit_snapshot(),
        steps: WalkState {
            nodes,
            current: walk.current(),
            next_id: walk.next_id(),
            pending_revisit: walk.pending_revisit(),
        },
    }
}

/// 체인의 사이클 상태를 `edit` 로 바꾼 뒤 기존 [`Cycles::restore`] 로 다시 세운다. 그 그래프 불변식이 거절하면 그 오류 그대로다.
fn restored(chain: &Chain, edit: impl FnOnce(&mut [CycleState])) -> Result<Chain, RestoreError> {
    let cycles = chain.cycles();
    let mut nodes: Vec<CycleState> = cycles.nodes().iter().map(state_of).collect();
    edit(&mut nodes);
    let state = CyclesState {
        next_id: cycles.next_name(),
        current: cycles.current_id(),
        pending_revisit: cycles.pending_revisit(),
        nodes,
    };
    Cycles::restore(rules(), state)
        .map(|cycles| Chain::restore(chain.id(), chain.existence(), chain.entry_snapshot(), chain.parent(), cycles))
}

fn at(number: u32) -> usize {
    usize::try_from(number - 1).unwrap()
}

/// 사이클마다 (번호, 종류, 닫힘, 부모, revisit_from, Entry, Exit).
type Shape = (u32, CycleKind, bool, Option<u32>, Option<u32>, u32, Option<u32>);

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
                cycle.revisit_from().map(|id| id.to_ref().number()),
                cycle.entry_snapshot().number(),
                cycle.exit_snapshot().map(SnapshotRef::number),
            )
        })
        .collect()
}

#[test]
fn the_fixtures_are_what_the_picture_says() {
    use CycleKind::{Experiment, Interview};
    let tree = revisit_tree();
    let wanted: Vec<Shape> = vec![
        (1, Interview, true, None, None, 1, Some(1)),
        (2, Experiment, true, Some(1), None, 1, Some(2)),
        (3, Experiment, true, Some(2), None, 2, Some(3)),
        (4, Experiment, false, Some(2), Some(3), 2, None),
    ];
    assert_eq!(shape(&tree[0]), wanted);
    assert_eq!(tree[0].cycles().pending_revisit(), None, "되돌아간 결정은 C4 를 열며 소비됐다");
    // 바꾸지 않고 다시 세우면 같은 구조다 — 복원 도움 함수가 아무것도 지어내지 않는다.
    assert_eq!(shape(&restored(&tree[0], |_| {}).expect("그대로 복원된다")), wanted);
    let line = line(4);
    assert_eq!(
        shape(&line[0]),
        (1..=4).map(|n| (n, Interview, n != 4, (n > 1).then(|| n - 1), None, 1, (n != 4).then_some(1))).collect::<Vec<Shape>>()
    );
}

// ── 사건: 한 행에서 두 부분 입력을 함께 뽑는다 ─────────────────────────────

fn p(number: u32) -> PolicyName {
    PolicyName::new(number).expect("0 이 아니다")
}

/// 판마다 원문 글자를 받는다 — 같은 글자를 두 번 주면 같은 원문의 다른 판이 된다(§5.1).
fn ledger_of(texts: &[&str]) -> PolicyLedger {
    let records = texts
        .iter()
        .enumerate()
        .map(|(at, text)| {
            let n = u32::try_from(at).unwrap() + 1;
            let rules = digest_of(text.as_bytes()).expect("메모리의 바이트는 언제나 읽힌다");
            let proposal = ProposalDigest(digest_from_raw([u8::try_from(n).unwrap(); 32]));
            PolicyRecord::new(p(n), EngineVersion::V1, rules, proposal)
        })
        .collect::<Vec<_>>();
    PolicyLedger::from_records(u32::try_from(texts.len()).unwrap() + 1, records).expect("시험 원장은 꼴이 맞다")
}

#[derive(Clone, Copy)]
enum Base {
    /// 기준점 없음, 경계·발급기 칸 없음, 실패 시도 하나 — 보관 대기이거나 뒤 사건으로 대체됐다.
    Waiting,
    /// 기준점·`baseline` 경계(`None` = `null`)·`baseline.next_cycle_id` 를 함께 적고 `archived` 하나.
    Fixed { snapshot: u32, boundary: Option<u32>, issuer: u32 },
}

/// 사건 원장의 한 기록 — 3c 입력과 3h 입력이 이 한 행에서 함께 나온다.
#[derive(Clone, Copy)]
struct Row {
    kind: EventKind,
    before: Option<u32>,
    after: u32,
    /// 승인 경계 `after_cycle_ref` (`None` = `null`).
    at: Option<u32>,
    /// 승인 당시의 `next_cycle_id`.
    issuer: u32,
    base: Base,
}

/// §5.1 예시의 `start` — `chain:C1`, 경계 `null`, `next_cycle_id: 1`, 기준점 A1(보관 경계 `null`, 발급기 1).
fn start() -> Row {
    Row { kind: Start, before: None, after: 1, at: None, issuer: 1, base: Base::Fixed { snapshot: 1, boundary: None, issuer: 1 } }
}

/// `P<after - 1> → P<after>` 범위 변경 — 승인 경계 `at`, 승인 당시 발급기 `issuer`.
fn change(after: u32, at: u32, issuer: u32, base: Base) -> Row {
    Row { kind: ScopeChange, before: Some(after - 1), after, at: Some(at), issuer, base }
}

fn restore(policy: u32, at: u32, issuer: u32, base: Base) -> Row {
    Row { kind: RestoreResult, before: Some(policy), after: policy, at: Some(at), issuer, base }
}

/// 경계 `boundary` 에서 발급기 `issuer` 일 때 보관한 기준점 `A<snapshot>`.
fn kept(snapshot: u32, boundary: u32, issuer: u32) -> Base {
    Base::Fixed { snapshot, boundary: Some(boundary), issuer }
}

/// 한 기록 목록과 그것이 도입한 판의 원장 — 셋은 같은 한 시점에서 뽑은 값이다.
struct Records {
    ledger: PolicyLedger,
    links: EventPolicyLinks,
    facts: Vec<EventCycleFacts>,
}

fn records_with(texts: &[&str], rows: &[Row]) -> Records {
    let links = rows
        .iter()
        .enumerate()
        .map(|(at, row)| EventPolicyLink::new(m(u32::try_from(at).unwrap() + 1), row.kind, row.before.map(p), Some(p(row.after))))
        .collect::<Vec<_>>();
    let facts = rows
        .iter()
        .enumerate()
        .map(|(at, row)| {
            let number = u32::try_from(at).unwrap() + 1;
            let baseline = match row.base {
                Base::Waiting => BaselineFacts::new(m(number), row.kind, None, Unwritten, Unwritten, vec![F]),
                Base::Fixed { snapshot, boundary, issuer } => BaselineFacts::new(
                    m(number),
                    row.kind,
                    Some(a(snapshot)),
                    Written(boundary.map(c)),
                    Written(issuer),
                    vec![A],
                ),
            };
            EventCycleFacts::new(baseline, ch(1), Written(row.at.map(c)), row.issuer)
        })
        .collect();
    Records {
        ledger: ledger_of(texts),
        links: EventPolicyLinks::from_links(u32::try_from(rows.len()).unwrap() + 1, links).expect("시험 사건 이름은 꼴이 맞다"),
        facts,
    }
}

/// 판마다 다른 원문 — 판 수는 판을 도입한 사건(`restore_result` 밖) 수다.
fn records(rows: &[Row]) -> Records {
    let introduced = rows.iter().filter(|row| row.kind != RestoreResult).count();
    let texts: Vec<String> = (1..=introduced).map(|n| format!("rule-{n}/\n")).collect();
    records_with(&texts.iter().map(String::as_str).collect::<Vec<_>>(), rows)
}

fn no_records() -> Records {
    Records { ledger: PolicyLedger::empty(), links: EventPolicyLinks::empty(), facts: Vec::new() }
}

fn select_with(
    target: u32,
    records: &Records,
    chains: &[Chain],
    registry: &SnapshotRegistry,
) -> Result<OpeningWorld, Vec<OpeningWorldProblem>> {
    select_opening_world(c(target), &records.ledger, &records.links, &records.facts, chains, registry)
}

fn selected(target: u32, records: &Records, chains: &[Chain]) -> OpeningWorld {
    select_with(target, records, chains, &registry(12)).unwrap_or_else(|problems| panic!("골라야 한다: {problems:#?}"))
}

fn refused(target: u32, records: &Records, chains: &[Chain]) -> Vec<OpeningWorldProblem> {
    select_with(target, records, chains, &registry(12)).expect_err("거절해야 한다")
}

/// 기대값 — 사건 번호·종류·기준점 번호·판 번호·보관 경계 번호·보관 발급기.
fn baseline(event: u32, kind: EventKind, world: u32, policy: u32, archived_at: u32, archive_issuer: u32) -> WorldSource {
    EventBaseline { event: m(event), kind, world: a(world), policy: p(policy), archived_at: c(archived_at), archive_issuer }
}

fn exit(world: u32) -> WorldSource {
    ParentExit { world: a(world) }
}

/// 고른 (부모, 출처, 실제 Entry, 판정).
fn opened(target: u32, records: &Records, chains: &[Chain]) -> (u32, WorldSource, u32, EntryVerdict) {
    let world = selected(target, records, chains);
    let check = world.check_entry();
    (world.parent().number(), world.source(), world.entry().number(), check.verdict())
}

// ── T 의 Exit ──────────────────────────────────────────────────────────────

#[test]
fn without_events_the_parent_exit_is_the_world_for_open_and_closed_targets() {
    // §6.2 「사건이 없으면 S(N) 은 언제나 T 의 Exit」 — 열린 C4·닫힌 C3·닫힌 C2. N 자신의 Exit(C3 의 A3)도 뿌리의 Entry 도 아니다.
    let tree = revisit_tree();
    assert_eq!(opened(4, &no_records(), &tree), (2, exit(2), 2, Matches));
    assert_eq!(opened(3, &no_records(), &tree), (2, exit(2), 2, Matches));
    assert_eq!(opened(2, &no_records(), &tree), (1, exit(1), 1, Matches));
    let world = selected(4, &no_records(), &tree);
    assert_eq!((world.target(), world.chain(), world.expected()), (c(4), ch(1), a(2)));
    assert_eq!(world.policy(), WorldSourcePolicy::ExitPolicyNotDerived, "Exit 의 확정 정책은 유도하지 않는다");
}

#[test]
fn events_archived_elsewhere_or_without_a_boundary_leave_the_parent_exit() {
    // start 의 기준점은 보관 경계가 null 이라 어느 T 의 후보도 아니다. C1 에서 보관한 M2 는 C2 의 출처일 뿐 C3·C4 의 출처가 아니다.
    let tree = revisit_tree();
    assert_eq!(opened(3, &records(&[start()]), &tree), (2, exit(2), 2, Matches));
    let at_first = records(&[start(), change(2, 1, 2, kept(5, 1, 2))]);
    assert_eq!(opened(4, &at_first, &tree), (2, exit(2), 2, Matches));
    assert_eq!(opened(3, &at_first, &tree), (2, exit(2), 2, Matches));
    // C2 는 실제 open_child 로 열려 Entry 가 C1 의 Exit A1 이다. M2 가 C2 이전에 C1 에서 보관됐으므로 ① 위반이다(§6.5 위조 거절의 꼴).
    assert_eq!(opened(2, &at_first, &tree), (1, baseline(2, ScopeChange, 5, 2, 1, 2), 1, Differs));
}

// ── 보관 발급기: 승인 시점이 아니라 보관 시점 ─────────────────────────────

#[test]
fn the_archive_issuer_decides_which_children_of_the_parent_see_the_baseline() {
    // C2 의 두 자식 C3(3)·C4(4). C2 에서 승인(발급기 3)하고 C2 에서 보관한 기준점의 보관 발급기만 바꾼다.
    let tree = revisit_tree();
    let at_issuer = |issuer| records(&[start(), change(2, 2, 3, kept(5, 2, issuer))]);
    // 보관 발급기 3 — C3 과 같고(≤) C4 보다 작다: 둘 다의 출처.
    assert_eq!(opened(3, &at_issuer(3), &tree), (2, baseline(2, ScopeChange, 5, 2, 2, 3), 2, Differs));
    assert_eq!(opened(4, &at_issuer(3), &tree), (2, baseline(2, ScopeChange, 5, 2, 2, 3), 2, Differs));
    // 보관 발급기 4 — C3 보다 크다: C3 이 열린 뒤 보관했다. 승인 발급기 3 은 C3 과 같지만 C3 의 출처가 아니다.
    assert_eq!(opened(3, &at_issuer(4), &tree), (2, exit(2), 2, Matches));
    assert_eq!(opened(4, &at_issuer(4), &tree), (2, baseline(2, ScopeChange, 5, 2, 2, 4), 2, Differs));
    // 보관 발급기 5 — 둘 다 이미 열렸다.
    assert_eq!(opened(3, &at_issuer(5), &tree), (2, exit(2), 2, Matches));
    assert_eq!(opened(4, &at_issuer(5), &tree), (2, exit(2), 2, Matches));
}

#[test]
fn approval_and_archive_at_different_boundaries_follow_the_archive_boundary() {
    // §6.5 (가): C4 에 승인(발급기 5, 보관 대기 중 인터뷰 C5), C5 에서 보관(발급기 6, A6). C6 = C5 의 자식.
    let chains = line(6);
    let rows = [start(), change(2, 4, 5, kept(6, 5, 6))];
    let records = records(&rows);
    // C5 의 부모는 C4 — M2 의 승인 경계는 C4 지만 보관 경계가 아니므로 출처가 아니다. 실제 Entry A1 = C4 의 Exit.
    assert_eq!(opened(5, &records, &chains), (4, exit(1), 1, Matches));
    // C6 의 부모는 C5 — 보관 경계 C5, 보관 발급기 6 ≤ 6. 실제 전이의 Entry 는 C5 의 Exit A1 이라 ① 이 맞지 않는다.
    assert_eq!(opened(6, &records, &chains), (5, baseline(2, ScopeChange, 6, 2, 5, 6), 1, Differs));
}

// ── 후보 가운데 이름이 가장 큰 것 ──────────────────────────────────────────

#[test]
fn the_largest_candidate_wins_and_later_records_keep_the_past_world_of_n() {
    let tree = revisit_tree();
    // M2·M3 모두 C2 에서 발급기 3 으로 승인·보관했다(같은 경계의 연속 사건) — 이름이 큰 M3.
    let mut rows = vec![start(), change(2, 2, 3, kept(5, 2, 3)), change(3, 2, 3, kept(6, 2, 3))];
    let first = records(&rows);
    assert_eq!(selected(3, &first, &tree).source(), baseline(3, ScopeChange, 6, 3, 2, 3));
    assert_eq!(selected(4, &first, &tree).source(), baseline(3, ScopeChange, 6, 3, 2, 3));

    // C2 로 되돌아간 뒤(발급기 4) C2 에서 승인·보관한 M4 를 덧붙인다 — C4 의 출처는 바뀌고, 이미 열린 C3 의 출처는 그대로다.
    rows.push(change(4, 2, 4, kept(7, 2, 4)));
    let second = records(&rows);
    assert_eq!(selected(3, &second, &tree).source(), baseline(3, ScopeChange, 6, 3, 2, 3), "늦은 보관이 C3 의 과거를 바꿨다");
    assert_eq!(selected(4, &second, &tree).source(), baseline(4, ScopeChange, 7, 4, 2, 4));

    // 더 큰 이름이라도 다른 경계(C3)에서 보관한 M5, 보관 대기인 M6 은 C2 의 앞선 후보를 가리지 않는다.
    rows.push(change(5, 3, 4, kept(8, 3, 4)));
    rows.push(change(6, 1, 4, Base::Waiting));
    let third = records(&rows);
    assert_eq!(selected(3, &third, &tree).source(), baseline(3, ScopeChange, 6, 3, 2, 3));
    assert_eq!(selected(4, &third, &tree).source(), baseline(4, ScopeChange, 7, 4, 2, 4));
}

#[test]
fn a_restore_result_archived_at_the_parent_is_its_own_source() {
    // §6.5 M4 의 꼴: P2 를 도입한 M2 뒤에 같은 판의 restore_result M3 이 C2 에서 보관됐다(발급기 4). 출처는 M3 자신이다.
    let tree = revisit_tree();
    let records = records(&[start(), change(2, 2, 3, kept(5, 2, 3)), restore(2, 2, 4, kept(6, 2, 4))]);
    let world = selected(4, &records, &tree);
    assert_eq!(world.source(), baseline(3, RestoreResult, 6, 2, 2, 4));
    assert_eq!(world.policy(), WorldSourcePolicy::PolicyAfter { event: m(3), policy: p(2) }, "판을 도입한 M2 로 바꾸지 않는다");
    // C3 은 M3 의 보관(발급기 4)보다 먼저 열렸다 — M2.
    assert_eq!(selected(3, &records, &tree).source(), baseline(2, ScopeChange, 5, 2, 2, 3));
}

#[test]
fn the_same_world_under_another_board_of_the_same_text_is_its_own_source() {
    // P3 은 P1 과 같은 원문을 다시 승인한 판이다. M3 의 기준점 A2 는 C2 의 Exit 과 같은 이름이다 — 그래도 출처는 M3·P3 이다(§5.1·§5.5).
    let tree = revisit_tree();
    let rows = [start(), change(2, 1, 2, kept(1, 1, 2)), change(3, 2, 3, kept(2, 2, 3))];
    let records = records_with(&["same/\n", "other/\n", "same/\n"], &rows);
    let world = selected(3, &records, &tree);
    assert_eq!(world.source(), baseline(3, ScopeChange, 2, 3, 2, 3));
    assert_ne!(world.source(), exit(2), "같은 세계 이름이라도 Exit 출처로 합치지 않는다");
    assert_eq!(world.policy(), WorldSourcePolicy::PolicyAfter { event: m(3), policy: p(3) }, "같은 원문의 P1 로 합치지 않는다");
    // 실제 전이의 Entry A2 와 같은 이름이므로 ① 이 맞는다 — 사건 기준점 갈래의 일치.
    assert_eq!(world.check_entry().verdict(), Matches);
}

// ── 검증 ①: 두 갈래 각각의 일치·불일치 ────────────────────────────────────

#[test]
fn an_entry_equal_to_an_event_baseline_unlike_the_parent_exit_matches() {
    // 복원 구조: C3 의 Entry 를 A5 로 적은 상태를 기존 Cycles::restore 가 받아들인다(일반 자식의 Entry 는 그 층이 보지 않는다).
    // 현행 Project load 의 ③(Entry = 부모 Exit)은 이 구조를 거절한다 — 사건 기준점과 Entry 를 잇는 제품 연결은 구현 대기다.
    let tree = revisit_tree();
    let chains = vec![restored(&tree[0], |nodes| nodes[at(3)].entry = a(5)).expect("Cycles::restore 가 받아들인다")];
    let records = records(&[start(), change(2, 2, 3, kept(5, 2, 3))]);
    let world = selected(3, &records, &chains);
    let check = world.check_entry();
    assert_eq!((world.parent(), world.source()), (c(2), baseline(2, ScopeChange, 5, 2, 2, 3)));
    assert_eq!((check.verdict(), check.entry(), check.expected()), (Matches, a(5), a(5)));
    assert_ne!(chains[0].cycles().nodes()[at(2)].exit_snapshot(), Some(a(5)), "부모 Exit 은 A2 그대로다");
}

#[test]
fn an_entry_unlike_the_parent_exit_without_a_candidate_differs() {
    // 같은 복원 구조에 사건이 없으면 S(C3) 은 C2 의 Exit A2 이고, 적힌 Entry A5 는 ① 을 어긴다.
    let tree = revisit_tree();
    let chains = vec![restored(&tree[0], |nodes| nodes[at(3)].entry = a(5)).expect("Cycles::restore 가 받아들인다")];
    let world = selected(3, &no_records(), &chains);
    let check = world.check_entry();
    assert_eq!((check.verdict(), check.entry(), check.expected()), (Differs, a(5), a(2)));
    assert_eq!(world.source(), exit(2));
}

#[test]
fn a_mismatch_keeps_n_t_entry_expected_source_and_the_unjudged_parts() {
    // 실제 전이(C3 의 Entry = C2 의 Exit A2)와 C2 에서 보관한 기준점 A5 — 불일치에서도 모든 값을 읽을 수 있다.
    let tree = revisit_tree();
    let records = records(&[start(), change(2, 2, 3, kept(5, 2, 3))]);
    let world = selected(3, &records, &tree);
    let check = world.check_entry();
    assert_eq!(check.verdict(), Differs);
    let opening = check.opening();
    assert!(std::ptr::eq(opening, &world), "판정은 같은 선택 값을 가리킨다");
    assert_eq!(
        (opening.target(), opening.parent(), check.entry(), check.expected(), opening.source()),
        (c(3), c(2), a(2), a(5), baseline(2, ScopeChange, 5, 2, 2, 3))
    );
    assert!(opening.inputs().cycle_refs().not_judged().contains(&(m(1), CycleField::AfterCycleRef, RefReading::NullNotJudged)));
    assert_eq!(opening.inputs().cycle_refs().chains_not_looked_up(), [m(1)]);
}

// ── 구조적 부모 ────────────────────────────────────────────────────────────

#[test]
fn a_branch_opened_after_a_revisit_follows_its_structural_parent_not_revisit_from() {
    // C4 는 Cycles 층의 실제 revisit·open_after_revisit 로 열렸다: parent C2, revisit_from C3. 프로젝트 세션의 revisit 실행이 아니다.
    let tree = revisit_tree();
    // 버린 실패 C3 에서 승인·보관한 M2(발급기 4 ≤ 4)는 C4 의 출처가 아니다 — C3 을 부모로 읽었다면 A6 이 나왔다.
    let at_failure = records(&[start(), change(2, 3, 4, kept(6, 3, 4))]);
    assert_eq!(opened(4, &at_failure, &tree), (2, exit(2), 2, Matches));
    // 구조적 부모 C2 에서 보관한 M3 은 출처다.
    let at_parent = records(&[start(), change(2, 3, 4, kept(6, 3, 4)), change(3, 2, 4, kept(7, 2, 4))]);
    assert_eq!(opened(4, &at_parent, &tree), (2, baseline(3, ScopeChange, 7, 3, 2, 4), 2, Differs));
    // 사건이 없을 때도 C3 의 Exit A3 이 아니라 C2 의 Exit A2 다.
    assert_eq!(selected(4, &no_records(), &tree).source(), exit(2));
}

// ── 대상·뿌리·부모 오류: Entry 불일치나 후보 없음으로 바꾸지 않는다 ──────────

fn only_inputs(problems: &[OpeningWorldProblem]) -> &[EventInputsProblem] {
    match problems {
        [OpeningWorldProblem::Inputs(found)] => found,
        other => panic!("입력 갈래 하나만 와야 한다: {other:#?}"),
    }
}

fn ambiguous_lookup(problem: &OpeningWorldProblem) -> bool {
    matches!(problem, OpeningWorldProblem::Inputs(found)
        if matches!(found.as_slice(), [EventInputsProblem::CycleRefs(refs)]
            if refs.iter().any(|problem| matches!(problem, CycleRefProblem::AmbiguousLookup(_)))))
}

#[test]
fn a_missing_target_is_refused() {
    let not_found = |cycle| [OpeningWorldProblem::Target(OpeningTargetProblem::NotFound { cycle: c(cycle) })];
    assert_eq!(refused(9, &no_records(), &revisit_tree()), not_found(9));
    assert_eq!(refused(9, &records(&[start()]), &revisit_tree()), not_found(9));
    assert_eq!(refused(1, &no_records(), &[]), not_found(1));
}

#[test]
fn a_root_is_out_of_scope_not_success_or_before_adoption() {
    // 뿌리 C1 — start 사건이 있어도 없어도 범위 밖이다. 후속 Chain 의 첫 사이클(world() 의 C4)도 같다(§6.2 뿌리, §6.6).
    let root = |cycle, chain| [OpeningWorldProblem::RootOutOfScope { cycle: c(cycle), chain: ch(chain) }];
    assert_eq!(refused(1, &no_records(), &revisit_tree()), root(1, 1));
    assert_eq!(refused(1, &records(&[start()]), &revisit_tree()), root(1, 1));
    assert_eq!(refused(4, &no_records(), &world()), root(4, 2));
}

#[test]
fn a_target_owned_by_two_chain_entries_is_ambiguous_and_3j_is_reported_too() {
    let mut chains = revisit_tree();
    chains.push(chains[0].clone());
    let problems = refused(4, &no_records(), &chains);
    assert_eq!(problems.len(), 2, "{problems:#?}");
    assert!(ambiguous_lookup(&problems[0]), "{problems:#?}");
    assert_eq!(
        problems[1],
        OpeningWorldProblem::Target(OpeningTargetProblem::SeveralOwners { cycle: c(4), chains: vec![ch(1), ch(1)] })
    );
}

#[test]
fn a_target_whose_chain_name_is_repeated_is_ambiguous() {
    // chain:C1 이라는 이름의 다른 항목이 C7 을 지닌다. C4 를 소유한 항목은 하나지만 그 이름의 기록이 둘이다.
    let mut chains = revisit_tree();
    chains.push(Chain::restore(ch(1), x1(), a(1), None, Cycles::start_at(rules(), x1(), a(1), 7)));
    let problems = refused(4, &no_records(), &chains);
    assert_eq!(problems.len(), 2, "{problems:#?}");
    assert!(ambiguous_lookup(&problems[0]), "{problems:#?}");
    assert_eq!(
        problems[1],
        OpeningWorldProblem::Target(OpeningTargetProblem::OwnerChainRepeated { cycle: c(4), chain: ch(1), count: 2 })
    );
}

#[test]
fn a_parent_name_owned_by_another_chain_is_not_borrowed() {
    // chain:C2 는 C2 라는 이름의 열린 뿌리를 지닌다. C3 은 chain:C1 만 소유하지만 그 부모 이름 C2 를 두 체인이 소유한다 — 어느 쪽도
    // 고르지 않는다.
    let mut chains = line(3);
    chains.push(Chain::restore(ch(2), x1(), a(1), None, Cycles::start_at(rules(), x1(), a(1), 2)));
    let problems = refused(3, &no_records(), &chains);
    assert_eq!(problems.len(), 2, "{problems:#?}");
    assert!(ambiguous_lookup(&problems[0]), "{problems:#?}");
    assert_eq!(
        problems[1],
        OpeningWorldProblem::Parent(ParentProblem::SeveralOwners { cycle: c(3), parent: c(2), chains: vec![ch(1), ch(2)] })
    );
}

#[test]
fn a_closed_parent_without_an_exit_is_refused_without_a_substitute() {
    // 복원 구조: C2 의 Exit 칸을 비운 상태를 Cycles::restore 가 받아들인다. 현행 Project load 의 ②(닫힌 사이클의 Exit 실재)는 거절한다.
    let chains = vec![restored(&line(3)[0], |nodes| nodes[at(2)].exit = None).expect("Cycles::restore 가 받아들인다")];
    assert_eq!(
        refused(3, &no_records(), &chains),
        [OpeningWorldProblem::Parent(ParentProblem::ClosedWithoutExit { cycle: c(3), parent: c(2), chain: ch(1) })]
    );
}

#[test]
fn the_defensive_parent_branches_stay_unreachable_through_the_existing_restore() {
    // NotInOwnerChain·NotClosed 는 방어 갈래다 — 그 입력을 만드는 길을 기존 복원이 막는다는 것만 확인하고, 거절 성공으로 세지 않는다.
    let chain = &line(3)[0];
    let missing = restored(chain, |nodes| nodes[at(3)].parent = Some(crate::cycles::CycleId::from(c(9))));
    assert!(matches!(missing, Err(RestoreError::CycleParentNotEarlier { .. })), "{missing:?}");
    let reopened = restored(chain, |nodes| {
        let parent = &mut nodes[at(2)];
        (parent.status, parent.report, parent.exit) = (NodeStatus::Open, None, None);
    });
    assert!(reopened.is_err(), "열린 부모를 지닌 자식을 복원했다");
}

// ── 입력 검사(3j) 와 등록부 ────────────────────────────────────────────────

#[test]
fn input_problems_are_reported_with_location_problems_and_never_as_a_verdict() {
    let mut cut = records(&[start(), change(2, 2, 3, kept(5, 2, 3))]);
    cut.facts.truncate(1);
    // 대상이 정상이면 입력 갈래 하나뿐이다.
    let problems = refused(3, &cut, &revisit_tree());
    assert!(matches!(only_inputs(&problems), [EventInputsProblem::Correspondence(_)]), "{problems:#?}");
    // 뿌리·없는 대상과 함께 오면 둘 다 순서대로 온다.
    let problems = refused(1, &cut, &revisit_tree());
    assert_eq!(problems.len(), 2, "{problems:#?}");
    assert!(matches!(&problems[0], OpeningWorldProblem::Inputs(_)));
    assert_eq!(problems[1], OpeningWorldProblem::RootOutOfScope { cycle: c(1), chain: ch(1) });
    let problems = refused(9, &cut, &revisit_tree());
    assert_eq!(problems[1], OpeningWorldProblem::Target(OpeningTargetProblem::NotFound { cycle: c(9) }));
}

#[test]
fn a_baseline_missing_from_the_registry_is_refused_by_3j() {
    let records = records(&[start(), change(2, 2, 3, kept(5, 2, 3))]);
    let problems = select_with(3, &records, &revisit_tree(), &registry(4)).expect_err("3e 가 거절한다");
    match only_inputs(&problems) {
        [EventInputsProblem::CycleRefs(found)] => {
            assert!(found.iter().any(|problem| matches!(problem, CycleRefProblem::Point(_))), "{found:#?}")
        }
        other => panic!("{other:#?}"),
    }
}

#[test]
fn a_chosen_parent_exit_missing_from_the_registry_is_refused() {
    // 등록부는 A1 만 안다. S(C3) 은 C2 의 Exit A2 인데 조회되지 않는다.
    assert_eq!(
        select_with(3, &no_records(), &revisit_tree(), &registry(1)).expect_err("거절해야 한다"),
        [OpeningWorldProblem::ParentExitNotInRegistry { cycle: c(3), parent: c(2), world: a(2) }]
    );
}

#[test]
fn an_unchosen_parent_exit_is_not_looked_up() {
    // 같은 등록부(A1 만)에서 C2 에서 보관한 기준점 A1 이 출처이면 C2 의 Exit A2 의 등록 여부를 보지 않는다.
    let records = records(&[start(), change(2, 2, 3, kept(1, 2, 3))]);
    let world = select_with(3, &records, &revisit_tree(), &registry(1)).expect("고른다");
    assert_eq!(world.source(), baseline(2, ScopeChange, 1, 2, 2, 3));
    assert_eq!(world.check_entry().verdict(), Differs, "실제 Entry A2 는 기준점 A1 과 다르다");
}

#[test]
fn the_3j_result_is_kept_as_it_was_computed_in_the_same_call() {
    let tree = revisit_tree();
    let records = records(&[start(), change(2, 2, 3, kept(5, 2, 3))]);
    let agreed = check_event_inputs(&records.ledger, &records.links, &records.facts, &tree, &registry(12)).expect("3j 가 받아들인다");
    let world = selected(3, &records, &tree);
    assert_eq!(world.inputs(), &agreed);
    assert_eq!(
        world.inputs().cycle_refs().not_judged(),
        [(m(1), CycleField::AfterCycleRef, RefReading::NullNotJudged), (m(1), CycleField::BaselineAfterCycleRef, RefReading::NullNotJudged)]
    );
    assert_eq!(world.inputs().cycle_refs().chains_not_looked_up(), [m(1)]);
}

#[test]
fn inputs_cut_together_are_not_detected_and_are_chosen_within() {
    // 원장·정책 연결·사이클 입력을 함께 잘라낸 입력은 3j 가 알아내지 못한다 — 잘린 이력 안에서 고르며 완전하다고 주장하지 않는다.
    let tree = revisit_tree();
    let rows = [start(), change(2, 2, 3, kept(5, 2, 3)), change(3, 2, 3, kept(6, 2, 3))];
    assert_eq!(selected(3, &records(&rows), &tree).source(), baseline(3, ScopeChange, 6, 3, 2, 3));
    assert_eq!(selected(3, &records(&rows[..2]), &tree).source(), baseline(2, ScopeChange, 5, 2, 2, 3));
}

// ── 순수성과 한계 ──────────────────────────────────────────────────────────

#[test]
fn inputs_are_left_unchanged_and_the_answer_is_repeatable() {
    let tree = revisit_tree();
    let records = records(&[start(), change(2, 2, 3, kept(5, 2, 3)), change(3, 3, 4, kept(6, 3, 4))]);
    let registry = registry(12);
    let snapshot = || format!("{:?}{:?}{:?}{:?}{:?}", records.ledger, records.links, records.facts, tree, registry);
    let before = snapshot();
    let first = select_with(4, &records, &tree, &registry);
    let second = select_with(4, &records, &tree, &registry);
    assert_eq!(snapshot(), before, "입력을 바꿨다");
    assert_eq!(first, second);
    let first = first.expect("고른다");
    assert_eq!(first.check_entry(), first.check_entry());
    assert_eq!(first.source(), baseline(2, ScopeChange, 5, 2, 2, 3));
}

#[test]
fn every_problem_reads_and_the_list_names_what_it_does_not_show() {
    let problems = [
        OpeningWorldProblem::Inputs(Vec::new()),
        OpeningWorldProblem::Target(OpeningTargetProblem::NotFound { cycle: c(9) }),
        OpeningWorldProblem::Target(OpeningTargetProblem::SeveralOwners { cycle: c(4), chains: vec![ch(1), ch(1)] }),
        OpeningWorldProblem::Target(OpeningTargetProblem::OwnerChainRepeated { cycle: c(4), chain: ch(1), count: 2 }),
        OpeningWorldProblem::RootOutOfScope { cycle: c(1), chain: ch(1) },
        OpeningWorldProblem::Parent(ParentProblem::NotInOwnerChain { cycle: c(3), parent: c(2), chain: ch(1) }),
        OpeningWorldProblem::Parent(ParentProblem::SeveralOwners { cycle: c(3), parent: c(2), chains: vec![ch(1), ch(2)] }),
        OpeningWorldProblem::Parent(ParentProblem::NotClosed { cycle: c(3), parent: c(2), chain: ch(1) }),
        OpeningWorldProblem::Parent(ParentProblem::ClosedWithoutExit { cycle: c(3), parent: c(2), chain: ch(1) }),
        OpeningWorldProblem::ParentExitNotInRegistry { cycle: c(3), parent: c(2), world: a(2) },
    ];
    assert!(problems.iter().all(|problem| !problem.to_string().is_empty()));
    for needle in ["함께 잘라낸", "not_judged", "chains_not_looked_up", "검증 ②", "T9", "ExitPolicyNotDerived", "뿌리", "T8", "LedgerStart", "O-2", "O-3"] {
        assert!(OPENING_WORLD_DOES_NOT_SHOW.iter().any(|line| line.contains(needle)), "{needle}");
    }
}
