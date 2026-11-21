//! 한 시점에 건넨 정책 연결 입력(3c)과 사이클 입력(3h)의 대응 검사(「3j」) 시험 — **합성 입력**과 **실제 전이로 만든 체인**을 쓴다.
//!
//! 정책 판·사건 칸·시도 결과·등록부는 시험이 만든 값이다. 체인은 3h 시험과 같은 도움 함수(기존 전이 함수로 걸어 만든다)로 만든다. 승인은
//! 시험 자료의 `approved: yes` 이며 모니터 승인이 아니다. `state.yaml`·`gil start`·파일 시스템을 쓰지 않는다.
//!
//! 세계(3h 시험과 같다):
//!
//! ```text
//! chain:C1 (닫힘)  cycle:C1 ─▶ cycle:C2 ─▶ cycle:C3          모두 닫힘
//! chain:C2 (열림)  cycle:C4 ─▶ cycle:C5 ─▶ cycle:C6          C4·C5 닫힘, C6 열림
//! ```
//!
//! 기본 사건 목록(원장 순서):
//!
//! | 사건 | 종류 | 정책 | chain_ref | 승인 경계·발급 | baseline |
//! |---|---|---|---|---|---|
//! | M1 | start | 없음 → P1 | chain:C1 | `null`·1 | A1, 경계 `null`·1 |
//! | M2 | scope_change | P1 → P2 | chain:C1 | C2·3 | A2, C2·3 |
//! | M3 | scope_change | P2 → P3 | chain:C2 | C4·5 | 실패 뒤 A3, **C5·6** (대기 중 인터뷰 뒤 다른 경계에서 보관) |
//! | M4 | restore_result | P3 → P3 | chain:C2 | C5·6 | 보관 대기(실패 하나) |

use super::event_cycles_tests::{a, c, ch, m, registry, world};
use super::*;
use crate::Chain;
use crate::artifact::gilignore::EngineVersion;
use crate::artifact::{SnapshotRegistry, digest_from_raw, digest_of};
use crate::refs::CycleRef;

use AttemptSeen::{Archived as A, NotArchived as F};
use BaselineSlot::{Unwritten, Written};
use EventKind::{RestoreResult, ScopeChange, Start};

// ── 정책 쪽: 합성 원장과 정책 연결 ──────────────────────────────────────────

fn p(number: u32) -> PolicyName {
    PolicyName::new(number).expect("0 이 아니다")
}

/// 판마다 원문을 받는다. `seed` 로 제안 판 참조를 바꿔 같은 꼴의 다른 원장을 만들 수 있다.
fn ledger_seeded(count: u32, seed: u8) -> PolicyLedger {
    let records = (1..=count)
        .map(|n| {
            let rules = digest_of(format!("rule-{seed}-{n}/\n").as_bytes()).expect("메모리의 바이트는 언제나 읽힌다");
            let proposal = ProposalDigest(digest_from_raw([seed.wrapping_add(u8::try_from(n).unwrap()); 32]));
            PolicyRecord::new(p(n), EngineVersion::V1, rules, proposal)
        })
        .collect::<Vec<_>>();
    PolicyLedger::from_records(count + 1, records).expect("시험 원장은 꼴이 맞다")
}

fn ledger(count: u32) -> PolicyLedger {
    ledger_seeded(count, 0)
}

/// `(종류, before, after)` 목록을 M1 부터 이름 붙인다.
fn named_links(rows: &[(EventKind, Option<u32>, Option<u32>)]) -> EventPolicyLinks {
    let events = rows
        .iter()
        .enumerate()
        .map(|(at, (kind, before, after))| EventPolicyLink::new(m(at as u32 + 1), *kind, before.map(p), after.map(p)))
        .collect::<Vec<_>>();
    EventPolicyLinks::from_links(rows.len() as u32 + 1, events).expect("시험 사건 이름은 꼴이 맞다")
}

