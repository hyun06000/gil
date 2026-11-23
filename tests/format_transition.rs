//! The explicit storage-format transition 6 → 8 and 7 → 9 (Storage Transition Contract §7,
//! implementation order 1b) through the real development call path: `gil mcp --serve`.
//!
//! - check is read-only and returns the proposal; apply needs the user's relayed approval bound
//!   to that project, state, formats and proposal, and refuses everything else unwritten;
//! - only `.gil/state.yaml` changes (plus the original-state copy); every value except the end
//!   history is the same, every earlier ended Will is `achieved`, working files are untouched;
//! - unfinished restores, a held lock, invalid records, copy failures and conflicts, a failed
//!   save, a lost response and a resend each leave the record as the contract says;
//! - formats 4/5, 8 → 10, reverse transitions and adding the visual registry do not exist.
//!
//! Synthetic projects in temporary folders only; never `gil start` in the source repository.
//! Interruptions between the steps of apply are measured inside the crate
//! (`session::format_transition::tests`). Host screens and model behavior are not measured here.
#![cfg(all(feature = "will-end-history", not(feature = "approved-plan")))]

mod common;
use gil::{EndResult, NodeKind, ProjectSession};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const CHECK: &str = "gil_format_transition_check";
const APPLY: &str = "gil_format_transition_apply";

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Every entry under `root` and its bytes (`None` for a directory). Read only while no GIL
/// process holds the project lock (Windows refuses to read a locked lock file).
fn tree(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path.strip_prefix(root).unwrap().to_path_buf();
            let kind = std::fs::symlink_metadata(&path).unwrap().file_type();
            if kind.is_dir() {
                out.insert(name, None);
                walk(root, &path, out);
            } else if kind.is_file() {
                out.insert(name, Some(std::fs::read(&path).unwrap()));
            } else {
                // A link or a special entry is recorded as itself, never opened or followed.
                let link = std::fs::read_link(&path).map(|to| to.display().to_string()).unwrap_or_default();
                out.insert(name, Some(format!("entry {kind:?} {link}").into_bytes()));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Rewrites a record this build wrote (format 8/9) into the historical format 6/7 shape.
fn as_historical(state: &Path, format: u32) {
    let mut file: serde_norway::Value = serde_norway::from_slice(&std::fs::read(state).unwrap()).unwrap();
    assert_eq!(file["format"], serde_norway::Value::from(format + 2));
    common::historical_done_list(&mut file);
    file["format"] = serde_norway::Value::from(format);
    std::fs::write(state, serde_norway::to_string(&file).unwrap()).unwrap();
    let loaded = gil::load(common::spec(), state).expect("the synthetic historical record loads");
    assert!(!loaded.records_end_results());
}

/// A format 6 project with six ended Wills and an open Verify (its Will active), a working
/// file and a `.gilignore` the transition must not touch.
fn format_6(label: &str) -> (PathBuf, PathBuf) {
    let root = common::scratch(&format!("transition-{label}-{}", std::process::id()));
    std::fs::write(root.join("work.txt"), "user file").unwrap();
    std::fs::write(root.join(".gilignore"), "target/\n").unwrap();
    let state = root.join(gil::STATE_PATH);
    let mut session = ProjectSession::start(common::spec(), &state).unwrap();
    *session.project_mut() = common::bootstrap_from(session.project().clone());
    common::up_to_verify(session.project_mut());
    session.commit().unwrap();
    drop(session);
    as_historical(&state, gil::CHAIN_FORMAT);
    (root, state)
}

/// What the contract allows to change, applied to the original by this test itself.
fn expected_after(original: &[u8], from: u32) -> serde_norway::Value {
    use serde_norway::Value as Y;
    let mut file: Y = serde_norway::from_slice(original).unwrap();
    file["format"] = Y::from(from + 2);
    for (_, existence) in file["existences"].as_mapping_mut().unwrap() {
        let journey = existence["journey"].as_mapping_mut().unwrap();
        let done = journey.remove("done_wills").unwrap();
        let ended: Vec<Y> = done.as_sequence().unwrap().iter().map(|record| {
            let mut record = record.clone();
            record["end_result"] = Y::from("achieved");
            record
        }).collect();
        journey.insert("ended_wills".into(), Y::Sequence(ended));
    }
    file
}

struct Mcp {
    kid: std::process::Child,
    out: BufReader<std::process::ChildStdout>,
    next: i64,
}

impl Mcp {
    fn serve(settings: &Path) -> Self {
        let mut kid = Command::new(env!("CARGO_BIN_EXE_gil")).args(["mcp", "--serve"])
            .env("GIL_MONITOR_STATE_DIR", settings.join("bindings"))
            .current_dir(std::env::temp_dir())
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("gil mcp --serve");
        let out = BufReader::new(kid.stdout.take().unwrap());
        let mut mcp = Self { kid, out, next: 1 };
        mcp.request("initialize", json!({"protocolVersion": "2025-06-18", "capabilities": {},
            "clientInfo": {"name": "gil-test", "version": "0"}}));
        mcp.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        mcp
    }
    fn send(&mut self, frame: &Value) {
        let stdin = self.kid.stdin.as_mut().unwrap();
        writeln!(stdin, "{frame}").unwrap();
        stdin.flush().unwrap();
    }
    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next;
        self.next += 1;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let mut line = String::new();
        self.out.read_line(&mut line).unwrap();
        let got: Value = serde_json::from_str(&line).unwrap_or_else(|err| panic!("{line:?}: {err}"));
        assert_eq!(got["id"], json!(id));
        got
    }
    fn call(&mut self, name: &str, arguments: Value) -> Value {
        self.request("tools/call", json!({"name": name, "arguments": arguments}))
    }
    /// The structured answer of an executed call (not a wire error).
    fn body(&mut self, name: &str, arguments: Value) -> Value {
        let reply = self.call(name, arguments);
        assert!(reply.get("error").is_none(), "{reply}");
        reply["result"]["structuredContent"].clone()
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        drop(self.kid.stdin.take());
        let _ = self.kid.wait();
    }
}

fn settings(label: &str) -> PathBuf {
    let at = std::env::temp_dir().join(format!("gil-transition-settings-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&at);
    std::fs::create_dir_all(&at).unwrap();
    at
}

fn approval(checked: &Value, root: &Path, approved: bool) -> Value {
    let mut with = checked["apply_with"].clone();
    assert_eq!(with["project_root"], json!(root.display().to_string()));
    with["user_approved"] = json!(approved);
    with
}

fn copy_path(state: &Path, checked: &Value) -> PathBuf {
    let file = checked["proposal"]["copy_file"].as_str().unwrap();
    state.parent().unwrap().join(file.strip_prefix(".gil/").unwrap())
}

#[test]
fn six_to_eight_through_the_mcp_path_keeps_every_value_but_the_end_history() {
    let (root, state) = format_6("six");
    let original = std::fs::read(&state).unwrap();
    let before = tree(&root);
    let promised: Vec<_> = gil::load(common::spec(), &state).unwrap().current_existence().journey()
        .ended_wills().iter().map(|e| e.will().clone()).collect();
    assert_eq!(promised.len(), 6);
    let at = settings("six");
    let mut mcp = Mcp::serve(&at);

    // The tool surface: both transition tools, kept apart from the visual migration tools.
    let listed = mcp.request("tools/list", json!({}));
    let tools = listed["result"]["tools"].as_array().unwrap();
    for name in [CHECK, APPLY] {
        let tool = tools.iter().find(|t| t["name"] == name).unwrap_or_else(|| panic!("{name} not listed"));
        assert_eq!(tool["_meta"]["ui"]["visibility"], json!(["model"]));
    }
    if cfg!(feature = "visual-authoring") {
        let migration: Vec<Value> = serde_json::from_str(include_str!("../src/mcp/migration-tools.json")).unwrap();
        for one in migration {
            let listed = tools.iter().find(|t| t["name"] == one["name"]).expect("visual migration tool");
            assert_eq!(listed["description"], one["description"], "the visual migration tool changed meaning");
            assert_eq!(listed["inputSchema"], one["inputSchema"]);
        }
    }

    // Check: the proposal, and nothing written.
    let checked = mcp.body(CHECK, json!({"project_root": root}));
    assert_eq!(checked["ok"], true, "{checked}");
    assert_eq!(checked["status"], "ready", "{checked}");
    let proposal = &checked["proposal"];
    assert_eq!(proposal["current_format"], 6);
    assert_eq!(proposal["target_format"], 8);
    assert_eq!(proposal["state_sha256"], json!(sha256(&original)));
    assert_eq!(proposal["project_root"], json!(root.display().to_string()));
    assert_eq!(proposal["state_file"], ".gil/state.yaml");
    assert_eq!(proposal["copy_present"], "absent");
    assert_eq!(proposal["changes"]["records"], json!([{"existence": "existence:X1", "ended_wills": 6}]));
    for key in ["compatibility", "reverse", "preserved", "copy_file"] {
        assert!(!proposal[key].is_null(), "the proposal must say {key}: {proposal}");
    }
    assert!(checked["said"].as_str().unwrap().contains("not an approval"), "{checked}");
    assert_eq!(tree(&root), before, "the check wrote something");

    // A declined approval applies nothing.
    let declined = mcp.call(APPLY, approval(&checked, &root, false));
    assert_eq!(declined["result"]["isError"], true);
    assert_eq!(declined["result"]["structuredContent"]["code"], "approval_required");
    assert_eq!(tree(&root), before);

    // Apply with the approval relayed for this proposal.
    let applied = mcp.body(APPLY, approval(&checked, &root, true));
    assert_eq!(applied["ok"], true, "{applied}");
    assert_eq!((applied["previous_format"].clone(), applied["storage_format"].clone()), (json!(6), json!(8)));
    assert_eq!(applied["converted_records"], 6);
    assert_eq!(applied["copy"], "created");
    let now = std::fs::read(&state).unwrap();
    assert_eq!(applied["state_sha256_after"], json!(sha256(&now)));
    let copy = copy_path(&state, &checked);
    assert_eq!(std::fs::read(&copy).unwrap(), original, "the copy must be the original bytes");

    // Only the state file changed; the copy (and its folder) is the only addition.
    let after = tree(&root);
    let relative = |p: &Path| p.strip_prefix(&root).unwrap().to_path_buf();
    for (path, bytes) in &before {
        if path == &relative(&state) { continue; }
        assert_eq!(after.get(path), Some(bytes), "{path:?} changed");
    }
    let added: Vec<_> = after.keys().filter(|p| !before.contains_key(*p)).cloned().collect();
    assert_eq!(added, vec![relative(copy.parent().unwrap()), relative(&copy)]);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&copy).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(std::fs::metadata(copy.parent().unwrap()).unwrap().permissions().mode() & 0o777, 0o700);
        // Written by the exclusive record writer, whose file is owner-only.
        assert_eq!(std::fs::metadata(&state).unwrap().permissions().mode() & 0o777, 0o600);
    }
    assert!(!state.with_file_name("state.yaml.writing").exists());

    // Every value but the allowed changes is the same.
    assert_eq!(serde_norway::from_slice::<serde_norway::Value>(&now).unwrap(), expected_after(&original, 6));
    drop(mcp);

    // A new process reads format 8: the same Wills in the same order, all achieved.
    let project = gil::load(common::spec(), &state).unwrap();
    assert!(project.records_end_results());
    let ended = project.current_existence().journey().ended_wills();
    assert_eq!(ended.iter().map(|e| e.will().clone()).collect::<Vec<_>>(), promised);
    assert!(ended.iter().all(|e| e.result() == EndResult::Achieved));
    assert!(project.active_will().is_some(), "the open Verify keeps its Will");

    // Restarted server: status works, the check says already transitioned, a resend writes nothing.
    let mut mcp = Mcp::serve(&at);
    let status = mcp.call("gil_status", json!({"project_root": root}));
    assert_ne!(status["result"]["isError"], true, "{status}");
    drop(mcp);
    let settled = tree(&root);
    let mut mcp = Mcp::serve(&at);
    let read = mcp.body(CHECK, json!({"project_root": root}));
    assert_eq!(read["status"], "already_transitioned", "{read}");
    assert_eq!(read["copies"], json!([checked["proposal"]["copy_file"]]));
    let resent = mcp.call(APPLY, approval(&checked, &root, true));
    let resent = &resent["result"]["structuredContent"];
    assert_eq!(resent["code"], "already_transitioned", "{resent}");
    assert_eq!(resent["wrote"], false);
    assert_eq!(resent["copy_for_request"], "present_matching");
    assert_eq!(resent["state_is_that_transition"], true);
    drop(mcp);
    assert_eq!(tree(&root), settled, "a check or resend wrote or copied again");

    // The format 8 meaning is now available: the open Verify can end unachieved.
    let mut session = ProjectSession::open(common::spec(), &state).unwrap();
    let will = session.project().active_will().unwrap().id();
    std::fs::write(root.join("attempt.md"), "1/3 done; 2 inputs unreadable; the rest unverified").unwrap();
    let report = common::full_report(&common::spec(), gil::CycleKind::Experiment, NodeKind::Verify)
        .with("evidence_files", "attempt.md");
    let closed = session.close_step_unachieved(report, will).unwrap();
    assert_eq!(closed.result, EndResult::Unachieved);
    session.commit().unwrap();
}

#[test]
fn an_approval_is_bound_to_its_project_state_formats_and_proposal() {
    let (root, state) = format_6("bind-a");
    // Another project with byte-identical records: the same state digest, another location.
    let twin = root.with_file_name(format!("{}-twin", root.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&twin);
    copy_dir(&root, &twin);
    assert_eq!(std::fs::read(twin.join(gil::STATE_PATH)).unwrap(), std::fs::read(&state).unwrap());
    let (before, twin_before) = (tree(&root), tree(&twin));
    let at = settings("bind");
    let mut mcp = Mcp::serve(&at);
    let checked = mcp.body(CHECK, json!({"project_root": root}));
    let twin_checked = mcp.body(CHECK, json!({"project_root": twin}));
    assert_eq!(checked["apply_with"]["state_sha256"], twin_checked["apply_with"]["state_sha256"]);
    assert_ne!(checked["proposal_sha256"], twin_checked["proposal_sha256"], "the proposal must bind the location");

    let refused = |mcp: &mut Mcp, input: Value| {
        let reply = mcp.call(APPLY, input);
        assert_eq!(reply["result"]["isError"], true, "{reply}");
        let body = reply["result"]["structuredContent"].clone();
        assert_eq!(body["class"], "refused", "{body}");
        assert_eq!(body["state_changed"], false);
        body["code"].as_str().unwrap().to_owned()
    };
    // The other project's approval, its state digest equal.
    let mut other = approval(&checked, &root, true);
    other["project_root"] = json!(twin);
    assert_eq!(refused(&mut mcp, other), "proposal_mismatch");
    // Another proposal digest, another state digest, another source format, a declined approval.
    let mut input = approval(&checked, &root, true);
    input["proposal_sha256"] = json!("0".repeat(64));
    assert_eq!(refused(&mut mcp, input), "proposal_mismatch");
    let mut input = approval(&checked, &root, true);
    input["state_sha256"] = json!("0".repeat(64));
    assert_eq!(refused(&mut mcp, input), "state_changed");
    let mut input = approval(&checked, &root, true);
    input["current_format"] = json!(7);
    input["target_format"] = json!(9);
    assert_eq!(refused(&mut mcp, input), "target_mismatch");
    assert_eq!(refused(&mut mcp, approval(&checked, &root, false)), "approval_required");
    // A missing approval or a null is a wire error: nothing is executed.
    let mut input = approval(&checked, &root, true);
    input.as_object_mut().unwrap().remove("user_approved");
    let wire = mcp.call(APPLY, input);
    assert_eq!(wire["error"]["code"], -32602, "{wire}");
    assert!(wire["error"]["message"].as_str().unwrap().contains("A missing approval is not an approval"), "{wire}");
    let mut input = approval(&checked, &root, true);
    input["user_approved"] = Value::Null;
    assert_eq!(mcp.call(APPLY, input)["error"]["code"], -32602);
    drop(mcp);
    assert_eq!(tree(&root), before);
    assert_eq!(tree(&twin), twin_before);

    // The state moves on after the check: the approval is stale and is refused.
    let mut session = ProjectSession::open(common::spec(), &state).unwrap();
    std::fs::write(root.join("verified.md"), "observed").unwrap();
    let report = common::full_report(&common::spec(), gil::CycleKind::Experiment, NodeKind::Verify)
        .with("evidence_files", "verified.md");
    session.close_step(report).unwrap();
    session.commit().unwrap();
    drop(session);
    let moved = tree(&root);
    let mut mcp = Mcp::serve(&at);
    assert_eq!(refused(&mut mcp, approval(&checked, &root, true)), "state_changed");
    drop(mcp);
    assert_eq!(tree(&root), moved);

    // A fresh check gives a new proposal; that one, approved, applies to this project only.
    let mut mcp = Mcp::serve(&at);
    let again = mcp.body(CHECK, json!({"project_root": root}));
    assert_ne!(again["proposal_sha256"], checked["proposal_sha256"]);
    assert_eq!(again["proposal"]["changes"]["records"][0]["ended_wills"], 7);
    let applied = mcp.body(APPLY, approval(&again, &root, true));
    assert_eq!(applied["ok"], true, "{applied}");
    drop(mcp);
    assert_eq!(tree(&twin), twin_before, "the twin project changed");
    std::fs::remove_dir_all(&twin).unwrap();
}

#[test]
fn blocked_records_are_refused_and_every_file_is_kept() {
    let at = settings("blocked");

    // An unfinished restore is not recovered or cleaned up.
    let (root, state) = format_6("blocked-restore");
    // Named like the restore area's own preparation folder, so every platform recognizes it.
    std::fs::create_dir_all(state.parent().unwrap().join("restore").join("preparing-4242-1")).unwrap();
    let before = tree(&root);
    let mut mcp = Mcp::serve(&at);
    let checked = mcp.call(CHECK, json!({"project_root": root}));
    assert_eq!(checked["result"]["structuredContent"]["code"], "needs_recovery", "{checked}");
    let input = json!({"project_root": root, "current_format": 6, "target_format": 8,
        "state_sha256": sha256(&std::fs::read(&state).unwrap()), "proposal_sha256": "a".repeat(64), "user_approved": true});
    assert_eq!(mcp.call(APPLY, input)["result"]["structuredContent"]["code"], "needs_recovery");
    drop(mcp);
    assert_eq!(tree(&root), before);

    // Another GIL command holds the lock.
    let (root, state) = format_6("blocked-busy");
    let mut mcp = Mcp::serve(&at);
    let checked = mcp.body(CHECK, json!({"project_root": root}));
    let held = ProjectSession::open(common::spec(), &state).unwrap();
    assert_eq!(mcp.call(CHECK, json!({"project_root": root}))["result"]["structuredContent"]["code"], "busy");
    assert_eq!(mcp.call(APPLY, approval(&checked, &root, true))["result"]["structuredContent"]["code"], "busy");
    drop(held);
    drop(mcp);
    assert_eq!(std::fs::read(&state).unwrap()[..9], *b"format: 6");
    assert!(!state.parent().unwrap().join("conversion").exists());

    // Invalid records: an unknown field, and a missing manifest object.
    let (root, state) = format_6("blocked-field");
    let mut text = std::fs::read_to_string(&state).unwrap();
    text.push_str("stray: value\n");
    std::fs::write(&state, text).unwrap();
    let before = tree(&root);
    let mut mcp = Mcp::serve(&at);
    assert_eq!(mcp.call(CHECK, json!({"project_root": root}))["result"]["structuredContent"]["code"], "record_invalid");
    drop(mcp);
    assert_eq!(tree(&root), before);
    let (root, state) = format_6("blocked-object");
    // The manifest the registry names (not an unreferenced one the fixture also holds).
    let file: serde_norway::Value = serde_norway::from_slice(&std::fs::read(&state).unwrap()).unwrap();
    let digest = file["artifacts"]["snapshots"][0]["manifest"]["digest"].as_str().unwrap().to_owned();
    std::fs::remove_file(state.parent().unwrap().join("artifacts/manifests/sha256").join(&digest[..2]).join(&digest[2..])).unwrap();
    let before = tree(&root);
    let mut mcp = Mcp::serve(&at);
    assert_eq!(mcp.call(CHECK, json!({"project_root": root}))["result"]["structuredContent"]["code"], "record_invalid");
    drop(mcp);
    assert_eq!(tree(&root), before);
}

#[test]
fn copy_failures_leave_format_6_and_keep_what_was_there() {
    let at = settings("failures");

    // Something that is not a folder stands where the copy folder belongs: it cannot be inspected
    // safely, so even the check refuses, and nothing is replaced.
    let (root, state) = format_6("fail-copy-entry");
    let mut mcp = Mcp::serve(&at);
    let checked = mcp.body(CHECK, json!({"project_root": root}));
    std::fs::write(state.parent().unwrap().join("conversion"), "not a folder").unwrap();
    let before = tree(&root);
    assert_eq!(mcp.call(CHECK, json!({"project_root": root}))["result"]["structuredContent"]["code"], "copy_unreadable");
    assert_eq!(mcp.call(APPLY, approval(&checked, &root, true))["result"]["structuredContent"]["code"], "copy_unreadable");
    drop(mcp);
    assert_eq!(tree(&root), before);

    // The copy cannot be created in its folder: not applied, format 6 kept.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let (root, state) = format_6("fail-copy-create");
        let mut mcp = Mcp::serve(&at);
        let checked = mcp.body(CHECK, json!({"project_root": root}));
        let folder = state.parent().unwrap().join("conversion");
        std::fs::create_dir(&folder).unwrap();
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o500)).unwrap();
        let before = tree(&root);
        let failed = mcp.call(APPLY, approval(&checked, &root, true));
        let failed = &failed["result"]["structuredContent"];
        assert_eq!((failed["code"].clone(), failed["class"].clone()), (json!("copy_failed"), json!("not_applied")), "{failed}");
        assert_eq!(failed["state_changed"], false);
        drop(mcp);
        assert_eq!(tree(&root), before);
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o700)).unwrap();
    }

    // A different file already at the copy name: never overwritten, apply refused.
    let (root, state) = format_6("fail-conflict");
    let mut mcp = Mcp::serve(&at);
    let checked = mcp.body(CHECK, json!({"project_root": root}));
    let copy = copy_path(&state, &checked);
    std::fs::create_dir_all(copy.parent().unwrap()).unwrap();
    std::fs::write(&copy, "someone else's bytes").unwrap();
    let blocked = mcp.body(CHECK, json!({"project_root": root}));
    assert_eq!(blocked["status"], "blocked", "{blocked}");
    assert_eq!(blocked["proposal"]["copy_present"], "present_different");
    let before = tree(&root);
    let failed = mcp.call(APPLY, approval(&checked, &root, true));
    let failed = &failed["result"]["structuredContent"];
    assert_eq!((failed["code"].clone(), failed["class"].clone()), (json!("copy_conflict"), json!("not_applied")), "{failed}");
    drop(mcp);
    assert_eq!(tree(&root), before);

}

