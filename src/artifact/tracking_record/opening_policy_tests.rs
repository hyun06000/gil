//! 사이클 N 을 열 당시의 적용 정책 P(N) 선택(§6.2) 시험 — **합성 입력**과 **실제 전이로 만든 체인**을 쓴다.
//!
//! 정책 판·사건 칸·시도 결과·등록부는 시험이 만든 값이다. 체인은 3h 시험의 도움 함수(기존 전이 함수로 걸어 만든다)로 만든다. 승인은
//! 시험 자료의 `approved: yes` 이며 모니터 승인이 아니다. `state.yaml`·`gil start`·파일 시스템을 쓰지 않는다.
//!
//! 기대값은 구현에서 옮기지 않고 계약에서 도출했다 — §6.2 의 정의(「N 이전의 사건 = `next_cycle_id ≤ N`」, 「이름이 가장 큰 것의
//! `policy_after`」, 「없으면 정책 없음」), §6.5 의 두 사례 표(되돌아가기, 보관 대기와 인터뷰 (가)·(나))에 적힌 `P=…`, §5.1 의
//! 판 이름 규칙(같은 원문의 재승인은 새 판), §5.3 의 같은 경계 연속 승인.
//!
//! 세계 두 가지:
//!
//! ```text
//! world()   (3h·3j 시험과 같다)
//!   chain:C1 (닫힘)  cycle:C1 ─▶ cycle:C2 ─▶ cycle:C3          모두 닫힘
//!   chain:C2 (열림)  cycle:C4 ─▶ cycle:C5 ─▶ cycle:C6          C4·C5 닫힘, C6 열림
//! line(n)
//!   chain:C1 (열림)  cycle:C1 ─▶ … ─▶ cycle:C<n>               C<n> 만 열림
//! ```
//!
//! 모든 사이클은 기존 도움 함수가 만든 Interview 다. P(N) 의 정의는 대상의 종류·계보를 쓰지 않으므로 선택의 기대값은 그대로다.

use super::event_cycles_tests::{a, c, ch, close_interview, m, open_interview, registry, rules, world, x1};
use super::*;
use crate::artifact::gilignore::EngineVersion;
use crate::artifact::{SnapshotRegistry, digest_from_raw, digest_of};
use crate::cycles::{CycleId, Cycles};
use crate::{Chain, CycleKind};

use AttemptSeen::{Archived as A, NotArchived as F};
use BaselineSlot::{Unwritten, Written};
use EventKind::{RestoreResult, ScopeChange, Start};
use OpeningSelection::{Applied, BeforeAdoptionInGivenHistory as Before};

// ── 체인 ───────────────────────────────────────────────────────────────────

/// 열린 체인 하나에 `count` 개의 사이클 — 마지막만 열려 있다.
fn line(count: u32) -> Vec<Chain> {
    let mut cycles = Cycles::start_at(rules(), x1(), a(1), 1);
    for _ in 1..count {
        close_interview(&mut cycles, "open_child");
        open_interview(&mut cycles);
    }
    vec![Chain::restore(ch(1), x1(), a(1), None, cycles)]
}

#[test]
fn the_line_fixture_is_what_the_picture_says() {
    let chains = line(9);
    let seen: Vec<(u32, bool)> =
        chains[0].cycles().nodes().iter().map(|cycle| (cycle.id().to_ref().number(), cycle.is_closed())).collect();
    let wanted: Vec<(u32, bool)> = (1..=9).map(|n| (n, n != 9)).collect();
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
    /// `after_cycle_ref` (`None` = `null`).
    at: Option<u32>,
    /// 승인 당시의 `next_cycle_id`.
    issuer: u32,
    base: Base,
}

/// §5.1 예시의 `start` — `chain:C1`, 경계 `null`, `next_cycle_id: 1`, 기준점 A1(경계 `null`, 발급기 1).
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

fn select(target: u32, records: &Records, chains: &[Chain]) -> Result<OpeningPolicy, Vec<OpeningPolicyProblem>> {
    select_opening_policy(c(target), &records.ledger, &records.links, &records.facts, chains, &registry(12))
}

