//! An unattended session over the real `gil mcp --serve` process: no Monitor, no person.
//!
//! The runner chooses the mode with the process environment (`GIL_MCP_DISPLAY=headless` and
//! `GIL_MCP_APPROVAL=unattended`). What this file fixes:
//!
//! - the tool list carries no display tool and no App tool, and the two runner tools are marked
//!   for the runner; a server started without the choice does not have them at all;
//! - a plan proposal, a requested recovery and (tracked-scope builds) a start proposal are answered
//!   through the runner tools for the exact digest read, and every such record says
//!   `unattended_runner`, never a Monitor button;
//! - the model's receipts say what waits for the user, a recovery waits only after the model asked
//!   for it, and a wrong digest or an unrequested recovery records nothing;
//! - the choice without a headless display serves nothing.
//!
//! Scope of the evidence: synthetic tool-path checks. The runner calls are harness input standing
//! in for a user; nothing here shows that a model follows the receipts or keeps the runner tools
//! unused. This process cannot tell the model from the runner: that separation is the runner's.
#![cfg(all(
    feature = "exploration-flow",
    feature = "product-safety",
    feature = "visual-authoring",
    any(target_os = "macos", all(target_os = "linux", target_arch = "x86_64"))
))]
#[path = "support_wave1/mod.rs"]
mod support;
#[path = "support_wave1/exploration.rs"]
mod exploration;
use exploration::*;
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use support::*;

const UNATTENDED: &[(&str, &str)] = &[("GIL_MCP_DISPLAY", "headless"), ("GIL_MCP_APPROVAL", "unattended")];

fn rig(label: &str, seed: &[(&str, &str)], start: Value) -> Rig {
    let root = scratch(label);
    let settings = scratch(&format!("{label}-settings"));
    for (path, bytes) in seed {
        let at = root.join(path);
        fs::create_dir_all(at.parent().unwrap()).unwrap();
        fs::write(at, bytes).unwrap();
    }
    let c = Client::serve_with_env(&settings, UNATTENDED);
    let record = product_state_path(&root, &private_local_app_data(&settings));
    let mut r = Rig { root, record, settings, scope: String::new(), c, last_close: String::new() };
    ok(r.model("gil_start", start), "start");
    r
}
fn pending(r: &mut Rig) -> Vec<Value> {
    let review = ok(r.model("gil_unattended_review", json!({})), "runner review");
    assert_eq!(structured(&review)["approval_source"], "unattended_runner");
    structured(&review)["pending"].as_array().unwrap().clone()
}
fn decide(r: &mut Rig, subject: &str, digest: &Value, decision: &str) -> Value {
    r.model("gil_unattended_decide", json!({"subject":subject,"digest":digest,"decision":decision}))
}
fn awaiting(frame: &Value) -> Value { structured(frame)["awaiting_user"].clone() }
fn everything(frame: &Value) -> String { frame["result"]["content"].to_string() }

/// The opening Interview up to an approved, closed plan, answered by the runner.
fn interview(r: &mut Rig, ids: &[&str]) {
    let mut basis = Vec::new();
    for (kind, report) in [("question", QUESTION), ("interpretation", INTERPRETATION)] {
        let opened = ok(r.model("gil_open", json!({"kind":kind,"contract":CONTRACT})), kind);
        basis.push(step_ref(&opened, kind));
        let closed = ok(r.model("gil_close", json!({"report":report})), kind);
        assert!(awaiting(&closed).is_null(), "nothing waits for the user yet: {closed}");
    }
    let opened = ok(r.model("gil_open", json!({"kind":"synthesis","contract":CONTRACT})), "synthesis");
    let synthesis = step_ref(&opened, "synthesis");
    let proposed = ok(r.model("gil_plan_propose", json!({"draft":draft(ids, &basis, &[])})), "propose");
    assert_eq!(awaiting(&proposed), json!(["plan"]), "{proposed}");
    assert!(everything(&proposed).contains("no Monitor"), "{proposed}");
    // The model has no way to close the Synthesis before the answer.
    let read = r.read();
    assert!(read["proposal"]["receipt"].is_null(), "{read}");
    let waits = pending(r);
    assert_eq!(waits.len(), 1, "{waits:?}");
    assert_eq!((waits[0]["subject"].as_str(), waits[0]["synthesis_ref"].as_str()), (Some("plan"), Some(synthesis.as_str())));
    assert_eq!(waits[0]["draft"]["items"].as_array().unwrap().len(), ids.len(), "the runner reads the exact proposal");
    let before = record_bytes(&r.record);
    refused(decide(r, "plan", &json!("0".repeat(64)), "approve"), "an answer for a digest that does not wait");
    assert_eq!(record_bytes(&r.record), before, "a stale answer recorded something");
    let done = ok(decide(r, "plan", &waits[0]["digest"], "approve"), "runner approves the plan");
    assert_eq!(structured(&done)["result"]["receipt"], json!({"decision":"approve","source":"unattended_runner"}));
    assert_eq!(structured(&done)["pending"], json!([]));
    let read = r.read();
    assert_eq!(read["proposal"]["receipt"]["source"], "unattended_runner");
    let closed = ok(r.model("gil_close", json!({"report":read["close_report"]})), "close synthesis");
    assert!(awaiting(&closed).is_null(), "{closed}");
    let outcome = ok(r.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})), "open outcome");
    let outcome_ref = step_ref(&outcome, "outcome");
    ok(r.model("gil_close", json!({"report":format!("verdict: success\nlesson: 합의한 요구를 기록했다\nsynthesis_ref: {synthesis}\nnext_direction.action: close_cycle\nnext_direction.reason: 승인된 요구를 탐색한다\n")})), "close outcome");
    ok(r.model("gil_close", json!({"report":format!("verdict: success\noutcome_ref: {outcome_ref}\nhandoff_summary: 합성 요구를 무인 실행자의 응답으로 승인받았다\nnext_direction.action: open_child\nnext_direction.reason: 다음 단계로 간다\n")})), "close interview cycle");
}

