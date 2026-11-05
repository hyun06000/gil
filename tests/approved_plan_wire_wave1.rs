//! Wave 1 E — the M6-D approval contract over the real `gil mcp --serve` stdio process.
//!
//! What this checks is wiring, not domain truth (src/approved_plan/tests.rs owns that): the
//! App-only capability lives in `_meta` and never in model content or the record; bare
//! `approved: yes` and tampered/stale close_reports are refused; edits void earlier consent;
//! a restarted process reissues the capability; the Experiment is gated until Outcome and the
//! Interview Cycle close; Verify needs the bound plan document bytes plus fresh evidence;
//! restore/revisit stay blocked; a reinterview request forces an Interview; request_changes
//! closes with approved:no; a Closing Interview needs its own approval and the next Chain
//! cannot reuse it. Only a synthetic temporary project is touched. Every App decision here is a
//! synthetic harness input — it proves the protocol, never a person's consent or a visible screen.
//! Wave 2 D: the record path follows the product layout (Windows private app data below an
//! isolated LOCALAPPDATA), the child environment is isolated, and a missing record is a failure.
#![cfg(feature = "approved-plan")]
#[path = "support_wave1/mod.rs"]
mod support;
use serde_json::{Value, json};
use std::path::Path;
use support::*;
#[cfg(all(feature = "product-safety", feature = "visual-authoring", any(target_os = "macos", windows)))]
#[path = "support_wave1/revisit_cases.rs"]
mod revisit_cases;

struct Rig { root: std::path::PathBuf, record: std::path::PathBuf, scope: String, c: Client }

