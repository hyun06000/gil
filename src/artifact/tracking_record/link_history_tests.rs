//! 정책 원장과 사건의 정책 연결 칸의 추가 전후 보존 검사(「3d」) 시험 — **합성 입력**만 쓴다.
//!
//! 여기의 정책 판·사건 연결은 시험이 만든 값이다. 모니터 승인, 실제 사건 기록, `state.yaml`, `gil start` 를 쓰지 않는다.

use super::*;
use crate::artifact::gilignore::EngineVersion;
use crate::artifact::{ContentDigest, digest_from_raw, digest_of};

use EventKind::{RestoreResult, ScopeChange, Start};

// ── 도구 ───────────────────────────────────────────────────────────────────

type Row = (EventKind, Option<u32>, Option<u32>);

fn p(number: u32) -> PolicyName {
    PolicyName::new(number).expect("0 이 아니다")
}

fn m(number: u32) -> EventName {
    EventName::new(number).expect("0 이 아니다")
}

fn rules(text: &str) -> ContentDigest {
    digest_of(text.as_bytes()).expect("메모리의 바이트는 언제나 읽힌다")
}

/// 판마다 원문을 받는다. 제안 참조는 자리에서 정해지므로 같은 원문 목록의 앞부분은 같은 판들이다.
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

/// `rule-1`, `rule-2`, … — `ledger(k)` 는 언제나 `ledger(n)` (n ≥ k) 의 앞부분이다.
fn ledger(count: u32) -> PolicyLedger {
    let texts: Vec<String> = (1..=count).map(|n| format!("rule-{n}/\n")).collect();
    ledger_of(&texts.iter().map(String::as_str).collect::<Vec<_>>())
}

/// `(종류, before, after)` 목록을 M1 부터 이름 붙인다. 이름·발급기의 꼴만 맞추고 정책 연결은 보지 않는다.
fn links(rows: &[Row]) -> EventPolicyLinks {
    let events = rows
        .iter()
        .enumerate()
        .map(|(at, (kind, before, after))| EventPolicyLink::new(m(at as u32 + 1), *kind, before.map(p), after.map(p)))
        .collect::<Vec<_>>();
    EventPolicyLinks::from_links(rows.len() as u32 + 1, events).expect("시험 사건 이름은 꼴이 맞다")
}

fn history(
    before: (&PolicyLedger, &EventPolicyLinks),
    after: (&PolicyLedger, &EventPolicyLinks),
) -> Result<(usize, usize), Vec<HistoryProblem>> {
    check_policy_history(before, after).map(|kept| (kept.policies_added(), kept.events_added()))
}

fn links_problems(ledger: &PolicyLedger, links: &EventPolicyLinks) -> Vec<LinkProblem> {
    check_policy_links(ledger, links).expect_err("이 시점은 거절되어야 한다")
}

const START: Row = (Start, None, Some(1));

// ── 정상 사례 — 제약을 지나치게 좁히지 않았는가 ───────────────────────────

#[test]
fn empty_to_empty_and_unchanged_inputs_are_kept() {
    let empty = (&PolicyLedger::empty(), &EventPolicyLinks::empty());
    assert_eq!(history(empty, empty), Ok((0, 0)));

    let ledger = ledger(2);
    let events = links(&[START, (RestoreResult, Some(1), Some(1)), (ScopeChange, Some(1), Some(2))]);
    assert_eq!(history((&ledger, &events), (&ledger, &events)), Ok((0, 0)));
    assert_eq!(check_link_append_only(&events, &events), Ok(0));
}

#[test]
fn the_first_policy_may_arrive_by_start_or_by_first_adoption() {
    let empty = (&PolicyLedger::empty(), &EventPolicyLinks::empty());
    let started = links(&[START]);
    assert_eq!(history(empty, (&ledger(1), &started)), Ok((1, 1)), "새 프로젝트의 start");
    let adopted = links(&[(ScopeChange, None, Some(1))]);
    assert_eq!(history(empty, (&ledger(1), &adopted)), Ok((1, 1)), "기존 프로젝트의 첫 채택");
}

