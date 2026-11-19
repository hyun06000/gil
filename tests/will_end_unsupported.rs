//! **종료 이력을 모르는 빌드**가 형식 8/9 와 미달성 요청을 어떻게 다루는가.
//!
//! `will-end-history` 를 켜지 않은 모든 구성(기본·`chain-core`·승인 계획·제품 보호)에서 짓는다.
//!
//! - 형식 8/9 파일은 구조를 해석하기 전에 `UnknownFormat` 으로 거절되고, 그 폴더의 어느
//!   파일도 달라지지 않는다. 복구도 정리도 하지 않는다.
//! - 미달성 요청은 조용히 달성 닫기로 읽히지 않는다. 명시적 전환이 필요하다고 말하고
//!   거절한다 — 아직 없는 전환 명령을 실행할 수 있는 것처럼 말하지 않는다.
//! - 도구 표면은 그 입력을 내놓지 않는다.
#![cfg(not(feature = "will-end-history"))]

mod common;
use gil::{ActionError, NodeKind, Project, ProjectSession, SessionError, StoreError, WillRef};
use serde_norway::Value;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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

/// 이 빌드가 쓴 기록을 **형식 8/9 를 선언한 파일**로 바꿔 놓는다.
///
/// 이 빌드는 그 형식을 쓸 수 없으므로 합성한다: 번호를 올리고 `done_wills` 를 `ended_wills`
/// 로 바꾼다. 관문은 구조를 해석하기 전에 서므로 나머지 꼴은 판정에 쓰이지 않는다.
fn declared_as(root: &Path, format: u32) {
    let path = root.join(gil::STATE_PATH);
    let mut file: Value = serde_norway::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    file["format"] = Value::from(format);
    for (_, existence) in file["existences"].as_mapping_mut().unwrap() {
        let journey = existence["journey"].as_mapping_mut().unwrap();
        let renamed: serde_norway::Mapping = journey.iter().map(|(key, value)| match key.as_str() {
            Some("done_wills") => (Value::from("ended_wills"), value.clone()),
            _ => (key.clone(), value.clone()),
        }).collect();
        *journey = renamed;
    }
    std::fs::write(&path, serde_norway::to_string(&file).unwrap()).unwrap();
}

fn started(label: &str) -> PathBuf {
    let root = common::scratch(label);
    std::fs::write(root.join("work.txt"), "user file").unwrap();
    let session = ProjectSession::start(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    session.commit().unwrap();
    root
}

#[test]
fn formats_8_and_9_are_refused_before_anything_is_touched() {
    for format in [8, 9] {
        let root = started(&format!("end-unsupported-{format}"));
        declared_as(&root, format);
        let before = tree(&root);
        let path = root.join(gil::STATE_PATH);
        let known = gil::newest_known_format();
        assert!(known < 8, "이 빌드는 형식 8 을 모른다");

        match gil::load(common::spec(), &path) {
            Err(StoreError::UnknownFormat { found, known: said }) => assert_eq!((found, said), (format, known)),
            other => panic!("형식 {format} 이 읽혔다: {other:?}"),
        }
        // 변경 세션도, 읽기 전용 세션도 열리지 않는다 — 복구·임시 객체 회수도 돌지 않는다.
        assert!(ProjectSession::open(common::spec(), &path).is_err());
        assert!(ProjectSession::open_read_only(common::spec(), &path).is_err());
        assert_eq!(tree(&root), before, "형식 {format} 을 거절하면서 폴더를 바꿨다");

        // CLI 도 같다: 조회든 변경이든 거절하고 그대로 둔다.
        for args in [vec!["status"], vec!["context"], vec!["open", "question"], vec!["close"]] {
            let mut child = Command::new(env!("CARGO_BIN_EXE_gil")).args(&args).current_dir(&root)
                .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
            child.stdin.take().unwrap().write_all(b"objective: a\nnext_action: b\ndone_when: c\n").unwrap();
            let done = child.wait_with_output().unwrap();
            assert!(!done.status.success(), "{args:?} 가 형식 {format} 에서 성공했다");
            assert!(String::from_utf8_lossy(&done.stderr).contains(&format.to_string()), "{}", String::from_utf8_lossy(&done.stderr));
        }
        assert_eq!(tree(&root), before, "CLI 가 형식 {format} 의 폴더를 바꿨다");
    }
}

#[test]
fn an_unachieved_request_is_refused_with_a_conversion_notice_and_closes_nothing() {
    let root = common::scratch("end-unsupported-request");
    let mut session = ProjectSession::start(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    common::opened(session.project_mut(), NodeKind::Question);
    session.commit().unwrap();
    assert!(!session.project().records_end_results());
    let will: WillRef = session.project().active_will().unwrap().id();
    // The whole folder is compared byte for byte, the lock file included — so it is read only
    // while no session holds the lock. Windows refuses to read a byte-range-locked file (os error
    // 33); the product lock is kept as it is and the comparison follows the lock's lifetime.
    drop(session);
    let before = tree(&root);
    let mut session = ProjectSession::open(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    let memory = format!("{:?}", session.project());
    let format = if cfg!(feature = "chain-core") { gil::CHAIN_FORMAT } else { gil::FORMAT };

    let report = common::full_report(&common::spec(), gil::CycleKind::Interview, NodeKind::Question);
    let err = session.close_step_unachieved(report, will).unwrap_err();
    match &err {
        SessionError::Action(ActionError::EndHistoryUnsupported { format: said }) => assert_eq!(*said, format),
        other => panic!("전환 안내 거절이 아니다: {other}"),
    }
    let said = err.to_string();
    assert!(said.contains("8/9"), "어느 형식이 필요한지 말해야 한다: {said}");
    assert!(!said.contains("gil convert") && !said.contains("gil migrate"), "없는 전환 명령을 안내했다: {said}");
    assert_eq!(format!("{:?}", session.project()), memory);
    drop(session);
    assert_eq!(tree(&root), before);

    // CLI: 표시가 조용히 떨어져 달성 닫기가 되지 않는다.
    let mut child = Command::new(env!("CARGO_BIN_EXE_gil")).args(["close", "--unachieved", &will.to_string()]).current_dir(&root)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all("question: 무엇을\nchoices: 가 / 나\nresponse: 가\n".as_bytes()).unwrap();
    let done = child.wait_with_output().unwrap();
    assert!(!done.status.success());
    assert!(String::from_utf8_lossy(&done.stderr).contains("8/9"), "{}", String::from_utf8_lossy(&done.stderr));
    assert_eq!(tree(&root), before, "거절된 미달성 요청이 기록을 바꿨다");
    let project: Project = gil::load(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    assert_eq!(project.active_will().map(|active| active.id()), Some(will), "약속이 그대로 걸려 있어야 한다");
}

#[test]
fn a_new_project_keeps_the_done_list_shape_of_its_format() {
    let root = started("end-unsupported-shape");
    let text = std::fs::read_to_string(root.join(gil::STATE_PATH)).unwrap();
    assert!(text.contains("done_wills: []"), "{text}");
    assert!(!text.contains("ended_wills") && !text.contains("end_result"), "{text}");
}