/// A synthetic entry planted at the state writer's temporary name.
#[derive(Clone, Copy, Debug)]
enum Planted { File, Folder, #[cfg(unix)] Link, #[cfg(unix)] DanglingLink, #[cfg(unix)] Fifo }

fn plant(at: &Path, kind: Planted, sentinel: &Path) {
    match kind {
        Planted::File => std::fs::write(at, b"someone else's bytes").unwrap(),
        Planted::Folder => {
            std::fs::create_dir(at).unwrap();
            std::fs::write(at.join("inside.txt"), b"kept").unwrap();
        }
        #[cfg(unix)]
        Planted::Link => std::os::unix::fs::symlink(sentinel, at).unwrap(),
        #[cfg(unix)]
        Planted::DanglingLink => std::os::unix::fs::symlink(sentinel.with_file_name("absent.txt"), at).unwrap(),
        #[cfg(unix)]
        Planted::Fifo => {
            let name = std::ffi::CString::new(at.as_os_str().as_encoded_bytes()).unwrap();
            assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0, "mkfifo");
        }
    }
    let _ = sentinel;
}

/// Whatever already stands at `.gil/state.yaml.writing` — in a format 6 record, where the ordinary
/// even-format writer would have reused that name — is kept exactly: check and apply refuse with
/// the same reason, no copy is written, and the entry, a non-target file a link points to and every
/// other file keep their bytes and identity. An entry planted after the check is refused the same
/// way. After the user removes it, the same proposal applies.
#[test]
fn a_pre_existing_temporary_entry_of_any_kind_is_kept_and_stops_the_transition() {
    let at = settings("planted");
    let mut kinds = vec![Planted::File, Planted::Folder];
    #[cfg(unix)]
    kinds.extend([Planted::Link, Planted::DanglingLink, Planted::Fifo]);
    for kind in kinds {
        let label = format!("planted-{kind:?}").to_lowercase();
        let (root, state) = format_6(&label);
        // A synthetic non-target file inside the test area, outside the record folder.
        let sentinel = root.join("sentinel.txt");
        std::fs::write(&sentinel, b"synthetic non-target bytes").unwrap();
        let mut mcp = Mcp::serve(&at);
        let checked = mcp.body(CHECK, json!({"project_root": root}));
        assert_eq!(checked["status"], "ready", "{kind:?}: {checked}");
        // Planted after the check the user approved, before the apply.
        let temp = state.with_file_name("state.yaml.writing");
        plant(&temp, kind, &sentinel);
        drop(mcp);
        let before = tree(&root);
        let planted_identity = identity(&temp);
        let sentinel_identity = identity_of_file(&sentinel);

        let mut mcp = Mcp::serve(&at);
        let blocked = mcp.body(CHECK, json!({"project_root": root}));
        assert_eq!(blocked["status"], "blocked", "{kind:?}: {blocked}");
        let reason = blocked["blocked_by"][0].as_str().unwrap().to_owned();
        assert!(reason.contains("state.yaml.writing") && reason.contains("not overwritten"), "{kind:?}: {reason}");
        let refused = mcp.call(APPLY, approval(&checked, &root, true));
        let refused = &refused["result"]["structuredContent"];
        assert_eq!((refused["code"].clone(), refused["class"].clone(), refused["state_changed"].clone()),
            (json!("temporary_state_present"), json!("refused"), json!(false)), "{kind:?}: {refused}");
        assert!(refused["said"].as_str().unwrap().starts_with(&reason), "check and apply must say the same thing");
        drop(mcp);
        assert_eq!(tree(&root), before, "{kind:?}: a file changed");
        assert_eq!(identity(&temp), planted_identity, "{kind:?}: the planted entry was replaced");
        assert_eq!(identity_of_file(&sentinel), sentinel_identity, "{kind:?}: the link target changed");
        assert!(!state.parent().unwrap().join("conversion").exists(), "{kind:?}: a copy was written");

        // The user removes the entry; the same proposal is then applied.
        if matches!(kind, Planted::Folder) { std::fs::remove_dir_all(&temp).unwrap(); } else { std::fs::remove_file(&temp).unwrap(); }
        let mut mcp = Mcp::serve(&at);
        let again = mcp.body(CHECK, json!({"project_root": root}));
        assert_eq!(again["proposal_sha256"], checked["proposal_sha256"]);
        let applied = mcp.body(APPLY, approval(&again, &root, true));
        assert_eq!(applied["ok"], true, "{kind:?}: {applied}");
        drop(mcp);
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"synthetic non-target bytes");
    }
}

