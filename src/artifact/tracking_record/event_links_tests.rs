//! 정책 원장과 관리 사건의 정책 연결 검사(「3c」) 시험 — **합성 입력**만 쓴다.
//!
//! 여기의 정책 판·사건 연결은 시험이 만든 값이다. 모니터 승인, 실제 사건 기록, `state.yaml`, `gil start` 를 쓰지 않는다.

use super::*;
use crate::artifact::gilignore::EngineVersion;
use crate::artifact::{ContentDigest, digest_from_raw, digest_of};

use EventKind::{RestoreResult, ScopeChange, Start};

// ── 도구 ───────────────────────────────────────────────────────────────────

fn p(number: u32) -> PolicyName {
    PolicyName::new(number).expect("0 이 아니다")
}

fn m(number: u32) -> EventName {
    EventName::new(number).expect("0 이 아니다")
}

fn rules(text: &str) -> ContentDigest {
    digest_of(text.as_bytes()).expect("메모리의 바이트는 언제나 읽힌다")
}

/// 판마다 원문을 받는다 — 같은 원문을 여러 판에 줄 수 있다.
fn ledger_of(texts: &[&str]) -> PolicyLedger {
    let records = texts
        .iter()
        .enumerate()
        .map(|(at, text)| {
            let seed = u8::try_from(at + 1).unwrap();
            PolicyRecord::new(p(at as u32 + 1), EngineVersion::V1, rules(text), ProposalDigest(digest_from_raw([seed; 32])))
        })
        .collect::<Vec<_>>();
    PolicyLedger::from_records(texts.len() as u32 + 1, records).expect("시험 원장은 꼴이 맞다")
}

fn ledger(count: u32) -> PolicyLedger {
    let texts: Vec<String> = (1..=count).map(|n| format!("rule-{n}/\n")).collect();
    ledger_of(&texts.iter().map(String::as_str).collect::<Vec<_>>())
}

/// `(종류, before, after)` 목록을 M1 부터 이름 붙인다.
fn links(rows: &[(EventKind, Option<u32>, Option<u32>)]) -> EventPolicyLinks {
    let events = rows
        .iter()
        .enumerate()
        .map(|(at, (kind, before, after))| {
            EventPolicyLink::new(m(at as u32 + 1), *kind, before.map(p), after.map(p))
        })
        .collect::<Vec<_>>();
    EventPolicyLinks::from_links(rows.len() as u32 + 1, events).expect("시험 사건 이름은 꼴이 맞다")
}

fn problems(ledger: &PolicyLedger, links: &EventPolicyLinks) -> Vec<LinkProblem> {
    check_policy_links(ledger, links).expect_err("거절되어야 한다")
}

// ── 정상 연결 — 제약을 지나치게 좁히지 않았는가 ───────────────────────────

#[test]
fn no_policies_and_no_events_is_the_accepted_initial_state() {
    let agreed = check_policy_links(&PolicyLedger::empty(), &EventPolicyLinks::empty()).expect("초기 상태");
    assert_eq!(agreed.last_after(), None);
    assert_eq!(agreed.introduced_by(p(1)), None);
    assert_eq!(EventPolicyLinks::empty().next_event_id(), 1);
}

#[test]
fn a_chain_opened_by_start_agrees() {
    let ledger = ledger(3);
    let events = links(&[(Start, None, Some(1)), (ScopeChange, Some(1), Some(2)), (ScopeChange, Some(2), Some(3))]);
    let agreed = check_policy_links(&ledger, &events).expect("start 로 시작한 사슬");
    assert_eq!([1, 2, 3].map(|n| agreed.introduced_by(p(n))), [Some(m(1)), Some(m(2)), Some(m(3))]);
    assert_eq!(agreed.introduced_by(p(4)), None, "원장에 없는 판");
    assert_eq!(agreed.last_after(), Some(p(3)));
}

#[test]
fn an_existing_project_may_adopt_its_first_policy_by_scope_change() {
    let agreed = check_policy_links(&ledger(1), &links(&[(ScopeChange, None, Some(1))])).expect("첫 채택은 scope_change 일 수 있다");
    assert_eq!(agreed.introduced_by(p(1)), Some(m(1)));
    let later = links(&[(ScopeChange, None, Some(1)), (ScopeChange, Some(1), Some(2)), (RestoreResult, Some(2), Some(2))]);
    assert!(check_policy_links(&ledger(2), &later).is_ok());
}

