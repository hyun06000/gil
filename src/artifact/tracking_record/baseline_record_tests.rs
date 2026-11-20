//! 기준점·보관 시도 기록의 한 시점 내부 일관성 검사(「3e」) 시험 — **합성 입력**만 쓴다.
//!
//! 여기의 사건 칸·시도 결과·등록부는 시험이 만든 값이다. 실제 보관, 모니터 승인, 사건 기록, `state.yaml`,
//! `gil start` 를 쓰지 않는다. 창고를 쓰는 시험은 프로세스 번호가 붙은 임시 폴더를 새로 만든다.
//!
//! 각 시험 설명의 근거는 [공유 저장·전이 계약](../../../spec/GIL_Storage_Transition_Contract_v0.1.md)의 절이다.
//! 「적힘」은 계약 문장 그대로의 조건, 「도출」은 그 문장들에서 따라 나온 조건이다.

use std::collections::BTreeMap;
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

/// 기준점이 없는 사건 — 경계·발급기 칸이 없다(§5.1 예시의 M2, §5.8 「저장 1 뒤」).
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

fn agrees(events: &[BaselineFacts], registry: &SnapshotRegistry) -> BaselinesAgree {
    check_baselines(events, registry).unwrap_or_else(|problems| panic!("받아들여야 한다: {problems:#?}"))
}

fn problems(events: &[BaselineFacts], registry: &SnapshotRegistry) -> Vec<BaselineProblem> {
    check_baselines(events, registry).expect_err("거절해야 한다")
}

