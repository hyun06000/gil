//! 시작 준비·Monitor 승인 응답·정식 시작 확정의 시험 — **합성 임시 폴더**만 쓴다(실제 사용자 프로젝트·소스 저장소의
//! `gil start` 없음). Monitor 버튼은 capability 검사를 마친 뒤 부르는 [`decide`] 로 흉내 낸다. capability 자체의
//! 경계(app-only·메모리·모델 비노출)는 `src/mcp/start_tests.rs` 가 본다.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::*;

fn scratch(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gil-start-prep-{}-{label}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("시험 폴더");
    dir.canonicalize().expect("실제 경로")
}

fn s(root: &Path) -> String {
    root.display().to_string()
}

/// 작은 합성 프로젝트: 추적 파일 둘, 의존성 폴더 하나(완전히 제외될 자리), 비밀값 후보 하나.
fn project(label: &str) -> PathBuf {
    let root = scratch(label);
    fs::write(root.join("README.md"), "hello\n").unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/main.py"), "print(1)\n").unwrap();
    fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
    fs::write(root.join("node_modules/pkg/index.js"), "x\n").unwrap();
    fs::write(root.join(".env"), "TOKEN=secret-value\n").unwrap();
    root
}

const RULES_B: &str = "node_modules/\n.env\n";

fn prepared(root: &Path, rules: &str) -> Value {
    prepare(&s(root), PrepareInput { rules: rules.into(), project_recommendations: vec![], combine_with_revision: None, restart_canceled: false })
        .expect("제안")
}

fn proposal_of(v: &Value) -> (u32, String) {
    (v["proposal"]["revision"].as_u64().unwrap() as u32, v["proposal"]["digest"].as_str().unwrap().to_owned())
}

fn state(root: &Path) -> PathBuf {
    root.join(".gil/state.yaml")
}

fn open(root: &Path) -> crate::ProjectSession {
    crate::ProjectSession::open(crate::RuleSet::builtin().unwrap(), state(root)).expect("형식 10 기록이 load 검사를 통과한다")
}

fn manifest_paths(root: &Path) -> Vec<String> {
    let session = open(root);
    let project = session.project();
    let address = project.world_manifest(project.world_snapshot()).unwrap().clone();
    let store = crate::artifact::ObjectStore::at(&root.join(".gil"));
    store.read_manifest(&address).unwrap().paths().map(|p| p.as_str().to_owned()).collect()
}

fn objects_exist(root: &Path) -> bool {
    root.join(".gil/artifacts").exists()
}

fn approve(root: &Path, v: &Value) -> Result<Value, Refusal> {
    let (revision, digest) = proposal_of(v);
    decide(root, revision, &digest, true)
}

// ── 정상 시작 ──────────────────────────────────────────────────────────────

