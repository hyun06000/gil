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
//! - 비뿌리 사이클 N 의 세계 출처 S(N) 선택과 검증 ①(`opening_world`, 계약 §6.2)도 저장 경로가 아니다. 메모리의 원장·사건 입력·Chain
//!   목록·등록부만 받아 같은 호출에서 3j 를 한 번 실행하고, 파일·`state.yaml`·객체 창고를 읽지 않으며 3n·3o 의 시작점·구간이나 P(N)·B(T)
//!   선택을 쓰지 않는다. 제품의 열기·load 가 그것을 부르지 않으므로 아래 검사에 그 이름이 들어 있고, 그 원문의 코드 줄은 따로 검사한다.
//! - 닫힌 Experiment T 의 Exit 세계와 그 확정 정책 유도(`experiment_exit`, 계약 §6.1 의 「Experiment 의 Exit」 줄)도 저장 경로가 아니다.
//!   메모리의 원장·사건 입력·Chain 목록·등록부만 받아 같은 호출에서 P(N) 선택을 한 번 부르고(3j 는 그 안에서만 실행된다), 파일·
//!   `state.yaml`·객체 창고를 읽지 않으며 3n·3o 의 시작점·구간이나 B(T)·S(N) 선택을 쓰지 않는다. 제품 경로가 그것을 부르지 않으므로
//!   아래 검사에 그 이름이 들어 있고, 그 원문의 코드 줄은 따로 검사한다.
//! - 닫힌 Experiment T 의 저장 Exit 과 마지막 Outcome 계보의 기대 세계 비교(`exit_lineage`, Artifact Model §7.5)도 저장 경로가 아니다.
//!   메모리의 Chain 목록·등록부만 받아 기존 계보 조회(`Walk::lineage`)를 한 번 쓰고, 정책 원장·사건 입력·파일·`state.yaml`·객체 창고를
//!   읽지 않으며 Cycle 을 닫는 규칙(`exit_would_be`)·P(N)·B(T)·S(N)·Experiment Exit 선택을 부르지 않는다. 제품의 close·load 가 그것을
//!   부르지 않으므로 아래 검사에 그 이름이 들어 있고, 그 원문의 코드 줄은 따로 검사한다.
//! - Experiment 안의 지정한 닫힌 Verify 가 확정한 세계와 그 확정 정책 유도(`verify_world`, 계약 §6.1 의 「Experiment 의 Verify」 줄)도 저장
//!   경로가 아니다. 메모리의 원장·사건 입력·Chain 목록·등록부만 받아 같은 호출에서 P(N) 선택을 한 번 부르고(3j 는 그 안에서만 실행된다),
//!   Experiment Exit 선택과 같은 소유 조회를 한 번 쓰며, 파일·`state.yaml`·객체 창고를 읽지 않고 계보 선택·현재 세계 유도·Exit·B(T)·S(N)·
//!   3n·3o 의 문을 쓰지 않는다. 제품 경로가 그것을 부르지 않으므로 아래 검사에 그 이름이 들어 있고, 그 원문의 코드 줄은 따로 검사한다.
//! - 범위 변경 사건의 세 값 읽기(`confirmed_world`, 계약 §6.1·§6.3)도 저장 경로가 아니다. 메모리의 원장·사건 입력·보충 칸·Chain 목록·
//!   등록부만 받아 같은 호출에서 기존 검사·선택(3j·3g·B(T)·S(N)·Experiment Exit·Exit 계보)을 다시 실행하고, 폴더 관측·객체 창고·상태
//!   저장으로 나가는 통로가 없으며 호출자가 고른 확정 정책·성공 값을 받는 문이 없다. 제품 경로가 그것을 부르지 않으므로 아래 검사에 그
//!   이름이 들어 있고, 그 원문의 코드 줄은 따로 검사한다.
//! - 보관 대기 사건의 명시 재시도(`baseline_retry`, 계약 §5.3·§5.4·§6.3)도 제품 저장 경로가 아니다. `tracking-policy` + Unix 에서만
//!   컴파일되고, 기존 `observe_scoped`·`capture_scoped`·객체 창고와 호출자가 건넨 내부 저장 경계만 부르며 `state.yaml`·ProjectSession·CLI·
//!   MCP·Monitor 에 닿지 않는다. 한 시도의 관측 루트·객체 창고·저장 경계는 시작할 때 한 번 정해지고 걸음마다 다시 받는 문이 없다.
//!   제품 경로가 그것을 부르지 않으므로 아래 검사에 그 이름이 들어 있고, 그 원문의 코드 줄은 따로 검사한다.
//! - 새 모듈은 `src/artifact/tracking_record/` 밖의 어떤 소스에서도 불리지 않는다 — **예외는 `tracking-policy` 개발 구성에서만
//!   컴파일되는 구현 순서 4번의 연결 파일**(시작 준비 `src/start_prep/`, 형식 10/11 기록 `src/artifact/tracking_state.rs`, 그 재수출
//!   블록 `start_policy`)과, **구현 순서 5번(같은 Chain 범위 변경, 2026-11-24 승인)의 정확한 연결**(재수출 블록 `tracking` 의 고정된
//!   이름 집합, `src/scope_change/` 세 파일의 고정된 참조 집합, 그것을 부르는 제품 줄의 `tracking-policy` + Unix 문)뿐이며, 그 파일들이
//!   실제로 그 기능 뒤에 있는지도 함께 본다(2026-11-23, 5번은 2026-11-24). 이 기능을 켠 빌드는 형식 10/11 을
//!   읽으므로 아래 「알지 못한다」 시험들은 이 기능이 꺼진 빌드에서만 짓는다. 켠 빌드의 거절은 `tests/tracking_start.rs` 가 본다.

#![cfg(not(feature = "tracking-policy"))]
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

/// 구현 순서 4·5번의 제품 연결이 서는 정확한 구성 조건 — 이 한 줄이 아니면 문이 아니다.
const UNIX_TRACKING_GATE: &str = "#[cfg(all(feature = \"tracking-policy\", unix))]";

/// 구현 순서 5번(같은 Chain 범위 변경, 2026-11-24 승인)의 재수출 블록 `artifact::tracking` — 공백만 정규화한 **블록 전체**. 블록의
/// 어느 이름이 늘거나 줄거나, 재시도 재수출의 `#[cfg(unix)]` 가 빠지면 이 글과 달라진다.
const TRACKING_REEXPORT_BLOCK: &str = concat!(
    "pub(crate) mod tracking { pub(crate) use super::tracking_record::{ AppliedPolicy, EventKind, EventName, ",
    "LastConfirmed, MarkSeen, PolicyFileReading, PolicyName, PolicyStep, RecordInputs, RecordStanding, ",
    "StandingAt, classify_policy_file, }; pub(crate) use super::tracking_state::{ATTEMPT_CAUSES, Archived, ",
    "Extracted, HaltAt, TrackedEvent}; #[cfg(unix)] pub(crate) use super::tracking_record::{ AfterFailure, ",
    "BaselineCommit, BaselineState, BoardPolicies, EventStanding, FailedAttempt, FailureCause, FailureRecord, ",
    "LedgerStart, NotStarted, OwnedRecords, RetryAttempt, RetryOutcome, RetryRequest, SaveAnswer, ScopeDelta, ",
    "UnknownWhy, run_retry, scope_delta_over, select_ledger_span, }; }",
);

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// 한 줄의 식별자 낱말들.
fn identifiers(line: &str) -> Vec<&str> {
    line.split(|c: char| !is_name_char(c)).filter(|word| !word.is_empty()).collect()
}

