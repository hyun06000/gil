//! 구현 순서 3a(정책 기록 자료형과 부분 검증)와 그 뒤의 동기화 순서 제어(편의상 3b), 정책 원장과
//! 관리 사건의 정책 연결 검사(편의상 3c), 그 두 시점의 추가 전후 보존 검사(편의상 3d), 기준점·보관 시도 기록의
//! 한 시점 일관성 검사(편의상 3e), 그 두 시점 비교(편의상 3f), 보관 대기·대체 위치 구분과 대체된 사건의 기준점 추가 거절
//! (편의상 3g), 실제 사이클 참조의 존재·닫힘·체인 소속과 발급 번호 관계 검사(편의상 3h)가 **제품 저장 경로를 바꾸지 않았는가.**
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
//! - 3h 의 사이클 참조 검사도 저장 경로가 아니다. 과거 경계로 되돌아간 사건·같은 경계의 연속 사건·대기 중 인터뷰 뒤 다른 경계에서
//!   보관한 사건처럼 실제 사이클 참조와 발급 번호가 든 기록도 형식 10/11 은 `UnknownFormat`, 알려진 형식은 모르는 칸으로 거절하고
//!   폴더를 바꾸지 않는다. 사건 칸이 사이클을 가리켜도 로더가 그 참조를 읽어 따르는 길이 생기지 않았다.
//! - 3i(편의상)의 승인 당시 세 칸 두 시점 보존 검사도 저장 경로가 아니다. 같은 사건의 앞(승인 경계에서 대기) 기록, 다른 경계에서
//!   보관한 뒤(세 칸 그대로) 기록, 승인 경계·발급 번호를 고쳐 쓴 뒤 기록도 형식 10/11 은 `UnknownFormat`, 알려진 형식은 모르는
//!   칸으로 각각 거절하고 폴더를 바꾸지 않는다. 앞뒤 기록을 견주어 저장하는 길도 생기지 않았다.
//! - 3j(편의상)의 한 시점 정책 연결·사이클 입력 대응 검사도 저장 경로가 아니다. 이 검사는 건네받은 값끼리만 견주며 실제 상태에서
//!   입력을 뽑는 길이 없다. 사건이 든 기록의 거절(형식 10/11 은 `UnknownFormat`, 알려진 형식은 모르는 칸)은 위 시험들 그대로이고,
//!   새 모듈의 이름도 아래 제품 호출 부재 검사에 들어 있다. 두 부분 입력이 함께 뽑힐 한 사건 목록(`restore_result` 포함)이 든 기록도
//!   형식 10/11 은 `UnknownFormat`, 알려진 형식은 모르는 칸으로 거절하고 폴더를 바꾸지 않는다.
//! - 루트 `.gilignore` 의 읽기 전용 실제 관측(`policy_file_read`)도 저장 경로가 아니다. 제품의 시작·관측·보관·복원·동기화가
//!   그것을 부르지 않으므로 아래 제품 호출 부재 검사에 그 이름이 들어 있다.
//! - 경로 하나의 계속 추적 판정(`path_continuity`, 편의상 3n)도 저장 경로가 아니다. 건넨 정책 구간과 경로 글자만 보고
//!   원장·상태·폴더를 읽지 않으며, 제품의 변경 검사·승인·보관·복원이 그것을 부르지 않으므로 아래 검사에 그 이름이 들어 있다.
//! - 건넨 정책 원장에서 명시된 시작 판부터 마지막 판까지 구간을 고르는 선택(`ledger_span`, 편의상 3o)도 저장 경로가 아니다.
//!   메모리의 원장·시작점·이미 해석된 정책 자료만 받고 `state.yaml`·폴더·객체 창고를 읽지 않으며, 제품 경로가 그것을 부르지
//!   않으므로 아래 검사에 그 이름이 들어 있다.
//! - 사이클 N 을 열 당시의 적용 정책 P(N) 선택(`opening_policy`, 계약 §6.2)도 저장 경로가 아니다. 메모리의 원장·사건 입력·Chain 목록·
//!   등록부만 받아 같은 호출에서 3j 를 실행하고, 파일·`state.yaml`·객체 창고를 읽지 않으며 3n·3o 의 시작점·구간을 만들지 않는다.
//!   제품 경로가 그것을 부르지 않으므로 아래 검사에 그 이름이 들어 있고, 그 원문의 코드 줄은 따로 검사한다.
//! - 닫힌 사이클 T 의 경계 세계 B(T) 출처 선택(`boundary_world`, 계약 §6.1)도 저장 경로가 아니다. 메모리의 원장·사건 입력·Chain 목록·
//!   등록부만 받아 같은 호출에서 3j 를 실행하고, 파일·`state.yaml`·객체 창고를 읽지 않으며 3n·3o 의 시작점·구간이나 P(N) 선택을
//!   쓰지 않는다. 제품 경로가 그것을 부르지 않으므로 아래 검사에 그 이름이 들어 있고, 그 원문의 코드 줄은 따로 검사한다.
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

