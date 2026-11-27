//! An unattended session over the real `gil mcp --serve` process: no Monitor, no person.
//!
//! The runner chooses the mode with the process environment (`GIL_MCP_DISPLAY=headless` and
//! `GIL_MCP_APPROVAL=unattended`). What this file fixes:
//!
//! - the tool list carries no display tool and no App tool, and the two runner tools are marked
//!   for the runner; a server started without the choice does not have them at all;
//! - a plan proposal and (tracked-scope builds) a start proposal — the two things a user decides —
//!   are answered through the runner tools for the exact digest read, and every such record says
//!   `unattended_runner`, never a Monitor button;
//! - the model's receipts say what waits for the user, and a wrong digest records nothing;
//! - going back after a failure waits for nobody: the agent's own gil_revisit carries it out;
//! - the choice without a headless display serves nothing.
//! - a tracked-scope project goes back inside its approved scope: tracked files return to the target
//!   Cycle's world while excluded paths, a link inside an excluded folder and the policy file stay
//!   as they are, and a recovery cut by process death rolls back and resumes.
//!
//! Not covered: a recovery after the tracked scope changed (refused by the Core; no test here).
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
    // The two bodies are objects of string fields on the wire, in the list and in every refusal's skeleton.
    for (name, field) in [("gil_open", "contract"), ("gil_close", "report")] {
        let tool = tools.iter().find(|tool| tool["name"] == name).unwrap();
        let property = &tool["inputSchema"]["properties"][field];
        // A Report field that asks for true or false takes a JSON boolean; a contract is strings only.
        let values = if field == "report" { json!({"type":["string","boolean"]}) } else { json!({"type":"string"}) };
        assert_eq!((property["type"].as_str(), &property["additionalProperties"]), (Some("object"), &values), "{name}.{field}");
        assert!(!tool["description"].as_str().unwrap().contains("STRING"), "{name} still describes a text body");
    }
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
fn a_plan_is_answered_by_the_runner_and_the_agent_goes_back_by_itself() {
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

    // Going back is the agent's own act: nothing waits for the user, and the runner has no answer to give.
    assert_eq!(pending(&mut r), Vec::<Value>::new());
    rpc_error(decide(&mut r, "revisit", &json!("0".repeat(64)), "approve"), "a recovery is not a subject the runner answers");
    let context = r.context();
    assert!(context.contains("gil_revisit") && !context.contains("버튼"), "{context}");
    let root = r.root.clone();
    let done = ok(revisit(&mut r.c, &root), "the agent goes back");
    let plan = structured(&done).clone();
    assert!(awaiting(&done).is_null(), "{done}");
    assert_eq!((plan["state"].as_str(), plan["from"].as_str(), plan["target"].as_str(), plan["approval_source"].as_str()),
        (Some("completed"), Some("cycle:C3"), Some("cycle:C2"), Some("agent_revisit")), "{done}");

    // The world is C2's again, the failed files are preserved, and the source record names no click.
    assert_eq!(world(&r.root), world_at_c2);
    let preserved = PathBuf::from(plan["archive"].as_str().unwrap());
    let source: Value = serde_json::from_slice(&fs::read(preserved.join("source.json")).unwrap()).unwrap();
    assert_eq!((source["approval_source"].as_str(), source["human_click"].as_bool()), (Some("agent_revisit"), Some(false)), "{source}");
    // The same call again finds the recovery done and changes nothing.
    let again = ok(revisit(&mut r.c, &root), "the same revisit again");
    assert_eq!(structured(&again)["state"], "completed", "{again}");
    assert_eq!(world(&r.root), world_at_c2);

    // The model continues from the recorded state: the next Experiment branches from the failure.
    assert_eq!(r.open_experiment("R1"), 4);
    assert_eq!(r.read()["current_experiment"]["revisit_from"], "cycle:C3");
    assert_eq!(r.experiments("R1")[..2], [("cycle:C2".to_owned(), "success".to_owned()), ("cycle:C3".to_owned(), "failure".to_owned())]);
}

