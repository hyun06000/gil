//! 원장 구간 선택(「3o」)의 시험 — **합성 자료**만 쓴다.
//!
//! 정책 원문은 시험이 만든 바이트이고, 원장·제안 참조는 시험이 만든 값이다. 모니터가 승인한 판이 아니다. 경로는 글자뿐이며
//! 어떤 폴더·파일·`state.yaml`·객체 창고도 열지 않는다. 3n 시험과 같은 경로 하나(`data/keep.csv`)를 그대로 두고 원장과
//! 시작점만 바꾼다.

use super::*;
use crate::artifact::gilignore::{ApprovedPolicy, Decision, EngineVersion, PathKind};
use crate::artifact::{EntryPath, digest_from_raw};

// ── 도구 ───────────────────────────────────────────────────────────────────

const PATH: &str = "data/keep.csv";
/// 그 경로를 건드리지 않는 판.
const TRACKS: &[u8] = b"tmp/\n";
/// 다른 바이트의 「건드리지 않는」 판.
const TRACKS_OTHER: &[u8] = b"build/\n*.log\n";
/// 그 경로를 부모 폴더째 제외하는 판.
const EXCLUDES: &[u8] = b"data/\n";
/// 부모 폴더를 제외한 뒤 그 경로만 다시 포함하는 판.
const REINCLUDES: &[u8] = b"data/\n!data/keep.csv\n";
/// 다른 규칙으로 그 경로를 제외하는 판.
const EXCLUDES_CSV: &[u8] = b"*.csv\n";

fn policy(bytes: &[u8]) -> ApprovedPolicy {
    ApprovedPolicy::compile(EngineVersion::V1, bytes).expect("시험 원문은 해석된다")
}

fn path(text: &str) -> EntryPath {
    EntryPath::parse(text).expect("시험 경로는 경로 계약을 지킨다")
}

fn name(number: u32) -> PolicyName {
    PolicyName::new(number).expect("0 이 아니다")
}

/// 정책들을 차례로 P1, P2, … 판으로 적은 합성 원장 — 판의 원문 주소는 그 정책의 정체성 그대로다.
fn ledger(boards: &[&ApprovedPolicy]) -> PolicyLedger {
    let records = boards
        .iter()
        .enumerate()
        .map(|(at, policy)| {
            let number = at as u32 + 1;
            let identity = policy.identity();
            let proposal = ProposalDigest(digest_from_raw([number as u8; 32]));
            PolicyRecord::new(name(number), identity.engine(), identity.rules_digest().clone(), proposal)
        })
        .collect::<Vec<_>>();
    PolicyLedger::from_records(records.len() as u32 + 1, records).expect("시험 원장은 꼴이 맞다")
}

/// 판 번호 `first..` 의 자료를 원장 순서대로 건넨다.
fn given<'a>(first: u32, boards: &[&'a ApprovedPolicy]) -> BoardPolicies<'a> {
    boards
        .iter()
        .enumerate()
        .fold(BoardPolicies::new(), |acc, (at, policy)| acc.with(name(first + at as u32), policy))
}

fn classify(ledger: &PolicyLedger, start: LedgerStart, policies: &BoardPolicies<'_>) -> PathContinuity {
    classify_path_over_ledger(ledger, start, policies, &path(PATH), PathKind::File).expect("구간을 고를 수 있다")
}

fn refused(ledger: &PolicyLedger, start: LedgerStart, policies: &BoardPolicies<'_>) -> LedgerSpanProblem {
    match classify_path_over_ledger(ledger, start, policies, &path(PATH), PathKind::File) {
        Err(problem) => problem,
        Ok(answer) => panic!("판정을 하지 말아야 한다: {answer:?}"),
    }
}

fn board(number: u32) -> LedgerStart {
    LedgerStart::Board(name(number))
}

fn continuous() -> PathContinuity {
    PathContinuity::ContinuouslyTracked { current: Decision::Tracked { by: None } }
}

// ── 요구 사례: 판 이름으로 시작하고 중간 판을 지킨다 ───────────────────────