/// 한 소스가 금지 이름을 **어떻게** 담고 있는가. 줄 단위 부분 문자열 검색이 잡는 모든 줄을 셋으로 가른다.
#[derive(Debug, Default, PartialEq)]
struct NameUses {
    /// 코드 줄에서 낱말 전체가 금지 이름인 것 — 실제 참조.
    referenced: std::collections::BTreeSet<String>,
    /// 코드 줄에서 금지 이름을 **속에 품은 다른 낱말**(예: `RetryRequested` 속의 `RetryRequest`) — 그 모듈의 참조가 아니다.
    inside_other_names: std::collections::BTreeSet<String>,
    /// 금지 이름이 주석 줄에만 있는 줄의 수 — 참조가 아니다.
    comment_lines: usize,
}

/// 금지 이름은 모두 식별자이므로, 주석이 아닌 줄의 부분 문자열 일치는 반드시 그 줄의 어느 낱말 **전체**이거나 어느 낱말의 **속**이다.
/// 따라서 기존 검색(`line.contains(name)`)이 잡던 코드 줄은 하나도 빠지지 않고 `referenced` 나 `inside_other_names` 에 드러난다.
/// 주석으로 치는 것은 `//` 로 **시작하는** 줄뿐이다 — 코드 뒤의 꼬리 주석은 코드로 본다(더 엄격한 쪽).
fn name_uses(text: &str, names: &[&str]) -> NameUses {
    let mut uses = NameUses::default();
    for line in text.lines().map(str::trim) {
        if !names.iter().any(|name| line.contains(name)) {
            continue;
        }
        if line.starts_with("//") {
            uses.comment_lines += 1;
            continue;
        }
        for word in identifiers(line) {
            if names.contains(&word) {
                uses.referenced.insert(word.to_string());
            } else if names.iter().any(|name| word.contains(name)) {
                uses.inside_other_names.insert(word.to_string());
            }
        }
    }
    uses
}

fn mentions_path(line: &str, needle: &str) -> bool {
    line.match_indices(needle).any(|(at, _)| !line[..at].chars().next_back().is_some_and(is_name_char))
}

/// `needle`(예: `scope::`)을 쓰는 코드 줄의 수와, 그 가운데 [`UNIX_TRACKING_GATE`] 뒤에 있지 **않은** 줄들.
///
/// 문 뒤로 치는 것은 셋뿐이다: 같은 줄에 그 속성이 있다 · 바로 앞 줄(사이의 주석과 다른 속성 줄은 건너뛴다)이 그 속성이다 · 그렇게
/// 문 뒤에 선 줄이 연 중괄호 구간 안이다. 조건이 한 글자라도 다른 속성(`unix` 가 빠진 것 등)은 문이 아니다.
fn ungated_uses(text: &str, needle: &str) -> (usize, Vec<String>) {
    let (mut uses, mut ungated) = (0, Vec::new());
    let mut cover: Option<i64> = None;
    let mut after_gate = false;
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        let same_line = line.contains(UNIX_TRACKING_GATE);
        let gate_only = line == UNIX_TRACKING_GATE;
        if mentions_path(line, needle) {
            uses += 1;
            if !(same_line || after_gate || cover.is_some()) {
                ungated.push(line.to_string());
            }
        }
        let opened = line.matches('{').count() as i64 - line.matches('}').count() as i64;
        cover = match cover {
            Some(depth) => Some(depth + opened).filter(|depth| *depth > 0),
            None => ((after_gate || (same_line && !gate_only)) && opened > 0).then_some(opened),
        };
        after_gate = gate_only || (after_gate && line.starts_with("#["));
    }
    (uses, ungated)
}

/// `src/artifact/mod.rs` 에서 `opening` 으로 여는 재수출 블록 — (여는 줄 번호, 닫는 줄 번호, 공백을 정규화한 블록 전체).
/// 여는 줄 바로 위에 이어진 속성 줄들 가운데 `#[cfg(feature = "tracking-policy")]` 가 없으면 블록으로 치지 않는다.
fn gated_reexport_block(mod_rs: &str, opening: &str) -> Result<(usize, usize, String), String> {
    let lines: Vec<&str> = mod_rs.lines().collect();
    let start = lines.iter().position(|line| *line == opening).ok_or_else(|| format!("`{opening}` 블록이 없다"))?;
    let end = start + lines[start..].iter().position(|line| *line == "}").ok_or("블록의 끝이 없다")?;
    let attributes: Vec<&str> = lines[..start].iter().rev().take_while(|line| line.trim_start().starts_with("#[")).copied().collect();
    if !attributes.contains(&"#[cfg(feature = \"tracking-policy\")]") {
        return Err(format!("`{opening}` 블록이 tracking-policy 뒤에 있지 않다: {attributes:?}"));
    }
    let text = lines[start..=end].join(" ").split_whitespace().collect::<Vec<_>>().join(" ");
    Ok((start + 1, end + 1, text))
}