/// The entry itself, never what it points to: kind, link target and (Unix) device/inode/mode.
fn identity(path: &Path) -> String {
    let meta = std::fs::symlink_metadata(path).unwrap();
    let link = std::fs::read_link(path).ok();
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        format!("{:?} {link:?} {} {} {:o}", meta.file_type(), meta.dev(), meta.ino(), meta.mode())
    }
    #[cfg(not(unix))]
    format!("{:?} {link:?} {}", meta.file_type(), meta.len())
}

fn identity_of_file(path: &Path) -> String {
    format!("{} {:?}", identity(path), std::fs::read(path).unwrap())
}

#[test]
fn formats_without_a_transition_are_refused() {
    let at = settings("formats");
    let mut mcp = Mcp::serve(&at);

    // Format 4 (no Chain) is not a source.
    let root = common::scratch(&format!("transition-format-4-{}", std::process::id()));
    common::legacy::record(&root, common::legacy::Layout::Cycles);
    let before = tree(&root);
    assert_eq!(mcp.call(CHECK, json!({"project_root": root}))["result"]["structuredContent"]["code"], "unsupported_format");
    let digest = sha256(&std::fs::read(root.join(gil::STATE_PATH)).unwrap());
    let ask = |from: u32, to: u32| json!({"project_root": root, "current_format": from, "target_format": to,
        "state_sha256": digest, "proposal_sha256": "a".repeat(64), "user_approved": true});
    assert_eq!(mcp.call(APPLY, ask(6, 8))["result"]["structuredContent"]["code"], "target_mismatch");
    for (from, to) in [(4, 8), (4, 6), (5, 7), (6, 9), (8, 6), (9, 7), (8, 10), (6, 10), (7, 11)] {
        assert_eq!(mcp.call(APPLY, ask(from, to))["result"]["structuredContent"]["code"], "unsupported_transition", "{from} → {to}");
    }
    assert_eq!(tree(&root), before);

    // A project this build started is already format 8.
    let root = common::scratch(&format!("transition-format-8-{}", std::process::id()));
    ProjectSession::start(common::spec(), root.join(gil::STATE_PATH)).unwrap().commit().unwrap();
    let before = tree(&root);
    let read = mcp.body(CHECK, json!({"project_root": root}));
    assert_eq!((read["status"].clone(), read["storage_format"].clone()), (json!("already_transitioned"), json!(8)), "{read}");
    assert_eq!(read["copies"], json!([]));

    // Format 10 is unknown to this build — or, in a tracking-policy build that reads 10/11, a format 10
    // record without its required `tracking` block is invalid. Either way nothing is converted or changed.
    let root = common::scratch(&format!("transition-format-10-{}", std::process::id()));
    let state = root.join(gil::STATE_PATH);
    ProjectSession::start(common::spec(), &state).unwrap().commit().unwrap();
    let text = std::fs::read_to_string(&state).unwrap().replacen("format: 8", "format: 10", 1);
    std::fs::write(&state, text).unwrap();
    let ten = tree(&root);
    let expected = if cfg!(feature = "tracking-policy") { "record_invalid" } else { "unknown_format" };
    assert_eq!(mcp.call(CHECK, json!({"project_root": root}))["result"]["structuredContent"]["code"], expected);
    drop(mcp);
    assert_eq!(tree(&root), ten);
    let _ = before;
}

