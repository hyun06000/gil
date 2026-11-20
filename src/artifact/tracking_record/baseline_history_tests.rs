//! 기준점과 보관 시도 결과 순서의 두 시점 비교(「3f」) 시험 — **합성 입력**만 쓴다.
//!
//! 여기의 사건 칸·시도 결과·등록부는 시험이 만든 값이다. 실제 보관, 모니터 승인, 사건 기록, `state.yaml`,
//! `gil start` 를 쓰지 않는다. 창고를 쓰는 시험은 프로세스 번호가 붙은 임시 폴더를 새로 만든다.
//!
//! 「전후 각각 유효한 변경」을 거절하는 시험은 3e 의 [`check_baselines`] 가 두 시점을 **실제로 통과시킨다**는 것을 먼저
//! 확인한다 — 이 비교가 한 시점 검사와 다른 문제를 잡는다는 근거다.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::*;
use crate::artifact::{Manifest, ManifestAddress, ObjectStore, SnapshotRegistry};
use crate::refs::{CycleRef, SnapshotRef};

use AttemptSeen::{Archived as A, NotArchived as F};
use BaselineSlot::{Unwritten, Written};
use EventKind::{RestoreResult, ScopeChange, Start};

// ── 도구 ───────────────────────────────────────────────────────────────────

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

/// 기준점이 없는 사건 — 경계·발급기 칸이 없다.
fn waiting(event: u32, kind: EventKind, attempts: &[AttemptSeen]) -> BaselineFacts {
    BaselineFacts::new(m(event), kind, None, Unwritten, Unwritten, attempts.to_vec())
}

/// 기준점과 경계·발급기 칸이 모두 적힌 사건. `boundary: None` 은 값이 `null` 로 **적힌** 경계다.
fn fixed(event: u32, kind: EventKind, snapshot: u32, boundary: Option<u32>, issuer: u32, attempts: &[AttemptSeen]) -> BaselineFacts {
    BaselineFacts::new(m(event), kind, Some(a(snapshot)), Written(boundary.map(c)), Written(issuer), attempts.to_vec())
}

/// §5.1 예시의 `start` — `after_cycle_ref: null`, `next_cycle_id: 1`, `snapshot_ref: A1`, `[{result: archived}]`.
fn start() -> BaselineFacts {
    fixed(1, Start, 1, None, 1, &[A])
}

fn kept(before: &[BaselineFacts], after: &[BaselineFacts], registry: &SnapshotRegistry) -> BaselineHistoryKept {
    check_baseline_history((before, registry), (after, registry))
        .unwrap_or_else(|problems| panic!("받아들여야 한다: {problems:#?}"))
}

fn refused(before: &[BaselineFacts], after: &[BaselineFacts], registry: &SnapshotRegistry) -> Vec<BaselineHistoryProblem> {
    check_baseline_history((before, registry), (after, registry)).expect_err("거절해야 한다")
}

/// 3e 의 한 시점 검사가 두 시점을 **각각 통과시킨다** — 아래 거절은 한 시점 검사로는 보이지 않는다.
fn each_point_passes_alone(before: &[BaselineFacts], after: &[BaselineFacts], registry: &SnapshotRegistry) {
    assert!(check_baselines(before, registry).is_ok(), "변경 전이 3e 를 통과해야 한다: {before:#?}");
    assert!(check_baselines(after, registry).is_ok(), "변경 후가 3e 를 통과해야 한다: {after:#?}");
}

/// 앞선 기준점 보존 위반만 나왔다.
fn past_changes(before: &[BaselineFacts], after: &[BaselineFacts], registry: &SnapshotRegistry) -> Vec<BaselineChange> {
    match refused(before, after, registry).as_slice() {
        [BaselineHistoryProblem::PastBaselines(changes)] => changes.clone(),
        other => panic!("보존 위반 하나만 나와야 한다: {other:#?}"),
    }
}

/// 입력 대응 문제만 나왔다.
fn unmatched(before: &[BaselineFacts], after: &[BaselineFacts], registry: &SnapshotRegistry) -> Vec<MatchProblem> {
    match refused(before, after, registry).as_slice() {
        [BaselineHistoryProblem::Unmatched(found)] => found.clone(),
        other => panic!("입력 대응 문제 하나만 나와야 한다: {other:#?}"),
    }
}

fn scratch(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gil-baseline-history-{}-{label}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("시험이 쓸 자리를 만들 수 있어야 한다");
    dir
}