fn chosen(target: u32, records: &Records, chains: &[Chain]) -> OpeningSelection {
    select(target, records, chains).unwrap_or_else(|problems| panic!("골라야 한다: {problems:#?}")).selection()
}

fn refused(target: u32, records: &Records, chains: &[Chain]) -> Vec<OpeningPolicyProblem> {
    select(target, records, chains).expect_err("거절해야 한다")
}

/// 기대값 — 사건 번호·종류·판 번호·그 사건의 승인 당시 `next_cycle_id`.
fn applied(event: u32, kind: EventKind, policy: u32, next_cycle_id: u32) -> OpeningSelection {
    Applied { event: m(event), kind, policy: p(policy), next_cycle_id }
}

// ── 사례 A: §6.5 「되돌아가기」 ─────────────────────────────────────────────
//
// | 사건 | 종류 | 정책 | 승인 경계·next_cycle_id | 기준점 |
// |---|---|---|---|---|
// | M1 | start | 없음 → P1 | `null`·1 | A1 |
// | M2 | scope_change | P1 → P2 | C4·5 | A6 (C4·5) |
// | M3 | scope_change | P2 → P3 | C7·8 | A9 (C7·8) |
// | M4 | restore_result | P3 → P3 | C4·9 | A10 (C4·9) |
//
// §6.5 의 표: C5·C6·C7 은 P=P2, C8 은 P=P3, C9 는 P=P3 이며 출처는 M4. 그 앞 C1–C4 는 M1 의 P1.
// 계보(C6 은 C4 의 자식, C9 도 C4 의 자식)는 정의에 들지 않으므로 직선 체인에서 같은 값이 나와야 한다.

fn revisit_rows() -> Vec<Row> {
    vec![
        start(),
        change(Some(1), 2, 1, 4, 5, kept(6, 4, 5)),
        change(Some(2), 3, 1, 7, 8, kept(9, 7, 8)),
        restore(3, 1, 4, 9, kept(10, 4, 9)),
    ]
}

#[test]
fn the_revisit_example_of_6_5_gives_the_policy_of_each_cycle_as_it_was_opened() {
    let records = records(&revisit_rows());
    let chains = line(9);
    let seen: Vec<(u32, OpeningSelection)> = (1..=9).map(|n| (n, chosen(n, &records, &chains))).collect();
    assert_eq!(
        seen,
        [
            (1, applied(1, Start, 1, 1)),
            (2, applied(1, Start, 1, 1)),
            (3, applied(1, Start, 1, 1)),
            (4, applied(1, Start, 1, 1)),
            (5, applied(2, ScopeChange, 2, 5)),
            (6, applied(2, ScopeChange, 2, 5)),
            (7, applied(2, ScopeChange, 2, 5)),
            (8, applied(3, ScopeChange, 3, 8)),
            (9, applied(4, RestoreResult, 3, 9)),
        ]
    );
}

#[test]
fn next_cycle_id_below_equal_and_above_n() {
    // M2 의 next_cycle_id 는 5. N=C6 이면 5 < 6, N=C5 이면 5 = 5 로 N 이전, N=C4 이면 5 > 4 로 N 이후.
    let records = records(&revisit_rows());
    let chains = line(9);
    assert_eq!(chosen(6, &records, &chains), applied(2, ScopeChange, 2, 5), "next_cycle_id < N");
    assert_eq!(chosen(5, &records, &chains), applied(2, ScopeChange, 2, 5), "next_cycle_id = N");
    assert_eq!(chosen(4, &records, &chains), applied(1, Start, 1, 1), "next_cycle_id > N 인 M2 는 N 이전이 아니다");
}

#[test]
fn a_past_n_does_not_take_the_last_board_of_the_ledger() {
    let records = records(&revisit_rows());
    let last = records.ledger.policies().last().map(PolicyRecord::name);
    assert_eq!(last, Some(p(3)), "원장의 마지막 판 — 지금의 적용 정책");
    let past = chosen(6, &records, &line(9));
    assert_eq!(past, applied(2, ScopeChange, 2, 5));
    assert!(!matches!(past, Applied { policy, .. } if Some(policy) == last));
}