#[test]
fn a_next_policy_with_its_scope_change_is_appended() {
    let before = links(&[START]);
    let after = links(&[START, (ScopeChange, Some(1), Some(2))]);
    assert_eq!(history((&ledger(1), &before), (&ledger(2), &after)), Ok((1, 1)));
}

#[test]
fn a_policy_keeping_restore_result_appends_an_event_only() {
    let before = links(&[START, (ScopeChange, Some(1), Some(2))]);
    let after = links(&[START, (ScopeChange, Some(1), Some(2)), (RestoreResult, Some(2), Some(2))]);
    let ledger = ledger(2);
    assert_eq!(history((&ledger, &before), (&ledger, &after)), Ok((0, 1)), "정책 원장은 그대로다");
}

#[test]
fn a_new_version_of_the_same_rules_is_appended_not_merged() {
    let before_ledger = ledger_of(&["target/\n", "node_modules/\n"]);
    let after_ledger = ledger_of(&["target/\n", "node_modules/\n", "target/\n"]);
    assert_eq!(after_ledger.policies()[0].rules(), after_ledger.policies()[2].rules(), "P1 과 P3 은 같은 원문이다");
    let before = links(&[START, (ScopeChange, Some(1), Some(2))]);
    let after = links(&[START, (ScopeChange, Some(1), Some(2)), (ScopeChange, Some(2), Some(3))]);
    assert_eq!(history((&before_ledger, &before), (&after_ledger, &after)), Ok((1, 1)));
}

#[test]
fn several_correct_additions_between_two_points_are_accepted() {
    // 명령이 아니라 두 값의 비교다 — 한 번에 하나만 덧붙이라는 제한이 없다.
    let before = links(&[START]);
    let after = links(&[
        START,
        (ScopeChange, Some(1), Some(2)),
        (RestoreResult, Some(2), Some(2)),
        (ScopeChange, Some(2), Some(3)),
        (RestoreResult, Some(3), Some(3)),
    ]);
    assert_eq!(history((&ledger(1), &before), (&ledger(3), &after)), Ok((2, 4)));
    let empty = (&PolicyLedger::empty(), &EventPolicyLinks::empty());
    assert_eq!(history(empty, (&ledger(3), &after)), Ok((3, 5)), "빈 상태에서 여러 판과 사건");
}

// ── 사건 연결 칸 비교 자체 — 어느 칸이 바뀌었는지 가른다 ───────────────────

#[test]
fn removed_or_truncated_events_are_named() {
    let before = links(&[START, (ScopeChange, Some(1), Some(2)), (RestoreResult, Some(2), Some(2))]);
    assert_eq!(
        check_link_append_only(&before, &links(&[START, (ScopeChange, Some(1), Some(2))])),
        Err(LinkAppendViolation::Removed { event: m(3) })
    );
    assert_eq!(check_link_append_only(&before, &EventPolicyLinks::empty()), Err(LinkAppendViolation::Removed { event: m(1) }));
}

#[test]
fn each_changed_slot_is_told_apart() {
    let before = links(&[START, (ScopeChange, Some(1), Some(2))]);
    let cases: [(&[Row], u32, &[LinkSlot]); 6] = [
        (&[(ScopeChange, None, Some(1)), (ScopeChange, Some(1), Some(2))], 1, &[LinkSlot::Kind]),
        (&[START, (ScopeChange, Some(2), Some(2))], 2, &[LinkSlot::PolicyBefore]),
        (&[START, (ScopeChange, Some(1), Some(3))], 2, &[LinkSlot::PolicyAfter]),
        (&[START, (ScopeChange, None, None)], 2, &[LinkSlot::PolicyBefore, LinkSlot::PolicyAfter]),
        (&[START, (RestoreResult, Some(2), Some(1))], 2, &[LinkSlot::Kind, LinkSlot::PolicyBefore, LinkSlot::PolicyAfter]),
        // 끝을 지우고 다른 사건을 붙여도 지운 사건의 자리가 바뀐 것이다.
        (&[START, (RestoreResult, Some(1), Some(1))], 2, &[LinkSlot::Kind, LinkSlot::PolicyAfter]),
    ];
    for (rows, event, slots) in cases {
        assert_eq!(
            check_link_append_only(&before, &links(rows)),
            Err(LinkAppendViolation::Changed { event: m(event), slots: slots.to_vec() }),
            "{rows:?}"
        );
    }
}