#[test]
fn the_start_board_is_chosen_by_name_and_middle_boards_are_kept() {
    // P1 포함 → P2 제외 → P3 재포함. P1 과 P3 은 같은 원문이고, 같은 해석 값 하나를 두 판에 건넨다.
    let (tracks, excludes) = (policy(TRACKS), policy(EXCLUDES));
    let ledger = ledger(&[&tracks, &excludes, &tracks]);
    let records = ledger.policies();
    assert_eq!(records[0].rules(), records[2].rules(), "P1 과 P3 은 같은 원문의 다른 판이다");
    assert_ne!(records[0].name(), records[2].name());
    let policies = given(1, &[&tracks, &excludes, &tracks]);

    // P1 에서 시작: 구간 P1 → P2 → P3, P2 자리가 빠졌다 → 새로 추적.
    let from_p1 = select_ledger_span(&ledger, board(1), &policies).unwrap();
    assert_eq!(from_p1.boards(), [name(1), name(2), name(3)]);
    assert_eq!((from_p1.applied_board(), from_p1.applied_span().len()), (name(3), 3));
    let answer = from_p1.classify(&path(PATH), PathKind::File);
    assert_eq!(answer, PathContinuity::NewlyTracked { current: Decision::Tracked { by: None }, not_tracked_at: vec![1] });
    assert_eq!(from_p1.board_at(1), Some(name(2)), "빠진 자리는 P2 다");

    // 같은 원장, P3 에서 시작: 한 판짜리 구간 → 계속 추적. 원문 지문으로 찾았다면 P1 이 걸렸을 것이다.
    let by_digest = records.iter().find(|record| record.rules() == records[2].rules()).unwrap().name();
    assert_eq!(by_digest, name(1), "지문으로 찾으면 앞의 판이 먼저 나온다");
    let from_p3 = select_ledger_span(&ledger, board(3), &policies).unwrap();
    assert_eq!(from_p3.boards(), [name(3)]);
    assert_eq!((from_p3.applied_board(), from_p3.applied_span().len()), (name(3), 1));
    assert_eq!(from_p3.classify(&path(PATH), PathKind::File), continuous());
}

#[test]
fn an_exclusion_before_the_start_board_does_not_matter() {
    let (excludes, tracks, other) = (policy(EXCLUDES), policy(TRACKS), policy(TRACKS_OTHER));
    let ledger = ledger(&[&excludes, &tracks, &other]);

    // P2 에서 시작하면 P1 의 제외는 구간 밖이다. P1 의 자료는 건네지 않아도 된다.
    assert_eq!(classify(&ledger, board(2), &given(2, &[&tracks, &other])), continuous());
    // 건네도 판정에 쓰이지 않는다(정체성만 견준다).
    assert_eq!(classify(&ledger, board(2), &given(1, &[&excludes, &tracks, &other])), continuous());

    // 대조: P1 에서 시작하면 그 제외가 구간 안이다.
    let from_p1 = select_ledger_span(&ledger, board(1), &given(1, &[&excludes, &tracks, &other])).unwrap();
    let answer = from_p1.classify(&path(PATH), PathKind::File);
    assert_eq!(answer, PathContinuity::NewlyTracked { current: Decision::Tracked { by: None }, not_tracked_at: vec![0] });
    assert_eq!(from_p1.board_at(0), Some(name(1)));
}

#[test]
fn a_pending_exclusion_proposal_cannot_narrow_the_comparison() {
    let ledger_of_applied = ledger;
    let (tracks, other, proposal) = (policy(TRACKS), policy(TRACKS_OTHER), policy(EXCLUDES));
    let ledger = ledger(&[&tracks, &other]);
    let policies = given(1, &[&tracks, &other]);
    assert_eq!(classify(&ledger, board(1), &policies), continuous(), "승인 전 제안은 원장에 없다");

    // 제안을 다음 판 이름으로 끼워 넣으면 거절한다 — 구간의 끝은 원장의 마지막 판이다.
    let slipped = given(1, &[&tracks, &other]).with(name(3), &proposal);
    assert_eq!(
        refused(&ledger, board(1), &slipped),
        LedgerSpanProblem::Policies(vec![BoardPolicyProblem::NotInLedger { name: name(3) }])
    );
    // 원장의 판 자리에 제안을 건네도 거절한다.
    let swapped = given(1, &[&tracks, &proposal]);
    let found = proposal.identity().rules_digest().clone();
    assert_eq!(
        refused(&ledger, board(1), &swapped),
        LedgerSpanProblem::Policies(vec![BoardPolicyProblem::Mismatch { name: name(2), fields: vec!["rules"], found }])
    );

    // 대조: 같은 제외가 정말 적용된 판(원장의 P3)이면 범위 밖이다 — 범위를 정하는 것은 원장이다.
    let later = ledger_of_applied(&[&tracks, &other, &proposal]);
    assert!(matches!(
        classify(&later, board(1), &given(1, &[&tracks, &other, &proposal])),
        PathContinuity::OutsideCurrentScope { current: Decision::Excluded { .. } }
    ));
}