/// 실제 사이클 참조와 발급 번호가 든 사건 넷 — `start`(경계 `null`), C1 에서 승인·보관(발급 2), 같은 C1 경계의 대기(발급 2,
/// 대체됨), 과거 경계 C1 에서 승인하고 대기 중 인터뷰 C2 뒤 C2 에서 보관(발급 3). 3h 가 받아들이는 꼴의 합성 값이며 실제 사건
/// 기록이 아니다.
fn synthetic_cycle_ref_events() -> Value {
    let mut events = synthetic_events();
    let more: Value = serde_norway::from_str(
        "- id: M2\n  kind: scope_change\n  chain_ref: chain:C1\n  after_cycle_ref: cycle:C1\n  review_cycle_ref: cycle:C1\n  \
         next_cycle_id: 2\n  policy_before_ref: policy:P1\n  policy_after_ref: policy:P2\n  world_before_ref: snapshot:A1\n  \
         baseline: {after_cycle_ref: cycle:C1, next_cycle_id: 2, snapshot_ref: snapshot:A1, attempts: [{result: archived}]}\n\
         - id: M3\n  kind: scope_change\n  chain_ref: chain:C1\n  after_cycle_ref: cycle:C1\n  review_cycle_ref: cycle:C1\n  \
         next_cycle_id: 2\n  policy_before_ref: policy:P2\n  policy_after_ref: policy:P3\n  world_before_ref: snapshot:A1\n  \
         baseline: {snapshot_ref: null, attempts: []}\n\
         - id: M4\n  kind: scope_change\n  chain_ref: chain:C1\n  after_cycle_ref: cycle:C1\n  review_cycle_ref: cycle:C1\n  \
         next_cycle_id: 2\n  policy_before_ref: policy:P3\n  policy_after_ref: policy:P4\n  world_before_ref: snapshot:A1\n  \
         baseline: {after_cycle_ref: cycle:C2, next_cycle_id: 3, snapshot_ref: snapshot:A1, \
         attempts: [{result: failed, cause: external_change}, {result: archived}]}\n",
    )
    .unwrap();
    events.as_sequence_mut().unwrap().extend(more.as_sequence().unwrap().iter().cloned());
    events
}

#[test]
fn records_carrying_cycle_references_are_refused_untouched() {
    let known = gil::newest_known_format();
    let events = synthetic_cycle_ref_events();
    for format in [10u32, 11] {
        let root = started(&format!("cycle-refs-format-{format}"));
        edit_state(&root, |file| {
            file["format"] = Value::from(format);
            let mut tracking = synthetic_tracking();
            tracking["next_policy_id"] = Value::from(5);
            for name in ["P2", "P3", "P4"] {
                let mut next = tracking["policies"][0].clone();
                next["id"] = Value::from(name);
                tracking["policies"].as_sequence_mut().unwrap().push(next);
            }
            tracking["next_event_id"] = Value::from(5);
            tracking["events"] = events.clone();
            file["tracking"] = tracking;
        });
        let before = tree(&root);
        let path = root.join(gil::STATE_PATH);
        match gil::load(common::spec(), &path) {
            Err(StoreError::UnknownFormat { found, known: said }) => assert_eq!((found, said), (format, known)),
            other => panic!("사이클 참조가 든 형식 {format} 이 읽혔다: {other:?}"),
        }
        assert!(ProjectSession::open(common::spec(), &path).is_err());
        assert!(ProjectSession::open_read_only(common::spec(), &path).is_err());
        assert_eq!(tree(&root), before, "형식 {format} 을 거절하면서 폴더를 바꿨다");
        let _ = std::fs::remove_dir_all(&root);
    }

    let root = started("cycle-refs-known-format");
    let path = root.join(gil::STATE_PATH);
    edit_state(&root, |file| file["events"] = events.clone());
    let before = tree(&root);
    match gil::load(common::spec(), &path) {
        Err(StoreError::Decode(err)) => assert!(err.to_string().contains("events"), "{err}"),
        other => panic!("사이클 참조가 든 `events` 가 읽혔다: {other:?}"),
    }
    assert!(ProjectSession::open(common::spec(), &path).is_err());
    assert!(ProjectSession::open_read_only(common::spec(), &path).is_err());
    assert_eq!(tree(&root), before, "`events` 를 거절하면서 폴더를 바꿨다");
    let _ = std::fs::remove_dir_all(&root);
}