/// 기본 목록의 정책 연결 — 앞 `count` 개.
fn policy_rows(count: usize) -> Vec<(EventKind, Option<u32>, Option<u32>)> {
    let all = [(Start, None, Some(1)), (ScopeChange, Some(1), Some(2)), (ScopeChange, Some(2), Some(3)), (RestoreResult, Some(3), Some(3))];
    all[..count].to_vec()
}

/// 기본 목록 앞 `count` 개의 (원장, 정책 연결). 원장은 그 사건들이 도입한 판까지다.
fn policy_side(count: usize) -> (PolicyLedger, EventPolicyLinks) {
    let rows = policy_rows(count);
    let introduced = rows.iter().filter(|(kind, _, _)| *kind != RestoreResult).count();
    (ledger(introduced as u32), named_links(&rows))
}

// ── 사이클 쪽: 합성 사건 칸 ────────────────────────────────────────────────

/// 사건의 `baseline` 쪽.
#[derive(Clone, Copy)]
enum Base {
    /// 기준점 없음, 경계·발급기 칸 없음, 실패 시도 하나.
    Waiting,
    /// 실패 시도 `failures` 개 뒤 `archived` 하나, 기준점·경계(`None` = `null`)·발급기를 함께 적음.
    Fixed { failures: usize, snapshot: u32, boundary: Option<u32>, issuer: u32 },
}