#[test]
fn several_changes_and_policy_keeping_restore_results_agree() {
    let events = links(&[
        (Start, None, Some(1)),
        (RestoreResult, Some(1), Some(1)),
        (ScopeChange, Some(1), Some(2)),
        (RestoreResult, Some(2), Some(2)),
        (RestoreResult, Some(2), Some(2)),
        (ScopeChange, Some(2), Some(3)),
        (ScopeChange, Some(3), Some(4)),
        (RestoreResult, Some(4), Some(4)),
    ]);
    let agreed = check_policy_links(&ledger(4), &events).expect("restore_result 는 정책을 유지한다");
    assert_eq!([1, 2, 3, 4].map(|n| agreed.introduced_by(p(n))), [Some(m(1)), Some(m(3)), Some(m(6)), Some(m(7))]);
    assert_eq!(agreed.last_after(), Some(p(4)), "마지막 사건이 restore_result 여도 마지막 판과 같다");
}

#[test]
fn distinct_versions_with_the_same_rules_are_never_merged() {
    // P1 과 P3 은 같은 원문이다. 같은 원문을 다시 승인하면 새 판이다(계약 §5.1).
    let ledger = ledger_of(&["target/\n", "node_modules/\n", "target/\n"]);
    assert_eq!(ledger.policies()[0].rules(), ledger.policies()[2].rules());
    let events = links(&[(Start, None, Some(1)), (ScopeChange, Some(1), Some(2)), (ScopeChange, Some(2), Some(3))]);
    let agreed = check_policy_links(&ledger, &events).expect("같은 원문의 다른 판");
    assert_eq!((agreed.introduced_by(p(1)), agreed.introduced_by(p(3))), (Some(m(1)), Some(m(3))));

    // 같은 원문이라고 P3 의 자리에 P1 을 다시 적으면 P1 이 두 번 도입되고 P3 은 도입되지 않는다.
    let merged = links(&[(Start, None, Some(1)), (ScopeChange, Some(1), Some(2)), (ScopeChange, Some(2), Some(1))]);
    assert_eq!(
        problems(&ledger, &merged),
        vec![
            LinkProblem::IntroducedAgain { policy: p(1), first: m(1), again: m(3) },
            LinkProblem::NotIntroduced { policy: p(3) },
            LinkProblem::LastMismatch { last_policy: p(3), last_after: p(1) },
        ]
    );
}

// ── 사슬(T3) ───────────────────────────────────────────────────────────────

#[test]
fn a_broken_before_after_chain_is_refused() {
    let gap = links(&[(Start, None, Some(1)), (ScopeChange, Some(1), Some(2)), (ScopeChange, Some(1), Some(3))]);
    assert_eq!(
        problems(&ledger(3), &gap),
        vec![LinkProblem::BrokenChain { event: m(3), before: Some(p(1)), previous_after: Some(p(2)) }]
    );
    let dropped = links(&[(Start, None, Some(1)), (ScopeChange, None, Some(2))]);
    assert_eq!(
        problems(&ledger(2), &dropped),
        vec![LinkProblem::BrokenChain { event: m(2), before: None, previous_after: Some(p(1)) }]
    );
    let first = links(&[(ScopeChange, Some(1), Some(1))]);
    assert_eq!(
        problems(&ledger(1), &first),
        vec![LinkProblem::FirstBeforePresent { event: m(1), before: p(1) }],
        "첫 사건의 before 는 없음이어야 한다"
    );
}

#[test]
fn start_appears_only_as_the_first_event() {
    let late = links(&[(ScopeChange, None, Some(1)), (Start, Some(1), Some(2))]);
    assert_eq!(problems(&ledger(2), &late), vec![LinkProblem::StartNotFirst { event: m(2) }]);
    let restart = links(&[(Start, None, Some(1)), (Start, None, Some(2))]);
    assert_eq!(
        problems(&ledger(2), &restart),
        vec![
            LinkProblem::BrokenChain { event: m(2), before: None, previous_after: Some(p(1)) },
            LinkProblem::StartNotFirst { event: m(2) },
        ]
    );
}

// ── 참조와 도입(T4) ────────────────────────────────────────────────────────

#[test]
fn references_outside_the_given_ledger_are_refused() {
    let after = links(&[(Start, None, Some(1)), (ScopeChange, Some(1), Some(3))]);
    assert_eq!(
        problems(&ledger(2), &after),
        vec![
            LinkProblem::UnknownPolicy { event: m(2), field: LinkField::After, policy: p(3) },
            LinkProblem::NotIntroduced { policy: p(2) },
            LinkProblem::LastMismatch { last_policy: p(2), last_after: p(3) },
        ]
    );
    let before = links(&[(Start, None, Some(1)), (RestoreResult, Some(9), Some(9))]);
    assert_eq!(
        problems(&ledger(1), &before),
        vec![
            LinkProblem::BrokenChain { event: m(2), before: Some(p(9)), previous_after: Some(p(1)) },
            LinkProblem::UnknownPolicy { event: m(2), field: LinkField::Before, policy: p(9) },
            LinkProblem::UnknownPolicy { event: m(2), field: LinkField::After, policy: p(9) },
            LinkProblem::LastMismatch { last_policy: p(1), last_after: p(9) },
        ]
    );
}

