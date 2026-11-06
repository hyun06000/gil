//! Synthetic App consent, actual product-profile stdio and on-disk records; no user project.
use super::*;
use std::fs;

fn failed(label: &str, protect: Option<&str>) -> (Rig, std::path::PathBuf) {
    let root = scratch(label);
    let settings = scratch(&format!("{label}-settings"));
    fs::write(root.join("report.txt"), "baseline report").unwrap();
    if let Some(path) = protect {
        fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
        fs::write(root.join(path), "original protected content").unwrap();
        if path.starts_with("private/") {
            fs::write(root.join(".gitignore"), "private/\n").unwrap();
        }
    }
    let c = Client::serve(&settings);
    let record = product_state_path(&root, &private_local_app_data(&settings));
    let mut r = Rig {
        root,
        record,
        scope: String::new(),
        c,
    };
    ok(r.model("gil_start", json!({})), "start");
    let prepared = ok(r.model("gil_monitor_prepare", json!({})), "prepare");
    r.scope = structured(&prepared)["scope_id"].as_str().unwrap().into();
    let (basis, synthesis) = r.ask_and_interpret();
    ok(
        r.model(
            "gil_plan_propose",
            json!({"draft":draft(&["A","B"], &basis, &[])}),
        ),
        "propose",
    );
    r.approve_via_app(1);
    r.close_approved_interview(&synthesis, "open_child");
    fail_attempt(&mut r, protect);
    (r, settings)
}
fn fail_attempt(r: &mut Rig, protect: Option<&str>) {
    let (n, document) = r.open_experiment("A");
    let verify = r.walk_to_verify("A");
    fs::write(r.root.join("plan.md"), document).unwrap();
    fs::write(
        r.root.join("evidence.md"),
        format!("Attempt {n}: incomplete daily source data; cannot calculate volatility."),
    )
    .unwrap();
    fs::write(r.root.join("report.txt"), format!("partial attempt {n}")).unwrap();
    if let Some(path) = protect {
        fs::write(r.root.join(path), "changed protected content").unwrap();
    }
    ok(r.c.call("gil_close", verify_args(&r.root,&verify,"execution: 合成資料を調査\nresult: source incomplete\nevidence_files: evidence.md\nplan_document: plan.md\n")), "verify");
    ok(
        r.model("gil_open", json!({"kind":"analysis","contract":CONTRACT})),
        "analysis",
    );
    ok(r.model("gil_close", json!({"report":"hypothesis_fit: no\nproblem_solved: no\nsuccess_condition_met: no\nguardrail_triggered: no\ninterpretation: need complete data\n"})), "close analysis");
    let o = ok(
        r.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})),
        "outcome",
    );
    let outcome = step_ref(&o, "outcome");
    ok(r.model("gil_close", json!({"report":"verdict: failure\nlesson: verify source completeness before calculating\nnext_direction.action: close_cycle\nnext_direction.reason: retry with full source\n"})), "failure outcome");
    ok(r.model("gil_close", json!({"report":format!("verdict: failure\noutcome_ref: {outcome}\nhandoff_summary: incomplete source preserved\nnext_direction.action: revisit\nnext_direction.target_cycle_ref: cycle:C1\nnext_direction.reason: retry with complete data\n")})), "failure cycle");
}
fn review(r: &mut Rig) -> (Value, String) {
    let response = ok(r.app("gil_revisit_review", json!({})), "recovery review");
    let token = response["result"]["_meta"]["gil/revisitApprovalToken"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(!response["result"]["content"].to_string().contains(&token));
    assert!(!structured(&response).to_string().contains(&token));
    (structured(&response).clone(), token)
}
fn apply(r: &mut Rig, value: &Value, token: &str) -> Value {
    r.app(
        "gil_revisit_apply",
        json!({"digest":value["digest"],"token":token}),
    )
}
fn tree(at: &Path) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
    fn visit(
        base: &Path,
        at: &Path,
        out: &mut std::collections::BTreeMap<std::path::PathBuf, Vec<u8>>,
    ) {
        for entry in fs::read_dir(at).unwrap() {
            let p = entry.unwrap().path();
            if p.is_dir() {
                visit(base, &p, out);
            } else {
                out.insert(
                    p.strip_prefix(base).unwrap().to_owned(),
                    fs::read(p).unwrap(),
                );
            }
        }
    }
    let mut out = Default::default();
    visit(at, at, &mut out);
    out
}

