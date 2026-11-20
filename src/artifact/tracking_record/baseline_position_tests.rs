//! 보관 대기·대체 위치 구분과 변경 전부터 대체된 사건의 기준점 추가 거절(「3g」) 시험 — **합성 입력**만 쓴다.
//!
//! 여기의 사건 칸·시도 결과·등록부는 시험이 만든 값이다. 실제 보관, 모니터 승인, 사건 기록, `state.yaml`, `gil start` 를 쓰지
//! 않는다. 창고를 쓰는 시험은 프로세스 번호가 붙은 임시 폴더를 새로 만든다.
//!
//! 위치 조건으로 거절하는 시험은 3f 의 [`check_baseline_history`] 가 같은 입력을 **실제로 통과시킨다**는 것을 먼저 확인한다 —
//! 이 검사가 3f 의 범위를 넓힌 것이 아니라 다른 조건을 따로 본다는 근거다.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::*;
use crate::artifact::{Manifest, ManifestAddress, ObjectStore, SnapshotRegistry};
use crate::refs::{CycleRef, SnapshotRef};

use AttemptSeen::{Archived as A, NotArchived as F};
use BaselinePosition::{HasBaseline, ReplacedInList, WaitingInList};
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
fn waiting(event: u32, attempts: &[AttemptSeen]) -> BaselineFacts {
    BaselineFacts::new(m(event), ScopeChange, None, Unwritten, Unwritten, attempts.to_vec())
}

/// 기준점과 경계·발급기 칸이 모두 적힌 사건.
fn fixed(event: u32, snapshot: u32, boundary: u32, issuer: u32, attempts: &[AttemptSeen]) -> BaselineFacts {
    BaselineFacts::new(m(event), ScopeChange, Some(a(snapshot)), Written(Some(c(boundary))), Written(issuer), attempts.to_vec())
}

/// §5.1 예시의 `start` — `after_cycle_ref: null`, `next_cycle_id: 1`, `snapshot_ref: A1`, `[{result: archived}]`.
fn start() -> BaselineFacts {
    BaselineFacts::new(m(1), Start, Some(a(1)), Written(None), Written(1), vec![A])
}

fn positions(events: &[BaselineFacts], registry: &SnapshotRegistry) -> Vec<(EventName, BaselinePosition)> {
    classify_baseline_positions(events, registry)
        .unwrap_or_else(|problems| panic!("구분해야 한다: {problems:#?}"))
        .entries()
        .to_vec()
}

fn kept(before: &[BaselineFacts], after: &[BaselineFacts], registry: &SnapshotRegistry) -> ReplacedBaselinesKept {
    check_replaced_baselines((before, registry), (after, registry))
        .unwrap_or_else(|problems| panic!("받아들여야 한다: {problems:#?}"))
}

fn refused(before: &[BaselineFacts], after: &[BaselineFacts], registry: &SnapshotRegistry) -> Vec<ReplacedBaselineProblem> {
    check_replaced_baselines((before, registry), (after, registry)).expect_err("거절해야 한다")
}

/// 3f 의 두 시점 비교는 **통과한다** — 아래 거절은 3f 로는 보이지 않는다.
fn history_alone_passes(before: &[BaselineFacts], after: &[BaselineFacts], registry: &SnapshotRegistry) {
    assert!(
        check_baseline_history((before, registry), (after, registry)).is_ok(),
        "3f 가 받아들여야 한다: {before:#?} → {after:#?}"
    );
}

/// 위치 조건 위반만 나왔다.
fn rearchived(before: &[BaselineFacts], after: &[BaselineFacts], registry: &SnapshotRegistry) -> Vec<ReplacedBaselineProblem> {
    let found = refused(before, after, registry);
    assert!(
        found.iter().all(|problem| matches!(problem, ReplacedBaselineProblem::ReplacedGainedBaseline { .. })),
        "위치 조건 위반만 나와야 한다: {found:#?}"
    );
    found
}

/// 기존 3f 갈래 하나만 나왔고 위치 조건 위반은 없다.
fn history_only(before: &[BaselineFacts], after: &[BaselineFacts], registry: &SnapshotRegistry) -> Vec<BaselineHistoryProblem> {
    match refused(before, after, registry).as_slice() {
        [ReplacedBaselineProblem::History(found)] => found.clone(),
        other => panic!("3f 갈래 하나만 나와야 한다: {other:#?}"),
    }
}