#[test]
fn preparation_archives_nothing_and_the_approved_policy_is_the_initial_snapshot_and_first_entry() {
    let root = project("normal");
    let begun = begin(&s(&root)).unwrap();
    assert_eq!(begun["phase"], "preparing");
    // 일반·비밀값 추천은 미리 선택, 빌드 출력 후보는 미선택. `.gitignore` 없음 → 카탈로그만.
    let recs = begun["recommendations"].as_array().unwrap();
    assert!(recs.iter().any(|r| r["key"] == "gil:dependencies-node" || r["rules"].as_array().unwrap().iter().any(|x| x == "node_modules/")));
    assert!(recs.iter().filter(|r| r["category"] == "build_output").all(|r| r["default_selected"] == false));
    assert!(recs.iter().filter(|r| r["category"] == "secrets").all(|r| r["default_selected"] == true));
    // 승인 전: 상태 파일·Chain·Snapshot·객체 없음.
    assert!(!state(&root).exists());
    assert!(!objects_exist(&root));

    let p = prepared(&root, RULES_B);
    assert_eq!(p["phase"], "awaiting_approval");
    let tracked: Vec<&str> = p["proposal"]["tracked"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    assert!(tracked.contains(&"README.md") && tracked.contains(&"src/main.py"));
    assert!(!tracked.iter().any(|t| t.starts_with("node_modules") || *t == ".env" || *t == ".gilignore"));
    let nm = p["proposal"]["excluded"].as_array().unwrap().iter().find(|e| e["path"] == "node_modules").unwrap();
    assert_eq!(nm["interior"], "not_explored", "완전히 제외된 폴더는 들여다보지 않는다");
    assert_eq!(p["proposal"]["policy_file"]["effect"], "create");
    assert!(!state(&root).exists() && !objects_exist(&root), "제안도 아무것도 보관하지 않는다");
    assert!(!root.join(".gilignore").exists(), "승인 전에는 .gilignore 를 쓰지 않는다");

    let started = approve(&root, &p).unwrap();
    assert_eq!(started["phase"], "started", "{started}");
    assert_eq!(started["started"]["snapshot"], "snapshot:A1");
    assert_eq!(started["started"]["policy_file_sync_base"], "present");
    assert_eq!(fs::read(root.join(".gilignore")).unwrap(), RULES_B.as_bytes(), "승인 원문 바이트 그대로");

    let text = fs::read_to_string(state(&root)).unwrap();
    assert!(text.starts_with("format: 10\n"), "{text}");
    assert!(text.contains("kind: start") && text.contains("snapshot_ref: snapshot:A1") && text.contains(&format!("proposal_digest: {}", proposal_of(&p).1)));
    let session = open(&root);
    let chain = session.project().current_chain().unwrap();
    assert_eq!(chain.entry_snapshot().to_string(), "snapshot:A1");
    assert_eq!(session.project().tracking().unwrap().start().snapshot.to_string(), "snapshot:A1");
    drop(session);
    let paths = manifest_paths(&root);
    assert!(paths.contains(&"README.md".to_owned()));
    assert!(!paths.iter().any(|p| p.starts_with("node_modules") || p == ".env" || p == ".gilignore"), "{paths:?}");
}

#[test]
fn a_repeated_or_lost_response_returns_the_same_start_and_issues_nothing_twice() {
    let root = project("repeat");
    begin(&s(&root)).unwrap();
    let p = prepared(&root, RULES_B);
    let first = approve(&root, &p).unwrap();
    let bytes = fs::read(state(&root)).unwrap();
    // 같은 버튼 응답의 재전송, 기록된 승인의 적용 재시도, 조회 — 같은 결과, 상태 바이트 그대로.
    let again = approve(&root, &p).unwrap();
    let applied = apply(&root, proposal_of(&p).0, &proposal_of(&p).1).unwrap();
    assert_eq!(again["started"], first["started"]);
    assert_eq!(applied["started"], first["started"]);
    assert_eq!(fs::read(state(&root)).unwrap(), bytes);
    assert_eq!(status(&s(&root)).unwrap()["phase"], "started");
    // 다른 결정은 거절, 다시 begin 은 기존 프로젝트로 안내.
    assert_eq!(decide(&root, proposal_of(&p).0, &proposal_of(&p).1, false).unwrap_err().code, "response_final");
    let refused = begin(&s(&root)).unwrap_err();
    assert_eq!(refused.code, "already_started", "{}", refused.said);
}

// ── 좁은 결합 예외 ─────────────────────────────────────────────────────────

#[test]
fn absent_base_then_disk_c_needs_an_announced_combination_d_with_its_own_approval() {
    let root = project("combine");
    begin(&s(&root)).unwrap(); // 기준 absent
    let b = prepared(&root, RULES_B);
    // 승인 저장 직전에 사용자가 다른 .gilignore C 를 만든다.
    let c = "node_modules/\n*.log\n";
    fs::write(root.join(".gilignore"), c).unwrap();
    let held = approve(&root, &b).unwrap();
    assert_eq!(held["phase"], "approved");
    assert_eq!(held["application_held"]["code"], "combine_required", "{held}");
    assert_eq!(held["application_held"]["detail"]["announce_once"], true);
    assert_eq!(fs::read_to_string(root.join(".gilignore")).unwrap(), c, "승인 전 디스크 불변");
    assert!(!state(&root).exists());
    // 같은 적용을 다시 해도 안내는 한 번만 기록된다.
    let again = apply(&root, proposal_of(&b).0, &proposal_of(&b).1).unwrap_err();
    assert_eq!(again.code, "combine_required");
    assert_eq!(again.detail["announce_once"], false);

    // 결합안 D — 마지막 일치 규칙 순서를 지킨 에이전트의 원문.
    let d = "node_modules/\n.env\n*.log\n";
    let dv = prepare(&s(&root), PrepareInput { rules: d.into(), project_recommendations: vec![], combine_with_revision: Some(proposal_of(&b).0), restart_canceled: false }).unwrap();
    let comb = &dv["proposal"]["combination"];
    assert_eq!(comb["approved_rules"], RULES_B);
    assert_eq!(comb["disk_rules"], c);
    assert!(comb["diff_vs_approved"].as_array().unwrap().iter().any(|l| l == "+ *.log"));
    assert!(comb["diff_vs_disk"].as_array().unwrap().iter().any(|l| l == "+ .env"));
    // B 의 승인을 D 에 쓰지 않는다: B 는 대체됐다.
    assert_eq!(apply(&root, proposal_of(&b).0, &proposal_of(&b).1).unwrap_err().code, "superseded");
    assert_eq!(decide(&root, proposal_of(&dv).0, &proposal_of(&b).1, true).unwrap_err().code, "stale_proposal");
    assert_eq!(fs::read_to_string(root.join(".gilignore")).unwrap(), c, "D 작성만으로 디스크를 바꾸지 않는다");

    let started = approve(&root, &dv).unwrap();
    assert_eq!(started["phase"], "started", "{started}");
    assert_eq!(fs::read_to_string(root.join(".gilignore")).unwrap(), d, "승인된 D 의 바이트로 교체");
    assert_eq!(started["started"]["policy_file_sync_base"], "present");
}

#[test]
fn a_disk_changed_after_the_combined_approval_is_never_overwritten() {
    let root = project("combine-after");
    begin(&s(&root)).unwrap();
    let b = prepared(&root, RULES_B);
    fs::write(root.join(".gilignore"), "node_modules/\n").unwrap();
    assert_eq!(approve(&root, &b).unwrap()["application_held"]["code"], "combine_required");
    let dv = prepare(&s(&root), PrepareInput { rules: "node_modules/\n.env\n# merged\n".into(), project_recommendations: vec![], combine_with_revision: Some(proposal_of(&b).0), restart_canceled: false }).unwrap();
    // D 가 보여 준 C 와 다른 C' 로 사용자가 다시 바꾼다.
    fs::write(root.join(".gilignore"), "node_modules/\n# edited again\n").unwrap();
    let held = approve(&root, &dv).unwrap();
    assert_eq!(held["application_held"]["code"], "policy_file_changed", "{held}");
    assert_eq!(held["phase"], "condition_changed", "이 판은 다시 적용하지 않는다");
    assert_eq!(fs::read_to_string(root.join(".gilignore")).unwrap(), "node_modules/\n# edited again\n");
    assert!(!state(&root).exists());
}

#[test]
fn three_different_contents_stay_held_and_are_not_resolved_automatically() {
    let root = project("held");
    fs::write(root.join(".gilignore"), "node_modules/\n").unwrap(); // 처음 확인한 A
    begin(&s(&root)).unwrap();
    let b = prepared(&root, RULES_B);
    fs::write(root.join(".gilignore"), "*.tmp\n").unwrap(); // C
    let held = approve(&root, &b).unwrap();
    assert_eq!(held["application_held"]["code"], "held_three_way", "{held}");
    assert_eq!(fs::read_to_string(root.join(".gilignore")).unwrap(), "*.tmp\n");
    assert!(!state(&root).exists());
    // 결합 예외는 기준 absent 에만 — 여기서는 거절.
    let refused = prepare(&s(&root), PrepareInput { rules: "x\n".into(), project_recommendations: vec![], combine_with_revision: Some(proposal_of(&b).0), restart_canceled: false }).unwrap_err();
    assert_eq!(refused.code, "combination_not_allowed");
}

// ── 재확인: 규칙·목록 변화 vs 내용만 변화 ──────────────────────────────────

#[test]
fn a_changed_tracked_list_needs_reapproval_but_content_only_and_excluded_interior_changes_do_not() {
    let root = project("recheck");
    begin(&s(&root)).unwrap();
    let p = prepared(&root, RULES_B);
    fs::write(root.join("NEW.txt"), "new\n").unwrap();
    let held = approve(&root, &p).unwrap();
    assert_eq!(held["application_held"]["code"], "scope_changed");
    assert_eq!(held["application_held"]["detail"]["recorded"], true);
    assert_eq!(held["application_held"]["detail"]["detail"]["added"][0], "NEW.txt");
    fs::remove_file(root.join("NEW.txt")).unwrap();
    assert_eq!(apply(&root, proposal_of(&p).0, &proposal_of(&p).1).unwrap_err().code, "condition_changed",
        "목록을 되돌려도 같은 판의 승인을 다시 쓰지 않는다 — 새 제안·재승인이 필요하다");
    fs::write(root.join("NEW.txt"), "new\n").unwrap();
    assert!(!root.join(".gilignore").exists(), "보류된 적용은 정책 파일도 쓰지 않는다");
    assert!(!state(&root).exists() && !objects_exist(&root) || !state(&root).exists());

    let p2 = prepared(&root, RULES_B);
    // 내용만 바뀐 추적 파일, 완전히 제외된 폴더 안의 새 파일 — 재승인 없이 안정 관측으로 시작한다.
    fs::write(root.join("README.md"), "changed content\n").unwrap();
    fs::write(root.join("node_modules/pkg/extra.js"), "y\n").unwrap();
    let started = approve(&root, &p2).unwrap();
    assert_eq!(started["phase"], "started", "{started}");
    let session = open(&root);
    let project = session.project();
    let address = project.world_manifest(project.world_snapshot()).unwrap().clone();
    let store = crate::artifact::ObjectStore::at(&root.join(".gil"));
    let manifest = store.read_manifest(&address).unwrap();
    let readme = manifest.get(&crate::artifact::EntryPath::parse("README.md").unwrap()).unwrap();
    assert_eq!(*readme, crate::artifact::digest_of(&b"changed content\n"[..]).unwrap(), "보관 시점의 내용");
}

#[test]
fn a_nested_policy_file_holds_the_start_and_cannot_be_approved() {
    let root = project("nested");
    fs::write(root.join("src/.gilignore"), "*.py\n").unwrap();
    begin(&s(&root)).unwrap();
    let p = prepared(&root, RULES_B);
    assert_eq!(p["phase"], "blocked");
    assert_eq!(p["proposal"]["approvable"], false);
    assert_eq!(approve(&root, &p).unwrap_err().code, "blocked");
    assert!(!state(&root).exists());
}

#[cfg(unix)]
#[test]
fn a_tracked_link_is_refused_and_an_excluded_link_is_not_followed() {
    let root = project("links");
    let outside = scratch("links-outside");
    fs::write(outside.join("secret.txt"), "outside\n").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("node_modules/linked")).unwrap();
    begin(&s(&root)).unwrap();
    let p = prepared(&root, RULES_B);
    let started = approve(&root, &p).unwrap();
    assert_eq!(started["phase"], "started", "제외 폴더 안의 연결은 따라가지 않고 건너뛴다: {started}");
    let other = project("links-tracked");
    std::os::unix::fs::symlink(&outside, other.join("src/linked")).unwrap();
    begin(&s(&other)).unwrap();
    let refused = prepare(&s(&other), PrepareInput { rules: RULES_B.into(), project_recommendations: vec![], combine_with_revision: None, restart_canceled: false }).unwrap_err();
    assert_eq!(refused.code, "scope_unobservable");
    assert!(!state(&other).exists());
}

// ── 위조·오래된 응답, 승인 없는 적용 ───────────────────────────────────────