/// Format 7 → 9 keeps the visual registry and the owner-only private storage.
#[cfg(all(feature = "visual-authoring", any(all(target_os = "macos", target_arch = "aarch64"), windows)))]
#[test]
fn seven_to_nine_keeps_the_visual_registry_and_private_storage() {
    let root = std::env::temp_dir().join(format!("gil-transition-visual-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("work.txt"), "user file").unwrap();
    let at = settings("visual");
    let mut mcp = Mcp::serve(&at);
    let started = mcp.call("gil_start", json!({"project_root": root}));
    assert_eq!(started["result"]["structuredContent"]["storage_format"], 9, "{started}");
    drop(mcp);
    let state = product_state_path(&root);
    let mut project = common::bootstrap_from(gil::load(common::spec(), &state).unwrap());
    common::up_to_verify(&mut project);
    gil::save(&project, &state).unwrap();
    as_historical(&state, gil::CHAIN_VISUAL_FORMAT);
    let original = std::fs::read(&state).unwrap();

    let mut mcp = Mcp::serve(&at);
    let checked = mcp.body(CHECK, json!({"project_root": root}));
    assert_eq!(checked["status"], "ready", "{checked}");
    assert_eq!((checked["proposal"]["current_format"].clone(), checked["proposal"]["target_format"].clone()), (json!(7), json!(9)));

    // An entry already at the writer's temporary name stops 7 → 9 the same way and is kept.
    let temp = state.with_file_name("state.yaml.writing");
    std::fs::create_dir(&temp).unwrap();
    std::fs::write(temp.join("inside.txt"), b"kept").unwrap();
    assert_eq!(mcp.body(CHECK, json!({"project_root": root}))["status"], "blocked");
    assert_eq!(mcp.call(APPLY, approval(&checked, &root, true))["result"]["structuredContent"]["code"], "temporary_state_present");
    assert_eq!(std::fs::read(&state).unwrap(), original);
    assert_eq!(std::fs::read(temp.join("inside.txt")).unwrap(), b"kept");
    std::fs::remove_dir_all(&temp).unwrap();
    let applied = mcp.body(APPLY, approval(&checked, &root, true));
    assert_eq!(applied["storage_format"], 9, "{applied}");
    let now = std::fs::read(&state).unwrap();
    assert_eq!(serde_norway::from_slice::<serde_norway::Value>(&now).unwrap(), expected_after(&original, 7));
    let copy = copy_path(&state, &checked);
    assert_eq!(std::fs::read(&copy).unwrap(), original);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&copy).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(std::fs::metadata(&state).unwrap().permissions().mode() & 0o777, 0o600);
    }
    // Opening the converted record runs the private-storage check again (Windows: the DACL tree,
    // the copy folder included).
    let status = mcp.call("gil_status", json!({"project_root": root}));
    assert_ne!(status["result"]["isError"], true, "{status}");
    drop(mcp);
    let reread = gil::load(common::spec(), &state).unwrap();
    assert!(reread.records_end_results() && reread.visual_state().is_some());
    assert!(reread.current_existence().journey().ended_wills().iter().all(|e| e.result() == EndResult::Achieved));
}

