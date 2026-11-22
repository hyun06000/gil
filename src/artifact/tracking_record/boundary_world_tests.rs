//! 닫힌 사이클 T 의 경계 세계 B(T) 출처 선택(§6.1) 시험 — **합성 입력**과 **실제 전이로 만든 체인**을 쓴다.
//!
//! 정책 판·사건 칸·시도 결과·등록부는 시험이 만든 값이다. 체인은 3h 시험의 도움 함수(기존 전이 함수로 걸어 만든다)로 만든다. 승인은
//! 시험 자료의 `approved: yes` 이며 모니터 승인이 아니다. `state.yaml`·`gil start`·파일 시스템을 쓰지 않는다.
//!
//! 기대값은 구현에서 옮기지 않고 계약에서 도출했다 — §6.1 의 정의(「기준점을 T 에서 보관한(`baseline.after_cycle_ref = T`) 사건 가운데
//! 이름이 가장 큰 것의 기준점, 그런 사건이 없으면 T 의 `exit_snapshot_ref`」, 「사건의 기준점 → 그 사건의 `policy_after`」), §6.5 의 두
//! 사례 표(되돌아가기의 「B(C4) = A6」과 M4 의 C4 보관, 보관 대기와 인터뷰 (가)·(나)), §5.1 의 판 이름 규칙, §5.5 의 같은 세계, §6.3 의
//! 보관 대기.
//!
//! 세계 두 가지:
//!
//! ```text
//! world()          (3h·3j 시험과 같다)
//!   chain:C1 (닫힘)  cycle:C1 ─▶ cycle:C2 ─▶ cycle:C3          모두 닫힘
//!   chain:C2 (열림)  cycle:C4 ─▶ cycle:C5 ─▶ cycle:C6          C4·C5 닫힘, C6 열림
//! line(n) = line_from(A1, n)
//!   chain:C1 (열림)  cycle:C1 ─▶ … ─▶ cycle:C<n>               C<n> 만 열림
//! ```
//!
//! 모든 사이클은 기존 도움 함수가 만든 Interview 이므로 닫힌 사이클의 Exit 은 뿌리의 Entry 와 같다(`line` 은 A1, `line_from(A20, n)` 은
//! A20). B(T) 의 정의는 대상의 종류·계보를 쓰지 않으므로 선택의 기대값은 그대로다. §6.5 의 되돌아가기 계보(C6·C9 이 C4 의 자식)는
//! 직선 체인으로 옮겼다 — 실제 Experiment 세계 확정·revisit 전이의 검증이 아니다.

use super::event_cycles_tests::{a, c, ch, close_interview, m, open_interview, registry, rules, world, x1};
use super::*;
use crate::artifact::gilignore::EngineVersion;
use crate::artifact::{SnapshotRegistry, digest_from_raw, digest_of};
use crate::cycles::Cycles;
use crate::refs::SnapshotRef;
use crate::Chain;

use AttemptSeen::{Archived as A, NotArchived as F};
use BaselineSlot::{Unwritten, Written};
use BoundaryPolicy::{ExitPolicyNotDerived, PolicyAfter};
use BoundarySource::{EventBaseline, ExitOfTarget};
use EventKind::{RestoreResult, ScopeChange, Start};

// ── 체인 ───────────────────────────────────────────────────────────────────

/// 열린 체인 하나에 `count` 개의 사이클 — 마지막만 열려 있고, 닫힌 사이클의 Exit 은 모두 `entry` 다.
fn line_from(entry: SnapshotRef, count: u32) -> Vec<Chain> {
    let mut cycles = Cycles::start_at(rules(), x1(), entry, 1);
    for _ in 1..count {
        close_interview(&mut cycles, "open_child");
        open_interview(&mut cycles);
    }
    vec![Chain::restore(ch(1), x1(), entry, None, cycles)]
}

fn line(count: u32) -> Vec<Chain> {
    line_from(a(1), count)
}