// ── 빠짐없이: 생략·불일치·중복은 성공이 아니다 ─────────────────────────────

#[test]
fn omitting_a_middle_board_is_refused_not_skipped() {
    let (tracks, excludes) = (policy(TRACKS), policy(EXCLUDES));
    let ledger = ledger(&[&tracks, &excludes, &tracks]);
    // P2 를 건너뛴 자료라면 「계속 추적」이 나왔을 것이다. 그 답을 내지 않는다.
    let without_p2 = BoardPolicies::new().with(name(1), &tracks).with(name(3), &tracks);
    assert_eq!(
        refused(&ledger, board(1), &without_p2),
        LedgerSpanProblem::Policies(vec![BoardPolicyProblem::Missing { name: name(2) }])
    );
    // 빠진 판은 모두 알린다. 적용 정책도 예외가 아니다.
    let only_p1 = BoardPolicies::new().with(name(1), &tracks);
    assert_eq!(
        refused(&ledger, board(1), &only_p1),
        LedgerSpanProblem::Policies(vec![
            BoardPolicyProblem::Missing { name: name(2) },
            BoardPolicyProblem::Missing { name: name(3) },
        ])
    );
    assert_eq!(
        refused(&ledger, LedgerStart::BeforePolicyAdoption, &BoardPolicies::new()),
        LedgerSpanProblem::Policies((1..=3).map(|at| BoardPolicyProblem::Missing { name: name(at) }).collect())
    );
}

#[test]
fn a_mismatched_or_repeated_policy_is_refused() {
    let (tracks, excludes, other) = (policy(TRACKS), policy(EXCLUDES), policy(TRACKS_OTHER));
    let ledger = ledger(&[&excludes, &tracks]);

    // 시작 판보다 앞선 판이라도 건넨 자료가 틀리면 거절한다.
    let wrong_p1 = given(1, &[&other, &tracks]);
    let found = other.identity().rules_digest().clone();
    assert_eq!(
        refused(&ledger, board(2), &wrong_p1),
        LedgerSpanProblem::Policies(vec![BoardPolicyProblem::Mismatch { name: name(1), fields: vec!["rules"], found }])
    );
    // 같은 판에 두 번 — 같은 값이어도 어느 것을 쓸지 고르지 않는다.
    let twice = given(1, &[&excludes, &tracks]).with(name(2), &tracks);
    assert_eq!(
        refused(&ledger, board(1), &twice),
        LedgerSpanProblem::Policies(vec![BoardPolicyProblem::GivenTwice { name: name(2) }])
    );
    // 이름마다 하나씩 입력 순서로(원장 밖·불일치), 그 뒤에 구간 순서의 누락.
    let several = BoardPolicies::new().with(name(9), &tracks).with(name(1), &tracks);
    assert_eq!(
        refused(&ledger, board(1), &several),
        LedgerSpanProblem::Policies(vec![
            BoardPolicyProblem::NotInLedger { name: name(9) },
            BoardPolicyProblem::Mismatch {
                name: name(1),
                fields: vec!["rules"],
                found: tracks.identity().rules_digest().clone(),
            },
            BoardPolicyProblem::Missing { name: name(2) },
        ])
    );
}