/// `src` 아래의 모든 `.rs` — (`/` 로 적은 상대 경로, 글).
fn rust_sources(src: &Path) -> Vec<(String, String)> {
    let (mut found, mut stack) = (Vec::new(), vec![src.to_path_buf()]);
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                let relative: Vec<String> = path.strip_prefix(src).unwrap().components().map(|part| part.as_os_str().to_string_lossy().into_owned()).collect();
                found.push((relative.join("/"), std::fs::read_to_string(&path).unwrap()));
            }
        }
    }
    found.sort();
    found
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
        "opening_world",
        "select_opening_world",
        "OpeningWorld",
        "OpeningTargetProblem",
        "ParentProblem",
        "WorldSource",
        "EntryCheck",
        "EntryVerdict",
        "RootOutOfScope",
        "ParentExit",
        "NotInOwnerChain",
        "OPENING_WORLD_DOES_NOT_SHOW",
        "experiment_exit",
        "select_experiment_exit",
        "ExperimentExit",
        "ExperimentExitProblem",
        "ExperimentTargetProblem",
        "InterviewOutOfScope",
        "EXPERIMENT_EXIT_DOES_NOT_SHOW",
        "owned_cycle",
        "exit_lineage",
        "check_experiment_exit_lineage",
        "ExitLineageCheck",
        "ExitLineageProblem",
        "ExitLineageVerdict",
        "ExpectedSource",
        "OpenExperimentOutOfScope",
        "StoredExitMissing",
        "StoredExitNotInRegistry",
        "LastOutcomeUnreadable",
        "LineageVerifyLacksSnapshot",
        "ExpectedNotInRegistry",
        "EXIT_LINEAGE_DOES_NOT_SHOW",
        "verify_world",
        "select_verify_world",
        "VerifyWorld",
        "VerifyWorldProblem",
        "StepProblem",
        "NotVerifyOutOfScope",
        "OpenVerifyOutOfScope",
        "ClosedVerifyLacksSnapshot",
        "SnapshotNotInRegistry",
        "VERIFY_WORLD_DOES_NOT_SHOW",
        "confirmed_world",
        "read_scope_change",
        "ScopeChangeReading",
        "RecordInputs",
        "EventWorldFacts",
        "ScopeMarks",
        "MarkSeen",
        "ConfirmedWorld",
        "PolicyStep",
        "EventStanding",
        "ReadingProblem",
        "ScopeLimit",
        "WorldFactsMismatch",
        "StartProblem",
        "ReviewProblem",
        "SecondBasis",
        "BasisAt",
        "Unjudged",
        "AppliedPolicy",
        "CONFIRMED_WORLD_DOES_NOT_SHOW",
        "baseline_retry",
        "RetryAttempt",
        "run_retry",
        "AfterFailure",
        "BaselineState",
        "BaselineCommit",
        "AttemptBinding",
        "AttemptTicket",
        "RetryOutcome",
        "RetryReport",
        "RetryRequest",
        "RetryStep",
        "RetryOutOfOrder",
        "RetryClosed",
        "RetryRun",
        "ScopeDelta",
        "PathDifference",
        "DifferenceKind",
        "NotStarted",
        "BASELINE_RETRY_DOES_NOT_SHOW",
        "OwnedRecords",
        "SaveAnswer",
        "FailureRecord",
        "FailedAttempt",
        "FailureCause",
        "UnknownWhy",
        "SeenAt",
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
    // 허용하는 것은 `src/artifact/mod.rs` 의 모듈 선언 한 줄과, `tracking-policy` 뒤의 4번 연결 파일·재수출 블록, 그리고 아래에서
    // 정확히 고정하는 5번의 연결뿐이다.
    let gated_files = ["artifact/tracking_state.rs", "start_prep/"];
    let mod_rs = std::fs::read_to_string(src.join("artifact").join("mod.rs")).unwrap();
    let block_start = mod_rs.lines().position(|l| l.starts_with("pub(crate) mod start_policy {")).expect("재수출 블록") + 1;
    let block_end = block_start + mod_rs.lines().skip(block_start).position(|l| l == "}").expect("블록 끝");
    assert!(mod_rs.lines().nth(block_start - 2).unwrap().contains("#[cfg(feature = \"tracking-policy\")]"), "재수출 블록이 기능 뒤에 있다");
    let state = std::fs::read_to_string(src.join("artifact").join("tracking_state.rs")).unwrap();
    assert!(state.contains("#![cfg(feature = \"tracking-policy\")]"), "형식 10/11 기록 모듈은 기능 뒤에 있다");
    let lib = std::fs::read_to_string(src.join("lib.rs")).unwrap();
    assert!(lib.contains("#[cfg(all(feature = \"tracking-policy\", unix))]\nmod start_prep;"), "시작 준비 모듈은 기능 뒤에 있다");

    // ── 구현 순서 5번(같은 Chain 범위 변경, 2026-11-24 승인)의 연결 — 정확히 승인된 것만 ─────────────────────────────
    // (1) 재수출 블록 `artifact::tracking`: 바깥 `tracking-policy` 속성 뒤에 있고, 블록 전체가 고정한 글과 같다. 재시도 재수출은 그
    //     안에서 다시 `#[cfg(unix)]` 뒤에 있다. 허용은 그 블록의 줄 구간뿐이다 — 파일 전체가 아니다.
    let (tracking_open, tracking_close, tracking_block) = gated_reexport_block(&mod_rs, "pub(crate) mod tracking {").expect("범위 변경 재수출 블록");
    assert_eq!(tracking_block, TRACKING_REEXPORT_BLOCK, "범위 변경 재수출 블록이 승인된 이름 집합과 다르다");
    assert!(tracking_block.contains("#[cfg(unix)] pub(crate) use super::tracking_record::{ AfterFailure,"), "재시도 재수출은 unix 뒤에 있다");
    assert_eq!(tracking_block.matches("#[cfg(unix)]").count(), 1);
    // 범위 변경 모듈이 닿을 수 있는 이름 — 두 재수출 블록(`tracking`, 4번의 `start_policy`)이 내보내는 것뿐이다.
    let start_policy_block = mod_rs.lines().skip(block_start - 1).take(block_end - block_start + 1).collect::<Vec<_>>().join(" ");
    let exported: Vec<&str> = identifiers(&tracking_block).into_iter().chain(identifiers(&start_policy_block)).collect();

    // (2) `src/scope_change/` 의 세 파일: 파일을 통째로 예외로 두지 않는다. 파일마다 **실제로 참조하는 금지 이름의 정확한 집합**과,
    //     금지 이름을 속에 품었을 뿐인 다른 낱말의 정확한 집합을 고정한다. 하나가 늘어도 줄어도 실패한다. 줄 번호에 기대지 않는다.
    //     그 폴더의 다른 파일은 여기 없으므로 아래의 일반 규칙이 그대로 거절한다.
    let scope_files: [(&str, &[&str], &[&str]); 3] = [
        (
            "scope_change/mod.rs",
            &[
                // 기준점 보관 한 시도의 실행과 판정(§5.3·§6.3) — 재시도 바탕을 그대로 부른다.
                "RetryAttempt", "RetryRequest", "RetryOutcome", "NotStarted", "AfterFailure", "FailureRecord", "FailureCause", "FailedAttempt",
                // 그 바탕이 요구하는 실제 저장 경계(`state.yaml`)의 구현.
                "BaselineState", "BaselineCommit", "SaveAnswer", "OwnedRecords", "RecordInputs",
                // 저장된 기록의 읽기 — 사건 종류, 미완 복원 표시, 정책 파일 상태의 판정.
                "EventKind", "MarkSeen", "classify_policy_file",
                // 새 정책의 원문 보관과 동기화 기준(§5.2) — 4번의 `start_policy` 재수출을 통해.
                "NewPolicyBase", "base_for_new_policy", "store_policy_source",
            ],
            // 원장의 사건 이름과 조회 결과의 칸 이름 — 금지 이름을 속에 품었을 뿐이다.
            &["RetryRequested", "last_confirmed_world"],
        ),
        // 제안·응답 원장: 그 모듈을 참조하지 않는다. `RetryRequested` 는 이 파일이 정의하는 사건 이름이다.
        ("scope_change/ledger.rs", &[], &["RetryRequested"]),
        // 단위 시험(`#[cfg(test)]`): 조회 결과의 칸 `last_confirmed_world` 를 읽을 뿐이다.
        ("scope_change/tests.rs", &[], &["last_confirmed_world"]),
    ];
    let set = |words: &[&str]| words.iter().map(|word| word.to_string()).collect::<std::collections::BTreeSet<String>>();
    for (file, referenced, inside) in scope_files {
        let text = std::fs::read_to_string(src.join(file)).unwrap();
        let found = name_uses(&text, &names);
        assert_eq!(found.referenced, set(referenced), "{file}: 승인된 참조 집합과 다르다");
        assert_eq!(found.inside_other_names, set(inside), "{file}: 금지 이름을 품은 낱말이 예상과 다르다");
        for name in referenced {
            assert!(exported.contains(name), "{file}: {name} 은 재수출 블록이 내보내는 이름이 아니다");
        }
    }
    // (3) 그 연결은 `tracking-policy` + Unix 에서만 컴파일된다. 모듈 선언, 그 자식, 그것을 부르는 제품 줄 모두.
    assert!(lib.contains(&format!("{UNIX_TRACKING_GATE}\nmod scope_change;")), "범위 변경 모듈은 기능과 Unix 뒤에 있다");
    let sources = rust_sources(&src);
    assert_eq!(sources.iter().map(|(_, text)| text.lines().filter(|line| line.trim() == "mod scope_change;").count()).sum::<usize>(), 1);
    assert!(!sources.iter().any(|(_, text)| text.lines().any(|line| line.contains("#[path") && line.contains("scope_change"))), "다른 자리에서 끌어오지 않는다");
    let scope_mod = std::fs::read_to_string(src.join("scope_change").join("mod.rs")).unwrap();
    assert!(scope_mod.contains("\nmod ledger;\n"), "원장은 그 모듈의 자식이다");
    assert!(scope_mod.contains("#[cfg(all(test, not(feature = \"exploration-flow\")))]\nmod tests;"), "단위 시험은 시험 빌드의 자식이다");
    let callers: Vec<(&str, usize, Vec<String>)> = sources.iter()
        .filter(|(file, _)| !file.starts_with("scope_change/") && file != "mcp/scope.rs" && file != "mcp/scope_tests.rs")
        .map(|(file, text)| { let (uses, ungated) = ungated_uses(text, "scope_change::"); (file.as_str(), uses, ungated) })
        .filter(|(_, uses, _)| *uses > 0)
        .collect();
    assert_eq!(callers.iter().map(|(file, _, _)| *file).collect::<Vec<_>>(), ["mcp/monitor.rs", "session.rs", "store.rs"], "범위 변경 모듈을 부르는 제품 파일");
    assert!(callers.iter().all(|(_, _, ungated)| ungated.is_empty()), "문 밖에서 범위 변경 모듈을 부른다:\n{callers:#?}");
    // MCP 의 범위 도구 모듈과 그 등록·호출 경로도 같은 문 뒤에 있다(`mcp/scope.rs` 는 그 모듈 자신이다).
    let mcp = std::fs::read_to_string(src.join("mcp.rs")).unwrap();
    assert!(mcp.contains(&format!("{UNIX_TRACKING_GATE}\nmod scope;")), "MCP 범위 도구 모듈은 기능과 Unix 뒤에 있다");
    let routes: Vec<(&str, usize, Vec<String>)> = sources.iter()
        .filter(|(file, _)| (file == "mcp.rs" || file.starts_with("mcp/")) && file != "mcp/scope.rs" && file != "mcp/scope_tests.rs")
        .map(|(file, text)| { let (uses, ungated) = ungated_uses(text, "scope::"); (file.as_str(), uses, ungated) })
        .filter(|(_, uses, _)| *uses > 0)
        .collect();
    assert_eq!(routes.iter().map(|(file, _, _)| *file).collect::<Vec<_>>(), ["mcp.rs", "mcp/surface.rs"], "MCP 범위 도구를 등록·호출하는 파일");
    assert!(routes.iter().all(|(_, _, ungated)| ungated.is_empty()), "문 밖에서 MCP 범위 도구를 부른다:\n{routes:#?}");

    let allowed = |line: &str| {
        let line = line.replace('\\', "/");
        let numbered = |file: &str| line.strip_prefix(file).and_then(|rest| rest.split(':').next()).and_then(|n| n.parse::<usize>().ok());
        line.ends_with(": mod tracking_record;")
            || gated_files.iter().any(|file| line.starts_with(file))
            || numbered("artifact/mod.rs:").is_some_and(|n| n > block_start && n <= block_end)
            // 5번: 재수출 블록의 줄 구간, 그리고 위에서 참조 집합을 정확히 고정한 세 파일.
            || numbered("artifact/mod.rs:").is_some_and(|n| n > tracking_open && n < tracking_close)
            || scope_files.iter().any(|(file, _, _)| line.starts_with(&format!("{file}:")))
    };
    let outside: Vec<_> = users.iter().filter(|line| !allowed(line)).collect();
    assert!(outside.is_empty(), "새 모듈이 제품 소스에서 쓰였다:\n{outside:#?}");
    assert!(users.iter().any(|line| line.ends_with(": mod tracking_record;")), "{users:#?}");
}

