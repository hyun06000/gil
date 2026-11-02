//! Native Host adapter contract tests; no installed Host/plugin or user project is touched.
use gil::{ProjectSession, RuleSet, STATE_PATH};
#[cfg(not(feature = "approved-plan"))]
use gil::NodeKind;
use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
mod common;

fn fixture(label: &str) -> PathBuf {
    let root = std::env::temp_dir()
        .join(format!("gil-guard-{label}-{}", std::process::id()))
        .join("project");
    fs::create_dir_all(&root).unwrap();
    ProjectSession::start(RuleSet::builtin().unwrap(), root.join(STATE_PATH))
        .unwrap()
        .commit()
        .unwrap();
    root.canonicalize().unwrap()
}

fn hook(root: &Path, name: &str, args: Value) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_gil-write-guard"))
        .args(["hook", "--root"])
        .arg(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let value =
        json!({"hook_event_name":"PreToolUse","tool_name":name,"cwd":root,"tool_input":args});
    child
        .stdin
        .take()
        .unwrap()
        .write_all(value.to_string().as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    serde_json::from_slice(&out.stdout).unwrap()
}
fn denied(v: &Value) -> bool {
    v["hookSpecificOutput"]["permissionDecision"] == "deny"
}

#[test]
fn interview_blocks_all_direct_project_edit_surfaces_but_not_outside_files() {
    let root = fixture("interview");
    let before = fs::read(root.join(STATE_PATH)).unwrap();
    for (name, args) in [
        ("Write", json!({"file_path":"report.docx"})),
        ("Edit", json!({"file_path":root.join("report.docx")})),
        ("MultiEdit", json!({"file_path":"chart.png"})),
        ("NotebookEdit", json!({"notebook_path":"analysis.ipynb"})),
        (
            "apply_patch",
            json!({"command":"*** Begin Patch\n*** Add File: result.txt\n+x\n*** End Patch"}),
        ),
        (
            "apply_patch",
            json!({"command":"*** Begin Patch\n*** Update File: ../external.txt\n*** Move to: result.txt\n@@\n-x\n+y\n*** End Patch"}),
        ),
    ] {
        assert!(denied(&hook(&root, name, args)), "{name}");
    }
    assert_eq!(
        hook(&root, "Write", json!({"file_path":"../outside.txt"})),
        json!({})
    );
    assert_eq!(hook(&root, "WebSearch", json!({"query":"test"})), json!({}));
    assert_eq!(
        hook(&root, "Read", json!({"file_path":"report.docx"})),
        json!({})
    );
    assert_eq!(fs::read(root.join(STATE_PATH)).unwrap(), before);
    assert!(!root.join("report.docx").exists());
}

#[test]
fn shell_is_never_autoapproved_and_only_exact_guard_retry_passes() {
    let root = fixture("shell");
    let result = hook(&root, "Bash", json!({"command":"printf hello"}));
    assert!(denied(&result));
    let reason = result["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .unwrap();
    let wrapper = reason.split_once('\n').unwrap().1;
    assert_eq!(hook(&root, "Bash", json!({"command":wrapper})), json!({}));
    assert!(denied(&hook(
        &root,
        "Bash",
        json!({"command":format!("{wrapper}; touch report.docx")})
    )));
    assert!(denied(&hook(
        &root,
        "Bash",
        json!({"command":wrapper,"run_in_background":true})
    )));
}

#[test]
fn missing_or_busy_records_fail_closed_for_project_edits() {
    let root = fixture("busy");
    let held =
        ProjectSession::open_read_only(RuleSet::builtin().unwrap(), root.join(STATE_PATH)).unwrap();
    assert!(denied(&hook(
        &root,
        "Write",
        json!({"file_path":"report.docx"})
    )));
    // Outside files need no project-state lookup, even when another action holds its lock.
    assert_eq!(
        hook(&root, "Write", json!({"file_path":"../outside.txt"})),
        json!({})
    );
    drop(held);
    assert!(denied(&hook(&root, "Write", json!({}))));
    assert!(denied(&hook(
        &root,
        "apply_patch",
        json!({"command":"unrecognized patch"})
    )));
}

#[cfg(unix)]
#[test]
fn outside_symlink_into_project_does_not_bypass_edit_guard() {
    let root = fixture("symlink");
    let link = root.with_extension("alias");
    std::os::unix::fs::symlink(&root, &link).unwrap();
    assert!(denied(&hook(
        &root,
        "Write",
        json!({"file_path":link.join("new.docx")})
    )));
}

#[cfg(not(feature = "approved-plan"))]
#[test]
fn verify_opens_edits_and_closing_it_removes_permission() {
    let root = fixture("verify");
    let mut s = ProjectSession::open(RuleSet::builtin().unwrap(), root.join(STATE_PATH)).unwrap();
    *s.project_mut() = common::bootstrap_from(s.project().clone());
    common::up_to_verify(s.project_mut());
    s.commit().unwrap();
    drop(s);
    assert_eq!(
        hook(&root, "Write", json!({"file_path":"report.docx"})),
        json!({})
    );
    assert!(denied(&hook(
        &root,
        "Write",
        json!({"file_path":".gil/state.yaml"})
    )));
    #[cfg(target_os = "macos")]
    {
        let output = Command::new(env!("CARGO_BIN_EXE_gil-write-guard"))
            .args(["run", "--root"])
            .arg(&root)
            .args([
                "--",
                "printf verified > report.docx && printf forbidden > .gil/injected",
            ])
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert_eq!(fs::read(root.join("report.docx")).unwrap(), b"verified");
        assert!(!root.join(".gil/injected").exists());
    }
    let mut s = ProjectSession::open(RuleSet::builtin().unwrap(), root.join(STATE_PATH)).unwrap();
    common::close_here(
        s.project_mut(),
        common::full_report(
            &common::spec(),
            gil::CycleKind::Experiment,
            NodeKind::Verify,
        ),
    )
    .unwrap();
    s.commit().unwrap();
    drop(s);
    assert!(denied(&hook(
        &root,
        "Write",
        json!({"file_path":"report.docx"})
    )));
}

#[cfg(target_os = "macos")]
#[test]
fn foreground_command_holds_gil_lock_until_it_finishes() {
    let root = fixture("lifetime");
    let mut child = Command::new(env!("CARGO_BIN_EXE_gil-write-guard"))
        .args(["run", "--root"])
        .arg(&root)
        .args([
            "--",
            "printf ready > ../ready; while test ! -f ../release; do sleep 0.05; done",
        ])
        .current_dir(&root)
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !root.join("../ready").exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    if !root.join("../ready").exists() {
        let _ = child.kill();
        let _ = child.wait();
        panic!("guarded command did not start");
    }
    let busy =
        ProjectSession::open_read_only(RuleSet::builtin().unwrap(), root.join(STATE_PATH)).is_err();
    fs::write(root.join("../release"), b"release").unwrap();
    assert!(child.wait().unwrap().success());
    assert!(
        busy,
        "Verify close could race the running foreground command"
    );
    assert!(
        ProjectSession::open_read_only(RuleSet::builtin().unwrap(), root.join(STATE_PATH)).is_ok()
    );
}

#[cfg(target_os = "macos")]
#[test]
fn guarded_shell_really_denies_project_writes_and_permits_outside_output() {
    let root = fixture("sandbox");
    let output = Command::new(env!("CARGO_BIN_EXE_gil-write-guard"))
        .args(["run", "--root"])
        .arg(&root)
        .args([
            "--",
            "printf outside > ../outside-result.txt && printf blocked > report.docx",
        ])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!root.join("report.docx").exists());
    assert_eq!(
        fs::read(root.join("../outside-result.txt")).unwrap(),
        b"outside"
    );
    let output = Command::new(env!("CARGO_BIN_EXE_gil-write-guard"))
        .args(["run", "--root"])
        .arg(&root)
        .args(["--", "test -d .gil && printf read-ok"])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"read-ok");
    let output = Command::new(env!("CARGO_BIN_EXE_gil-write-guard"))
        .args(["run", "--root"])
        .arg(&root)
        .args(["--", "mv ../project ../moved-project"])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(root.exists());
}