fn scratch(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gil-baseline-record-{}-{label}", std::process::id()));
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

// ── 정상 기록 ──────────────────────────────────────────────────────────────

#[test]
fn no_events_is_the_state_before_any_adoption() {
    // §5.1: 정책을 채택하지 않은 프로젝트는 `events: []` 이다.
    let kept = agrees(&[], &SnapshotRegistry::new());
    assert!(kept.with_snapshot().is_empty() && kept.without_snapshot().is_empty());
}

#[test]
fn a_wait_with_neither_snapshot_nor_attempts_is_kept() {
    // §5.8 「저장 1 뒤」: `baseline: {snapshot_ref: null, attempts: []}`. 보관 대기는 유효한 상태다(§6.3, T5).
    // 실패 기록은 최선 노력이므로(§5.3) 시도가 없다는 것이 「시도하지 않았다」를 뜻하지 않는다 — 검사도 그렇게 읽지 않는다.
    let events = [start(), waiting(2, ScopeChange, &[])];
    let kept = agrees(&events, &registry(1));
    assert_eq!(kept.without_snapshot(), &[m(2)]);
    assert_eq!(kept.with_snapshot(), &[(m(1), a(1))]);
}

#[test]
fn a_wait_with_any_number_of_failed_attempts_is_kept() {
    // §5.3 「보관 대기 | 보관 시도 | 실패 | 실패 시도 기록, 보관 대기 유지」와 §6.3 의 재시도에는 횟수 제한이 없다.
    for count in [1usize, 2, 7, 300] {
        let events = [start(), waiting(2, ScopeChange, &vec![F; count])];
        agrees(&events, &registry(1));
    }
}

#[test]
fn the_start_baseline_with_a_null_boundary_is_kept() {
    // §5.1 예시: start 의 `baseline.after_cycle_ref: null`(보관한 경계, start 는 없음)·`next_cycle_id: 1`.
    // 도출: null 은 **적힌** 값이고 빠진 칸이 아니다.
    let kept = agrees(&[start()], &registry(1));
    assert_eq!(kept.with_snapshot(), &[(m(1), a(1))]);
}

#[test]
fn a_success_after_failures_is_kept() {
    // §6.5 (가): 실패가 쌓인 M2 를 C5 를 지난 경계에서 다시 시도해 성공 — 경계 C5·발급기 6·A6, archived 는 마지막에 한 번.
    let events = [start(), fixed(2, ScopeChange, 6, Some(5), 6, &[F, F, F, A])];
    let kept = agrees(&events, &registry(6));
    assert_eq!(kept.with_snapshot(), &[(m(1), a(1)), (m(2), a(6))]);
    assert!(kept.without_snapshot().is_empty());
}

#[test]
fn a_success_on_the_first_attempt_is_kept() {
    // §5.8 「저장 2 뒤」: `{after_cycle_ref: cycle:C4, next_cycle_id: 5, snapshot_ref: snapshot:A6, attempts: [{result: archived}]}`.
    agrees(&[start(), fixed(2, ScopeChange, 6, Some(4), 5, &[A])], &registry(6));
}

#[test]
fn the_same_snapshot_named_by_different_events_is_kept() {
    // §5.5: 범위를 바꿔도 추적 세계가 같으면 같은 이름이 나온다. 적용 사건은 사건 기록이 따로 지닌다 —
    // 보관본 재사용은 사건 중복이 아니다. §5.8 「같은 세계」, §6.5 의 `restore_result`.
    let events = [
        start(),
        fixed(2, ScopeChange, 1, Some(3), 4, &[A]),
        fixed(3, ScopeChange, 1, Some(6), 7, &[F, A]),
        fixed(4, RestoreResult, 1, Some(2), 9, &[A]),
    ];
    let kept = agrees(&events, &registry(1));
    assert!(kept.with_snapshot().iter().all(|(_, snapshot)| *snapshot == a(1)));
    assert_eq!(kept.with_snapshot().len(), 4);
}

#[test]
fn a_replaced_event_without_a_snapshot_is_not_judged_here() {
    // §5.2: 마지막이 아닌 사건에 기준점이 없으면 뒤 사건으로 대체된 것이다(T5). 그 위치 판단은 이 검사의 몫이 아니며,
    // 대체된 사건의 실패 시도 기록은 그대로 남는다(§6.5 (나)).
    let events = [start(), waiting(2, ScopeChange, &[F, F]), fixed(3, ScopeChange, 7, Some(5), 6, &[A])];
    let kept = agrees(&events, &registry(7));
    assert_eq!(kept.without_snapshot(), &[m(2)]);
}

#[test]
fn boundary_values_are_not_judged_here() {
    // 경계 값의 의미는 T7 의 몫이다. 특히 기존 프로젝트의 첫 채택 `scope_change`(§5.1 종류표)가 사이클 없는 경계에서
    // 저장될 수 있는지는 정본으로 확정할 수 없으므로, 일반 사건에 non-null 경계를 요구하지 않는다.
    // 발급기 값과 사건 발급기의 대소(T7)도 보지 않는다.
    agrees(&[fixed(1, ScopeChange, 1, None, 1, &[A])], &registry(1));
    agrees(&[fixed(1, ScopeChange, 1, Some(9), 2, &[A])], &registry(1));
    agrees(&[fixed(1, Start, 1, Some(3), 1, &[A])], &registry(1));
}

#[test]
fn failed_attempts_on_a_start_are_not_judged_here() {
    // §5.3: 보관 대기 상태의 start 는 없고, 승인 전·보관 전의 준비는 시작 준비 기록(4번 작업)에 남는다.
    // 시작 준비 중의 실패를 사건에 적는지는 4번이 정하므로 이 검사는 막지 않는다. archived 의 규칙은 그대로다.
    agrees(&[fixed(1, Start, 1, None, 1, &[F, A])], &registry(1));
}

// ── 잘못된 기록 ────────────────────────────────────────────────────────────

#[test]
fn an_archived_attempt_without_a_snapshot_is_refused() {
    // 적힘 — T6·§5.1: `archived` 는 `snapshot_ref` 가 있을 때만 나온다.
    for (attempts, positions) in [(vec![A], vec![0]), (vec![F, F, A], vec![2])] {
        let found = problems(&[start(), waiting(2, ScopeChange, &attempts)], &registry(1));
        assert_eq!(found, vec![BaselineProblem::ArchivedWithoutSnapshot { event: m(2), positions }]);
    }
}

#[test]
fn a_boundary_or_issuer_without_a_snapshot_is_refused() {
    // 적힘 — T6·§5.1: 보관 경계·발급기 값은 `snapshot_ref` 가 있을 때만, 그와 함께 적는다.
    // 값이 null 인 경계도 적힌 칸이다 — 기준점 없이 남을 수 없다.
    use BaselineField::{AfterCycleRef, NextCycleId};
    let cases = [
        (Written(Some(c(4))), Unwritten, vec![AfterCycleRef]),
        (Written(None), Unwritten, vec![AfterCycleRef]),
        (Unwritten, Written(5), vec![NextCycleId]),
        (Written(Some(c(4))), Written(5), vec![AfterCycleRef, NextCycleId]),
    ];
    for (boundary, issuer, fields) in cases {
        let event = BaselineFacts::new(m(2), ScopeChange, None, boundary, issuer, vec![F]);
        let found = problems(&[start(), event], &registry(1));
        assert_eq!(found, vec![BaselineProblem::BoundaryWithoutSnapshot { event: m(2), fields }]);
    }
}

#[test]
fn a_partly_written_baseline_is_refused() {
    // 도출 — §5.1 「`snapshot_ref` 와 함께 한 번만 적는다」, 예시의 「보관하면 … 함께 채운다」:
    // 기준점이 있는데 경계·발급기 칸이 빠지면 부분 기록이다. start 의 null 경계는 빠진 칸이 아니다(위의 정상 시험).
    use BaselineField::{AfterCycleRef, NextCycleId};
    let cases = [
        (Unwritten, Written(1), vec![AfterCycleRef]),
        (Written(None), Unwritten, vec![NextCycleId]),
        (Unwritten, Unwritten, vec![AfterCycleRef, NextCycleId]),
    ];
    for kind in [Start, ScopeChange] {
        for (boundary, issuer, missing) in cases.clone() {
            let event = BaselineFacts::new(m(1), kind, Some(a(1)), boundary, issuer, vec![A]);
            let found = problems(&[event], &registry(1));
            assert_eq!(found, vec![BaselineProblem::PartialBaseline { event: m(1), missing }], "{kind:?}");
        }
    }
}

#[test]
fn a_snapshot_without_its_archived_attempt_is_refused() {
    // 도출 — 저장 2 는 세계 이름·baseline·시도 기록을 한 번에 적어 기준점을 확정하고(§5.3, §5.8 「저장 2 뒤」),
    // 최선 노력으로 적힌 것은 **실패** 기록뿐이다(§5.3). 그래서 기준점이 있으면 archived 가 정확히 한 번 있다.
    for attempts in [vec![], vec![F], vec![F, F]] {
        let found = problems(&[start(), fixed(2, ScopeChange, 1, Some(4), 5, &attempts)], &registry(1));
        assert_eq!(found, vec![BaselineProblem::SnapshotWithoutArchived { event: m(2) }], "{attempts:?}");
    }
}

#[test]
fn archived_twice_is_refused() {
    // 적힘 — §5.1 「`archived` 는 … 마지막에 한 번 나온다」.
    let found = problems(&[fixed(1, Start, 1, None, 1, &[A, A])], &registry(1));
    assert_eq!(
        found,
        vec![
            BaselineProblem::ArchivedMoreThanOnce { event: m(1), positions: vec![0, 1] },
            BaselineProblem::ArchivedNotLast { event: m(1), position: 0, attempts: 2 },
        ]
    );
    let found = problems(&[start(), fixed(2, ScopeChange, 1, Some(4), 5, &[F, A, F, A])], &registry(1));
    assert!(found.contains(&BaselineProblem::ArchivedMoreThanOnce { event: m(2), positions: vec![1, 3] }), "{found:?}");
    assert!(found.contains(&BaselineProblem::ArchivedNotLast { event: m(2), position: 1, attempts: 4 }), "{found:?}");
}

#[test]
fn attempts_after_archived_are_refused() {
    // 적힘 — §5.1 「마지막에」. 기준점이 확정된 뒤 실패나 다른 시도가 덧붙는 것은 이 한 시점에서 보이는 모순이다.
    for (attempts, position) in [(vec![A, F], 0), (vec![F, A, F], 1), (vec![A, F, F, F], 0)] {
        let len = attempts.len();
        let found = problems(&[start(), fixed(2, ScopeChange, 1, Some(4), 5, &attempts)], &registry(1));
        assert_eq!(found, vec![BaselineProblem::ArchivedNotLast { event: m(2), position, attempts: len }], "{attempts:?}");
    }
}

#[test]
fn a_snapshot_missing_from_the_registry_is_refused() {
    // 적힘 — T6 「기준점은 등록부에 실재한다」. 이름의 조회만 본다(객체 검사는 아래 시험).
    let found = problems(&[start(), fixed(2, ScopeChange, 4, Some(4), 5, &[A])], &registry(3));
    assert_eq!(found, vec![BaselineProblem::UnknownSnapshot { event: m(2), snapshot: a(4) }]);
    let found = problems(&[start()], &SnapshotRegistry::new());
    assert_eq!(found, vec![BaselineProblem::UnknownSnapshot { event: m(1), snapshot: a(1) }]);
}

#[test]
fn a_start_without_a_snapshot_is_refused() {
    // 적힘 — §5.3 「start 사건은 정식 시작의 한 번 저장 안에서 기준점과 함께 만든다. 보관 대기 상태의 start 는 없다」.
    for attempts in [vec![], vec![F]] {
        let found = problems(&[waiting(1, Start, &attempts)], &registry(1));
        assert_eq!(found, vec![BaselineProblem::StartWithoutSnapshot { event: m(1) }], "{attempts:?}");
    }
}

#[test]
fn every_problem_is_returned_in_event_order() {
    let events = [
        waiting(1, Start, &[A]),
        fixed(2, ScopeChange, 9, Some(4), 5, &[A, F]),
        BaselineFacts::new(m(3), ScopeChange, None, Unwritten, Written(6), vec![]),
        BaselineFacts::new(m(4), RestoreResult, Some(a(1)), Written(Some(c(7))), Unwritten, vec![]),
    ];
    let found = problems(&events, &registry(1));
    assert_eq!(
        found,
        vec![
            BaselineProblem::ArchivedWithoutSnapshot { event: m(1), positions: vec![0] },
            BaselineProblem::StartWithoutSnapshot { event: m(1) },
            BaselineProblem::UnknownSnapshot { event: m(2), snapshot: a(9) },
            BaselineProblem::ArchivedNotLast { event: m(2), position: 0, attempts: 2 },
            BaselineProblem::BoundaryWithoutSnapshot { event: m(3), fields: vec![BaselineField::NextCycleId] },
            BaselineProblem::PartialBaseline { event: m(4), missing: vec![BaselineField::NextCycleId] },
            BaselineProblem::SnapshotWithoutArchived { event: m(4) },
        ]
    );
}

// ── 등록부는 읽기만 하고, 이름은 객체 검사가 아니다 ───────────────────────

#[test]
fn refusing_changes_neither_inputs_nor_registry_nor_files() {
    let root = scratch("unchanged");
    let store = ObjectStore::at(&root.join(".gil"));
    let manifest = store.put_manifest(&Manifest::new(Vec::new())).expect("빈 세계를 창고에 눕힐 수 있다");
    let mut registry = SnapshotRegistry::new();
    assert_eq!(registry.intern(manifest).unwrap(), a(1));
    fs::write(root.join("work.txt"), "user file").unwrap();

    let events = vec![start(), fixed(2, ScopeChange, 2, Some(4), 5, &[A, F]), waiting(3, Start, &[A])];
    let (events_copy, registry_copy, files) = (events.clone(), registry.clone(), tree(&root));
    let first = check_baselines(&events, &registry);
    assert!(first.is_err());
    assert_eq!(events, events_copy, "거절이 입력을 바꿨다");
    assert_eq!(registry, registry_copy, "거절이 등록부를 바꿨다");
    assert_eq!(registry.next_id(), 2, "검사가 이름을 발급했다");
    assert_eq!(tree(&root), files, "거절이 파일을 바꿨다");
    assert_eq!(check_baselines(&events, &registry), first, "같은 입력은 같은 결과를 낸다");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_registered_name_is_not_an_object_check() {
    // T6 의 「실재」 가운데 이 검사가 보는 것은 등록부의 이름뿐이다. manifest 객체를 지워도 결과가 같다 —
    // 그래서 성공은 객체의 실재·내용을 말하지 않고, 이 사실을 BASELINES_DO_NOT_SHOW 에 적는다.
    let root = scratch("object-gone");
    let store = ObjectStore::at(&root.join(".gil"));
    let manifest = store.put_manifest(&Manifest::new(Vec::new())).unwrap();
    let mut registry = SnapshotRegistry::new();
    registry.intern(manifest).unwrap();
    assert!(check_baselines(&[start()], &registry).is_ok());

    fs::remove_dir_all(store.root()).unwrap();
    assert!(check_baselines(&[start()], &registry).is_ok(), "이 검사는 객체를 보지 않는다");
    assert!(!store.root().exists(), "검사가 창고를 다시 만들었다");
    assert!(BASELINES_DO_NOT_SHOW.iter().any(|line| line.contains("manifest") && line.contains("실재")));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn the_checker_has_no_way_to_touch_files_or_state() {
    let text = include_str!("baseline_record.rs");
    for word in ["std::fs", "std::io", "File", "Path", "ProjectSession", "ObjectStore", "Cell", "static mut", "intern("] {
        assert!(!text.contains(word), "baseline_record.rs 가 {word:?} 를 쓴다");
    }
}

// ── 문장과 보장 범위 ───────────────────────────────────────────────────────

#[test]
fn every_problem_reads_as_a_sentence() {
    let samples = [
        (BaselineProblem::ArchivedWithoutSnapshot { event: m(2), positions: vec![0, 2] }, "1번째, 3번째"),
        (BaselineProblem::BoundaryWithoutSnapshot { event: m(2), fields: vec![BaselineField::AfterCycleRef] }, "baseline.after_cycle_ref"),
        (BaselineProblem::PartialBaseline { event: m(2), missing: vec![BaselineField::NextCycleId] }, "baseline.next_cycle_id"),
        (BaselineProblem::SnapshotWithoutArchived { event: m(2) }, "한 번에"),
        (BaselineProblem::ArchivedMoreThanOnce { event: m(2), positions: vec![0, 1] }, "한 번만"),
        (BaselineProblem::ArchivedNotLast { event: m(2), position: 0, attempts: 2 }, "마지막"),
        (BaselineProblem::UnknownSnapshot { event: m(2), snapshot: a(4) }, "snapshot:A4"),
        (BaselineProblem::StartWithoutSnapshot { event: m(1) }, "보관 대기"),
    ];
    for (problem, said) in samples {
        let text = problem.to_string();
        assert!(text.contains(said) && text.contains("event:M") && !text.contains('{'), "{problem:?} → {text:?}");
    }
}

#[test]
fn success_does_not_claim_what_was_not_checked() {
    for said in [
        "빠짐없이",
        "cause",
        "시간",
        "두 시점",
        "보관 성공",
        "권한",
        "manifest",
        "올바른 세계",
        "T7",
        "T8",
        "T5",
        "사용자 승인",
        "실험을 열어도",
        "원자성",
        "T12",
        "O-3",
    ] {
        assert!(BASELINES_DO_NOT_SHOW.iter().any(|line| line.contains(said)), "{said} 을 검사하지 않았다고 적어야 한다");
    }
    assert!(UNCHECKED_CONDITIONS.iter().any(|line| line.starts_with("T6") && line.contains("한 시점")));
}