fn scratch(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gil-baseline-position-{}-{label}", std::process::id()));
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

// ── 한 시점: 위치 구분 ─────────────────────────────────────────────────────

#[test]
fn an_empty_list_has_no_events() {
    // §5.1: 정책을 채택하지 않은 프로젝트는 `events: []` 이다.
    let found = classify_baseline_positions(&[], &SnapshotRegistry::new()).unwrap();
    assert!(found.is_empty() && found.waiting().is_none() && found.replaced().is_empty() && found.with_baseline().is_empty());
}

#[test]
fn a_list_ending_with_a_baseline_has_no_wait() {
    let reg = registry(2);
    assert_eq!(positions(&[start()], &reg), vec![(m(1), HasBaseline)]);
    let found = classify_baseline_positions(&[start(), fixed(2, 2, 4, 5, &[F, A])], &reg).unwrap();
    assert_eq!(found.entries(), &[(m(1), HasBaseline), (m(2), HasBaseline)]);
    assert_eq!(found.waiting(), None);
    assert_eq!(found.with_baseline(), vec![m(1), m(2)]);
}

#[test]
fn only_the_last_event_without_a_baseline_is_waiting() {
    // §5.2: 마지막 사건에 기준점이 없으면 보관 대기다. 시도가 없거나 실패가 여럿이어도 같다(§5.3·§6.3).
    for attempts in [vec![], vec![F], vec![F, F, F]] {
        let found = classify_baseline_positions(&[start(), waiting(2, &attempts)], &registry(1)).unwrap();
        assert_eq!(found.entries(), &[(m(1), HasBaseline), (m(2), WaitingInList)]);
        assert_eq!(found.waiting(), Some(m(2)));
        assert!(found.replaced().is_empty());
    }
}

#[test]
fn several_replaced_events_may_stand_before_the_waiting_one() {
    // §5.3 표 「다른 변경안의 모니터 승인 → 앞 사건은 기준점 없이 대체된다」 — 대체는 거듭될 수 있고 대체된 사건은 남는다.
    let events = [start(), waiting(2, &[F]), waiting(3, &[]), waiting(4, &[F, F]), waiting(5, &[])];
    let found = classify_baseline_positions(&events, &registry(1)).unwrap();
    assert_eq!(
        found.entries(),
        &[(m(1), HasBaseline), (m(2), ReplacedInList), (m(3), ReplacedInList), (m(4), ReplacedInList), (m(5), WaitingInList)]
    );
    assert_eq!(found.replaced(), vec![m(2), m(3), m(4)]);
    assert_eq!(found.waiting(), Some(m(5)));
}

#[test]
fn a_replaced_event_may_be_followed_by_a_fixed_one() {
    // §6.5 (나): M2 는 기준점 없이 대체되고 M3 이 보관된다. 기준점이 있는 앞쪽 사건은 대체 사건이 아니다.
    let reg = registry(7);
    let events = [start(), waiting(2, &[F]), fixed(3, 7, 5, 6, &[A])];
    assert_eq!(positions(&events, &reg), vec![(m(1), HasBaseline), (m(2), ReplacedInList), (m(3), HasBaseline)]);
    let events = [start(), fixed(2, 6, 4, 5, &[A]), waiting(3, &[]), fixed(4, 7, 6, 7, &[F, A]), waiting(5, &[F])];
    let found = classify_baseline_positions(&events, &reg).unwrap();
    assert_eq!(found.with_baseline(), vec![m(1), m(2), m(4)]);
    assert_eq!(found.replaced(), vec![m(3)]);
    assert_eq!(found.waiting(), Some(m(5)));
}

#[test]
fn the_same_snapshot_named_by_several_events_is_classified_per_event() {
    // §5.5: 보관본 재사용은 사건 중복이 아니다. 위치는 사건마다 따로다.
    let events = [
        start(),
        fixed(2, 1, 3, 4, &[A]),
        BaselineFacts::new(m(3), RestoreResult, Some(a(1)), Written(Some(c(2))), Written(9), vec![F, A]),
        waiting(4, &[]),
        waiting(5, &[]),
    ];
    assert_eq!(
        positions(&events, &registry(1)),
        vec![(m(1), HasBaseline), (m(2), HasBaseline), (m(3), HasBaseline), (m(4), ReplacedInList), (m(5), WaitingInList)]
    );
}

#[test]
fn names_out_of_place_leave_the_last_event_unknown() {
    // 순서가 서지 않으면 「마지막」을 말할 수 없다 — 3f 와 같은 이름 자리 규칙. 구분을 돌려주지 않는다.
    let reg = registry(1);
    let cases: [(Vec<BaselineFacts>, usize, u32, bool); 4] = [
        (vec![start(), waiting(3, &[])], 1, 3, false),                     // 빠진 M2
        (vec![start(), waiting(2, &[]), waiting(2, &[F])], 2, 2, true),    // 중복
        (vec![start(), waiting(3, &[]), waiting(2, &[])], 1, 3, false),    // 재배열
        (vec![waiting(2, &[]), start()], 0, 2, false),                     // 첫 자리부터 어긋남
    ];
    for (events, position, found, repeated) in cases {
        let problems = classify_baseline_positions(&events, &reg).expect_err("구분하면 안 된다");
        assert_eq!(problems, vec![PositionProblem::NameOutOfPlace { position, found: m(found), repeated }], "{events:#?}");
    }
}

#[test]
fn a_waiting_start_is_not_classified_as_a_wait() {
    // 종류: 보관 대기 상태의 `start` 는 없다(§5.3). 마지막 자리여도 「보관 대기」로 구분하지 않고 3e 의 거절을 돌려준다.
    let events = [BaselineFacts::new(m(1), Start, None, Unwritten, Unwritten, vec![F])];
    assert_eq!(
        classify_baseline_positions(&events, &SnapshotRegistry::new()),
        Err(vec![PositionProblem::Point(vec![BaselineProblem::StartWithoutSnapshot { event: m(1) }])])
    );
}

#[test]
fn a_contradictory_point_is_not_classified() {
    // 한 시점 기준점 기록 자체의 모순(3e)은 위치 구분보다 먼저다. 이름 문제와 함께면 둘 다 돌려준다.
    let reg = registry(1);
    let archived_without_snapshot = BaselineFacts::new(m(2), ScopeChange, None, Unwritten, Unwritten, vec![F, A]);
    let problems = classify_baseline_positions(&[start(), archived_without_snapshot.clone()], &reg).unwrap_err();
    assert_eq!(
        problems,
        vec![PositionProblem::Point(vec![BaselineProblem::ArchivedWithoutSnapshot { event: m(2), positions: vec![1] }])]
    );
    let unknown = [start(), fixed(2, 9, 4, 5, &[A])];
    assert!(matches!(
        classify_baseline_positions(&unknown, &reg).unwrap_err().as_slice(),
        [PositionProblem::Point(found)] if found == &vec![BaselineProblem::UnknownSnapshot { event: m(2), snapshot: a(9) }]
    ));
    let both = classify_baseline_positions(&[start(), archived_without_snapshot, waiting(2, &[])], &reg).unwrap_err();
    assert!(matches!(both.as_slice(), [PositionProblem::NameOutOfPlace { position: 2, .. }, PositionProblem::Point(_)]), "{both:#?}");
}

#[test]
fn one_point_cannot_tell_a_late_rearchive() {
    // 앞쪽에 기준점이 있는 사건은 「기준점 있음」이다. 그 기준점이 대체 전에 적혔는지(정당) 뒤에 적혔는지(재보관) 한 시점은 모른다
    // — 그래서 한 시점 구분은 재보관 위반을 말하지 않는다.
    let found = positions(&[start(), fixed(2, 1, 4, 5, &[A]), waiting(3, &[])], &registry(1));
    assert_eq!(found[1], (m(2), HasBaseline));
    assert!(POSITIONS_DO_NOT_SHOW.iter().any(|line| line.contains("한 시점의 구분만으로는 재보관 위반을 말하지 않는다")));
}

#[test]
fn a_truncated_list_shows_a_replaced_event_as_waiting() {
    // 입력 완전성은 보장 범위 밖이다: 실제 원장이 M1–M3 인데 M2 까지만 건네면 M2 는 「목록상 보관 대기」로 보인다.
    let reg = registry(1);
    let whole = [start(), waiting(2, &[F]), waiting(3, &[])];
    let cut = &whole[..2];
    assert_eq!(positions(&whole, &reg)[1], (m(2), ReplacedInList));
    assert_eq!(positions(cut, &reg)[1], (m(2), WaitingInList));
    assert!(POSITIONS_DO_NOT_SHOW.iter().any(|line| line.contains("끝이 잘려") && line.contains("목록상")));
}

// ── 두 시점: 변경 전부터 대체된 사건의 기준점 추가 ──────────────────────────

#[test]
fn a_replaced_event_gaining_a_baseline_passes_3f_but_is_refused_here() {
    // 3f 의 `a_replaced_event_gaining_a_baseline_is_not_judged_here` 와 같은 입력이다. 3f 는 그대로 받아들이고(범위 유지),
    // 위치 조건을 포함한 이 검사는 §5.2 「다시 보관하지 않는다」로 거절한다.
    let reg = registry(1);
    let before = [start(), waiting(2, &[F]), waiting(3, &[])];
    let after = [start(), fixed(2, 1, 5, 6, &[F, A]), waiting(3, &[F])];
    history_alone_passes(&before, &after, &reg);
    assert_eq!(
        rearchived(&before, &after, &reg),
        vec![ReplacedBaselineProblem::ReplacedGainedBaseline { event: m(2), last_before: m(3), snapshot: a(1) }]
    );
}

#[test]
fn every_replaced_event_gaining_a_baseline_is_named() {
    // 여러 대체 사건이 기준점을 얻으면 사건 순서대로 모두 돌려준다. 마지막이던 M4 의 최초 확정은 위반이 아니다.
    let reg = registry(3);
    let before = [start(), waiting(2, &[]), waiting(3, &[F]), waiting(4, &[])];
    let after = [start(), fixed(2, 2, 4, 5, &[A]), fixed(3, 3, 4, 5, &[F, A]), fixed(4, 3, 4, 5, &[A]), waiting(5, &[])];
    history_alone_passes(&before, &after, &reg);
    assert_eq!(
        rearchived(&before, &after, &reg),
        vec![
            ReplacedBaselineProblem::ReplacedGainedBaseline { event: m(2), last_before: m(4), snapshot: a(2) },
            ReplacedBaselineProblem::ReplacedGainedBaseline { event: m(3), last_before: m(4), snapshot: a(3) },
        ]
    );
}

#[test]
fn the_last_event_may_progress_while_replaced_ones_stay() {
    // 사: 이미 대체된 앞 사건은 그대로 두고 마지막 대기 사건에 실패가 붙거나 처음 성공한다.
    let reg = registry(7);
    let before = [start(), waiting(2, &[F]), waiting(3, &[F])];
    for (after_last, update) in [
        (waiting(3, &[F, F]), BaselineUpdate::AttemptsAdded { added: 1 }),
        (waiting(3, &[F, F, F, F]), BaselineUpdate::AttemptsAdded { added: 3 }),
        (fixed(3, 7, 5, 6, &[F, A]), BaselineUpdate::Fixed { added_before_archived: 0 }),
        (fixed(3, 7, 6, 7, &[F, F, F, A]), BaselineUpdate::Fixed { added_before_archived: 2 }),
    ] {
        let after = [start(), waiting(2, &[F]), after_last];
        let found = kept(&before, &after, &reg);
        assert_eq!(found.history().compared()[2], (m(3), update));
        assert_eq!(found.before().replaced(), vec![m(2)]);
        assert_eq!(found.before().waiting(), Some(m(3)));
        assert!(found.unjudged_late_attempts().is_empty());
    }
}

#[test]
fn the_waiting_event_may_stay_as_it_is() {
    // 가: 마지막 대기 사건이 그대로 남는다. 나: 실패 기록이 붙는다(횟수 제한 없음).
    let reg = registry(1);
    let before = [start(), waiting(2, &[])];
    for later in [vec![], vec![F], vec![F; 9]] {
        let found = kept(&before, &[start(), waiting(2, &later)], &reg);
        assert_eq!(found.after().waiting(), Some(m(2)));
    }
}

#[test]
fn a_waiting_event_may_succeed_for_the_first_time() {
    // 다: 마지막 대기 사건의 최초 성공.
    let reg = registry(6);
    let found = kept(&[start(), waiting(2, &[F])], &[start(), fixed(2, 6, 5, 6, &[F, A])], &reg);
    assert_eq!(found.after().entries(), &[(m(1), HasBaseline), (m(2), HasBaseline)]);
}

#[test]
fn a_waiting_event_left_as_it_was_may_be_replaced_by_new_events() {
    // 라: 마지막 대기 사건을 그대로 남긴 채 뒤에 새 사건이 붙는다(§5.3 표 「다른 변경안의 모니터 승인」). 새 사건은 여럿이어도 된다.
    let reg = registry(7);
    let before = [start(), waiting(2, &[F])];
    let after = [start(), waiting(2, &[F]), waiting(3, &[])];
    let found = kept(&before, &after, &reg);
    assert_eq!(found.after().replaced(), vec![m(2)]);
    assert_eq!(found.after().waiting(), Some(m(3)));
    assert_eq!(found.history().added(), &[m(3)]);
    let after = [start(), waiting(2, &[F]), waiting(3, &[F, F]), fixed(4, 7, 6, 7, &[A]), waiting(5, &[]), waiting(6, &[F])];
    let found = kept(&before, &after, &reg);
    assert_eq!(found.history().added(), &[m(3), m(4), m(5), m(6)]);
    assert_eq!(found.after().replaced(), vec![m(2), m(3), m(5)]);
}

#[test]
fn a_waiting_event_with_new_failures_may_be_replaced() {
    // 마: 마지막 대기 사건에 실패가 붙은 뒤 새 사건이 붙는다.
    let reg = registry(7);
    let found = kept(&[start(), waiting(2, &[])], &[start(), waiting(2, &[F, F]), fixed(3, 7, 5, 6, &[A])], &reg);
    assert_eq!(found.history().compared()[1], (m(2), BaselineUpdate::AttemptsAdded { added: 2 }));
    assert!(found.unjudged_late_attempts().is_empty(), "변경 전에 마지막이던 사건의 실패는 판단 보류 대상이 아니다");
}

#[test]
fn a_waiting_event_that_succeeded_may_then_be_followed_by_new_events() {
    // 바: 변경 후 M2 는 마지막이 아니지만 재보관 위반이 아니다 — 두 관측 사이에 성공 뒤 새 승인이 있었을 수 있다.
    let reg = registry(7);
    let before = [start(), waiting(2, &[F])];
    for after in [
        vec![start(), fixed(2, 6, 4, 5, &[F, A]), waiting(3, &[])],
        vec![start(), fixed(2, 6, 4, 5, &[F, F, A]), waiting(3, &[F]), fixed(4, 7, 6, 7, &[A]), waiting(5, &[])],
    ] {
        let found = kept(&before, &after, &reg);
        assert_eq!(found.before().waiting(), Some(m(2)));
        assert_eq!(found.after().entries()[1], (m(2), HasBaseline));
    }
}

#[test]
fn the_order_of_events_between_two_points_is_unknown() {
    // 한계: 바의 입력은 「성공 뒤 새 승인」에서도, 「새 승인으로 대체된 뒤 재보관」(§5.2 위반)에서도 똑같이 나온다. 두 시점으로는
    // 실행 순서를 알 수 없으므로 받아들이고, 그 사실을 보장 범위 밖으로 적는다. 뒤 시점의 위치만으로 막지 않는다.
    let reg = registry(6);
    let before = [start(), waiting(2, &[])];
    let after = [start(), fixed(2, 6, 4, 5, &[A]), waiting(3, &[])];
    assert!(check_replaced_baselines((&before, &reg), (&after, &reg)).is_ok());
    // 중간 관측이 있으면 위반이 보인다 — 대체가 먼저 관측된 경우다.
    let middle = [start(), waiting(2, &[]), waiting(3, &[])];
    assert!(check_replaced_baselines((&before, &reg), (&middle, &reg)).is_ok());
    assert!(matches!(
        rearchived(&middle, &after, &reg).as_slice(),
        [ReplacedBaselineProblem::ReplacedGainedBaseline { event, .. }] if *event == m(2)
    ));
    assert!(POSITIONS_DO_NOT_SHOW.iter().any(|line| line.contains("실행 순서")));
}

#[test]
fn late_failures_on_a_replaced_event_are_neither_refused_nor_judged() {
    // 이번 범위에서 결정하지 않는다: 이미 대체된 사건에 실패 기록만 덧붙은 경우. 거절하지 않고, 성공 값에 판단하지 않은 것으로
    // 드러낸다. 기준점 추가 거절을 대체된 사건의 모든 기록 변경 금지로 넓히지 않았다.
    let reg = registry(1);
    let before = [start(), waiting(2, &[F]), waiting(3, &[])];
    let after = [start(), waiting(2, &[F, F, F]), waiting(3, &[])];
    let found = kept(&before, &after, &reg);
    assert_eq!(found.unjudged_late_attempts(), vec![(m(2), 2)]);
    assert!(POSITIONS_DO_NOT_SHOW.iter().any(|line| line.contains("실패 기록만 뒤늦게") && line.contains("정당하다고 판단하지도 않는다")));
}

#[test]
fn empty_and_unchanged_inputs_are_kept() {
    let found = kept(&[], &[], &SnapshotRegistry::new());
    assert!(found.before().is_empty() && found.after().is_empty());
    let events = [start(), waiting(2, &[F]), fixed(3, 1, 5, 6, &[A]), waiting(4, &[])];
    let found = kept(&events, &events, &registry(1));
    assert_eq!(found.before(), found.after());
    // 처음 보이는 사건들은 어떤 위치여도 된다.
    assert!(check_replaced_baselines((&[], &registry(1)), (&events, &registry(1))).is_ok());
}

// ── 기존 3f 검사의 의미가 그대로인가 ───────────────────────────────────────

#[test]
fn changes_to_fixed_baselines_are_still_refused_by_3f() {
    // 확정 기준점의 교체·삭제, 시도 바꿔치기는 3f 가 계속 거절한다. 위치 조건 위반으로 섞어 말하지 않는다.
    let reg = registry(7);
    let before = [start(), fixed(2, 6, 4, 5, &[F, A]), waiting(3, &[F])];
    for after in [
        vec![start(), fixed(2, 7, 4, 5, &[F, A]), waiting(3, &[F])], // 기준점 교체
        vec![start(), waiting(2, &[F]), waiting(3, &[F])],           // 대기로 되돌림
        vec![start(), fixed(2, 6, 4, 5, &[A]), waiting(3, &[F])],    // 앞선 실패 삭제
        vec![start(), fixed(2, 6, 4, 5, &[F, A]), fixed(3, 7, 6, 7, &[A])], // 실패를 성공으로 바꿔치기
    ] {
        let found = history_only(&before, &after, &reg);
        assert_eq!(found, check_baseline_history((&before, &reg), (&after, &reg)).unwrap_err(), "3f 의 실패를 그대로 담는다");
        assert!(found.iter().any(|problem| matches!(problem, BaselineHistoryProblem::PastBaselines(_) | BaselineHistoryProblem::AfterPoint(_))));
    }
}

#[test]
fn a_rearchive_together_with_a_3f_failure_reports_both_branches() {
    // 대응이 섰다면 3f 의 보존 위반·변경 후 입력 오류와 위치 조건 위반을 따로 돌려준다.
    let reg = registry(6);
    let before = [start(), waiting(2, &[F, F]), waiting(3, &[])];
    // M2 의 앞선 실패 하나를 지우고 기준점을 적었다 — 3f 의 시도 삭제와 위치 조건 위반이 함께다.
    let after = [start(), fixed(2, 6, 4, 5, &[F, A]), waiting(3, &[])];
    let found = refused(&before, &after, &reg);
    assert!(matches!(
        found.as_slice(),
        [ReplacedBaselineProblem::History(history), ReplacedBaselineProblem::ReplacedGainedBaseline { event, .. }]
            if matches!(history.as_slice(), [BaselineHistoryProblem::PastBaselines(_)]) && *event == m(2)
    ), "{found:#?}");
    // 변경 후 입력 자체의 모순(3e)과 위치 조건 위반.
    let broken = BaselineFacts::new(m(3), ScopeChange, None, Unwritten, Written(4), vec![]);
    let after = [start(), fixed(2, 6, 4, 5, &[F, F, A]), broken];
    let found = refused(&before, &after, &reg);
    assert!(matches!(
        found.as_slice(),
        [ReplacedBaselineProblem::History(history), ReplacedBaselineProblem::ReplacedGainedBaseline { .. }]
            if matches!(history.as_slice(), [BaselineHistoryProblem::AfterPoint(_)])
    ), "{found:#?}");
}

#[test]
fn an_unmatched_input_is_not_turned_into_a_position_success() {
    // 입력 대응이 서지 않으면 어느 사건이 대체됐는지 정할 수 없다. 위치 조건을 평가하지 않고(위반도 성공도 말하지 않고) 3f 의
    // 대응 실패를 그대로 돌려준다. 아래 사례는 모두 대응만 맞았다면 위치 조건 위반이 되었을 입력이다.
    let reg = registry(6);
    let before = [start(), waiting(2, &[]), waiting(3, &[])];
    let restore_three = BaselineFacts::new(m(3), RestoreResult, None, Unwritten, Unwritten, vec![]);
    let cases = [
        // 변경 전 목록의 이름이 어긋남
        (vec![start(), waiting(3, &[]), waiting(2, &[])], vec![start(), fixed(2, 6, 4, 5, &[A]), waiting(3, &[])]),
        // 변경 후 목록의 이름이 어긋남(중복)
        (before.to_vec(), vec![start(), fixed(2, 6, 4, 5, &[A]), waiting(2, &[]), waiting(3, &[])]),
        // 변경 후에 앞선 사건이 없음
        (before.to_vec(), vec![start(), fixed(2, 6, 4, 5, &[A])]),
        // 같은 이름의 종류가 다름
        (before.to_vec(), vec![start(), fixed(2, 6, 4, 5, &[A]), restore_three]),
    ];
    for (earlier, later) in cases {
        let found = history_only(&earlier, &later, &reg);
        assert!(found.iter().any(|problem| matches!(problem, BaselineHistoryProblem::Unmatched(_))), "{found:#?}");
    }
}

#[test]
fn a_contradictory_before_point_is_not_positioned() {
    // 변경 전 입력이 3e 를 통과하지 못하면 그 목록의 위치를 정하지 않는다. 결과는 3f 의 변경 전 입력 실패다.
    let reg = registry(6);
    let broken = BaselineFacts::new(m(2), ScopeChange, None, Written(None), Unwritten, vec![]);
    let before = [start(), broken, waiting(3, &[])];
    let after = [start(), fixed(2, 6, 4, 5, &[A]), waiting(3, &[])];
    let found = history_only(&before, &after, &reg);
    assert!(matches!(found.as_slice(), [BaselineHistoryProblem::BeforePoint(_), ..]), "{found:#?}");
}

// ── 독립 모형 대조 ─────────────────────────────────────────────────────────

/// 모형의 사건 상태 — 시도 둘 이하. 기준점은 A1, 경계 C1, 발급기 1 로 고정한다(값의 교체는 3f 의 모형이 본다).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Model {
    Waiting(usize),
    Fixed { failed_before: usize },
}