// Real Core/guard processes, with read-only records as in a restricted Host. This remains a
// synthetic protocol acceptance test: the App decisions below are not human UI observations.
#[cfg(all(target_os = "macos", feature = "visual-authoring"))]
fn guard_boundary(root: &Path, writable: bool) {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let profile = format!("(version 1)(allow default)(deny file-write* (subpath {}))",
        serde_json::to_string(root.join(".gil").to_str().unwrap()).unwrap());
    let command = |action: &str| {
        // Direct hooks can observe read-only records. Shell guards run through the Host's
        // normal per-command approval route, then apply their own mandatory sandbox.
        let mut c = if action == "hook" {
            let mut c = Command::new("/usr/bin/sandbox-exec");
            c.args(["-p", &profile, env!("CARGO_BIN_EXE_gil-write-guard")]);
            c
        } else { Command::new(env!("CARGO_BIN_EXE_gil-write-guard")) };
        c.args([action, "--root"]).arg(root).current_dir(root);
        c
    };
    let hook = |name: &str, args: Value| {
        let mut c = command("hook").stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
        c.stdin.take().unwrap().write_all(json!({"hook_event_name":"PreToolUse", "tool_name":name,
            "cwd":root,"tool_input":args}).to_string().as_bytes()).unwrap();
        let out = c.wait_with_output().unwrap();
        assert!(out.status.success());
        serde_json::from_slice::<Value>(&out.stdout).unwrap()
    };
    for (tool, args) in [
        ("Write", json!({"file_path":"report.docx"})),
        ("Edit", json!({"file_path":"guard-result.txt"})),
        ("apply_patch", json!({"input":"*** Begin Patch\n*** Add File: report.docx\n+probe\n*** End Patch"}))
    ] {
        let r = hook(tool, args);
        assert_eq!(r["hookSpecificOutput"]["permissionDecision"] == "deny", !writable, "{tool}: {r}");
        assert!(!r.to_string().contains("잠금 파일을 열지"), "unexpected lock failure: {r}");
    }
    assert_eq!(hook("Write", json!({"file_path":"../external.txt"})), json!({}));
    assert_eq!(hook("WebSearch", json!({"query":"synthetic"})), json!({}));
    assert_eq!(hook("Write", json!({"file_path":".gil/forbidden"}))["hookSpecificOutput"]["permissionDecision"], "deny");
    let before = std::fs::read(root.join("guard-result.txt")).ok();
    let out = command("run").args(["--", "printf outside > ../outside-probe.txt && printf measured > guard-result.txt"]).output().unwrap();
    assert_eq!(out.status.success(), writable, "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(std::fs::read(root.join("../outside-probe.txt")).unwrap(), b"outside");
    if writable { assert_eq!(std::fs::read(root.join("guard-result.txt")).unwrap(), b"measured"); }
    else { assert_eq!(std::fs::read(root.join("guard-result.txt")).ok(), before); }
    let out = command("run").args(["--", "printf forbidden > .gil/forbidden"]).output().unwrap();
    assert!(!out.status.success());
    assert!(!root.join(".gil/forbidden").exists());
}

#[cfg(all(target_os = "macos", feature = "visual-authoring"))]
fn auto_guard(settings: &Path, mut input: Value) -> Value {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let home = settings.join("home"); std::fs::create_dir_all(&home).unwrap();
    input["session_id"] = json!("closing-session"); input["cwd"] = json!(home);
    let mut child = Command::new(gil()).args(["write-guard","hook","--auto"]).env("HOME",&home)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(input.to_string().as_bytes()).unwrap();
    let out=child.wait_with_output().unwrap(); assert!(out.status.success());
    serde_json::from_slice(&out.stdout).unwrap()
}

#[cfg(all(target_os = "macos", feature = "visual-authoring"))]
#[test]
fn guarded_project_completes_approved_closing_without_outside_write_regressions() {
    let root = scratch("guard-closing-project");
    let settings = scratch("guard-closing-settings");
    let record = product_state_path(&root, &private_local_app_data(&settings));
    let mut r = Rig { root: root.clone(), record, scope: String::new(), c: Client::serve(&settings) };
    let started = ok(r.model("gil_start", json!({})), "start");
    auto_guard(&settings, json!({"hook_event_name":"PostToolUse","tool_name":"mcp__gil__gil_start","tool_input":{"project_root":root},"tool_response":started["result"]}));
    assert_eq!(auto_guard(&settings,json!({"hook_event_name":"Stop","last_assistant_message":"done"}))["decision"],"block");
    refused(r.model("gil_await_user",json!({"question":"What is the goal?"})),"wait before Question");
    let next = &structured(&started)["next_call"];
    assert_eq!(next["name"], "show_gil_monitor");
    r.scope = next["arguments"]["scope_id"].as_str().unwrap().to_owned();
    ok(r.c.call("show_gil_monitor", next["arguments"].clone()), "show");
    guard_boundary(&root, false);
    let (basis, synthesis) = r.ask_and_interpret();
    refused(r.model("gil_await_user",json!({"question":"Approve the scope?"})),"wait before proposal");
    ok(r.model("gil_plan_propose", json!({"draft":draft(&["files"], &basis, &[])})), "propose");
    let wait = ok(r.model("gil_await_user",json!({"question":"Approve the scope?"})),"wait for approval");
    auto_guard(&settings,json!({"hook_event_name":"PostToolUse","tool_name":"mcp__gil__gil_await_user","tool_input":{"project_root":root,"question":"Approve the scope?"},"tool_response":wait["result"]}));
    assert_eq!(auto_guard(&settings,json!({"hook_event_name":"Stop","last_assistant_message":"Approve the scope?"})),json!({}));
    guard_boundary(&root, false); // Opening approval wait.
    r.approve_via_app(1);
    assert_eq!(auto_guard(&settings,json!({"hook_event_name":"Stop","last_assistant_message":"Approve the scope?"}))["decision"],"block");
    r.close_approved_interview(&synthesis, "open_child");
    guard_boundary(&root, false); // Between cycles.
    let (_, document) = r.open_experiment("files");
    for (kind, report) in [("define", "problem: protected output\nsuccess_condition: measured bytes saved"),
        ("hypothesis", "hypothesis: Verify permits output\nrationale: guard follows Core state\nguardrail: outside writes remain allowed")] {
        ok(r.model("gil_open", json!({"kind":kind,"contract":CONTRACT})), kind);
        guard_boundary(&root, false);
        ok(r.model("gil_close", json!({"report":report})), kind);
        guard_boundary(&root, false);
    }
    let opened = ok(r.model("gil_open", json!({"kind":"verify","contract":CONTRACT})), "verify");
    let verify = step_ref(&opened, "verify");
    guard_boundary(&root, true);
    refused(r.model("gil_await_user",json!({"question":"Can I finish?"})),"Verify cannot masquerade as user wait");
    assert_eq!(auto_guard(&settings,json!({"hook_event_name":"Stop","last_assistant_message":"done"}))["decision"],"block");
    std::fs::write(root.join("plan.md"), &document).unwrap();
    ok(r.c.call("gil_close", verify_args(&root, &verify,
        "execution: protected shell wrote the measured bytes\nresult: guard-result.txt contains measured\nplan_document: plan.md\nevidence_files: guard-result.txt\n")), "close verify");
    guard_boundary(&root, false);
    ok(r.model("gil_open", json!({"kind":"analysis","contract":CONTRACT})), "analysis");
    guard_boundary(&root, false);
    ok(r.model("gil_close", json!({"report":"interpretation: actual protected output matches the expected bytes\nhypothesis_fit: yes\nproblem_solved: yes\nsuccess_condition_met: yes\nguardrail_triggered: no\n"})), "close analysis");
    let opened = ok(r.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})), "outcome");
    let outcome = step_ref(&opened, "outcome");
    guard_boundary(&root, false);
    ok(r.model("gil_close", json!({"report":"verdict: success\nlesson: Verify alone permits protected writes\nnext_direction.action: close_cycle\nnext_direction.reason: measured output preserved\n"})), "close outcome");
    guard_boundary(&root, false);
    ok(r.model("gil_close", json!({"report":format!("verdict: success\noutcome_ref: {outcome}\nhandoff_summary: measured protected output preserved\nnext_direction.action: open_child\nnext_direction.reason: final Closing review\n")})), "close experiment");
    guard_boundary(&root, false); // Existing output unchanged after Verify.
    ok(r.model("gil_open", json!({"kind":"interview"})), "closing");
    let mut basis = Vec::new();
    for (kind, report) in [("question", QUESTION), ("interpretation", INTERPRETATION)] {
        let opened = ok(r.model("gil_open", json!({"kind":kind,"contract":CONTRACT})), kind);
        basis.push(step_ref(&opened, kind));
        guard_boundary(&root, false);
        ok(r.model("gil_close", json!({"report":report})), kind);
    }
    let opened = ok(r.model("gil_open", json!({"kind":"synthesis","contract":CONTRACT})), "closing synthesis");
    let synthesis = step_ref(&opened, "synthesis");
    guard_boundary(&root, false); // Before proposal and while awaiting its decision.
    ok(r.model("gil_plan_propose", json!({"draft":draft(&["files"], &basis, &[("chain_verdict","success")])})), "closing proposal");
    guard_boundary(&root, false);
    r.approve_via_app(2);
    guard_boundary(&root, false); // Approval alone never permits Artifact writes.
    r.close_approved_interview(&synthesis, "close_chain");
    guard_boundary(&root, false);
    ok(r.model("gil_close", json!({"report":format!("verdict: success\nclosing_synthesis_ref: {synthesis}\nhandoff_summary: guard output and Closing boundaries verified\n")})), "chain report");
    guard_boundary(&root, false); // Closed Chain.
    assert_eq!(auto_guard(&settings,json!({"hook_event_name":"Stop","last_assistant_message":"Chain complete"})),json!({}));
    assert_eq!(auto_guard(&settings,json!({"hook_event_name":"UserPromptSubmit","prompt":"An unrelated web-only question"})),json!({}));
    assert_eq!(auto_guard(&settings,json!({"hook_event_name":"Stop","last_assistant_message":"Unrelated answer"})),json!({}));
}