#[cfg(target_os = "macos")]
#[test]
fn read_only_records_allow_guard_lookup_without_write_access() {
    let root = fixture("read-only-records");
    let before = fs::read(root.join(STATE_PATH)).unwrap();
    let profile = format!("(version 1)(allow default)(deny file-write* (subpath {}))",
        serde_json::to_string(root.join(".gil").to_str().unwrap()).unwrap());
    let input = json!({"hook_event_name":"PreToolUse","tool_name":"Write","cwd":root,
        "tool_input":{"file_path":"report.docx"}});
    let mut kid = Command::new("/usr/bin/sandbox-exec")
        .args(["-p", &profile, env!("CARGO_BIN_EXE_gil-write-guard"), "hook", "--root"])
        .arg(&root).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    kid.stdin.take().unwrap().write_all(input.to_string().as_bytes()).unwrap();
    let out = kid.wait_with_output().unwrap();
    assert!(out.status.success());
    let decision: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(denied(&decision));
    assert!(decision["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap()
        .contains("열린 Experiment Verify"), "must read state, not deny on lock error: {decision}");
    let out = Command::new("/usr/bin/sandbox-exec")
        .args(["-p", &profile, env!("CARGO_BIN_EXE_gil-write-guard"), "run", "--root"])
        .arg(&root).args(["--", "printf outside > ../read-only-outside.txt; printf blocked > report.docx"])
        .current_dir(&root).output().unwrap();
    assert!(!out.status.success());
    // A restrictive macOS parent cannot install a second sandbox. The guard reports the
    // approved-wrapper route before running any payload; it must not silently drop protection.
    assert!(String::from_utf8_lossy(&out.stderr).contains("guard_sandbox_unavailable"));
    assert!(!root.join("../read-only-outside.txt").exists());
    assert!(!root.join("report.docx").exists());
    assert_eq!(fs::read(root.join(STATE_PATH)).unwrap(), before);
}

#[cfg(target_os = "macos")]
#[test]
fn unavailable_state_restricts_project_but_does_not_block_external_shell_work() {
    let root = fixture("unavailable-state");
    let held = ProjectSession::open_read_only(RuleSet::builtin().unwrap(), root.join(STATE_PATH)).unwrap();
    let run = || Command::new(env!("CARGO_BIN_EXE_gil-write-guard"))
        .args(["run", "--root"]).arg(&root)
        .args(["--", "printf outside > ../unavailable-outside.txt && printf blocked > report.docx"])
        .current_dir(&root).output().unwrap();
    let out = run();
    assert!(!out.status.success());
    assert_eq!(fs::read(root.join("../unavailable-outside.txt")).unwrap(), b"outside");
    assert!(!root.join("report.docx").exists());
    drop(held);
    fs::write(root.join(STATE_PATH), "corrupt test record").unwrap();
    fs::remove_file(root.join("../unavailable-outside.txt")).unwrap();
    let out = run();
    assert!(!out.status.success());
    assert_eq!(fs::read(root.join("../unavailable-outside.txt")).unwrap(), b"outside");
    assert!(!root.join("report.docx").exists());
    assert_eq!(fs::read(root.join(STATE_PATH)).unwrap(), b"corrupt test record");
}