#[test]
fn event_numbers_and_the_event_issuer_are_not_compared_with_the_cycle_number() {
    // 사건 이름의 수나 next_event_id(5) 를 사이클 번호와 견주면 N=C2 에서 M2(번호 2 ≤ 2) 가, N=C5 에서 next_event_id 5 ≤ 5 로
    // 모든 사건이 잡힌다. 정의는 사건의 next_cycle_id 만 본다.
    let records = records(&revisit_rows());
    assert_eq!(records.links.next_event_id(), 5);
    let chains = line(9);
    assert_eq!(chosen(2, &records, &chains), applied(1, Start, 1, 1));
    assert_eq!(chosen(3, &records, &chains), applied(1, Start, 1, 1));
    assert_eq!(chosen(5, &records, &chains), applied(2, ScopeChange, 2, 5));
}

// ── 사례 B: §6.5 「보관 대기와 인터뷰」 (가) — 승인 발급기 5, 보관 발급기 6 ──────
//
// | M1 | start | 없음 → P1 | `null`·1 | A1 |
// | M2 | scope_change | P1 → P2 | C4·5 | A6, **C5·6** (대기 중 인터뷰 C5 가 닫힌 뒤 보관) |
//
// §6.5: 「C6 = C5의 자식, Experiment P=P2」. C5 는 승인(발급 5) 뒤에 연 인터뷰이므로 P(C5) 도 P2 — 보관 발급기 6 으로 고르면 P1 이
// 나온다.

fn interview_while_waiting_rows() -> Vec<Row> {
    vec![start(), change(Some(1), 2, 1, 4, 5, Base::Fixed { snapshot: 6, boundary: Some(5), issuer: 6 })]
}

#[test]
fn the_approval_issuer_not_the_archive_issuer_decides() {
    let records = records(&interview_while_waiting_rows());
    let chains = line(6);
    assert_eq!(chosen(4, &records, &chains), applied(1, Start, 1, 1));
    assert_eq!(chosen(5, &records, &chains), applied(2, ScopeChange, 2, 5), "baseline.next_cycle_id 6 은 기준이 아니다");
    assert_eq!(chosen(6, &records, &chains), applied(2, ScopeChange, 2, 5));
}

#[test]
fn an_interview_opened_while_waiting_is_given_the_applied_policy_not_its_confirmed_one() {
    // §6.3: 대기 중에 연 인터뷰의 Entry·Exit 은 마지막 확정 세계(A5)이고 확정 정책 P1 을 물려받는다. P(C5) 는 열 당시의 적용
    // 정책 P2 다. 이 선택은 P2 를 돌려주며 P1(확정 정책)을 돌려주지 않는다 — 그리고 결과에는 확정 정책을 담는 자리도, 3o 의
    // 시작점으로 바꾸는 문도 없다(그 부재는 경계 시험의 원문 검사가 본다).
    let records = records(&interview_while_waiting_rows());
    let chains = line(6);
    let interview = chains[0].cycles().node(CycleId::from(c(5))).expect("C5 가 있다");
    assert_eq!(interview.kind(), CycleKind::Interview);
    let found = select(5, &records, &chains).expect("골라야 한다");
    assert_eq!(found.selection(), applied(2, ScopeChange, 2, 5));
    assert_ne!(found.selection(), applied(1, Start, 1, 1), "P(N) 은 인터뷰가 물려받은 확정 정책이 아니다");
    assert!(OPENING_POLICY_DOES_NOT_SHOW.iter().any(|line| line.contains("Interview 의 확정 정책")));
    assert!(OPENING_POLICY_DOES_NOT_SHOW.iter().any(|line| line.contains("LedgerStart")));
}

// ── 사례 C: §6.5 (나) — 기준점 없이 대체된 사건도 적용 사건이다 ──────────────
//
// | M1 | start | 없음 → P1 | `null`·1 | A1 |
// | M2 | scope_change | P1 → P2 | C4·5 | 없음(실패 하나) — 뒤 M3 으로 대체 |
// | M3 | scope_change | P2 → P3 | C5·6 | A7 (C5·6) |
//
// §6.5: 「C6 = C5의 자식, Experiment P=P3 … M2는 기준점 없이 대체됨」. 대기 중에 연 C5 는 M2 의 승인(발급 5) 뒤이므로 P2.