fn facts_of(list: &[Model]) -> Vec<BaselineFacts> {
    list.iter()
        .enumerate()
        .map(|(at, state)| {
            let event = u32::try_from(at + 1).unwrap();
            match *state {
                Model::Waiting(failed) => waiting(event, &vec![F; failed]),
                Model::Fixed { failed_before } => {
                    let mut attempts = vec![F; failed_before];
                    attempts.push(A);
                    fixed(event, 1, 1, 1, &attempts)
                }
            }
        })
        .collect()
}

const STATES: [Model; 5] = [
    Model::Waiting(0),
    Model::Waiting(1),
    Model::Waiting(2),
    Model::Fixed { failed_before: 0 },
    Model::Fixed { failed_before: 1 },
];

/// 길이 셋 이하의 모든 목록.
fn lists() -> Vec<Vec<Model>> {
    let mut out = vec![Vec::new()];
    let mut frontier = vec![Vec::new()];
    for _ in 0..3 {
        let mut next = Vec::new();
        for list in &frontier {
            for state in STATES {
                let mut longer: Vec<Model> = list.clone();
                longer.push(state);
                next.push(longer);
            }
        }
        out.extend(next.iter().cloned());
        frontier = next;
    }
    out
}

/// 정본의 전이를 펼쳐 닿는 목록 — 비교기의 규칙을 따라 쓰지 않았다.
///
/// - 마지막 대기 사건: 실패 기록(§5.3), 저장 2 로 확정(§5.3·§5.8).
/// - 끝에 새 사건: 저장 1 은 기준점 없이 만든다(§5.8). 그 뒤의 확정·실패는 그 사건이 마지막인 동안 위 전이로 일어난다.
/// - 결정하지 않은 전이: 대체된 사건의 실패 기록 추가. 이 검사가 거절하지 않으므로 모형에도 넣는다.
/// - 대체된 사건의 확정은 없다(§5.2 「다시 보관하지 않는다」). 확정된 사건에 닿는 전이는 없다.
fn reachable(from: &[Model]) -> BTreeSet<Vec<Model>> {
    let mut seen = BTreeSet::new();
    let mut queue = vec![from.to_vec()];
    while let Some(list) = queue.pop() {
        if list.len() > 3 || !seen.insert(list.clone()) {
            continue;
        }
        let last = list.len().checked_sub(1);
        for (at, state) in list.iter().enumerate() {
            let Model::Waiting(failed) = *state else { continue };
            if failed < 2 {
                let mut next = list.clone();
                next[at] = Model::Waiting(failed + 1);
                queue.push(next);
            }
            if Some(at) == last && failed < 2 {
                let mut next = list.clone();
                next[at] = Model::Fixed { failed_before: failed };
                queue.push(next);
            }
        }
        let mut appended = list.clone();
        appended.push(Model::Waiting(0));
        queue.push(appended);
    }
    seen
}