#[cfg(all(feature = "tracking-policy", unix))]
#[test]
fn a_tracked_scope_start_is_approved_by_the_runner_and_recorded_as_unattended() {
    let mut r = rig("unattended-start", &[("app.py", "print('hi')\n")], json!({"start_kind":"tracked_scope", "locale":"en"}));
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
    // The language named on gil_start holds through the preparation and after the start, in this
    // process and in the next one: nothing falls back to the default language in between.
    let hangul = |text: &str| text.chars().any(|c| ('가'..='힣').contains(&c));
    // (The recommendation catalog inside the proposal is still written in Korean in the Core itself;
    // that text is not from the language table and is not what this checks.)
    for (what, frame) in [("prepare", &prepared), ("status", &status)] {
        let said = format!("{} {}", structured(frame)["said"].as_str().unwrap_or_default(), structured(frame)["next"].as_str().unwrap_or_default());
        assert!(!hangul(&said), "{what} does not speak the named language: {said}");
    }
    assert!(!hangul(&r.context()), "the started project speaks the named language");
    assert_eq!(structured(&status)["history"][0]["response"], json!({"decision":"approve","source":"unattended_runner"}));
    assert!(awaiting(&status).is_null(), "{status}");
    assert_eq!(fs::read_to_string(r.root.join(".gilignore")).unwrap(), rules, "the approved bytes are the policy file");
    // The started project reads back in a new process: the stored approval mark is accepted on load.
    let (exit, noise) = std::mem::replace(&mut r.c, Client::serve_with_env(&r.settings, UNATTENDED)).eof();
    assert!(exit.success() && noise.is_empty(), "{exit:?} {noise:?}");
    let context = r.context();
    assert!(context.contains("cycle:C1") && !hangul(&context), "{context}");
}

/// A tracked-scope project goes back inside its approved scope: the tracked files return to the
/// target Cycle's world, and nothing the policy excludes — a dependency folder with a link in it,
/// a secret file, the policy file itself — is created, changed or removed.
#[cfg(all(feature = "tracking-policy", unix))]
const RULES: &str = "node_modules/\n.env\n";

/// A tracked-scope project after a successful C2 and a failed C3 that also touched excluded paths.
#[cfg(all(feature = "tracking-policy", unix))]
fn tracked_failure(label: &str) -> (Rig, std::collections::BTreeMap<String, Vec<u8>>) {
    let rules = RULES;
    let mut r = rig(label, &[("app.py", "print('hi')\n"), ("node_modules/pkg/index.js", "module.exports = 1\n"), (".env", "KEY=first\n")],
        json!({"start_kind":"tracked_scope"}));
    std::os::unix::fs::symlink("../pkg/index.js", { fs::create_dir_all(r.root.join("node_modules/.bin")).unwrap(); r.root.join("node_modules/.bin/pkg") }).unwrap();
    ok(r.model("gil_start_prepare", json!({"rules":rules})), "prepare");
    let waits = pending(&mut r);
    ok(decide(&mut r, "start", &waits[0]["digest"], "approve"), "runner approves the start");
    interview(&mut r, &["R1"]);

    // C2 succeeds and leaves a tracked file. C3 changes tracked files, also touches excluded ones, and fails.
    let experiment = |r: &mut Rig, files: &[(&str, &str)], evidence: &str, fails_back_to: Option<&str>| {
        let n = r.open_experiment("R1");
        r.open_verify("R1", "합의한 형식으로 출력하는가?");
        for (path, bytes) in files { r.write(path, bytes); }
        let failed = fails_back_to.is_some();
        ok(r.model("gil_close", json!({"report":format!("execution: 합성 자료를 관측했다\nresult: 관측을 남겼다\nplan_document: {PLAN_DOCUMENT}\nevidence_files: {evidence}\n")})), "close verify");
        ok(r.model("gil_open", json!({"kind":"analysis","contract":CONTRACT})), "analysis");
        ok(r.model("gil_close", json!({"report":analysis_report(failed)})), "close analysis");
        let outcome = step_ref(&ok(r.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})), "outcome"), "outcome");
        ok(r.model("gil_close", json!({"report":outcome_report(failed)})), "close outcome");
        ok(r.model("gil_close", json!({"report":cycle_report(n, &outcome, fails_back_to)})), "close experiment");
        n
    };
    assert_eq!(experiment(&mut r, &[("evidence/C2.md", "형식이 맞다\n"), ("app.py", "print('second')\n")], "evidence/C2.md", None), 2);
    let world_at_c2 = world(&r.root);
    assert_eq!(experiment(&mut r, &[("evidence/C3.md", "형식이 깨졌다\n"), ("app.py", "broken\n"), ("lib/new.py", "x = 1\n"),
        (".env", "KEY=changed-during-c3\n"), ("node_modules/pkg/index.js", "module.exports = 3\n"), ("node_modules/added.js", "3\n")], "evidence/C3.md", Some("cycle:C2")), 3);
    (r, world_at_c2)
}