#[test]
fn the_line_fixture_is_what_the_picture_says() {
    let chains = line_from(a(20), 5);
    let seen: Vec<(u32, bool, Option<SnapshotRef>)> = chains[0]
        .cycles()
        .nodes()
        .iter()
        .map(|cycle| (cycle.id().to_ref().number(), cycle.is_closed(), cycle.exit_snapshot()))
        .collect();
    let wanted: Vec<(u32, bool, Option<SnapshotRef>)> =
        (1..=5).map(|n| (n, n != 5, (n != 5).then_some(a(20)))).collect();
    assert_eq!(seen, wanted);
    assert!(!chains[0].is_closed());
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

/// 서로 다른 원문의 판 `count` 개.
fn ledger(count: usize) -> PolicyLedger {
    let texts: Vec<String> = (1..=count).map(|n| format!("rule-{n}/\n")).collect();
    ledger_of(&texts.iter().map(String::as_str).collect::<Vec<_>>())
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
    chain: u32,
    /// 승인 경계 `after_cycle_ref` (`None` = `null`).
    at: Option<u32>,
    /// 승인 당시의 `next_cycle_id`.
    issuer: u32,
    base: Base,
}

/// §5.1 예시의 `start` — `chain:C1`, 경계 `null`, `next_cycle_id: 1`, 기준점 A1(보관 경계 `null`, 발급기 1).
fn start() -> Row {
    Row { kind: Start, before: None, after: 1, chain: 1, at: None, issuer: 1, base: Base::Fixed { snapshot: 1, boundary: None, issuer: 1 } }
}

fn change(before: Option<u32>, after: u32, chain: u32, at: u32, issuer: u32, base: Base) -> Row {
    Row { kind: ScopeChange, before, after, chain, at: Some(at), issuer, base }
}

fn restore(policy: u32, chain: u32, at: u32, issuer: u32, base: Base) -> Row {
    Row { kind: RestoreResult, before: Some(policy), after: policy, chain, at: Some(at), issuer, base }
}

/// 승인 경계에서 바로 보관한 기준점.
fn kept(snapshot: u32, at: u32, issuer: u32) -> Base {
    Base::Fixed { snapshot, boundary: Some(at), issuer }
}

fn links_of(rows: &[Row]) -> EventPolicyLinks {
    let events = rows
        .iter()
        .enumerate()
        .map(|(at, row)| EventPolicyLink::new(m(u32::try_from(at).unwrap() + 1), row.kind, row.before.map(p), Some(p(row.after))))
        .collect::<Vec<_>>();
    EventPolicyLinks::from_links(u32::try_from(rows.len()).unwrap() + 1, events).expect("시험 사건 이름은 꼴이 맞다")
}

fn fact_of(number: u32, row: &Row) -> EventCycleFacts {
    let baseline = match row.base {
        Base::Waiting => BaselineFacts::new(m(number), row.kind, None, Unwritten, Unwritten, vec![F]),
        Base::Fixed { snapshot, boundary, issuer } => {
            BaselineFacts::new(m(number), row.kind, Some(a(snapshot)), Written(boundary.map(c)), Written(issuer), vec![A])
        }
    };
    EventCycleFacts::new(baseline, ch(row.chain), Written(row.at.map(c)), row.issuer)
}

fn facts_of(rows: &[Row]) -> Vec<EventCycleFacts> {
    rows.iter().enumerate().map(|(at, row)| fact_of(u32::try_from(at).unwrap() + 1, row)).collect()
}

/// 한 기록 목록과 그것이 도입한 판 수만큼의 원장.
struct Records {
    ledger: PolicyLedger,
    links: EventPolicyLinks,
    facts: Vec<EventCycleFacts>,
}

fn records(rows: &[Row]) -> Records {
    let introduced = rows.iter().filter(|row| row.kind != RestoreResult).count();
    Records { ledger: ledger(introduced), links: links_of(rows), facts: facts_of(rows) }
}

fn no_records() -> Records {
    Records { ledger: PolicyLedger::empty(), links: EventPolicyLinks::empty(), facts: Vec::new() }
}

fn select_with(
    target: u32,
    records: &Records,
    chains: &[Chain],
    registry: &SnapshotRegistry,
) -> Result<BoundaryWorld, Vec<BoundaryWorldProblem>> {
    select_boundary_world(c(target), &records.ledger, &records.links, &records.facts, chains, registry)
}

fn select(target: u32, records: &Records, chains: &[Chain]) -> Result<BoundaryWorld, Vec<BoundaryWorldProblem>> {
    select_with(target, records, chains, &registry(12))
}

fn chosen(target: u32, records: &Records, chains: &[Chain]) -> BoundarySource {
    select(target, records, chains).unwrap_or_else(|problems| panic!("골라야 한다: {problems:#?}")).source()
}

fn refused(target: u32, records: &Records, chains: &[Chain]) -> Vec<BoundaryWorldProblem> {
    select(target, records, chains).expect_err("거절해야 한다")
}

/// 기대값 — 사건 번호·종류·기준점 번호·판 번호·보관 경계 번호·보관 발급기.
fn baseline(event: u32, kind: EventKind, world: u32, policy: u32, archived_at: u32, archive_issuer: u32) -> BoundarySource {
    EventBaseline { event: m(event), kind, world: a(world), policy: p(policy), archived_at: c(archived_at), archive_issuer }
}

fn exit(world: u32) -> BoundarySource {
    ExitOfTarget { world: a(world) }
}

// ── 사례 A: §6.5 「되돌아가기」 ─────────────────────────────────────────────
//
// | 사건 | 종류 | 정책 | 승인 경계·next_cycle_id | 기준점 (보관 경계·발급기) |
// |---|---|---|---|---|
// | M1 | start | 없음 → P1 | `null`·1 | A1 (`null`·1) |
// | M2 | scope_change | P1 → P2 | C4·5 | A6 (C4·5) |
// | M3 | scope_change | P2 → P3 | C7·8 | A9 (C7·8) |
// | M4 | restore_result | P3 → P3 | C4·9 | A10 (C4·9) |
//
// §6.5: 「C8 실패, C4로 되돌아가려 함 B(C4) = A6」은 M4 가 생기기 전의 기록이고, M4 뒤의 C4 경계는 M4 의 기준점 A10(P3) 이다.

fn revisit_rows() -> Vec<Row> {
    vec![
        start(),
        change(Some(1), 2, 1, 4, 5, kept(6, 4, 5)),
        change(Some(2), 3, 1, 7, 8, kept(9, 7, 8)),
        restore(3, 1, 4, 9, kept(10, 4, 9)),
    ]
}

#[test]
fn the_revisit_example_of_6_5_gives_each_boundary_its_world() {
    let records = records(&revisit_rows());
    let chains = line(9);
    let seen: Vec<(u32, BoundarySource)> = (1..=8).map(|n| (n, chosen(n, &records, &chains))).collect();
    assert_eq!(
        seen,
        [
            // start 의 보관 경계는 null 이다 — 어느 T 도 아니므로 C1 은 Exit.
            (1, exit(1)),
            (2, exit(1)),
            (3, exit(1)),
            // C4 에서 보관한 M2(A6) 와 M4(A10) 가운데 이름이 큰 M4. restore_result 도 사건의 기준점이고 판은 그 policy_after P3.
            (4, baseline(4, RestoreResult, 10, 3, 4, 9)),
            (5, exit(1)),
            (6, exit(1)),
            (7, baseline(3, ScopeChange, 9, 3, 7, 8)),
            (8, exit(1)),
        ]
    );
}

#[test]
fn before_the_restore_result_was_recorded_the_c4_boundary_is_a6() {
    // §6.5 「B(C4) = A6은 P2의 세계」 — M4 가 아직 없는 기록. 지금 건넨 기록에서의 선택이다.
    let rows = revisit_rows();
    let earlier = Records { ledger: ledger(3), links: links_of(&rows[..3]), facts: facts_of(&rows[..3]) };
    let chains = line(9);
    assert_eq!(chosen(4, &earlier, &chains), baseline(2, ScopeChange, 6, 2, 4, 5));
    let found = select(4, &earlier, &chains).expect("골라야 한다");
    assert_eq!(found.policy(), PolicyAfter { event: m(2), policy: p(2) });
}

#[test]
fn a_restore_result_keeps_its_own_event_as_the_source() {
    // M4 는 새 판을 도입하지 않는다. 같은 판 P3 을 도입한 M3 으로 출처를 바꾸지 않는다.
    let records = records(&revisit_rows());
    let found = select(4, &records, &line(9)).expect("골라야 한다");
    assert_eq!(found.policy(), PolicyAfter { event: m(4), policy: p(3) });
    assert!(matches!(found.source(), EventBaseline { kind: RestoreResult, .. }));
    assert_eq!(found.world(), a(10));
}

// ── 사례 B: §6.5 (가) — 승인 경계 C4, 보관 경계 C5 ─────────────────────────
//
// | M1 | start | 없음 → P1 | `null`·1 | A1 |
// | M2 | scope_change | P1 → P2 | C4·5 | A6, **C5·6** (대기 중 인터뷰 C5 가 닫힌 뒤 보관) |

fn interview_while_waiting_rows() -> Vec<Row> {
    vec![start(), change(Some(1), 2, 1, 4, 5, Base::Fixed { snapshot: 6, boundary: Some(5), issuer: 6 })]
}

#[test]
fn the_archive_boundary_not_the_approval_boundary_decides() {
    let records = records(&interview_while_waiting_rows());
    let chains = line(6);
    // 승인 경계 C4 에는 보관하지 않았다 — C4 의 Exit 이다.
    assert_eq!(chosen(4, &records, &chains), exit(1), "승인 경계 after_cycle_ref 를 보관 경계로 쓰지 않는다");
    // 보관 경계 C5 의 B 가 M2 의 기준점이며, 보관 경계·발급기가 그대로 남는다(승인 발급기 5 가 아니다).
    assert_eq!(chosen(5, &records, &chains), baseline(2, ScopeChange, 6, 2, 5, 6));
}

// ── 사례 C: §6.5 (나) — 기준점 없이 대체된 사건은 후보가 아니다 ───────────────
//
// | M1 | start | 없음 → P1 | `null`·1 | A1 |
// | M2 | scope_change | P1 → P2 | C4·5 | 없음(실패 하나) — 뒤 M3 으로 대체 |
// | M3 | scope_change | P2 → P3 | C5·6 | A7 (C5·6) |

#[test]
fn a_replaced_event_without_a_baseline_is_not_a_candidate() {
    let rows = [start(), change(Some(1), 2, 1, 4, 5, Base::Waiting), change(Some(2), 3, 1, 5, 6, kept(7, 5, 6))];
    let records = records(&rows);
    let chains = line(6);
    assert_eq!(chosen(4, &records, &chains), exit(1));
    assert_eq!(chosen(5, &records, &chains), baseline(3, ScopeChange, 7, 3, 5, 6));
}

// ── 사례 D: 보관 대기인 뒤 사건이 같은 경계의 보관 기준점을 가리지 않는다 ─────

#[test]
fn a_later_waiting_event_does_not_hide_an_archived_baseline() {
    // C4 에서 M2 를 승인·보관(A6)한 뒤 같은 C4 에서 M3 을 승인했고 아직 보관하지 못했다(§6.3 의 보관 대기). 마지막 확정 세계는 A6 다.
    let rows = [start(), change(Some(1), 2, 1, 4, 5, kept(6, 4, 5)), change(Some(2), 3, 1, 4, 5, Base::Waiting)];
    let records = records(&rows);
    let found = select(4, &records, &line(6)).expect("골라야 한다");
    assert_eq!(found.source(), baseline(2, ScopeChange, 6, 2, 4, 5), "이름이 더 큰 M3 은 기준점이 없다");
    assert_eq!(found.policy(), PolicyAfter { event: m(2), policy: p(2) }, "적용 정책 P3 을 확정 정책으로 대입하지 않는다");
    assert_eq!(records.ledger.policies().last().map(PolicyRecord::name), Some(p(3)), "전제: 지금의 적용 정책은 P3");
}

#[test]
fn a_later_event_archived_elsewhere_does_not_take_this_boundary() {
    // 같은 C4 에서 승인한 M3 이 대기 중 인터뷰 C5 뒤에 보관됐다 — C4 의 B 는 여전히 M2, C5 의 B 는 M3.
    let rows = [
        start(),
        change(Some(1), 2, 1, 4, 5, kept(6, 4, 5)),
        change(Some(2), 3, 1, 4, 5, Base::Fixed { snapshot: 7, boundary: Some(5), issuer: 6 }),
    ];
    let records = records(&rows);
    let chains = line(6);
    assert_eq!(chosen(4, &records, &chains), baseline(2, ScopeChange, 6, 2, 4, 5));
    assert_eq!(chosen(5, &records, &chains), baseline(3, ScopeChange, 7, 3, 5, 6));
}

// ── 사례 E: 같은 경계의 여러 기준점 — 이름이 가장 큰 사건 ─────────────────

#[test]
fn among_baselines_archived_at_the_same_boundary_the_largest_name_wins() {
    // C2 경계에서 사이클을 열지 않고 세 번 승인·보관했다(발급 3).
    let rows = [
        start(),
        change(Some(1), 2, 1, 2, 3, kept(2, 2, 3)),
        change(Some(2), 3, 1, 2, 3, kept(3, 2, 3)),
        change(Some(3), 4, 1, 2, 3, kept(4, 2, 3)),
    ];
    let records = records(&rows);
    let chains = line(4);
    assert_eq!(chosen(2, &records, &chains), baseline(4, ScopeChange, 4, 4, 2, 3));
    // 앞의 두 사건을 지운 것이 아니다 — 함께 잘린 기록에서는 그 앞 사건이 골라진다.
    let cut = Records { ledger: ledger(3), links: links_of(&rows[..3]), facts: facts_of(&rows[..3]) };
    assert_eq!(chosen(2, &cut, &chains), baseline(3, ScopeChange, 3, 3, 2, 3));
}

// ── 사례 F: 다른 경계의 기준점은 쓰지 않는다 ────────────────────────────────

#[test]
fn baselines_archived_at_other_boundaries_are_excluded() {
    let rows = [start(), change(Some(1), 2, 1, 2, 3, kept(2, 2, 3)), change(Some(2), 3, 1, 4, 5, kept(4, 4, 5))];
    let records = records(&rows);
    let chains = line(6);
    let seen: Vec<(u32, BoundarySource)> = (1..=5).map(|n| (n, chosen(n, &records, &chains))).collect();
    assert_eq!(
        seen,
        [
            (1, exit(1)),
            (2, baseline(2, ScopeChange, 2, 2, 2, 3)),
            // C3 는 M2 의 보관 경계 뒤지만 C3 에서 보관한 사건이 없다 — 앞 경계의 기준점을 끌어오지 않는다.
            (3, exit(1)),
            (4, baseline(3, ScopeChange, 4, 3, 4, 5)),
            (5, exit(1)),
        ]
    );
}

#[test]
fn the_archive_issuer_does_not_filter_the_choice() {
    // 보관 발급기 8 은 T(C2) 보다 훨씬 크다. S(N) 의 「N 이전의 기준점」 같은 필터가 없으므로 그대로 고른다.
    let rows = [start(), change(Some(1), 2, 1, 2, 3, Base::Fixed { snapshot: 2, boundary: Some(2), issuer: 8 })];
    let records = records(&rows);
    assert_eq!(chosen(2, &records, &line(9)), baseline(2, ScopeChange, 2, 2, 2, 8));
}

// ── 사례 G: 같은 세계 이름, 같은 원문 — 합치지 않는다 ─────────────────────────

#[test]
fn an_event_baseline_with_the_exit_world_stays_an_event_source() {
    // §5.5 같은 세계: M2 의 기준점이 C2 의 Exit 과 같은 A1 이다. 출처는 M2(P2)이며 Exit 이 아니다.
    let rows = [start(), change(Some(1), 2, 1, 2, 3, kept(1, 2, 3))];
    let records = records(&rows);
    let chains = line(4);
    assert_eq!(chains[0].cycles().nodes()[1].exit_snapshot(), Some(a(1)), "전제: C2 의 Exit 은 A1");
    let at_two = select(2, &records, &chains).expect("골라야 한다");
    let at_three = select(3, &records, &chains).expect("골라야 한다");
    assert_eq!(at_two.world(), at_three.world(), "같은 세계 이름");
    assert_eq!(at_two.source(), baseline(2, ScopeChange, 1, 2, 2, 3));
    assert_eq!(at_two.policy(), PolicyAfter { event: m(2), policy: p(2) });
    assert_eq!(at_three.source(), exit(1));
    assert_eq!(at_three.policy(), ExitPolicyNotDerived);
}

#[test]
fn two_boards_archived_with_the_same_world_are_not_merged() {
    // M2(P2)·M3(P3) 이 같은 C2 에서 같은 A2 를 보관했다. 이름이 큰 M3 과 그 판 P3 이다.
    let rows = [start(), change(Some(1), 2, 1, 2, 3, kept(2, 2, 3)), change(Some(2), 3, 1, 2, 3, kept(2, 2, 3))];
    let records = records(&rows);
    let found = select(2, &records, &line(4)).expect("골라야 한다");
    assert_eq!(found.source(), baseline(3, ScopeChange, 2, 3, 2, 3));
    assert_ne!(found.policy(), PolicyAfter { event: m(2), policy: p(2) });
}

#[test]
fn boards_with_the_same_source_stay_distinct() {
    // §5.1: P1 과 P3 은 원문 주소가 같지만 다른 판이다.
    let rows = [start(), change(Some(1), 2, 1, 2, 3, kept(2, 2, 3)), change(Some(2), 3, 1, 4, 5, kept(3, 4, 5))];
    let records = Records { ledger: ledger_of(&["keep/\n", "drop/\n", "keep/\n"]), links: links_of(&rows), facts: facts_of(&rows) };
    let boards = records.ledger.policies();
    assert_eq!(boards[0].rules(), boards[2].rules(), "전제: P1 과 P3 의 원문이 같다");
    let chains = line(6);
    assert_eq!(select(4, &records, &chains).expect("골라야 한다").policy(), PolicyAfter { event: m(3), policy: p(3) });
    assert_eq!(select(2, &records, &chains).expect("골라야 한다").policy(), PolicyAfter { event: m(2), policy: p(2) });
}

// ── 사례 H: Exit 출처 — 확정 정책을 유도하지 않는다 ───────────────────────────

#[test]
fn the_exit_source_does_not_derive_its_confirmed_policy() {
    // (가) 의 C4: 기준점을 C4 에서 보관한 사건이 없다. P(C4) 는 M1 의 P1, 지금의 적용 정책은 P2 다. 어느 것도 대입하지 않는다.
    let records = records(&interview_while_waiting_rows());
    let chains = line(6);
    let found = select(4, &records, &chains).expect("골라야 한다");
    assert_eq!(found.source(), exit(1));
    assert_eq!(found.policy(), ExitPolicyNotDerived);
    let opening = select_opening_policy(c(4), &records.ledger, &records.links, &records.facts, &chains, &registry(12))
        .expect("P(C4) 를 고른다");
    assert_eq!(opening.selection(), OpeningSelection::Applied { event: m(1), kind: Start, policy: p(1), next_cycle_id: 1 });
    assert_eq!(records.ledger.policies().last().map(PolicyRecord::name), Some(p(2)));
    assert!(BOUNDARY_WORLD_DOES_NOT_SHOW.iter().any(|line| line.contains("ExitPolicyNotDerived") && line.contains("P(T)")));
}

#[test]
fn empty_records_give_the_exit_and_not_before_adoption() {
    // 사건이 없는 프로젝트(§6.2 「현행 규칙 그대로」) — 세계는 Exit 이고 확정 정책은 유도하지 않았다(정책 채택 전이라고 표시하지 않는다).
    let found = select(2, &no_records(), &world()).expect("골라야 한다");
    assert_eq!((found.target(), found.chain()), (c(2), ch(1)));
    assert_eq!(found.source(), exit(1));
    assert_eq!(found.policy(), ExitPolicyNotDerived);
    assert!(found.inputs().events().is_empty());
}

// ── 사례 I: 체인을 건너는 사건과 3j 의 한계 ─────────────────────────────────
//
// | M1 | start | 없음 → P1 | chain:C1 | `null`·1 | A1, `null`·1 |
// | M2 | scope_change | P1 → P2 | chain:C1 | C2·3 | A2, C2·3 |
// | M3 | scope_change | P2 → P3 | chain:C2 | C4·5 | 실패 뒤 A3, C5·6 |
// | M4 | restore_result | P3 → P3 | chain:C2 | C5·6 | 보관 대기(실패 하나) |

fn two_chain_records() -> Records {
    let rows = [
        start(),
        change(Some(1), 2, 1, 2, 3, kept(2, 2, 3)),
        change(Some(2), 3, 2, 4, 5, Base::Fixed { snapshot: 3, boundary: Some(5), issuer: 6 }),
        restore(3, 2, 5, 6, Base::Waiting),
    ];
    let mut records = records(&rows);
    records.facts[2] = EventCycleFacts::new(
        BaselineFacts::new(m(3), ScopeChange, Some(a(3)), Written(Some(c(5))), Written(6), vec![F, A]),
        ch(2),
        Written(Some(c(4))),
        5,
    );
    records
}

#[test]
fn across_two_chains_with_a_waiting_restore_result() {
    let records = two_chain_records();
    let chains = world();
    let seen: Vec<(u32, BoundarySource)> = (1..=5).map(|n| (n, chosen(n, &records, &chains))).collect();
    assert_eq!(
        seen,
        [
            (1, exit(1)),
            (2, baseline(2, ScopeChange, 2, 2, 2, 3)),
            (3, exit(1)),
            // chain:C2 의 C4 는 M3 의 승인 경계일 뿐이다.
            (4, exit(1)),
            // M4 는 이름이 더 크고 승인 경계도 C5 지만 기준점이 없다.
            (5, baseline(3, ScopeChange, 3, 3, 5, 6)),
        ]
    );
    assert_eq!(select(5, &records, &chains).expect("골라야 한다").chain(), ch(2));
    assert_eq!(select(2, &records, &chains).expect("골라야 한다").chain(), ch(1));
}

#[test]
fn the_unjudged_parts_of_3j_stay_visible() {
    let records = two_chain_records();
    let found = select(5, &records, &world()).expect("골라야 한다");
    let refs = found.inputs().cycle_refs();
    assert_eq!(
        refs.not_judged(),
        [
            (m(1), CycleField::AfterCycleRef, RefReading::NullNotJudged),
            (m(1), CycleField::BaselineAfterCycleRef, RefReading::NullNotJudged),
            (m(4), CycleField::BaselineAfterCycleRef, RefReading::UnwrittenNotJudged),
        ]
    );
    assert_eq!(refs.chains_not_looked_up(), [m(1)]);
    assert_eq!(
        found.inputs().events(),
        [(m(1), Start), (m(2), ScopeChange), (m(3), ScopeChange), (m(4), RestoreResult)]
    );
    let direct = check_event_inputs(&records.ledger, &records.links, &records.facts, &world(), &registry(12)).expect("3j 통과");
    assert_eq!(found.inputs(), &direct, "같은 호출의 3j 결과 그대로다");
    // Exit 을 고른 결과도 같은 3j 결과를 지닌다.
    assert_eq!(select(4, &records, &world()).expect("골라야 한다").inputs(), &direct);
}

#[test]
fn inputs_cut_together_are_not_detected_and_are_chosen_within() {
    // 원장과 두 사건 입력을 함께 M2 까지 자르면 서로 맞으므로 3j 가 통과하고, 잘린 이력 안에서 C5 의 Exit 을 고른다. 한계의 기록이다.
    let full = two_chain_records();
    let rows = [start(), change(Some(1), 2, 1, 2, 3, kept(2, 2, 3))];
    let cut = Records { ledger: ledger(2), links: links_of(&rows), facts: full.facts[..2].to_vec() };
    let chains = world();
    assert_eq!(chosen(5, &full, &chains), baseline(3, ScopeChange, 3, 3, 5, 6));
    assert_eq!(chosen(5, &cut, &chains), exit(1));
    assert!(BOUNDARY_WORLD_DOES_NOT_SHOW[0].contains("함께 잘라낸"));
}

// ── 등록부 ─────────────────────────────────────────────────────────────────

fn only_inputs(problems: &[BoundaryWorldProblem]) -> &[EventInputsProblem] {
    match problems {
        [BoundaryWorldProblem::Inputs(found)] => found,
        other => panic!("입력 갈래 하나만 있어야 한다: {other:#?}"),
    }
}

#[test]
fn a_baseline_missing_from_the_registry_is_refused_by_3j() {
    // M2 의 기준점 A6 이 등록부(A1–A5)에 없다 — 3j 안의 3e 가 거절한다.
    let records = records(&interview_while_waiting_rows());
    let problems = select_with(5, &records, &line(6), &registry(5)).expect_err("거절해야 한다");
    match only_inputs(&problems) {
        [EventInputsProblem::CycleRefs(found)] => assert!(
            found.iter().any(|problem| matches!(problem, CycleRefProblem::Point(points)
                if points.contains(&BaselineProblem::UnknownSnapshot { event: m(2), snapshot: a(6) }))),
            "{found:#?}"
        ),
        other => panic!("{other:#?}"),
    }
}

#[test]
fn a_chosen_exit_missing_from_the_registry_is_refused() {
    // 닫힌 사이클의 Exit 이 A20 인데 등록부는 A1–A12 만 안다.
    let chains = line_from(a(20), 4);
    assert_eq!(
        refused(2, &no_records(), &chains),
        [BoundaryWorldProblem::ExitNotInRegistry { cycle: c(2), world: a(20) }]
    );
    let known = select_with(2, &no_records(), &chains, &registry(20)).expect("A20 을 알면 고른다");
    assert_eq!(known.source(), exit(20));
}

#[test]
fn an_unchosen_exit_is_not_looked_up() {
    // C2 에서 보관한 M2 가 있으면 C2 의 Exit(A20) 은 고르지 않으며 그 등록 여부도 보지 않는다(보이지 않는 것 목록에 적었다).
    let rows = [start(), change(Some(1), 2, 1, 2, 3, kept(2, 2, 3))];
    let records = records(&rows);
    let chains = line_from(a(20), 4);
    assert_eq!(chosen(2, &records, &chains), baseline(2, ScopeChange, 2, 2, 2, 3));
    assert_eq!(refused(3, &records, &chains), [BoundaryWorldProblem::ExitNotInRegistry { cycle: c(3), world: a(20) }]);
    assert!(BOUNDARY_WORLD_DOES_NOT_SHOW.iter().any(|line| line.contains("고르지 않은 Exit")));
}

// ── 입력 오류: 3j 의 실패를 그대로 담고 고르지 않는다 ─────────────────────────

#[test]
fn a_ledger_that_the_links_do_not_explain_is_refused() {
    let rows = [start(), change(Some(1), 2, 1, 2, 3, kept(2, 2, 3))];
    let records = Records { ledger: ledger(3), links: links_of(&rows), facts: facts_of(&rows) };
    let problems = refused(2, &records, &world());
    match only_inputs(&problems) {
        [EventInputsProblem::PolicyLinks(found)] => {
            assert!(found.contains(&LinkProblem::NotIntroduced { policy: p(3) }), "{found:#?}");
        }
        other => panic!("{other:#?}"),
    }
}

#[test]
fn a_count_mismatch_between_the_two_inputs_is_refused() {
    let mut records = two_chain_records();
    records.facts.truncate(3);
    let problems = refused(5, &records, &world());
    match only_inputs(&problems) {
        [EventInputsProblem::Correspondence(found)] => assert_eq!(
            found,
            &[
                InputMismatch::CountDiffers { policy_links: 4, cycle_facts: 3 },
                InputMismatch::Unmatched { side: InputSide::PolicyLinks, position: 3, event: m(4) },
            ]
        ),
        other => panic!("{other:#?}"),
    }
}

#[test]
fn a_kind_mismatch_between_the_two_inputs_is_refused() {
    // 사이클 입력의 M3 을 restore_result 로 뽑았다 — 정책 연결 입력의 M3 은 scope_change. 그 기준점이 C5 의 B 가 될 자리였다.
    let mut records = two_chain_records();
    records.facts[2] = EventCycleFacts::new(
        BaselineFacts::new(m(3), RestoreResult, Some(a(3)), Written(Some(c(5))), Written(6), vec![F, A]),
        ch(2),
        Written(Some(c(4))),
        5,
    );
    let problems = refused(5, &records, &world());
    match only_inputs(&problems) {
        [EventInputsProblem::Correspondence(found)] => assert_eq!(
            found,
            &[InputMismatch::KindDiffers { event: m(3), policy_links: ScopeChange, cycle_facts: RestoreResult }]
        ),
        other => panic!("{other:#?}"),
    }
}

#[test]
fn a_name_mismatch_between_the_two_inputs_is_refused() {
    let mut records = two_chain_records();
    records.facts[3] = fact_of(5, &restore(3, 2, 5, 6, Base::Waiting));
    let problems = refused(5, &records, &world());
    let found = only_inputs(&problems);
    assert!(found.iter().any(|problem| matches!(problem, EventInputsProblem::CycleRefs(_))), "{found:#?}");
    assert!(
        found.contains(&EventInputsProblem::Correspondence(vec![InputMismatch::NameDiffers {
            position: 3,
            policy_links: m(4),
            cycle_facts: m(5)
        }])),
        "{found:#?}"
    );
}

#[test]
fn a_baseline_boundary_in_another_chain_is_refused() {
    // chain:C1 의 사건이 chain:C2 의 C5 에서 보관했다고 적었다(T7, §6.6) — C5 의 B 로 고르지 않는다.
    let rows = [start(), change(Some(1), 2, 1, 2, 3, Base::Fixed { snapshot: 2, boundary: Some(5), issuer: 6 })];
    let problems = refused(5, &records(&rows), &world());
    match only_inputs(&problems) {
        [EventInputsProblem::CycleRefs(found)] => assert!(
            found.contains(&CycleRefProblem::OtherChain {
                event: m(2),
                field: CycleField::BaselineAfterCycleRef,
                cycle: c(5),
                chain: ch(1),
                owner: ch(2)
            }),
            "{found:#?}"
        ),
        other => panic!("{other:#?}"),
    }
}

// ── 대상 오류: 「기준점 없음」·「정책 없음」으로 바꾸지 않는다 ───────────────────

#[test]
fn a_missing_target_is_not_an_exit_or_a_missing_baseline() {
    let records = two_chain_records();
    let not_found = |cycle| [BoundaryWorldProblem::Target(BoundaryTargetProblem::NotFound { cycle: c(cycle) })];
    assert_eq!(refused(7, &records, &world()), not_found(7));
    assert_eq!(refused(7, &no_records(), &world()), not_found(7));
    assert_eq!(refused(1, &no_records(), &[]), not_found(1));
}

#[test]
fn an_open_target_is_refused() {
    // C6 은 열려 있다 — 닫힌 경계가 아니다. 그 Entry 나 열린 사이클의 Verify 계보로 대신하지 않는다.
    let records = two_chain_records();
    assert_eq!(
        refused(6, &records, &world()),
        [BoundaryWorldProblem::Target(BoundaryTargetProblem::TargetNotClosed { cycle: c(6), chain: ch(2) })]
    );
    assert_eq!(
        refused(4, &no_records(), &line(4)),
        [BoundaryWorldProblem::Target(BoundaryTargetProblem::TargetNotClosed { cycle: c(4), chain: ch(1) })]
    );
}

#[test]
fn an_open_target_claimed_as_an_archive_boundary_reports_both() {
    // M2 가 열린 C4 에서 보관했다고 적었다 — 3h 의 OpenCycle 과 대상의 열림을 함께 돌려준다.
    let rows = [start(), change(Some(1), 2, 1, 2, 3, Base::Fixed { snapshot: 2, boundary: Some(4), issuer: 5 })];
    let problems = refused(4, &records(&rows), &line(4));
    assert_eq!(problems.len(), 2, "{problems:#?}");
    assert!(matches!(&problems[0], BoundaryWorldProblem::Inputs(found)
        if matches!(found.as_slice(), [EventInputsProblem::CycleRefs(refs)]
            if refs.contains(&CycleRefProblem::OpenCycle { event: m(2), field: CycleField::BaselineAfterCycleRef, cycle: c(4) }))));
    assert_eq!(
        problems[1],
        BoundaryWorldProblem::Target(BoundaryTargetProblem::TargetNotClosed { cycle: c(4), chain: ch(1) })
    );
}

#[test]
fn a_target_owned_by_two_chain_entries_is_ambiguous() {
    let records = two_chain_records();
    let mut chains = world();
    chains.push(chains[0].clone());
    let problems = refused(2, &records, &chains);
    assert_eq!(problems.len(), 2, "{problems:#?}");
    assert!(matches!(&problems[0], BoundaryWorldProblem::Inputs(found)
        if matches!(found.as_slice(), [EventInputsProblem::CycleRefs(refs)] if refs.iter().any(|problem| matches!(problem, CycleRefProblem::AmbiguousLookup(_))))));
    assert_eq!(
        problems[1],
        BoundaryWorldProblem::Target(BoundaryTargetProblem::SeveralOwners { cycle: c(2), chains: vec![ch(1), ch(1)] })
    );
}

#[test]
fn a_target_whose_chain_name_is_repeated_is_ambiguous() {
    // chain:C2 라는 이름의 다른 항목이 C7 을 지닌다. C4 를 소유한 항목은 하나지만 그 이름의 기록이 둘이다.
    let mut chains = world();
    chains.push(Chain::restore(ch(2), x1(), a(1), None, Cycles::start_at(rules(), x1(), a(1), 7)));
    let problems = refused(4, &no_records(), &chains);
    assert_eq!(problems.len(), 2, "{problems:#?}");
    assert!(matches!(&problems[0], BoundaryWorldProblem::Inputs(_)));
    assert_eq!(
        problems[1],
        BoundaryWorldProblem::Target(BoundaryTargetProblem::OwnerChainRepeated { cycle: c(4), chain: ch(2), count: 2 })
    );
}

#[test]
fn input_and_target_problems_are_reported_together_in_order() {
    let mut records = two_chain_records();
    records.facts.truncate(3);
    let problems = refused(8, &records, &world());
    assert_eq!(problems.len(), 2, "{problems:#?}");
    assert!(matches!(&problems[0], BoundaryWorldProblem::Inputs(_)));
    assert_eq!(problems[1], BoundaryWorldProblem::Target(BoundaryTargetProblem::NotFound { cycle: c(8) }));
    assert!(problems.iter().all(|problem| !problem.to_string().is_empty()));
    let exit_missing = BoundaryWorldProblem::ExitNotInRegistry { cycle: c(2), world: a(20) };
    assert!(!exit_missing.to_string().is_empty());
    for target in [
        BoundaryTargetProblem::TargetNotClosed { cycle: c(6), chain: ch(2) },
        BoundaryTargetProblem::ClosedWithoutExit { cycle: c(6), chain: ch(2) },
        BoundaryTargetProblem::OwnerChainRepeated { cycle: c(4), chain: ch(2), count: 2 },
    ] {
        assert!(!BoundaryWorldProblem::Target(target).to_string().is_empty());
    }
}

// ── 순수성 ─────────────────────────────────────────────────────────────────

#[test]
fn inputs_are_left_unchanged_and_the_answer_is_repeatable() {
    let records = two_chain_records();
    let chains = world();
    let registry = registry(12);
    let before = format!("{:?}{:?}{:?}{:?}{:?}", records.ledger, records.links, records.facts, chains, registry);
    let first = select_with(5, &records, &chains, &registry);
    let second = select_with(5, &records, &chains, &registry);
    let exit_first = select_with(4, &records, &chains, &registry);
    let after = format!("{:?}{:?}{:?}{:?}{:?}", records.ledger, records.links, records.facts, chains, registry);
    assert_eq!(before, after, "입력을 바꿨다");
    assert_eq!(first, second);
    assert_eq!(first.expect("골라야 한다").source(), baseline(3, ScopeChange, 3, 3, 5, 6));
    assert_eq!(exit_first.expect("골라야 한다").source(), exit(1));
}

#[test]
fn the_result_has_no_n_and_the_list_names_what_it_does_not_show() {
    // 보이지 않는 것 목록이 요청 범위의 한계를 모두 적는다.
    for needle in ["함께 잘라낸", "not_judged", "chains_not_looked_up", "ExitPolicyNotDerived", "T8", "S(N)", "LedgerStart", "O-2", "O-3"] {
        assert!(BOUNDARY_WORLD_DOES_NOT_SHOW.iter().any(|line| line.contains(needle)), "{needle}");
    }
}
