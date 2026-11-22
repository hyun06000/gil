//! 정책 원문 보관 경계(「3m」)의 시험 — **합성 자료**만 쓴다.
//!
//! 원문·`.gilignore`·`.gitignore`·`.gil/state.yaml` 은 시험이 만든 바이트다. 모니터가 승인한 원문도, 실제 GIL 상태도
//! 아니며 `gil start` 를 부르지 않는다. 창고를 쓰는 시험은 임시 루트를 배타적으로 새로 만들고(이미 있는 이름은 건드리지
//! 않고 다른 이름으로 다시 시도한다), 합성 파일을 그 루트 안에만 쓰며, 끝날 때 그 루트만 지운다.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::*;
use crate::artifact::gilignore::{ApprovedPolicy, PolicyIdentity};
use crate::artifact::{ObjectError, ObjectStore, digest_from_raw, digest_of};

// ── 도구 ───────────────────────────────────────────────────────────────────

fn address(bytes: &[u8]) -> ContentDigest {
    digest_of(bytes).expect("메모리의 바이트는 언제나 읽힌다")
}

/// 시험용 제안 참조 — 아무 제안 원장도 가리키지 않는 합성 값이다.
fn proposal(seed: u8) -> ProposalDigest {
    ProposalDigest(digest_from_raw([seed; 32]))
}

fn name(number: u32) -> PolicyName {
    PolicyName::new(number).expect("0 이 아니다")
}

/// 합성 프로젝트 하나 — 루트의 `.gilignore`·`.gitignore` 와 `.gil/state.yaml` 자리에 이 시험만의 바이트를 둔다.
///
/// 루트 `.gilignore` 의 내용은 어느 시험 원문과도 다르다. 결과가 그 파일에서 나왔다면 주소가 어긋나 드러난다.
fn project(label: &str) -> (PathBuf, ObjectStore) {
    let root = fresh_root(label);
    fs::create_dir(root.join(".gil")).expect("새 루트 안에 .gil 을 만들 수 있어야 한다");
    new_file(&root.join(".gilignore"), DISK_GILIGNORE);
    new_file(&root.join(".gitignore"), b"from-git/\n");
    new_file(&root.join(".gil").join("state.yaml"), b"synthetic: not a GIL state\n");
    let store = ObjectStore::at(&root.join(".gil"));
    (root, store)
}