/// 위 검사의 도우미들이 **정확히 승인된 연결만** 받아들이는가 — 작은 합성 소스 글로 본다. 제품 파일을 고치거나 실행하지 않는다.
#[test]
fn the_boundary_helpers_accept_the_exact_approved_connection_and_refuse_extra_references_and_missing_gates() {
    let names = ["RetryRequest", "run_retry", "confirmed_world", "tracking_record"];
    let set = |words: &[&str]| words.iter().map(|word| word.to_string()).collect::<std::collections::BTreeSet<String>>();
    // 승인된 참조 하나, 금지 이름을 품은 다른 낱말 둘, 주석의 언급 하나.
    let approved = "use crate::artifact::tracking::RetryRequest;\n// run_retry 를 부르지 않는다\nlet e = Event::RetryRequested;\nlet w = view.last_confirmed_world;\n";
    let found = name_uses(approved, &names);
    assert_eq!(found, NameUses { referenced: set(&["RetryRequest"]), inside_other_names: set(&["RetryRequested", "last_confirmed_world"]), comment_lines: 1 });
    // 예상 밖 참조 하나가 더해지면 참조 집합이 달라진다 — 직접 경로든, 꼬리 주석 속이든(꼬리 주석은 코드로 본다).
    for extra in ["let out = run_retry(event);\n", "use crate::artifact::tracking_record::Anything;\n", "let x = 1; // confirmed_world\n"] {
        let found = name_uses(&format!("{approved}{extra}"), &names);
        assert_ne!(found.referenced, set(&["RetryRequest"]), "{extra}");
    }
    // 기존 부분 문자열 검색이 잡는 코드 줄은 하나도 빠지지 않는다.
    for line in ["x.run_retry()", "RetryRequestedAgain", "a::confirmed_world::b", "mod tracking_record;"] {
        let found = name_uses(line, &names);
        assert!(!found.referenced.is_empty() || !found.inside_other_names.is_empty(), "{line}");
    }

    // 문: 같은 줄, 바로 앞 줄(주석·다른 속성은 건너뛴다), 문 뒤의 줄이 연 구간.
    let gate = UNIX_TRACKING_GATE;
    let gated = format!("{gate}\nmod scope;\nstruct S {{\n    {gate}\n    // 설명\n    #[allow(dead_code)]\n    grants: scope::Grants,\n    other: u8,\n}}\nfn f() {{\n    let s = S {{ {gate} grants: scope::Grants::default(), other: 0 }};\n    {gate}\n    if scope::NAMES.contains(&name) {{\n        return scope::call(name);\n    }}\n}}\n");
    assert_eq!(ungated_uses(&gated, "scope::"), (4, vec![]));
    // 문이 없거나, 구간이 닫힌 뒤이거나, 조건이 다른 속성(`unix` 없음)이면 문 밖이다. 다른 낱말 속의 `scope::` 는 세지 않는다.
    assert_eq!(ungated_uses("fn f() { scope::call(name) }\n", "scope::"), (1, vec!["fn f() { scope::call(name) }".to_string()]));
    assert_eq!(ungated_uses(&format!("{gate}\nif a {{\n    b();\n}}\nscope::call(name);\n"), "scope::").1, ["scope::call(name);"]);
    assert_eq!(ungated_uses("#[cfg(feature = \"tracking-policy\")]\nlet t = scope::catalog(t);\n", "scope::").1, ["let t = scope::catalog(t);"]);
    assert_eq!(ungated_uses(&format!("{gate}\nlet a = 1;\nlet t = scope::catalog(t);\n"), "scope::").1, ["let t = scope::catalog(t);"]);
    assert_eq!(ungated_uses("let t = telescope::look();\n", "scope::"), (0, vec![]));

    // 재수출 블록: 바깥 속성이 있어야 블록이고, 블록의 글은 이름 하나·속성 하나의 차이도 드러낸다.
    let block = "#[cfg(feature = \"tracking-policy\")]\n#[allow(unused_imports)]\npub(crate) mod tracking {\n    pub(crate) use super::tracking_record::{\n        A, B,\n    };\n    #[cfg(unix)]\n    pub(crate) use super::tracking_record::{C};\n}\n";
    let expected = "pub(crate) mod tracking { pub(crate) use super::tracking_record::{ A, B, }; #[cfg(unix)] pub(crate) use super::tracking_record::{C}; }";
    assert_eq!(gated_reexport_block(block, "pub(crate) mod tracking {"), Ok((3, 9, expected.to_string())));
    assert_ne!(gated_reexport_block(&block.replace("A, B,", "A, B, Extra,"), "pub(crate) mod tracking {").unwrap().2, expected);
    assert_ne!(gated_reexport_block(&block.replace("    #[cfg(unix)]\n", ""), "pub(crate) mod tracking {").unwrap().2, expected);
    assert!(gated_reexport_block(&block.replace("#[cfg(feature = \"tracking-policy\")]\n", ""), "pub(crate) mod tracking {").is_err(), "바깥 문이 없다");
    assert!(gated_reexport_block(&block.replace("#[cfg(feature = \"tracking-policy\")]", "#[cfg(feature = \"other\")]"), "pub(crate) mod tracking {").is_err());
    assert!(gated_reexport_block("mod other {\n}\n", "pub(crate) mod tracking {").is_err(), "블록이 없다");
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

/// S(N) 선택·검증 ① 의 원문에서 주석과 문자열 목록 줄을 뺀 **코드 줄**이 파일·상태·창고·3n/3o·P(N)·B(T) 의 문을 쓰지 않고, 같은 호출의
/// 입력으로 3j 를 한 번 부르며, 다른 입력에서 얻은 3j 성공 값이나 호출자가 고른 부모·기대 Entry 를 받는 함수가 없는가.
#[test]
fn the_opening_world_selection_touches_no_file_state_or_store_and_takes_no_shortcut() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("artifact").join("tracking_record").join("opening_world.rs");
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
        "select_boundary_world",
        "BoundarySource",
        "BoundaryPolicy",
        "revisit_from",
        "put_blob",
    ];
    let used: Vec<String> = code
        .iter()
        .flat_map(|line| forbidden.iter().filter(|name| line.contains(*name)).map(move |name| format!("{name}: {line}")))
        .collect();
    assert!(used.is_empty(), "S(N) 선택이 파일·상태·창고·구간·P(N)·B(T)·revisit_from 의 문을 쓴다:\n{used:#?}");
    let calls: Vec<&&str> = code.iter().filter(|line| line.contains("check_event_inputs(")).collect();
    assert_eq!(calls, [&"let inputs = check_event_inputs(ledger, links, facts, chains, registry);"], "같은 호출의 입력으로 3j 를 한 번 불러야 한다");
    // 함수의 인자 자리(한 줄 서명의 괄호 안, 또는 여러 줄 서명에서 `fn 이름(` 줄과 `)` 줄 사이)에 성공 값·부모·기대 세계가 있으면 안 된다.
    let mut parameters: Vec<String> = Vec::new();
    let mut in_signature = false;
    for line in &code {
        match (in_signature, line.contains("fn ") && line.contains('('), line.find('('), line.find(')')) {
            (false, true, Some(open), Some(close)) if open < close => parameters.push(line[open..close].to_string()),
            (false, true, _, None) => in_signature = true,
            (true, ..) if line.starts_with(')') => in_signature = false,
            (true, ..) => parameters.push(line.to_string()),
            _ => {}
        }
    }
    assert!(parameters.iter().any(|taken| taken.contains("target: CycleRef")), "서명을 읽지 못했다: {parameters:#?}");
    let shortcuts: Vec<&String> = parameters
        .iter()
        .filter(|taken| ["Agree", "parent", "expected", "entry", "Verdict", "WorldSource", "Located"].iter().any(|name| taken.contains(name)))
        .collect();
    assert!(shortcuts.is_empty(), "검사를 건너뛰는 인자가 있다: {shortcuts:#?}");
    let signature: Vec<&&str> = code.iter().filter(|line| line.starts_with("pub(crate) fn select_opening_world(")).collect();
    assert_eq!(signature.len(), 1, "선택의 문은 하나다");
}