#[test]
fn the_tool_list_has_no_display_or_app_tool_and_marks_the_runner_tools() {
    let settings = scratch("unattended-tools-settings");
    let mut c = Client::serve_with_env(&settings, UNATTENDED);
    let listed = c.request("tools/list", json!({}));
    let tools = listed["result"]["tools"].as_array().unwrap();
    let mut runner = Vec::new();
    for tool in tools {
        let name = tool["name"].as_str().unwrap();
        assert!(!name.contains("monitor") && !name.contains("companion"), "a display tool is listed: {name}");
        let visibility = &tool["_meta"]["ui"]["visibility"];
        assert_ne!(visibility, &json!(["app"]), "an App tool is listed: {name}");
        if visibility == &json!(["runner"]) { runner.push(name); }
    }
    assert_eq!(runner, ["gil_unattended_review", "gil_unattended_decide"]);
    for name in ["gil_start", "gil_open", "gil_close", "gil_context", "gil_plan_propose", "gil_plan_read", "gil_revisit"] {
        assert!(tools.iter().any(|tool| tool["name"] == name), "{name} is missing");
    }
    assert_eq!(c.request("resources/list", json!({}))["result"]["resources"], json!([]));
    // The removed tools are not merely hidden: they cannot be called here.
    for (name, args) in [("show_gil_monitor", json!({"scope_id":format!("project:{}", "0".repeat(64))})), ("gil_plan_review", json!({"scope_id":"x"}))] {
        rpc_error(c.call(name, args), name);
    }
    // A server started without the choice has no runner tool, listed or callable.
    for env in [&[][..], &[("GIL_MCP_DISPLAY", "headless")][..]] {
        let mut plain = Client::serve_with_env(&scratch("unattended-plain-settings"), env);
        let listed = plain.request("tools/list", json!({}));
        assert!(!listed["result"]["tools"].as_array().unwrap().iter().any(|tool| tool["name"].as_str().unwrap().contains("unattended")), "{env:?}");
        rpc_error(plain.call("gil_unattended_review", json!({"project_root":std::env::temp_dir()})), "runner tool on an ordinary server");
    }
}