#[test]
fn reordering_shows_as_the_first_changed_place() {
    let before = links(&[START, (RestoreResult, Some(1), Some(1)), (ScopeChange, Some(1), Some(2))]);
    let swapped = links(&[START, (ScopeChange, Some(1), Some(2)), (RestoreResult, Some(2), Some(2))]);
    assert_eq!(
        check_link_append_only(&before, &swapped),
        Err(LinkAppendViolation::Changed { event: m(2), slots: vec![LinkSlot::Kind, LinkSlot::PolicyAfter] })
    );
    // 이름은 자리에 묶여 있다 — 이름만 바꾼 목록은 3c 의 꼴 검사에서 이미 세워지지 않는다.
    let renamed = vec![
        EventPolicyLink::new(m(2), Start, None, Some(p(1))),
        EventPolicyLink::new(m(1), RestoreResult, Some(p(1)), Some(p(1))),
    ];
    assert!(EventPolicyLinks::from_links(3, renamed).is_err());
}

// ── 두 시점 비교 — 실패가 어느 갈래인지 가른다 ─────────────────────────────

#[test]
fn swapping_the_first_kind_passes_both_single_point_checks_but_not_history() {
    let ledger = ledger(1);
    let before = links(&[START]);
    let after = links(&[(ScopeChange, None, Some(1))]);
    assert!(check_policy_links(&ledger, &before).is_ok() && check_policy_links(&ledger, &after).is_ok());
    assert_eq!(
        history((&ledger, &before), (&ledger, &after)),
        Err(vec![HistoryProblem::PastEventLinks(LinkAppendViolation::Changed { event: m(1), slots: vec![LinkSlot::Kind] })])
    );
    assert_eq!(
        history((&ledger, &after), (&ledger, &before)),
        Err(vec![HistoryProblem::PastEventLinks(LinkAppendViolation::Changed { event: m(1), slots: vec![LinkSlot::Kind] })]),
        "반대 방향도 같다"
    );
}

#[test]
fn replacing_a_past_event_by_another_consistent_link_is_refused() {
    // M2 가 restore_result 였던 과거를 scope_change 로 바꾸고 그에 맞는 P2 를 붙였다. 두 시점은 각각 일관된다.
    let before = links(&[START, (RestoreResult, Some(1), Some(1))]);
    let after = links(&[START, (ScopeChange, Some(1), Some(2))]);
    assert!(check_policy_links(&ledger(2), &after).is_ok());
    assert_eq!(
        history((&ledger(1), &before), (&ledger(2), &after)),
        Err(vec![HistoryProblem::PastEventLinks(LinkAppendViolation::Changed {
            event: m(2),
            slots: vec![LinkSlot::Kind, LinkSlot::PolicyAfter],
        })])
    );
    // 사건의 자리를 바꿔 다시 맞춘 경우도 같다.
    let before = links(&[START, (RestoreResult, Some(1), Some(1)), (ScopeChange, Some(1), Some(2))]);
    let after = links(&[START, (ScopeChange, Some(1), Some(2)), (RestoreResult, Some(2), Some(2))]);
    assert_eq!(
        history((&ledger(2), &before), (&ledger(2), &after)),
        Err(vec![HistoryProblem::PastEventLinks(LinkAppendViolation::Changed {
            event: m(2),
            slots: vec![LinkSlot::Kind, LinkSlot::PolicyAfter],
        })])
    );
}