fn fact(number: u32, kind: EventKind, chain: u32, after: BaselineSlot<Option<CycleRef>>, issuer: u32, base: Base) -> EventCycleFacts {
    let baseline = match base {
        Base::Waiting => BaselineFacts::new(m(number), kind, None, Unwritten, Unwritten, vec![F]),
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

fn m1() -> EventCycleFacts {
    fact(1, Start, 1, Written(None), 1, Base::Fixed { failures: 0, snapshot: 1, boundary: None, issuer: 1 })
}

fn m2(kind: EventKind) -> EventCycleFacts {
    fact(2, kind, 1, at(2), 3, Base::Fixed { failures: 0, snapshot: 2, boundary: Some(2), issuer: 3 })
}

/// C4 에서 승인했고, 대기 중 인터뷰 C5 가 닫힌 뒤 그 경계(발급 6)에서 보관했다.
fn m3() -> EventCycleFacts {
    fact(3, ScopeChange, 2, at(4), 5, Base::Fixed { failures: 1, snapshot: 3, boundary: Some(5), issuer: 6 })
}

fn m4(kind: EventKind) -> EventCycleFacts {
    fact(4, kind, 2, at(5), 6, Base::Waiting)
}

/// 기본 목록의 사이클 입력 — 앞 `count` 개.
fn cycle_side(count: usize) -> Vec<EventCycleFacts> {
    let all = vec![m1(), m2(ScopeChange), m3(), m4(RestoreResult)];
    all.into_iter().take(count).collect()
}

// ── 검사 도구 ──────────────────────────────────────────────────────────────

fn check(
    ledger: &PolicyLedger,
    links: &EventPolicyLinks,
    facts: &[EventCycleFacts],
) -> Result<EventInputsAgree, Vec<EventInputsProblem>> {
    check_event_inputs(ledger, links, facts, &world(), &registry(9))
}

fn agree(ledger: &PolicyLedger, links: &EventPolicyLinks, facts: &[EventCycleFacts]) -> EventInputsAgree {
    check(ledger, links, facts).unwrap_or_else(|problems| panic!("받아들여야 한다: {problems:#?}"))
}

fn refused(ledger: &PolicyLedger, links: &EventPolicyLinks, facts: &[EventCycleFacts]) -> Vec<EventInputsProblem> {
    check(ledger, links, facts).expect_err("거절해야 한다")
}

fn policy_alone(ledger: &PolicyLedger, links: &EventPolicyLinks) -> Result<PolicyLinksAgree, Vec<LinkProblem>> {
    check_policy_links(ledger, links)
}

fn cycles_alone(facts: &[EventCycleFacts]) -> Result<CycleRefsAgree, Vec<CycleRefProblem>> {
    check_cycle_refs(facts, &world(), &registry(9))
}

/// 두 부분이 각각 기존 검사를 통과한다 — 「따로 보면 통과하지만」의 전제를 먼저 확인한다.
fn each_part_passes(ledger: &PolicyLedger, links: &EventPolicyLinks, facts: &[EventCycleFacts]) {
    if let Err(problems) = policy_alone(ledger, links) {
        panic!("정책 연결 입력이 3c 를 통과해야 시험의 전제가 선다: {problems:#?}");
    }
    if let Err(problems) = cycles_alone(facts) {
        panic!("사이클 입력이 3h 를 통과해야 시험의 전제가 선다: {problems:#?}");
    }
}

use InputMismatch::{CountDiffers, KindDiffers, NameDiffers, Unmatched};
use InputSide::{CycleFacts, PolicyLinks};

// ── 정상 ───────────────────────────────────────────────────────────────────

#[test]
fn the_fixture_events_are_what_the_table_says() {
    let (ledger, links) = policy_side(4);
    let facts = cycle_side(4);
    assert_eq!(ledger.policies().len(), 3);
    let policy: Vec<_> = links.events().iter().map(|link| (link.name(), link.kind(), link.before(), link.after())).collect();
    assert_eq!(
        policy,
        [
            (m(1), Start, None, Some(p(1))),
            (m(2), ScopeChange, Some(p(1)), Some(p(2))),
            (m(3), ScopeChange, Some(p(2)), Some(p(3))),
            (m(4), RestoreResult, Some(p(3)), Some(p(3))),
        ]
    );
    let cycles: Vec<_> = facts.iter().map(|fact| (fact.event(), fact.baseline().kind(), fact.chain(), fact.issuer())).collect();
    assert_eq!(
        cycles,
        [(m(1), Start, ch(1), 1), (m(2), ScopeChange, ch(1), 3), (m(3), ScopeChange, ch(2), 5), (m(4), RestoreResult, ch(2), 6)]
    );
}

#[test]
fn empty_lists_and_an_empty_ledger_agree() {
    let agreed = agree(&PolicyLedger::empty(), &EventPolicyLinks::empty(), &[]);
    assert!(agreed.events().is_empty());
    assert_eq!(agreed.policy_links().last_after(), None);
    assert!(agreed.cycle_refs().events().is_empty() && agreed.cycle_refs().not_judged().is_empty());
    // 체인 목록이 비어도 사건이 없으면 조회할 것이 없다.
    let empty = check_event_inputs(&PolicyLedger::empty(), &EventPolicyLinks::empty(), &[], &[], &SnapshotRegistry::new());
    assert!(empty.is_ok(), "{empty:?}");
}

#[test]
fn the_same_event_list_agrees_and_keeps_both_successes_as_they_are() {
    let (ledger, links) = policy_side(4);
    let facts = cycle_side(4);
    let agreed = agree(&ledger, &links, &facts);
    assert_eq!(agreed.events(), [(m(1), Start), (m(2), ScopeChange), (m(3), ScopeChange), (m(4), RestoreResult)]);
    assert_eq!(agreed.policy_links(), &policy_alone(&ledger, &links).unwrap(), "3c 의 성공 값 그대로");
    assert_eq!(agreed.cycle_refs(), &cycles_alone(&facts).unwrap(), "3h 의 성공 값 그대로");
}

#[test]
fn the_two_issuers_are_not_compared() {
    // 사건 발급기(next_event_id = 5)와 마지막 사건의 Cycle 발급 번호(6)는 서로 다른 발급기다. 다르다고 거절하지 않고, 같게 만들어도
    // 다른 대응 문제를 숨기지 않는다.
    let (ledger, links) = policy_side(4);
    let facts = cycle_side(4);
    assert_eq!((links.next_event_id(), facts[3].issuer()), (5, 6));
    agree(&ledger, &links, &facts);
    let (ledger3, links3) = policy_side(3);
    assert_eq!((links3.next_event_id(), facts[3].issuer()), (4, 6));
    let found = refused(&ledger3, &links3, &facts);
    assert!(matches!(&found[..], [EventInputsProblem::Correspondence(_)]), "{found:#?}");
}

#[test]
fn an_event_archived_at_another_boundary_is_accepted_as_it_is() {
    // M3 은 C4 에서 승인했고 C5 에서 보관했다(§6.3). 대응 검사는 경계를 보지 않으며 3h 의 판정을 그대로 둔다.
    let (ledger, links) = policy_side(4);
    let agreed = agree(&ledger, &links, &cycle_side(4));
    let third = agreed.cycle_refs().events()[2];
    assert_eq!((third.event(), third.after(), third.baseline_after()), (m(3), RefReading::Checked(c(4)), RefReading::Checked(c(5))));
}

#[test]
fn null_unwritten_and_unlooked_chains_stay_unjudged_in_the_success() {
    use RefReading::{NullNotJudged, UnwrittenNotJudged};
    let (ledger, links) = policy_side(4);
    let facts = cycle_side(4);
    let agreed = agree(&ledger, &links, &facts);
    let direct = cycles_alone(&facts).unwrap();
    assert_eq!(
        agreed.cycle_refs().not_judged(),
        [
            (m(1), CycleField::AfterCycleRef, NullNotJudged),
            (m(1), CycleField::BaselineAfterCycleRef, NullNotJudged),
            (m(4), CycleField::BaselineAfterCycleRef, UnwrittenNotJudged),
        ]
    );
    assert_eq!(agreed.cycle_refs().not_judged(), direct.not_judged());
    assert_eq!(agreed.cycle_refs().chains_not_looked_up(), [m(1)]);

    // 기존 프로젝트의 첫 채택 — 승인 경계 칸이 없고, 가리킨 chain:C9 는 조회하지 않았다. 거절도 승인도 하지 않은 채 남는다.
    let adoption = named_links(&[(ScopeChange, None, Some(1))]);
    let unwritten = [fact(1, ScopeChange, 9, Unwritten, 1, Base::Waiting)];
    let agreed = agree(&ledger_seeded(1, 7), &adoption, &unwritten);
    assert_eq!(
        agreed.cycle_refs().not_judged(),
        [(m(1), CycleField::AfterCycleRef, UnwrittenNotJudged), (m(1), CycleField::BaselineAfterCycleRef, UnwrittenNotJudged)]
    );
    assert_eq!(agreed.cycle_refs().chains_not_looked_up(), [m(1)], "chain:C9 의 실재를 말하지 않는다");
}

// ── 개수 ───────────────────────────────────────────────────────────────────

#[test]
fn a_longer_policy_link_list_is_refused_even_though_each_part_passes() {
    let (ledger, links) = policy_side(4);
    let facts = cycle_side(3);
    each_part_passes(&ledger, &links, &facts);
    assert_eq!(
        refused(&ledger, &links, &facts),
        [EventInputsProblem::Correspondence(vec![
            CountDiffers { policy_links: 4, cycle_facts: 3 },
            Unmatched { side: PolicyLinks, position: 3, event: m(4) },
        ])]
    );
}

#[test]
fn a_longer_cycle_fact_list_is_refused_even_though_each_part_passes() {
    // 정책 연결 M1–M3, 사이클 입력 M1–M4.
    let (ledger, links) = policy_side(3);
    let facts = cycle_side(4);
    each_part_passes(&ledger, &links, &facts);
    assert_eq!(
        refused(&ledger, &links, &facts),
        [EventInputsProblem::Correspondence(vec![
            CountDiffers { policy_links: 3, cycle_facts: 4 },
            Unmatched { side: CycleFacts, position: 3, event: m(4) },
        ])]
    );
    // 두 사건 이상 길어도 짝 없는 사건을 모두 드러낸다. 짧은 쪽에 맞춰 버리지 않는다.
    let (ledger, links) = policy_side(2);
    assert_eq!(
        refused(&ledger, &links, &facts),
        [EventInputsProblem::Correspondence(vec![
            CountDiffers { policy_links: 2, cycle_facts: 4 },
            Unmatched { side: CycleFacts, position: 2, event: m(3) },
            Unmatched { side: CycleFacts, position: 3, event: m(4) },
        ])]
    );
}

#[test]
fn one_empty_side_is_refused_even_though_each_part_passes() {
    let facts = cycle_side(4);
    let (empty_ledger, empty_links) = (PolicyLedger::empty(), EventPolicyLinks::empty());
    each_part_passes(&empty_ledger, &empty_links, &facts);
    let found = refused(&empty_ledger, &empty_links, &facts);
    let mut wanted = vec![CountDiffers { policy_links: 0, cycle_facts: 4 }];
    wanted.extend((0..4).map(|at| Unmatched { side: CycleFacts, position: at, event: m(at as u32 + 1) }));
    assert_eq!(found, [EventInputsProblem::Correspondence(wanted)]);

    let (ledger, links) = policy_side(4);
    each_part_passes(&ledger, &links, &[]);
    let found = refused(&ledger, &links, &[]);
    let mut wanted = vec![CountDiffers { policy_links: 4, cycle_facts: 0 }];
    wanted.extend((0..4).map(|at| Unmatched { side: PolicyLinks, position: at, event: m(at as u32 + 1) }));
    assert_eq!(found, [EventInputsProblem::Correspondence(wanted)]);
}

// ── 종류 ───────────────────────────────────────────────────────────────────

#[test]
fn the_same_name_with_a_different_kind_is_refused_even_though_each_part_passes() {
    // 정책 연결의 M2 는 scope_change, 사이클 입력의 M2 는 restore_result.
    let (ledger, links) = policy_side(4);
    let facts = vec![m1(), m2(RestoreResult), m3(), m4(RestoreResult)];
    each_part_passes(&ledger, &links, &facts);
    assert_eq!(
        refused(&ledger, &links, &facts),
        [EventInputsProblem::Correspondence(vec![KindDiffers {
            event: m(2),
            policy_links: ScopeChange,
            cycle_facts: RestoreResult,
        }])]
    );

    // 여러 사건의 종류가 다르면 모두 돌려준다. 어느 쪽 값으로도 덮어 받아들이지 않는다.
    let facts = vec![fact(1, ScopeChange, 1, Written(None), 1, Base::Fixed { failures: 0, snapshot: 1, boundary: None, issuer: 1 }), m2(ScopeChange), m3(), m4(ScopeChange)];
    each_part_passes(&ledger, &links, &facts);
    assert_eq!(
        refused(&ledger, &links, &facts),
        [EventInputsProblem::Correspondence(vec![
            KindDiffers { event: m(1), policy_links: Start, cycle_facts: ScopeChange },
            KindDiffers { event: m(4), policy_links: RestoreResult, cycle_facts: ScopeChange },
        ])]
    );
}

// ── 이름 ───────────────────────────────────────────────────────────────────

#[test]
fn reordered_or_duplicated_policy_links_are_refused_by_the_existing_constructor() {
    // 정책 연결 쪽의 이름 꼴은 3c 의 생성자가 막는다. 그런 목록은 이 검사까지 오지 않는다 — 새 규칙으로 다시 정의하지 않는다.
    let link = |number: u32, kind, before: Option<u32>, after: Option<u32>| EventPolicyLink::new(m(number), kind, before.map(p), after.map(p));
    let reordered = vec![link(1, Start, None, Some(1)), link(3, ScopeChange, Some(2), Some(3)), link(2, ScopeChange, Some(1), Some(2))];
    assert_eq!(
        EventPolicyLinks::from_links(4, reordered),
        Err(EventNamesError::OutOfSequence { found: m(3), expected: m(2), position: 1 })
    );
    let duplicated = vec![link(1, Start, None, Some(1)), link(1, ScopeChange, Some(1), Some(2))];
    assert_eq!(EventPolicyLinks::from_links(3, duplicated), Err(EventNamesError::DuplicateName { name: m(1), position: 1 }));
}

#[test]
fn reordered_cycle_facts_fail_both_the_name_place_rule_and_the_correspondence() {
    // 사이클 입력은 생성자가 이름 꼴을 보지 않는다. 3h 의 이름 자리 규칙이 첫 자리를 거절하고, 대응 갈래는 이름이 다른 자리를 모두
    // 드러낸다. 이름이 다른 자리는 같은 사건이 아니므로 종류를 견주지 않고, 이름으로 다시 짝지어 보정하지도 않는다.
    let (ledger, links) = policy_side(4);
    let facts = vec![m1(), m3(), m2(ScopeChange), m4(RestoreResult)];
    let alone = cycles_alone(&facts).expect_err("3h 가 이름 자리를 거절한다");
    assert!(alone.contains(&CycleRefProblem::NameOutOfPlace { position: 1, found: m(3), repeated: false }), "{alone:#?}");
    assert_eq!(
        refused(&ledger, &links, &facts),
        [
            EventInputsProblem::CycleRefs(alone),
            EventInputsProblem::Correspondence(vec![
                NameDiffers { position: 1, policy_links: m(2), cycle_facts: m(3) },
                NameDiffers { position: 2, policy_links: m(3), cycle_facts: m(2) },
            ]),
        ]
    );

    // 종류까지 다른 두 사건(scope_change M3, restore_result M4)을 맞바꿔도 이름이 다른 자리의 종류 차이는 보고하지 않는다 — 같은 사건으로
    // 짝짓지 않았기 때문이다.
    let swapped = vec![m1(), m2(ScopeChange), m4(RestoreResult), m3()];
    let found = refused(&ledger, &links, &swapped);
    assert!(
        matches!(&found[..], [EventInputsProblem::CycleRefs(_), EventInputsProblem::Correspondence(mismatch)]
            if mismatch == &[
                NameDiffers { position: 2, policy_links: m(3), cycle_facts: m(4) },
                NameDiffers { position: 3, policy_links: m(4), cycle_facts: m(3) },
            ]),
        "{found:#?}"
    );
}

#[test]
fn duplicated_or_gapped_cycle_facts_are_not_renamed_to_fit() {
    let (ledger, links) = policy_side(3);
    // 중복: M1, M2, M2 — 같은 길이여도 세 번째 자리는 M3 이 아니다. 중복을 지워 맞추지 않는다.
    let duplicated = vec![m1(), m2(ScopeChange), m2(ScopeChange)];
    let alone = cycles_alone(&duplicated).expect_err("3h 가 중복을 거절한다");
    assert!(alone.contains(&CycleRefProblem::NameOutOfPlace { position: 2, found: m(2), repeated: true }), "{alone:#?}");
    assert_eq!(
        refused(&ledger, &links, &duplicated),
        [
            EventInputsProblem::CycleRefs(alone),
            EventInputsProblem::Correspondence(vec![NameDiffers { position: 2, policy_links: m(3), cycle_facts: m(2) }]),
        ]
    );

    // 빈틈: M1, M2, M4 — M4 를 M3 으로 다시 매기지 않는다.
    let gapped = vec![m1(), m2(ScopeChange), m4(ScopeChange)];
    let found = refused(&ledger, &links, &gapped);
    assert!(
        matches!(&found[..], [EventInputsProblem::CycleRefs(_), EventInputsProblem::Correspondence(mismatch)]
            if mismatch == &[NameDiffers { position: 2, policy_links: m(3), cycle_facts: m(4) }]),
        "{found:#?}"
    );
}

// ── 기존 검사의 실패 보존 ──────────────────────────────────────────────────

#[test]
fn a_policy_link_failure_alone_is_kept_as_it_is() {
    // 원장에 P4 가 더 있지만 어느 사건도 도입하지 않는다(3c 의 NotIntroduced·LastMismatch). 사이클 입력과 대응은 맞다 — 그 성공이 정책
    // 쪽 실패를 덮지 않는다.
    let (_, links) = policy_side(4);
    let ledger = ledger(4);
    let facts = cycle_side(4);
    let alone = policy_alone(&ledger, &links).expect_err("3c 가 거절한다");
    assert!(cycles_alone(&facts).is_ok());
    assert_eq!(refused(&ledger, &links, &facts), [EventInputsProblem::PolicyLinks(alone)]);
}

#[test]
fn a_cycle_fact_failure_alone_is_kept_as_it_is() {
    // M4 의 승인 경계가 열린 C6 이다(3h 의 OpenCycle·CycleNotBeforeIssuer). 정책 연결과 대응은 맞다.
    let (ledger, links) = policy_side(4);
    let facts = vec![m1(), m2(ScopeChange), m3(), fact(4, RestoreResult, 2, at(6), 6, Base::Waiting)];
    assert!(policy_alone(&ledger, &links).is_ok());
    let alone = cycles_alone(&facts).expect_err("3h 가 거절한다");
    assert!(alone.contains(&CycleRefProblem::OpenCycle { event: m(4), field: CycleField::AfterCycleRef, cycle: c(6) }), "{alone:#?}");
    assert_eq!(refused(&ledger, &links, &facts), [EventInputsProblem::CycleRefs(alone)]);
}

#[test]
fn existing_failures_and_a_mismatch_are_all_returned_in_order() {
    // 정책 쪽은 원장이 하나 남고(3c 실패), 사이클 쪽은 열린 사이클을 가리키며(3h 실패), 사이클 입력은 한 사건 짧고 M2 의 종류가 다르다.
    let (_, links) = policy_side(4);
    let ledger = ledger(4);
    let facts = vec![m1(), m2(RestoreResult), fact(3, ScopeChange, 2, at(6), 7, Base::Waiting)];
    let policy = policy_alone(&ledger, &links).expect_err("3c 가 거절한다");
    let cycles = cycles_alone(&facts).expect_err("3h 가 거절한다");
    assert_eq!(
        refused(&ledger, &links, &facts),
        [
            EventInputsProblem::PolicyLinks(policy),
            EventInputsProblem::CycleRefs(cycles),
            EventInputsProblem::Correspondence(vec![
                CountDiffers { policy_links: 4, cycle_facts: 3 },
                Unmatched { side: PolicyLinks, position: 3, event: m(4) },
                KindDiffers { event: m(2), policy_links: ScopeChange, cycle_facts: RestoreResult },
            ]),
        ]
    );
}

#[test]
fn an_ambiguous_chain_list_is_kept_as_the_cycle_failure() {
    // 체인 목록의 모호함은 3h 의 실패 그대로다. 대응이 맞아도 성공이 되지 않는다.
    let (ledger, links) = policy_side(4);
    let facts = cycle_side(4);
    let mut chains: Vec<Chain> = world();
    chains.push(chains[0].clone());
    let alone = check_cycle_refs(&facts, &chains, &registry(9)).expect_err("3h 가 모호함을 거절한다");
    assert_eq!(check_event_inputs(&ledger, &links, &facts, &chains, &registry(9)), Err(vec![EventInputsProblem::CycleRefs(alone)]));
}

#[test]
fn the_check_changes_neither_inputs_nor_chains_nor_registry() {
    let (ledger, links) = policy_side(4);
    let facts = vec![m1(), m2(RestoreResult), m3()];
    let chains = world();
    let registry = registry(9);
    let kept = (ledger.clone(), links.clone(), facts.clone(), format!("{chains:?}"), format!("{registry:?}"));
    let _ = check_event_inputs(&ledger, &links, &facts, &chains, &registry);
    let _ = check_event_inputs(&ledger, &links, &cycle_side(4), &chains, &registry);
    assert_eq!((ledger, links, facts, format!("{chains:?}"), format!("{registry:?}")), kept);
}

#[test]
fn every_problem_reads_as_a_sentence_naming_the_events() {
    let (ledger, links) = policy_side(4);
    let facts = vec![m1(), m3(), m2(RestoreResult)];
    let found = refused(&ledger, &links, &facts);
    for problem in &found {
        let text = problem.to_string();
        assert!(!text.contains('{') && text.contains("event:M"), "{problem:?} → {text:?}");
    }
    for mismatch in [
        CountDiffers { policy_links: 4, cycle_facts: 3 },
        Unmatched { side: CycleFacts, position: 3, event: m(4) },
        NameDiffers { position: 1, policy_links: m(2), cycle_facts: m(3) },
        KindDiffers { event: m(2), policy_links: ScopeChange, cycle_facts: RestoreResult },
    ] {
        let text = mismatch.to_string();
        assert!(!text.contains('{') && !text.is_empty(), "{mismatch:?} → {text:?}");
    }
    assert!(KindDiffers { event: m(2), policy_links: ScopeChange, cycle_facts: RestoreResult }.to_string().contains("restore_result"));
}

// ── 한계 ───────────────────────────────────────────────────────────────────

#[test]
fn lists_cut_together_still_pass() {
    // 원장·정책 연결·사이클 입력의 끝을 함께 잘라 건네면 서로 맞으므로 통과한다 — 실제 원장에서 빠짐없이 뽑혔는지는 보이지 않는다.
    let (ledger, links) = policy_side(2);
    let agreed = agree(&ledger, &links, &cycle_side(2));
    assert_eq!(agreed.events(), [(m(1), Start), (m(2), ScopeChange)]);
    assert!(EVENT_INPUTS_DO_NOT_SHOW.iter().any(|line| line.contains("함께 잘라")));
    assert!(EVENT_INPUTS_DO_NOT_SHOW[0].contains("빠짐없이"));
}

#[test]
fn a_different_record_with_the_same_name_and_kind_is_not_told_apart() {
    // 사이클 입력의 M2 를 다른 경계·다른 기준점의 scope_change 로, 원장을 다른 원문·제안의 판들로 바꿔 넣어도 이름·종류가 같고 각 부분이
    // 기존 검사를 통과하면 통과한다. 같은 원본 기록이라는 증명이 아니다.
    let (_, links) = policy_side(4);
    let other_ledger = ledger_seeded(3, 9);
    assert_ne!(other_ledger, ledger(3));
    let other_m2 = fact(2, ScopeChange, 1, at(1), 2, Base::Fixed { failures: 0, snapshot: 7, boundary: Some(1), issuer: 2 });
    assert_ne!(other_m2, m2(ScopeChange));
    let facts = vec![m1(), other_m2, m3(), m4(RestoreResult)];
    let agreed = agree(&other_ledger, &links, &facts);
    assert_eq!(agreed.events(), agree(&ledger(3), &links, &cycle_side(4)).events());
    assert!(EVENT_INPUTS_DO_NOT_SHOW.iter().any(|line| line.contains("같은 이름·같은 종류")));
}

#[test]
fn success_does_not_claim_what_was_not_checked() {
    let said = EVENT_INPUTS_DO_NOT_SHOW.join("\n");
    for topic in [
        "빠짐없이",
        "같은 상태",
        "함께 잘라",
        "같은 이름·같은 종류",
        "원문 객체",
        "승인 응답",
        "두 시점",
        "review_cycle_ref",
        "world_before_ref",
        "null",
        "현재 세계",
        "다음 실험",
        "BaseUndecided",
        "O-3",
        "등록부 매핑",
        "제품 경로",
    ] {
        assert!(said.contains(topic), "{topic} 을 검사하지 않았다고 적어야 한다");
    }
    assert!(UNCHECKED_CONDITIONS.iter().any(|line| line.starts_with("T1") && line.contains("event_inputs")));
}