#[test]
fn the_choice_without_a_headless_display_serves_nothing() {
    for env in [&[("GIL_MCP_APPROVAL", "unattended")][..], &[("GIL_MCP_DISPLAY", "headless"), ("GIL_MCP_APPROVAL", "always")][..]] {
        let settings = scratch("unattended-refused-settings");
        let out = isolated_command(&settings).envs(env.iter().copied()).stdin(std::process::Stdio::null()).output().expect("gil mcp --serve");
        assert!(!out.status.success(), "{env:?} served");
        assert!(out.stdout.is_empty(), "{env:?} wrote to the protocol stream");
        assert!(String::from_utf8_lossy(&out.stderr).contains("GIL_MCP_APPROVAL"), "{env:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
}

#[test]
fn a_plan_and_a_requested_recovery_are_answered_by_the_runner_and_recorded_as_unattended() {
    let mut r = rig("unattended-flow", &[("data/dates.csv", "id,date\n1,2026-01-02\n2,02/01/2026\n")], direct_start());
    assert_eq!(pending(&mut r), Vec::<Value>::new(), "a fresh project waits for nothing");
    interview(&mut r, &["R1"]);
    assert_eq!(yaml(&r.record)["chains"]["nodes"][0]["approved_plan"]["proposals"][0]["receipt"]["source"], "unattended_runner");

    let c2 = r.open_experiment("R1");
    r.run(c2, &Run { requirement: "R1", problem: "허용 날짜 형식을 벗어나는 값이 존재하는가?",
        files: &[("evidence/C2.md", "벗어나는 값 1건\n")], evidence: "evidence/C2.md", fails_back_to: None });
    let world_at_c2 = world(&r.root);
    let c3 = r.open_experiment("R1");
    r.run(c3, &Run { requirement: "R1", problem: "모든 날짜를 공통 형식으로 변환할 수 있는가?",
        files: &[("evidence/C3.md", "2행을 변환하지 못했다\n"), ("data/dates.csv", "broken\n")], evidence: "evidence/C3.md", fails_back_to: Some("cycle:C2") });
    assert_ne!(world(&r.root), world_at_c2);

    // A recovery plan exists, but nobody asked for it: the runner is offered nothing and can approve nothing.
    assert_eq!(pending(&mut r), Vec::<Value>::new());
    let (record_before, world_before) = (record_bytes(&r.record), world(&r.root));
    refused(decide(&mut r, "revisit", &json!("0".repeat(64)), "approve"), "a recovery the model did not ask for");
    assert_eq!((record_bytes(&r.record), world(&r.root)), (record_before.clone(), world_before.clone()));

    // The model asks. Its refusal says the decision is the user's and that it waits outside.
    let asked = refused(r.model("gil_revisit", json!({})), "the model-side revisit without an approval");
    assert_eq!(awaiting(&asked), json!(["revisit"]), "{asked}");
    assert!(everything(&asked).contains("no Monitor") && everything(&asked).contains("바꾸지 않았다"), "{asked}");
    assert_eq!((record_bytes(&r.record), world(&r.root)), (record_before, world_before));
    let waits = pending(&mut r);
    assert_eq!(waits.len(), 1, "{waits:?}");
    let plan = &waits[0]["plan"];
    assert_eq!((waits[0]["subject"].as_str(), plan["from"].as_str(), plan["target"].as_str()), (Some("revisit"), Some("cycle:C3"), Some("cycle:C2")));
    refused(decide(&mut r, "revisit", &waits[0]["digest"], "request_changes"), "any answer but approval");
    let done = ok(decide(&mut r, "revisit", &waits[0]["digest"], "approve"), "runner approves the recovery");
    let result = &structured(&done)["result"];
    assert_eq!((result["state"].as_str(), result["approval_source"].as_str()), (Some("completed"), Some("unattended_runner")), "{done}");
    assert_eq!(structured(&done)["pending"], json!([]));

    // The world is C2's again, the failed files are preserved, and the source record names no click.
    assert_eq!(world(&r.root), world_at_c2);
    let preserved = PathBuf::from(plan["archive"].as_str().unwrap());
    let source: Value = serde_json::from_slice(&fs::read(preserved.join("source.json")).unwrap()).unwrap();
    assert_eq!((source["approval_source"].as_str(), source["human_click"].as_bool()), (Some("unattended_runner"), Some(false)), "{source}");
    refused(decide(&mut r, "revisit", &waits[0]["digest"], "approve"), "the same recovery twice");

    // The model continues from the recorded state: the next Experiment branches from the failure.
    assert_eq!(r.open_experiment("R1"), 4);
    assert_eq!(r.read()["current_experiment"]["revisit_from"], "cycle:C3");
    assert_eq!(r.experiments("R1")[..2], [("cycle:C2".to_owned(), "success".to_owned()), ("cycle:C3".to_owned(), "failure".to_owned())]);
}

#[cfg(all(feature = "tracking-policy", unix))]
#[test]
fn a_tracked_scope_start_is_approved_by_the_runner_and_recorded_as_unattended() {
    let mut r = rig("unattended-start", &[("app.py", "print('hi')\n")], json!({"start_kind":"tracked_scope"}));
    assert_eq!(pending(&mut r), Vec::<Value>::new(), "no proposal yet");
    let status = ok(r.model("gil_start_status", json!({})), "status");
    assert_eq!(structured(&status)["phase"], "preparing");
    let rules = "# approved by the runner\n.env\n";
    let prepared = ok(r.model("gil_start_prepare", json!({"rules":rules})), "prepare");
    assert_eq!((structured(&prepared)["phase"].as_str(), awaiting(&prepared)), (Some("awaiting_approval"), json!(["start"])), "{prepared}");
    assert!(!everything(&prepared).contains("show_gil_monitor"), "{prepared}");
    let waits = pending(&mut r);
    assert_eq!((waits.len(), waits[0]["subject"].as_str(), waits[0]["kind"].as_str()), (1, Some("start"), Some("decide")), "{waits:?}");
    refused(decide(&mut r, "start", &json!("0".repeat(64)), "approve"), "an answer for a digest that does not wait");
    assert!(!r.root.join(".gil/state.yaml").exists(), "a stale answer started the project");
    let done = ok(decide(&mut r, "start", &waits[0]["digest"], "approve"), "runner approves the start");
    assert_eq!(structured(&done)["result"]["phase"], "started", "{done}");
    let status = ok(r.model("gil_start_status", json!({})), "status after the start");
    assert_eq!(structured(&status)["phase"], "started");
    assert_eq!(structured(&status)["history"][0]["response"], json!({"decision":"approve","source":"unattended_runner"}));
    assert!(awaiting(&status).is_null(), "{status}");
    assert_eq!(fs::read_to_string(r.root.join(".gilignore")).unwrap(), rules, "the approved bytes are the policy file");
    // The started project reads back in a new process: the stored approval mark is accepted on load.
    let (exit, noise) = std::mem::replace(&mut r.c, Client::serve_with_env(&r.settings, UNATTENDED)).eof();
    assert!(exit.success() && noise.is_empty(), "{exit:?} {noise:?}");
    assert!(r.context().contains("cycle:C1"));
}