#[test]
fn stale_forged_or_missing_approvals_change_nothing() {
    let root = project("forged");
    begin(&s(&root)).unwrap();
    let p1 = prepared(&root, RULES_B);
    let p2 = prepared(&root, "node_modules/\n");
    assert_eq!(decide(&root, proposal_of(&p1).0, &proposal_of(&p1).1, true).unwrap_err().code, "stale_proposal");
    assert_eq!(decide(&root, proposal_of(&p2).0, &"0".repeat(64), true).unwrap_err().code, "stale_proposal");
    assert_eq!(apply(&root, proposal_of(&p2).0, &proposal_of(&p2).1).unwrap_err().code, "not_approved");
    assert_eq!(apply(&root, 1, &"f".repeat(64)).unwrap_err().code, "unknown_proposal");
    assert!(!state(&root).exists());
    // 다른 폴더로 복사한 준비 기록은 쓰지 않는다.
    let copy = scratch("forged-copy");
    fs::create_dir_all(copy.join(".gil/start")).unwrap();
    fs::copy(root.join(".gil/start/preparation.json"), copy.join(".gil/start/preparation.json")).unwrap();
    assert_eq!(status(&s(&copy)).unwrap_err().code, "preparation_other_folder");
}

// ── 취소·재시작 ────────────────────────────────────────────────────────────

#[test]
fn cancel_keeps_records_never_autostarts_and_restart_needs_a_new_approval() {
    let root = project("cancel");
    begin(&s(&root)).unwrap();
    let p = prepared(&root, RULES_B);
    let canceled = cancel(&s(&root)).unwrap();
    assert_eq!(canceled["phase"], "canceled");
    // 조회·재연결(begin)·이전 요청의 재전송은 시작하지 않는다.
    assert_eq!(begin(&s(&root)).unwrap()["phase"], "canceled");
    assert_eq!(status(&s(&root)).unwrap()["phase"], "canceled");
    assert_eq!(approve(&root, &p).unwrap_err().code, "canceled");
    assert_eq!(apply(&root, proposal_of(&p).0, &proposal_of(&p).1).unwrap_err().code, "canceled");
    let ledger: Value = serde_json::from_slice(&fs::read(root.join(".gil/start/preparation.json")).unwrap()).unwrap();
    assert_eq!(ledger["proposals"].as_array().unwrap().len(), 1, "취소는 기록을 지우지 않는다");
    assert!(!state(&root).exists());
    // 명시적 재시작 의사가 있어야 하고, 앞 시기의 제안·승인을 쓰지 않는다.
    assert_eq!(prepare(&s(&root), PrepareInput { rules: RULES_B.into(), project_recommendations: vec![], combine_with_revision: None, restart_canceled: false }).unwrap_err().code, "canceled");
    let r = prepare(&s(&root), PrepareInput { rules: RULES_B.into(), project_recommendations: vec![], combine_with_revision: None, restart_canceled: true }).unwrap();
    assert_eq!(apply(&root, proposal_of(&p).0, &proposal_of(&p).1).unwrap_err().code, "superseded");
    assert_eq!(approve(&root, &r).unwrap()["phase"], "started");
}

#[test]
fn a_disconnect_is_not_cancel_the_wait_survives_a_fresh_reader() {
    let root = project("disconnect");
    begin(&s(&root)).unwrap();
    let p = prepared(&root, RULES_B);
    // 「앱 종료·연결 끊김」은 아무 기록도 남기지 않는다 — 새 프로세스가 읽어도 승인 대기 그대로다.
    let read = status(&s(&root)).unwrap();
    assert_eq!(read["phase"], "awaiting_approval");
    assert_eq!(read["proposal_digest"], proposal_of(&p).1);
}

// ── 실패·중단·불확실 ───────────────────────────────────────────────────────

#[test]
fn a_policy_file_write_failure_keeps_the_approval_and_the_start_and_reports_the_file_separately() {
    let root = project("write-fail");
    begin(&s(&root)).unwrap();
    let p = prepared(&root, RULES_B);
    WRITE_FAULT.with(|f| *f.borrow_mut() = Some("rename"));
    let started = approve(&root, &p);
    WRITE_FAULT.with(|f| *f.borrow_mut() = None);
    let started = started.unwrap();
    assert_eq!(started["phase"], "started", "{started}");
    assert_eq!(started["policy_file"], "update_failed");
    assert_eq!(started["started"]["policy_file_sync_base"], "absent", "쓰기가 닿지 않았음이 확인돼 이전 기준으로 되돌렸다");
    assert!(!root.join(".gilignore").exists());
    assert!(!root.join(".gilignore.gil-writing").exists(), "우리가 만든 임시 파일만 치운다");
}

#[test]
fn an_unknown_replacement_result_leaves_the_base_unconfirmed() {
    let root = project("write-unknown");
    begin(&s(&root)).unwrap();
    let p = prepared(&root, RULES_B);
    WRITE_FAULT.with(|f| *f.borrow_mut() = Some("sync"));
    let started = approve(&root, &p);
    WRITE_FAULT.with(|f| *f.borrow_mut() = None);
    let started = started.unwrap();
    assert_eq!(started["policy_file"], "unsettled");
    assert_eq!(started["started"]["policy_file_sync_base"], "unconfirmed");
}

#[test]
fn an_interrupted_write_is_not_retried_automatically_when_the_disk_differs() {
    let root = project("interrupted");
    begin(&s(&root)).unwrap();
    let p = prepared(&root, RULES_B);
    let (revision, digest) = proposal_of(&p);
    // Monitor 응답은 저장됐고, 쓰기 1단계(unconfirmed) 직후 프로세스가 끝났다 — 파일은 쓰이지 않았고 결과 기록도 없다.
    let path = root.join(".gil/start/preparation.json");
    let mut ledger: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    ledger["responses"] = serde_json::json!([{"revision": revision, "proposal_digest": digest, "decision": "approve", "source": "monitor_button"}]);
    ledger["base"] = serde_json::json!({"state": "unconfirmed"});
    ledger["events"] = serde_json::json!([{"kind": "sync_started", "revision": revision, "target": p["proposal"]["rules_digest"]}]);
    fs::write(&path, serde_json::to_vec(&ledger).unwrap()).unwrap();
    // 결과가 기록되지 않은 쓰기 — 그 판의 승인을 다시 쓰지 않는다(재시작한 새 읽기에서도 같다).
    assert_eq!(status(&s(&root)).unwrap()["phase"], "sync_outcome_unrecorded");
    assert_eq!(apply(&root, revision, &digest).unwrap_err().code, "sync_outcome_unrecorded");
    assert!(review(&root).unwrap().1.is_none(), "적용 capability 도 나오지 않는다");
    assert!(!state(&root).exists());
    // 새 제안·새 승인: unconfirmed 기준에서는 자동으로 쓰지 않고 시작은 따로 확정된다.
    let p2 = prepared(&root, RULES_B);
    assert_eq!(p2["proposal"]["policy_file"]["effect"], "not_written_unconfirmed");
    let started = approve(&root, &p2).unwrap();
    assert_eq!(started["phase"], "started");
    assert_eq!(started["policy_file"], "not_written");
    assert_eq!(started["started"]["policy_file_sync_base"], "unconfirmed");
    assert!(!root.join(".gilignore").exists(), "unconfirmed 에서는 자동으로 쓰지 않는다");
}

// ── load 의 연결 검사 ──────────────────────────────────────────────────────

fn started_project(label: &str) -> (PathBuf, String) {
    let root = project(label);
    begin(&s(&root)).unwrap();
    let p = prepared(&root, RULES_B);
    approve(&root, &p).unwrap();
    (root, proposal_of(&p).1)
}

fn load_fails(root: &Path) -> String {
    crate::store::load(crate::RuleSet::builtin().unwrap(), state(root)).expect_err("거절해야 한다").to_string()
}

#[test]
fn load_refuses_a_policy_without_its_exact_monitor_approval() {
    let (root, digest) = started_project("load-approval");
    let path = state(&root);
    let text = fs::read_to_string(&path).unwrap();
    fs::write(&path, text.replace(&digest, &"a".repeat(64))).unwrap();
    assert!(load_fails(&root).contains("P1"), "다른 제안 digest");
    fs::write(&path, &text).unwrap();
    let ledger = root.join(".gil/start/preparation.json");
    let saved = fs::read(&ledger).unwrap();
    let mut value: Value = serde_json::from_slice(&saved).unwrap();
    value["responses"][0]["source"] = serde_json::json!("agent");
    fs::write(&ledger, serde_json::to_vec(&value).unwrap()).unwrap();
    load_fails(&root);
    fs::remove_file(&ledger).unwrap();
    load_fails(&root);
    fs::write(&ledger, saved).unwrap();
    open(&root);
}