#[test]
fn an_event_without_a_baseline_still_applies() {
    let rows = [start(), change(Some(1), 2, 1, 4, 5, Base::Waiting), change(Some(2), 3, 1, 5, 6, kept(7, 5, 6))];
    let records = records(&rows);
    let chains = line(6);
    assert_eq!(chosen(5, &records, &chains), applied(2, ScopeChange, 2, 5), "기준점이 없다고 M1 로 내려가지 않는다");
    assert_eq!(chosen(6, &records, &chains), applied(3, ScopeChange, 3, 6));
}

// ── 사례 D: §5.3 같은 경계의 연속 승인 — 같은 next_cycle_id ─────────────────

#[test]
fn among_events_with_the_same_issuer_the_largest_name_wins() {
    // C2 경계에서 사이클을 열지 않고 세 번 승인했다(발급 3). M2·M3 은 대기 뒤 대체, M4 가 보관했다.
    let rows = [
        start(),
        change(Some(1), 2, 1, 2, 3, Base::Waiting),
        change(Some(2), 3, 1, 2, 3, Base::Waiting),
        change(Some(3), 4, 1, 2, 3, kept(2, 2, 3)),
    ];
    let records = records(&rows);
    let chains = line(4);
    assert_eq!(chosen(2, &records, &chains), applied(1, Start, 1, 1));
    assert_eq!(chosen(3, &records, &chains), applied(4, ScopeChange, 4, 3));
    assert_eq!(chosen(4, &records, &chains), applied(4, ScopeChange, 4, 3));
}

// ── 사례 E: 같은 원문의 두 판 ──────────────────────────────────────────────

#[test]
fn boards_with_the_same_source_stay_distinct_with_their_own_events() {
    // §5.1: 같은 원문을 나중에 다시 승인하면 새 판이 된다. P1 과 P3 은 원문 주소가 같지만 다른 판이고 다른 사건이 도입했다.
    let rows = [start(), change(Some(1), 2, 1, 2, 3, kept(2, 2, 3)), change(Some(2), 3, 1, 4, 5, kept(3, 4, 5))];
    let records = Records { ledger: ledger_of(&["keep/\n", "drop/\n", "keep/\n"]), links: links_of(&rows), facts: facts_of(&rows) };
    let boards = records.ledger.policies();
    assert_eq!(boards[0].rules(), boards[2].rules(), "전제: P1 과 P3 의 원문이 같다");
    let chains = line(6);
    assert_eq!(chosen(2, &records, &chains), applied(1, Start, 1, 1));
    assert_eq!(chosen(3, &records, &chains), applied(2, ScopeChange, 2, 3));
    assert_eq!(chosen(5, &records, &chains), applied(3, ScopeChange, 3, 5));
    assert_ne!(chosen(2, &records, &chains), chosen(5, &records, &chains), "같은 원문이라도 판과 사건 출처가 다르다");
}

// ── 사례 F: 정책 채택 전 ───────────────────────────────────────────────────

#[test]
fn empty_records_are_before_adoption_for_a_real_target() {
    let records = Records { ledger: PolicyLedger::empty(), links: EventPolicyLinks::empty(), facts: Vec::new() };
    let found = select(3, &records, &world()).expect("골라야 한다");
    assert_eq!(found.selection(), Before);
    assert_eq!((found.target(), found.chain()), (c(3), ch(1)));
    assert!(found.inputs().events().is_empty());
}

