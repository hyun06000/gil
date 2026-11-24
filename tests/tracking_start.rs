//! 구현 순서 4번 — 시작 준비 → 정확한 제안 → Monitor(App-only) 승인 → 최초 보관 → 정식 시작을 **실제 `gil` stdio 프로세스**로
//! 확인한다(`tracking-policy` 개발 구성). 합성 임시 프로젝트만 쓴다. 여기의 App 호출은 시험 하네스의 입력이며 사람의 동의나
//! 보이는 화면의 증거가 아니다 — 프로토콜과 기록의 연결만 증명한다.
#![cfg(all(feature = "tracking-policy", unix))]
#[path = "support_wave1/mod.rs"]
mod support;
mod common;
use serde_json::{Value, json};
use support::*;

fn sc(frame: &Value) -> &Value {
    &frame["result"]["structuredContent"]
}

fn is_error(frame: &Value) -> bool {
    frame["result"]["isError"] == json!(true)
}

fn project(name: &str) -> std::path::PathBuf {
    let root = scratch(name).canonicalize().unwrap();
    std::fs::write(root.join("README.md"), "hello\n").unwrap();
    std::fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
    std::fs::write(root.join("node_modules/pkg/i.js"), "x\n").unwrap();
    root
}

#[test]
fn this_build_reads_formats_10_and_11_and_refuses_a_policy_without_its_monitor_approval_untouched() {
    assert_eq!(gil::newest_known_format(), 11);
    // 형식 8 기록에 tracking 을 붙이거나, 형식 10 으로 번호만 올린 합성 기록은 승인 응답·원문 객체가 없어 거절된다.
    let root = project("tracking-forged");
    let state = root.join(".gil/state.yaml");
    let session = gil::ProjectSession::start(common::spec(), &state).unwrap();
    session.commit().unwrap();
    drop(session);
    let original = std::fs::read_to_string(&state).unwrap();
    let block = format!(
        "tracking:\n  next_policy_id: 2\n  next_event_id: 2\n  policies:\n  - id: P1\n    engine: 1\n    rules: {{algorithm: sha256, digest: {d}}}\n    proposal_digest: {d}\n  policy_file:\n    sync_base: {{state: absent}}\n  events:\n  - id: M1\n    kind: start\n    chain_ref: chain:C1\n    after_cycle_ref: null\n    review_cycle_ref: null\n    next_cycle_id: 1\n    policy_before_ref: null\n    policy_after_ref: policy:P1\n    world_before_ref: null\n    baseline: {{after_cycle_ref: null, next_cycle_id: 1, snapshot_ref: snapshot:A1, attempts: [{{result: archived}}]}}\n  archive_halt: null\n",
        d = "ab".repeat(32));
    for text in [format!("{original}{block}"), format!("{}{block}", original.replacen("format: 8", "format: 10", 1))] {
        std::fs::write(&state, &text).unwrap();
        let before = std::fs::read(&state).unwrap();
        let err = gil::load(common::spec(), &state).expect_err("승인·원문 없는 정책은 읽히지 않는다");
        assert!(matches!(err, gil::StoreError::Tracking(_)), "{err}");
        assert!(gil::ProjectSession::open(common::spec(), &state).is_err());
        assert_eq!(std::fs::read(&state).unwrap(), before, "거절은 아무것도 바꾸지 않는다");
    }
    std::fs::write(&state, &original).unwrap();
    assert!(gil::load(common::spec(), &state).is_ok(), "기존 형식 8 은 그대로 읽힌다");
}