/// 이번 호출이 **새로 만든** 임시 루트. 후보 이름이 이미 있으면 그 항목을 그대로 두고 다른 이름으로 다시 시도한다.
/// 그 밖의 생성 오류는 시험 실패다. 먼저 지우지 않는다 — 남의 폴더일 수 있다.
fn fresh_root(label: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    loop {
        let candidate = std::env::temp_dir().join(format!(
            "gil-source-store-{}-{}-{}-{label}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        match fs::create_dir(&candidate) {
            Ok(()) => return candidate,
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => panic!("시험 임시 루트를 만들지 못했다 {}: {err}", candidate.display()),
        }
    }
}

/// 새 루트 안에 파일을 **새로** 만든다 — 같은 이름이 있으면 덮어쓰지 않고 시험 실패다.
fn new_file(at: &Path, bytes: &[u8]) {
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(at)
        .unwrap_or_else(|err| panic!("합성 파일 {} 을 새로 만들지 못했다: {err}", at.display()));
    file.write_all(bytes).unwrap();
}

const DISK_GILIGNORE: &[u8] = b"# disk copy, never the input\non-disk-only/\n";

/// 폴더 아래 모든 일반 파일의 (상대 경로 → 바이트). 폴더 자체는 세지 않는다.
fn files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let kind = fs::symlink_metadata(&path).unwrap().file_type();
            if kind.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(path.strip_prefix(root).unwrap().to_path_buf(), fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

/// 창고 안에서 그 지문의 객체 파일들 — 창고의 배치 규칙을 다시 적지 않고 이름으로만 찾는다.
fn object_files(store: &ObjectStore, digest: &ContentDigest) -> Vec<PathBuf> {
    let hex = digest.hex();
    let mut found = Vec::new();
    if store.root().exists() {
        for (path, _) in files(store.root()) {
            if path.file_name().and_then(|name| name.to_str()) == Some(&hex[2..]) {
                found.push(store.root().join(path));
            }
        }
    }
    found
}

/// 창고 `tmp/` 에 남은 파일 — 실패한 확정이 잔해를 남기지 않는지 본다.
fn temp_leftovers(store: &ObjectStore) -> Vec<PathBuf> {
    match store.tmp().exists() {
        true => files(&store.tmp()).into_keys().collect(),
        false => Vec::new(),
    }
}

fn identity_of(source: &[u8]) -> PolicyIdentity {
    ApprovedPolicy::compile(EngineVersion::V1, source).expect("시험 원문은 해석된다").identity().clone()
}

/// 줄바꿈·주석·끝 공백·마지막 줄바꿈 유무·빈 원문 — 모두 해석되며 정규화하면 서로 같아질 수 있는 바이트들.
const VARIANTS: [&[u8]; 6] = [
    b"node_modules/\n",
    b"node_modules/\r\n",
    b"node_modules/",
    b"# \xec\x9d\x98\xec\xa1\xb4\xec\x84\xb1\nnode_modules/\n",
    b"node_modules/ \n",
    b"",
];

/// 엔진이 해석하지 못하는 원문(역슬래시 경로) — 3a 시험과 같은 합성 예.
const UNPARSABLE: &[u8] = b"src\\gen\n";

// ── 새 원문의 보관과 왕복 ─────────────────────────────────────────────────

#[test]
fn new_sources_are_stored_byte_for_byte_and_read_back_through_the_existing_lookups() {
    let (root, store) = project("roundtrip");
    let before = files(&root);

    let mut stored = Vec::new();
    for source in VARIANTS {
        let expected = address(source);
        let result = store_policy_source(&store, EngineVersion::V1, &expected, source)
            .unwrap_or_else(|err| panic!("{:?} 를 보관하지 못했다: {err}", String::from_utf8_lossy(source)));
        assert_eq!(result.address(), &expected, "결과의 주소는 기대 주소다");
        assert_eq!(result.identity(), &identity_of(source), "정체성은 기존 정책 엔진이 같은 바이트로 세운 것이다");
        assert_eq!(result.identity().engine(), EngineVersion::V1);
        assert_ne!(expected, address(DISK_GILIGNORE), "루트 .gilignore 의 바이트가 아니다");

        // 기존 조회 경로(3a 의 읽기 전용 PolicySourceLookup → ObjectStore::read_blob)로 바이트 단위 왕복.
        assert_eq!(store.rules_source(&expected), SourceLookup::Found(source.to_vec()));
        let mut bytes = Vec::new();
        store.read_blob(&expected, &mut bytes).unwrap();
        assert_eq!(bytes, source);
        stored.push(result);
    }

    // 정규화하지 않았다: 여섯 원문은 여섯 주소다.
    let mut addresses: Vec<_> = stored.iter().map(|result| result.address().clone()).collect();
    addresses.sort_by_key(|digest| digest.hex());
    addresses.dedup();
    assert_eq!(addresses.len(), VARIANTS.len(), "줄바꿈·주석·공백·끝 줄바꿈 차이가 같은 주소로 합쳐졌다");

    // 3a 의 원문 검사가 그 창고에서 판마다 같은 정체성을 다시 세운다.
    let records = VARIANTS
        .iter()
        .enumerate()
        .map(|(at, source)| {
            PolicyRecord::new(name(at as u32 + 1), EngineVersion::V1, address(source), proposal(at as u8 + 1))
        })
        .collect();
    let ledger = PolicyLedger::from_records(VARIANTS.len() as u32 + 1, records).unwrap();
    let checked = check_policy_sources(&ledger, &store).expect("보관한 원문은 3a 검사를 통과한다");
    for (at, result) in stored.iter().enumerate() {
        assert_eq!(checked.identity_of(name(at as u32 + 1)), Some(result.identity()));
    }

    // 바뀐 것은 원문 객체 여섯 개를 더한 것뿐이다 — 정책 파일·Git 파일·상태 자리는 그대로이고 tmp/ 에 잔해가 없다.
    let after = files(&root);
    for (path, bytes) in &before {
        assert_eq!(after.get(path), Some(bytes), "{} 가 바뀌었다", path.display());
    }
    let added: Vec<_> = after.keys().filter(|path| !before.contains_key(*path)).collect();
    assert_eq!(added.len(), VARIANTS.len(), "{added:#?}");
    for source in VARIANTS {
        let objects = object_files(&store, &address(source));
        assert_eq!(objects.len(), 1, "{objects:?}");
        assert!(objects[0].starts_with(store.root().join("blobs")), "blob 층에 있다: {}", objects[0].display());
        assert_eq!(fs::read(&objects[0]).unwrap(), source);
    }
    assert!(temp_leftovers(&store).is_empty());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn the_root_gilignore_and_gitignore_are_never_the_input() {
    // 루트 파일이 없어도, 입력과 다른 내용이어도 결과는 건넨 바이트의 것이다. 루트 파일은 읽히지도 바뀌지도 않는다.
    let (root, store) = project("no-disk-input");
    fs::remove_file(root.join(".gilignore")).unwrap();
    fs::remove_file(root.join(".gitignore")).unwrap();
    let source: &[u8] = b"build/\n";
    let result = store_policy_source(&store, EngineVersion::V1, &address(source), source).unwrap();
    assert_eq!(result.identity(), &identity_of(source));
    assert!(!root.join(".gilignore").exists(), "보관이 .gilignore 를 만들었다");
    assert!(!root.join(".gitignore").exists());

    // 루트 .gilignore 의 바이트를 기대 주소로 건네도 입력 바이트가 그 내용이 아니면 거절된다 — 디스크로 메우지 않는다.
    fs::write(root.join(".gilignore"), DISK_GILIGNORE).unwrap();
    let problem = store_policy_source(&store, EngineVersion::V1, &address(DISK_GILIGNORE), source).unwrap_err();
    assert_eq!(problem, SourceStoreProblem::AddressMismatch { expected: address(DISK_GILIGNORE), found: address(source) });
    assert!(object_files(&store, &address(DISK_GILIGNORE)).is_empty(), "디스크의 .gilignore 가 창고에 들어갔다");
    assert_eq!(fs::read(root.join(".gilignore")).unwrap(), DISK_GILIGNORE);
    let _ = fs::remove_dir_all(&root);
}

// ── 같은 원문의 재시도 ─────────────────────────────────────────────────────

#[test]
fn retrying_the_same_source_gives_the_same_success_without_a_second_object() {
    let (root, store) = project("retry");
    let source: &[u8] = b"target/\r\n# keep\n!target/keep.txt\n";
    let expected = address(source);
    let first = store_policy_source(&store, EngineVersion::V1, &expected, source).unwrap();
    let objects = object_files(&store, &expected);
    assert_eq!(objects.len(), 1);
    let written = fs::metadata(&objects[0]).unwrap().modified().unwrap();
    let settled = files(&root);

    for _ in 0..3 {
        let again = store_policy_source(&store, EngineVersion::V1, &expected, source).unwrap();
        assert_eq!(again, first, "같은 원문의 재시도는 같은 결과다");
    }
    assert_eq!(files(&root), settled, "재시도가 창고나 다른 파일을 바꿨다");
    assert_eq!(object_files(&store, &expected), objects, "두 번째 객체가 생겼다");
    assert_eq!(fs::metadata(&objects[0]).unwrap().modified().unwrap(), written, "기존 객체를 다시 썼다");

    // 쓰기 전에 거절된 시도 뒤의 재시도도 같은 성공이다 — 거절은 흔적을 남기지 않는다.
    assert!(store_policy_source(&store, EngineVersion::V1, &address(b"other/\n"), source).is_err());
    assert_eq!(store_policy_source(&store, EngineVersion::V1, &expected, source).unwrap(), first);
    assert_eq!(files(&root), settled);
    let _ = fs::remove_dir_all(&root);
}

// ── 쓰기 전 거절 ───────────────────────────────────────────────────────────

#[test]
fn a_wrong_expected_address_is_refused_before_any_write() {
    let (root, store) = project("address-mismatch");
    let before = files(&root);
    let source: &[u8] = b"dist/\n";
    let wrong = address(b"dist/\r\n");

    let problem = store_policy_source(&store, EngineVersion::V1, &wrong, source).unwrap_err();
    assert_eq!(problem, SourceStoreProblem::AddressMismatch { expected: wrong.clone(), found: address(source) });
    assert!(problem.refused_before_write());
    assert!(!store.root().exists(), "거절이 창고 폴더를 만들었다");
    assert_eq!(files(&root), before);

    // 기대 주소의 객체가 이미 있어도 같다 — 그 객체는 그대로이고, 건넨 바이트의 객체는 생기지 않는다.
    let other: &[u8] = b"dist/\r\n";
    store_policy_source(&store, EngineVersion::V1, &wrong, other).unwrap();
    let settled = files(&root);
    let problem = store_policy_source(&store, EngineVersion::V1, &wrong, source).unwrap_err();
    assert!(matches!(problem, SourceStoreProblem::AddressMismatch { .. }), "{problem}");
    assert_eq!(files(&root), settled);
    assert_eq!(store.rules_source(&address(source)), SourceLookup::Missing);
    assert_eq!(store.rules_source(&wrong), SourceLookup::Found(other.to_vec()));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn unparsable_rules_are_refused_before_any_write() {
    let (root, store) = project("rejected");
    let before = files(&root);
    let expected = address(UNPARSABLE);

    let problem = store_policy_source(&store, EngineVersion::V1, &expected, UNPARSABLE).unwrap_err();
    match &problem {
        SourceStoreProblem::Rejected(rejected) => {
            assert_eq!(rejected.identity.rules_digest(), &expected, "거절도 같은 바이트의 정체성으로 말한다");
            assert!(!rejected.problems.is_empty());
        }
        other => panic!("해석 실패가 아닌 것으로 거절했다: {other}"),
    }
    assert!(problem.refused_before_write());
    assert!(!store.root().exists(), "해석 실패가 창고 폴더를 만들었다");
    assert_eq!(files(&root), before);
    assert_eq!(store.rules_source(&expected), SourceLookup::Missing);

    // 주소도 틀리고 해석도 안 되면 주소가 먼저다(3a 의 읽기 검사와 같은 순서).
    let wrong = address(b"x/\n");
    let problem = store_policy_source(&store, EngineVersion::V1, &wrong, UNPARSABLE).unwrap_err();
    assert_eq!(problem, SourceStoreProblem::AddressMismatch { expected: wrong, found: expected });
    assert_eq!(files(&root), before);
    let _ = fs::remove_dir_all(&root);
}

// ── 창고 오류 ──────────────────────────────────────────────────────────────

#[test]
fn a_damaged_existing_object_is_the_stores_error_and_stays_as_found() {
    let (root, store) = project("damaged-existing");
    let source: &[u8] = b"coverage/\n";
    let expected = address(source);
    store_policy_source(&store, EngineVersion::V1, &expected, source).unwrap();
    let object = object_files(&store, &expected).pop().unwrap();
    fs::write(&object, b"tampered").unwrap();
    let before = files(&root);

    for _ in 0..2 {
        let problem = store_policy_source(&store, EngineVersion::V1, &expected, source).unwrap_err();
        assert_eq!(
            problem,
            SourceStoreProblem::Store(ObjectError::Corrupt {
                kind: crate::artifact::store::ObjectKind::Blob,
                address: expected.hex(),
                found: address(b"tampered").hex(),
            }),
            "창고의 손상 오류 그대로여야 한다"
        );
        assert!(!problem.refused_before_write());
        assert_eq!(fs::read(&object).unwrap(), b"tampered", "손상된 객체를 고쳐 썼다");
        assert_eq!(files(&root), before, "정책 파일·상태 자리·창고가 바뀌었다");
        assert!(temp_leftovers(&store).is_empty(), "{:?}", temp_leftovers(&store));
    }
    assert!(matches!(store.rules_source(&expected), SourceLookup::Failed { .. }), "손상은 없음이 아니다");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn an_object_that_does_not_read_back_as_written_is_not_success() {
    // 창고가 확정 직후 다시 읽은 바이트가 다르면 성공이 아니다 — 창고 시험이 쓰는 같은 시험 전용 틈으로 일으킨다.
    let (root, store) = project("landed-wrong");
    let source: &[u8] = b"out/\n";
    let expected = address(source);
    crate::artifact::store::meddle::plant(b"not what was written");

    let problem = store_policy_source(&store, EngineVersion::V1, &expected, source).unwrap_err();
    assert!(
        matches!(&problem, SourceStoreProblem::Store(ObjectError::Corrupt { address, .. }) if *address == expected.hex()),
        "{problem}"
    );
    let object = object_files(&store, &expected).pop().unwrap();
    assert_eq!(fs::read(&object).unwrap(), b"not what was written");

    // 재시도도 성공이 아니며 창고를 고치지 않는다.
    let problem = store_policy_source(&store, EngineVersion::V1, &expected, source).unwrap_err();
    assert!(matches!(problem, SourceStoreProblem::Store(ObjectError::Corrupt { .. })), "{problem}");
    assert_eq!(fs::read(&object).unwrap(), b"not what was written");
    assert_eq!(fs::read(root.join(".gilignore")).unwrap(), DISK_GILIGNORE);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn an_unusable_store_reports_its_own_error_and_keeps_the_policy_files() {
    // `.gil/artifacts` 자리에 폴더가 아닌 파일이 있다 — 창고를 열 수 없다.
    let (root, store) = project("unusable-store");
    fs::write(store.root(), b"not a directory").unwrap();
    let before = files(&root);
    let source: &[u8] = b"logs/\n";

    let problem = store_policy_source(&store, EngineVersion::V1, &address(source), source).unwrap_err();
    assert!(matches!(problem, SourceStoreProblem::Store(ObjectError::Io { .. })), "{problem}");
    assert!(!problem.refused_before_write());
    assert_eq!(files(&root), before, "창고 오류가 정책 파일·상태 자리·그 자리의 파일을 바꿨다");
    let _ = fs::remove_dir_all(&root);
}

// ── 성공의 뜻 ──────────────────────────────────────────────────────────────

#[test]
fn the_boundary_lists_what_success_does_not_show() {
    let text = SOURCE_STORE_DOES_NOT_SHOW.join("\n");
    for needle in ["승인", "적용", "state.yaml", "sync_base", ".gilignore", ".gitignore", "새로 썼는지", "잠금"] {
        assert!(text.contains(needle), "성공이 보이지 않는 것에 {needle} 가 없다");
    }
    // 오류 문구는 쓰기 전 거절과 창고 오류를 구분해 말한다.
    let mismatch = SourceStoreProblem::AddressMismatch { expected: address(b"a"), found: address(b"b") };
    assert!(mismatch.to_string().contains("창고에 쓰지 않았다"), "{mismatch}");
    let store = SourceStoreProblem::Store(ObjectError::Io { doing: "시험이", source: "합성".to_string() });
    assert!(store.to_string().contains("보관하지 못했다"), "{store}");
    let other = SourceStoreProblem::StoredAtOtherAddress { expected: address(b"a"), found: address(b"b") };
    assert!(other.to_string().contains("보관 성공으로 보지 않는다"), "{other}");
}