#[test]
fn old_failure_requires_exact_app_consent_preserves_evidence_and_retries_twice() {
    let (mut r, settings) = failed("recovery-two-retries", None);
    let original = record_bytes(&r.record);
    let objects = tree(&r.record.parent().unwrap().join("artifacts"));
    refused(r.model("gil_revisit", json!({})), "no approval");
    refused(
        r.model("gil_open", json!({"kind":"experiment"})),
        "no implicit bypass",
    );
    #[cfg(all(target_os = "macos", feature = "visual-authoring"))]
    conversation_and_files_at_recovery(&mut r, &settings);
    let (v, t) = review(&mut r);
    assert_eq!(original, record_bytes(&r.record));
    assert!(
        !r.record
            .parent()
            .unwrap()
            .join("revisit-preserved")
            .exists()
    );
    refused(apply(&mut r, &v, "invented"), "forged capability");
    let mut stale = v.clone();
    stale["digest"] = json!("old");
    refused(apply(&mut r, &stale, &t), "stale digest");
    fs::write(r.root.join("unrecorded.txt"), "user edit").unwrap();
    refused(apply(&mut r, &v, &t), "changed after preview");
    assert_eq!(original, record_bytes(&r.record));
    fs::remove_file(r.root.join("unrecorded.txt")).unwrap();
    let done = ok(apply(&mut r, &v, &t), "approved recovery");
    assert_eq!(structured(&done)["state"], "completed");
    #[cfg(all(target_os = "macos", feature = "visual-authoring"))]
    conversation_and_files_at_recovery(&mut r, &settings);
    // The chat continuation must re-read Core, not repeat the pre-click approval wait.
    let resumed_context = r.context_readonly();
    let next = resumed_context.split("[지금 바로 할 호출]\n").nth(1).unwrap().lines().next().unwrap();
    assert!(next.contains("되돌아왔다") && next.contains("gil_open"), "{next}");
    assert!(!next.contains("버튼 승인을 기다린다"), "{next}");
    assert_eq!(
        fs::read_to_string(r.root.join("report.txt")).unwrap(),
        "baseline report"
    );
    assert!(!r.root.join("evidence.md").exists());
    assert_eq!(objects, tree(&r.record.parent().unwrap().join("artifacts")));
    let old: Value = serde_norway::from_slice(&original).unwrap();
    let new: Value = serde_norway::from_slice(&record_bytes(&r.record)).unwrap();
    for field in [
        "format",
        "next_will_id",
        "current_existence_ref",
        "existences",
        "existence_states",
        "artifacts",
        "visuals",
    ] {
        assert_eq!(old[field], new[field], "revisit changed preserved {field}");
    }
    assert_eq!(
        old["chains"]["nodes"][0]["approved_plan"],
        new["chains"]["nodes"][0]["approved_plan"]
    );
    assert_eq!(
        old["chains"]["nodes"][0]["cycles"]["nodes"],
        new["chains"]["nodes"][0]["cycles"]["nodes"]
    );
    let archive = r.root.join(v["archive"].as_str().unwrap());
    let mappings: Value =
        serde_json::from_slice(&fs::read(archive.join("files.json")).unwrap()).unwrap();
    let evidence = mappings
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["path"] == "evidence.md")
        .unwrap();
    assert!(
        fs::read_to_string(archive.join(evidence["saved_as"].as_str().unwrap()))
            .unwrap()
            .contains("incomplete daily")
    );
    // Same approved operation is idempotent; then restart and retry the same plan item.
    ok(apply(&mut r, &v, &t), "duplicate approval");
    r.c = Client::serve(&settings);
    let read = r.read();
    assert_eq!(read["pending"][0]["id"], "A");
    assert_eq!(read["executed_cycles"].as_array().unwrap().len(), 1);
    fail_attempt(&mut r, None);
    let (second, t2) = review(&mut r);
    ok(apply(&mut r, &second, &t2), "second recovery");
    let (n, doc) = r.open_experiment("A");
    let verify = r.walk_to_verify("A");
    r.finish_experiment(n, "A", &verify, &doc);
    let read = r.read();
    assert_eq!(read["executed_cycles"].as_array().unwrap().len(), 3);
    assert_eq!(read["pending"][0]["id"], "B");
    let status = r.context_readonly();
    assert!(
        status.contains("incomplete source") || status.contains("verify source completeness"),
        "{status}"
    );
    assert_eq!(
        objects
            .iter()
            .filter(|(path, bytes)| fs::read(
                r.record.parent().unwrap().join("artifacts").join(path)
            )
            .ok()
            .as_ref()
                == Some(bytes))
            .count(),
        objects.len()
    );
}