impl Rig {
    fn model(&mut self, name: &str, mut args: Value) -> Value {
        if name != "show_gil_monitor" { args["project_root"] = json!(self.root); }
        self.c.call(name, args)
    }
    fn app(&mut self, name: &str, mut args: Value) -> Value { args["scope_id"] = json!(self.scope); self.c.call(name, args) }
    fn read(&mut self) -> Value { structured(&ok(self.model("gil_plan_read", json!({})), "gil_plan_read")).clone() }
    /// Reading twice must not mint approval, advance state, or expose an App capability.
    fn context_readonly(&mut self) -> String {
        let before = record_bytes(&self.record);
        let first = said(&ok(self.model("gil_context", json!({})), "context"));
        let second = said(&ok(self.model("gil_context", json!({})), "context again"));
        assert_eq!(first, second, "context must be a deterministic projection");
        assert_eq!(record_bytes(&self.record), before, "context changed the record");
        assert!(!first.contains("gil/planApprovalToken"));
        first
    }
    /// Question + Interpretation + open Synthesis; returns (basis refs, synthesis ref).
    fn ask_and_interpret(&mut self) -> (Vec<String>, String) {
        let mut basis = Vec::new();
        for (kind, report) in [("question", QUESTION), ("interpretation", INTERPRETATION)] {
            let opened = ok(self.model("gil_open", json!({"kind":kind,"contract":CONTRACT})), kind);
            basis.push(step_ref(&opened, kind));
            ok(self.model("gil_close", json!({"report":report})), kind);
        }
        let syn = ok(self.model("gil_open", json!({"kind":"synthesis","contract":CONTRACT})), "synthesis");
        (basis, step_ref(&syn, "synthesis"))
    }
    fn approve_via_app(&mut self, revision: u64) -> String {
        let review = ok(self.app("gil_plan_review", json!({})), "review");
        let t = token(&review).expect("capability in _meta");
        let p = &structured(&review)["proposal"];
        assert_eq!(p["revision"], json!(revision), "{review}");
        ok(self.app("gil_plan_decide", json!({"revision":revision,"digest":p["digest"],"token":t,"decision":"approve"})), "approve");
        let read = self.read();
        assert_eq!(read["proposal"]["receipt"]["decision"], "approve");
        assert_eq!(read["proposal"]["receipt"]["source"], "monitor_button");
        t
    }
    fn close_approved_interview(&mut self, synthesis: &str, action: &str) {
        let read = self.read();
        ok(self.model("gil_close", json!({"report":read["close_report"]})), "close synthesis");
        let outcome = ok(self.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})), "open outcome");
        let outcome_ref = step_ref(&outcome, "outcome");
        ok(self.model("gil_close", json!({"report":format!("verdict: success\nlesson: 합의한 범위를 기록했다\nsynthesis_ref: {synthesis}\nnext_direction.action: close_cycle\nnext_direction.reason: 승인된 범위를 실험으로 확인한다\n")})), "close outcome");
        let closed = ok(self.model("gil_close", json!({"report":format!("verdict: success\noutcome_ref: {outcome_ref}\nhandoff_summary: 합성 범위를 Monitor 응답으로 승인받았다\nnext_direction.action: {action}\nnext_direction.reason: 다음 단계로 간다\n")})), "close interview cycle");
        assert!(said(&closed).contains(" · interview · success"), "{closed}");
    }
    fn open_experiment(&mut self, item: &str) -> (u32, String) {
        let opened = ok(self.model("gil_open", json!({"kind":"experiment"})), "open experiment");
        let n = cycle_number(&opened);
        let read = self.read();
        assert_eq!(read["current_item"]["id"], item, "{read}");
        let document = read["document"].as_str().expect("document after binding").to_owned();
        (n, document)
    }
    fn walk_to_verify(&mut self, item: &str) -> String {
        ok(self.model("gil_open", json!({"kind":"define","contract":CONTRACT})), "define");
        ok(self.model("gil_close", json!({"report":format!("problem: 항목 {item}을 합성 자료로 확인한다\nsuccess_condition: evidence 파일에 결과가 저장된다\n")})), "close define");
        ok(self.model("gil_open", json!({"kind":"hypothesis","contract":CONTRACT})), "hypothesis");
        ok(self.model("gil_close", json!({"report":format!("hypothesis: 합성 자료만으로 항목 {item}을 확인할 수 있다\nrationale: 자료가 제공됐다\nguardrail: 외부 데이터를 쓰지 않는다\n")})), "close hypothesis");
        let v = ok(self.model("gil_open", json!({"kind":"verify","contract":CONTRACT})), "verify");
        step_ref(&v, "verify")
    }
    fn finish_experiment(&mut self, n: u32, item: &str, verify: &str, document: &str) {
        std::fs::create_dir_all(self.root.join("plan")).unwrap();
        std::fs::create_dir_all(self.root.join("evidence")).unwrap();
        std::fs::write(self.root.join("plan/approved-plan.md"), document).unwrap();
        let evidence = format!("evidence/C{n}-{item}.md");
        std::fs::write(self.root.join(&evidence), format!("# 합성 관측 C{n}\n\n항목 {item} 결과 (시험 전용)\n")).unwrap();
        ok(self.c.call("gil_close", verify_args(&self.root, verify, &format!("execution: 합성 자료를 확인하고 결과를 저장했다\nresult: 항목 {item} 결과 파일을 남겼다\nplan_document: plan/approved-plan.md\nevidence_files: {evidence}\n"))), "close verify");
        ok(self.model("gil_open", json!({"kind":"analysis","contract":CONTRACT})), "analysis");
        ok(self.model("gil_close", json!({"report":"hypothesis_fit: yes\nproblem_solved: yes\nsuccess_condition_met: yes\nguardrail_triggered: no\ninterpretation: 합성 결과가 가설과 일치했다\n"})), "close analysis");
        let o = ok(self.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})), "experiment outcome");
        assert!(said(&o).contains("Outcome Step의 close_cycle"));
        let outcome_ref = step_ref(&o, "outcome");
        ok(self.model("gil_close", json!({"report":format!("verdict: success\nlesson: 항목 {item}을 파일 증거로 확인했다\nnext_direction.action: close_cycle\nnext_direction.reason: 다음 항목으로 간다\n")})), "close experiment outcome");
        let before = record_bytes(&self.record);
        refused(self.model("gil_close", json!({"report":format!("verdict: success\noutcome_ref: {outcome_ref}\nhandoff_summary: synthetic result\nnext_direction.action: close_cycle\nnext_direction.reason: incorrect copied Step direction\n")})), "Step direction cannot close Experiment Cycle");
        assert_eq!(record_bytes(&self.record), before);
        let closed = ok(self.model("gil_close", json!({"report":format!("verdict: success\noutcome_ref: {outcome_ref}\nhandoff_summary: 항목 {item} 완료\nnext_direction.action: open_child\nnext_direction.reason: 남은 항목을 이어간다\n")})), "close experiment cycle");
        assert!(said(&closed).contains(&format!("닫았다: cycle:C{n} · experiment · success")), "{closed}");
    }
}