#[test]
fn load_refuses_mismatched_baseline_events_and_missing_policy_objects() {
    let (root, _) = started_project("load-links");
    let path = state(&root);
    let text = fs::read_to_string(&path).unwrap();
    for (from, to) in [
        ("snapshot_ref: snapshot:A1", "snapshot_ref: snapshot:A2"),
        ("kind: start", "kind: scope_change"),
        ("kind: start", "kind: invented"),
        ("archive_halt: null", "archive_halt: {at: event:M1}"),
        ("next_cycle_id: 1\n    policy_before_ref", "next_cycle_id: 2\n    policy_before_ref"),
        ("format: 10", "format: 8"),
    ] {
        assert!(text.contains(from), "{from}");
        fs::write(&path, text.replacen(from, to, 1)).unwrap();
        load_fails(&root);
    }
    fs::write(&path, &text).unwrap();
    let rules = crate::artifact::digest_of(RULES_B.as_bytes()).unwrap().hex();
    let blob = root.join(".gil/artifacts/blobs/sha256").join(&rules[..2]).join(&rules[2..]);
    if blob.exists() {
        let kept = fs::read(&blob).unwrap();
        fs::remove_file(&blob).unwrap();
        load_fails(&root);
        fs::write(&blob, kept).unwrap();
    }
    open(&root);
}

// ── 시작 뒤에도 같은 정책 ──────────────────────────────────────────────────

#[test]
fn after_start_change_detection_and_verify_capture_use_the_approved_policy() {
    let (root, _) = started_project("after");
    let session = open(&root);
    assert!(matches!(session.world_state().unwrap(), crate::WorldState::Clean { .. }));
    drop(session);
    // 제외 경로·정책 파일의 변화는 추적 세계의 변화가 아니다.
    fs::write(root.join("node_modules/pkg/index.js"), "changed\n").unwrap();
    fs::write(root.join(".env"), "TOKEN=other\n").unwrap();
    fs::write(root.join(".gilignore"), "node_modules/\n.env\n# user edit\n").unwrap();
    assert!(matches!(open(&root).world_state().unwrap(), crate::WorldState::Clean { .. }));
    fs::write(root.join("README.md"), "tracked change\n").unwrap();
    assert!(matches!(open(&root).world_state().unwrap(), crate::WorldState::Dirty { .. }));
    // 최신 정책 복원은 아직 없다 — 실행 전에 거절하고 아무것도 바꾸지 않는다.
    let err = open(&root).restore().unwrap_err();
    assert!(matches!(err, crate::SessionError::TrackingUnsupported { .. }), "{err}");
    assert_eq!(fs::read_to_string(root.join("README.md")).unwrap(), "tracked change\n");
}

#[test]
fn an_existing_end_history_project_is_unchanged_by_this_build() {
    let root = scratch("format8");
    fs::write(root.join("a.txt"), "a\n").unwrap();
    let session = crate::ProjectSession::start(crate::RuleSet::builtin().unwrap(), state(&root)).unwrap();
    session.commit().unwrap();
    drop(session);
    let before = fs::read(state(&root)).unwrap();
    assert!(String::from_utf8_lossy(&before).starts_with("format: 8\n"));
    let session = open(&root);
    assert!(session.project().tracking().is_none());
    session.commit().unwrap();
    assert_eq!(fs::read(state(&root)).unwrap(), before, "읽고 다시 써도 형식·바이트가 같다 — 자동 전환 없음");
    drop(session);
    let refused = begin(&s(&root)).unwrap_err();
    assert_eq!(refused.code, "already_started", "{}", refused.said);
}

// ── 독립 검토가 찾은 경우의 회귀 ───────────────────────────────────────────

#[test]
fn a_swapped_rules_text_or_a_damaged_record_is_never_read_as_approved() {
    let root = project("swapped");
    begin(&s(&root)).unwrap();
    let p = prepared(&root, RULES_B);
    CAPTURE_FAULT.with(|f| *f.borrow_mut() = Some(())); // 다시 시도할 수 있는 보류로 승인만 남긴다
    assert_eq!(approve(&root, &p).unwrap()["application_held"]["code"], "capture_failed");
    let path = root.join(".gil/start/preparation.json");
    let saved = fs::read(&path).unwrap();
    use base64::Engine as _;
    for (field, value) in [
        ("rules", serde_json::json!(B64.encode("# swapped\n"))),
        ("rules_digest", serde_json::json!(crate::artifact::digest_of(&b"# swapped\n"[..]).unwrap().hex())),
        ("engine", serde_json::json!(99)),
    ] {
        let mut ledger: Value = serde_json::from_slice(&saved).unwrap();
        ledger["proposals"][0][field] = value;
        fs::write(&path, serde_json::to_vec(&ledger).unwrap()).unwrap();
        let refused = apply(&root, proposal_of(&p).0, &proposal_of(&p).1).unwrap_err();
        assert_eq!(refused.code, "preparation_unreadable", "{field}: {}", refused.said);
        assert!(!state(&root).exists() && !root.join(".gilignore").exists());
    }
    let mut ledger: Value = serde_json::from_slice(&saved).unwrap();
    ledger["base"] = serde_json::json!({"state": "present", "digest": "zz"});
    fs::write(&path, serde_json::to_vec(&ledger).unwrap()).unwrap();
    assert_eq!(status(&s(&root)).unwrap_err().code, "preparation_unreadable", "손상은 패닉이 아니라 거절");
    fs::write(&path, &saved).unwrap();
    assert_eq!(apply(&root, proposal_of(&p).0, &proposal_of(&p).1).unwrap()["phase"], "started");
}

#[test]
fn a_combination_needs_the_announcement_and_the_latest_approved_proposal() {
    let root = project("combine-guard");
    begin(&s(&root)).unwrap();
    let b = prepared(&root, RULES_B);
    // 승인 전이면 결합안 자리가 없다.
    fs::write(root.join(".gilignore"), "*.log\n").unwrap();
    let early = prepare(&s(&root), PrepareInput { rules: "x/\n".into(), project_recommendations: vec![], combine_with_revision: Some(proposal_of(&b).0), restart_canceled: false }).unwrap_err();
    assert_eq!(early.code, "combination_not_allowed");
}

// ── 수정 묶음(2026-11-23 검토 결함 네 가지)의 회귀 ─────────────────────────

#[test]
fn a_file_shown_on_an_absent_base_stays_held_and_is_not_made_writable() {
    let root = project("absent-shown");
    begin(&s(&root)).unwrap(); // 실제 준비 기준: 없음
    fs::write(root.join(".gilignore"), "node_modules/\n").unwrap(); // 첫 제안 전에 생긴 C
    let p = prepared(&root, RULES_B);
    assert_eq!(p["proposal"]["policy_file"]["effect"], "held_base_undecided");
    assert_eq!(p["proposal"]["approvable"], false, "표시만으로 쓰기 권한이 생기지 않는다");
    assert_eq!(p["phase"], "policy_file_held");
    assert_eq!(approve(&root, &p).unwrap_err().code, "blocked");
    assert_eq!(fs::read_to_string(root.join(".gilignore")).unwrap(), "node_modules/\n");
    assert!(!state(&root).exists());
    let ledger: Value = serde_json::from_slice(&fs::read(root.join(".gil/start/preparation.json")).unwrap()).unwrap();
    assert_eq!(ledger["base"]["state"], "absent", "기준을 present(C) 로 바꾸지 않는다");
    // 파일을 정리해 기준과 맞추면 정상 경로로 시작한다.
    fs::remove_file(root.join(".gilignore")).unwrap();
    let p2 = prepared(&root, RULES_B);
    assert_eq!(p2["proposal"]["policy_file"]["effect"], "create");
    assert_eq!(approve(&root, &p2).unwrap()["phase"], "started");
}