#[test]
fn an_event_without_an_after_policy_is_refused() {
    let events = links(&[(Start, None, Some(1)), (ScopeChange, Some(1), None)]);
    assert_eq!(problems(&ledger(1), &events), vec![LinkProblem::MissingAfter { event: m(2) }]);
    let restore = links(&[(RestoreResult, None, None)]);
    assert_eq!(
        problems(&ledger(1), &restore),
        vec![LinkProblem::MissingAfter { event: m(1) }, LinkProblem::NotIntroduced { policy: p(1) }]
    );
}

#[test]
fn missing_and_repeated_introductions_are_refused() {
    // P2 는 원장에 있지만 어느 start·scope_change 도 도입하지 않았다.
    let missing = links(&[(Start, None, Some(1))]);
    assert_eq!(
        problems(&ledger(2), &missing),
        vec![LinkProblem::NotIntroduced { policy: p(2) }, LinkProblem::LastMismatch { last_policy: p(2), last_after: p(1) }]
    );
    // scope_change 가 판을 바꾸지 않으면 같은 판을 두 번 도입한 것이다.
    let repeated = links(&[(Start, None, Some(1)), (ScopeChange, Some(1), Some(1))]);
    assert_eq!(
        problems(&ledger(1), &repeated),
        vec![LinkProblem::IntroducedAgain { policy: p(1), first: m(1), again: m(2) }]
    );
}

#[test]
fn introductions_follow_the_ledger_order() {
    let swapped = links(&[(Start, None, Some(2)), (ScopeChange, Some(2), Some(1)), (ScopeChange, Some(1), Some(3))]);
    assert_eq!(
        problems(&ledger(3), &swapped),
        vec![
            LinkProblem::IntroducedOutOfOrder { event: m(1), found: p(2), expected: p(1) },
            LinkProblem::IntroducedOutOfOrder { event: m(2), found: p(1), expected: p(2) },
        ]
    );
}

#[test]
fn the_last_policy_and_the_last_after_must_match() {
    let behind = links(&[(Start, None, Some(1)), (ScopeChange, Some(1), Some(2)), (RestoreResult, Some(2), Some(2))]);
    assert_eq!(
        problems(&ledger(3), &behind),
        vec![LinkProblem::NotIntroduced { policy: p(3) }, LinkProblem::LastMismatch { last_policy: p(3), last_after: p(2) }]
    );
}

#[test]
fn policies_only_or_events_only_is_a_mismatch() {
    assert_eq!(
        problems(&ledger(2), &EventPolicyLinks::empty()),
        vec![LinkProblem::PoliciesWithoutEvents { policies: 2 }]
    );
    assert_eq!(
        problems(&PolicyLedger::empty(), &links(&[(Start, None, Some(1))])),
        vec![LinkProblem::EventsWithoutPolicies { events: 1 }]
    );
}

#[test]
fn restore_result_never_introduces_or_changes_a_policy() {
    // 바꾸는 restore_result — P2 를 도입한 것으로 세지 않는다.
    let changes = links(&[(Start, None, Some(1)), (RestoreResult, Some(1), Some(2))]);
    assert_eq!(
        problems(&ledger(2), &changes),
        vec![
            LinkProblem::RestoreChangesPolicy { event: m(2), before: Some(p(1)), after: p(2) },
            LinkProblem::NotIntroduced { policy: p(2) },
        ]
    );
    // 첫 사건인 restore_result — 정책을 새로 들이는 길이 아니다.
    let first = links(&[(RestoreResult, None, Some(1))]);
    assert_eq!(
        problems(&ledger(1), &first),
        vec![
            LinkProblem::RestoreChangesPolicy { event: m(1), before: None, after: p(1) },
            LinkProblem::NotIntroduced { policy: p(1) },
        ]
    );
    // 같은 판을 유지하는 restore_result 만으로는 뒤 판이 도입되지 않는다.
    let kept = links(&[(Start, None, Some(1)), (RestoreResult, Some(1), Some(1))]);
    assert_eq!(
        problems(&ledger(2), &kept),
        vec![LinkProblem::NotIntroduced { policy: p(2) }, LinkProblem::LastMismatch { last_policy: p(2), last_after: p(1) }]
    );
}

// ── 사건 이름(T1 의 사건 부분) ─────────────────────────────────────────────

fn named(numbers: &[u32]) -> Vec<EventPolicyLink> {
    numbers.iter().map(|n| EventPolicyLink::new(m(*n), ScopeChange, None, Some(p(1)))).collect()
}

