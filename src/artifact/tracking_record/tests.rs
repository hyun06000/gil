//! 정책 기록 자료형과 부분 검증의 시험 — **합성 자료**만 쓴다.
//!
//! 여기의 정책 판·제안 참조·동기화 기준은 시험이 만든 값이다. 모니터가 실제로 승인한 기록이 아니며,
//! 실제 사용자 자료·`gil start`·`state.yaml` 을 쓰지 않는다. 창고를 쓰는 시험은 프로세스 번호가 붙은
//! 임시 폴더를 새로 만든다.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::*;
use crate::artifact::gilignore::ApprovedPolicy;
use crate::artifact::{ObjectStore, digest_from_raw, digest_of};

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

fn record(number: u32, rules: &str, seed: u8) -> PolicyRecord {
    PolicyRecord::new(name(number), EngineVersion::V1, address(rules.as_bytes()), proposal(seed))
}

fn ledger(records: Vec<PolicyRecord>) -> PolicyLedger {
    let next = records.len() as u32 + 1;
    PolicyLedger::from_records(next, records).expect("시험 원장은 꼴이 맞다")
}

fn stored(id: &str, engine: u32, algorithm: &str, digest: &str, proposal: &str) -> StoredPolicy {
    StoredPolicy {
        id: id.to_string(),
        engine,
        rules: StoredRules { algorithm: algorithm.to_string(), digest: digest.to_string() },
        proposal_digest: proposal.to_string(),
    }
}

fn hex_of(text: &str) -> String {
    address(text.as_bytes()).hex()
}

const A: &str = "target/\n";
const B: &str = "target/\nnode_modules/\n";

/// 시험만 쓰는 꼴 — 계약 §5.1 의 `tracking` 블록 가운데 이 모듈이 다루는 칸만 모은 **조각**이다.
/// 제품의 저장 꼴도, 독립 파일 형식도 아니다.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fragment {
    next_policy_id: u32,
    policies: Vec<StoredPolicy>,
    policy_file: StoredPolicyFile,
}

fn scratch(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gil-tracking-record-{}-{label}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("시험이 쓸 자리를 만들 수 있어야 한다");
    dir
}

/// 창고 안에서 그 지문의 객체 파일을 찾는다 — 창고의 배치 규칙을 다시 적지 않고 이름으로만 찾는다.
fn object_file(root: &Path, digest: &ContentDigest) -> PathBuf {
    fn walk(dir: &Path, hex: &str, found: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, hex, found);
            } else if let Some(file) = path.file_name().and_then(|name| name.to_str()) {
                if file.len() >= 32 && hex.ends_with(file) {
                    found.push(path);
                }
            }
        }
    }
    let mut found = Vec::new();
    walk(root, &digest.hex(), &mut found);
    assert_eq!(found.len(), 1, "객체 파일이 정확히 하나여야 한다: {found:?}");
    found.remove(0)
}

// ── 직렬화 왕복 ────────────────────────────────────────────────────────────

#[test]
fn a_well_formed_fragment_round_trips_with_every_value_kept() {
    let text = format!(
        "next_policy_id: 3\n\
         policies:\n\
         - id: P1\n  engine: 1\n  rules:\n    algorithm: sha256\n    digest: {a}\n  proposal_digest: {p1}\n\
         - id: P2\n  engine: 1\n  rules:\n    algorithm: sha256\n    digest: {b}\n  proposal_digest: {p2}\n\
         policy_file:\n  sync_base:\n    state: present\n    digest: {a}\n",
        a = hex_of(A),
        b = hex_of(B),
        p1 = proposal(0xa1).hex(),
        p2 = proposal(0xb2).hex(),
    );
    let fragment: Fragment = serde_norway::from_str(&text).expect("꼴이 맞는 조각이다");
    let ledger = PolicyLedger::from_stored(fragment.next_policy_id, fragment.policies).expect("원장의 꼴이 맞다");
    let base = fragment.policy_file.sync_base.into_sync_base().expect("기준의 꼴이 맞다");

    assert_eq!(ledger.next_policy_id(), 3);
    assert_eq!(ledger.policies(), &[record(1, A, 0xa1), record(2, B, 0xb2)]);
    assert_eq!(ledger.policies()[0].name().to_string(), "policy:P1");
    assert_eq!(base, SyncBase::Present(address(A.as_bytes())));

    let (next, policies) = ledger.to_stored();
    let again = Fragment {
        next_policy_id: next,
        policies,
        policy_file: StoredPolicyFile { sync_base: StoredSyncBase::of(&base) },
    };
    assert_eq!(serde_norway::to_string(&again).unwrap(), text, "왕복한 글자가 같아야 한다");

    // 숫자로만 된 지문은 쓰는 쪽이 따옴표로 감싼다. 따옴표가 있든 없든 같은 값으로 읽힌다.
    let digits = proposal(0x11).hex();
    let written = serde_norway::to_string(&StoredPolicy::of(&record(1, A, 0x11))).unwrap();
    assert!(written.contains(&format!("proposal_digest: '{digits}'")), "{written}");
    let read: StoredPolicy = serde_norway::from_str(&written).unwrap();
    assert_eq!(PolicyLedger::from_stored(2, vec![read]).unwrap().policies(), &[record(1, A, 0x11)]);
}