#[test]
fn the_monitor_approved_start_commits_once_over_the_real_stdio_process() {
    let root = project("tracking-stdio");
    let settings = scratch("tracking-stdio-settings");
    let mut c = Client::serve(&settings);
    let r = root.display().to_string();
    // 카탈로그: 승인 도구는 App 전용이고 모델 도구에는 승인 값을 실을 칸이 없다.
    let listed = c.request("tools/list", json!({}));
    let tools = listed["result"]["tools"].as_array().unwrap();
    let tool = |name: &str| tools.iter().find(|t| t["name"] == name).unwrap_or_else(|| panic!("{name}")).clone();
    assert_eq!(tool("gil_start_decide")["_meta"]["ui"]["visibility"], json!(["app"]));
    assert_eq!(tool("gil_start_review")["_meta"]["ui"]["visibility"], json!(["app"]));
    assert_eq!(tool("gil_start_apply")["_meta"]["ui"]["visibility"], json!(["app"]));
    assert_eq!(tool("gil_start_list")["_meta"]["ui"]["visibility"], json!(["app"]));

    let begun = c.call("gil_start", json!({"project_root": r, "start_kind": "tracked_scope"}));
    assert!(!is_error(&begun), "{begun}");
    assert_eq!(sc(&begun)["phase"], "preparing");
    let scope = sc(&begun)["scope_id"].as_str().unwrap().to_owned();
    assert!(!root.join(".gil/state.yaml").exists() && !root.join(".gil/artifacts").exists());
    // 준비 중에도 같은 Monitor 경로가 준비 화면만 보여 준다 — 가짜 Chain 없음.
    let shown = c.call("show_gil_monitor", json!({"scope_id": scope}));
    assert_eq!(sc(&shown)["start_review_supported"], true, "{shown}");
    assert!(sc(&shown).get("view").is_none());

    let prepared = c.call("gil_start_prepare", json!({"project_root": r, "rules": "node_modules/\n"}));
    assert!(!is_error(&prepared), "{prepared}");
    let digest = sc(&prepared)["proposal"]["digest"].as_str().unwrap().to_owned();
    let revision = sc(&prepared)["proposal"]["revision"].clone();
    // 적용은 App 전용이다: 모델 입력(project_root)은 schema 가 받지 않고, capability 없이는 거절된다.
    let model_apply = c.call("gil_start_apply", json!({"project_root": r, "proposal_digest": digest}));
    assert!(model_apply.get("error").is_some() || is_error(&model_apply), "{model_apply}");
    let forged = c.call("gil_start_apply", json!({"scope_id": scope, "revision": revision, "digest": digest, "token": "f".repeat(64)}));
    assert!(is_error(&forged), "{forged}");

    let review = c.call("gil_start_review", json!({"scope_id": scope}));
    let token = review["result"]["_meta"]["gil/startApprovalToken"].as_str().expect("App 전용 capability").to_owned();
    assert!(!review["result"]["content"].to_string().contains(&token) && !sc(&review).to_string().contains(&token));
    let decided = c.call("gil_start_decide", json!({"scope_id": scope, "revision": revision, "digest": digest, "token": token, "decision": "approve"}));
    assert_eq!(sc(&decided)["phase"], "started", "{decided}");
    let state = std::fs::read_to_string(root.join(".gil/state.yaml")).unwrap();
    assert!(state.starts_with("format: 10\n") && !state.contains(&token));
    assert_eq!(std::fs::read_to_string(root.join(".gilignore")).unwrap(), "node_modules/\n");

    // 응답 유실 뒤: 같은 capability 재전송은 거절, 적용·조회는 같은 결과(중복 발급 없음).
    let resent = c.call("gil_start_decide", json!({"scope_id": scope, "revision": revision, "digest": digest, "token": token, "decision": "approve"}));
    assert!(is_error(&resent));
    let again = c.call("gil_start_status", json!({"project_root": r}));
    assert_eq!(sc(&again)["started"], sc(&decided)["started"]);
    let after = c.call("gil_start_review", json!({"scope_id": scope}));
    assert!(after["result"]["_meta"].get("gil/startApprovalToken").is_none() && after["result"]["_meta"].get("gil/startApplyToken").is_none());
    assert_eq!(std::fs::read_to_string(root.join(".gil/state.yaml")).unwrap(), state);

    // 새 프로세스(재시작)에서도 같은 기록을 읽고, 정식 프로젝트의 문맥·Monitor 가 열린다.
    drop(c);
    let mut c = Client::serve(&settings);
    let status = c.call("gil_start_status", json!({"project_root": r}));
    assert_eq!(sc(&status)["phase"], "started");
    let context = c.call("gil_context", json!({"project_root": r}));
    assert!(!is_error(&context), "{context}");
    let shown = c.call("show_gil_monitor", json!({"scope_id": scope}));
    assert!(sc(&shown).get("view").is_some(), "정식 시작 뒤에는 그래프 view: {shown}");
    assert_eq!(sc(&shown)["tracking"]["policy"], "policy:P1");
    // 정식 시작 뒤 되돌아가기·복원은 최신 정책 복원이 없어 실행 전에 거절된다.
    let restore = c.call("gil_restore", json!({"project_root": r}));
    assert!(is_error(&restore) || sc(&restore)["ok"] == false, "{restore}");
    assert_eq!(std::fs::read_to_string(root.join(".gil/state.yaml")).unwrap(), state);
}