#[test]
fn documented_proposal_opens_and_closes_with_schema_rejections_leaving_no_trace() {
    let root = scratch("documented-plan-project");
    let settings = scratch("documented-plan-settings");
    let record = product_state_path(&root, &private_local_app_data(&settings));
    let mut r = Rig { root: root.clone(), record, scope: String::new(), c: Client::serve(&settings) };
    let started = ok(r.model("gil_start", json!({})), "start");
    let next = &structured(&started)["next_call"];
    r.scope = next["arguments"]["scope_id"].as_str().unwrap().to_owned();
    ok(r.c.call("show_gil_monitor", next["arguments"].clone()), "show");
    let (basis, synthesis) = r.ask_and_interpret();
    // Parse the actual shared documentation, not a hand-copied fixture. Only the selected
    // project path and real lineage refs are substituted as the instructions require.
    let section = include_str!("../src/mcp/workflow.md").split("**Approved plans").nth(1).unwrap();
    let example = section.split("```json").nth(1).unwrap().split("```").next().unwrap();
    let mut input: Value = serde_json::from_str(example.trim()).unwrap();
    input["project_root"] = json!(root);
    input["draft"]["synthesis"]["basis_refs"] = json!(basis.join("\n"));
    let before = record_bytes(&r.record);
    let secret = "/synthetic/private-schema-secret";
    let mut flat = input["draft"].clone();
    flat["project_root"] = json!(secret);
    let mut extra = input.clone();
    extra["draft"][secret] = json!(secret);
    let mut wrong_type = input.clone();
    wrong_type["draft"]["synthesis"]["statement"] = json!([secret]);
    let mut bad_root = input.clone();
    bad_root["project_root"] = json!([secret]);
    for invalid in [flat, extra, wrong_type, bad_root] {
        let response = rpc_error(r.c.call("gil_plan_propose", invalid), "invalid proposal");
        assert!(!response.to_string().contains(secret), "schema error echoed input");
        assert_eq!(record_bytes(&r.record), before, "schema refusal changed the record");
    }
    // Valid wire shape can still fail Core's checklist/required-field grammar.
    let mut mismatch = input.clone();
    mismatch["draft"]["synthesis"]["exploration_checklist"] = json!("A\nB");
    refused(r.c.call("gil_plan_propose", mismatch), "mismatched checklist");
    for field in ["statement", "basis_refs", "exploration_checklist"] {
        let mut missing = input.clone();
        missing["draft"]["synthesis"].as_object_mut().unwrap().remove(field);
        refused(r.c.call("gil_plan_propose", missing), "missing required Report field");
    }
    assert_eq!(record_bytes(&r.record), before);
    ok(r.c.call("gil_plan_propose", input.clone()), "documented opening");
    r.approve_via_app(1);
    r.close_approved_interview(&synthesis, "open_child");
    let (cycle, document) = r.open_experiment("A");
    let verify = r.walk_to_verify("A");
    r.finish_experiment(cycle, "A", &verify, &document);
    ok(r.model("gil_open", json!({"kind":"interview"})), "closing interview");
    let (basis, synthesis) = r.ask_and_interpret();
    input["draft"]["synthesis"]["basis_refs"] = json!(basis.join("\n"));
    input["draft"]["synthesis"]["chain_verdict"] = json!("failure");
    ok(r.c.call("gil_plan_propose", input), "documented closing retains executed items");
    r.approve_via_app(2);
    r.close_approved_interview(&synthesis, "close_chain");
    // Interview approval succeeds even when the human judges the Chain a failure.
    ok(r.model("gil_close", json!({"report":format!("verdict: failure\nclosing_synthesis_ref: {synthesis}\nhandoff_summary: synthetic final judgment\n")})), "close chain");
    let (status, noise, stderr) = r.c.eof_with_stderr();
    assert!(status.success() && noise.is_empty());
    assert!(!stderr.contains(secret), "schema rejection leaked input to stderr");
}