#[test]
fn protected_changes_refuse_without_touching_state_or_files() {
    for (i, path) in [
        ".env",
        ".ENV",
        ".git/HEAD",
        ".GIT/HEAD",
        "node_modules/cache.js",
        "private/report.txt",
    ]
    .iter()
    .enumerate()
    {
        let (mut r, _) = failed(&format!("recovery-protected-{i}"), Some(path));
        let state = record_bytes(&r.record);
        let world = fs::read(r.root.join(path)).unwrap();
        refused(r.app("gil_revisit_review", json!({})), "protected changes");
        assert_eq!(state, record_bytes(&r.record));
        assert_eq!(world, fs::read(r.root.join(path)).unwrap());
        assert!(
            !r.record
                .parent()
                .unwrap()
                .join("revisit-preserved")
                .exists()
        );
    }
}

#[test]
fn approved_recovery_survives_state_save_then_process_death_during_restore() {
    let (mut r, settings) = failed("recovery-crash", None);
    r.c = Client::serve_with_env(
        &settings,
        &[("GIL_REVISIT_FAIL", "recovery-after-state-save")],
    );
    let (v, t) = review(&mut r);
    refused(
        apply(&mut r, &v, &t),
        "injected failure after durable state",
    );
    assert_eq!(
        fs::read_to_string(r.root.join("report.txt")).unwrap(),
        "partial attempt 2"
    );
    // The exact MCP transport resumes only a durable approval, then dies after a real write.
    r.c = Client::serve_with_env(&settings, &[("GIL_RESTORE_CRASH", "after-first-apply")]);
    r.c.call_expecting_crash("gil_revisit", json!({"project_root":r.root}));
    assert!(
        r.record
            .parent()
            .unwrap()
            .join("restore/active/PLAN")
            .is_file(),
        "a real transaction must remain after abort"
    );
    r.c = Client::serve(&settings);
    ok(
        r.model("gil_revisit", json!({})),
        "resume durable approval after death",
    );
    assert_eq!(
        fs::read_to_string(r.root.join("report.txt")).unwrap(),
        "baseline report"
    );
    assert!(
        r.root
            .join(v["archive"].as_str().unwrap())
            .join("approval.json")
            .is_file()
    );
    let (n, _) = r.open_experiment("A");
    assert_eq!(n, 3);
}