#[cfg(all(feature = "visual-authoring", any(all(target_os = "macos", target_arch = "aarch64"), windows)))]
fn product_state_path(root: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let canonical = root.canonicalize().unwrap();
        let mut identity = Vec::new();
        for unit in canonical.as_os_str().encode_wide() {
            identity.extend_from_slice(&unit.to_le_bytes());
        }
        let key = sha256(&identity);
        PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap())
            .join("GIL/project-state-v1").join(key).join(gil::STATE_PATH)
    }
    #[cfg(not(windows))]
    { root.join(gil::STATE_PATH) }
}

/// An actually released older executable refuses the converted record and changes nothing
/// (contract §13). Needs an explicit binary; never inferred from PATH. Synthetic data only.
#[test]
#[ignore = "explicit older release binary required (GIL_TRANSITION_OLD_BINARY)"]
fn an_older_release_refuses_the_converted_record_without_changing_it() {
    let binary = std::env::var_os("GIL_TRANSITION_OLD_BINARY").expect("explicit release executable required");
    let (root, _) = format_6("old-release");
    let at = settings("old-release");
    let mut mcp = Mcp::serve(&at);
    let checked = mcp.body(CHECK, json!({"project_root": root}));
    assert_eq!(mcp.body(APPLY, approval(&checked, &root, true))["ok"], true);
    drop(mcp);
    let before = tree(&root);
    for args in [vec!["status"], vec!["context"], vec!["close"]] {
        let output = Command::new(&binary).args(&args).current_dir(&root)
            .stdin(Stdio::null()).output().unwrap();
        let text = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        assert!(!output.status.success(), "{args:?} succeeded on format 8: {text}");
        assert!(text.contains('8'), "{args:?} must refuse the format, not fail otherwise: {text}");
        eprintln!("{args:?}: {}", text.lines().next().unwrap_or_default());
    }
    assert_eq!(tree(&root), before, "the older release changed the converted record");
}
