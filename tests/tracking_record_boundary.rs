//! 구현 순서 3a(정책 기록 자료형과 부분 검증)와 그 뒤의 동기화 순서 제어(편의상 3b)가
//! **제품 저장 경로를 바꾸지 않았는가.**
//!
//! 모든 빌드 구성에서 짓는다. 시험 자료는 합성 프로젝트다(독립 임시 폴더, 프로세스 번호가 붙은 이름).
//!
//! - 이 빌드는 형식 10/11 을 알지 못한다: 구조를 해석하기 전에 `UnknownFormat` 으로 거절하고 폴더를
//!   바꾸지 않는다.
//! - 지금 형식의 `state.yaml` 에 `tracking` 블록(정책 원장·동기화 기준)을 덧붙여도 로더가 그것을 받아들여
//!   무시한 채 진행하지 않는다 — 모르는 칸으로 거절하고 폴더를 바꾸지 않는다.
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