/// Experiment Exit 유도의 원문에서 주석과 문자열 목록 줄을 뺀 **코드 줄**이 파일·상태·창고·3n/3o·B(T)·S(N) 의 문, 원장의 판 목록·원문
/// 지문·사건의 기준점·계보 칸을 쓰지 않고, 같은 호출의 입력으로 P(N) 선택을 한 번 부르며 3j 를 따로 부르지 않는가. 다른 입력에서 얻은
/// P(N)·3j 성공 값이나 호출자가 고른 세계·정책을 받는 함수가 없는가.
#[test]
fn the_experiment_exit_derivation_reuses_one_opening_policy_and_touches_no_file_state_or_store() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("artifact").join("tracking_record").join("experiment_exit.rs");
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
        "select_boundary_world",
        "BoundarySource",
        "BoundaryPolicy",
        "select_opening_world",
        "WorldSource",
        "check_event_inputs",
        "check_policy_links",
        "check_cycle_refs",
        "policies()",
        "next_policy_id",
        "rules()",
        "baseline()",
        "issuer()",
        "events()",
        "entry_snapshot",
        "parent",
        "revisit_from",
        "put_blob",
    ];
    let used: Vec<String> = code
        .iter()
        .flat_map(|line| forbidden.iter().filter(|name| line.contains(*name)).map(move |name| format!("{name}: {line}")))
        .collect();
    assert!(used.is_empty(), "Experiment Exit 유도가 파일·상태·창고·구간·B(T)·S(N)·3j·원장 판·기준점·계보의 문을 쓴다:\n{used:#?}");
    let calls: Vec<&&str> = code.iter().filter(|line| line.contains("select_opening_policy(")).collect();
    assert_eq!(
        calls,
        [&"let opening = select_opening_policy(target, ledger, links, facts, chains, registry);"],
        "같은 호출의 입력으로 P(N) 선택을 한 번 불러야 한다"
    );
    // 함수의 인자 자리(한 줄 서명의 괄호 안, 또는 여러 줄 서명에서 `fn 이름(` 줄과 `)` 줄 사이)에 성공 값·세계·정책이 있으면 안 된다.
    let mut parameters: Vec<String> = Vec::new();
    let mut in_signature = false;
    for line in &code {
        match (in_signature, line.contains("fn ") && line.contains('('), line.find('('), line.find(')')) {
            (false, true, Some(open), Some(close)) if open < close => parameters.push(line[open..close].to_string()),
            (false, true, _, None) => in_signature = true,
            (true, ..) if line.starts_with(')') => in_signature = false,
            (true, ..) => parameters.push(line.to_string()),
            _ => {}
        }
    }
    assert!(parameters.iter().any(|taken| taken.contains("target: CycleRef")), "서명을 읽지 못했다: {parameters:#?}");
    let shortcuts: Vec<&String> = parameters
        .iter()
        .filter(|taken| {
            ["Agree", "Opening", "Selection", "PolicyName", "EventName", "SnapshotRef", "world", "exit", "Located"]
                .iter()
                .any(|name| taken.contains(name))
        })
        .collect();
    assert!(shortcuts.is_empty(), "검사를 건너뛰는 인자가 있다: {shortcuts:#?}");
    let signature: Vec<&&str> = code.iter().filter(|line| line.starts_with("pub(crate) fn select_experiment_exit(")).collect();
    assert_eq!(signature.len(), 1, "유도의 문은 하나다");
}