#[test]
fn a_target_before_every_event_is_before_adoption() {
    // 기존 프로젝트의 첫 채택(§5.1: scope_change, 없음 → P1)을 C2 경계에서 저장했다(발급 3). C1·C2 를 열 때는 사건이 없었다.
    let rows = [
        change(None, 1, 1, 2, 3, kept(2, 2, 3)),
        change(Some(1), 2, 1, 3, 4, kept(3, 3, 4)),
    ];
    let records = records(&rows);
    let chains = line(5);
    assert_eq!(chosen(1, &records, &chains), Before);
    assert_eq!(chosen(2, &records, &chains), Before);
    assert_eq!(chosen(3, &records, &chains), applied(1, ScopeChange, 1, 3));
    assert_eq!(chosen(4, &records, &chains), applied(2, ScopeChange, 2, 4));
}

// ── 사례 G: 3j 시험의 세계 — 체인을 건너는 사건과 기준점 없는 restore_result ─────
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
    // 3j 시험과 같이 M3 은 실패 한 번 뒤에 보관했다.
    records.facts[2] = EventCycleFacts::new(
        BaselineFacts::new(m(3), ScopeChange, Some(a(3)), Written(Some(c(5))), Written(6), vec![F, A]),
        ch(2),
        Written(Some(c(4))),
        5,
    );
    records
}

#[test]
fn across_two_chains_and_with_a_waiting_restore_result() {
    let records = two_chain_records();
    let chains = world();
    let seen: Vec<(u32, OpeningSelection)> = (1..=6).map(|n| (n, chosen(n, &records, &chains))).collect();
    assert_eq!(
        seen,
        [
            (1, applied(1, Start, 1, 1)),
            (2, applied(1, Start, 1, 1)),
            (3, applied(2, ScopeChange, 2, 3)),
            // chain:C2 의 첫 사이클 — 사건의 chain_ref 로 거르지 않는다.
            (4, applied(2, ScopeChange, 2, 3)),
            (5, applied(3, ScopeChange, 3, 5)),
            // 기준점이 없는 restore_result 가 출처다. 같은 판 P3 을 도입한 M3 으로 바꾸지 않는다.
            (6, applied(4, RestoreResult, 3, 6)),
        ]
    );
    let found = select(6, &records, &chains).expect("골라야 한다");
    assert_eq!(found.chain(), ch(2));
}

#[test]
fn the_unjudged_parts_of_3j_stay_visible() {
    // M1 의 두 경계는 null, M4 의 baseline 경계는 칸 없음 — 판단하지 않았다. 실제 사이클 참조가 없는 M1 만 chain_ref 를 조회하지 않았다.
    let records = two_chain_records();
    let found = select(6, &records, &world()).expect("골라야 한다");
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
    // 같은 호출의 3j 결과 그대로다.
    let direct = check_event_inputs(&records.ledger, &records.links, &records.facts, &world(), &registry(12)).expect("3j 통과");
    assert_eq!(found.inputs(), &direct);
}

#[test]
fn inputs_cut_together_are_not_detected_and_are_chosen_within() {
    // 원장과 두 사건 입력을 함께 M2 까지 자르면 서로 맞으므로 3j 가 통과하고, 잘린 이력 안에서 고른다. 이것은 한계의 기록이다.
    let full = two_chain_records();
    let cut = Records { ledger: ledger(2), links: links_of(&[start(), change(Some(1), 2, 1, 2, 3, kept(2, 2, 3))]), facts: full.facts[..2].to_vec() };
    let chains = world();
    assert_eq!(chosen(6, &full, &chains), applied(4, RestoreResult, 3, 6));
    assert_eq!(chosen(6, &cut, &chains), applied(2, ScopeChange, 2, 3));
    assert!(OPENING_POLICY_DOES_NOT_SHOW[0].contains("함께 잘라낸"));
}

// ── 입력 오류: 3j 의 실패를 그대로 담고 고르지 않는다 ─────────────────────────

fn only_inputs(problems: &[OpeningPolicyProblem]) -> &[EventInputsProblem] {
    match problems {
        [OpeningPolicyProblem::Inputs(found)] => found,
        other => panic!("입력 갈래 하나만 있어야 한다: {other:#?}"),
    }
}