#[test]
fn valid_policies_in_any_order_choose_the_same_span() {
    let boards = [policy(TRACKS), policy(EXCLUDES), policy(REINCLUDES), policy(EXCLUDES_CSV), policy(TRACKS)];
    let refs: Vec<&ApprovedPolicy> = boards.iter().collect();
    let ledger = ledger(&refs);
    let in_order = given(1, &refs);
    // 원장과 다른 입력 순서 — 거꾸로, 뒤섞음. 판 선택과 구간 순서는 원장의 것이어야 한다.
    let orders: [[u32; 5]; 2] = [[5, 4, 3, 2, 1], [3, 1, 5, 2, 4]];
    let paths: [(&str, PathKind); 4] =
        [(PATH, PathKind::File), ("data", PathKind::Directory), ("tmp/x.txt", PathKind::File), ("notes.md", PathKind::File)];
    let mut compared = 0;
    for order in orders {
        let shuffled = order
            .iter()
            .fold(BoardPolicies::new(), |acc, at| acc.with(name(*at), refs[*at as usize - 1]));
        let starts = std::iter::once(LedgerStart::BeforePolicyAdoption).chain((1..=5).map(board));
        for start in starts {
            let expected = select_ledger_span(&ledger, start, &in_order).unwrap();
            let got = select_ledger_span(&ledger, start, &shuffled).unwrap();
            assert_eq!(got.boards(), expected.boards(), "{order:?} {start}");
            assert_eq!(got.applied_span().len(), expected.applied_span().len());
            assert_eq!(got.applied_board(), name(5));
            for (text, kind) in paths {
                let at = path(text);
                assert_eq!(got.classify(&at, kind), expected.classify(&at, kind), "{order:?} {start} {text}");
                compared += 1;
            }
        }
    }
    assert_eq!(compared, 48);
}

#[test]
fn a_compound_error_keeps_its_order_and_reports_each_name_once() {
    let (excludes, tracks, other) = (policy(EXCLUDES), policy(TRACKS), policy(TRACKS_OTHER));
    let ledger = ledger(&[&excludes, &tracks, &other]);
    // 입력 순서: P2(틀림) · P9(원장 밖) · P1(틀림) · P9(다시) · P2(다시, 틀림). P3 은 건네지 않았다.
    let compound = BoardPolicies::new()
        .with(name(2), &other)
        .with(name(9), &tracks)
        .with(name(1), &tracks)
        .with(name(9), &tracks)
        .with(name(2), &other);
    assert_eq!(
        refused(&ledger, board(1), &compound),
        LedgerSpanProblem::Policies(vec![
            // 여러 번 건넨 P2 는 중복 한 번만 — 두 자료 모두 틀렸지만 정체성 비교를 하지 않는다. 누락도 아니다.
            BoardPolicyProblem::GivenTwice { name: name(2) },
            // 원장 밖 이름은 두 번 건넸어도 한 번만.
            BoardPolicyProblem::NotInLedger { name: name(9) },
            BoardPolicyProblem::Mismatch {
                name: name(1),
                fields: vec!["rules"],
                found: tracks.identity().rules_digest().clone(),
            },
            BoardPolicyProblem::Missing { name: name(3) },
        ])
    );
    // 중복된 판에서는 같은 올바른 자료를 두 번 건네도 어느 것도 고르지 않는다.
    let right_twice = given(1, &[&excludes, &tracks, &other]).with(name(3), &other);
    assert_eq!(
        refused(&ledger, board(1), &right_twice),
        LedgerSpanProblem::Policies(vec![BoardPolicyProblem::GivenTwice { name: name(3) }])
    );
}

#[test]
fn a_huge_board_number_does_not_size_the_lookup() {
    let (tracks, other) = (policy(TRACKS), policy(TRACKS_OTHER));
    let ledger = ledger(&[&tracks, &other]);
    let huge = given(1, &[&tracks, &other])
        .with(name(u32::MAX), &tracks)
        .with(name(u32::MAX - 1), &other);
    assert_eq!(
        refused(&ledger, board(1), &huge),
        LedgerSpanProblem::Policies(vec![
            BoardPolicyProblem::NotInLedger { name: name(u32::MAX) },
            BoardPolicyProblem::NotInLedger { name: name(u32::MAX - 1) },
        ])
    );
}