#[test]
fn approval_contract_holds_over_the_real_stdio_process() {
    let root = scratch("plan-wire-project");
    let settings = scratch("plan-wire-settings");
    let c = Client::serve(&settings);
    let record = product_state_path(&root, &private_local_app_data(&settings));
    let state = |_: &Path| record_bytes(&record);
    let mut r = Rig { root: root.clone(), record: record.clone(), scope: String::new(), c };

    // tools/list: App tools are not model tools.
    let listed = r.c.request("tools/list", json!({}));
    let tools = listed["result"]["tools"].as_array().unwrap();
    for (name, app) in [("gil_plan_propose", false), ("gil_plan_read", false), ("gil_plan_request_interview", false), ("gil_plan_review", true), ("gil_plan_decide", true)] {
        let t = tools.iter().find(|t| t["name"] == name).unwrap_or_else(|| panic!("{name} missing from tools/list"));
        assert_eq!(t["_meta"]["ui"]["visibility"], json!([if app { "app" } else { "model" }]), "{name}");
    }

    // start once; follow the returned show_gil_monitor; display stays unconfirmed.
    let started = ok(r.model("gil_start", json!({})), "gil_start");
    let text = started["result"]["content"].as_array().unwrap().iter().filter_map(|c| c["text"].as_str()).collect::<String>();
    let at = text.find("show_gil_monitor(").expect("start names show_gil_monitor") + "show_gil_monitor(".len();
    let end = text[at..].find(')').unwrap();
    r.scope = serde_json::from_str::<Value>(&text[at..at + end]).unwrap()["scope_id"].as_str().unwrap().to_owned();
    assert_eq!(started["result"]["structuredContent"]["display_confirmed"], json!(false));
    assert!(r.record.is_file(), "record missing at the product layout path {} (record_storage={})", r.record.display(), started["result"]["structuredContent"]["record_storage"]);
    if cfg!(windows) { assert_eq!(started["result"]["structuredContent"]["record_storage"], json!("private_app_data")); }
    else { assert!(!private_local_app_data(&settings).join("GIL").exists(), "LOCALAPPDATA store must not appear on a non-Windows host"); }
    ok(r.c.call("show_gil_monitor", json!({"scope_id":r.scope})), "show_gil_monitor");

    let (basis, synthesis) = r.ask_and_interpret();
    assert_eq!(synthesis, "step:C1/S3");

    // bare approved:yes is refused and writes nothing; review without a proposal has no capability.
    let before = state(&root);
    refused(r.model("gil_close", json!({"report":format!("statement: 합성 범위\nbasis_refs: {}\napproved: yes\n", basis.join("\n"))})), "bare approved:yes");
    assert_eq!(state(&root), before);
    let empty = ok(r.app("gil_plan_review", json!({})), "review without proposal");
    assert!(structured(&empty)["proposal"].is_null() && token(&empty).is_none(), "{empty}");

    // propose / idempotent / read
    let d = draft(&["A", "B"], &basis, &[]);
    // The field-test shape (scope/items/synthesis at the top level) is refused by name: the
    // -32602 message says 'draft' is missing and where the fields belong, without echoing input.
    let flat = rpc_error(r.model("gil_plan_propose", json!({"scope":d["scope"],"items":d["items"],"synthesis":d["synthesis"],"exploration_checklist":"A"})), "flat propose");
    let message = flat["error"]["message"].as_str().unwrap_or_default().to_owned();
    assert!(message.contains("'draft' is missing") && message.contains("inside draft"), "{message}");
    let first = ok(r.model("gil_plan_propose", json!({"draft":d})), "propose");
    assert_eq!(structured(&first)["proposal"]["revision"], 1);
    assert!(structured(&first)["proposal"]["receipt"].is_null());
    assert!(!first.to_string().contains("planApprovalToken"));
    let again = ok(r.model("gil_plan_propose", json!({"draft":d})), "propose again");
    assert_eq!(structured(&again)["proposal"]["digest"], structured(&first)["proposal"]["digest"]);
    let rev1 = r.read()["proposal"].clone();
    assert!(r.context_readonly().contains("Monitor 버튼 응답을 기다린다"));

    // capability only in _meta; state untouched; stable within a process
    let before = state(&root);
    let review = ok(r.app("gil_plan_review", json!({})), "review");
    let t1 = token(&review).expect("token");
    assert_eq!(t1.len(), 64);
    assert!(!structured(&review).to_string().contains(&t1) && !review["result"]["content"].to_string().contains(&t1));
    assert_eq!(state(&root), before);
    assert_eq!(token(&ok(r.app("gil_plan_review", json!({})), "review again")).as_deref(), Some(t1.as_str()));

    // wrong scope / token / digest refused; schema violations rejected; no receipt recorded
    let base = json!({"revision":1,"digest":rev1["digest"],"token":t1,"decision":"approve"});
    let mut other = base.clone(); other["scope_id"] = json!(format!("project:{}", "0".repeat(64)));
    refused(r.c.call("gil_plan_decide", other), "other scope");
    let mut bad = base.clone(); bad["token"] = json!("f".repeat(64));
    refused(r.app("gil_plan_decide", bad), "wrong token");
    let mut bad = base.clone(); bad["digest"] = json!("0".repeat(64));
    refused(r.app("gil_plan_decide", bad), "wrong digest");
    let mut bad = base.clone(); bad["approved"] = json!(true);
    rpc_error(r.app("gil_plan_decide", bad), "extra field");
    rpc_error(r.model("gil_plan_propose", json!({"draft":{"approved":true}})), "malformed draft");
    assert_eq!(state(&root), before);
    assert!(r.read()["proposal"]["receipt"].is_null());

    // edit → revision 2; old decision and old close_report are void
    // An unanswered proposal offers no close_report; a forged revision-1 approval must be refused now and later.
    assert!(r.read()["close_report"].is_null());
    let close_report_rev1 = format!("statement: 합성 자료로 항목을 순서대로 확인하고 파일 증거를 남긴다\nbasis_refs: |\n  {}\nexploration_checklist: |\n  A\n  B\napproved: yes\nplan_revision: 1\nplan_digest: {}\n", basis.join("\n  "), rev1["digest"].as_str().unwrap());
    let mut edited = d.clone(); edited["scope"]["goal"] = json!("합성 자료 기반 항목별 보고서 (수정)");
    let second = ok(r.model("gil_plan_propose", json!({"draft":edited})), "propose edited");
    assert_eq!(structured(&second)["proposal"]["revision"], 2);
    let rev2 = structured(&second)["proposal"].clone();
    refused(r.app("gil_plan_decide", base.clone()), "decide on superseded revision");
    let before = state(&root);
    refused(r.model("gil_close", json!({"report":close_report_rev1})), "close with revision-1 report");
    assert_eq!(state(&root), before);

    // restart: scope survives, capability is reissued, the old one is refused
    let old = std::mem::replace(&mut r.c, Client::serve(&settings));
    let (status, noise) = old.eof();
    assert!(status.success(), "first process exit: {status:?}");
    assert!(noise.is_empty(), "non-JSON on stdout: {noise:?}");
    let review = ok(r.app("gil_plan_review", json!({})), "review after restart");
    let t2 = token(&review).expect("token after restart");
    assert_ne!(t2, t1);
    assert_eq!(structured(&review)["proposal"]["revision"], 2);
    refused(r.app("gil_plan_decide", json!({"revision":2,"digest":rev2["digest"],"token":t1,"decision":"approve"})), "old capability after restart");
    let accepted = ok(r.app("gil_plan_decide", json!({"revision":2,"digest":rev2["digest"],"token":t2,"decision":"approve"})), "approve revision 2");
    assert_eq!(structured(&accepted)["proposal"]["receipt"]["decision"], "approve");

    assert!(r.context_readonly().contains("close_report를 그대로 gil_close"));

    // duplicate approve idempotent; flip refused; no capability after receipt; no token in record
    let saved = state(&root);
    ok(r.app("gil_plan_decide", json!({"revision":2,"digest":rev2["digest"],"token":t2,"decision":"approve"})), "duplicate approve");
    assert_eq!(state(&root), saved);
    refused(r.app("gil_plan_decide", json!({"revision":2,"digest":rev2["digest"],"token":t2,"decision":"request_changes"})), "flip");
    assert!(token(&ok(r.app("gil_plan_review", json!({})), "review after receipt")).is_none());
    let record = String::from_utf8(saved).unwrap();
    assert!(!record.contains(&t1) && !record.contains(&t2), "capability written into the record");

    // close only with the exact close_report
    let report = r.read()["close_report"].as_str().unwrap().to_owned();
    assert!(report.contains("plan_revision") && report.contains("plan_digest"), "{report}");
    refused(r.model("gil_close", json!({"report":report.replace("합성 자료로 항목을", "다른 문장으로 항목을")})), "tampered close_report");
    refused(r.model("gil_close", json!({"report":close_report_rev1})), "revision-1 report after revision-2 approval");
    ok(r.model("gil_close", json!({"report":report})), "close synthesis");

    // Experiment gated until Outcome and the Interview Cycle close; then binds A
    refused(r.model("gil_open", json!({"kind":"experiment"})), "experiment after synthesis");
    let outcome = ok(r.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})), "open outcome");
    assert!(said(&outcome).contains("Outcome Step의 close_cycle"));
    let outcome_ref = step_ref(&outcome, "outcome");
    ok(r.model("gil_close", json!({"report":format!("verdict: success\nlesson: 합의한 범위를 기록했다\nsynthesis_ref: {synthesis}\nnext_direction.action: close_cycle\nnext_direction.reason: 승인된 범위를 실험으로 확인한다\n")})), "close outcome");
    refused(r.model("gil_open", json!({"kind":"experiment"})), "experiment before cycle close");
    refused(r.model("gil_cycle", json!({"action":"close"})), "legacy gil_cycle close");
    let before_cycle = record_bytes(&r.record);
    refused(r.model("gil_close", json!({"report":format!("verdict: success\noutcome_ref: {outcome_ref}\nhandoff_summary: synthetic scope\nnext_direction.action: close_cycle\nnext_direction.reason: incorrect copied Step direction\n")})), "Step direction cannot close Interview Cycle");
    assert_eq!(record_bytes(&r.record), before_cycle);
    ok(r.model("gil_close", json!({"report":format!("verdict: success\noutcome_ref: {outcome_ref}\nhandoff_summary: 합성 범위를 승인받았다\nnext_direction.action: open_child\nnext_direction.reason: 승인 항목을 순서대로 실행한다\n")})), "close interview cycle");
    let read = r.read();
    assert_eq!(read["active_revision"], 2);
    assert_eq!(read["pending"].as_array().unwrap().iter().map(|i| i["id"].as_str().unwrap()).collect::<Vec<_>>(), ["A", "B"]);
    assert!(read["document"].is_null(), "no binding before the Experiment opens");
    let (c2, document) = r.open_experiment("A");
    let opened_read = r.read();
    assert!(opened_read["next"].as_str().unwrap().contains("gil_open(kind: define)"));
    assert!(!opened_read["next"].as_str().unwrap().contains("Monitor 응답 후"));
    let read_only_bytes = record_bytes(&r.record);
    assert_eq!(r.read(), opened_read);
    assert_eq!(record_bytes(&r.record), read_only_bytes);
    assert!(document.contains("Revision: 2"));
    let context = r.context_readonly();
    assert!(context.contains("gil_open(kind: define)") && context.contains("이번 Experiment: A"), "{context}");

    // Verify: document bytes + separate fresh evidence; four refusals first
    let verify = r.walk_to_verify("A");
    let context = r.context_readonly();
    assert!(context.contains("열린 Verify") && context.contains("plan_document") && context.contains("evidence_files"), "{context}");
    std::fs::create_dir_all(root.join("plan")).unwrap();
    std::fs::create_dir_all(root.join("evidence")).unwrap();
    std::fs::write(root.join(format!("evidence/C{c2}-A.md")), "# 합성 관측\n\n항목 A 결과\n").unwrap();
    let before = state(&root);
    refused(r.c.call("gil_close", verify_args(&root, &verify, &format!("execution: 확인했다\nresult: 저장했다\nevidence_files: evidence/C{c2}-A.md\n"))), "verify without plan_document");
    std::fs::write(root.join("plan/approved-plan.md"), &document).unwrap();
    refused(r.c.call("gil_close", verify_args(&root, &verify, "execution: 확인했다\nresult: 저장했다\nplan_document: plan/approved-plan.md\nevidence_files: plan/approved-plan.md\n")), "plan document as the only evidence");
    std::fs::write(root.join("plan/approved-plan.md"), document.replace("Revision: 2", "Revision: 1")).unwrap();
    refused(r.c.call("gil_close", verify_args(&root, &verify, &format!("execution: 확인했다\nresult: 저장했다\nplan_document: plan/approved-plan.md\nevidence_files: evidence/C{c2}-A.md\n"))), "wrong document bytes");
    refused(r.c.call("gil_close", verify_args(&root, &verify, "execution: 확인했다\nresult: 채팅으로 보고했다\n")), "no evidence_files");
    assert_eq!(state(&root), before);
    let refused_read = r.read();
    assert_eq!(refused_read["document"], opened_read["document"], "failed closes never materialize or change the document");
    assert_eq!(refused_read["executed_cycles"], opened_read["executed_cycles"]);
    assert_eq!(refused_read["pending"], opened_read["pending"]);
    assert!(refused_read["next"].as_str().unwrap().contains("열린 Verify"));
    assert!(!refused_read["next"].as_str().unwrap().contains("Synthesis를 닫"));
    r.finish_experiment(c2, "A", &verify, &document);
    let context = r.context_readonly();
    assert!(context.contains("gil_open(kind: experiment)"), "{context}");
    assert!(!context.contains("gil_plan_read.document") && context.contains("닫힌 Experiment의 실행 연결: A"), "closed Cycle still directs writing: {context}");
    // A new server must reconstruct the same projection from the persisted approval/binding.
    let old = std::mem::replace(&mut r.c, Client::serve(&settings));
    let (status, noise) = old.eof();
    assert!(status.success() && noise.is_empty());
    assert_eq!(r.context_readonly(), context);

    let read = r.read();
    assert_eq!(read["executed_cycles"][0]["item_id"], "A");
    assert_eq!(read["executed_cycles"][0]["revision"], 2);

    // restore / revisit stay blocked (product-safety / approved-plan lock) and change nothing
    let before = state(&root);
    refused(r.model("gil_restore", json!({})), "restore blocked");
    refused(r.model("gil_revisit", json!({})), "revisit blocked");
    assert_eq!(state(&root), before);

    // reinterview request preserved through the Experiment; next cycle must be an Interview
    let (c3, _) = r.open_experiment("B");
    ok(r.model("gil_open", json!({"kind":"define","contract":CONTRACT})), "define");
    refused(r.model("gil_plan_request_interview", json!({"reason":"   "})), "blank reason");
    ok(r.model("gil_plan_request_interview", json!({"reason":"항목 D가 B 다음에 필요하다"})), "request interview");
    ok(r.model("gil_plan_request_interview", json!({"reason":"항목 D가 B 다음에 필요하다"})), "request again");
    assert_eq!(r.read()["reinterview_requests"].as_array().unwrap().len(), 1);
    refused(r.model("gil_open", json!({"kind":"interview"})), "interview while Define is open");
    ok(r.model("gil_close", json!({"report":"problem: 항목 B을 합성 자료로 확인한다\nsuccess_condition: evidence 파일에 결과가 저장된다\n"})), "close define");
    ok(r.model("gil_open", json!({"kind":"hypothesis","contract":CONTRACT})), "hypothesis");
    ok(r.model("gil_close", json!({"report":"hypothesis: 합성 자료만으로 항목 B을 확인할 수 있다\nrationale: 자료가 제공됐다\nguardrail: 외부 데이터를 쓰지 않는다\n"})), "close hypothesis");
    let v = ok(r.model("gil_open", json!({"kind":"verify","contract":CONTRACT})), "verify");
    let verify = step_ref(&v, "verify");
    r.finish_experiment(c3, "B", &verify, &document);
    let read = r.read();
    assert_eq!(read["reinterview_requests"].as_array().unwrap().len(), 1, "{read}");
    assert!(read["pending"].as_array().unwrap().is_empty());
    let context = r.context_readonly();
    assert!(context.contains("gil_open(kind: interview)"), "{context}");
    assert!(!context.contains("현재 Cycle의 남은 Step을 실제 순서대로 마치고"), "closed Cycle still directs old steps: {context}");
    refused(r.model("gil_open", json!({"kind":"experiment"})), "experiment while reinterview pending");
    let opened = ok(r.model("gil_open", json!({"kind":"interview"})), "open interview");
    let c4 = cycle_number(&opened);

    // request_changes → approved:no → question; re-proposal keeps approved items; re-approval
    let (basis, _) = r.ask_and_interpret();
    let p3 = ok(r.model("gil_plan_propose", json!({"draft":draft(&["A","B","D"], &basis, &[])})), "propose rev3");
    assert_eq!(structured(&p3)["proposal"]["revision"], 3);
    assert_eq!(structured(&p3)["proposal"]["base_revision"], 2);
    let review = ok(r.app("gil_plan_review", json!({})), "review rev3");
    let t3 = token(&review).unwrap();
    let d3 = structured(&p3)["proposal"]["digest"].clone();
    ok(r.app("gil_plan_decide", json!({"revision":3,"digest":d3,"token":t3,"decision":"request_changes"})), "request_changes");
    let read = r.read();
    let report = read["close_report"].as_str().unwrap().to_owned();
    // close_report uses the block form for every field.
    assert!(report.contains("approved: |\n  no\n"), "{report}");
    let forged = report.replace("approved: |\n  no\n", "approved: |\n  yes\n");
    assert_ne!(forged, report);
    refused(r.model("gil_close", json!({"report":forged})), "forged approved:yes");
    refused(r.model("gil_open", json!({"kind":"experiment"})), "experiment with Synthesis open");
    let closed = ok(r.model("gil_close", json!({"report":report})), "close with approved:no");
    assert!(said(&closed).contains("question"), "{closed}");
    refused(r.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})), "outcome after approved:no");
    let (basis, synthesis4) = r.ask_and_interpret();
    let mut altered = draft(&["A","B","D"], &basis, &[]); altered["items"][1]["done_when"] = json!("바뀐 조건");
    refused(r.model("gil_plan_propose", json!({"draft":altered})), "altered approved item");
    refused(r.model("gil_plan_propose", json!({"draft":draft(&["A","D"], &basis, &[])})), "removed approved item");
    let p4 = ok(r.model("gil_plan_propose", json!({"draft":draft(&["A","B","D"], &basis, &[])})), "propose rev4");
    assert_eq!(structured(&p4)["proposal"]["revision"], 4);
    refused(r.app("gil_plan_decide", json!({"revision":3,"digest":d3,"token":t3,"decision":"approve"})), "approve superseded rev3");
    r.approve_via_app(4);
    r.close_approved_interview(&synthesis4, "open_child");
    let read = r.read();
    assert_eq!(read["active_revision"], 4);
    assert_eq!(read["pending"].as_array().unwrap().iter().map(|i| i["id"].as_str().unwrap()).collect::<Vec<_>>(), ["D"], "{read}");
    assert!(read["reinterview_requests"].as_array().unwrap().is_empty());
    let _ = c4;

    // inserted item D executes under revision 4; the stale revision-2 document is refused
    let (c5, document4) = r.open_experiment("D");
    assert!(document4.contains("Revision: 4"));
    let verify = r.walk_to_verify("D");
    std::fs::write(root.join(format!("evidence/C{c5}-D.md")), "pre").unwrap();
    refused(r.c.call("gil_close", verify_args(&root, &verify, &format!("execution: 확인했다\nresult: 저장했다\nplan_document: plan/approved-plan.md\nevidence_files: evidence/C{c5}-D.md\n"))), "stale revision-2 document");
    r.finish_experiment(c5, "D", &verify, &document4);
    let read = r.read();
    assert_eq!(read["executed_cycles"].as_array().unwrap().iter().map(|e| format!("{}@{}", e["item_id"].as_str().unwrap(), e["revision"])).collect::<Vec<_>>(), ["A@2", "B@2", "D@4"]);

    // Closing Interview needs its own approval; Chain closes; next Chain cannot reuse it
    ok(r.model("gil_open", json!({"kind":"interview"})), "open closing interview");
    let (basis, synthesis6) = r.ask_and_interpret();
    let closing = draft(&["A","B","D"], &basis, &[("chain_verdict", "success")]);
    let p5 = ok(r.model("gil_plan_propose", json!({"draft":closing})), "propose closing");
    assert_eq!(structured(&p5)["proposal"]["revision"], 5);
    refused(r.model("gil_close", json!({"report":format!("statement: 합성 자료로 항목을 순서대로 확인하고 파일 증거를 남긴다\nbasis_refs: {}\nexploration_checklist: |\n  A\n  B\n  D\nchain_verdict: success\napproved: yes\n", basis.join("\n"))})), "bare approved:yes on closing synthesis");
    let t5 = r.approve_via_app(5);
    r.close_approved_interview(&synthesis6, "close_chain");
    refused(r.model("gil_open", json!({"kind":"experiment"})), "experiment while Chain Report is due");
    let chain = ok(r.model("gil_close", json!({"report":format!("verdict: success\nclosing_synthesis_ref: {synthesis6}\nhandoff_summary: 합성 항목 A,B,D를 파일 증거와 함께 완료했다\n")})), "close chain");
    assert!(said(&chain).starts_with("닫았다: chain:"), "{chain}");
    refused(r.model("gil_open", json!({"kind":"experiment"})), "experiment after Chain closed");
    let next = ok(r.model("gil_open", json!({"kind":"chain"})), "open next chain");
    assert!(said(&next).contains("Opening Interview"), "{next}");
    let read = r.read();
    assert_eq!(read["supported"], true);
    assert!(read["proposal"].is_null() && read["active_revision"].is_null() && read["pending"].as_array().unwrap().is_empty() && read["executed_cycles"].as_array().unwrap().is_empty(), "{read}");
    let (basis, _) = r.ask_and_interpret();
    let px = ok(r.model("gil_plan_propose", json!({"draft":draft(&["X"], &basis, &[])})), "propose in new chain");
    assert_eq!(structured(&px)["proposal"]["revision"], 1);
    assert!(structured(&px)["proposal"]["base_revision"].is_null());
    refused(r.app("gil_plan_decide", json!({"revision":1,"digest":structured(&p5)["proposal"]["digest"],"token":t5,"decision":"approve"})), "old chain digest/capability");
    r.approve_via_app(1);

    // clean shutdown
    let Rig { c, .. } = r;
    let (status, noise) = c.eof();
    assert!(status.success(), "{status:?}");
    assert!(noise.is_empty(), "non-JSON stdout: {noise:?}");
    std::fs::remove_dir_all(root).ok();
    std::fs::remove_dir_all(settings).ok();
}