/// Exit 계보 검사의 원문에서 주석과 문자열 목록 줄을 뺀 **코드 줄**이 파일·상태·창고·정책 원장·사건 입력의 문, Cycle 을 닫는 규칙과
/// 현재 세계 유도(`exit_would_be`·`world_snapshot`), 이름·순서·출처 칸(`revisit_from`·목록 끝·최댓값), P(N)·B(T)·S(N)·Experiment Exit
/// 선택을 쓰지 않고, 소유 조회와 계보 조회를 한 번씩만 쓰는가. 호출자가 고른 세계·기대값·사건을 받는 함수가 없는가.
#[test]
fn the_exit_lineage_check_reads_one_lineage_and_touches_no_file_state_store_or_event() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("artifact").join("tracking_record").join("exit_lineage.rs");
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
        "put_blob",
        "PolicyLedger",
        "EventPolicyLinks",
        "EventCycleFacts",
        "select_opening_policy",
        "select_experiment_exit",
        "select_boundary_world",
        "select_opening_world",
        "check_event_inputs",
        "exit_would_be",
        "world_snapshot",
        "verify_in_lineage",
        "revisit_from",
        "parent",
        "history()",
        "nodes()",
        ".last()",
        "max",
        "next_id",
        "sort",
    ];
    let used: Vec<String> = code
        .iter()
        .flat_map(|line| forbidden.iter().filter(|name| line.contains(*name)).map(move |name| format!("{name}: {line}")))
        .collect();
    assert!(used.is_empty(), "Exit 계보 검사가 파일·상태·창고·사건·닫기 규칙·이름 순서·다른 선택의 문을 쓴다:\n{used:#?}");
    let lineage: Vec<&&str> = code.iter().filter(|line| line.contains(".lineage(")).collect();
    assert_eq!(lineage.len(), 1, "계보는 기존 Walk::lineage 로 한 번 읽는다: {lineage:#?}");
    let owner: Vec<&&str> = code.iter().filter(|line| line.contains("owned_cycle(")).collect();
    assert_eq!(owner.len(), 1, "대상은 Experiment Exit 선택과 같은 소유 조회로 한 번 정한다: {owner:#?}");
    // 함수의 인자 자리(한 줄 서명의 괄호 안, 또는 여러 줄 서명에서 `fn 이름(` 줄과 `)` 줄 사이)에 세계·기대값·사건이 있으면 안 된다.
    // `pub(crate)` 의 괄호를 인자로 읽지 않도록 `fn ` 뒤부터 본다.
    let mut parameters: Vec<String> = Vec::new();
    let mut in_signature = false;
    for line in &code {
        let line = &line[line.find("fn ").unwrap_or(0)..];
        match (in_signature, line.starts_with("fn ") && line.contains('('), line.find('('), line.find(')')) {
            (false, true, Some(open), Some(close)) if open < close => parameters.push(line[open..close].to_string()),
            (false, true, _, None) => in_signature = true,
            (true, ..) if line.starts_with(')') => in_signature = false,
            (true, ..) => parameters.push(line.to_string()),
            _ => {}
        }
    }
    assert!(parameters.iter().any(|taken| taken.contains("target: CycleRef")), "서명을 읽지 못했다: {parameters:#?}");
    let shortcuts: Vec<&String> = parameters
        .iter()
        .filter(|taken| {
            ["SnapshotRef", "StepRef", "world", "expected", "exit", "Event", "Ledger", "Opening", "Boundary", "Selection"]
                .iter()
                .any(|name| taken.contains(name))
        })
        .collect();
    assert!(shortcuts.is_empty(), "검사를 건너뛰는 인자가 있다: {shortcuts:#?}");
    let signature: Vec<&&str> = code.iter().filter(|line| line.starts_with("pub(crate) fn check_experiment_exit_lineage(")).collect();
    assert_eq!(signature.len(), 1, "검사의 문은 하나다");
}