// ── 시작점: 정책 채택 전, 빈 원장, 없는 판 ─────────────────────────────────

#[test]
fn before_policy_adoption_spans_every_board_after_the_legacy_observation() {
    let (tracks, excludes) = (policy(TRACKS), policy(EXCLUDES));
    let before = LedgerStart::BeforePolicyAdoption;

    let one = ledger(&[&tracks]);
    let span = select_ledger_span(&one, before, &given(1, &[&tracks])).unwrap();
    assert_eq!((span.start(), span.boards(), span.applied_span().len()), (before, &[name(1)][..], 2));
    assert_eq!((span.board_at(0), span.board_at(1), span.board_at(2)), (None, Some(name(1)), None));
    assert_eq!(span.classify(&path(PATH), PathKind::File), continuous());

    let two = ledger(&[&excludes, &tracks]);
    let span = select_ledger_span(&two, before, &given(1, &[&excludes, &tracks])).unwrap();
    let answer = span.classify(&path(PATH), PathKind::File);
    assert_eq!(answer, PathContinuity::NewlyTracked { current: Decision::Tracked { by: None }, not_tracked_at: vec![1] });
    assert_eq!(span.board_at(1), Some(name(1)), "정책 채택 전 다음 자리가 P1 이다");
    // 판 이름으로 P1 에서 시작하는 것과 정책 채택 전 시작은 다른 구간이다.
    assert_eq!(select_ledger_span(&two, board(1), &given(1, &[&excludes, &tracks])).unwrap().applied_span().len(), 2);
}

#[test]
fn an_empty_ledger_keeps_the_existing_span_boundary() {
    let empty = PolicyLedger::empty();
    let before = LedgerStart::BeforePolicyAdoption;
    let problem = refused(&empty, before, &BoardPolicies::new());
    assert_eq!(problem, LedgerSpanProblem::Span(SpanProblem::NoAppliedPolicy));
    assert_eq!(problem.to_string(), SpanProblem::NoAppliedPolicy.to_string(), "3n 의 경계 그대로");

    assert_eq!(
        refused(&empty, board(1), &BoardPolicies::new()),
        LedgerSpanProblem::StartNotInLedger { start: name(1), boards: 0 }
    );
    let stray = policy(TRACKS);
    assert_eq!(
        refused(&empty, before, &BoardPolicies::new().with(name(1), &stray)),
        LedgerSpanProblem::Policies(vec![BoardPolicyProblem::NotInLedger { name: name(1) }]),
        "빈 원장에 판을 끼워 넣을 수 없다"
    );
}

#[test]
fn a_start_board_missing_from_the_ledger_is_refused() {
    let (tracks, other) = (policy(TRACKS), policy(TRACKS_OTHER));
    let ledger = ledger(&[&tracks, &other]);
    let policies = given(1, &[&tracks, &other]);
    assert_eq!(refused(&ledger, board(3), &policies), LedgerSpanProblem::StartNotInLedger { start: name(3), boards: 2 });
    assert!(select_ledger_span(&ledger, board(3), &policies).is_err());
}

// ── 3n 그대로 ──────────────────────────────────────────────────────────────

#[test]
fn the_answer_is_3n_over_the_whole_ledger_span() {
    let boards = [policy(TRACKS), policy(EXCLUDES), policy(REINCLUDES), policy(EXCLUDES_CSV), policy(TRACKS)];
    let refs: Vec<&ApprovedPolicy> = boards.iter().collect();
    let ledger = ledger(&refs);
    let policies = given(1, &refs);
    let paths: [(&str, PathKind); 5] = [
        (PATH, PathKind::File),
        ("data", PathKind::Directory),
        ("tmp/x.txt", PathKind::File),
        ("notes.md", PathKind::File),
        (".git/config", PathKind::File),
    ];
    let starts = std::iter::once(LedgerStart::BeforePolicyAdoption).chain((1..=5).map(board));
    let mut compared = 0;
    for start in starts {
        let (confirmed, later) = match start {
            LedgerStart::BeforePolicyAdoption => (ConfirmedStart::BeforePolicyAdoption, refs.clone()),
            LedgerStart::Board(at) => {
                let first = at.number() as usize - 1;
                (ConfirmedStart::Policy(refs[first]), refs[first + 1..].to_vec())
            }
        };
        let by_hand = AppliedSpan::new(confirmed, later).unwrap();
        let chosen = select_ledger_span(&ledger, start, &policies).unwrap();
        assert_eq!(chosen.applied_span().len(), by_hand.len(), "{start}");
        assert_eq!(chosen.applied_board(), name(5));
        for (text, kind) in paths {
            let at = path(text);
            let expected = classify_path_continuity(&by_hand, &at, kind);
            assert_eq!(chosen.classify(&at, kind), expected, "{start} {text}");
            assert_eq!(classify_path_over_ledger(&ledger, start, &policies, &at, kind), Ok(expected));
            compared += 1;
        }
    }
    assert_eq!(compared, 30);
}