#[test]
fn a_ledger_that_the_links_do_not_explain_is_refused() {
    // 원장에는 P3 이 있는데 사건은 P2 까지만 도입한다(T4).
    let rows = [start(), change(Some(1), 2, 1, 2, 3, kept(2, 2, 3))];
    let records = Records { ledger: ledger(3), links: links_of(&rows), facts: facts_of(&rows) };
    let problems = refused(3, &records, &world());
    match only_inputs(&problems) {
        [EventInputsProblem::PolicyLinks(found)] => {
            assert!(found.contains(&LinkProblem::NotIntroduced { policy: p(3) }), "{found:#?}");
        }
        other => panic!("{other:#?}"),
    }
}

#[test]
fn a_broken_policy_chain_is_refused() {
    // M2 의 before 가 앞 사건의 after(P1) 가 아니다(T3).
    let rows = [start(), change(None, 2, 1, 2, 3, kept(2, 2, 3))];
    let problems = refused(3, &records(&rows), &world());
    match only_inputs(&problems) {
        [EventInputsProblem::PolicyLinks(found)] => assert!(
            found.iter().any(|problem| matches!(problem, LinkProblem::BrokenChain { event, .. } if *event == m(2))),
            "{found:#?}"
        ),
        other => panic!("{other:#?}"),
    }
}

#[test]
fn a_count_mismatch_between_the_two_inputs_is_refused() {
    let mut records = two_chain_records();
    records.facts.truncate(3);
    let problems = refused(6, &records, &world());
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
    // 사이클 입력의 M4 를 scope_change 로 뽑았다 — 정책 연결 입력의 M4 는 restore_result.
    let mut records = two_chain_records();
    records.facts[3] = fact_of(4, &Row { kind: ScopeChange, ..restore(3, 2, 5, 6, Base::Waiting) });
    let problems = refused(6, &records, &world());
    match only_inputs(&problems) {
        [EventInputsProblem::Correspondence(found)] => assert_eq!(
            found,
            &[InputMismatch::KindDiffers { event: m(4), policy_links: RestoreResult, cycle_facts: ScopeChange }]
        ),
        other => panic!("{other:#?}"),
    }
}