#[test]
fn a_fresh_process_holds_an_attempt_without_a_recorded_outcome_from_the_disk_record_alone() {
    // 디스크 상태는 시험이 쓴 합성 기록이다(승인 응답 + 결과 없는 적용 시도) — 실제 중단·전원 장애를 일으킨 것이 아니다.
    // 증명하는 것은 그 기록을 처음 읽는 **새 `gil` 프로세스**가 프로세스 메모리 없이 같은 판을 보류한다는 것뿐이다.
    let root = project("tracking-open-attempt");
    let settings = scratch("tracking-open-attempt-settings");
    let r = root.display().to_string();
    let mut c = Client::serve(&settings);
    let begun = c.call("gil_start", json!({"project_root": r, "start_kind": "tracked_scope"}));
    assert!(!is_error(&begun), "{begun}");
    let scope = sc(&begun)["scope_id"].as_str().unwrap().to_owned();
    let prepared = c.call("gil_start_prepare", json!({"project_root": r, "rules": "node_modules/\n"}));
    let digest = sc(&prepared)["proposal"]["digest"].as_str().unwrap().to_owned();
    let revision = sc(&prepared)["proposal"]["revision"].as_u64().unwrap();
    drop(c);
    let path = root.join(".gil/start/preparation.json");
    let mut ledger: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    ledger["responses"] = json!([{"revision": revision, "proposal_digest": digest, "decision": "approve", "source": "monitor_button"}]);
    ledger["events"] = json!([{"kind": "attempt_started", "revision": revision, "attempt": 1, "proposal_digest": digest}]);
    std::fs::write(&path, serde_json::to_vec_pretty(&ledger).unwrap()).unwrap();

    let mut c = Client::serve(&settings);
    let status = c.call("gil_start_status", json!({"project_root": r}));
    assert_eq!(sc(&status)["phase"], "attempt_outcome_unrecorded", "{status}");
    let review = c.call("gil_start_review", json!({"scope_id": scope}));
    assert!(review["result"]["_meta"].get("gil/startApplyToken").is_none(), "{review}");
    assert!(review["result"]["_meta"].get("gil/startApprovalToken").is_none(), "{review}");
    assert!(!root.join(".gil/state.yaml").exists() && !root.join(".gilignore").exists());
    let after: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(after["events"], ledger["events"], "조회는 미해결 시도를 지우거나 새 시도를 열지 않는다");
    assert_eq!(after["responses"], ledger["responses"], "승인 기록은 그대로다");

    // 새 제안과 그 판의 App 승인으로는 시작한다.
    let next = c.call("gil_start_prepare", json!({"project_root": r, "rules": "node_modules/\n"}));
    let d2 = sc(&next)["proposal"]["digest"].as_str().unwrap().to_owned();
    let v2 = sc(&next)["proposal"]["revision"].clone();
    let review = c.call("gil_start_review", json!({"scope_id": scope}));
    let token = review["result"]["_meta"]["gil/startApprovalToken"].as_str().expect("새 판의 승인 capability").to_owned();
    let decided = c.call("gil_start_decide", json!({"scope_id": scope, "revision": v2, "digest": d2, "token": token, "decision": "approve"}));
    assert_eq!(sc(&decided)["phase"], "started", "{decided}");
}

#[test]
fn the_cli_never_falls_back_to_a_whole_tree_start() {
    let root = project("tracking-cli");
    let out = std::process::Command::new(gil()).arg("start").current_dir(&root).output().unwrap();
    assert!(!out.status.success());
    assert!(!root.join(".gil/state.yaml").exists());
}