#[test]
fn truncating_policies_and_events_together_is_refused_in_both_branches() {
    // 뒤 시점만 보면 일관된다 — P2 와 그 사건을 함께 지웠다.
    let before = links(&[START, (ScopeChange, Some(1), Some(2))]);
    let after = links(&[START]);
    assert!(check_policy_links(&ledger(1), &after).is_ok());
    assert_eq!(
        history((&ledger(2), &before), (&ledger(1), &after)),
        Err(vec![
            HistoryProblem::PastPolicy(AppendViolation::Removed { name: p(2) }),
            HistoryProblem::PastEventLinks(LinkAppendViolation::Removed { event: m(2) }),
        ])
    );
    // 사건만 지우면 뒤 시점의 연결도 깨진다.
    assert_eq!(
        history((&ledger(2), &before), (&ledger(2), &after)),
        Err(vec![
            HistoryProblem::AfterLinks(links_problems(&ledger(2), &after)),
            HistoryProblem::PastEventLinks(LinkAppendViolation::Removed { event: m(2) }),
        ])
    );
    assert_eq!(
        history((&ledger(2), &before), (&ledger(2), &EventPolicyLinks::empty())),
        Err(vec![
            HistoryProblem::AfterLinks(vec![LinkProblem::PoliciesWithoutEvents { policies: 2 }]),
            HistoryProblem::PastEventLinks(LinkAppendViolation::Removed { event: m(1) }),
        ])
    );
}

#[test]
fn rewriting_past_policies_and_events_into_a_consistent_story_is_refused() {
    let before_ledger = ledger_of(&["a/\n", "b/\n"]);
    let before = links(&[START, (ScopeChange, Some(1), Some(2))]);
    // P2 의 원문을 바꾸고 사건은 그대로 — 연결만 보면 일관된다.
    let rewritten = ledger_of(&["a/\n", "c/\n"]);
    assert!(check_policy_links(&rewritten, &before).is_ok());
    assert_eq!(
        history((&before_ledger, &before), (&rewritten, &before)),
        Err(vec![HistoryProblem::PastPolicy(AppendViolation::Changed { name: p(2), fields: vec!["rules"] })])
    );
    // P1 을 지우고 P2 를 P1 자리로 당기며 첫 사건을 첫 채택으로 고쳤다 — 두 갈래 모두에서 걸린다.
    let shifted = ledger_of(&["b/\n"]);
    let story = links(&[(ScopeChange, None, Some(1))]);
    assert!(check_policy_links(&shifted, &story).is_ok());
    assert_eq!(
        history((&before_ledger, &before), (&shifted, &story)),
        Err(vec![
            HistoryProblem::PastPolicy(AppendViolation::Changed { name: p(1), fields: vec!["rules"] }),
            HistoryProblem::PastEventLinks(LinkAppendViolation::Changed { event: m(1), slots: vec![LinkSlot::Kind] }),
        ])
    );
}

#[test]
fn a_policy_without_its_event_or_an_event_without_its_policy_is_refused() {
    let before = links(&[START]);
    let policy_only = history((&ledger(1), &before), (&ledger(2), &before));
    assert_eq!(policy_only, Err(vec![HistoryProblem::AfterLinks(links_problems(&ledger(2), &before))]));
    assert_eq!(
        links_problems(&ledger(2), &before),
        vec![LinkProblem::NotIntroduced { policy: p(2) }, LinkProblem::LastMismatch { last_policy: p(2), last_after: p(1) }]
    );

    let event_only = links(&[START, (ScopeChange, Some(1), Some(2))]);
    assert_eq!(
        history((&ledger(1), &before), (&ledger(1), &event_only)),
        Err(vec![HistoryProblem::AfterLinks(vec![
            LinkProblem::UnknownPolicy { event: m(2), field: LinkField::After, policy: p(2) },
            LinkProblem::LastMismatch { last_policy: p(1), last_after: p(2) },
        ])])
    );
}

#[test]
fn an_inconsistent_before_or_after_point_is_named_as_such() {
    // 변경 전 입력 자체가 틀렸다(P2 를 도입한 사건이 없다). 뒤 시점과 두 보존 비교는 통과한다.
    let broken = links(&[START]);
    let fixed = links(&[START, (ScopeChange, Some(1), Some(2))]);
    assert_eq!(
        history((&ledger(2), &broken), (&ledger(2), &fixed)),
        Err(vec![HistoryProblem::BeforeLinks(links_problems(&ledger(2), &broken))])
    );
    // 변경 후 입력 자체가 틀렸다(같은 판의 재도입). 보존 비교는 통과한다.
    let again = links(&[START, (ScopeChange, Some(1), Some(1))]);
    assert_eq!(
        history((&ledger(1), &broken), (&ledger(1), &again)),
        Err(vec![HistoryProblem::AfterLinks(vec![LinkProblem::IntroducedAgain { policy: p(1), first: m(1), again: m(2) }])])
    );
    // 둘 다 틀렸으면 둘 다 알린다 — 변경 전이 먼저다.
    assert_eq!(
        history((&ledger(2), &broken), (&ledger(2), &links(&[START, (ScopeChange, Some(1), Some(1))]))),
        Err(vec![
            HistoryProblem::BeforeLinks(links_problems(&ledger(2), &broken)),
            HistoryProblem::AfterLinks(links_problems(&ledger(2), &links(&[START, (ScopeChange, Some(1), Some(1))]))),
        ])
    );
}