#[test]
fn duplicate_missing_reversed_names_and_a_wrong_issuer_are_refused() {
    assert!(EventPolicyLinks::from_links(3, named(&[1, 2])).is_ok());
    assert_eq!(
        EventPolicyLinks::from_links(3, named(&[1, 1])),
        Err(EventNamesError::DuplicateName { name: m(1), position: 1 })
    );
    assert_eq!(
        EventPolicyLinks::from_links(3, named(&[1, 3])),
        Err(EventNamesError::OutOfSequence { found: m(3), expected: m(2), position: 1 })
    );
    assert_eq!(
        EventPolicyLinks::from_links(2, named(&[2])),
        Err(EventNamesError::OutOfSequence { found: m(2), expected: m(1), position: 0 }),
        "M1 이 빠졌다"
    );
    assert_eq!(
        EventPolicyLinks::from_links(3, named(&[2, 1])),
        Err(EventNamesError::OutOfSequence { found: m(2), expected: m(1), position: 0 }),
        "역순"
    );
    for issuer in [0, 1, 2, 4] {
        assert_eq!(
            EventPolicyLinks::from_links(issuer, named(&[1, 2])),
            Err(EventNamesError::WrongIssuer { found: issuer, wanted: 3 })
        );
    }
    assert_eq!(EventPolicyLinks::from_links(0, Vec::new()), Err(EventNamesError::WrongIssuer { found: 0, wanted: 1 }));
}

#[test]
fn event_names_use_the_shared_number_rules_and_kinds_are_a_closed_set() {
    assert_eq!(EventName::parse_bare("M12"), Ok(m(12)));
    assert_eq!((m(3).id(), m(3).to_string()), ("M3".to_string(), "event:M3".to_string()));
    for bad in ["M0", "M01", "M", "m1", "P1", "M-1", "M 1", "event:M1", ""] {
        assert!(EventName::parse_bare(bad).is_err(), "{bad:?}");
    }
    assert_eq!(EventName::new(0), None);
    for kind in [Start, ScopeChange, RestoreResult] {
        assert_eq!(EventKind::from_name(kind.name()), Some(kind));
    }
    for other in ["Start", "scope-change", "restore", "rename", "custom", ""] {
        assert_eq!(EventKind::from_name(other), None, "{other:?}");
    }
}

// ── 거절은 아무것도 바꾸지 않는다 ─────────────────────────────────────────

#[test]
fn checking_never_changes_its_inputs() {
    let ledger = ledger(2);
    let events = links(&[(Start, None, Some(1)), (RestoreResult, Some(1), Some(2))]);
    let (ledger_before, events_before) = (ledger.clone(), events.clone());
    let first = check_policy_links(&ledger, &events);
    assert!(first.is_err());
    assert_eq!((&ledger, &events), (&ledger_before, &events_before), "거절이 입력을 바꿨다");
    assert_eq!(check_policy_links(&ledger, &events), first, "같은 입력은 같은 결과를 낸다");
    assert_eq!(ledger.next_policy_id(), 3);
    assert_eq!(events.next_event_id(), 3);
}

#[test]
fn every_problem_reads_as_a_sentence() {
    let samples = [
        LinkProblem::PoliciesWithoutEvents { policies: 1 },
        LinkProblem::EventsWithoutPolicies { events: 1 },
        LinkProblem::MissingAfter { event: m(1) },
        LinkProblem::UnknownPolicy { event: m(1), field: LinkField::Before, policy: p(2) },
        LinkProblem::FirstBeforePresent { event: m(1), before: p(1) },
        LinkProblem::BrokenChain { event: m(2), before: None, previous_after: Some(p(1)) },
        LinkProblem::StartNotFirst { event: m(2) },
        LinkProblem::RestoreChangesPolicy { event: m(2), before: Some(p(1)), after: p(2) },
        LinkProblem::IntroducedAgain { policy: p(1), first: m(1), again: m(2) },
        LinkProblem::IntroducedOutOfOrder { event: m(1), found: p(2), expected: p(1) },
        LinkProblem::NotIntroduced { policy: p(2) },
        LinkProblem::LastMismatch { last_policy: p(2), last_after: p(1) },
    ];
    for problem in samples {
        let text = problem.to_string();
        assert!(!text.is_empty() && !text.contains("{"), "{problem:?} → {text:?}");
    }
    assert!(LinkProblem::UnknownPolicy { event: m(1), field: LinkField::Before, policy: p(2) }
        .to_string()
        .contains("policy_before_ref"));
}

#[test]
fn success_does_not_claim_what_was_not_checked() {
    for said in [
        "빠짐없이",
        "proposal_digest",
        "실제 적용",
        "T7",
        "T8",
        "T9",
        "T5",
        "T6",
        "Entry",
        "T12",
        "O-3",
        "T13",
        "restore_result",
    ] {
        assert!(LINKS_DO_NOT_SHOW.iter().any(|line| line.contains(said)), "{said} 을 검사하지 않았다고 적어야 한다");
    }
}
