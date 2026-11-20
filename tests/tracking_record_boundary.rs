//! 구현 순서 3a(정책 기록 자료형과 부분 검증)와 그 뒤의 동기화 순서 제어(편의상 3b), 정책 원장과
//! 관리 사건의 정책 연결 검사(편의상 3c), 그 두 시점의 추가 전후 보존 검사(편의상 3d), 기준점·보관 시도 기록의
//! 한 시점 일관성 검사(편의상 3e), 그 두 시점 비교(편의상 3f), 보관 대기·대체 위치 구분과 대체된 사건의 기준점 추가 거절
//! (편의상 3g)이 **제품 저장 경로를 바꾸지 않았는가.**
//!
//! 모든 빌드 구성에서 짓는다. 시험 자료는 합성 프로젝트다(독립 임시 폴더, 프로세스 번호가 붙은 이름).
//!
//! - 이 빌드는 형식 10/11 을 알지 못한다: 구조를 해석하기 전에 `UnknownFormat` 으로 거절하고 폴더를
//!   바꾸지 않는다.
//! - 지금 형식의 `state.yaml` 에 `tracking` 블록(정책 원장·동기화 기준)을 덧붙여도 로더가 그것을 받아들여
//!   무시한 채 진행하지 않는다 — 모르는 칸으로 거절하고 폴더를 바꾸지 않는다.
//! - 관리 사건(`events`)을 담은 기록도 같다 — 3c 의 검사기는 저장 경로가 아니므로 사건이 든 기록을 읽게
//!   만들지 않는다.
//! - 3d 의 두 시점 비교도 저장 경로가 아니다. 앞선 기록과 새 기록을 견주는 곳이 생겨도 위의 거절은 그대로다.
//! - 3e 의 기준점 검사도 저장 경로가 아니다. 보관 대기 사건(기준점 없음, 실패 시도)과 기준점이 있는 사건이 든
//!   기록도 형식 10/11 은 `UnknownFormat`, 알려진 형식은 모르는 칸으로 거절하고 폴더를 바꾸지 않는다.
//! - 3f 의 두 시점 비교도 저장 경로가 아니다. 같은 사건이 보관 대기(실패 시도)였다가 기준점이 확정된 앞뒤 두 기록도
//!   형식 10/11 은 `UnknownFormat`, 알려진 형식은 모르는 칸으로 각각 거절하고 폴더를 바꾸지 않는다.
//! - 3g 의 위치 구분·대체된 사건의 기준점 추가 거절도 저장 경로가 아니다. 대체된 사건(M2)과 보관 대기 사건(M3)이 든 기록과,
//!   그 M2 에 뒤늦게 기준점이 적힌 기록도 형식 10/11 은 `UnknownFormat`, 알려진 형식은 모르는 칸으로 각각 거절하고 폴더를
//!   바꾸지 않는다. 대체·보관 대기를 담는 새 저장 칸이나 플래그도 없다.
//! - 새 모듈은 `src/artifact/tracking_record/` 밖의 어떤 소스에서도 불리지 않는다.