#[test]
fn each_sync_base_state_round_trips() {
    for base in [SyncBase::Present(address(B.as_bytes())), SyncBase::Absent, SyncBase::Unconfirmed] {
        let text = serde_norway::to_string(&StoredSyncBase::of(&base)).unwrap();
        let read: StoredSyncBase = serde_norway::from_str(&text).unwrap();
        assert_eq!(read.into_sync_base().unwrap(), base, "{text}");
    }
    // 지문이 없는 두 값에는 digest 칸 자체가 적히지 않는다.
    assert_eq!(serde_norway::to_string(&StoredSyncBase::of(&SyncBase::Absent)).unwrap(), "state: absent\n");
    assert_eq!(serde_norway::to_string(&StoredSyncBase::of(&SyncBase::Unconfirmed)).unwrap(), "state: unconfirmed\n");
}

#[test]
fn an_empty_ledger_is_the_not_yet_adopted_shape() {
    let empty = PolicyLedger::from_stored(1, Vec::new()).expect("정책을 채택하지 않은 원장");
    assert_eq!(empty, PolicyLedger::empty());
    assert_eq!(empty.to_stored(), (1, Vec::new()));
}

// ── 이름·순서·발급기 (T1 의 정책 부분) ─────────────────────────────────────

#[test]
fn malformed_policy_names_are_refused() {
    for bad in ["", "P0", "P01", "p1", "Q1", "policy:P1", "P 1", " P1", "P-1", "P+1", "P", "P1.0", "P٣"] {
        let err = PolicyLedger::from_stored(2, vec![stored(bad, 1, "sha256", &hex_of(A), &proposal(1).hex())])
            .expect_err(bad);
        assert!(matches!(err, LedgerShapeError::BadName { position: 0, .. }), "{bad:?}: {err:?}");
    }
}

#[test]
fn duplicates_gaps_reordering_and_a_wrong_issuer_are_refused() {
    let at = |n: u32| record(n, A, n as u8);
    let cases: [(u32, Vec<PolicyRecord>, LedgerShapeError); 6] = [
        (3, vec![at(1), at(1)], LedgerShapeError::DuplicateName { name: name(1), position: 1 }),
        (3, vec![at(1), at(3)], LedgerShapeError::OutOfSequence { found: name(3), expected: name(2), position: 1 }),
        (3, vec![at(2), at(1)], LedgerShapeError::OutOfSequence { found: name(2), expected: name(1), position: 0 }),
        (2, vec![at(2)], LedgerShapeError::OutOfSequence { found: name(2), expected: name(1), position: 0 }),
        (2, vec![at(1), at(2)], LedgerShapeError::WrongIssuer { found: 2, wanted: 3 }),
        (4, vec![at(1), at(2)], LedgerShapeError::WrongIssuer { found: 4, wanted: 3 }),
    ];
    for (next, records, wanted) in cases {
        assert_eq!(PolicyLedger::from_records(next, records), Err(wanted));
    }
    assert_eq!(
        PolicyLedger::from_records(0, Vec::new()),
        Err(LedgerShapeError::WrongIssuer { found: 0, wanted: 1 })
    );
}