/// Verify 세계 유도의 원문에서 주석과 문자열 목록 줄을 뺀 **코드 줄**이 파일·상태·창고·3j·구간의 문, 계보 선택과 현재 세계·Exit 유도
/// (`lineage`·`world_snapshot`·`exit_would_be`·Exit 칸), 이름·순서·출처 칸(`parent`·`revisit_from`·목록 끝·최댓값), B(T)·S(N)·Experiment Exit·
/// Exit 계보 선택을 쓰지 않고, 같은 호출의 입력으로 P(N) 선택·소유 조회·등록부 조회를 한 번씩만 쓰는가. 호출자가 고른 세계·정책·성공 값을
/// 받는 함수가 없는가.
#[test]
fn the_verify_world_derivation_reuses_one_opening_policy_and_reads_only_the_named_verify() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("artifact").join("tracking_record").join("verify_world.rs");
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
        "put_blob",
        "LedgerStart",
        "ConfirmedStart",
        "AppliedSpan",
        "BoardPolicies",
        "select_ledger_span",
        "classify_path",
        "select_boundary_world",
        "BoundarySource",
        "BoundaryPolicy",
        "select_opening_world",
        "WorldSource",
        "select_experiment_exit",
        "check_experiment_exit_lineage",
        "check_event_inputs",
        "check_policy_links",
        "check_cycle_refs",
        "policies()",
        "next_policy_id",
        "rules()",
        "baseline()",
        "issuer()",
        "events()",
        "entry_snapshot",
        "exit_snapshot",
        "world_snapshot",
        "exit_would_be",
        "verify_in_lineage",
        "lineage(",
        "current(",
        "history()",
        "parent",
        "revisit_from",
        ".last()",
        "max",
        "sort",
    ];
    let used: Vec<String> = code
        .iter()
        .flat_map(|line| forbidden.iter().filter(|name| line.contains(*name)).map(move |name| format!("{name}: {line}")))
        .collect();
    assert!(used.is_empty(), "Verify 세계 유도가 파일·상태·창고·3j·구간·계보 선택·현재 세계·Exit·다른 선택의 문을 쓴다:\n{used:#?}");
    let calls: Vec<&&str> = code.iter().filter(|line| line.contains("select_opening_policy(")).collect();
    assert_eq!(
        calls,
        [&"let opening = select_opening_policy(target.cycle(), ledger, links, facts, chains, registry);"],
        "같은 호출의 입력으로 대상 Step 의 Cycle 에 대해 P(N) 선택을 한 번 불러야 한다"
    );
    let owner: Vec<&&str> = code.iter().filter(|line| line.contains("owned_cycle(")).collect();
    assert_eq!(owner.len(), 1, "대상 Cycle 은 Experiment Exit 선택과 같은 소유 조회로 한 번 정한다: {owner:#?}");
    let lookups: Vec<&&str> = code.iter().filter(|line| line.contains(".resolve(")).collect();
    assert_eq!(lookups.len(), 1, "등록부 조회는 지정한 Verify 의 세계 하나다: {lookups:#?}");
    // 함수의 인자 자리(한 줄 서명의 괄호 안, 또는 여러 줄 서명에서 `fn 이름(` 줄과 `)` 줄 사이)에 성공 값·세계·정책이 있으면 안 된다.
    // `pub(crate)` 의 괄호를 인자로 읽지 않도록 `fn ` 뒤부터 본다.
    let mut parameters: Vec<String> = Vec::new();
    let mut in_signature = false;
    for line in &code {
        let line = &line[line.find("fn ").unwrap_or(0)..];
        match (in_signature, line.starts_with("fn ") && line.contains('('), line.find('('), line.find(')')) {
            (false, true, Some(open), Some(close)) if open < close => parameters.push(line[open..close].to_string()),
            (false, true, _, None) => in_signature = true,
            (true, ..) if line.starts_with(')') => in_signature = false,
            (true, ..) => parameters.push(line.to_string()),
            _ => {}
        }
    }
    assert!(parameters.iter().any(|taken| taken.contains("target: StepRef")), "서명을 읽지 못했다: {parameters:#?}");
    let shortcuts: Vec<&String> = parameters
        .iter()
        .filter(|taken| {
            ["Agree", "Opening", "Selection", "PolicyName", "EventName", "SnapshotRef", "world", "exit", "Located", "Boundary"]
                .iter()
                .any(|name| taken.contains(name))
        })
        .collect();
    assert!(shortcuts.is_empty(), "검사를 건너뛰는 인자가 있다: {shortcuts:#?}");
    let signature: Vec<&&str> = code.iter().filter(|line| line.starts_with("pub(crate) fn select_verify_world(")).collect();
    assert_eq!(signature.len(), 1, "유도의 문은 하나다");
}

/// 원문에서 주석과 문자열 목록 줄을 뺀 **코드 줄**.
fn code_lines(text: &str) -> Vec<&str> {
    text.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with("//") && !line.starts_with('"')).collect()
}

/// 범위 변경 사건의 세 값 읽기의 코드 줄이 폴더 관측·객체 창고·상태 저장·재시도의 문을 쓰지 않고, 같은 호출의 입력으로 기존 검사·선택을
/// 부르며, 호출자가 고른 확정 정책·경계·성공 값을 받는 문이 없는가.
#[test]
fn the_scope_change_reading_has_no_channel_to_observation_store_or_state_and_takes_no_chosen_policy() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("artifact").join("tracking_record").join("confirmed_world.rs");
    let text = std::fs::read_to_string(&path).unwrap();
    let code = code_lines(&text);
    let forbidden = [
        "std::fs",
        "std::io",
        "std::env",
        "std::path",
        "File",
        "OpenOptions",
        "read_to_string",
        "ObjectStore",
        "put_blob",
        "put_manifest",
        "read_manifest",
        "read_blob",
        "observe_scoped",
        "capture_scoped",
        "evaluate_scope",
        "ScopedWorld",
        "load_policy",
        "ApprovedPolicy",
        "Project",
        "state.yaml",
        "intern(",
        "baseline_retry",
        "BaselineState",
        "RetryAttempt",
        "run_retry",
        "save_baseline",
        "record_failure",
    ];
    let used: Vec<String> = code
        .iter()
        .flat_map(|line| forbidden.iter().filter(|name| line.contains(*name)).map(move |name| format!("{name}: {line}")))
        .collect();
    assert!(used.is_empty(), "읽기가 관측·창고·상태·재시도의 문을 쓴다:\n{used:#?}");

    // 기존 검사·선택을 같은 호출의 입력으로 부른다.
    for call in [
        "let agreed = check_event_inputs(ledger, links, facts, chains, registry);",
        "classify_baseline_positions(&baselines, registry)",
        "select_boundary_world(target, i.ledger, i.links, i.facts, i.chains, i.registry)",
        "select_opening_world(target, i.ledger, i.links, i.facts, i.chains, i.registry)",
        "select_experiment_exit(target, i.ledger, i.links, i.facts, i.chains, i.registry)",
        "check_experiment_exit_lineage(target, i.chains, i.registry)",
    ] {
        assert!(code.iter().any(|line| line.contains(call)), "기존 검사·선택을 같은 호출의 입력으로 불러야 한다: {call}");
    }

    // 역사 경계(승인·보관)의 근거도 지금 경계와 같은 유도를 지난다 — B(T) 를 고르는 자리는 그 유도 하나뿐이다.
    let picks: Vec<&&str> = code.iter().filter(|line| line.contains("select_boundary_world(")).collect();
    assert_eq!(picks.len(), 1, "B(T) 선택은 한 유도 안에서만 불러야 한다: {picks:#?}");
    for line in ["let basis = Derivation { inputs: &earlier, chain, start }", ".boundary(boundary)"] {
        assert!(code.contains(&line), "역사 경계의 근거는 앞의 사건만 남긴 입력으로 같은 유도를 불러야 한다: {line}");
    }

    // 읽기를 만드는 문은 하나이고, 호출자가 건네는 것은 대상 사건 이름과 한 입력 묶음뿐이다.
    let doors: Vec<&&str> = code.iter().filter(|line| line.contains("-> Result<ScopeChangeReading")).collect();
    assert_eq!(
        doors,
        [&"pub(crate) fn read_scope_change(event: EventName, inputs: &RecordInputs<'_>) -> Result<ScopeChangeReading, Vec<ReadingProblem>> {"],
        "읽기를 만드는 문이 하나여야 하고 확정 정책·경계·성공 값을 받지 않아야 한다"
    );
    // 함수의 인자 자리에 다른 호출의 성공 값(…Agree)이 있으면 안 된다.
    let parameters = |line: &str| match (line.contains("fn "), line.find('('), line.find(')')) {
        (true, Some(open), Some(close)) if open < close => line[open..close].to_string(),
        _ if line.contains(": &") && line.ends_with(',') => line.to_string(),
        _ => String::new(),
    };
    let takes_agreement: Vec<&&str> = code.iter().filter(|line| parameters(line).contains("Agree")).collect();
    assert!(takes_agreement.is_empty(), "다른 호출의 성공 값을 받는 문이 있다: {takes_agreement:#?}");
}