#[test]
fn a_name_mismatch_between_the_two_inputs_is_refused() {
    // 사이클 입력의 넷째 자리가 M5 다 — 대응 실패이고 3h 의 이름 자리도 실패한다. 둘 다 그대로 담는다.
    let mut records = two_chain_records();
    records.facts[3] = fact_of(5, &restore(3, 2, 5, 6, Base::Waiting));
    let problems = refused(6, &records, &world());
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
fn cycle_reference_problems_are_refused() {
    // M2 의 승인 경계가 열린 C6 이고(번호 6 ≥ 발급 3), M3 의 경계는 없는 C9 다.
    let rows = [
        start(),
        change(Some(1), 2, 2, 6, 3, Base::Waiting),
        change(Some(2), 3, 2, 9, 10, Base::Waiting),
    ];
    let problems = refused(5, &records(&rows), &world());
    match only_inputs(&problems) {
        [EventInputsProblem::CycleRefs(found)] => {
            assert!(found.contains(&CycleRefProblem::OpenCycle { event: m(2), field: CycleField::AfterCycleRef, cycle: c(6) }));
            assert!(found.contains(&CycleRefProblem::UnknownCycle { event: m(3), field: CycleField::AfterCycleRef, cycle: c(9) }));
        }
        other => panic!("{other:#?}"),
    }
}

#[test]
fn a_baseline_missing_from_the_registry_is_refused() {
    let records = two_chain_records();
    let problems =
        select_opening_policy(c(6), &records.ledger, &records.links, &records.facts, &world(), &registry(1)).expect_err("거절해야 한다");
    match only_inputs(&problems) {
        [EventInputsProblem::CycleRefs(found)] => {
            assert!(found.iter().any(|problem| matches!(problem, CycleRefProblem::Point(_))), "{found:#?}");
        }
        other => panic!("{other:#?}"),
    }
}

// ── 대상 오류: 「정책 없음」으로 바꾸지 않는다 ───────────────────────────────

#[test]
fn a_missing_target_is_not_before_adoption() {
    let records = two_chain_records();
    assert_eq!(refused(7, &records, &world()), [OpeningPolicyProblem::Target(TargetProblem::NotFound { cycle: c(7) })]);
    // 사건이 없어도 같다.
    let empty = Records { ledger: PolicyLedger::empty(), links: EventPolicyLinks::empty(), facts: Vec::new() };
    assert_eq!(refused(7, &empty, &world()), [OpeningPolicyProblem::Target(TargetProblem::NotFound { cycle: c(7) })]);
    assert_eq!(refused(1, &empty, &[]), [OpeningPolicyProblem::Target(TargetProblem::NotFound { cycle: c(1) })]);
}

#[test]
fn a_target_owned_by_two_chain_entries_is_ambiguous() {
    let records = two_chain_records();
    let mut chains = world();
    chains.push(chains[0].clone());
    let problems = refused(2, &records, &chains);
    assert_eq!(problems.len(), 2, "{problems:#?}");
    assert!(matches!(&problems[0], OpeningPolicyProblem::Inputs(found)
        if matches!(found.as_slice(), [EventInputsProblem::CycleRefs(refs)] if refs.iter().any(|problem| matches!(problem, CycleRefProblem::AmbiguousLookup(_))))));
    assert_eq!(problems[1], OpeningPolicyProblem::Target(TargetProblem::SeveralOwners { cycle: c(2), chains: vec![ch(1), ch(1)] }));
}

#[test]
fn a_target_whose_chain_name_is_repeated_is_ambiguous() {
    // chain:C2 라는 이름의 다른 항목이 C7 을 지닌다. C4 를 소유한 항목은 하나지만 그 이름의 기록이 둘이다.
    let records = two_chain_records();
    let mut chains = world();
    chains.push(Chain::restore(ch(2), x1(), a(1), None, Cycles::start_at(rules(), x1(), a(1), 7)));
    let problems = refused(4, &records, &chains);
    assert_eq!(problems.len(), 2, "{problems:#?}");
    assert!(matches!(&problems[0], OpeningPolicyProblem::Inputs(_)));
    assert_eq!(
        problems[1],
        OpeningPolicyProblem::Target(TargetProblem::OwnerChainRepeated { cycle: c(4), chain: ch(2), count: 2 })
    );
}

#[test]
fn input_and_target_problems_are_reported_together_in_order() {
    let mut records = two_chain_records();
    records.facts.truncate(3);
    let problems = refused(8, &records, &world());
    assert_eq!(problems.len(), 2, "{problems:#?}");
    assert!(matches!(&problems[0], OpeningPolicyProblem::Inputs(_)));
    assert_eq!(problems[1], OpeningPolicyProblem::Target(TargetProblem::NotFound { cycle: c(8) }));
    assert!(problems.iter().all(|problem| !problem.to_string().is_empty()));
}

// ── 순수성 ─────────────────────────────────────────────────────────────────

#[test]
fn inputs_are_left_unchanged_and_the_answer_is_repeatable() {
    let records = two_chain_records();
    let chains = world();
    let registry = registry(12);
    let before = format!("{:?}{:?}{:?}{:?}{:?}", records.ledger, records.links, records.facts, chains, registry);
    let first = select_opening_policy(c(5), &records.ledger, &records.links, &records.facts, &chains, &registry);
    let second = select_opening_policy(c(5), &records.ledger, &records.links, &records.facts, &chains, &registry);
    let after = format!("{:?}{:?}{:?}{:?}{:?}", records.ledger, records.links, records.facts, chains, registry);
    assert_eq!(before, after, "입력을 바꿨다");
    assert_eq!(first, second);
    assert_eq!(first.expect("골라야 한다").selection(), applied(3, ScopeChange, 3, 5));
}

#[test]
fn an_empty_registry_and_no_events_need_no_store() {
    let empty = Records { ledger: PolicyLedger::empty(), links: EventPolicyLinks::empty(), facts: Vec::new() };
    let found = select_opening_policy(c(1), &empty.ledger, &empty.links, &empty.facts, &line(1), &SnapshotRegistry::new());
    assert_eq!(found.expect("골라야 한다").selection(), Before);
}