#[test]
fn the_same_rules_under_different_versions_is_not_a_duplicate() {
    // 같은 원문을 다시 승인하면 같은 내용 주소의 **다른 판**이다. 제안 참조까지 같아도 판은 둘이다.
    let ledger = ledger(vec![record(1, A, 1), record(2, A, 2), record(3, A, 2)]);
    let sources = ProvidedSources::new().with(address(A.as_bytes()), A);
    let matched = check_policy_sources(&ledger, &sources).expect("세 판 모두 같은 원문이다");
    assert_eq!(matched.len(), 3);
    assert_eq!(matched.identity_of(name(1)), matched.identity_of(name(3)), "내용의 정체성은 같다");
    assert_ne!(ledger.policies()[0].name(), ledger.policies()[2].name(), "판의 이름은 다르다");

    // 같은 이름이 두 번이면 내용이 달라도 중복이다.
    assert_eq!(
        PolicyLedger::from_records(3, vec![record(1, A, 1), record(1, B, 2)]),
        Err(LedgerShapeError::DuplicateName { name: name(1), position: 1 })
    );
}

// ── 엔진·주소의 꼴 (T2 의 구조 부분) ───────────────────────────────────────

#[test]
fn unknown_engines_and_malformed_addresses_are_refused() {
    let good = hex_of(A);
    let proposal = proposal(1).hex();
    for engine in [0, 2, u32::MAX] {
        let err = PolicyLedger::from_stored(2, vec![stored("P1", engine, "sha256", &good, &proposal)]).unwrap_err();
        assert_eq!(err, LedgerShapeError::UnknownEngine { name: "P1".into(), found: engine });
    }
    for algorithm in ["sha512", "SHA256", "", "sha-256"] {
        let err = PolicyLedger::from_stored(2, vec![stored("P1", 1, algorithm, &good, &proposal)]).unwrap_err();
        assert!(matches!(err, LedgerShapeError::UnknownDigestAlgorithm { .. }), "{algorithm:?}: {err:?}");
    }
    let upper = good.to_uppercase();
    let short = &good[..63];
    let long = format!("{good}0");
    let spaced = format!(" {}", &good[1..]);
    let not_hex = format!("g{}", &good[1..]);
    for bad in [upper.as_str(), short, long.as_str(), spaced.as_str(), not_hex.as_str(), ""] {
        let err = PolicyLedger::from_stored(2, vec![stored("P1", 1, "sha256", bad, &proposal)]).unwrap_err();
        assert!(matches!(err, LedgerShapeError::DigestNotCanonical { field: "rules.digest", .. }), "{err:?}");
        let err = PolicyLedger::from_stored(2, vec![stored("P1", 1, "sha256", &good, bad)]).unwrap_err();
        assert!(matches!(err, LedgerShapeError::DigestNotCanonical { field: "proposal_digest", .. }), "{err:?}");
    }
}