/// 명시 재시도의 원문이 `tracking-policy` + Unix 뒤에 있고, 기존 실제 관측·보관을 그 자리에서 부르며, 확정 정책을 읽기에서만 얻고,
/// 보고의 내용과 표를 모듈 밖에서 지어낼 수 없으며, 제품의 상태 저장·세션·도구에 닿지 않는가.
#[test]
fn the_baseline_retry_is_feature_gated_observes_for_real_and_reaches_no_product_state() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("artifact").join("tracking_record");
    let text = std::fs::read_to_string(dir.join("baseline_retry.rs")).unwrap();
    let code = code_lines(&text);
    assert!(code.contains(&"#![cfg(all(feature = \"tracking-policy\", unix))]"), "재시도 모듈은 기능 뒤에 있다");
    let module = std::fs::read_to_string(dir.join("mod.rs")).unwrap();
    assert!(
        module.contains("#[cfg(all(feature = \"tracking-policy\", unix))]\n#[allow(unused_imports)]\npub(crate) use baseline_retry::{"),
        "재시도 모듈의 재수출은 기능 뒤에 있다"
    );
    assert!(
        module.contains("#[cfg(all(test, feature = \"tracking-policy\", unix))]\nmod baseline_retry_tests;"),
        "재시도 시험은 같은 기능 뒤에서만 짓는다"
    );

    // 실제 관측·보관·manifest 저장을 이 모듈이 **이 시도의 대상으로** 그 자리에서 부른다 — 결과를 받아 통과시키기만 하는 문이 아니다.
    for call in [
        "observe_scoped(&targets.root, self.applied_policy()).map_err(Missed::Observe)",
        "capture_scoped(&targets.root, self.applied_policy(), &targets.store).map_err(Missed::Observe)",
        "targets.store.put_manifest(captured).map_err(|problem| Missed::Store(problem.to_string()))",
        "Phase::Saving { commit, .. } => ReportBody::Saved(self.state.save_baseline(commit)),",
        "Phase::Rereading { .. } | Phase::Checking { .. } => ReportBody::Reread(self.state.reread()),",
        "let delta = scope_delta(&span, &self.base, &world.manifest);",
    ] {
        assert!(code.contains(&call), "실제 관측 경로와 범위 변화 검사를 이 시도의 대상으로 불러야 한다: {call}");
    }
    // 시도를 만드는 문은 읽기를 다시 실행하고, 실행 전제를 확인하고, 확정 정책을 그 읽기에서만 얻고, 대상을 한 번 정한다.
    for line in [
        "let before = read_scope_change(event, &inputs).map_err(NotStarted::Reading)?;",
        "let unconfirmed = unconfirmed_prerequisites(&before);",
        "let confirmed_policy = before.confirmed().policy();",
        "let targets = Targets::of(root).map_err(|said| NotStarted::Target { said })?;",
        "let store = ObjectStore::at(&root.join(GIL_DIR));",
    ] {
        assert!(code.contains(&line), "시도는 읽기가 유도한 값과 한 번 정한 대상에서 시작해야 한다: {line}");
    }
    // 대상(루트·객체 창고·저장 경계)을 받는 문은 시도를 시작하는 두 문뿐이다. 걸음을 실행하는 문은 대상을 받지 않는다.
    assert!(code.contains(&"pub(crate) fn perform(&mut self) -> Result<RetryReport, RetryClosed> {"), "perform 은 대상을 받지 않는다");
    let count = |wanted: &str| code.iter().filter(|line| **line == wanted).count();
    assert_eq!(count("root: &Path,"), 2, "루트를 받는 문은 begin 과 run_retry 뿐이다");
    assert_eq!(count("state: &'a mut dyn BaselineState,"), 2, "저장 경계는 begin 이 받아 시도가 지닌다(인자와 칸)");
    assert_eq!(count("state: &mut dyn BaselineState,"), 1, "run_retry 가 받아 begin 에 건넨다");
    let takes_targets: Vec<&&str> = code
        .iter()
        .filter(|line| line.contains("fn ") && line.contains('(') && line.contains(')'))
        .filter(|line| line.contains("pub(crate)"))
        .filter(|line| line.contains("&Path") || line.contains("ObjectStore") || line.contains("BaselineState"))
        .collect();
    assert!(takes_targets.is_empty(), "한 줄 서명의 공개 문이 대상을 다시 받는다: {takes_targets:#?}");
    // 보고의 내용과 표는 모듈 밖에서 만들 수 없다 — 칸이 비공개이고 만드는 공개 문이 없다.
    for private in ["enum ReportBody {", "ticket: AttemptTicket,", "body: ReportBody,", "attempt: u64,", "step: u32,"] {
        assert!(code.contains(&private), "비공개 칸이어야 한다: {private}");
    }
    let report_makers: Vec<&&str> = code.iter().filter(|line| line.contains("RetryReport {") && !line.contains("struct") && !line.contains("impl")).collect();
    assert_eq!(report_makers, [&"Ok(RetryReport { ticket: self.ticket(), body })"], "보고를 만드는 자리는 perform 하나다");

    let forbidden = [
        "state.yaml",
        "ProjectSession",
        "crate::session",
        "crate::store",
        "crate::project",
        "crate::mcp",
        "crate::monitor",
        "crate::command",
        "start_prep",
        "tracking_state",
        "policy_file_write",
        "write_root_policy_file",
        "read_root_policy_file",
        "std::fs",
        "File::",
        "OpenOptions",
        "remove_file",
        "rename(",
    ];
    let used: Vec<String> = code
        .iter()
        .flat_map(|line| forbidden.iter().filter(|name| line.contains(*name)).map(move |name| format!("{name}: {line}")))
        .collect();
    assert!(used.is_empty(), "재시도가 제품 상태·세션·도구·작업 파일 쓰기의 문을 쓴다:\n{used:#?}");
}