#[test]
fn a_file_confirmed_at_preparation_is_synchronized_to_the_approved_text() {
    let root = project("present-c");
    fs::write(root.join(".gilignore"), "node_modules/\n").unwrap();
    begin(&s(&root)).unwrap(); // 처음부터 확인·저장된 present(C)
    let p = prepared(&root, RULES_B);
    assert_eq!(p["proposal"]["policy_file"]["effect"], "replace");
    assert_eq!(p["proposal"]["policy_file"]["diff"], serde_json::json!(["  node_modules/", "+ .env"]));
    let started = approve(&root, &p).unwrap();
    assert_eq!(started["phase"], "started", "{started}");
    assert_eq!(started["facts"]["policy_file_write"], "written");
    assert_eq!(fs::read_to_string(root.join(".gilignore")).unwrap(), RULES_B);
}

#[test]
fn a_change_found_right_before_the_replacement_holds_the_start_and_keeps_the_records() {
    let root = project("edit-before-replace");
    begin(&s(&root)).unwrap();
    let p = prepared(&root, RULES_B);
    EDIT_BEFORE_REPLACE.with(|e| *e.borrow_mut() = Some(b"*.tmp\n".to_vec()));
    let held = approve(&root, &p).unwrap();
    assert_eq!(held["application_held"]["code"], "policy_file_changed", "{held}");
    assert_eq!(held["phase"], "condition_changed");
    assert!(!state(&root).exists(), "덮어쓰지 않았다는 이유만으로 시작하지 않는다");
    assert_eq!(fs::read_to_string(root.join(".gilignore")).unwrap(), "*.tmp\n");
    let ledger: Value = serde_json::from_slice(&fs::read(root.join(".gil/start/preparation.json")).unwrap()).unwrap();
    let kinds: Vec<&str> = ledger["events"].as_array().unwrap().iter().map(|e| e["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["attempt_started", "sync_started", "sync_settled", "condition_changed", "attempt_settled"], "앞선 사실을 지우지 않고 남긴다");
    assert_eq!(ledger["events"][2]["result"], "condition_changed");
    assert_eq!(ledger["events"][4]["outcome"], "condition_changed", "확인된 변경은 그 시도의 결과로 확정된다");
    assert_eq!(ledger["base"]["state"], "unconfirmed");
    assert_eq!(ledger["responses"].as_array().unwrap().len(), 1, "승인 응답은 그대로다");
    assert_eq!(apply(&root, proposal_of(&p).0, &proposal_of(&p).1).unwrap_err().code, "condition_changed");
    // 새 제안·새 승인: 기준 unconfirmed 에서는 자동으로 쓰지 않고, 시작은 따로 확정된다.
    let p2 = prepared(&root, RULES_B);
    assert_eq!(p2["proposal"]["policy_file"]["effect"], "not_written_unconfirmed");
    let started = approve(&root, &p2).unwrap();
    assert_eq!(started["phase"], "started", "{started}");
    assert_eq!(started["facts"]["policy_file_write"], "not_written");
    assert_eq!(started["started"]["policy_file_sync_base"], "unconfirmed");
    assert_eq!(fs::read_to_string(root.join(".gilignore")).unwrap(), "*.tmp\n");
}

#[test]
fn a_path_condition_change_needs_a_new_proposal_but_an_unstable_observation_does_not() {
    let root = project("path-change");
    begin(&s(&root)).unwrap();
    let p = prepared(&root, RULES_B);
    let fifo = std::ffi::CString::new(root.join("src/pipe").display().to_string()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    let held = approve(&root, &p).unwrap();
    assert_eq!(held["application_held"]["code"], "condition_changed", "{held}");
    assert!(!state(&root).exists() && !root.join(".gilignore").exists());
    fs::remove_file(root.join("src/pipe")).unwrap();
    assert_eq!(apply(&root, proposal_of(&p).0, &proposal_of(&p).1).unwrap_err().code, "condition_changed");

    let other = project("unstable");
    begin(&s(&other)).unwrap();
    let p = prepared(&other, RULES_B);
    CAPTURE_FAULT.with(|f| *f.borrow_mut() = Some(()));
    let held = approve(&other, &p).unwrap();
    assert_eq!(held["application_held"]["code"], "capture_failed", "{held}");
    assert_eq!(held["phase"], "approved", "관측 불안정은 조건 변경이 아니다");
    assert!(!state(&other).exists() && !other.join(".gilignore").exists(), "보관 실패 때 정책 파일을 쓰지 않는다");
    assert_eq!(apply(&other, proposal_of(&p).0, &proposal_of(&p).1).unwrap()["phase"], "started");
}

#[test]
fn a_failed_or_uncertain_final_save_is_settled_by_reading_the_stored_state() {
    let root = project("commit-fail");
    begin(&s(&root)).unwrap();
    let p = prepared(&root, RULES_B);
    COMMIT_FAULT.with(|f| *f.borrow_mut() = Some(()));
    let held = approve(&root, &p).unwrap();
    assert_eq!(held["application_held"]["code"], "start_not_saved", "{held}");
    assert!(!state(&root).exists());
    // 파일 쓰기 성공·승인 기록·보관 객체는 따로 남아 있고, 다시 시도하면 같은 판으로 시작한다.
    assert_eq!(fs::read_to_string(root.join(".gilignore")).unwrap(), RULES_B);
    let again = apply(&root, proposal_of(&p).0, &proposal_of(&p).1).unwrap();
    assert_eq!(again["phase"], "started");
    assert_eq!(again["facts"]["policy_file_write"], "already_matches");
}

#[test]
fn the_screen_shows_the_stored_proposal_and_reports_the_current_disk_separately() {
    let root = project("bound-view");
    fs::write(root.join(".gilignore"), "node_modules/\n").unwrap();
    begin(&s(&root)).unwrap();
    let p = prepared(&root, RULES_B);
    let shown_before = p["proposal"].clone();
    fs::write(root.join(".gilignore"), "edited after the proposal\n").unwrap();
    let (review_value, grant) = review(&root).unwrap();
    assert_eq!(review_value["proposal"], shown_before, "지금 디스크가 제안 판의 표시에 섞이지 않는다");
    assert_eq!(review_value["current"]["policy_file_matches_proposal"], false);
    assert!(grant.is_some(), "조회는 승인을 만들지 않으며 capability 발급 여부만 알린다");
    assert!(state(&root).exists() == false);
    let held = approve(&root, &p).unwrap();
    assert_eq!(held["application_held"]["code"], "held_three_way", "기준 A·승인 B·디스크 C 가 모두 다르면 기존 보류 그대로");
    assert!(!state(&root).exists());
    assert_eq!(fs::read_to_string(root.join(".gilignore")).unwrap(), "edited after the proposal\n");
}

#[test]
fn a_shortened_list_can_be_read_in_full_for_the_same_proposal() {
    let root = project("long-list");
    for i in 0..650 { fs::write(root.join(format!("f{i:04}.txt")), "x").unwrap(); }
    begin(&s(&root)).unwrap();
    let p = prepared(&root, RULES_B);
    assert_eq!(p["proposal"]["tracked_truncated"], true);
    let (revision, digest) = proposal_of(&p);
    let total = p["proposal"]["tracked_count"].as_u64().unwrap() as usize;
    let mut all = Vec::new();
    let mut offset = 0;
    loop {
        let page = list(&root, revision, &digest, "tracked", offset).unwrap();
        all.extend(page["items"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_owned()));
        match page["next_offset"].as_u64() { Some(next) => offset = next as usize, None => break }
    }
    assert_eq!(all.len(), total);
    assert_eq!(list(&root, revision + 1, &digest, "tracked", 0).unwrap_err().code, "unknown_proposal", "다른 판의 목록은 주지 않는다");
    assert!(!state(&root).exists());
}

#[test]
fn a_combined_text_equal_to_either_part_still_needs_its_own_approval() {
    for (label, d_equals_c) in [("combine-eq-c", true), ("combine-eq-b", false)] {
        let root = project(label);
        begin(&s(&root)).unwrap();
        let b = prepared(&root, RULES_B);
        let c = "node_modules/\n*.log\n";
        fs::write(root.join(".gilignore"), c).unwrap();
        assert_eq!(approve(&root, &b).unwrap()["application_held"]["code"], "combine_required");
        let d = if d_equals_c { c } else { RULES_B };
        let dv = prepare(&s(&root), PrepareInput { rules: d.into(), project_recommendations: vec![], combine_with_revision: Some(proposal_of(&b).0), restart_canceled: false })
            .unwrap_or_else(|e| panic!("{label}: 바이트가 같다는 이유로 거절하지 않는다: {}", e.said));
        assert!(dv["proposal"]["combination"].is_object());
        assert_eq!(dv["phase"], "awaiting_approval", "{label}: 자동 승인하지 않는다");
        assert_eq!(apply(&root, proposal_of(&b).0, &proposal_of(&b).1).unwrap_err().code, "superseded");
        let started = approve(&root, &dv).unwrap();
        assert_eq!(started["phase"], "started", "{label}: {started}");
        assert_eq!(fs::read_to_string(root.join(".gilignore")).unwrap(), d);
    }
}

// ── 두 번째 수정(조건 변경 기록의 영속성, 읽기 불가와 조건 변경의 구분) ─────────────

#[test]
fn a_detected_change_that_cannot_be_recorded_is_reported_as_unrecorded_and_held_from_the_saved_attempt() {
    let root = project("cc-unrecorded");
    begin(&s(&root)).unwrap();
    let p = prepared(&root, RULES_B);
    fs::write(root.join("NEW.txt"), "new\n").unwrap();
    CONDITION_SAVE_FAULT.with(|f| *f.borrow_mut() = Some(()));
    let held = approve(&root, &p).unwrap();
    assert_eq!(held["application_held"]["code"], "condition_change_unrecorded", "{held}");
    assert_eq!(held["application_held"]["detail"]["recorded"], false);
    assert_eq!(held["application_held"]["detail"]["detected"], "scope_changed");
    assert!(!state(&root).exists());
    let ledger: Value = serde_json::from_slice(&fs::read(root.join(".gil/start/preparation.json")).unwrap()).unwrap();
    let kinds: Vec<&str> = ledger["events"].as_array().unwrap().iter().map(|e| e["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["attempt_started"], "저장되지 않은 변경을 기록된 것처럼 남기지 않고, 관측 전의 시도 기록만 남는다");
    assert_eq!(ledger["responses"].as_array().unwrap().len(), 1, "승인 응답은 그대로다");
    // 변경이 되돌려져도(지금 디스크는 제안과 같다) 같은 판을 다시 적용하지 않는다 — 판정은 저장된 미해결 시도에서 온다.
    fs::remove_file(root.join("NEW.txt")).unwrap();
    assert_eq!(apply(&root, proposal_of(&p).0, &proposal_of(&p).1).unwrap_err().code, "attempt_outcome_unrecorded");
    assert_eq!(status(&s(&root)).unwrap()["phase"], "attempt_outcome_unrecorded");
    assert!(review(&root).unwrap().1.is_none());
    assert_eq!(ledger_block(&root, proposal_of(&p).0), Some("attempt_outcome_unrecorded"), "새로 읽은 기록만으로 같은 판정");
    let p2 = prepared(&root, RULES_B);
    assert_eq!(approve(&root, &p2).unwrap()["phase"], "started");
}

#[test]
fn a_recorded_change_survives_a_fresh_reader_even_without_the_condition_event() {
    let root = project("cc-partial");
    begin(&s(&root)).unwrap();
    let p = prepared(&root, RULES_B);
    let (revision, digest) = proposal_of(&p);
    // 앞선 후보가 남길 수 있던 부분 저장: 파일 쓰기 결과는 condition_changed 인데 ConditionChanged 사건은 없다.
    let path = root.join(".gil/start/preparation.json");
    let mut ledger: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    ledger["responses"] = serde_json::json!([{"revision": revision, "proposal_digest": digest, "decision": "approve", "source": "monitor_button"}]);
    ledger["base"] = serde_json::json!({"state": "unconfirmed"});
    ledger["events"] = serde_json::json!([
        {"kind": "sync_started", "revision": revision, "target": p["proposal"]["rules_digest"]},
        {"kind": "sync_settled", "revision": revision, "result": "condition_changed", "base": {"state": "unconfirmed"}},
    ]);
    fs::write(&path, serde_json::to_vec(&ledger).unwrap()).unwrap();
    assert_eq!(status(&s(&root)).unwrap()["phase"], "condition_changed");
    assert_eq!(apply(&root, revision, &digest).unwrap_err().code, "condition_changed");
    assert!(review(&root).unwrap().1.is_none());
    assert!(!state(&root).exists() && !root.join(".gilignore").exists());
}

#[test]
fn a_change_found_at_replacement_is_recorded_in_one_save_with_its_outcome() {
    let root = project("cc-one-save");
    begin(&s(&root)).unwrap();
    let p = prepared(&root, RULES_B);
    EDIT_BEFORE_REPLACE.with(|e| *e.borrow_mut() = Some(b"*.tmp\n".to_vec()));
    CONDITION_SAVE_FAULT.with(|f| *f.borrow_mut() = Some(()));
    let held = approve(&root, &p).unwrap();
    assert_eq!(held["application_held"]["code"], "condition_change_unrecorded", "{held}");
    let ledger: Value = serde_json::from_slice(&fs::read(root.join(".gil/start/preparation.json")).unwrap()).unwrap();
    let kinds: Vec<&str> = ledger["events"].as_array().unwrap().iter().map(|e| e["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["attempt_started", "sync_started"], "결과와 조건 변경은 함께 저장되거나 함께 빠진다");
    assert_eq!(fs::read_to_string(root.join(".gilignore")).unwrap(), "*.tmp\n");
    // 쓰기 시작만 남은 기록은 재시작 뒤에도 같은 판의 재사용을 허락하지 않는다 — 저장된 사실이 먼저 답한다.
    assert_eq!(apply(&root, proposal_of(&p).0, &proposal_of(&p).1).unwrap_err().code, "sync_outcome_unrecorded");
    assert!(ledger_block(&root, proposal_of(&p).0).is_some());
}

fn ledger_block(root: &Path, revision: u32) -> Option<&'static str> {
    let ledger = super::read_checked(&root.join(".gil")).unwrap().unwrap();
    ledger.not_reusable(revision)
}

#[test]
fn an_unreadable_policy_file_at_replacement_stops_this_application_without_a_condition_change() {
    let root = project("unreadable-replace");
    begin(&s(&root)).unwrap();
    let p = prepared(&root, RULES_B);
    UNREADABLE_BEFORE_REPLACE.with(|e| *e.borrow_mut() = Some(()));
    let held = approve(&root, &p).unwrap();
    assert_eq!(held["application_held"]["code"], "policy_file_unreadable", "{held}");
    assert_eq!(held["phase"], "approved", "읽기 불가는 조건 변경이 아니다 — 승인을 취소하지 않는다");
    assert!(!state(&root).exists());
    let ledger: Value = serde_json::from_slice(&fs::read(root.join(".gil/start/preparation.json")).unwrap()).unwrap();
    let kinds: Vec<&str> = ledger["events"].as_array().unwrap().iter().map(|e| e["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["attempt_started", "sync_started", "sync_settled", "attempt_settled"]);
    assert_eq!(ledger["events"][2]["result"], "unreadable_before_replace");
    assert_eq!(ledger["events"][3]["outcome"], "stopped");
    assert_eq!(ledger["events"][3]["code"], "policy_file_unreadable");
    assert_eq!(ledger["base"]["state"], "unconfirmed", "쓰기를 시작했으므로 읽지 못한 디스크를 없음으로 바꾸지 않는다");
    assert!(ledger_block(&root, proposal_of(&p).0).is_none());
    // 읽을 수 있게 되면 같은 승인으로 다시 시도한다 — unconfirmed 기준이라 자동으로 쓰지 않고 시작은 따로 확정된다.
    fs::remove_dir(root.join(".gilignore")).unwrap();
    let started = apply(&root, proposal_of(&p).0, &proposal_of(&p).1).unwrap();
    assert_eq!(started["phase"], "started", "{started}");
    assert_eq!(started["facts"]["policy_file_write"], "not_written");
    assert_eq!(started["started"]["policy_file_sync_base"], "unconfirmed");
}

#[test]
fn an_unreadable_policy_file_before_the_write_starts_records_nothing_and_keeps_the_approval() {
    let root = project("unreadable-presync");
    begin(&s(&root)).unwrap();
    let p = prepared(&root, RULES_B);
    UNREADABLE_BEFORE_SYNC.with(|e| *e.borrow_mut() = Some(()));
    let held = approve(&root, &p).unwrap();
    assert_eq!(held["application_held"]["code"], "policy_file_unreadable", "{held}");
    assert_eq!(held["phase"], "approved");
    assert!(!state(&root).exists());
    let ledger: Value = serde_json::from_slice(&fs::read(root.join(".gil/start/preparation.json")).unwrap()).unwrap();
    assert!(ledger["events"].as_array().unwrap().iter().all(|e| !matches!(e["kind"].as_str(), Some("sync_started" | "sync_settled" | "condition_changed"))), "{ledger}");
    assert_eq!(ledger["base"]["state"], "absent", "쓰기를 시작하지 않았고 읽기 실패를 없음이나 변경으로 바꾸지 않는다");
    let kinds: Vec<&str> = ledger["events"].as_array().unwrap().iter().map(|e| e["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["attempt_started", "attempt_settled"]);
    assert_eq!(ledger["events"][1]["outcome"], "stopped", "읽기 불가의 안전 종료가 확정됐다 — 조건 변경이 아니다");
    assert!(ledger_block(&root, proposal_of(&p).0).is_none());
    fs::remove_dir(root.join(".gilignore")).unwrap();
    let started = apply(&root, proposal_of(&p).0, &proposal_of(&p).1).unwrap();
    assert_eq!(started["phase"], "started", "{started}");
    assert_eq!(started["facts"]["policy_file_write"], "written");
}

#[test]
fn an_unrecorded_write_outcome_on_a_successful_write_still_commits_the_start_and_reads_back() {
    let root = project("settle-save-fail");
    begin(&s(&root)).unwrap();
    let p = prepared(&root, RULES_B);
    SETTLE_SAVE_FAULT.with(|f| *f.borrow_mut() = Some(()));
    let started = approve(&root, &p).unwrap();
    assert_eq!(started["phase"], "started", "{started}");
    assert_eq!(started["started"]["policy_file_sync_base"], "present", "정식 시작의 저장이 같은 기준을 지닌다");
    // 정식 시작이 확정된 뒤에는 같은 승인의 재전송·조회가 같은 결과다.
    assert_eq!(apply(&root, proposal_of(&p).0, &proposal_of(&p).1).unwrap()["started"], started["started"]);
}

#[test]
fn an_empty_policy_file_is_present_not_absent() {
    let root = project("empty-file");
    fs::write(root.join(".gilignore"), "").unwrap();
    begin(&s(&root)).unwrap();
    let ledger: Value = serde_json::from_slice(&fs::read(root.join(".gil/start/preparation.json")).unwrap()).unwrap();
    assert_eq!(ledger["base"]["state"], "present", "0바이트 일반 파일은 있음 + digest 다");
    let p = prepared(&root, RULES_B);
    assert_eq!(p["proposal"]["policy_file"]["effect"], "replace");
    assert_eq!(approve(&root, &p).unwrap()["phase"], "started");
    assert_eq!(fs::read_to_string(root.join(".gilignore")).unwrap(), RULES_B);
}

// ── 세 번째 수정(적용 시도를 관측 전에 남기고, 결과가 없는 시도를 저장된 기록으로 보류) ─────────────

fn kinds_of(root: &Path) -> Vec<String> {
    let ledger: Value = serde_json::from_slice(&fs::read(root.join(".gil/start/preparation.json")).unwrap()).unwrap();
    ledger["events"].as_array().unwrap().iter().map(|e| e["kind"].as_str().unwrap().to_owned()).collect()
}

#[test]
fn an_attempt_whose_start_record_is_not_saved_observes_and_applies_nothing() {
    let root = project("attempt-start-not-saved");
    begin(&s(&root)).unwrap();
    let p = prepared(&root, RULES_B);
    ATTEMPT_START_FAULT.with(|f| *f.borrow_mut() = Some(false));
    // 관측이 돌았다면 이 capture 고장이 쓰였을 것이다 — 남아 있으면 관측하지 않았다는 뜻이다.
    CAPTURE_FAULT.with(|f| *f.borrow_mut() = Some(()));
    let held = approve(&root, &p).unwrap();
    assert_eq!(held["application_held"]["code"], "attempt_not_recorded", "{held}");
    assert_ne!(held["phase"], "started");
    assert!(CAPTURE_FAULT.with(|f| f.borrow_mut().take()).is_some(), "시도 기록 없이 관측하지 않는다");
    assert!(!state(&root).exists() && !root.join(".gilignore").exists());
    assert!(kinds_of(&root).is_empty());
    // 저장되지 않았음이 기록으로 확인된다 — 같은 승인으로 다시 시도할 수 있다.
    assert!(ledger_block(&root, proposal_of(&p).0).is_none());
    let started = apply(&root, proposal_of(&p).0, &proposal_of(&p).1).unwrap();
    assert_eq!(started["phase"], "started", "{started}");
}

#[test]
fn an_attempt_whose_start_record_outcome_is_unknown_is_held_by_the_saved_record() {
    let root = project("attempt-start-unknown");
    begin(&s(&root)).unwrap();
    let p = prepared(&root, RULES_B);
    ATTEMPT_START_FAULT.with(|f| *f.borrow_mut() = Some(true));
    CAPTURE_FAULT.with(|f| *f.borrow_mut() = Some(()));
    let held = approve(&root, &p).unwrap();
    assert_eq!(held["application_held"]["code"], "attempt_not_recorded", "{held}");
    assert!(CAPTURE_FAULT.with(|f| f.borrow_mut().take()).is_some(), "저장을 확인하지 못하면 관측하지 않는다");
    assert!(!state(&root).exists() && !root.join(".gilignore").exists());
    // 실제로는 기록이 남았다 — 결과가 없는 시도이므로 같은 판을 보류한다(문제없었다고 단정하지 않는다).
    assert_eq!(kinds_of(&root), ["attempt_started"]);
    assert_eq!(ledger_block(&root, proposal_of(&p).0), Some("attempt_outcome_unrecorded"));
    assert_eq!(apply(&root, proposal_of(&p).0, &proposal_of(&p).1).unwrap_err().code, "attempt_outcome_unrecorded");
    assert!(review(&root).unwrap().1.is_none(), "적용 capability 를 내주지 않는다");
    let status_now = status(&s(&root)).unwrap();
    assert_eq!(status_now["phase"], "attempt_outcome_unrecorded");
    assert!(!state(&root).exists(), "조회는 시작을 만들지 않는다");
}

#[test]
fn a_safe_stop_whose_outcome_cannot_be_saved_stays_unresolved_and_a_saved_one_keeps_the_approval() {
    // 결과 저장 성공: 관측 불안정의 안전 종료가 확정되고, 같은 승인으로 다시 시도해 시작한다.
    let root = project("stop-settled");
    begin(&s(&root)).unwrap();
    let p = prepared(&root, RULES_B);
    CAPTURE_FAULT.with(|f| *f.borrow_mut() = Some(()));
    let held = approve(&root, &p).unwrap();
    assert_eq!(held["application_held"]["code"], "capture_failed", "{held}");
    assert_eq!(held["phase"], "approved");
    assert_eq!(kinds_of(&root), ["attempt_started", "attempt_settled"]);
    assert!(ledger_block(&root, proposal_of(&p).0).is_none());
    let started = apply(&root, proposal_of(&p).0, &proposal_of(&p).1).unwrap();
    assert_eq!(started["phase"], "started", "{started}");
    // 정상 확정 뒤의 재전송·응답 유실·조회는 같은 결과이며 새 판·사건·Chain 을 만들지 않는다.
    let state_bytes = fs::read(state(&root)).unwrap();
    let again = apply(&root, proposal_of(&p).0, &proposal_of(&p).1).unwrap();
    assert_eq!(again["started"], started["started"]);
    assert_eq!(status(&s(&root)).unwrap()["started"], started["started"]);
    assert_eq!(fs::read(state(&root)).unwrap(), state_bytes);

    // 결과 저장 실패: 멈췄다는 사실을 확정하지 못했으므로 승인을 유지한 재시도로 말하지 않고, 같은 판을 보류한다.
    let root = project("stop-unsettled");
    begin(&s(&root)).unwrap();
    let p = prepared(&root, RULES_B);
    CAPTURE_FAULT.with(|f| *f.borrow_mut() = Some(()));
    ATTEMPT_SETTLE_FAULT.with(|f| *f.borrow_mut() = Some(()));
    let held = approve(&root, &p).unwrap();
    assert_eq!(held["application_held"]["code"], "attempt_outcome_unrecorded", "{held}");
    assert_eq!(held["application_held"]["detail"]["stopped"], "capture_failed");
    assert_eq!(held["phase"], "attempt_outcome_unrecorded");
    assert_eq!(kinds_of(&root), ["attempt_started"]);
    // 지금 디스크는 제안 때와 같지만, 다시 읽었다는 이유로 미해결을 지우지 않는다.
    assert_eq!(apply(&root, proposal_of(&p).0, &proposal_of(&p).1).unwrap_err().code, "attempt_outcome_unrecorded");
    assert_eq!(ledger_block(&root, proposal_of(&p).0), Some("attempt_outcome_unrecorded"));
    assert!(!state(&root).exists() && !root.join(".gilignore").exists());
    assert_eq!(kinds_of(&root), ["attempt_started"], "보류된 판에는 새 시도를 열지 않는다");
    // 승인 기록은 지우지 않는다. 앞으로 가는 길은 새 제안·새 승인이다.
    let p2 = prepared(&root, RULES_B);
    assert_eq!(approve(&root, &p2).unwrap()["phase"], "started");
}

#[test]
fn a_confirmed_unsaved_final_state_settles_the_attempt_and_an_unsettled_one_is_held() {
    let root = project("commit-settled");
    begin(&s(&root)).unwrap();
    let p = prepared(&root, RULES_B);
    COMMIT_FAULT.with(|f| *f.borrow_mut() = Some(()));
    assert_eq!(approve(&root, &p).unwrap()["application_held"]["code"], "start_not_saved");
    let ledger: Value = serde_json::from_slice(&fs::read(root.join(".gil/start/preparation.json")).unwrap()).unwrap();
    let last = ledger["events"].as_array().unwrap().last().unwrap().clone();
    assert_eq!((last["kind"].as_str(), last["code"].as_str()), (Some("attempt_settled"), Some("start_not_saved")));

    let root = project("commit-unsettled");
    begin(&s(&root)).unwrap();
    let p = prepared(&root, RULES_B);
    COMMIT_FAULT.with(|f| *f.borrow_mut() = Some(()));
    ATTEMPT_SETTLE_FAULT.with(|f| *f.borrow_mut() = Some(()));
    let held = approve(&root, &p).unwrap();
    assert_eq!(held["application_held"]["code"], "attempt_outcome_unrecorded", "{held}");
    assert_eq!(held["application_held"]["detail"]["stopped"], "start_not_saved");
    assert!(!state(&root).exists());
    assert_eq!(apply(&root, proposal_of(&p).0, &proposal_of(&p).1).unwrap_err().code, "attempt_outcome_unrecorded");
}

#[test]
fn a_malformed_attempt_record_is_refused_not_read_as_resolved() {
    let root = project("attempt-malformed");
    begin(&s(&root)).unwrap();
    let p = prepared(&root, RULES_B);
    let (revision, digest) = proposal_of(&p);
    let path = root.join(".gil/start/preparation.json");
    let mut ledger: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    ledger["responses"] = serde_json::json!([{"revision": revision, "proposal_digest": digest, "decision": "approve", "source": "monitor_button"}]);
    // 시작하지 않은 시도의 결과, 다른 제안을 가리키는 시도.
    for events in [
        serde_json::json!([{"kind": "attempt_settled", "revision": revision, "attempt": 1, "outcome": "stopped", "code": "x"}]),
        serde_json::json!([{"kind": "attempt_started", "revision": revision, "attempt": 1, "proposal_digest": "0".repeat(64)}]),
    ] {
        ledger["events"] = events;
        fs::write(&path, serde_json::to_vec(&ledger).unwrap()).unwrap();
        assert_eq!(apply(&root, revision, &digest).unwrap_err().code, "preparation_unreadable");
        assert!(!state(&root).exists());
    }
}

/// Every entry below `root` with its bytes (folders as `None`), for "nothing was touched".
fn entries(root: &Path) -> std::collections::BTreeMap<PathBuf, Option<Vec<u8>>> {
    fn walk(root: &Path, at: &Path, out: &mut std::collections::BTreeMap<PathBuf, Option<Vec<u8>>>) {
        for entry in fs::read_dir(at).unwrap() {
            let path = entry.unwrap().path();
            let rel = path.strip_prefix(root).unwrap().to_owned();
            if fs::symlink_metadata(&path).unwrap().is_dir() {
                out.insert(rel, None);
                walk(root, &path, out);
            } else {
                out.insert(rel, Some(fs::read(&path).unwrap()));
            }
        }
    }
    let mut out = std::collections::BTreeMap::new();
    walk(root, root, &mut out);
    out
}

/// The interrupted bootstrap: `.gil` and its lock file exist, nothing was recorded. The same request
/// is the retry, in every build that holds this start.
#[test]
fn a_bootstrap_that_stopped_before_recording_anything_is_retried_by_the_same_request() {
    for (label, lock_file) in [("retry-empty", false), ("retry-lock", true)] {
        let root = project(label);
        fs::create_dir(root.join(".gil")).unwrap();
        if lock_file { fs::write(root.join(".gil").join(crate::lock::LOCK_FILE), b"").unwrap(); }
        let begun = begin(&s(&root)).unwrap();
        assert_eq!(begun["phase"], "preparing", "{begun}");
        assert!(root.join(".gil/start/preparation.json").is_file());
        assert!(!state(&root).exists());
    }
}

/// A `.gil` that holds something other than the lock file, with no preparation and no state file. A
/// build that also holds the image-report creation cannot tell whose interrupted creation it is, so
/// the preparation refuses **under the lock, before recovery or reaping**, and every byte stays. A
/// build with this start alone keeps its bootstrap rule (GIL's own internal leftovers are continued).
#[test]
fn leftovers_that_appeared_before_the_lock_are_preserved_where_another_start_exists() {
    let root = project("other-start-leftovers");
    // GIL's own internal folder and nothing else: the smallest leftover that is not "nothing recorded".
    fs::create_dir_all(root.join(".gil/artifacts")).unwrap();
    let before = entries(&root);
    let answer = begin(&s(&root));
    if OTHER_START {
        assert_eq!(answer.unwrap_err().code, "start_record_unsettled");
        let mut after = entries(&root);
        // The lock file is the only entry a refused request may leave: it is how the question was asked.
        after.remove(Path::new(".gil").join(crate::lock::LOCK_FILE).as_path());
        assert_eq!(after, before, "nothing was reaped, adopted or recorded");
    } else {
        assert_eq!(answer.unwrap()["phase"], "preparing");
    }
}

/// `cancel` and the application retry read the started record after releasing the lock. A project that
/// is not a tracked-scope one, or a state file that is gone by then, is a refusal and never a panic.
#[test]
fn a_started_record_of_another_kind_or_a_vanished_one_is_refused_without_a_panic() {
    let root = project("other-kind-cancel");
    fs::create_dir(root.join(".gil")).unwrap();
    fs::write(state(&root), b"format: 999\n").unwrap();
    let before = fs::read(state(&root)).unwrap();
    let refused = cancel(&s(&root)).unwrap_err();
    assert!(["state_unreadable", "not_tracking_project"].contains(&refused.code), "{}", refused.code);
    assert_eq!(apply_path(&root, &"0".repeat(64)).unwrap_err().code, refused.code);
    assert_eq!(fs::read(state(&root)).unwrap(), before);
    assert_eq!(state_gone().code, "state_unreadable");
}