#[test]
fn server_restart_invalidates_private_consent_and_corrupt_evidence_refuses() {
    let (mut r, settings) = failed("recovery-token-restart", None);
    let (v, t) = review(&mut r);
    let state = record_bytes(&r.record);
    r.c = Client::serve(&settings);
    refused(apply(&mut r, &v, &t), "old process token");
    let (_, fresh) = review(&mut r);
    assert_ne!(fresh, t);
    // The immutable failed evidence must be readable, not merely a valid graph reference.
    let blob = fs::read_dir(r.record.parent().unwrap().join("artifacts/blobs/sha256"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let file = fs::read_dir(blob).unwrap().next().unwrap().unwrap().path();
    let bytes = fs::read(&file).unwrap();
    fs::write(&file, b"corrupt").unwrap();
    refused(apply(&mut r, &v, &fresh), "corrupt immutable evidence");
    assert_eq!(state, record_bytes(&r.record));
    assert!(
        !r.record
            .parent()
            .unwrap()
            .join("revisit-preserved")
            .exists()
    );
    fs::write(file, bytes).unwrap();
}

#[test]
fn preservation_area_symlink_cannot_redirect_recovery_writes() {
    let (mut r, _) = failed("recovery-archive-link", None);
    let (v, t) = review(&mut r);
    let state = record_bytes(&r.record);
    let outside = scratch("recovery-outside");
    fs::write(outside.join("sentinel"), "untouched").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        &outside,
        r.record.parent().unwrap().join("revisit-preserved"),
    )
    .unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(
        &outside,
        r.record.parent().unwrap().join("revisit-preserved"),
    )
    .unwrap();
    refused(apply(&mut r, &v, &t), "no follow archive boundary");
    assert_eq!(state, record_bytes(&r.record));
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
    assert_eq!(
        fs::read_to_string(r.root.join("report.txt")).unwrap(),
        "partial attempt 2"
    );
}

#[test]
fn failed_item_can_be_reinterviewed_and_rebound_to_the_new_approved_revision() {
    let (mut r, _) = failed("recovery-reinterview", None);
    let (v, t) = review(&mut r);
    ok(apply(&mut r, &v, &t), "restore before reinterview");
    ok(
        r.model("gil_open", json!({"kind":"interview"})),
        "new interview branch",
    );
    let (basis, synthesis) = r.ask_and_interpret();
    ok(
        r.model(
            "gil_plan_propose",
            json!({"draft":draft(&["A","B"],&basis,&[])}),
        ),
        "repropose",
    );
    r.approve_via_app(2);
    r.close_approved_interview(&synthesis, "open_child");
    let (n, doc) = r.open_experiment("A");
    let bindings = r.read()["executed_cycles"].clone();
    assert_eq!(bindings[0]["revision"], 1);
    assert_eq!(bindings[1]["revision"], 2);
    let verify = r.walk_to_verify("A");
    r.finish_experiment(n, "A", &verify, &doc);
    assert_eq!(r.read()["pending"][0]["id"], "B");
}

#[cfg(all(target_os = "macos", feature = "visual-authoring"))]
fn conversation_and_files_at_recovery(r: &mut Rig, settings: &Path) {
    let before = record_bytes(&r.record);
    let artifacts = tree(&r.root);
    let context = ok(r.model("gil_context", json!({})), "resume");
    auto_guard(settings, json!({"hook_event_name":"PostToolUse","tool_name":"mcp__gil__gil_context",
        "tool_input":{"project_root":r.root},"tool_response":context["result"]}));
    // Neither recovery approval nor the recovered cycle boundary needs a dummy Question.
    refused(r.model("gil_await_user", json!({"question":"Approve recovery?"})), "optional check unsupported here");
    for _ in 0..10 {
        assert_eq!(auto_guard(settings, json!({"hook_event_name":"Stop","stop_hook_active":true,
            "last_assistant_message":"Here is the requested chat-only report"})), json!({}));
        assert_eq!(auto_guard(settings, json!({"hook_event_name":"PreToolUse","tool_name":"mcp__ccd_turn__wrap_up",
            "tool_input":{"ask":"Waiting for your decision"}})), json!({}));
    }
    assert_eq!(auto_guard(settings, json!({"hook_event_name":"UserPromptSubmit","prompt":"An unrelated question"})), json!({}));
    let deny = auto_guard(settings, json!({"hook_event_name":"PreToolUse","tool_name":"Write",
        "tool_input":{"file_path":r.root.join("report.txt")}}));
    assert_eq!(deny, json!({})); // Retired Host hooks do not enforce file policy.
    assert_eq!(before, record_bytes(&r.record));
    assert_eq!(artifacts, tree(&r.root));
}