#[cfg(all(feature = "tracking-policy", unix))]
#[test]
fn a_tracked_scope_project_goes_back_inside_its_approved_scope_and_leaves_excluded_paths_alone() {
    let (mut r, world_at_c2) = tracked_failure("unattended-tracked-recovery");
    let rules = RULES;

    let root = r.root.clone();
    let done = ok(revisit(&mut r.c, &root), "the agent goes back");
    let plan = structured(&done).clone();
    assert_eq!((plan["state"].as_str(), plan["from"].as_str(), plan["target"].as_str(), plan["policy"].as_str()), (Some("completed"), Some("cycle:C3"), Some("cycle:C2"), Some("approved-tracking-policy-scope-v1")), "{plan}");
    let mut listed: Vec<(&str, &str)> = plan["changes"].as_array().unwrap().iter().map(|c| (c["path"].as_str().unwrap(), c["action"].as_str().unwrap())).collect();
    listed.sort();
    assert_eq!(listed, [("app.py", "replace"), ("evidence/C3.md", "preserve"), ("lib/new.py", "preserve")], "only tracked paths are in the plan");

    // Tracked files are C2's again. Excluded paths keep what C3 left there; the policy file is untouched.
    let now = world(&r.root);
    assert_eq!(now["app.py"], b"print('second')\n");
    assert!(!now.contains_key("evidence/C3.md") && !now.contains_key("lib/new.py") && now.contains_key("evidence/C2.md"));
    assert_eq!(now[".env"], b"KEY=changed-during-c3\n");
    assert_eq!(now["node_modules/pkg/index.js"], b"module.exports = 3\n");
    assert_eq!(now["node_modules/added.js"], b"3\n");
    assert_eq!(now[".gilignore"], rules.as_bytes());
    assert!(fs::symlink_metadata(r.root.join("node_modules/.bin/pkg")).unwrap().file_type().is_symlink());
    for (path, bytes) in &world_at_c2 {
        if !path.starts_with("node_modules/") && path != ".env" { assert_eq!(&now[path], bytes, "{path}"); }
    }
    // The replaced and removed tracked files are preserved; excluded files were never copied.
    let preserved = PathBuf::from(plan["archive"].as_str().unwrap());
    let mapping: Value = serde_json::from_slice(&fs::read(preserved.join("files.json")).unwrap()).unwrap();
    let saved: Vec<&str> = mapping.as_array().unwrap().iter().map(|m| m["path"].as_str().unwrap()).collect();
    assert!(saved.contains(&"app.py") && saved.contains(&"lib/new.py") && !saved.iter().any(|p| p.starts_with("node_modules/") || *p == ".env" || *p == ".gilignore"), "{saved:?}");
    // The Chain goes on: the next Experiment branches from the failure, in a new process too.
    let (exit, noise) = std::mem::replace(&mut r.c, Client::serve_with_env(&r.settings, UNATTENDED)).eof();
    assert!(exit.success() && noise.is_empty(), "{exit:?} {noise:?}");
    assert_eq!(r.open_experiment("R1"), 4);
    assert_eq!(r.read()["current_experiment"]["revisit_from"], "cycle:C3");
}