#[test]
fn merging_a_new_version_into_an_old_one_with_the_same_rules_is_refused() {
    let texts = ["target/\n", "node_modules/\n"];
    let before_ledger = ledger_of(&texts);
    let before = links(&[START, (ScopeChange, Some(1), Some(2))]);
    // 같은 원문이니 새 판 없이 P1 을 다시 가리켰다.
    let back = links(&[START, (ScopeChange, Some(1), Some(2)), (ScopeChange, Some(2), Some(1))]);
    assert_eq!(
        history((&before_ledger, &before), (&before_ledger, &back)),
        Err(vec![HistoryProblem::AfterLinks(vec![
            LinkProblem::IntroducedAgain { policy: p(1), first: m(1), again: m(3) },
            LinkProblem::LastMismatch { last_policy: p(2), last_after: p(1) },
        ])])
    );
    // P3 을 붙였지만 사건은 P1 을 가리킨다.
    let with_p3 = ledger_of(&["target/\n", "node_modules/\n", "target/\n"]);
    assert_eq!(
        history((&before_ledger, &before), (&with_p3, &back)),
        Err(vec![HistoryProblem::AfterLinks(links_problems(&with_p3, &back))])
    );
    // 새 판을 붙이며 과거 사건을 그 판으로 옮겼다(P1 을 쓰던 자리를 P3 으로) — 과거 사건 변경이다.
    let moved = links(&[(Start, None, Some(3)), (ScopeChange, Some(3), Some(2))]);
    let problems = history((&before_ledger, &before), (&with_p3, &moved)).expect_err("거절");
    assert!(problems.contains(&HistoryProblem::PastEventLinks(LinkAppendViolation::Changed {
        event: m(1),
        slots: vec![LinkSlot::PolicyAfter],
    })));
}

// ── 작은 모형과의 대조 — 지나친 허용도 지나친 제한도 없는가 ───────────────

/// 길이 `max` 까지의 일관된 사건 줄거리를 모두 만든다: 첫 사건은 `start` 또는 첫 채택, 그 뒤는 새 판의 `scope_change`
/// 또는 판을 유지하는 `restore_result`. 원장은 도입된 판 수만큼의 `ledger(k)` 다.
fn every_story(max: usize) -> Vec<Vec<Row>> {
    let mut stories = vec![Vec::new()];
    let mut frontier: Vec<Vec<Row>> = vec![vec![START], vec![(ScopeChange, None, Some(1))]];
    while let Some(story) = frontier.pop() {
        if story.len() < max {
            let current = story.last().and_then(|row| row.2).unwrap();
            let next = introduced(&story) + 1;
            for row in [(ScopeChange, Some(current), Some(next)), (RestoreResult, Some(current), Some(current))] {
                let mut longer = story.clone();
                longer.push(row);
                frontier.push(longer);
            }
        }
        stories.push(story);
    }
    stories
}

fn introduced(story: &[Row]) -> u32 {
    story.iter().filter(|row| row.0 != RestoreResult).count() as u32
}

