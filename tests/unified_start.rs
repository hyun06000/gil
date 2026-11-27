//! One executable, two kinds of new project — the explicit `start_kind` selection over the **real
//! `gil mcp --serve` stdio process**, in a build that holds both the tracked-scope preparation and
//! the image-report creation.
//!
//! What this file fixes:
//!
//! - a folder with no `.gil` starts nothing until a kind is chosen, and an unknown kind starts nothing;
//! - `image_reports` creates the visual project at once (no preparation record), `tracked_scope`
//!   creates only a preparation (no state, no archive) until the App-only approval;
//! - neither request, nor a request without a kind, nor a fresh process, turns one kind into the
//!   other: the existing record is continued or reported and its bytes stay;
//! - leftovers whose origin cannot be told are preserved by every start request.
//!
//! Scope of the evidence: synthetic temporary projects and synthetic tool-path input. The App calls
//! are harness input, not a person's click, and nothing here shows a rendered Host screen. A build
//! with only one kind does not compile this file; its zero tests are not a pass.
#![cfg(all(feature = "tracking-policy", feature = "visual-authoring", unix))]
#[path = "support_wave1/mod.rs"]
mod support;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use support::*;

fn sc(frame: &Value) -> &Value {
    &frame["result"]["structuredContent"]
}

/// Every entry below `root`: folders as `None`, files by their bytes, links by their target.
fn tree(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    fn walk(root: &Path, at: &Path, out: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
        for entry in std::fs::read_dir(at).unwrap() {
            let path = entry.unwrap().path();
            let meta = std::fs::symlink_metadata(&path).unwrap();
            let rel = path.strip_prefix(root).unwrap().to_owned();
            if meta.is_symlink() {
                out.insert(rel, Some(std::fs::read_link(&path).unwrap().as_os_str().as_encoded_bytes().to_vec()));
            } else if meta.is_dir() {
                out.insert(rel, None);
                walk(root, &path, out);
            } else {
                out.insert(rel, Some(std::fs::read(&path).unwrap()));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn project(name: &str) -> (PathBuf, PathBuf, String) {
    let root = scratch(name).canonicalize().unwrap();
    std::fs::write(root.join("README.md"), "hello\n").unwrap();
    std::fs::create_dir_all(root.join("data")).unwrap();
    std::fs::write(root.join("data/raw.csv"), "1,2,3\n").unwrap();
    let settings = scratch(&format!("{name}-settings"));
    let r = root.display().to_string();
    (root, settings, r)
}

fn start(c: &mut Client, r: &str, kind: Option<&str>) -> Value {
    let mut args = json!({"project_root": r});
    if let Some(kind) = kind {
        args["start_kind"] = json!(kind);
    }
    c.call("gil_start", args)
}

#[test]
fn the_catalog_offers_both_kinds_and_names_no_storage_number_or_build_flag() {
    let (_root, settings, _r) = project("unified-catalog");
    let mut c = Client::serve(&settings);
    let listed = c.request("tools/list", json!({}));
    let tools = listed["result"]["tools"].as_array().unwrap();
    let start = tools.iter().find(|tool| tool["name"] == "gil_start").expect("gil_start");
    assert_eq!(start["inputSchema"]["properties"]["start_kind"]["enum"], json!(["tracked_scope", "image_reports"]));
    assert_eq!(start["inputSchema"]["required"], json!(["project_root"]), "the selection stays optional on the wire");
    assert_eq!(start["inputSchema"]["additionalProperties"], json!(false));
    assert!(start.get("outputSchema").is_none(), "one tool answers a preparation or a receipt: {start}");
    let description = start["description"].as_str().unwrap();
    for needed in ["tracked_scope", "image_reports", "never converted", "start_kind_required", "Monitor approval"] {
        assert!(description.contains(needed), "{needed}: {description}");
    }
    for hidden in ["format 9", "format 10", "format 11", "tracking-policy", "visual-authoring", "exploration-flow"] {
        assert!(!description.contains(hidden), "{hidden}: {description}");
    }
    // Both kinds keep their own tools in the same catalog.
    for name in ["gil_start_prepare", "gil_scope_propose", "gil_report_visuals", "gil_plan_propose"] {
        assert!(tools.iter().any(|tool| tool["name"] == name), "{name}");
    }
}

#[test]
fn an_empty_folder_starts_nothing_until_a_kind_is_chosen() {
    let (root, settings, r) = project("unified-choose");
    let mut c = Client::serve(&settings);
    let before = tree(&root);

    let asked = refused(start(&mut c, &r, None), "a start without a kind in an empty folder");
    assert_eq!(sc(&asked)["code"], "start_kind_required", "{asked}");
    assert_eq!(sc(&asked)["created"], false);
    let offered: Vec<&str> = sc(&asked)["start_kinds"].as_array().unwrap().iter().map(|kind| kind["value"].as_str().unwrap()).collect();
    assert_eq!(offered, vec!["tracked_scope", "image_reports"]);
    assert!(sc(&asked)["start_kinds"].as_array().unwrap().iter().all(|kind| kind["summary"].as_str().is_some_and(|text| text.len() > 40)));
    assert_eq!(tree(&root), before, "asking wrote nothing, not even a lock or a continuity note");

    let unknown = refused(start(&mut c, &r, Some("format_11")), "an unknown kind");
    assert_eq!(sc(&unknown)["code"], "start_kind_unknown", "{unknown}");
    rpc_error(c.call("gil_start", json!({"project_root": r, "start_kind": 7})), "a kind that is not a string");
    rpc_error(c.call("gil_start", json!({"project_root": r, "kind": "image_reports"})), "a misspelled input");
    assert_eq!(tree(&root), before, "no refusal started the other kind");

    // The same question again from a new process: still nothing, still the same answer.
    drop(c);
    let mut c = Client::serve(&settings);
    let again = refused(start(&mut c, &r, None), "the same request after a restart");
    assert_eq!(sc(&again)["code"], "start_kind_required");
    assert_eq!(tree(&root), before);
}

#[test]
fn image_reports_creates_the_visual_project_at_once_and_nothing_turns_it_into_a_tracked_one() {
    let (root, settings, r) = project("unified-images");
    let mut c = Client::serve(&settings);
    let started = ok(start(&mut c, &r, Some("image_reports")), "image-report start");
    assert_eq!(sc(&started)["ok"], true, "{started}");
    assert_eq!(sc(&started)["storage_format"], json!(gil::END_HISTORY_VISUAL_FORMAT), "{started}");
    assert_eq!(sc(&started)["next_call"]["name"], "show_gil_monitor", "the direct start hands the Monitor over: {started}");
    let state = std::fs::read(root.join(".gil/state.yaml")).unwrap();
    assert!(state.starts_with(format!("format: {}\n", gil::END_HISTORY_VISUAL_FORMAT).as_bytes()));
    assert!(!root.join(".gil/start").exists(), "no tracked-scope preparation was begun");
    assert!(!root.join(".gilignore").exists(), "no tracking policy file was written");
    let scope = sc(&started)["next_call"]["arguments"]["scope_id"].as_str().unwrap().to_owned();
    let shown = c.call("show_gil_monitor", json!({"scope_id": scope}));
    assert!(sc(&shown).get("view").is_some(), "a started project shows its graph: {shown}");
    assert!(sc(&shown).get("tracking").is_none_or(Value::is_null), "an image-report project has no tracking block: {shown}");

    let before = tree(&root);
    let tracked = refused(start(&mut c, &r, Some("tracked_scope")), "tracked scope over an image-report project");
    assert_eq!(sc(&tracked)["code"], "already_started", "{tracked}");
    refused(start(&mut c, &r, None), "a repeated start without a kind");
    refused(start(&mut c, &r, Some("image_reports")), "a repeated image-report start");
    // The preparation tools have nothing to continue here; they answer before taking the lock.
    let prepare = refused(c.call("gil_start_prepare", json!({"project_root": r, "rules": "data/\n"})), "a proposal over an image-report project");
    assert_eq!(sc(&prepare)["code"], "already_started", "{prepare}");
    refused(c.call("gil_start_cancel", json!({"project_root": r})), "a cancel over an image-report project");
    assert_eq!(tree(&root), before, "refusals changed nothing in the image-report project");
    // A scope change has no tracked scope to change here. It may open the project the ordinary way,
    // so only the record and the absence of any tracking file are compared.
    let scope_change = c.call("gil_scope_propose", json!({"project_root": r, "rules": "data/\n"}));
    assert!(scope_change.get("error").is_some() || !is_ok(&scope_change), "{scope_change}");
    assert_eq!(std::fs::read(root.join(".gil/state.yaml")).unwrap(), state);
    assert!(!root.join(".gil/start").exists() && !root.join(".gilignore").exists());

    // A new process reads the same project as the same kind.
    drop(c);
    let mut c = Client::serve(&settings);
    ok(c.call("gil_context", json!({"project_root": r})), "context after a restart");
    refused(start(&mut c, &r, Some("tracked_scope")), "tracked scope after a restart");
    assert_eq!(std::fs::read(root.join(".gil/state.yaml")).unwrap(), state);
    assert!(!root.join(".gil/start").exists());
}

#[test]
fn tracked_scope_archives_nothing_before_the_monitor_approval_and_nothing_turns_it_into_an_image_project() {
    let (root, settings, r) = project("unified-tracked");
    let mut c = Client::serve(&settings);
    let begun = start(&mut c, &r, Some("tracked_scope"));
    assert!(is_ok(&begun), "{begun}");
    assert_eq!(sc(&begun)["phase"], "preparing", "{begun}");
    let scope = sc(&begun)["scope_id"].as_str().unwrap().to_owned();
    assert!(!root.join(".gil/state.yaml").exists() && !root.join(".gil/artifacts").exists(), "a preparation is not a project");
    // The hint of a committed start ("this folder is now a GIL project") is not said about a preparation.
    let said = begun["result"]["content"].to_string();
    assert!(!said.contains("gil_open(kind: question)"), "a preparation was announced as a started project: {said}");

    // The other kind never replaces the preparation; the refusal is decided before any lock is taken.
    let before = tree(&root);
    let images = refused(start(&mut c, &r, Some("image_reports")), "image reports over an open preparation");
    assert_eq!(sc(&images)["code"], "start_kind_conflict", "{images}");
    assert_eq!(tree(&root), before);

    // Without a kind, and from a new process, the same preparation is continued.
    let ledger = std::fs::read(root.join(".gil/start/preparation.json")).unwrap();
    drop(c);
    let mut c = Client::serve(&settings);
    let resumed = start(&mut c, &r, None);
    assert!(is_ok(&resumed), "{resumed}");
    assert_eq!(sc(&resumed)["phase"], "preparing");
    assert_eq!(sc(&resumed)["scope_id"], json!(scope));
    assert_eq!(std::fs::read(root.join(".gil/start/preparation.json")).unwrap(), ledger, "continuing wrote no second preparation");
    assert!(!root.join(".gil/state.yaml").exists());

    // Proposal and the App-only approval: only now does a project exist.
    let prepared = c.call("gil_start_prepare", json!({"project_root": r, "rules": "data/\n"}));
    assert!(is_ok(&prepared), "{prepared}");
    let (revision, digest) = (sc(&prepared)["proposal"]["revision"].clone(), sc(&prepared)["proposal"]["digest"].clone());
    assert!(!root.join(".gil/state.yaml").exists() && !root.join(".gil/artifacts").exists(), "a proposal archives nothing");
    refused(start(&mut c, &r, Some("image_reports")), "image reports while a proposal waits");
    let review = c.call("gil_start_review", json!({"scope_id": scope}));
    let capability = review["result"]["_meta"]["gil/startApprovalToken"].as_str().expect("App-only capability").to_owned();
    let decided = c.call("gil_start_decide", json!({"scope_id": scope, "revision": revision, "digest": digest, "token": capability, "decision": "approve"}));
    assert_eq!(sc(&decided)["phase"], "started", "{decided}");
    let state = std::fs::read(root.join(".gil/state.yaml")).unwrap();
    assert!(state.starts_with(format!("format: {}\n", gil::TRACKING_FORMAT).as_bytes()), "a tracked-scope project, with no visual registry");
    assert_eq!(std::fs::read_to_string(root.join(".gilignore")).unwrap(), "data/\n");

    // After the start: every further start request reports the project and changes nothing.
    let images = refused(start(&mut c, &r, Some("image_reports")), "image reports over a tracked project");
    assert_eq!(sc(&images)["code"], "start_kind_conflict");
    let again = c.call("gil_start", json!({"project_root": r, "start_kind": "tracked_scope"}));
    assert!(!is_ok(&again), "{again}");
    let bare = c.call("gil_start", json!({"project_root": r}));
    assert!(!is_ok(&bare), "{bare}");
    assert_eq!(std::fs::read(root.join(".gil/state.yaml")).unwrap(), state);

    // A report of this project cannot take image attachments. The limit is on writing: a close that
    // carries visuals is refused before anything is read into the record, and the read of a report's
    // visuals answers truthfully that there are none. Neither converts the project.
    let attach = c.call("gil_close", json!({"project_root": r, "report": "verdict: success\n", "expected_owner": {"kind": "cycle", "ref": "cycle:C1"},
        "visuals": {"attachments": [], "uses": []}}));
    assert!(attach.get("error").is_some() || !is_ok(&attach), "{attach}");
    assert!(attach.to_string().contains("visual_upgrade_required"), "the refusal names the missing image-report storage: {attach}");
    // The refusal names the close this project does take. It must not ask for corrected visuals:
    // a model that was told to fix them resent the same close until its budget ended (2026-11-27).
    let said = attach.to_string();
    assert!(said.contains("{project_root, report}") && said.contains("visuals 를 고쳐 다시 보내도 같은 거절이다"), "{attach}");
    assert!(!said.contains("아래 이유대로 visuals"), "{attach}");
    let visuals = c.call("gil_report_visuals", json!({"project_root": r, "owner": {"kind": "cycle", "ref": "cycle:C1"}}));
    assert!(is_ok(&visuals), "{visuals}");
    assert_eq!(sc(&visuals)["visuals"]["uses"], json!([]), "a tracked-scope project holds no visuals: {visuals}");
    assert_eq!(std::fs::read(root.join(".gil/state.yaml")).unwrap(), state, "neither request converted the project");

    drop(c);
    let mut c = Client::serve(&settings);
    let status = c.call("gil_start_status", json!({"project_root": r}));
    assert_eq!(sc(&status)["phase"], "started", "{status}");
    ok(c.call("gil_context", json!({"project_root": r})), "context after a restart");
    assert_eq!(std::fs::read(root.join(".gil/state.yaml")).unwrap(), state, "reading after a restart converted nothing");
}

#[test]
fn leftovers_of_an_unknown_creation_are_preserved_by_every_start_request() {
    let (root, settings, r) = project("unified-remnant");
    std::fs::create_dir_all(root.join(".gil/artifacts/tmp")).unwrap();
    std::fs::write(root.join(".gil/artifacts/tmp/unfinished"), b"keep evidence").unwrap();
    let before = tree(&root);
    let mut c = Client::serve(&settings);
    let tracked = refused(start(&mut c, &r, Some("tracked_scope")), "tracked scope over unknown leftovers");
    assert_eq!(sc(&tracked)["code"], "start_record_unsettled", "{tracked}");
    refused(start(&mut c, &r, Some("image_reports")), "image reports over unknown leftovers");
    refused(start(&mut c, &r, None), "a start without a kind over unknown leftovers");
    for tool in ["gil_start_prepare", "gil_start_cancel"] {
        let mut args = json!({"project_root": r});
        if tool == "gil_start_prepare" { args["rules"] = json!("data/\n"); }
        let answer = refused(c.call(tool, args), "a preparation tool over unknown leftovers");
        assert_eq!(sc(&answer)["code"], "start_record_unsettled", "{answer}");
    }
    assert_eq!(tree(&root), before, "nothing was reaped, locked, adopted or created");
}

/// The interrupted bootstrap has an ordinary retry: a `.gil` that holds nothing, or only the lock
/// file, records nothing of either kind. The tracked-scope start begins there again; the
/// image-report creation still never creates into an existing `.gil`, and a start without a kind
/// still starts nothing.
#[test]
fn a_start_that_stopped_before_recording_anything_is_retried_by_the_tracked_scope_start() {
    for (name, lock_file) in [("unified-interrupted-empty", false), ("unified-interrupted-lock", true)] {
        let (root, settings, r) = project(name);
        std::fs::create_dir(root.join(".gil")).unwrap();
        if lock_file { std::fs::write(root.join(".gil/project.lock"), b"").unwrap(); }
        let before = tree(&root);
        let mut c = Client::serve(&settings);
        refused(start(&mut c, &r, None), "a start without a kind over an empty .gil");
        refused(start(&mut c, &r, Some("image_reports")), "image reports into an existing .gil");
        assert_eq!(tree(&root), before, "the refusals recorded nothing");
        assert!(!root.join(".gil/state.yaml").exists());

        let begun = start(&mut c, &r, Some("tracked_scope"));
        assert!(is_ok(&begun), "{begun}");
        assert_eq!(sc(&begun)["phase"], "preparing", "{begun}");
        assert!(root.join(".gil/start/preparation.json").is_file());
        assert!(!root.join(".gil/state.yaml").exists() && !root.join(".gil/artifacts").exists(), "a retried preparation is still not a project");
        // From a new process the same preparation is continued, and the other kind stays refused.
        drop(c);
        let mut c = Client::serve(&settings);
        assert_eq!(sc(&start(&mut c, &r, None))["phase"], "preparing");
        assert_eq!(sc(&refused(start(&mut c, &r, Some("image_reports")), "image reports over the retried preparation"))["code"], "start_kind_conflict");
    }
}