#[test]
fn unknown_fields_missing_fields_and_explicit_nulls_never_pass_as_a_record() {
    let a = hex_of(A);
    let p = proposal(1).hex();
    let base = format!("id: P1\nengine: 1\nrules:\n  algorithm: sha256\n  digest: {a}\nproposal_digest: {p}\n");
    assert!(serde_norway::from_str::<StoredPolicy>(&base).is_ok(), "기준 꼴은 읽힌다");

    // 승인·적용 여부를 담은 칸은 기록에 없다 — 덧붙인 bool 은 모르는 칸으로 거절된다.
    for extra in ["approved: true\n", "applied: true\n", "monitor_approved: true\n", "source: gitignore\n"] {
        let text = format!("{base}{extra}");
        assert!(serde_norway::from_str::<StoredPolicy>(&text).is_err(), "{extra}");
    }
    let rules_extra = base.replace("  algorithm: sha256\n", "  algorithm: sha256\n  verified: true\n");
    assert!(serde_norway::from_str::<StoredPolicy>(&rules_extra).is_err());

    let without = |field: &str| -> String {
        base.lines().filter(|line| !line.starts_with(field)).map(|line| format!("{line}\n")).collect()
    };
    for missing in [without("id:"), without("engine:"), without("proposal_digest:")] {
        assert!(serde_norway::from_str::<StoredPolicy>(&missing).is_err(), "{missing}");
    }
    let no_rules = format!("id: P1\nengine: 1\nproposal_digest: {p}\n");
    assert!(serde_norway::from_str::<StoredPolicy>(&no_rules).is_err());

    // 끝까지 읽어 본다: serde 가 거절하거나, 받은 글자를 원장의 꼴 검사가 거절해야 한다.
    // (serde_norway 는 문자열 칸의 평문 `null`·`~` 을 글자 "null"·"~" 로 읽는다. 그 글자는 이름도 지문도
    // 아니므로 꼴 검사가 거절한다 — 「없음」으로 바뀌어 지나가지 않는다.)
    let refused = |text: &str| -> bool {
        match serde_norway::from_str::<StoredPolicy>(text) {
            Err(_) => true,
            Ok(policy) => PolicyLedger::from_stored(2, vec![policy]).is_err(),
        }
    };
    for nulled in [
        base.replace(&format!("proposal_digest: {p}"), "proposal_digest: null"),
        base.replace(&format!("proposal_digest: {p}"), "proposal_digest: ~"),
        base.replace(&format!("proposal_digest: {p}"), "proposal_digest: ''"),
        base.replace("engine: 1", "engine: null"),
        base.replace("id: P1", "id: null"),
        base.replace("id: P1", "id: ~"),
        format!("id: P1\nengine: 1\nrules: null\nproposal_digest: {p}\n"),
        base.replace(&format!("  digest: {a}"), "  digest: null"),
        base.replace("  algorithm: sha256", "  algorithm: null"),
        base.replace("engine: 1", "engine: \"1\""),
        base.replace("engine: 1", "engine: 1.0"),
        base.replace("engine: 1", "engine: -1"),
    ] {
        assert!(refused(&nulled), "받아들였다: {nulled}");
    }
}

#[test]
fn sync_base_refuses_impossible_combinations() {
    let a = hex_of(A);
    let shape = |text: &str| -> Result<SyncBase, String> {
        let stored: StoredSyncBase = serde_norway::from_str(text).map_err(|err| err.to_string())?;
        stored.into_sync_base().map_err(|err| err.to_string())
    };
    assert_eq!(shape(&format!("state: present\ndigest: {a}\n")), Ok(SyncBase::Present(address(A.as_bytes()))));
    assert_eq!(shape("state: absent\n"), Ok(SyncBase::Absent));
    assert_eq!(shape("state: unconfirmed\n"), Ok(SyncBase::Unconfirmed));

    let refused = [
        "state: present\n".to_string(),
        "state: present\ndigest: null\n".to_string(),
        format!("state: present\ndigest: {}\n", a.to_uppercase()),
        format!("state: absent\ndigest: {a}\n"),
        "state: absent\ndigest: null\n".to_string(),
        format!("state: unconfirmed\ndigest: {a}\n"),
        "state: unconfirmed\ndigest: null\n".to_string(),
        "state: Present\n".to_string(),
        "state: stale\n".to_string(),
        "state: null\n".to_string(),
        "digest: null\n".to_string(),
        format!("state: present\ndigest: {a}\nconfirmed_at: now\n"),
    ];
    for text in &refused {
        assert!(shape(text).is_err(), "받아들였다: {text}");
    }
    let typed = |text: &str| serde_norway::from_str::<StoredSyncBase>(text).unwrap().into_sync_base();
    assert_eq!(typed("state: present\n"), Err(SyncBaseShapeError::MissingDigest));
    assert_eq!(typed("state: present\ndigest: null\n"), Err(SyncBaseShapeError::MissingDigest));
    assert_eq!(typed("state: absent\ndigest: null\n"), Err(SyncBaseShapeError::UnexpectedDigest { state: "absent" }));

    // `policy_file` 자리: 기준이 없거나 null 이면 꼴이 아니다.
    assert!(serde_norway::from_str::<StoredPolicyFile>("sync_base: null\n").is_err());
    assert!(serde_norway::from_str::<StoredPolicyFile>("{}\n").is_err());
    assert!(serde_norway::from_str::<StoredPolicyFile>("sync_base:\n  state: absent\nsynced: true\n").is_err());
}

// ── 원문의 실재·주소·해석 (T2 의 나머지) ───────────────────────────────────

