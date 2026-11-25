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
//!   이름 집합, `src/scope_change/` 세 파일과 그것을 부르는 제품 줄의 **승인된 자리의 열** — 품은 항목·줄 글·횟수·순서 — 과
//!   `tracking-policy` + Unix 문)뿐이며, 그 파일들이
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

/// 검출된 한 줄의 자리 — 그 줄을 품은 항목(파일 맨 앞 칸에서 시작하는 항목, 그 안의 메서드가 있으면 그것까지)과 공백을 정규화한 줄 글.
#[derive(Debug, Clone, PartialEq)]
struct Site {
    item: String,
    line: String,
}

/// 항목을 여는 줄의 이름 부분 — 가시성 표시를 떼고 첫 `(`·`{`·`;`·`=` 앞까지(예: `fn drive<'a>`, `impl BaselineState for StateAdapter<'_>`).
fn without_visibility(line: &str) -> &str {
    ["pub(crate) ", "pub(super) ", "pub "].iter().find_map(|visibility| line.strip_prefix(visibility)).unwrap_or(line)
}

fn item_key(line: &str) -> String {
    let line = without_visibility(line);
    let end = line.find(['(', '{', ';', '=']).unwrap_or(line.len());
    line[..end].split_whitespace().collect::<Vec<_>>().join(" ")
}

fn opens_fn(line: &str) -> bool {
    let line = without_visibility(line);
    ["fn ", "async ", "const ", "unsafe "].iter().any(|start| line.starts_with(start))
        && line.split('(').next().is_some_and(|head| head.split_whitespace().any(|word| word == "fn"))
}

/// `detect` 가 참인 줄들을 **나온 순서대로, 중복도 그대로** 자리와 함께 모은다. 범용 Rust 파서가 아니다: 이 저장소의 포맷(항목은 맨
/// 앞 칸에서, `impl` 안의 메서드는 네 칸 들여 시작한다)에 기대는 작은 줄 스캐너이며, 승인된 소스 구간을 가리키는 데만 쓴다.
fn sites(text: &str, detect: impl Fn(&str) -> bool) -> Vec<Site> {
    let (mut outer, mut inner): (String, Option<String>) = (String::new(), None);
    let mut found = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let may_open = !line.starts_with("//") && !line.starts_with('#') && !line.starts_with(['}', ')', ']']);
        if may_open && !raw.starts_with([' ', '\t']) {
            outer = item_key(line);
            inner = None;
        } else if may_open && raw.starts_with("    ") && !raw.starts_with("     ") && opens_fn(line) {
            inner = Some(item_key(line));
        }
        if detect(line) {
            let item = match &inner {
                Some(method) => format!("{outer} :: {method}"),
                None => outer.clone(),
            };
            found.push(Site { item, line: line.split_whitespace().collect::<Vec<_>>().join(" ") });
        }
    }
    found
}

/// 찾은 자리들이 승인된 자리들과 **정확히 같은 열**인가 — 같으면 빈 목록, 아니면 차이의 설명. 열로 견주므로 같은 줄의 중복, 같은 이름의
/// 다른 자리 사용, 다른 항목으로의 이동, 승인 자리의 누락이 모두 차이다(이름의 집합으로 견주지 않는다).
fn site_differences(found: &[Site], approved: &[(&str, &str)]) -> Vec<String> {
    let found: Vec<(&str, &str)> = found.iter().map(|site| (site.item.as_str(), site.line.as_str())).collect();
    if found == approved {
        return Vec::new();
    }
    let mut left = approved.to_vec();
    let mut differences = Vec::new();
    for site in &found {
        match left.iter().position(|one| one == site) {
            Some(at) => {
                left.remove(at);
            }
            None => differences.push(format!("승인되지 않은 자리: [{}] {}", site.0, site.1)),
        }
    }
    differences.extend(left.iter().map(|site| format!("승인된 자리가 없다: [{}] {}", site.0, site.1)));
    if differences.is_empty() {
        differences.push("자리들의 순서가 승인된 것과 다르다".to_string());
    }
    differences
}