#[test]
fn history_is_kept_exactly_when_the_earlier_story_is_a_prefix() {
    let stories = every_story(4);
    assert_eq!(stories.len(), 1 + 2 + 4 + 8 + 16);
    let (mut kept, mut refused) = (0, 0);
    for earlier in &stories {
        for later in &stories {
            let (before_ledger, after_ledger) = (ledger(introduced(earlier)), ledger(introduced(later)));
            let (before, after) = (links(earlier), links(later));
            let result = history((&before_ledger, &before), (&after_ledger, &after));
            // 모형: 앞 줄거리가 뒤 줄거리의 앞부분이면 받아들이고, 아니면 거절한다.
            let prefix = later.len() >= earlier.len() && later[..earlier.len()] == earlier[..];
            match prefix {
                true => {
                    let added = (introduced(later) - introduced(earlier)) as usize;
                    assert_eq!(result, Ok((added, later.len() - earlier.len())), "{earlier:?} → {later:?}");
                    kept += 1;
                }
                false => {
                    let problems = result.expect_err("앞부분이 아니면 거절되어야 한다");
                    assert!(
                        problems.iter().all(|problem| matches!(
                            problem,
                            HistoryProblem::PastPolicy(_) | HistoryProblem::PastEventLinks(_)
                        )),
                        "두 시점은 모두 일관되므로 과거 변경으로만 거절해야 한다: {earlier:?} → {later:?}: {problems:?}"
                    );
                    assert!(
                        problems.iter().any(|problem| matches!(problem, HistoryProblem::PastEventLinks(_))),
                        "사건 줄거리가 달라졌다: {earlier:?} → {later:?}"
                    );
                    refused += 1;
                }
            }
        }
    }
    assert_eq!(kept + refused, 31 * 31);
    assert!(kept > 31 && refused > 0, "kept {kept}, refused {refused}");
}

// ── 거절은 아무것도 바꾸지 않는다 ─────────────────────────────────────────

#[test]
fn checking_never_changes_its_inputs() {
    let before = (ledger(2), links(&[START, (ScopeChange, Some(1), Some(2))]));
    let after = (ledger(1), links(&[(ScopeChange, None, Some(1))]));
    let (before_copy, after_copy) = (before.clone(), after.clone());
    let first = history((&before.0, &before.1), (&after.0, &after.1));
    assert!(first.is_err());
    assert_eq!((&before, &after), (&before_copy, &after_copy), "거절이 입력을 바꿨다");
    assert_eq!(history((&before.0, &before.1), (&after.0, &after.1)), first, "같은 입력은 같은 결과를 낸다");
    assert_eq!(check_link_append_only(&before.1, &after.1), check_link_append_only(&before_copy.1, &after_copy.1));
}

#[test]
fn the_checker_has_no_way_to_touch_files_or_state() {
    let text = include_str!("link_history.rs");
    for word in ["std::fs", "std::io", "File", "Path", "ProjectSession", "ObjectStore", "Cell", "static mut"] {
        assert!(!text.contains(word), "link_history.rs 가 {word:?} 를 쓴다");
    }
}

// ── 문장과 보장 범위 ───────────────────────────────────────────────────────

#[test]
fn every_problem_reads_as_a_sentence_naming_its_branch() {
    let samples = [
        (HistoryProblem::BeforeLinks(vec![LinkProblem::NotIntroduced { policy: p(2) }]), "변경 전"),
        (HistoryProblem::AfterLinks(vec![LinkProblem::NotIntroduced { policy: p(2) }]), "변경 후"),
        (HistoryProblem::PastPolicy(AppendViolation::Removed { name: p(1) }), "과거 정책"),
        (HistoryProblem::PastEventLinks(LinkAppendViolation::Removed { event: m(1) }), "과거 사건"),
        (
            HistoryProblem::PastEventLinks(LinkAppendViolation::Changed {
                event: m(1),
                slots: vec![LinkSlot::Kind, LinkSlot::PolicyBefore, LinkSlot::PolicyAfter],
            }),
            "kind, policy_before_ref, policy_after_ref",
        ),
    ];
    for (problem, said) in samples {
        let text = problem.to_string();
        assert!(text.contains(said) && !text.contains('{'), "{problem:?} → {text:?}");
    }
}

#[test]
fn success_does_not_claim_what_was_not_checked() {
    for said in [
        "빠짐없이",
        "사용자 승인",
        "실제 적용",
        "사건 기록 전체",
        "world_before_ref",
        "보관 시도",
        "cause",
        "허용된 갱신",
        "T7",
        "원자성",
        "권한",
        "LINKS_DO_NOT_SHOW",
    ] {
        assert!(HISTORY_DOES_NOT_SHOW.iter().any(|line| line.contains(said)), "{said} 을 검사하지 않았다고 적어야 한다");
    }
}