/// 조회가 실패하는 경계 — 읽기 오류가 「없음」으로 바뀌지 않는지 본다.
struct Failing;

impl PolicySourceLookup for Failing {
    fn rules_source(&self, _: &ContentDigest) -> SourceLookup {
        SourceLookup::Failed { reason: "합성 읽기 오류".into() }
    }
}

#[test]
fn matching_sources_give_the_engine_identity_and_nothing_more() {
    let ledger = ledger(vec![record(1, A, 1), record(2, B, 2)]);
    let sources = ProvidedSources::new().with(address(A.as_bytes()), A).with(address(B.as_bytes()), B);
    let matched = check_policy_sources(&ledger, &sources).expect("두 원문이 주소와 맞고 해석된다");
    let engine = ApprovedPolicy::compile(EngineVersion::V1, B.as_bytes()).unwrap();
    assert_eq!(matched.identity_of(name(2)), Some(engine.identity()), "#174 의 정체성을 그대로 쓴다");
    assert_eq!(matched.identity_of(name(3)), None);

    // 이 결과가 보인 것은 T1 의 정책 부분과 T2 뿐이다. 사건·세계·사이클·중단·파일 쓰기는 보이지 않았다.
    assert_eq!(PolicySourcesMatch::CHECKED, [Condition::T1PolicyNames, Condition::T2PolicySources]);
    for condition in ["T1 사건", "T3", "T4", "T5", "T6", "T7", "T8", "T9", "T10", "T11", "T12", "T13"] {
        assert!(
            UNCHECKED_CONDITIONS.iter().any(|said| said.starts_with(condition)),
            "{condition} 을 검사하지 않았다고 적어야 한다"
        );
    }
}

#[test]
fn missing_failed_mismatched_and_unreadable_sources_are_all_reported() {
    let bad_rules = "src\\gen\n";
    let ledger = ledger(vec![record(1, A, 1), record(2, B, 2), record(3, bad_rules, 3), record(4, "x/\n", 4)]);
    // P1: 없음. P2: 다른 바이트(주소 불일치). P3: 주소는 맞으나 해석 불가. P4: 정상.
    let sources = ProvidedSources::new()
        .with(address(B.as_bytes()), "node_modules/\n")
        .with(address(bad_rules.as_bytes()), bad_rules)
        .with(address(b"x/\n"), "x/\n");
    let problems = check_policy_sources(&ledger, &sources).expect_err("문제가 있는 원장이 통과했다");
    assert_eq!(problems.len(), 3, "{problems:?}");
    assert_eq!(problems[0], SourceProblem { name: name(1), kind: SourceProblemKind::Missing });
    assert_eq!(
        problems[1],
        SourceProblem { name: name(2), kind: SourceProblemKind::AddressMismatch { found: address(b"node_modules/\n") } }
    );
    assert!(
        matches!(&problems[2], SourceProblem { name: n, kind: SourceProblemKind::Rejected(rejected) }
            if *n == name(3) && rejected.identity.rules_digest() == &address(bad_rules.as_bytes())),
        "{:?}",
        problems[2]
    );

    // 조회 실패는 실패로 남는다 — 없음으로 바뀌지 않는다.
    let problems = check_policy_sources(&ledger, &Failing).unwrap_err();
    assert_eq!(problems.len(), 4);
    assert!(problems.iter().all(|problem| matches!(problem.kind, SourceProblemKind::LookupFailed { .. })));

    // 원문을 하나도 받지 못했으면 판마다 없음이다. 비어 있는 원장만 원문 없이 지나간다.
    assert_eq!(check_policy_sources(&ledger, &ProvidedSources::new()).unwrap_err().len(), 4);
    assert_eq!(check_policy_sources(&PolicyLedger::empty(), &ProvidedSources::new()).unwrap().len(), 0);
}