// ── 보장 범위와 문장 ───────────────────────────────────────────────────────

#[test]
fn the_selection_neither_reads_nor_reinterprets() {
    let text = include_str!("ledger_span.rs");
    for word in [
        "std::fs", "std::io", "std::path", "fs::", "io::", "ObjectStore", "ProjectSession", "SnapshotRegistry",
        "read_root_policy_file", "evaluate_tree", "check_policy_sources", "PolicySourceLookup", "ProvidedSources",
        "rules_source", "read_blob", "digest_of", "compile(", "decide(", "Baseline", "Event", "static mut", "Cell",
    ] {
        assert!(!text.contains(word), "ledger_span.rs 가 {word:?} 를 쓴다");
    }
    for reused in ["AppliedSpan::new(", "classify_path_continuity(&self.span"] {
        assert!(text.contains(reused), "3n 을 그대로 써야 한다: {reused}");
    }
    // 건넨 자료는 색인에서 한 번만 훑는다 — 판마다 입력을 다시 훑는 길이 없다.
    assert_eq!(text.matches("for (name, policy) in &self.given").count(), 1, "색인 한 번");
    for rescan in ["policies.given", "given[..", "given.iter()"] {
        assert!(!text.contains(rescan), "ledger_span.rs 가 입력을 다시 훑는다: {rescan}");
    }
}

#[test]
fn success_does_not_claim_what_was_not_checked() {
    for said in ["최신 원장", "§6.1", "모니터 승인", "기준점", "다시 읽거나 해석", "O-2", "O-3"] {
        assert!(LEDGER_SPAN_DOES_NOT_SHOW.iter().any(|line| line.contains(said)), "{said} 가 빠졌다");
    }
    assert!(LEDGER_SPAN_DOES_NOT_SHOW[0].contains("빠짐없이 썼다"));
}

#[test]
fn every_problem_reads_as_a_sentence() {
    let tracks = policy(TRACKS);
    let found = tracks.identity().rules_digest().clone();
    let samples: [(LedgerSpanProblem, &str); 6] = [
        (LedgerSpanProblem::StartNotInLedger { start: name(3), boards: 2 }, "policy:P3"),
        (LedgerSpanProblem::Span(SpanProblem::NoAppliedPolicy), "정책 채택 전"),
        (LedgerSpanProblem::Policies(vec![BoardPolicyProblem::NotInLedger { name: name(4) }]), "승인 전 제안"),
        (LedgerSpanProblem::Policies(vec![BoardPolicyProblem::GivenTwice { name: name(1) }]), "두 번"),
        (LedgerSpanProblem::Policies(vec![BoardPolicyProblem::Missing { name: name(2) }]), "건너뛰고"),
        (
            LedgerSpanProblem::Policies(vec![BoardPolicyProblem::Mismatch { name: name(2), fields: vec!["rules"], found }]),
            "rules",
        ),
    ];
    for (problem, said) in samples {
        let text = problem.to_string();
        assert!(text.contains(said) && !text.contains('{'), "{problem:?} → {text:?}");
    }
    assert_eq!(LedgerStart::BeforePolicyAdoption.to_string(), "정책 채택 전");
    assert_eq!(board(2).to_string(), "policy:P2");
}