mod common;
use gil::{ProjectSession, StoreError};
use serde_norway::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// 그 폴더 아래의 모든 파일과 바이트 — 「아무것도 바꾸지 않았다」를 재는 눈.
fn tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                out.insert(path.strip_prefix(root).unwrap().into(), Vec::new());
                walk(root, &path, out);
            } else {
                out.insert(path.strip_prefix(root).unwrap().into(), std::fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn started(label: &str) -> PathBuf {
    let root = common::scratch(&format!("tracking-record-{}-{label}", std::process::id()));
    std::fs::write(root.join("work.txt"), "user file").unwrap();
    let session = ProjectSession::start(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    session.commit().unwrap();
    root
}

fn edit_state(root: &Path, change: impl FnOnce(&mut Value)) {
    let path = root.join(gil::STATE_PATH);
    let mut file: Value = serde_norway::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    change(&mut file);
    std::fs::write(&path, serde_norway::to_string(&file).unwrap()).unwrap();
}

/// 계약 §5.1 꼴의 합성 `tracking` 블록 — 모니터가 승인한 기록이 아니다.
fn synthetic_tracking() -> Value {
    serde_norway::from_str(&format!(
        "next_policy_id: 2\n\
         next_event_id: 1\n\
         policies:\n\
         - id: P1\n  engine: 1\n  rules: {{algorithm: sha256, digest: {d}}}\n  proposal_digest: {d}\n\
         policy_file:\n  sync_base: {{state: absent}}\n\
         events: []\n\
         archive_halt: null\n",
        d = "ab".repeat(32)
    ))
    .unwrap()
}

#[test]
fn this_build_still_refuses_formats_10_and_11_untouched() {
    let known = gil::newest_known_format();
    assert!(known < 10, "이 빌드는 형식 10 을 모른다 (newest_known_format = {known})");
    for format in [10u32, 11] {
        let root = started(&format!("format-{format}"));
        edit_state(&root, |file| {
            file["format"] = Value::from(format);
            file["tracking"] = synthetic_tracking();
        });
        let before = tree(&root);
        let path = root.join(gil::STATE_PATH);
        match gil::load(common::spec(), &path) {
            Err(StoreError::UnknownFormat { found, known: said }) => assert_eq!((found, said), (format, known)),
            other => panic!("형식 {format} 이 읽혔다: {other:?}"),
        }
        assert!(ProjectSession::open(common::spec(), &path).is_err());
        assert!(ProjectSession::open_read_only(common::spec(), &path).is_err());
        assert_eq!(tree(&root), before, "형식 {format} 을 거절하면서 폴더를 바꿨다");
        let _ = std::fs::remove_dir_all(&root);
    }
}

#[test]
fn a_tracking_block_in_a_known_format_is_refused_not_ignored() {
    let root = started("tracking-in-known-format");
    let path = root.join(gil::STATE_PATH);
    let format = gil::load(common::spec(), &path).map(|_| ()).map_err(|err| err.to_string());
    assert_eq!(format, Ok(()), "합성 프로젝트는 읽혀야 한다");

    for (key, value) in [("tracking", synthetic_tracking()), ("policy_file", synthetic_tracking()["policy_file"].clone())] {
        let copy = std::fs::read(&path).unwrap();
        edit_state(&root, |file| file[key] = value);
        let before = tree(&root);
        match gil::load(common::spec(), &path) {
            Err(StoreError::Decode(err)) => assert!(err.to_string().contains(key), "{err}"),
            other => panic!("`{key}` 이 든 기록이 읽혔다: {other:?}"),
        }
        assert!(ProjectSession::open(common::spec(), &path).is_err());
        assert!(ProjectSession::open_read_only(common::spec(), &path).is_err());
        assert_eq!(tree(&root), before, "`{key}` 을 거절하면서 폴더를 바꿨다");
        std::fs::write(&path, copy).unwrap();
    }
    let _ = std::fs::remove_dir_all(&root);
}

/// 계약 §5.1 꼴의 합성 사건 하나 — 실제 사건 기록이 아니다.
fn synthetic_events() -> Value {
    serde_norway::from_str(
        "- id: M1\n  kind: start\n  chain_ref: chain:C1\n  after_cycle_ref: null\n  review_cycle_ref: null\n  \
         next_cycle_id: 1\n  policy_before_ref: null\n  policy_after_ref: policy:P1\n  world_before_ref: null\n  \
         baseline: {after_cycle_ref: null, next_cycle_id: 1, snapshot_ref: snapshot:A1, attempts: [{result: archived}]}\n",
    )
    .unwrap()
}

#[test]
fn records_carrying_management_events_are_refused_untouched() {
    let known = gil::newest_known_format();
    for format in [10u32, 11] {
        let root = started(&format!("events-format-{format}"));
        edit_state(&root, |file| {
            file["format"] = Value::from(format);
            let mut tracking = synthetic_tracking();
            tracking["next_event_id"] = Value::from(2);
            tracking["events"] = synthetic_events();
            file["tracking"] = tracking;
        });
        let before = tree(&root);
        let path = root.join(gil::STATE_PATH);
        match gil::load(common::spec(), &path) {
            Err(StoreError::UnknownFormat { found, known: said }) => assert_eq!((found, said), (format, known)),
            other => panic!("사건이 든 형식 {format} 이 읽혔다: {other:?}"),
        }
        assert!(ProjectSession::open(common::spec(), &path).is_err());
        assert!(ProjectSession::open_read_only(common::spec(), &path).is_err());
        assert_eq!(tree(&root), before, "형식 {format} 을 거절하면서 폴더를 바꿨다");
        let _ = std::fs::remove_dir_all(&root);
    }

    let root = started("events-in-known-format");
    let path = root.join(gil::STATE_PATH);
    edit_state(&root, |file| file["events"] = synthetic_events());
    let before = tree(&root);
    match gil::load(common::spec(), &path) {
        Err(StoreError::Decode(err)) => assert!(err.to_string().contains("events"), "{err}"),
        other => panic!("`events` 가 든 기록이 읽혔다: {other:?}"),
    }
    assert!(ProjectSession::open(common::spec(), &path).is_err());
    assert!(ProjectSession::open_read_only(common::spec(), &path).is_err());
    assert_eq!(tree(&root), before, "`events` 를 거절하면서 폴더를 바꿨다");
    let _ = std::fs::remove_dir_all(&root);
}

/// 계약 §5.1·§5.8 꼴의 합성 사건 둘 — `start`(기준점 있음)와 보관 대기 `scope_change`(기준점 없음, 실패 시도 하나).
/// 실제 사건 기록이 아니다.
fn synthetic_waiting_events() -> Value {
    let mut events = synthetic_events();
    let waiting: Value = serde_norway::from_str(
        "id: M2\nkind: scope_change\nchain_ref: chain:C1\nafter_cycle_ref: cycle:C1\nreview_cycle_ref: cycle:C1\n\
         next_cycle_id: 2\npolicy_before_ref: policy:P1\npolicy_after_ref: policy:P2\nworld_before_ref: snapshot:A1\n\
         baseline: {snapshot_ref: null, attempts: [{result: failed, cause: external_change}]}\n",
    )
    .unwrap();
    events.as_sequence_mut().unwrap().push(waiting);
    events
}

#[test]
fn records_carrying_baselines_and_attempts_are_refused_untouched() {
    let known = gil::newest_known_format();
    for format in [10u32, 11] {
        let root = started(&format!("baselines-format-{format}"));
        edit_state(&root, |file| {
            file["format"] = Value::from(format);
            let mut tracking = synthetic_tracking();
            tracking["next_policy_id"] = Value::from(3);
            let second = tracking["policies"][0].clone();
            tracking["policies"].as_sequence_mut().unwrap().push(second);
            tracking["policies"][1]["id"] = Value::from("P2");
            tracking["next_event_id"] = Value::from(3);
            tracking["events"] = synthetic_waiting_events();
            file["tracking"] = tracking;
        });
        let before = tree(&root);
        let path = root.join(gil::STATE_PATH);
        match gil::load(common::spec(), &path) {
            Err(StoreError::UnknownFormat { found, known: said }) => assert_eq!((found, said), (format, known)),
            other => panic!("기준점·시도가 든 형식 {format} 이 읽혔다: {other:?}"),
        }
        assert!(ProjectSession::open(common::spec(), &path).is_err());
        assert!(ProjectSession::open_read_only(common::spec(), &path).is_err());
        assert_eq!(tree(&root), before, "형식 {format} 을 거절하면서 폴더를 바꿨다");
        let _ = std::fs::remove_dir_all(&root);
    }

    let root = started("baselines-in-known-format");
    let path = root.join(gil::STATE_PATH);
    let baseline = synthetic_waiting_events()[1]["baseline"].clone();
    for (key, value) in [("events", synthetic_waiting_events()), ("baseline", baseline)] {
        let copy = std::fs::read(&path).unwrap();
        edit_state(&root, |file| file[key] = value);
        let before = tree(&root);
        match gil::load(common::spec(), &path) {
            Err(StoreError::Decode(err)) => assert!(err.to_string().contains(key), "{err}"),
            other => panic!("`{key}` 이 든 기록이 읽혔다: {other:?}"),
        }
        assert!(ProjectSession::open(common::spec(), &path).is_err());
        assert!(ProjectSession::open_read_only(common::spec(), &path).is_err());
        assert_eq!(tree(&root), before, "`{key}` 을 거절하면서 폴더를 바꿨다");
        std::fs::write(&path, copy).unwrap();
    }
    let _ = std::fs::remove_dir_all(&root);
}

/// 같은 M2 의 앞뒤 두 기록 — 보관 대기(실패 시도 둘)와, 그 실패를 그대로 둔 채 기준점을 확정한 기록. 합성 값이다.
fn synthetic_before_and_after_events() -> [Value; 2] {
    let mut before = synthetic_waiting_events();
    before[1]["baseline"] = serde_norway::from_str(
        "{snapshot_ref: null, attempts: [{result: failed, cause: external_change}, {result: failed, cause: external_change}]}",
    )
    .unwrap();
    let mut after = before.clone();
    after[1]["baseline"] = serde_norway::from_str(
        "{after_cycle_ref: cycle:C1, next_cycle_id: 2, snapshot_ref: snapshot:A1, \
         attempts: [{result: failed, cause: external_change}, {result: failed, cause: external_change}, {result: archived}]}",
    )
    .unwrap();
    [before, after]
}

#[test]
fn before_and_after_baseline_records_are_each_refused_untouched() {
    let known = gil::newest_known_format();
    for (label, events) in ["before", "after"].into_iter().zip(synthetic_before_and_after_events()) {
        for format in [10u32, 11] {
            let root = started(&format!("history-{label}-format-{format}"));
            edit_state(&root, |file| {
                file["format"] = Value::from(format);
                let mut tracking = synthetic_tracking();
                tracking["next_policy_id"] = Value::from(3);
                let second = tracking["policies"][0].clone();
                tracking["policies"].as_sequence_mut().unwrap().push(second);
                tracking["policies"][1]["id"] = Value::from("P2");
                tracking["next_event_id"] = Value::from(3);
                tracking["events"] = events.clone();
                file["tracking"] = tracking;
            });
            let before = tree(&root);
            let path = root.join(gil::STATE_PATH);
            match gil::load(common::spec(), &path) {
                Err(StoreError::UnknownFormat { found, known: said }) => assert_eq!((found, said), (format, known)),
                other => panic!("{label} 기록의 형식 {format} 이 읽혔다: {other:?}"),
            }
            assert!(ProjectSession::open(common::spec(), &path).is_err());
            assert!(ProjectSession::open_read_only(common::spec(), &path).is_err());
            assert_eq!(tree(&root), before, "{label} 기록의 형식 {format} 을 거절하면서 폴더를 바꿨다");
            let _ = std::fs::remove_dir_all(&root);
        }

        let root = started(&format!("history-{label}-known-format"));
        let path = root.join(gil::STATE_PATH);
        edit_state(&root, |file| file["events"] = events.clone());
        let before = tree(&root);
        match gil::load(common::spec(), &path) {
            Err(StoreError::Decode(err)) => assert!(err.to_string().contains("events"), "{err}"),
            other => panic!("{label} 기록의 `events` 가 읽혔다: {other:?}"),
        }
        assert!(ProjectSession::open(common::spec(), &path).is_err());
        assert!(ProjectSession::open_read_only(common::spec(), &path).is_err());
        assert_eq!(tree(&root), before, "{label} 기록의 `events` 를 거절하면서 폴더를 바꿨다");
        let _ = std::fs::remove_dir_all(&root);
    }
}

/// 같은 프로젝트의 앞뒤 두 기록 — 앞: M2 는 대체됨(실패 하나), M3 이 보관 대기. 뒤: 대체된 M2 에 기준점이 적혔다(3g 가
/// 거절하는 꼴). 합성 값이다.
fn synthetic_replaced_events() -> [Value; 2] {
    let mut before = synthetic_waiting_events();
    let mut third = before[1].clone();
    third["id"] = Value::from("M3");
    third["policy_before_ref"] = Value::from("policy:P2");
    third["policy_after_ref"] = Value::from("policy:P3");
    third["baseline"] = serde_norway::from_str("{snapshot_ref: null, attempts: []}").unwrap();
    before.as_sequence_mut().unwrap().push(third);
    let mut after = before.clone();
    after[1]["baseline"] = serde_norway::from_str(
        "{after_cycle_ref: cycle:C1, next_cycle_id: 2, snapshot_ref: snapshot:A1, \
         attempts: [{result: failed, cause: external_change}, {result: archived}]}",
    )
    .unwrap();
    [before, after]
}

#[test]
fn replaced_and_rearchived_records_are_each_refused_untouched() {
    let known = gil::newest_known_format();
    for (label, events) in ["replaced", "rearchived"].into_iter().zip(synthetic_replaced_events()) {
        for format in [10u32, 11] {
            let root = started(&format!("positions-{label}-format-{format}"));
            edit_state(&root, |file| {
                file["format"] = Value::from(format);
                let mut tracking = synthetic_tracking();
                tracking["next_policy_id"] = Value::from(4);
                for name in ["P2", "P3"] {
                    let mut next = tracking["policies"][0].clone();
                    next["id"] = Value::from(name);
                    tracking["policies"].as_sequence_mut().unwrap().push(next);
                }
                tracking["next_event_id"] = Value::from(4);
                tracking["events"] = events.clone();
                file["tracking"] = tracking;
            });
            let before = tree(&root);
            let path = root.join(gil::STATE_PATH);
            match gil::load(common::spec(), &path) {
                Err(StoreError::UnknownFormat { found, known: said }) => assert_eq!((found, said), (format, known)),
                other => panic!("{label} 기록의 형식 {format} 이 읽혔다: {other:?}"),
            }
            assert!(ProjectSession::open(common::spec(), &path).is_err());
            assert!(ProjectSession::open_read_only(common::spec(), &path).is_err());
            assert_eq!(tree(&root), before, "{label} 기록의 형식 {format} 을 거절하면서 폴더를 바꿨다");
            let _ = std::fs::remove_dir_all(&root);
        }

        let root = started(&format!("positions-{label}-known-format"));
        let path = root.join(gil::STATE_PATH);
        edit_state(&root, |file| file["events"] = events.clone());
        let before = tree(&root);
        match gil::load(common::spec(), &path) {
            Err(StoreError::Decode(err)) => assert!(err.to_string().contains("events"), "{err}"),
            other => panic!("{label} 기록의 `events` 가 읽혔다: {other:?}"),
        }
        assert!(ProjectSession::open(common::spec(), &path).is_err());
        assert!(ProjectSession::open_read_only(common::spec(), &path).is_err());
        assert_eq!(tree(&root), before, "{label} 기록의 `events` 를 거절하면서 폴더를 바꿨다");
        let _ = std::fs::remove_dir_all(&root);
    }
}

#[test]
fn no_product_source_calls_the_new_module() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let own = src.join("artifact").join("tracking_record");
    let names = [
        "tracking_record",
        "PolicyLedger",
        "check_policy_sources",
        "check_append_only",
        "classify_policy_file",
        "StoredSyncBase",
        "PolicySourcesMatch",
        "sync_order",
        "PolicyFileSync",
        "begin_cleanup",
        "SyncRequest",
        "base_for_new_policy",
        "event_links",
        "check_policy_links",
        "EventPolicyLink",
        "PolicyLinksAgree",
        "LinkProblem",
        "EventKind",
        "EventNamesError",
        "LINKS_DO_NOT_SHOW",
        "link_history",
        "check_link_append_only",
        "check_policy_history",
        "LinkAppendViolation",
        "LinkSlot",
        "HistoryProblem",
        "HistoryKept",
        "HISTORY_DOES_NOT_SHOW",
        "baseline_record",
        "check_baselines",
        "BaselineFacts",
        "BaselineSlot",
        "BaselineField",
        "BaselineProblem",
        "BaselinesAgree",
        "AttemptSeen",
        "BASELINES_DO_NOT_SHOW",
        "baseline_history",
        "check_baseline_history",
        "BaselineHistoryKept",
        "BaselineHistoryProblem",
        "BaselineChange",
        "BaselineUpdate",
        "MatchProblem",
        "BASELINE_HISTORY_DOES_NOT_SHOW",
        "first_name_out_of_place",
        "baseline_position",
        "classify_baseline_positions",
        "check_replaced_baselines",
        "BaselinePosition",
        "PositionProblem",
        "ReplacedBaselineProblem",
        "ReplacedBaselinesKept",
        "ReplacedGainedBaseline",
        "WaitingInList",
        "ReplacedInList",
        "unjudged_late_attempts",
        "POSITIONS_DO_NOT_SHOW",
    ];
    let mut users = Vec::new();
    let mut stack = vec![src.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                if path != own {
                    stack.push(path);
                }
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            for (number, line) in text.lines().enumerate() {
                if names.iter().any(|name| line.contains(name)) {
                    users.push(format!("{}:{}: {}", path.strip_prefix(&src).unwrap().display(), number + 1, line.trim()));
                }
            }
        }
    }
    // 허용하는 것은 `src/artifact/mod.rs` 의 모듈 선언 한 줄뿐이다(재수출 없음).
    let allowed = |line: &str| line.ends_with(": mod tracking_record;");
    let outside: Vec<_> = users.iter().filter(|line| !allowed(line)).collect();
    assert!(outside.is_empty(), "새 모듈이 제품 소스에서 쓰였다:\n{outside:#?}");
    assert!(users.iter().any(|line| line.ends_with(": mod tracking_record;")), "{users:#?}");
}