#[test]
fn the_object_store_is_read_only_and_tells_missing_from_corrupt() {
    let root = scratch("store");
    let store = ObjectStore::at(&root.join(".gil"));
    // 시험 준비로 합성 원문을 창고에 넣는다 — 이 모듈에는 창고에 쓰는 기능이 없다.
    let a = store.put_blob(A.as_bytes()).unwrap();
    let b = store.put_blob(B.as_bytes()).unwrap();
    assert_eq!(a, address(A.as_bytes()), "창고 주소 = 원문 sha256 = rules_digest");

    let ledger = ledger(vec![record(1, A, 1), record(2, B, 2), record(3, "missing/\n", 3)]);
    let problems = check_policy_sources(&ledger, &store).unwrap_err();
    assert_eq!(problems, vec![SourceProblem { name: name(3), kind: SourceProblemKind::Missing }]);

    let at = object_file(store.root(), &b);
    fs::write(&at, b"tampered").unwrap();
    let ledger = self::ledger(vec![record(1, A, 1), record(2, B, 2)]);
    let problems = check_policy_sources(&ledger, &store).unwrap_err();
    assert_eq!(problems.len(), 1);
    assert!(
        matches!(&problems[0], SourceProblem { name: n, kind: SourceProblemKind::LookupFailed { .. } } if *n == name(2)),
        "손상은 조회 실패다: {:?}",
        problems[0]
    );
    assert_eq!(fs::read(&at).unwrap(), b"tampered", "검사가 창고를 고쳐 쓰지 않았다");
    let _ = fs::remove_dir_all(&root);
}

// ── 추가 전후 비교 ─────────────────────────────────────────────────────────

#[test]
fn appending_is_the_only_accepted_change() {
    let before = ledger(vec![record(1, A, 1), record(2, B, 2)]);
    assert_eq!(check_append_only(&before, &before), Ok(0));
    assert_eq!(check_append_only(&before, &ledger(vec![record(1, A, 1), record(2, B, 2), record(3, A, 3)])), Ok(1));
    assert_eq!(check_append_only(&PolicyLedger::empty(), &before), Ok(2));

    let cases = [
        (vec![record(1, A, 1)], AppendViolation::Removed { name: name(2) }),
        (vec![], AppendViolation::Removed { name: name(1) }),
        (vec![record(1, B, 1), record(2, B, 2)], AppendViolation::Changed { name: name(1), fields: vec!["rules"] }),
        (vec![record(1, A, 9), record(2, B, 2)], AppendViolation::Changed { name: name(1), fields: vec!["proposal_digest"] }),
        // 순서 바꾸기: 이름은 자리에 묶이므로 앞 자리의 내용이 바뀐 것으로 걸린다.
        (vec![record(1, B, 2), record(2, A, 1)], AppendViolation::Changed { name: name(1), fields: vec!["rules", "proposal_digest"] }),
        // 끝을 지우고 새 판을 붙여도 지운 판의 자리가 바뀐 것이다.
        (vec![record(1, A, 1), record(2, A, 3)], AppendViolation::Changed { name: name(2), fields: vec!["rules", "proposal_digest"] }),
    ];
    for (records, wanted) in cases {
        assert_eq!(check_append_only(&before, &ledger(records)), Err(wanted));
    }
}

// ── §5.2 판정표 ────────────────────────────────────────────────────────────