/// 같은 M2 의 앞뒤 세 기록 — 앞: C1 에서 승인(발급 2)하고 실패 하나로 대기. 뒤: 대기 중 인터뷰 C2 뒤 C2 에서 보관했고 승인 칸은
/// 그대로(3i 가 받아들이는 꼴). 고쳐 쓴 뒤: 같은 보관에 맞춰 승인 경계·발급 번호를 C2·3 으로 고쳤다(3i 가 거절하는 꼴). 합성 값이다.
fn synthetic_approval_history_events() -> [(&'static str, Value); 3] {
    let before = synthetic_waiting_events();
    let mut kept = before.clone();
    kept[1]["baseline"] = serde_norway::from_str(
        "{after_cycle_ref: cycle:C2, next_cycle_id: 3, snapshot_ref: snapshot:A1, \
         attempts: [{result: failed, cause: external_change}, {result: archived}]}",
    )
    .unwrap();
    let mut rewritten = kept.clone();
    rewritten[1]["after_cycle_ref"] = Value::from("cycle:C2");
    rewritten[1]["next_cycle_id"] = Value::from(3);
    [("before", before), ("kept", kept), ("rewritten", rewritten)]
}

#[test]
fn before_and_after_approval_records_are_each_refused_untouched() {
    let known = gil::newest_known_format();
    for (label, events) in synthetic_approval_history_events() {
        for format in [10u32, 11] {
            let root = started(&format!("approval-{label}-format-{format}"));
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

        let root = started(&format!("approval-{label}-known-format"));
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

/// 3c 입력(이름·종류·앞뒤 정책)과 3h 입력(이름·종류·체인·경계·발급 번호·baseline)이 함께 뽑힐 한 `events` 목록 — M2 는 `restore_result`.
/// 대응 검사(3j)가 받아들이는 꼴의 합성 값이며, 저장 경로는 여전히 이 기록을 읽지 않는다.
fn synthetic_single_list_events() -> Value {
    let mut events = synthetic_events();
    let more: Value = serde_norway::from_str(
        "- id: M2\n  kind: restore_result\n  chain_ref: chain:C1\n  after_cycle_ref: cycle:C1\n  review_cycle_ref: null\n  \
         next_cycle_id: 2\n  policy_before_ref: policy:P1\n  policy_after_ref: policy:P1\n  world_before_ref: snapshot:A1\n  \
         baseline: {snapshot_ref: null, attempts: [{result: failed, cause: external_change}]}\n",
    )
    .unwrap();
    events.as_sequence_mut().unwrap().extend(more.as_sequence().unwrap().iter().cloned());
    events
}

#[test]
fn a_single_event_list_for_both_partial_inputs_is_refused_untouched() {
    let known = gil::newest_known_format();
    let events = synthetic_single_list_events();
    for format in [10u32, 11] {
        let root = started(&format!("single-list-format-{format}"));
        edit_state(&root, |file| {
            file["format"] = Value::from(format);
            let mut tracking = synthetic_tracking();
            tracking["next_event_id"] = Value::from(3);
            tracking["events"] = events.clone();
            file["tracking"] = tracking;
        });
        let before = tree(&root);
        let path = root.join(gil::STATE_PATH);
        match gil::load(common::spec(), &path) {
            Err(StoreError::UnknownFormat { found, known: said }) => assert_eq!((found, said), (format, known)),
            other => panic!("한 사건 목록이 든 형식 {format} 이 읽혔다: {other:?}"),
        }
        assert!(ProjectSession::open(common::spec(), &path).is_err());
        assert!(ProjectSession::open_read_only(common::spec(), &path).is_err());
        assert_eq!(tree(&root), before, "형식 {format} 을 거절하면서 폴더를 바꿨다");
        let _ = std::fs::remove_dir_all(&root);
    }

    let root = started("single-list-known-format");
    let path = root.join(gil::STATE_PATH);
    edit_state(&root, |file| file["events"] = events.clone());
    let before = tree(&root);
    match gil::load(common::spec(), &path) {
        Err(StoreError::Decode(err)) => assert!(err.to_string().contains("events"), "{err}"),
        other => panic!("한 사건 목록이 든 `events` 가 읽혔다: {other:?}"),
    }
    assert!(ProjectSession::open(common::spec(), &path).is_err());
    assert!(ProjectSession::open_read_only(common::spec(), &path).is_err());
    assert_eq!(tree(&root), before, "`events` 를 거절하면서 폴더를 바꿨다");
    let _ = std::fs::remove_dir_all(&root);
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
        "NewPolicyBase",
        "ThreeWayDifference",
        "HOLD_CALLER_DUTIES",
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
        "event_cycles",
        "check_cycle_refs",
        "EventCycleFacts",
        "CycleRefsAgree",
        "CycleRefProblem",
        "CycleField",
        "RefReading",
        "EventRefs",
        "LookupProblem",
        "ChainLookup",
        "NullNotJudged",
        "UnwrittenNotJudged",
        "chains_not_looked_up",
        "CYCLE_REFS_DO_NOT_SHOW",
        "event_cycle_history",
        "check_event_cycle_history",
        "EventCycleHistoryKept",
        "EventCycleHistoryProblem",
        "ApprovalChange",
        "ApprovalField",
        "PastApproval",
        "BeforeRefs",
        "AfterRefs",
        "match_events",
        "boundary_text",
        "APPROVAL_HISTORY_DOES_NOT_SHOW",
        "event_inputs",
        "check_event_inputs",
        "EventInputsAgree",
        "EventInputsProblem",
        "InputMismatch",
        "InputSide",
        "CountDiffers",
        "NameDiffers",
        "KindDiffers",
        "EVENT_INPUTS_DO_NOT_SHOW",
        "policy_file_read",
        "read_root_policy_file",
        "ReadProblem",
        "source_store",
        "store_policy_source",
        "PolicySourceStored",
        "SourceStoreProblem",
        "StoredAtOtherAddress",
        "refused_before_write",
        "SOURCE_STORE_DOES_NOT_SHOW",
        "path_continuity",
        "classify_path_continuity",
        "AppliedSpan",
        "ConfirmedStart",
        "BeforePolicyAdoption",
        "PathContinuity",
        "ContinuouslyTracked",
        "NewlyTracked",
        "OutsideCurrentScope",
        "is_comparison_target",
        "SpanProblem",
        "NoAppliedPolicy",
        "legacy_observation_contains",
        "earlier_not_containing",
        "CONTINUITY_DOES_NOT_SHOW",
        "ledger_span",
        "select_ledger_span",
        "classify_path_over_ledger",
        "LedgerSpan",
        "LedgerStart",
        "BoardPolicies",
        "BoardPolicyProblem",
        "StartNotInLedger",
        "NotInLedger",
        "GivenTwice",
        "differing_fields",
        "applied_board",
        "board_at",
        "LEDGER_SPAN_DOES_NOT_SHOW",
        "opening_policy",
        "select_opening_policy",
        "OpeningPolicy",
        "OpeningPolicyProblem",
        "OpeningSelection",
        "BeforeAdoptionInGivenHistory",
        "TargetProblem",
        "SeveralOwners",
        "OwnerChainRepeated",
        "OPENING_POLICY_DOES_NOT_SHOW",
        "boundary_world",
        "select_boundary_world",
        "BoundaryWorld",
        "BoundaryWorldProblem",
        "BoundarySource",
        "BoundaryPolicy",
        "BoundaryTargetProblem",
        "EventBaseline",
        "ExitOfTarget",
        "ExitPolicyNotDerived",
        "ExitNotInRegistry",
        "TargetNotClosed",
        "ClosedWithoutExit",
        "archive_issuer",
        "BOUNDARY_WORLD_DOES_NOT_SHOW",
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

/// P(N) 선택의 원문에서 주석과 문자열 목록 줄을 뺀 **코드 줄**이 파일·상태·창고·3n/3o 의 문을 쓰지 않고, 같은 호출의 입력으로 3j 를
/// 부르며, 다른 입력에서 얻은 3j 성공 값을 받는 함수가 없는가.
#[test]
fn the_opening_policy_selection_touches_no_file_state_or_store_and_builds_no_span() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("artifact").join("tracking_record").join("opening_policy.rs");
    let text = std::fs::read_to_string(&path).unwrap();
    let code: Vec<&str> =
        text.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with("//") && !line.starts_with('"')).collect();
    let forbidden = [
        "std::fs",
        "std::io",
        "std::env",
        "File",
        "OpenOptions",
        "read_to_string",
        "ObjectStore",
        "Project",
        "state.yaml",
        "gilignore",
        "read_root_policy_file",
        "store_policy_source",
        "LedgerStart",
        "ConfirmedStart",
        "AppliedSpan",
        "BoardPolicies",
        "select_ledger_span",
        "classify_path",
    ];
    let used: Vec<String> = code
        .iter()
        .flat_map(|line| forbidden.iter().filter(|name| line.contains(*name)).map(move |name| format!("{name}: {line}")))
        .collect();
    assert!(used.is_empty(), "P(N) 선택이 파일·상태·창고·구간의 문을 쓴다:\n{used:#?}");
    let calls: Vec<&&str> = code.iter().filter(|line| line.contains("check_event_inputs(")).collect();
    assert_eq!(calls, [&"let inputs = check_event_inputs(ledger, links, facts, chains, registry);"], "같은 호출의 입력으로 3j 를 한 번 불러야 한다");
    // 함수의 인자 자리(한 줄 서명의 괄호 안, 또는 여러 줄 서명의 `이름: &타입,` 줄)에 성공 값 타입이 있으면 안 된다.
    let parameters = |line: &str| match (line.contains("fn "), line.find('('), line.find(')')) {
        (true, Some(open), Some(close)) if open < close => line[open..close].to_string(),
        _ if line.contains(": &") && line.ends_with(',') => line.to_string(),
        _ => String::new(),
    };
    let takes_agreement: Vec<&&str> = code.iter().filter(|line| parameters(line).contains("Agree")).collect();
    assert!(takes_agreement.is_empty(), "3j 성공 값을 받는 문이 있다: {takes_agreement:#?}");
}

/// B(T) 선택의 원문에서 주석과 문자열 목록 줄을 뺀 **코드 줄**이 파일·상태·창고·3n/3o·P(N) 의 문을 쓰지 않고, 같은 호출의 입력으로
/// 3j 를 부르며, 다른 입력에서 얻은 3j 성공 값을 받는 함수가 없는가.
#[test]
fn the_boundary_world_selection_touches_no_file_state_or_store_and_builds_no_span() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("artifact").join("tracking_record").join("boundary_world.rs");
    let text = std::fs::read_to_string(&path).unwrap();
    let code: Vec<&str> =
        text.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with("//") && !line.starts_with('"')).collect();
    let forbidden = [
        "std::fs",
        "std::io",
        "std::env",
        "File",
        "OpenOptions",
        "read_to_string",
        "ObjectStore",
        "Project",
        "state.yaml",
        "gilignore",
        "read_root_policy_file",
        "store_policy_source",
        "LedgerStart",
        "ConfirmedStart",
        "AppliedSpan",
        "BoardPolicies",
        "select_ledger_span",
        "classify_path",
        "select_opening_policy",
        "OpeningSelection",
        "put_blob",
    ];
    let used: Vec<String> = code
        .iter()
        .flat_map(|line| forbidden.iter().filter(|name| line.contains(*name)).map(move |name| format!("{name}: {line}")))
        .collect();
    assert!(used.is_empty(), "B(T) 선택이 파일·상태·창고·구간·P(N) 의 문을 쓴다:\n{used:#?}");
    let calls: Vec<&&str> = code.iter().filter(|line| line.contains("check_event_inputs(")).collect();
    assert_eq!(calls, [&"let inputs = check_event_inputs(ledger, links, facts, chains, registry);"], "같은 호출의 입력으로 3j 를 한 번 불러야 한다");
    // 함수의 인자 자리(한 줄 서명의 괄호 안, 또는 여러 줄 서명의 `이름: &타입,` 줄)에 성공 값 타입이 있으면 안 된다.
    let parameters = |line: &str| match (line.contains("fn "), line.find('('), line.find(')')) {
        (true, Some(open), Some(close)) if open < close => line[open..close].to_string(),
        _ if line.contains(": &") && line.ends_with(',') => line.to_string(),
        _ => String::new(),
    };
    let takes_agreement: Vec<&&str> = code.iter().filter(|line| parameters(line).contains("Agree")).collect();
    assert!(takes_agreement.is_empty(), "3j 성공 값을 받는 문이 있다: {takes_agreement:#?}");
}
