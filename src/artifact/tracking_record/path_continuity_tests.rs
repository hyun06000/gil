//! 계속 추적 판정(「3n」)의 시험 — **합성 자료**만 쓴다.
//!
//! 정책 원문은 시험이 만든 바이트이고 모니터가 승인한 판이 아니다. 경로는 글자뿐이며 어떤 폴더·파일·원장·`state.yaml` 도
//! 열지 않는다. 핵심 사례는 **같은 경로 하나**(`data/keep.csv`)를 그대로 두고 건넨 정책 구간만 바꾼다 — 답이 달라졌다면
//! 그것은 구간의 차이에서만 나온 것이다.

use super::path_continuity::legacy_observation_contains;
use super::*;
use crate::artifact::EntryPath;
use crate::artifact::gilignore::{ApprovedPolicy, Decision, EngineVersion, PathKind, Protection};

// ── 도구 ───────────────────────────────────────────────────────────────────

/// 모든 사례가 묻는 하나의 경로.
const PATH: &str = "data/keep.csv";

/// 그 경로를 건드리지 않는 판 — 규칙이 있지만 `data/keep.csv` 에는 맞지 않는다.
const TRACKS: &[u8] = b"tmp/\n";
/// 다른 바이트의 「건드리지 않는」 판 — 같은 원문의 재사용과 구분하려고 둔다.
const TRACKS_OTHER: &[u8] = b"build/\n*.log\n";
/// 그 경로를 부모 폴더째 제외하는 판.
const EXCLUDES: &[u8] = b"data/\n";
/// 부모 폴더를 제외한 뒤 그 경로만 다시 포함하는 판 — 재포함 해석은 엔진의 몫이다.
const REINCLUDES: &[u8] = b"data/\n!data/keep.csv\n";
/// 다른 규칙으로 그 경로를 제외하는 판.
const EXCLUDES_CSV: &[u8] = b"*.csv\n";

fn policy(bytes: &[u8]) -> ApprovedPolicy {
    ApprovedPolicy::compile(EngineVersion::V1, bytes).expect("시험 원문은 해석된다")
}

fn path(text: &str) -> EntryPath {
    EntryPath::parse(text).expect("시험 경로는 경로 계약을 지킨다")
}