#[test]
fn the_policy_file_table_matches_the_contract() {
    use PolicyFileReading as R;
    let applied = address(B.as_bytes());
    let old = address(A.as_bytes());
    let other = address(b"edited by hand\n");
    let unreadable = DiskReading::Unreadable { reason: "합성 읽기 오류".into() };

    let bases = [SyncBase::Present(old.clone()), SyncBase::Present(applied.clone()), SyncBase::Absent, SyncBase::Unconfirmed];
    for base in &bases {
        // 적용 원문과 같으면 기준이 무엇이든 동기화됨.
        assert_eq!(classify_policy_file(&applied, base, &DiskReading::Present(applied.clone())), R::MatchesApplied);
        // 읽지 못하면 무엇이든 판정 불가 — 없음으로도 동기화됨으로도 보지 않는다.
        assert!(matches!(classify_policy_file(&applied, base, &unreadable), R::Unreadable { .. }));
    }

    let rows = [
        // 기준과 같은 구판 — 갱신 필요(쓰기 후보).
        (SyncBase::Present(old.clone()), DiskReading::Present(old.clone()), R::StaleMatchesBase),
        (SyncBase::Absent, DiskReading::Absent, R::StaleMatchesBase),
        // 기준과도 다르다 — 사용자 편집·삭제.
        (SyncBase::Present(old.clone()), DiskReading::Present(other.clone()), R::UserChanged { deleted: false }),
        (SyncBase::Present(old.clone()), DiskReading::Absent, R::UserChanged { deleted: true }),
        (SyncBase::Absent, DiskReading::Present(other.clone()), R::UserChanged { deleted: false }),
        // Core 가 B 를 쓰고 기준을 B 로 옮긴 뒤 사용자가 A 로 되돌렸다 — 사용자 편집이다. 이전 기준이 남아 덮어쓰지 않는다.
        (SyncBase::Present(applied.clone()), DiskReading::Present(old.clone()), R::UserChanged { deleted: false }),
        (SyncBase::Present(applied.clone()), DiskReading::Absent, R::UserChanged { deleted: true }),
        // 쓰기 도중 중단 — 원인 미확인. 디스크가 무엇이든(적용 원문 제외) 자동으로 쓰지 않는다.
        (SyncBase::Unconfirmed, DiskReading::Present(old.clone()), R::BaseUnconfirmed),
        (SyncBase::Unconfirmed, DiskReading::Present(other.clone()), R::BaseUnconfirmed),
        (SyncBase::Unconfirmed, DiskReading::Absent, R::BaseUnconfirmed),
    ];
    for (base, disk, wanted) in rows {
        assert_eq!(classify_policy_file(&applied, &base, &disk), wanted, "{base:?} / {disk:?}");
    }
}

#[test]
fn an_unreadable_file_is_never_an_absent_one() {
    assert_eq!(DiskReading::from_read(Err(io::ErrorKind::NotFound.into())), DiskReading::Absent);
    for kind in [io::ErrorKind::PermissionDenied, io::ErrorKind::IsADirectory, io::ErrorKind::InvalidData, io::ErrorKind::Other] {
        assert!(matches!(DiskReading::from_read(Err(kind.into())), DiskReading::Unreadable { .. }), "{kind:?}");
    }
    assert_eq!(DiskReading::from_read(Ok(B.as_bytes().to_vec())), DiskReading::Present(address(B.as_bytes())));
    // 빈 파일은 있는 파일이다.
    assert_eq!(DiskReading::from_read(Ok(Vec::new())), DiskReading::Present(address(b"")));

    // 실제 파일 시스템: 정책 파일 자리에 디렉터리가 있으면 읽지 못함이지 없음이 아니다.
    let root = scratch("unreadable");
    fs::create_dir(root.join(".gilignore")).unwrap();
    let reading = DiskReading::from_read(fs::read(root.join(".gilignore")));
    assert!(matches!(reading, DiskReading::Unreadable { .. }), "{reading:?}");
    assert!(matches!(
        classify_policy_file(&address(B.as_bytes()), &SyncBase::Absent, &reading),
        PolicyFileReading::Unreadable { .. }
    ));
    assert_eq!(DiskReading::from_read(fs::read(root.join("nothing-here"))), DiskReading::Absent);
    let _ = fs::remove_dir_all(&root);
}

// ── 이름이 승인·적용·완전 검증을 말하지 않는다 ─────────────────────────────

#[test]
fn no_item_here_is_named_as_an_approval_an_application_or_a_full_check() {
    let sources = [
        ("mod.rs", include_str!("mod.rs")),
        ("event_links.rs", include_str!("event_links.rs")),
        ("source.rs", include_str!("source.rs")),
        ("sync.rs", include_str!("sync.rs")),
        ("sync_order.rs", include_str!("sync_order.rs")),
        ("wire.rs", include_str!("wire.rs")),
    ];
    let forbidden = ["approv", "applicable", "apply", "applied", "verified", "complete", "valid"];
    for (file, text) in sources {
        for line in text.lines() {
            let line = line.trim_start();
            let Some(rest) = ["pub(crate) fn ", "pub(crate) struct ", "pub(crate) enum ", "pub(crate) const ", "pub(crate) trait "]
                .iter()
                .find_map(|head| line.strip_prefix(head))
            else {
                continue;
            };
            let item: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            let lower = item.to_lowercase();
            assert!(
                !forbidden.iter().any(|word| lower.contains(word)),
                "{file}: `{item}` 은 승인·적용·완전 검증으로 읽힐 수 있다"
            );
        }
    }
}