/// The process dies after the first file of an approved tracked-scope recovery was replaced. The
/// next process rolls the half-done restore back although the folder as a whole cannot be observed
/// (the excluded dependency folder holds a link), and the same approval is then resumed by the model.
#[cfg(all(feature = "tracking-policy", unix, debug_assertions))]
#[test]
fn a_tracked_scope_recovery_interrupted_by_process_death_rolls_back_and_resumes() {
    let (mut r, world_at_c2) = tracked_failure("unattended-tracked-crash");
    let world_at_c3 = world(&r.root);
    let crashing: Vec<(&str, &str)> = UNATTENDED.iter().copied().chain([("GIL_RESTORE_CRASH", "after-first-apply")]).collect();
    let (exit, noise) = std::mem::replace(&mut r.c, Client::serve_with_env(&r.settings, &crashing)).eof();
    assert!(exit.success() && noise.is_empty(), "{exit:?} {noise:?}");
    r.c.call_expecting_crash("gil_revisit", json!({"project_root":r.root}));
    assert_ne!(world(&r.root), world_at_c3, "the restore had started when the process died");

    r.c = Client::serve_with_env(&r.settings, UNATTENDED);
    // The started recovery is durable. The model's next gil_revisit first rolls the half-done restore
    // back — by the touched paths alone, since the whole folder cannot be observed — and then resumes it.
    let resumed = ok(r.model("gil_revisit", json!({})), "resume the approved recovery");
    assert!(awaiting(&resumed).is_null(), "{resumed}");
    assert_eq!(pending(&mut r), Vec::<Value>::new());
    let now = world(&r.root);
    assert_eq!(now["app.py"], world_at_c2["app.py"]);
    assert!(!now.contains_key("evidence/C3.md") && !now.contains_key("lib/new.py"));
    assert_eq!((now[".env"].as_slice(), now["node_modules/added.js"].as_slice(), now[".gilignore"].as_slice()), (&b"KEY=changed-during-c3\n"[..], &b"3\n"[..], RULES.as_bytes()));
    assert_eq!(r.open_experiment("R1"), 4);
}

/// A new Interview approves a second revision of the plan. The plan document GIL saved for the
/// first revision is its own file: the next Verify finds the second revision's document there.
#[test]
fn the_saved_plan_document_follows_the_revision_a_new_interview_approved() {
    let mut r = rig("unattended-plan-revision", &[("data/in.txt", "input\n")], direct_start());
    interview(&mut r, &["R1"]);
    let c2 = r.open_experiment("R1");
    r.run(c2, &Run { requirement: "R1", problem: "입력 파일이 있는가?", files: &[("evidence/C2.md", "있다\n")], evidence: "evidence/C2.md", fails_back_to: None });
    let first = fs::read_to_string(r.root.join(PLAN_DOCUMENT)).unwrap();

    // The list changes only in an Interview; the files stay as they are.
    ok(r.model("gil_open", json!({"kind":"interview"})), "a new Interview");
    interview(&mut r, &["R1", "R2"]);
    assert_eq!(fs::read_to_string(r.root.join(PLAN_DOCUMENT)).unwrap(), first, "an Interview changes no file");
    assert_eq!(r.read()["active_plan"]["revision"], 2);

    // open_verify itself checks that the saved file is the bound document, here revision 2's.
    let c4 = r.open_experiment("R2");
    r.run(c4, &Run { requirement: "R2", problem: "두 번째 항목을 확인할 수 있는가?", files: &[("evidence/C4.md", "확인했다\n")], evidence: "evidence/C4.md", fails_back_to: None });
    let second = fs::read_to_string(r.root.join(PLAN_DOCUMENT)).unwrap();
    assert!(second != first && second.contains("R2"), "the document of revision 2 replaced GIL's earlier save");
}
