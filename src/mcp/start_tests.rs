//! 시작 도구의 App capability 경계 시험 — 합성 임시 폴더, 실제 MCP 호출 함수(`call`)를 그대로 쓴다.

use super::*;
use crate::mcp::bindings::Store;

fn scratch(label: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("gil-start-tool-{}-{label}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dir = dir.canonicalize().unwrap();
    std::fs::write(dir.join("a.txt"), "a\n").unwrap();
    std::fs::create_dir_all(dir.join("node_modules")).unwrap();
    std::fs::write(dir.join("node_modules/x.js"), "x\n").unwrap();
    dir
}

fn wire(result: CallToolResult) -> Value {
    serde_json::to_value(result).unwrap()
}

#[test]
fn the_start_approval_capability_is_app_only_private_bound_single_use_and_never_stored() {
    let root = scratch("capability");
    let monitor = Monitor::with_store(Store::at(root.with_extension("consent")));
    let grants = Grants::default();
    let r = root.display().to_string();
    let begun = wire(call(&monitor, &grants, "gil_start", json!({"project_root": r})));
    assert_ne!(begun["isError"], json!(true), "{begun}");
    let scope = begun["structuredContent"]["scope_id"].as_str().unwrap().to_owned();
    assert_eq!(begun["structuredContent"]["next_call"]["name"], "show_gil_monitor");
    assert!(!root.join(".gil/state.yaml").exists());

    // 모델 도구는 capability 를 내지 않는다.
    let prepared = wire(call(&monitor, &grants, "gil_start_prepare", json!({"project_root": r, "rules": "node_modules/\n"})));
    assert!(prepared.get("_meta").is_none() || prepared["_meta"].get("gil/startApprovalToken").is_none());
    let p = &prepared["structuredContent"]["proposal"];
    let (revision, digest) = (p["revision"].clone(), p["digest"].clone());

    let review = wire(call(&monitor, &grants, "gil_start_review", json!({"scope_id": scope})));
    let token = review["_meta"]["gil/startApprovalToken"].as_str().unwrap().to_owned();
    assert_eq!(token.len(), 64);
    assert!(!review["structuredContent"].to_string().contains(&token));
    assert!(!review["content"].to_string().contains(&token));
    assert!(!format!("{grants:?}").contains(&token));

    let request = json!({"scope_id": scope, "revision": revision, "digest": digest, "token": token, "decision": "approve"});
    // 위조: 다른 scope, 다른 판·digest, 새 프로세스(빈 grants), 추측한 token — 모두 거절하고 아무것도 기록하지 않는다.
    for (bad, grants_used) in [
        ({ let mut v = request.clone(); v["scope_id"] = json!("project:other"); v }, &grants),
        ({ let mut v = request.clone(); v["digest"] = json!("0".repeat(64)); v }, &grants),
        ({ let mut v = request.clone(); v["token"] = json!("f".repeat(64)); v }, &grants),
        (request.clone(), &Grants::default()),
    ] {
        assert_eq!(wire(call(&monitor, grants_used, "gil_start_decide", bad))["isError"], json!(true));
    }
    let ledger = std::fs::read_to_string(root.join(".gil/start/preparation.json")).unwrap();
    assert!(!ledger.contains("\"responses\": [\n    {"), "위조 응답은 기록되지 않는다");

    let decided = wire(call(&monitor, &grants, "gil_start_decide", request.clone()));
    assert_eq!(decided["structuredContent"]["phase"], "started", "{decided}");
    // 쓰고 버린 capability — 재전송은 거절되고, 결과는 조회로 확인한다.
    assert_eq!(wire(call(&monitor, &grants, "gil_start_decide", request))["isError"], json!(true));
    let status = wire(call(&monitor, &grants, "gil_start_status", json!({"project_root": r})));
    assert_eq!(status["structuredContent"]["phase"], "started");
    for file in [root.join(".gil/state.yaml"), root.join(".gil/start/preparation.json")] {
        assert!(!std::fs::read_to_string(file).unwrap().contains(&token), "capability 는 기록에 없다");
    }
    // 결정용 capability 는 적용에 쓸 수 없다(쓰임이 다르다).
    // 정식 시작 뒤 review 는 capability 를 내지 않는다.
    let after = wire(call(&monitor, &grants, "gil_start_review", json!({"scope_id": scope})));
    assert!(after.get("_meta").and_then(|m| m.get("gil/startApprovalToken")).is_none());
}

#[test]
fn the_catalog_hides_the_approval_tools_from_the_model() {
    let tools = super::super::surface::tools();
    for name in APP_NAMES {
        let tool = tools.iter().find(|t| t.name == *name).expect("catalogued");
        let raw = serde_json::to_value(tool).unwrap();
        assert_eq!(raw["_meta"]["ui"]["visibility"], json!(["app"]), "{name}");
    }
    for name in ["gil_start", "gil_start_prepare", "gil_start_status", "gil_start_cancel"] {
        let raw = serde_json::to_value(tools.iter().find(|t| t.name == name).unwrap()).unwrap();
        assert_eq!(raw["_meta"]["ui"]["visibility"], json!(["model"]), "{name}");
        assert!(!raw["inputSchema"]["properties"].as_object().unwrap().contains_key("token"), "{name}");
    }
    // 모델이 승인 값을 실어 보낼 칸이 없다.
    assert!(!valid("gil_start_apply", &json!({"project_root": "/x", "proposal_digest": "d"})), "적용은 모델의 project_root 입력을 받지 않는다");
    assert!(!valid("gil_start_apply", &json!({"scope_id": "s", "revision": 1, "digest": "d"})), "capability 없는 적용 없음");
    assert!(!valid("gil_start_prepare", &json!({"project_root": "/x", "rules": "", "decision": "approve"})));
}

#[test]
fn a_held_approval_is_retried_only_with_the_separate_apply_capability() {
    let root = scratch("apply-capability");
    let monitor = Monitor::with_store(Store::at(root.with_extension("consent")));
    let grants = Grants::default();
    let r = root.display().to_string();
    let scope = wire(call(&monitor, &grants, "gil_start", json!({"project_root": r})))["structuredContent"]["scope_id"].as_str().unwrap().to_owned();
    let p = wire(call(&monitor, &grants, "gil_start_prepare", json!({"project_root": r, "rules": "node_modules/\n"})));
    let (revision, digest) = (p["structuredContent"]["proposal"]["revision"].clone(), p["structuredContent"]["proposal"]["digest"].clone());
    std::fs::write(root.join("late.txt"), "changes the tracked list\n").unwrap();
    let review = wire(call(&monitor, &grants, "gil_start_review", json!({"scope_id": scope})));
    let token = review["_meta"]["gil/startApprovalToken"].as_str().unwrap().to_owned();
    let held = wire(call(&monitor, &grants, "gil_start_decide", json!({"scope_id": scope, "revision": revision, "digest": digest, "token": token, "decision": "approve"})));
    assert_eq!(held["structuredContent"]["application_held"]["code"], "scope_changed", "{held}");
    std::fs::remove_file(root.join("late.txt")).unwrap();
    // 승인된 제안에는 결정용이 아닌 적용용 capability 만 나온다. 서로 바꿔 쓸 수 없다.
    let review = wire(call(&monitor, &grants, "gil_start_review", json!({"scope_id": scope})));
    assert!(review["_meta"].get("gil/startApprovalToken").is_none());
    let apply_token = review["_meta"]["gil/startApplyToken"].as_str().unwrap().to_owned();
    let as_decision = wire(call(&monitor, &grants, "gil_start_decide", json!({"scope_id": scope, "revision": revision, "digest": digest, "token": apply_token, "decision": "approve"})));
    assert_eq!(as_decision["isError"], json!(true));
    let review = wire(call(&monitor, &grants, "gil_start_review", json!({"scope_id": scope})));
    let apply_token = review["_meta"]["gil/startApplyToken"].as_str().unwrap().to_owned();
    let started = wire(call(&monitor, &grants, "gil_start_apply", json!({"scope_id": scope, "revision": revision, "digest": digest, "token": apply_token})));
    assert_eq!(started["structuredContent"]["phase"], "started", "{started}");
    assert!(!std::fs::read_to_string(root.join(".gil/start/preparation.json")).unwrap().contains(&apply_token));
}