fn span<'a>(start: ConfirmedStart<'a>, later: &[&'a ApprovedPolicy]) -> AppliedSpan<'a> {
    AppliedSpan::new(start, later.to_vec()).expect("적용 정책이 있는 구간이다")
}

fn classify(span: &AppliedSpan<'_>) -> PathContinuity {
    classify_path_continuity(span, &path(PATH), PathKind::File)
}

fn tracked_by_nothing() -> Decision {
    Decision::Tracked { by: None }
}

fn newly(not_tracked_at: Vec<usize>, current: Decision) -> PathContinuity {
    PathContinuity::NewlyTracked { current, not_tracked_at }
}

// ── 요구 사례: 같은 경로, 다른 정책 이력 ───────────────────────────────────

#[test]
fn tracked_in_every_board_is_continuous() {
    let (p1, p2, p3) = (policy(TRACKS), policy(REINCLUDES), policy(TRACKS_OTHER));
    let got = classify(&span(ConfirmedStart::Policy(&p1), &[&p2, &p3]));
    assert_eq!(got, PathContinuity::ContinuouslyTracked { current: tracked_by_nothing() });
    assert!(got.is_comparison_target());
}

#[test]
fn excluded_then_included_again_is_newly_tracked() {
    let (p1, p2, p3) = (policy(TRACKS), policy(EXCLUDES), policy(REINCLUDES));
    let got = classify(&span(ConfirmedStart::Policy(&p1), &[&p2, &p3]));
    let current = p3.decide(&path(PATH), PathKind::File);
    assert!(matches!(&current, Decision::Tracked { by: Some(rule) } if rule.line == 2 && rule.negated));
    assert_eq!(got, newly(vec![1], current));
    assert!(!got.is_comparison_target());
}

#[test]
fn same_first_and_last_board_still_changes_with_a_middle_exclusion() {
    // 확정 정책과 적용 정책이 같은 원문이다. 가운데 판 하나만 다르다.
    let (first, middle_tracks, middle_excludes, last) =
        (policy(TRACKS), policy(TRACKS_OTHER), policy(EXCLUDES_CSV), policy(TRACKS));
    assert_eq!(first.identity(), last.identity(), "처음과 마지막은 같은 원문의 판이다");

    let kept = classify(&span(ConfirmedStart::Policy(&first), &[&middle_tracks, &last]));
    let broken = classify(&span(ConfirmedStart::Policy(&first), &[&middle_excludes, &last]));

    assert_eq!(kept, PathContinuity::ContinuouslyTracked { current: tracked_by_nothing() });
    assert_eq!(broken, newly(vec![1], tracked_by_nothing()));
    // 끝만 보는 판정이었다면 둘이 같았을 것이다.
    assert_eq!(kept.current(), broken.current());
}

#[test]
fn first_inclusion_is_newly_tracked() {
    let (p1, p2) = (policy(EXCLUDES), policy(REINCLUDES));
    let got = classify(&span(ConfirmedStart::Policy(&p1), &[&p2]));
    assert_eq!(got, newly(vec![0], p2.decide(&path(PATH), PathKind::File)));
}

#[test]
fn every_untracked_position_is_listed_in_order() {
    let (p1, p2, p3, p4) = (policy(EXCLUDES), policy(TRACKS), policy(EXCLUDES_CSV), policy(TRACKS));
    let got = classify(&span(ConfirmedStart::Policy(&p1), &[&p2, &p3, &p4]));
    assert_eq!(got, newly(vec![0, 2], tracked_by_nothing()));
}

#[test]
fn excluded_by_the_applied_policy_is_outside_the_current_scope() {
    let (p1, p2) = (policy(TRACKS), policy(EXCLUDES));
    let got = classify(&span(ConfirmedStart::Policy(&p1), &[&p2]));
    let PathContinuity::OutsideCurrentScope { current: Decision::Excluded { rule } } = &got else {
        panic!("현재 범위 밖이어야 한다: {got:?}");
    };
    assert_eq!((rule.line, rule.pattern.as_str(), rule.negated), (1, "data/", false));
    assert_eq!(got.current(), &p2.decide(&path(PATH), PathKind::File));
    assert!(!got.is_comparison_target());
}

#[test]
fn a_single_board_span_answers_from_that_board_alone() {
    let (tracks, excludes) = (policy(TRACKS), policy(EXCLUDES));
    assert_eq!(
        classify(&span(ConfirmedStart::Policy(&tracks), &[])),
        PathContinuity::ContinuouslyTracked { current: tracked_by_nothing() }
    );
    assert!(matches!(
        classify(&span(ConfirmedStart::Policy(&excludes), &[])),
        PathContinuity::OutsideCurrentScope { current: Decision::Excluded { .. } }
    ));
    let only = span(ConfirmedStart::Policy(&tracks), &[]);
    assert_eq!(only.len(), 1);
    assert!(std::ptr::eq(only.applied(), &tracks), "한 판짜리 구간의 적용 정책은 그 확정 정책이다");
}

#[test]
fn a_proposal_that_excludes_does_not_remove_a_continuous_path() {
    let (p1, p2) = (policy(TRACKS), policy(TRACKS_OTHER));
    let proposal = policy(EXCLUDES); // 승인 저장 전의 제안 — 구간에 넣지 않는다
    assert!(!proposal.decide(&path(PATH), PathKind::File).is_tracked(), "제안은 그 경로를 빼려 한다");

    let before_approval = span(ConfirmedStart::Policy(&p1), &[&p2]);
    let got = classify(&before_approval);
    assert_eq!(got, PathContinuity::ContinuouslyTracked { current: tracked_by_nothing() });
    assert!(got.is_comparison_target(), "승인 전 변경 검사 대상으로 남는다");

    // 대조: 같은 판을 「적용된 판」으로 건네면 그때는 현재 범위 밖이다 — 차이는 입력에 넣었는가뿐이다.
    let if_applied = classify(&span(ConfirmedStart::Policy(&p1), &[&p2, &proposal]));
    assert!(matches!(if_applied, PathContinuity::OutsideCurrentScope { .. }));
}

// ── 정책 채택 전 기록 ──────────────────────────────────────────────────────

#[test]
fn before_policy_adoption_the_start_is_the_legacy_observation() {
    let (tracks, excludes, reincludes) = (policy(TRACKS), policy(EXCLUDES), policy(REINCLUDES));
    let before = ConfirmedStart::BeforePolicyAdoption;

    assert_eq!(
        classify(&span(before, &[&tracks])),
        PathContinuity::ContinuouslyTracked { current: tracked_by_nothing() }
    );
    assert!(matches!(
        classify(&span(before, &[&excludes])),
        PathContinuity::OutsideCurrentScope { current: Decision::Excluded { .. } }
    ));
    assert_eq!(
        classify(&span(before, &[&excludes, &reincludes])),
        newly(vec![1], reincludes.decide(&path(PATH), PathKind::File))
    );
}

#[test]
fn before_policy_adoption_without_an_applied_board_is_refused() {
    let refused = AppliedSpan::new(ConfirmedStart::BeforePolicyAdoption, Vec::new()).unwrap_err();
    assert_eq!(refused, SpanProblem::NoAppliedPolicy);
    let said = refused.to_string();
    assert!(said.contains("정책 채택 전") && said.contains("현행 규칙"), "{said}");
}

#[test]
fn the_legacy_observation_is_not_an_empty_policy() {
    let empty = policy(b"");
    // 기존 관측은 루트의 `.gil` 디렉터리만 건너뛴다. 빈 정책은 `.git`·루트 `.gilignore` 도 시스템 보호로 뺀다.
    let cases: [(&str, PathKind, bool, bool); 9] = [
        (".gil/state.yaml", PathKind::File, false, false),
        (".gil", PathKind::Directory, false, false),
        (".gil", PathKind::File, true, true),
        ("deep/.gil/state.yaml", PathKind::File, false, false),
        (".git/config", PathKind::File, true, false),
        (".gilignore", PathKind::File, true, false),
        ("sub/.gilignore", PathKind::File, true, false),
        (".GIL/state.yaml", PathKind::File, true, false),
        (PATH, PathKind::File, true, true),
    ];
    for (text, kind, legacy, by_empty) in cases {
        let at = path(text);
        assert_eq!(legacy_observation_contains(&at, kind), legacy, "기존 관측 {text} {kind:?}");
        assert_eq!(empty.decide(&at, kind).is_tracked(), by_empty, "빈 정책 {text} {kind:?}");
    }
}

// ── 엔진의 구분을 그대로 둔다 ──────────────────────────────────────────────

#[test]
fn system_protection_and_boundaries_keep_the_engine_decision() {
    let (p1, p2) = (policy(TRACKS), policy(b"!**/config\n!*ignore\n"));
    let cases: [(&str, PathKind); 6] = [
        (".git/config", PathKind::File),
        (".gil/state.yaml", PathKind::File),
        (".gilignore", PathKind::File),
        ("sub/.gilignore", PathKind::File),
        ("sub/.gil/state.yaml", PathKind::File),
        ("sub/.gil", PathKind::Directory),
    ];
    for start in [ConfirmedStart::BeforePolicyAdoption, ConfirmedStart::Policy(&p1)] {
        let span = span(start, &[&p2]);
        for (text, kind) in cases {
            let at = path(text);
            let got = classify_path_continuity(&span, &at, kind);
            assert_eq!(got, PathContinuity::OutsideCurrentScope { current: p2.decide(&at, kind) }, "{text}");
        }
    }
    let span = span(ConfirmedStart::Policy(&p1), &[&p2]);
    let at = |text| classify_path_continuity(&span, &path(text), PathKind::File).current().clone();
    assert!(matches!(at(".git/config"), Decision::Protected { protection: Protection::GitMetadata, .. }));
    assert!(matches!(at(".gil/state.yaml"), Decision::Protected { protection: Protection::InternalStore, .. }));
    assert!(matches!(at(".gilignore"), Decision::Protected { protection: Protection::PolicyFile, .. }));
    assert!(matches!(at("sub/.gilignore"), Decision::NestedPolicyFile { .. }));
    assert!(matches!(at("sub/.gil/state.yaml"), Decision::NestedGilBoundary { .. }));
}

#[test]
fn the_path_kind_is_handed_to_the_engine_unchanged() {
    // `data/` 는 디렉터리 전용 규칙이다 — 같은 글자 `data` 라도 파일이면 맞지 않는다(엔진의 해석).
    let (p1, p2) = (policy(TRACKS), policy(b"data/\n"));
    let span = span(ConfirmedStart::Policy(&p1), &[&p2]);
    assert!(classify_path_continuity(&span, &path("data"), PathKind::File).is_comparison_target());
    assert!(matches!(
        classify_path_continuity(&span, &path("data"), PathKind::Directory),
        PathContinuity::OutsideCurrentScope { .. }
    ));
}

/// 작은 판 묶음의 모든 구간(길이 1–3, 두 시작)에서 결과가 **판마다의 엔진 판정만으로** 정해지는지 견준다.
#[test]
fn the_answer_is_exactly_the_engines_decisions_over_the_span() {
    let boards = [policy(TRACKS), policy(EXCLUDES), policy(REINCLUDES), policy(EXCLUDES_CSV)];
    let paths = [(PATH, PathKind::File), ("data", PathKind::Directory), ("tmp/x", PathKind::File), ("a.csv", PathKind::File)];
    let mut sequences: Vec<Vec<usize>> = Vec::new();
    for a in 0..boards.len() {
        sequences.push(vec![a]);
        for b in 0..boards.len() {
            sequences.push(vec![a, b]);
            for c in 0..boards.len() {
                sequences.push(vec![a, b, c]);
            }
        }
    }
    let mut seen = [0usize; 3];
    for sequence in &sequences {
        for adopted_before in [false, true] {
            let picked: Vec<&ApprovedPolicy> = sequence.iter().map(|&i| &boards[i]).collect();
            let (start, later, legacy_at_zero) = match adopted_before {
                true => (ConfirmedStart::BeforePolicyAdoption, picked.clone(), true),
                false => (ConfirmedStart::Policy(picked[0]), picked[1..].to_vec(), false),
            };
            let span = span(start, &later);
            for (text, kind) in paths {
                let at = path(text);
                let mut contains: Vec<bool> = Vec::new();
                if legacy_at_zero {
                    contains.push(legacy_observation_contains(&at, kind));
                }
                contains.extend(picked.iter().map(|board| board.decide(&at, kind).is_tracked()));
                let current = picked.last().unwrap().decide(&at, kind);
                let missing: Vec<usize> = contains[..contains.len() - 1]
                    .iter()
                    .enumerate()
                    .filter(|(_, inside)| !**inside)
                    .map(|(at, _)| at)
                    .collect();
                let want = match (current.is_tracked(), missing.is_empty()) {
                    (false, _) => PathContinuity::OutsideCurrentScope { current },
                    (true, true) => PathContinuity::ContinuouslyTracked { current },
                    (true, false) => PathContinuity::NewlyTracked { current, not_tracked_at: missing },
                };
                let got = classify_path_continuity(&span, &at, kind);
                seen[match &got {
                    PathContinuity::ContinuouslyTracked { .. } => 0,
                    PathContinuity::NewlyTracked { .. } => 1,
                    PathContinuity::OutsideCurrentScope { .. } => 2,
                }] += 1;
                assert_eq!(got, want, "{sequence:?} before={adopted_before} {text}");
            }
        }
    }
    assert!(seen.iter().all(|count| *count > 0), "세 갈래가 모두 나와야 한다: {seen:?}");
}

// ── 보장 범위와 문장 ───────────────────────────────────────────────────────

#[test]
fn the_classifier_has_no_way_to_touch_files_or_records() {
    let text = include_str!("path_continuity.rs");
    for word in [
        "std::fs", "std::io", "std::path", "fs::", "io::", "ObjectStore", "ProjectSession", "SnapshotRegistry",
        "PolicyLedger", "read_root_policy_file", "evaluate_tree", "observe(", "static mut", "Cell",
    ] {
        assert!(!text.contains(word), "path_continuity.rs 가 {word:?} 를 쓴다");
    }
}

#[test]
fn success_does_not_claim_what_was_not_checked() {
    for said in ["완전성", "승인", "확정 정책", "내용", "제외 기간", "O-2", "O-3"] {
        assert!(CONTINUITY_DOES_NOT_SHOW.iter().any(|line| line.contains(said)), "{said} 가 빠졌다");
    }
    assert!(CONTINUITY_DOES_NOT_SHOW[0].contains("건넨 구간에 대한 판정"));
}

#[test]
fn every_result_reads_as_a_sentence() {
    let (p1, p2, p3) = (policy(TRACKS), policy(EXCLUDES), policy(REINCLUDES));
    let continuous = classify(&span(ConfirmedStart::Policy(&p1), &[]));
    let newly = classify(&span(ConfirmedStart::Policy(&p1), &[&p2, &p3]));
    let outside = classify(&span(ConfirmedStart::Policy(&p1), &[&p2]));
    let samples = [(continuous, "견줄 대상"), (newly, "1번 자리"), (outside, "data/")];
    for (result, said) in samples {
        let text = result.to_string();
        assert!(text.contains(said) && !text.contains('{'), "{result:?} → {text:?}");
    }
}