#[test]
fn the_check_matches_a_model_built_from_the_transitions() {
    // 길이 셋 이하·시도 둘 이하 156개 목록의 모든 쌍(24,336)을 「정본 전이를 펼쳐 닿는가」 모형과 대조한다. 모든 목록은 3e 를
    // 통과한다. 3f 가 받아들이지만 이 검사가 거절하는 쌍(변경 전부터 대체된 사건의 기준점 추가)이 실제로 있다.
    let reg = registry(1);
    let all = lists();
    assert_eq!(all.len(), 156);
    let mut accepted = 0usize;
    let mut refused_beyond_3f = 0usize;
    for earlier in &all {
        let targets = reachable(earlier);
        let before = facts_of(earlier);
        assert!(check_baselines(&before, &reg).is_ok());
        for later in &all {
            let after = facts_of(later);
            let model = targets.contains(later);
            let found = check_replaced_baselines((&before, &reg), (&after, &reg));
            assert_eq!(found.is_ok(), model, "앞 {earlier:?} → 뒤 {later:?}: {found:?}");
            let history = check_baseline_history((&before, &reg), (&after, &reg));
            assert!(found.is_err() || history.is_ok(), "이 검사가 3f 보다 너그러울 수 없다");
            if let (Ok(_), Err(problems)) = (&history, &found) {
                assert!(problems.iter().all(|p| matches!(p, ReplacedBaselineProblem::ReplacedGainedBaseline { .. })));
                refused_beyond_3f += 1;
            }
            accepted += usize::from(model);
        }
    }
    assert!(accepted > 500, "{accepted}");
    assert!(refused_beyond_3f > 100, "{refused_beyond_3f}");
}