/// 검출된 코드 줄에서 **낱말 전체가 금지 이름**인 것들(실제 참조)을 나온 순서대로, 중복도 그대로. 금지 이름을 속에 품었을 뿐인 다른
/// 낱말(`RetryRequested` 속의 `RetryRequest`)과 주석 줄은 참조가 아니다 — 그래도 그 줄들은 위의 자리 비교에서 빠지지 않는다.
fn references(found: &[Site], names: &[&str]) -> Vec<String> {
    found.iter().filter(|site| !site.line.starts_with("//"))
        .flat_map(|site| identifiers(&site.line).into_iter().filter(|word| names.contains(word)).map(str::to_string).collect::<Vec<_>>())
        .collect()
}

// ── 구현 순서 5번의 승인된 자리들(2026-11-24) ───────────────────────────────────────────────────────────────────────────────
// 각 항목은 (그 줄을 품은 항목, 공백을 정규화한 줄 글)이며 소스에 나오는 순서 그대로다. 금지 이름 검색이 잡는 **모든** 줄이 여기 있다 —
// 실제 참조, 금지 이름을 속에 품은 낱말이 있는 줄, 주석 줄. 한 줄이 늘거나 줄거나 옮겨 가면 이 목록과 달라진다.

/// `src/scope_change/mod.rs` — 재수출 `artifact::tracking`·`start_policy` 를 통해 기준점 보관의 한 시도를 실행·판정하고(`drive`·
/// `attempt`·`cause_word`), 그 바탕이 요구하는 실제 저장 경계를 구현하며(`impl BaselineState for StateAdapter`), 저장된 기록을 읽고
/// (`owned`·`restore_mark`·`policy_file_word`), 새 정책의 원문과 동기화 기준을 다룬다(`apply`·`sync_policy_file`).
const SCOPE_MOD_SITES: [(&str, &str); 62] = [
    (r#""#, r#"//! 관측·범위 변화 검사·객체 확정·저장 결과의 재확인은 `baseline_retry` 의 `RetryAttempt` 가 한다(그 모듈의 `run_retry` 와 같은 순서로"#),
    (r#""#, r#"//! ([`AttemptBinding::holds_in`])가 그대로일 때만 한 저장 단위(세계 이름 등록·`baseline` 경계/발급기/참조·`archived` 시도)를 한 번의"#),
    (r#""#, r#"//! 승인 저장 때의 `sync_base` 는 `base_for_new_policy` 그대로다. 세 내용이 모두 다르면 저장 1 을 보류하고(2026-11-22 결정), 계약이"#),
    (r#"use crate::artifact::tracking::"#, r#"AfterFailure, BaselineCommit, BaselineState, EventKind, EventName, FailedAttempt, FailureCause, FailureRecord, HaltAt,"#),
    (r#"use crate::artifact::tracking::"#, r#"MarkSeen, OwnedRecords, PolicyName, RecordStanding, RetryAttempt, RetryOutcome, RetryRequest, SaveAnswer, StandingAt,"#),
    (r#"fn restore_mark"#, r#"fn restore_mark(gil: &Path) -> MarkSeen {"#),
    (r#"fn restore_mark"#, r#"Ok(None) => MarkSeen::Absent,"#),
    (r#"fn restore_mark"#, r#"Ok(Some(_)) => MarkSeen::Present,"#),
    (r#"fn restore_mark"#, r#"Err(_) => MarkSeen::NotProvided,"#),
    (r#"fn owned"#, r#"fn owned(project: &Project, restore: MarkSeen) -> Result<OwnedRecords, String> {"#),
    (r#"fn owned"#, r#"Ok(OwnedRecords {"#),
    (r#"fn standing_view"#, r#""last_confirmed_world": standing.confirmed.as_ref().map(|confirmed| confirmed.world.to_string()),"#),
    (r#"fn policy_file_word"#, r#"use crate::artifact::tracking::{PolicyFileReading, classify_policy_file};"#),
    (r#"fn policy_file_word"#, r#"match classify_policy_file(tracking.applied().rules(), tracking.sync_base(), &reading(&disk_now(root))) {"#),
    (r#"fn apply"#, r#"if restore_mark(gil) != MarkSeen::Absent {"#),
    (r#"fn apply"#, r#"let chosen = match sp::base_for_new_policy(&rules, &base, &disk) {"#),
    (r#"fn apply"#, r#"Ok(sp::NewPolicyBase::Chosen(chosen)) => chosen,"#),
    (r#"fn apply"#, r#"Ok(sp::NewPolicyBase::Held(three)) => {"#),
    (r#"fn apply"#, r#"sp::store_policy_source(&store, engine, &rules, &bytes)"#),
    (r#"fn apply"#, r#"kind: EventKind::ScopeChange,"#),
    (r#"enum BaseStep"#, r#"NotStarted,"#),
    (r#"impl BaseStep :: fn word"#, r#"BaseStep::NotStarted => "not_started","#),
    (r#"impl BaseStep :: fn returned"#, r#"BaseStep::NotStarted => Value::Null,"#),
    (r#"impl FileSync :: fn untouched"#, r#"FileSync { word, start: BaseStep::NotStarted, file: "not_started", base: BaseStep::NotStarted, stored_base, halted_before: None }"#),
    (r#"fn sync_policy_file"#, r#"word, start, file: "not_started", base: BaseStep::NotStarted, stored_base, halted_before };"#),
    (r#"fn sync_policy_file"#, r#"return FileSync { word: "written_halted_base_unconfirmed", start, file: "replaced", base: BaseStep::NotStarted,"#),
    (r#"fn sync_policy_file"#, r#"return FileSync { word: "update_failed_halted_base_unconfirmed", start, file: "not_replaced", base: BaseStep::NotStarted,"#),
    (r#"fn sync_policy_file"#, r#"_ => FileSync { word: "unsettled", start, file: "unknown", base: BaseStep::NotStarted, stored_base, halted_before: None },"#),
    (r#"fn sync_policy_file"#, r#"/// `baseline_retry` 의 내부 저장 경계를 실제 `state.yaml` 에 잇는다. 세 문 모두 프로젝트 잠금 안이다."#),
    (r#"impl StateAdapter<'_> :: fn fresh"#, r#"fn fresh(&self) -> Result<(Project, OwnedRecords), String> {"#),
    (r#"impl BaselineState for StateAdapter<'_>"#, r#"impl BaselineState for StateAdapter<'_> {"#),
    (r#"impl BaselineState for StateAdapter<'_> :: fn save_baseline"#, r#"fn save_baseline(&mut self, commit: &BaselineCommit) -> SaveAnswer {"#),
    (r#"impl BaselineState for StateAdapter<'_> :: fn save_baseline"#, r#"let not = |said: String| SaveAnswer::NotStored { said };"#),
    (r#"impl BaselineState for StateAdapter<'_> :: fn save_baseline"#, r#"Ok(()) => SaveAnswer::Stored,"#),
    (r#"impl BaselineState for StateAdapter<'_> :: fn save_baseline"#, r#"Err(e) => SaveAnswer::Uncertain { said: e.to_string() },"#),
    (r#"impl BaselineState for StateAdapter<'_> :: fn reread"#, r#"fn reread(&mut self) -> Result<OwnedRecords, String> {"#),
    (r#"impl BaselineState for StateAdapter<'_> :: fn record_failure"#, r#"fn record_failure(&mut self, failure: &FailedAttempt) -> FailureRecord {"#),
    (r#"impl BaselineState for StateAdapter<'_> :: fn record_failure"#, r#"let not = |said: String| FailureRecord::NotRecorded { said };"#),
    (r#"impl BaselineState for StateAdapter<'_> :: fn record_failure"#, r#"Ok(()) => FailureRecord::Recorded,"#),
    (r#"fn cause_word"#, r#"fn cause_word(cause: &FailureCause) -> &'static str {"#),
    (r#"fn cause_word"#, r#"FailureCause::UnconfirmedChange { .. } => "external_change","#),
    (r#"fn cause_word"#, r#"FailureCause::UnstableObservation { .. } => "unstable_observation","#),
    (r#"fn cause_word"#, r#"FailureCause::ObservationRefused { .. } | FailureCause::TargetChanged { .. } => "unobservable","#),
    (r#"fn cause_word"#, r#"FailureCause::ObjectStore { .. } | FailureCause::StateNotStored { .. } => "store_failure","#),
    (r#"enum Driven"#, r#"Ended { boundary: crate::refs::CycleRef, outcome: RetryOutcome },"#),
    (r#"enum Driven"#, r#"/// `baseline_retry` 의 한 시도를 걸음마다 민다 — 그 모듈의 `run_retry` 와 같은 순서(`perform` → `advance`)이고, 다만 **보관을 진행시키는 걸음**"#),
    (r#"fn drive<'a>"#, r#"inputs: crate::artifact::tracking::RecordInputs<'a>,"#),
    (r#"fn drive<'a>"#, r#"state: &'a mut dyn BaselineState,"#),
    (r#"fn drive<'a>"#, r#") -> Result<Driven, crate::artifact::tracking::NotStarted> {"#),
    (r#"fn drive<'a>"#, r#"let mut attempt = RetryAttempt::begin(event, inputs, root, state)?;"#),
    (r#"fn drive<'a>"#, r#"Some(RetryRequest::Observe { .. }) => Some("observe"),"#),
    (r#"fn drive<'a>"#, r#"Some(RetryRequest::Capture { .. }) => Some("capture"),"#),
    (r#"fn drive<'a>"#, r#"Some(RetryRequest::StoreManifest { .. }) => Some("store_manifest"),"#),
    (r#"fn drive<'a>"#, r#"Some(RetryRequest::Save { .. }) => Some("save"),"#),
    (r#"fn attempt"#, r#"Ok(Driven::Ended { outcome: RetryOutcome::Failed { .. }, .. }) if stopped_at_boundary == Some("save") => {"#),
    (r#"fn attempt"#, r#"RetryOutcome::Confirmed { commit, delta, .. } => json!({"#),
    (r#"fn attempt"#, r#"RetryOutcome::Failed { cause, now } => {"#),
    (r#"fn attempt"#, r#"AfterFailure::StillWaiting { record, .. } => (Some(true), matches!(record, FailureRecord::Recorded)),"#),
    (r#"fn attempt"#, r#"AfterFailure::BindingGone { .. } => (Some(false), false),"#),
    (r#"fn attempt"#, r#"AfterFailure::NotVerified { .. } => (None, false),"#),
    (r#"fn attempt"#, r#"RetryOutcome::Unknown { why, .. } => json!({"result": "unknown", "why": format!("{why:?}")}),"#),
    (r#"fn retry"#, r#"after.events.push(Event::RetryRequested { event: event.to_string(), boundary: cycle.to_string(), source: MONITOR.into() });"#),
];

/// `src/scope_change/ledger.rs` — 그 모듈을 참조하지 않는다. 이 파일이 정의하는 사건 이름 `RetryRequested` 가 금지 이름을 품었을 뿐이다.
const SCOPE_LEDGER_SITES: [(&str, &str); 1] = [
    (r#"enum Event"#, r#"RetryRequested { event: String, boundary: String, source: String },"#),
];

/// `src/scope_change/tests.rs` — 그 모듈을 참조하지 않는다. 조회 결과의 칸 `last_confirmed_world` 를 읽는 단언들이다.
const SCOPE_TESTS_SITES: [(&str, &str); 6] = [
    (r#"fn approval_applies_the_policy_then_a_failed_archive_waits_through_an_interview_until_the_explicit_retry_opens_the_next_experiment"#, r#"assert_eq!(seen["tracking"]["last_confirmed_world"], "snapshot:A1");"#),
    (r#"fn approval_applies_the_policy_then_a_failed_archive_waits_through_an_interview_until_the_explicit_retry_opens_the_next_experiment"#, r#"assert_eq!(inside["tracking"]["last_confirmed_world"], "snapshot:A1");"#),
    (r#"fn approval_applies_the_policy_then_a_failed_archive_waits_through_an_interview_until_the_explicit_retry_opens_the_next_experiment"#, r#"assert_eq!(retried["tracking"]["last_confirmed_world"], "snapshot:A2");"#),
    (r#"fn when_the_final_reread_fails_the_attempts_facts_are_reported_but_no_current_state_is_claimed"#, r#"assert_eq!(seen["tracking"]["last_confirmed_world"], "snapshot:A2");"#),
    (r#"fn a_halt_applied_right_after_a_finished_baseline_save_keeps_that_save"#, r#"assert_eq!(seen["tracking"]["last_confirmed_world"], "snapshot:A2");"#),
    (r#"fn a_halt_received_between_archive_steps_starts_no_further_step_and_records_no_failure"#, r#"assert_eq!(seen["tracking"]["last_confirmed_world"], "snapshot:A1", "{step}: 확정된 세계가 없다");"#),
];

/// `crate::scope_change::` 를 부르는 제품 줄 전부 — (파일, 항목, 줄). load 의 승인 대응 검사, 쓰기 열기의 중단 요청 적용, 보관·저장
/// 앞의 중단 요청 확인, Monitor 값의 세 값. 범위 변경 모듈 자신과 MCP 범위 도구 모듈(`mcp/scope.rs`)은 뺀다.
const SCOPE_CHANGE_CALL_SITES: [(&str, &str, &str); 4] = [
    (r#"mcp/monitor.rs"#, r#"impl Monitor :: fn view"#, r#"match crate::scope_change::standing_view(session.project()) {"#),
    (r#"session.rs"#, r#"impl ProjectSession :: fn open_at"#, r#"crate::scope_change::absorb_halt_request(&mut session)"#),
    (r#"session.rs"#, r#"impl ProjectSession :: fn refuse_archive_while_halted"#, r#"if crate::scope_change::halt_requested(&gil_dir(&self.state_path)?) {"#),
    (r#"store.rs"#, r#"fn load_text"#, r#"crate::scope_change::verify_approvals(gil, tracking, &standing).map_err(StoreError::Tracking)?;"#),
];

/// MCP 범위 도구 모듈(`scope::`)의 등록·호출 줄 전부 — (파일, 항목, 줄). capability 보관소의 칸과 초기화, 호출의 분배, 카탈로그 등록,
/// 입력 검사.
const MCP_SCOPE_ROUTE_SITES: [(&str, &str, &str); 6] = [
    (r#"mcp.rs"#, r#"struct GilServer"#, r#"scope_grants: Arc<scope::Grants>,"#),
    (r#"mcp.rs"#, r#"impl GilServer :: fn with_display"#, r#"#[cfg(all(feature = "tracking-policy", unix))] scope_grants: Arc::new(scope::Grants::default()),"#),
    (r#"mcp.rs"#, r#"impl GilServer :: async fn dispatch"#, r#"if scope::NAMES.contains(&name.as_str()) {"#),
    (r#"mcp.rs"#, r#"impl GilServer :: async fn dispatch"#, r#"return tokio::task::spawn_blocking(move || locale::with(locale, || scope::call(&monitor, &grants, &name, input))).await"#),
    (r#"mcp/surface.rs"#, r#"fn tools"#, r#"let tools = super::scope::catalog(super::start::catalog(tools));"#),
    (r#"mcp/surface.rs"#, r#"fn valid_input"#, r#"if super::scope::NAMES.contains(&tool.name.as_ref()) { return super::scope::valid(tool.name.as_ref(), input); }"#),
];

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

    // (2) `src/scope_change/` 의 세 파일: 파일도, 이름의 집합도 승인의 근거가 아니다. 금지 이름 검색이 잡는 **줄 하나하나**가 승인된
    //     자리(그 줄을 품은 항목 + 정규화한 줄 글)의 열과 정확히 같아야 한다 — 같은 이름을 다른 자리에서 더 쓰거나, 같은 줄을 한 번 더
    //     적거나, 다른 항목으로 옮기거나, 승인된 자리를 없애면 실패한다. 줄 번호에 기대지 않는다. 그 폴더의 다른 파일은 여기 없으므로
    //     아래의 일반 규칙이 그대로 거절한다.
    let detected = |line: &str| names.iter().any(|name| line.contains(name));
    let scope_files: [(&str, &[(&str, &str)]); 3] = [
        ("scope_change/mod.rs", &SCOPE_MOD_SITES),
        ("scope_change/ledger.rs", &SCOPE_LEDGER_SITES),
        ("scope_change/tests.rs", &SCOPE_TESTS_SITES),
    ];
    for (file, approved) in scope_files {
        let found = sites(&std::fs::read_to_string(src.join(file)).unwrap(), detected);
        let differences = site_differences(&found, approved);
        assert!(differences.is_empty(), "{file}: 승인된 자리와 다르다:\n{differences:#?}");
        // 실제 참조(낱말 전체가 금지 이름)는 두 재수출 블록이 내보내는 이름뿐이다 — 기록 모듈의 경로를 직접 쓰지 않는다.
        for name in references(&found, &names) {
            assert!(exported.contains(&name.as_str()), "{file}: {name} 은 재수출 블록이 내보내는 이름이 아니다");
        }
    }
    assert!(references(&sites(&std::fs::read_to_string(src.join("scope_change/ledger.rs")).unwrap(), detected), &names).is_empty());
    assert!(references(&sites(&std::fs::read_to_string(src.join("scope_change/tests.rs")).unwrap(), detected), &names).is_empty());
    // (3) 그 연결은 `tracking-policy` + Unix 에서만 컴파일된다. 모듈 선언, 그 자식, 그것을 부르는 제품 줄 모두.
    assert!(lib.contains(&format!("{UNIX_TRACKING_GATE}\nmod scope_change;")), "범위 변경 모듈은 기능과 Unix 뒤에 있다");
    let sources = rust_sources(&src);
    assert_eq!(sources.iter().map(|(_, text)| text.lines().filter(|line| line.trim() == "mod scope_change;").count()).sum::<usize>(), 1);
    assert!(!sources.iter().any(|(_, text)| text.lines().any(|line| line.contains("#[path") && line.contains("scope_change"))), "다른 자리에서 끌어오지 않는다");
    let scope_mod = std::fs::read_to_string(src.join("scope_change").join("mod.rs")).unwrap();
    assert!(scope_mod.contains("\nmod ledger;\n"), "원장은 그 모듈의 자식이다");
    // 시험 모듈은 시험 빌드에서만 있다. 탐색 계약 구성에서도 같은 시험이 돈다(Experiment 를 요구에 연결해 여는 helper 하나만 다르다) —
    // 문이 넓어진 것은 시험 빌드 안에서뿐이며, 제품 빌드에는 종전처럼 이 모듈이 없다.
    assert!(scope_mod.contains("\n#[cfg(test)]\nmod tests;"), "단위 시험은 시험 빌드의 자식이다");
    assert_eq!(scope_mod.lines().filter(|line| line.trim() == "mod tests;").count(), 1);
    // 그 모듈을 부르는 제품 줄과 MCP 범위 도구의 등록·호출 줄: 파일 목록이 아니라 **승인된 자리의 열**과 견주고(새 자리·중복·이동·
    // 누락은 실패), 그 줄 하나하나가 문 뒤에 있는지도 함께 본다.
    let routed = |needle: &str, covers: &dyn Fn(&str) -> bool| -> (Vec<(String, Site)>, Vec<String>) {
        let (mut found, mut ungated) = (Vec::new(), Vec::new());
        for (file, text) in sources.iter().filter(|(file, _)| covers(file) && file != "mcp/scope.rs" && file != "mcp/scope_tests.rs") {
            found.extend(sites(text, |line| !line.starts_with("//") && mentions_path(line, needle)).into_iter().map(|site| (file.clone(), site)));
            ungated.extend(ungated_uses(text, needle).1.into_iter().map(|line| format!("{file}: {line}")));
        }
        (found, ungated)
    };
    let as_rows = |found: &[(String, Site)]| found.iter().map(|(file, site)| (file.clone(), site.item.clone(), site.line.clone())).collect::<Vec<_>>();
    let approved_rows = |rows: &[(&str, &str, &str)]| rows.iter().map(|(file, item, line)| (file.to_string(), item.to_string(), line.to_string())).collect::<Vec<_>>();
    let (callers, ungated_callers) = routed("scope_change::", &|file: &str| !file.starts_with("scope_change/"));
    assert_eq!(as_rows(&callers), approved_rows(&SCOPE_CHANGE_CALL_SITES), "범위 변경 모듈을 부르는 제품 줄이 승인된 자리와 다르다");
    assert!(ungated_callers.is_empty(), "문 밖에서 범위 변경 모듈을 부른다:\n{ungated_callers:#?}");
    // MCP 의 범위 도구 모듈과 그 등록·호출 경로도 같은 문 뒤에 있다(`mcp/scope.rs` 는 그 모듈 자신이다).
    let mcp = std::fs::read_to_string(src.join("mcp.rs")).unwrap();
    assert!(mcp.contains(&format!("{UNIX_TRACKING_GATE}\nmod scope;")), "MCP 범위 도구 모듈은 기능과 Unix 뒤에 있다");
    let (routes, ungated_routes) = routed("scope::", &|file: &str| file == "mcp.rs" || file.starts_with("mcp/"));
    assert_eq!(as_rows(&routes), approved_rows(&MCP_SCOPE_ROUTE_SITES), "MCP 범위 도구의 등록·호출 줄이 승인된 자리와 다르다");
    assert!(ungated_routes.is_empty(), "문 밖에서 MCP 범위 도구를 부른다:\n{ungated_routes:#?}");

    let allowed = |line: &str| {
        let line = line.replace('\\', "/");
        let numbered = |file: &str| line.strip_prefix(file).and_then(|rest| rest.split(':').next()).and_then(|n| n.parse::<usize>().ok());
        line.ends_with(": mod tracking_record;")
            || gated_files.iter().any(|file| line.starts_with(file))
            || numbered("artifact/mod.rs:").is_some_and(|n| n > block_start && n <= block_end)
            // 5번: 재수출 블록의 줄 구간, 그리고 세 파일에서는 **승인된 자리의 줄 글과 같은 줄만**(경로만으로 허용하지 않는다 — 그
            // 자리의 항목·횟수·순서는 위의 열 비교가 본다).
            || numbered("artifact/mod.rs:").is_some_and(|n| n > tracking_open && n < tracking_close)
            || scope_files.iter().any(|(file, approved)| {
                let mut parts = line.splitn(3, ':');
                parts.next() == Some(*file) && parts.next().is_some()
                    && parts.next().is_some_and(|text| approved.iter().any(|(_, known)| text.split_whitespace().eq(known.split_whitespace())))
            })
    };
    let outside: Vec<_> = users.iter().filter(|line| !allowed(line)).collect();
    assert!(outside.is_empty(), "새 모듈이 제품 소스에서 쓰였다:\n{outside:#?}");
    assert!(users.iter().any(|line| line.ends_with(": mod tracking_record;")), "{users:#?}");
}

/// 위 검사의 도우미들이 **정확히 승인된 연결만** 받아들이는가 — 작은 합성 소스 글로 본다. 제품 파일을 고치거나 실행하지 않는다.
#[test]
fn the_boundary_helpers_accept_the_exact_approved_connection_and_refuse_extra_references_and_missing_gates() {
    let names = ["RetryRequest", "run_retry", "confirmed_world", "tracking_record"];
    let detected = |line: &str| names.iter().any(|name| line.contains(name));
    // 승인된 자리 넷: 참조 둘(서로 다른 항목), 금지 이름을 속에 품은 낱말이 있는 줄 하나, 주석 줄 하나.
    let source = "// run_retry 를 부르지 않는다\nuse crate::artifact::tracking::RetryRequest;\n\nfn first() {\n    let step = RetryRequest::Observe;\n}\n\nimpl Ledger {\n    pub(crate) fn note(&self) {\n        let e = Event::RetryRequested;\n    }\n}\n";
    let approved: [(&str, &str); 4] = [
        ("", "// run_retry 를 부르지 않는다"),
        ("use crate::artifact::tracking::RetryRequest", "use crate::artifact::tracking::RetryRequest;"),
        ("fn first", "let step = RetryRequest::Observe;"),
        ("impl Ledger :: fn note", "let e = Event::RetryRequested;"),
    ];
    // 정확히 승인된 자리는 받아들인다. 실제 참조는 둘뿐이다(주석과 `RetryRequested` 는 참조가 아니다).
    let found = sites(source, detected);
    assert_eq!(site_differences(&found, &approved), Vec::<String>::new(), "{found:#?}");
    assert_eq!(references(&found, &names), ["RetryRequest", "RetryRequest"]);
    let refused = |changed: &str, what: &str| {
        let differences = site_differences(&sites(changed, detected), &approved);
        assert!(!differences.is_empty(), "{what}: 받아들여졌다");
        differences
    };
    // 이미 승인된 **같은 이름**을 다른 자리에서 한 번 더 쓴다 — 이름의 집합은 그대로지만 자리가 늘었다.
    let extra = refused(&source.replace("fn first() {\n", "fn first() {\n    let again = RetryRequest::Capture;\n"), "같은 이름의 추가 사용");
    assert_eq!(extra, ["승인되지 않은 자리: [fn first] let again = RetryRequest::Capture;"]);
    // 같은 줄을 한 번 더 적는다 — 글자가 같아도 횟수가 다르다.
    let twice = refused(&source.replace("    let step = RetryRequest::Observe;\n", "    let step = RetryRequest::Observe;\n    let step = RetryRequest::Observe;\n"), "같은 참조의 중복");
    assert_eq!(twice, ["승인되지 않은 자리: [fn first] let step = RetryRequest::Observe;"]);
    // 같은 줄을 다른 항목으로 옮긴다 — 줄 글과 횟수는 그대로지만 품은 항목이 다르다.
    let moved = refused(&source.replace("fn first() {\n    let step = RetryRequest::Observe;\n}\n", "fn first() {\n}\n\nfn second() {\n    let step = RetryRequest::Observe;\n}\n"), "다른 항목으로 이동");
    assert_eq!(moved, ["승인되지 않은 자리: [fn second] let step = RetryRequest::Observe;", "승인된 자리가 없다: [fn first] let step = RetryRequest::Observe;"]);
    // 승인된 자리가 없어진다.
    let missing = refused(&source.replace("    let step = RetryRequest::Observe;\n", ""), "승인 자리의 누락");
    assert_eq!(missing, ["승인된 자리가 없다: [fn first] let step = RetryRequest::Observe;"]);
    // 예상 밖 참조 — 다른 금지 이름, 기록 모듈의 직접 경로, 꼬리 주석 속의 이름, 금지 이름을 품은 새 낱말, 새 주석 줄. 어느 것도 통과하지 않는다.
    for line in ["    let out = run_retry(event);\n", "    use crate::artifact::tracking_record::Anything;\n", "    let x = 1; // confirmed_world\n",
        "    let y = RetryRequestedAgain;\n", "    // confirmed_world 를 읽는다\n"] {
        refused(&source.replace("fn first() {\n", &format!("fn first() {{\n{line}")), line);
    }
    // 자리들이 같고 순서만 다른 것도 같지 않다.
    let reordered = [approved[1], approved[0], approved[2], approved[3]];
    assert_eq!(site_differences(&found, &reordered), ["자리들의 순서가 승인된 것과 다르다"]);

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

    // 호출 자리도 열로 견준다: 문 뒤의 승인된 자리 하나는 받아들이고, 같은 호출을 다른 항목에 하나 더 두면(문 뒤에 있어도) 거절한다.
    // 문만 떼어 내면 자리는 같아도 문 검사가 거절한다.
    let calls = |text: &str| sites(text, |line| !line.starts_with("//") && mentions_path(line, "scope_change::"));
    let routed = format!("fn load() {{\n    {gate}\n    crate::scope_change::verify(x);\n}}\n");
    let approved_call: [(&str, &str); 1] = [("fn load", "crate::scope_change::verify(x);")];
    assert!(site_differences(&calls(&routed), &approved_call).is_empty());
    assert!(ungated_uses(&routed, "scope_change::").1.is_empty());
    let elsewhere = format!("{routed}fn save() {{\n    {gate}\n    crate::scope_change::verify(x);\n}}\n");
    assert_eq!(site_differences(&calls(&elsewhere), &approved_call), ["승인되지 않은 자리: [fn save] crate::scope_change::verify(x);"]);
    assert!(ungated_uses(&elsewhere, "scope_change::").1.is_empty(), "문 뒤에 있다는 것만으로는 새 자리가 승인되지 않는다");
    let gate_removed = routed.replace(&format!("    {gate}\n"), "");
    assert!(site_differences(&calls(&gate_removed), &approved_call).is_empty());
    assert_eq!(ungated_uses(&gate_removed, "scope_change::").1, ["crate::scope_change::verify(x);"]);

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