/// 그 폴더 아래의 모든 파일과 바이트.
fn tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let key = path.strip_prefix(root).unwrap().to_path_buf();
            if path.is_dir() {
                out.insert(key, Vec::new());
                walk(root, &path, out);
            } else {
                out.insert(key, fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

// ── 정상 사례 — 제약을 지나치게 좁히지 않았는가 ───────────────────────────

#[test]
fn empty_and_unchanged_inputs_are_kept() {
    // §5.1: 정책을 채택하지 않은 프로젝트는 `events: []` 이다. 같은 값끼리의 비교는 변화 없음이다.
    let empty = kept(&[], &[], &SnapshotRegistry::new());
    assert!(empty.compared().is_empty() && empty.added().is_empty());

    let events = [start(), fixed(2, ScopeChange, 2, Some(4), 5, &[F, A]), waiting(3, ScopeChange, &[F])];
    let same = kept(&events, &events, &registry(2));
    assert_eq!(
        same.compared(),
        &[(m(1), BaselineUpdate::Unchanged), (m(2), BaselineUpdate::Unchanged), (m(3), BaselineUpdate::Unchanged)]
    );
    assert!(same.added().is_empty());
}

#[test]
fn a_wait_may_stay_waiting_and_gain_any_number_of_failed_attempts() {
    // §5.3 「보관 대기 | 보관 시도 | 실패 | 실패 시도 기록(가능하면), 보관 대기 유지」, §6.3 재시도에 횟수 제한이 없다.
    // 두 관측 사이에 실패가 여러 번 기록될 수 있으므로 「한 번에 하나」를 요구하지 않는다.
    for earlier in [vec![], vec![F], vec![F, F]] {
        for added in [1usize, 2, 7, 300] {
            let mut later = earlier.clone();
            later.extend(vec![F; added]);
            let before = [start(), waiting(2, ScopeChange, &earlier)];
            let after = [start(), waiting(2, ScopeChange, &later)];
            let found = kept(&before, &after, &registry(1));
            assert_eq!(found.compared()[1], (m(2), BaselineUpdate::AttemptsAdded { added }), "{earlier:?} + {added}");
        }
    }
}

#[test]
fn a_first_success_after_kept_failures_fixes_the_baseline() {
    // §6.5 (가): 실패가 쌓인 M2 를 인터뷰 C5 를 지난 경계에서 다시 시도해 성공 — 경계 C5·발급기 6·A6 을 함께 적는다.
    // 앞선 실패 기록은 그대로이고, 두 관측 사이에 실패가 더 있었어도 된다.
    let cases = [
        (vec![], vec![A], 0),
        (vec![F], vec![F, A], 0),
        (vec![F, F], vec![F, F, F, A], 1),
        (vec![F], vec![F, F, F, F, A], 3),
    ];
    for (earlier, later, added_before_archived) in cases {
        let before = [start(), waiting(2, ScopeChange, &earlier)];
        let after = [start(), fixed(2, ScopeChange, 6, Some(5), 6, &later)];
        let found = kept(&before, &after, &registry(6));
        assert_eq!(found.compared()[1], (m(2), BaselineUpdate::Fixed { added_before_archived }), "{later:?}");
    }
}

#[test]
fn a_new_event_may_already_be_fixed_when_first_seen() {
    // §5.1 「추가만 한다」: 새 사건은 끝에 붙는다. 두 관측 사이에 저장 1 과 저장 2 가 모두 일어났으면 처음 보일 때 이미
    // 기준점이 있다(§5.8). `start` 는 언제나 기준점과 함께 생긴다(§5.3).
    let found = kept(&[], &[start()], &registry(1));
    assert_eq!(found.added(), &[m(1)]);
    let found = kept(&[start()], &[start(), fixed(2, ScopeChange, 2, Some(4), 5, &[F, F, A])], &registry(2));
    assert_eq!(found.compared(), &[(m(1), BaselineUpdate::Unchanged)]);
    assert_eq!(found.added(), &[m(2)]);
}

#[test]
fn a_new_waiting_event_may_be_added() {
    // §5.8 「저장 1 뒤」: 새 사건은 기준점 없이 생긴다. 처음 보일 때 실패 기록이 이미 있을 수도 있다.
    for attempts in [vec![], vec![F], vec![F, F, F]] {
        let found = kept(&[start()], &[start(), waiting(2, ScopeChange, &attempts)], &registry(1));
        assert_eq!(found.added(), &[m(2)]);
    }
    // 여러 사건이 한 번에 덧붙어도 된다.
    let after = [start(), waiting(2, ScopeChange, &[F]), fixed(3, ScopeChange, 1, Some(6), 7, &[A]), waiting(4, ScopeChange, &[])];
    assert_eq!(kept(&[start()], &after, &registry(1)).added(), &[m(2), m(3), m(4)]);
}

#[test]
fn the_null_boundary_of_start_is_kept_as_a_written_value() {
    // §5.1 예시: start 의 보관 경계는 `null` 로 **적힌** 값이다. 그대로 두면 변화 없음이다(값을 바꾸면 아래 거절 시험).
    let found = kept(&[start()], &[start()], &registry(1));
    assert_eq!(found.compared(), &[(m(1), BaselineUpdate::Unchanged)]);
}

#[test]
fn the_same_snapshot_named_by_different_events_is_kept() {
    // §5.5: 범위를 바꿔도 추적 세계가 같으면 같은 이름이 나온다. 보관본 재사용은 사건 중복이 아니다.
    let before = [start(), fixed(2, ScopeChange, 1, Some(3), 4, &[A])];
    let after = [
        start(),
        fixed(2, ScopeChange, 1, Some(3), 4, &[A]),
        fixed(3, ScopeChange, 1, Some(6), 7, &[F, A]),
        fixed(4, RestoreResult, 1, Some(2), 9, &[A]),
    ];
    let found = kept(&before, &after, &registry(1));
    assert_eq!(found.added(), &[m(3), m(4)]);
}

#[test]
fn a_replaced_earlier_event_left_as_it_was_does_not_block_new_events() {
    // §5.2·§6.5 (나): 보관 대기 중 다른 변경안을 승인하면 앞 사건은 기준점 없이 대체되어 그대로 남는다. 실패 기록도 남는다.
    let before = [start(), waiting(2, ScopeChange, &[F, F])];
    let after = [start(), waiting(2, ScopeChange, &[F, F]), fixed(3, ScopeChange, 7, Some(5), 6, &[A])];
    let found = kept(&before, &after, &registry(7));
    assert_eq!(found.compared()[1], (m(2), BaselineUpdate::Unchanged));
    assert_eq!(found.added(), &[m(3)]);
}

#[test]
fn each_point_is_checked_against_its_own_registry() {
    // 등록부는 시점마다 건넨다. 뒤 시점에 새 이름이 생긴 것은 정상이다. 두 등록부의 매핑 보존은 보지 않는다
    // (BASELINE_HISTORY_DOES_NOT_SHOW) — 같은 이름이 다른 주소를 가리켜도 이 비교의 결과는 같다.
    let before = [start(), waiting(2, ScopeChange, &[F])];
    let after = [start(), fixed(2, ScopeChange, 2, Some(4), 5, &[F, A])];
    assert!(check_baseline_history((&before, &registry(1)), (&after, &registry(2))).is_ok());

    let moved = SnapshotRegistry::restore(2, vec![(1, ManifestAddress::from_parts("sha256", &[9; 32]).unwrap())]).unwrap();
    assert_ne!(moved.resolve(a(1)), registry(1).resolve(a(1)));
    assert!(check_baseline_history((&[start()], &registry(1)), (&[start()], &moved)).is_ok());
    assert!(BASELINE_HISTORY_DOES_NOT_SHOW.iter().any(|line| line.contains("매핑")));
}

#[test]
fn a_replaced_event_gaining_a_baseline_is_not_judged_here() {
    // §5.2 는 대체된 사건을 다시 보관하지 않는다고 적는다. 그것은 사건의 **위치**(T5)와 실행 자격의 조건이고, 이 비교는
    // 기준점 칸이 정본의 꼴(최초 확정)로만 달라졌는지 볼 뿐이다. 그래서 받아들이며, 이 성공을 그 조건의 충족으로 읽지
    // 않는다고 BASELINE_HISTORY_DOES_NOT_SHOW 에 적는다.
    let before = [start(), waiting(2, ScopeChange, &[F]), waiting(3, ScopeChange, &[])];
    let after = [start(), fixed(2, ScopeChange, 1, Some(5), 6, &[F, A]), waiting(3, ScopeChange, &[F])];
    kept(&before, &after, &registry(1));
    assert!(BASELINE_HISTORY_DOES_NOT_SHOW.iter().any(|line| line.contains("재보관") && line.contains("T5")));
}

// ── 거절 사례 — 각 시점은 유효하지만 보존이 깨진 변경 ─────────────────────

#[test]
fn a_fixed_snapshot_swapped_for_another_valid_name_is_refused() {
    // 적힘 — T6 「한 번만 적는다」. 바뀐 이름도 등록부에 있으므로 한 시점으로는 보이지 않는다.
    let reg = registry(7);
    for (before, after) in [
        ([start(), fixed(2, ScopeChange, 6, Some(4), 5, &[A])], [start(), fixed(2, ScopeChange, 7, Some(4), 5, &[A])]),
        ([start(), fixed(2, ScopeChange, 6, Some(4), 5, &[A])], [fixed(1, Start, 2, None, 1, &[A]), fixed(2, ScopeChange, 6, Some(4), 5, &[A])]),
    ] {
        each_point_passes_alone(&before, &after, &reg);
        let changed = past_changes(&before, &after, &reg);
        assert!(
            matches!(changed.as_slice(), [BaselineChange::SnapshotReplaced { .. }]),
            "{changed:#?}"
        );
    }
    let before = [start(), fixed(2, ScopeChange, 6, Some(4), 5, &[A])];
    let after = [start(), fixed(2, ScopeChange, 7, Some(4), 5, &[A])];
    assert_eq!(
        past_changes(&before, &after, &reg),
        vec![BaselineChange::SnapshotReplaced { event: m(2), before: a(6), after: a(7) }]
    );
}

#[test]
fn a_fixed_boundary_or_issuer_swapped_for_another_value_is_refused() {
    // 적힘 — T6·§5.1 「`snapshot_ref` 와 함께 한 번만 적는다」. 값의 의미(T7)는 3e 가 보지 않으므로 두 시점 모두 통과한다.
    // `null` 경계는 적힌 값이다 — null 에서 사이클로, 사이클에서 null 로 바꾸는 것도 교체다.
    let reg = registry(6);
    let at = |boundary: Option<u32>, issuer: u32| [start(), fixed(2, ScopeChange, 6, boundary, issuer, &[F, A])];
    let cases = [
        (at(Some(4), 5), at(Some(5), 5), BaselineChange::BoundaryChanged { event: m(2), before: Some(c(4)), after: Written(Some(c(5))) }),
        (at(Some(4), 5), at(None, 5), BaselineChange::BoundaryChanged { event: m(2), before: Some(c(4)), after: Written(None) }),
        (at(None, 5), at(Some(4), 5), BaselineChange::BoundaryChanged { event: m(2), before: None, after: Written(Some(c(4))) }),
        (at(Some(4), 5), at(Some(4), 6), BaselineChange::IssuerChanged { event: m(2), before: 5, after: Written(6) }),
    ];
    for (before, after, change) in cases {
        each_point_passes_alone(&before, &after, &reg);
        assert_eq!(past_changes(&before, &after, &reg), vec![change]);
    }
    // 시작 사건의 null 경계를 사이클로 바꾼다.
    let after = [fixed(1, Start, 1, Some(3), 1, &[A])];
    each_point_passes_alone(&[start()], &after, &reg);
    assert_eq!(
        past_changes(&[start()], &after, &reg),
        vec![BaselineChange::BoundaryChanged { event: m(1), before: None, after: Written(Some(c(3))) }]
    );
}

#[test]
fn a_fixed_baseline_cleared_back_to_waiting_is_refused() {
    // 도출 — §5.3 에는 확정된 기준점을 대기로 되돌리는 전이가 없고 T1 은 「기록은 추가만 한다」. 기준점·두 칸·archived 를
    // 함께 지우면 뒤 시점은 올바른 보관 대기처럼 보인다.
    let reg = registry(6);
    let before = [start(), fixed(2, ScopeChange, 6, Some(4), 5, &[F, A])];
    let after = [start(), waiting(2, ScopeChange, &[F])];
    each_point_passes_alone(&before, &after, &reg);
    assert_eq!(
        past_changes(&before, &after, &reg),
        vec![
            BaselineChange::AttemptsRemoved { event: m(2), before: 2, after: 1 },
            BaselineChange::SnapshotCleared { event: m(2), before: a(6) },
            BaselineChange::BoundaryChanged { event: m(2), before: Some(c(4)), after: Unwritten },
            BaselineChange::IssuerChanged { event: m(2), before: 5, after: Unwritten },
        ]
    );
    // 시도까지 그대로 두고 지우면 3e 가 잡는다(archived 는 기준점이 있을 때만). 이 비교도 기준점 삭제를 따로 말한다.
    let after = [start(), waiting(2, ScopeChange, &[])];
    each_point_passes_alone(&before, &after, &reg);
    assert!(past_changes(&before, &after, &reg).contains(&BaselineChange::SnapshotCleared { event: m(2), before: a(6) }));
}

#[test]
fn earlier_failed_attempts_removed_or_truncated_are_refused() {
    // 도출 — 적힌 시도를 지우는 전이가 없다(§5.3, T1). 실패 기록이 「최선 노력」인 것은 **쓰지 못할 수 있다**는 뜻이지 적힌
    // 기록을 지워도 된다는 뜻이 아니다.
    let reg = registry(6);
    let before = [start(), waiting(2, ScopeChange, &[F, F, F])];
    for later in [vec![F, F], vec![F], vec![]] {
        let after = [start(), waiting(2, ScopeChange, &later)];
        each_point_passes_alone(&before, &after, &reg);
        assert_eq!(
            past_changes(&before, &after, &reg),
            vec![BaselineChange::AttemptsRemoved { event: m(2), before: 3, after: later.len() }]
        );
    }
    // 앞의 실패를 지우고 성공으로 확정 — 줄어든 자리에 archived 가 와서 바꿔치기와 삭제가 함께 보인다.
    let after = [start(), fixed(2, ScopeChange, 6, Some(4), 5, &[F, A])];
    each_point_passes_alone(&before, &after, &reg);
    assert_eq!(
        past_changes(&before, &after, &reg),
        vec![
            BaselineChange::AttemptChanged { event: m(2), position: 1, before: F, after: A },
            BaselineChange::AttemptsRemoved { event: m(2), before: 3, after: 2 },
        ]
    );
}

#[test]
fn a_failed_attempt_swapped_for_success_is_refused() {
    // 도출 — 적힌 시도는 고치지 않는다. 앞 시점의 실패를 뒤 시점에서 archived 로 바꾸면 뒤 시점은 「첫 시도에 성공」처럼
    // 보이고 3e 를 통과한다.
    let reg = registry(6);
    for (earlier, later, position) in [(vec![F], vec![A], 0), (vec![F, F], vec![F, A], 1), (vec![F, F], vec![A, F, A], 0)] {
        let before = [start(), waiting(2, ScopeChange, &earlier)];
        let after = [start(), fixed(2, ScopeChange, 6, Some(4), 5, &later)];
        let after_alone = check_baselines(&after, &reg);
        if later.iter().filter(|attempt| **attempt == A).count() == 1 {
            each_point_passes_alone(&before, &after, &reg);
        } else {
            assert!(after_alone.is_err(), "archived 둘은 3e 가 거절한다");
        }
        let found = refused(&before, &after, &reg);
        let changes = found.iter().find_map(|problem| match problem {
            BaselineHistoryProblem::PastBaselines(changes) => Some(changes.clone()),
            _ => None,
        });
        assert_eq!(
            changes.as_deref().and_then(|changes| changes.first()),
            Some(&BaselineChange::AttemptChanged { event: m(2), position, before: F, after: A }),
            "{found:#?}"
        );
    }
}

#[test]
fn attempts_after_a_fixed_baseline_are_refused_by_both_checks() {
    // 적힘 — §5.1 「마지막에 한 번」: 뒤 시점만 봐도 3e 가 거절한다. 두 시점 비교는 그것이 확정 **뒤의** 추가라는 사실을
    // 따로 말한다.
    let reg = registry(6);
    let before = [start(), fixed(2, ScopeChange, 6, Some(4), 5, &[A])];
    let after = [start(), fixed(2, ScopeChange, 6, Some(4), 5, &[A, F])];
    assert_eq!(
        refused(&before, &after, &reg),
        vec![
            BaselineHistoryProblem::AfterPoint(vec![BaselineProblem::ArchivedNotLast { event: m(2), position: 0, attempts: 2 }]),
            BaselineHistoryProblem::PastBaselines(vec![BaselineChange::AttemptsAfterBaseline { event: m(2), added: 1 }]),
        ]
    );
}

// ── 거절 사례 — 한 시점 자체가 잘못됨 ─────────────────────────────────────

#[test]
fn a_bad_point_is_reported_as_that_point_with_the_one_point_problems() {
    let reg = registry(1);
    // 변경 전만 잘못됨 — 기준점 없는 start(§5.3). 3e 의 문제를 그대로 담는다.
    let bad = [waiting(1, Start, &[])];
    assert_eq!(
        refused(&bad, &[start()], &reg),
        vec![BaselineHistoryProblem::BeforePoint(vec![BaselineProblem::StartWithoutSnapshot { event: m(1) }])]
    );
    // 변경 후만 잘못됨 — 기준점 없는 archived.
    let before = [start(), waiting(2, ScopeChange, &[F])];
    let after = [start(), waiting(2, ScopeChange, &[F, A])];
    assert_eq!(
        refused(&before, &after, &reg),
        vec![BaselineHistoryProblem::AfterPoint(vec![BaselineProblem::ArchivedWithoutSnapshot {
            event: m(2),
            positions: vec![1]
        }])]
    );
    // 뒤 시점의 등록부에 기준점 이름이 없다.
    assert_eq!(
        check_baseline_history((&[start()], &reg), (&[start()], &SnapshotRegistry::new())),
        Err(vec![BaselineHistoryProblem::AfterPoint(vec![BaselineProblem::UnknownSnapshot { event: m(1), snapshot: a(1) }])])
    );
}

#[test]
fn every_branch_is_reported_in_order() {
    let reg = registry(6);
    let before = [start(), fixed(2, ScopeChange, 9, Some(4), 5, &[A])];
    let after = [fixed(1, Start, 2, None, 1, &[A]), fixed(2, ScopeChange, 9, Some(4), 5, &[A, F])];
    assert_eq!(
        refused(&before, &after, &reg),
        vec![
            BaselineHistoryProblem::BeforePoint(vec![BaselineProblem::UnknownSnapshot { event: m(2), snapshot: a(9) }]),
            BaselineHistoryProblem::AfterPoint(vec![
                BaselineProblem::UnknownSnapshot { event: m(2), snapshot: a(9) },
                BaselineProblem::ArchivedNotLast { event: m(2), position: 0, attempts: 2 },
            ]),
            BaselineHistoryProblem::PastBaselines(vec![
                BaselineChange::SnapshotReplaced { event: m(1), before: a(1), after: a(2) },
                BaselineChange::AttemptsAfterBaseline { event: m(2), added: 1 },
            ]),
        ]
    );
}

// ── 거절 사례 — 어느 사건끼리 견주는가 ─────────────────────────────────────

#[test]
fn a_missing_earlier_event_is_refused() {
    // 적힘 — T1·§5.1 「추가만 한다」. 뒤 시점만 보면 3e 는 통과한다.
    let reg = registry(1);
    let before = [start(), waiting(2, ScopeChange, &[F]), waiting(3, ScopeChange, &[])];
    for after in [vec![start(), waiting(2, ScopeChange, &[F])], vec![start()], vec![]] {
        each_point_passes_alone(&before, &after, &reg);
        let missing: Vec<_> = (after.len() + 1..=3).map(|n| MatchProblem::Missing { event: m(n as u32) }).collect();
        assert_eq!(unmatched(&before, &after, &reg), missing);
    }
}

#[test]
fn names_out_of_place_gaps_reorders_and_duplicates_are_refused() {
    // 적힘 — T1 「이름은 M1 부터 빈틈없이 오름차순」. 3e 는 이름 사이의 관계를 보지 않으므로 이 목록들을 통과시킨다 —
    // 이 비교는 같은 자리를 같은 사건으로 가정하기 전에 그것을 확인한다.
    let reg = registry(2);
    let before = [start(), waiting(2, ScopeChange, &[F])];
    let cases = [
        // 가운데가 빠졌다(빈틈).
        (vec![start(), waiting(3, ScopeChange, &[F])], 1, m(3), false),
        // 재배열.
        (vec![waiting(2, ScopeChange, &[F]), start()], 0, m(2), false),
        // 중복 — 같은 보관본이라 3e 는 그대로 받는다.
        (vec![start(), start()], 1, m(1), true),
        (vec![start(), waiting(2, ScopeChange, &[F]), waiting(2, ScopeChange, &[F, F])], 2, m(2), true),
    ];
    for (after, position, found, repeated) in cases {
        each_point_passes_alone(&before, &after, &reg);
        assert_eq!(
            unmatched(&before, &after, &reg),
            vec![MatchProblem::NameOutOfPlace { side: Side::After, position, found, repeated }],
            "{after:#?}"
        );
    }
    // 앞 시점 쪽도 같다.
    let before = [waiting(2, ScopeChange, &[])];
    assert_eq!(
        unmatched(&before, &[start(), waiting(2, ScopeChange, &[])], &reg),
        vec![MatchProblem::NameOutOfPlace { side: Side::Before, position: 0, found: m(2), repeated: false }]
    );
}

#[test]
fn an_event_of_another_kind_under_the_same_name_is_refused() {
    // 같은 자리에 같은 이름의 다른 종류 사건 — 같은 사건으로 견줄 수 없다. 서로 다른 사건 목록(다른 프로젝트의 기록 등)을
    // 이어 붙인 입력도 여기서 걸린다. 종류의 보존 규칙 자체는 3d 의 것이며 여기서는 대응의 표지로만 쓴다.
    let reg = registry(3);
    let before = [start(), waiting(2, ScopeChange, &[F])];
    let after = [start(), waiting(2, RestoreResult, &[F])];
    each_point_passes_alone(&before, &after, &reg);
    assert_eq!(
        unmatched(&before, &after, &reg),
        vec![MatchProblem::KindDiffers { event: m(2), before: ScopeChange, after: RestoreResult }]
    );
    let other_project = [fixed(1, ScopeChange, 3, None, 1, &[A]), fixed(2, ScopeChange, 3, Some(2), 3, &[A])];
    each_point_passes_alone(&before, &other_project, &reg);
    assert_eq!(
        unmatched(&before, &other_project, &reg),
        vec![MatchProblem::KindDiffers { event: m(1), before: Start, after: ScopeChange }]
    );
}

#[test]
fn without_a_match_no_preservation_is_claimed_or_compared() {
    // 대응이 서지 않으면 보존 비교 결과를 내지 않는다 — 어긋난 짝의 「보존 위반」이나 「보존 성공」을 말하지 않는다.
    let reg = registry(6);
    let before = [start(), fixed(2, ScopeChange, 6, Some(4), 5, &[A])];
    let swapped = [fixed(2, ScopeChange, 6, Some(4), 5, &[A]), start()];
    let found = refused(&before, &swapped, &reg);
    assert!(found.iter().all(|problem| !matches!(problem, BaselineHistoryProblem::PastBaselines(_))), "{found:#?}");
    assert!(found.iter().any(|problem| matches!(problem, BaselineHistoryProblem::Unmatched(_))), "{found:#?}");
    // 한 시점 검사의 갈래는 대응과 따로 돌려준다.
    let found = refused(&[waiting(1, Start, &[])], &[waiting(2, Start, &[])], &reg);
    assert_eq!(found.len(), 3, "{found:#?}");
    assert!(matches!(found[0], BaselineHistoryProblem::BeforePoint(_)));
    assert!(matches!(found[1], BaselineHistoryProblem::AfterPoint(_)));
    assert!(matches!(found[2], BaselineHistoryProblem::Unmatched(_)));
}

// ── 독립 모형과의 대조 ─────────────────────────────────────────────────────

/// 사건 하나의 기준점 칸. 모형과 검사가 같은 값을 쓴다.
type State = (Option<u32>, BaselineSlot<Option<u32>>, BaselineSlot<u32>, Vec<AttemptSeen>);

fn facts(state: &State) -> BaselineFacts {
    let (snapshot, boundary, issuer, attempts) = state;
    let boundary = match boundary {
        Unwritten => Unwritten,
        Written(value) => Written(value.map(c)),
    };
    BaselineFacts::new(m(1), ScopeChange, snapshot.map(a), boundary, *issuer, attempts.clone())
}

/// 시도가 셋 이하인 모든 상태 — 올바른 것과 잘못된 것을 모두 담는다.
fn universe() -> Vec<State> {
    let mut sequences: Vec<Vec<AttemptSeen>> = vec![vec![]];
    for length in 1..=3 {
        for bits in 0..(1u32 << length) {
            sequences.push((0..length).map(|at| if bits >> at & 1 == 1 { A } else { F }).collect());
        }
    }
    let mut states = Vec::new();
    for snapshot in [None, Some(1), Some(2)] {
        for boundary in [Unwritten, Written(None), Written(Some(1)), Written(Some(2))] {
            for issuer in [Unwritten, Written(1), Written(2)] {
                for attempts in &sequences {
                    states.push((snapshot, boundary, issuer, attempts.clone()));
                }
            }
        }
    }
    states
}

/// 정본의 전이만으로 갈 수 있는 상태 — 앞선 것을 그대로 둔 채 (a) 보관 대기 사건에 실패 시도를 하나 적거나
/// (b) 저장 2 로 archived 하나와 기준점·경계·발급기를 함께 적는다(§5.3). 비교기의 규칙이 아니라 전이를 펼쳐 만든다.
fn reachable(from: &State) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    let mut queue = vec![from.clone()];
    while let Some(state) = queue.pop() {
        if !seen.insert(format!("{state:?}")) || state.3.len() >= 3 {
            continue;
        }
        let (snapshot, boundary, issuer, attempts) = &state;
        if snapshot.is_some() {
            continue; // 확정된 사건에 닿는 전이는 없다.
        }
        let mut failed = attempts.clone();
        failed.push(F);
        queue.push((None, *boundary, *issuer, failed));
        for snapshot in [1, 2] {
            for boundary in [None, Some(1), Some(2)] {
                for issuer in [1, 2] {
                    let mut archived = attempts.clone();
                    archived.push(A);
                    queue.push((Some(snapshot), Written(boundary), Written(issuer), archived));
                }
            }
        }
    }
    seen
}

#[test]
fn the_comparison_matches_a_model_built_from_the_transitions() {
    // 모형: 두 시점이 각각 3e 를 통과하고, 뒤 상태가 앞 상태에서 정본의 전이(실패 기록·저장 2)를 0번 이상 적용해 닿는
    // 상태일 때만 받아들인다. 540개 상태의 모든 쌍(291,600)을 대조한다.
    let reg = registry(2);
    let states = universe();
    let valid: Vec<bool> = states.iter().map(|state| check_baselines(&[facts(state)], &reg).is_ok()).collect();
    let mut accepted = 0usize;
    let mut caught_beyond_one_point = 0usize;
    for (i, earlier) in states.iter().enumerate() {
        let targets = reachable(earlier);
        for (j, later) in states.iter().enumerate() {
            let model = valid[i] && valid[j] && targets.contains(&format!("{later:?}"));
            let found = check_baseline_history((&[facts(earlier)], &reg), (&[facts(later)], &reg));
            assert_eq!(found.is_ok(), model, "앞 {earlier:?} → 뒤 {later:?}: {found:?}");
            accepted += usize::from(model);
            caught_beyond_one_point += usize::from(valid[i] && valid[j] && !model);
        }
    }
    // 받아들인 쌍과, 두 시점이 각각 3e 를 통과하는데 거절한 쌍이 모두 충분히 있다 — 모형이 비어 있지 않다.
    assert!(accepted > 100, "{accepted}");
    assert!(caught_beyond_one_point > 1000, "{caught_beyond_one_point}");
}

// ── 순수성 ─────────────────────────────────────────────────────────────────

#[test]
fn comparing_changes_neither_inputs_nor_registries_nor_files() {
    let root = scratch("unchanged");
    let store = ObjectStore::at(&root.join(".gil"));
    let manifest = store.put_manifest(&Manifest::new(Vec::new())).expect("빈 세계를 창고에 눕힐 수 있다");
    let mut reg = SnapshotRegistry::new();
    assert_eq!(reg.intern(manifest).unwrap(), a(1));
    fs::write(root.join("work.txt"), "user file").unwrap();

    let before = vec![start(), waiting(2, ScopeChange, &[F, F])];
    for after in [
        vec![start(), fixed(2, ScopeChange, 1, Some(4), 5, &[F, F, A])],
        vec![start(), fixed(2, ScopeChange, 1, Some(4), 5, &[A])],
        vec![start()],
    ] {
        let copies = (before.clone(), after.clone(), reg.clone(), tree(&root));
        let first = check_baseline_history((&before, &reg), (&after, &reg));
        assert_eq!((before.clone(), after.clone(), reg.clone(), tree(&root)), copies, "비교가 입력·등록부·파일을 바꿨다");
        assert_eq!(reg.next_id(), 2, "비교가 이름을 발급했다");
        assert_eq!(check_baseline_history((&before, &reg), (&after, &reg)), first, "같은 입력은 같은 결과를 낸다");
    }
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn the_comparator_has_no_way_to_touch_files_or_state() {
    let text = include_str!("baseline_history.rs");
    for word in ["std::fs", "std::io", "File", "Path", "ProjectSession", "ObjectStore", "Cell", "static mut", "intern("] {
        assert!(!text.contains(word), "baseline_history.rs 가 {word:?} 를 쓴다");
    }
}

// ── 문장과 보장 범위 ───────────────────────────────────────────────────────

#[test]
fn every_problem_reads_as_a_sentence() {
    let changes = [
        (BaselineChange::SnapshotReplaced { event: m(2), before: a(6), after: a(7) }, "snapshot:A7"),
        (BaselineChange::SnapshotCleared { event: m(2), before: a(6) }, "보관 대기"),
        (BaselineChange::BoundaryChanged { event: m(1), before: None, after: Written(Some(c(3))) }, "null"),
        (BaselineChange::BoundaryChanged { event: m(2), before: Some(c(4)), after: Unwritten }, "칸 없음"),
        (BaselineChange::IssuerChanged { event: m(2), before: 5, after: Written(6) }, "baseline.next_cycle_id"),
        (BaselineChange::AttemptChanged { event: m(2), position: 0, before: F, after: A }, "1번째"),
        (BaselineChange::AttemptsRemoved { event: m(2), before: 3, after: 1 }, "지우지"),
        (BaselineChange::AttemptsAfterBaseline { event: m(2), added: 1 }, "마지막"),
    ];
    for (change, said) in changes {
        let text = change.to_string();
        assert!(text.contains(said) && text.contains("event:M") && !text.contains('{'), "{change:?} → {text:?}");
    }
    let matches = [
        (MatchProblem::NameOutOfPlace { side: Side::After, position: 1, found: m(3), repeated: false }, "event:M2"),
        (MatchProblem::NameOutOfPlace { side: Side::Before, position: 1, found: m(1), repeated: true }, "다시 쓴다"),
        (MatchProblem::Missing { event: m(2) }, "추가만"),
        (MatchProblem::KindDiffers { event: m(2), before: ScopeChange, after: RestoreResult }, "restore_result"),
    ];
    for (problem, said) in matches {
        let text = problem.to_string();
        assert!(text.contains(said) && text.contains("event:M") && !text.contains('{'), "{problem:?} → {text:?}");
    }
    let whole = BaselineHistoryProblem::Unmatched(vec![MatchProblem::Missing { event: m(2) }]).to_string();
    assert!(whole.contains("짝지을 수 없다") && whole.contains("event:M2"), "{whole}");
}

#[test]
fn success_does_not_claim_what_was_not_checked() {
    for said in [
        "cause",
        "재배열",
        "원본 시도 레코드",
        "되돌린",
        "기록되지 않은 실패",
        "끝이 잘려",
        "같은 이름·같은 종류",
        "link_history",
        "매핑",
        "manifest",
        "재보관",
        "T5",
        "덧붙일 자격",
        "T7",
        "T8",
        "권한",
        "원자성",
        "T12",
        "O-3",
    ] {
        assert!(BASELINE_HISTORY_DOES_NOT_SHOW.iter().any(|line| line.contains(said)), "{said} 을 검사하지 않았다고 적어야 한다");
    }
    assert!(UNCHECKED_CONDITIONS.iter().any(|line| line.starts_with("T6") && line.contains("두 시점")));
}