// ── 순수성 ─────────────────────────────────────────────────────────────────

#[test]
fn checking_changes_neither_inputs_nor_registries_nor_files() {
    let root = scratch("unchanged");
    let store = ObjectStore::at(&root.join(".gil"));
    let manifest = store.put_manifest(&Manifest::new(Vec::new())).expect("빈 세계를 창고에 눕힐 수 있다");
    let mut reg = SnapshotRegistry::new();
    assert_eq!(reg.intern(manifest).unwrap(), a(1));
    fs::write(root.join("work.txt"), "user file").unwrap();

    let before = vec![start(), waiting(2, &[F]), waiting(3, &[])];
    for after in [
        vec![start(), fixed(2, 1, 4, 5, &[F, A]), waiting(3, &[])],
        vec![start(), waiting(2, &[F]), fixed(3, 1, 4, 5, &[A])],
        vec![start()],
    ] {
        let copies = (before.clone(), after.clone(), reg.clone(), tree(&root));
        let first = check_replaced_baselines((&before, &reg), (&after, &reg));
        let one = classify_baseline_positions(&after, &reg);
        assert_eq!((before.clone(), after.clone(), reg.clone(), tree(&root)), copies, "검사가 입력·등록부·파일을 바꿨다");
        assert_eq!(reg.next_id(), 2, "검사가 이름을 발급했다");
        assert_eq!(check_replaced_baselines((&before, &reg), (&after, &reg)), first, "같은 입력은 같은 결과를 낸다");
        assert_eq!(classify_baseline_positions(&after, &reg), one);
    }
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn the_checker_has_no_way_to_touch_files_or_state() {
    let text = include_str!("baseline_position.rs");
    for word in ["std::fs", "std::io", "File", "Path", "ProjectSession", "ObjectStore", "Cell", "static mut", "intern(", "serde"] {
        assert!(!text.contains(word), "baseline_position.rs 가 {word:?} 를 쓴다");
    }
}

// ── 문장과 보장 범위 ───────────────────────────────────────────────────────

#[test]
fn every_problem_reads_as_a_sentence() {
    let texts = [
        (PositionProblem::NameOutOfPlace { position: 1, found: m(3), repeated: false }.to_string(), "event:M2"),
        (PositionProblem::NameOutOfPlace { position: 2, found: m(2), repeated: true }.to_string(), "다시 쓴다"),
        (PositionProblem::Point(vec![BaselineProblem::StartWithoutSnapshot { event: m(1) }]).to_string(), "start"),
        (
            ReplacedBaselineProblem::ReplacedGainedBaseline { event: m(2), last_before: m(3), snapshot: a(1) }.to_string(),
            "다시 보관하지 않는다",
        ),
        (
            ReplacedBaselineProblem::History(vec![BaselineHistoryProblem::Unmatched(vec![MatchProblem::Missing { event: m(2) }])])
                .to_string(),
            "짝지을 수 없다",
        ),
    ];
    for (text, said) in texts {
        assert!(text.contains(said) && text.contains("event:M") && !text.contains('{'), "{text:?}");
    }
    let text = ReplacedBaselineProblem::ReplacedGainedBaseline { event: m(2), last_before: m(3), snapshot: a(1) }.to_string();
    assert!(text.contains("event:M3") && text.contains("snapshot:A1"), "{text}");
}

#[test]
fn success_does_not_claim_what_was_not_checked() {
    for said in [
        "끝이 잘려",
        "잘못 연결",
        "실행 순서",
        "재보관 위반을 말하지 않는다",
        "다음 실험",
        "실패 기록만 뒤늦게",
        "보관 실행 권한",
        "T7",
        "T8",
        "매핑",
        "BaseUndecided",
        "O-1",
        "O-2",
        "O-3",
        "link_history",
        "제품 연결",
    ] {
        assert!(POSITIONS_DO_NOT_SHOW.iter().any(|line| line.contains(said)), "{said} 을 검사하지 않았다고 적어야 한다");
    }
    // 3f 의 「보이지 않은 것」은 그대로 재보관·T5 를 말하고, 이 검사가 따로 본다고 가리킨다.
    assert!(BASELINE_HISTORY_DOES_NOT_SHOW.iter().any(|line| line.contains("재보관") && line.contains("T5") && line.contains("check_replaced_baselines")));
    assert!(UNCHECKED_CONDITIONS.iter().any(|line| line.starts_with("T5") && line.contains("baseline_position")));
}
