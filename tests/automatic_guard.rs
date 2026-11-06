//! Both Host event dialects, real Core state, isolated settings; never installed Host acceptance.
#[cfg(all(target_os = "macos", not(feature = "approved-plan")))]
mod common;
use gil::{ActionContract, NodeKind, ProjectSession, RuleSet, STATE_PATH};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

struct Rig {
    base: PathBuf,
    root: PathBuf,
    home: PathBuf,
}
impl Rig {
    fn new(label: &str) -> Self {
        let base = std::env::temp_dir().join(format!("gil-auto-{label}-{}", std::process::id()));
        let root = base.join("project");
        let home = base.join("home");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&home).unwrap();
        ProjectSession::start(RuleSet::builtin().unwrap(), root.join(STATE_PATH))
            .unwrap()
            .commit()
            .unwrap();
        Self {
            base: base.canonicalize().unwrap(),
            root: root.canonicalize().unwrap(),
            home: home.canonicalize().unwrap(),
        }
    }
    fn hook(&self, input: Value) -> Value {
        self.host_hook("codex", input)
    }
    fn host_hook(&self, host: &str, mut input: Value) -> Value {
        input["session_id"] = input.get("session_id").cloned().unwrap_or(json!("s1"));
        input["cwd"] = json!(self.home); // Selected project is outside the chat cwd.
        let mut child = Command::new(env!("CARGO_BIN_EXE_gil"))
            .args(["write-guard", "hook", "--auto", "--host", host])
            .env("HOME", &self.home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.to_string().as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success());
        serde_json::from_slice(&out.stdout).unwrap()
    }
    fn post(&self, name: &str, extra: Value) -> Value {
        let mut args = extra;
        args["project_root"] = json!(self.root);
        self.hook(json!({"hook_event_name":"PostToolUse","tool_name":name,"tool_input":args,"tool_response":{"structuredContent":{"ok":true}}}))
    }
    fn pre(&self, name: &str, args: Value) -> Value {
        self.hook(json!({"hook_event_name":"PreToolUse","tool_name":name,"tool_input":args}))
    }
    fn stop(&self, message: &str) -> Value {
        self.hook(json!({"hook_event_name":"Stop","stop_hook_active":false,"last_assistant_message":message}))
    }
    fn question(&self) {
        let mut s =
            ProjectSession::open(RuleSet::builtin().unwrap(), self.root.join(STATE_PATH)).unwrap();
        s.open_action_step(
            NodeKind::Question,
            ActionContract::new("clarify goal", "ask user", "user gives goal"),
        )
        .unwrap();
        s.commit().unwrap();
    }
}
impl Drop for Rig {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}
fn denied(v: &Value) -> bool {
    v["hookSpecificOutput"]["permissionDecision"] == "deny"
}

#[test]
fn new_projects_enroll_without_local_hook_files_and_other_chats_are_not_gated() {
    let r = Rig::new("enroll");
    assert_eq!(r.pre("Bash", json!({"command":"true"})), json!({}));
    r.post("mcp__gil__gil_start", json!({}));
    assert!(denied(
        &r.pre("Write", json!({"file_path":r.root.join("report.docx")}))
    ));
    assert!(denied(&r.pre("exec_command", json!({"cmd":"echo result"}))));
    assert_eq!(
        r.pre("Write", json!({"file_path":r.base.join("unrelated.txt")})),
        json!({})
    );
    assert_eq!(
        r.pre("WebSearch", json!({"query":"file-free research"})),
        json!({})
    );
    assert_eq!(r.hook(json!({"hook_event_name":"Stop","session_id":"other-chat","last_assistant_message":"ordinary answer"})),json!({}));
    assert!(!r.root.join(".claude").exists());
    assert!(!r.root.join(".codex").exists());
    assert_eq!(r.stop("report complete"), json!({}));
    assert_eq!(r.pre("mcp__ccd_turn__wrap_up", json!({"ask":"what goal?","blocked":true})), json!({}));
}
#[test]
fn all_conversation_events_are_inert_and_preserve_legacy_waits_and_project_state() {
    let r = Rig::new("turns");
    r.question();
    r.post("mcp__gil_companion__gil_context", json!({}));
    let registry = r.home.join(".gil-host-guard/registry.json");
    let old = json!({"roots":[r.root],"sessions":{"codex:s1":{"root":r.root,
        "wait":{"question":"What is the goal?","state_digest":"legacy-digest"}}}});
    let old_bytes = serde_json::to_vec(&old).unwrap();
    fs::write(&registry, &old_bytes).unwrap();
    let state = fs::read(r.root.join(STATE_PATH)).unwrap();
    for host in ["claude", "codex"] {
        for id in [json!("s1"), json!("other"), Value::Null] {
            for prompt in ["a report please", "GIL 일시 중지", "continue GIL"] {
                assert_eq!(r.host_hook(host, json!({"hook_event_name":"UserPromptSubmit","session_id":id,"prompt":prompt})), json!({}));
            }
            for _ in 0..10 {
                assert_eq!(r.host_hook(host, json!({"hook_event_name":"Stop","session_id":id,"stop_hook_active":true,"last_assistant_message":"ordinary report"})), json!({}));
                assert_eq!(r.host_hook(host, json!({"hook_event_name":"PreToolUse","session_id":id,"tool_name":"mcp__ccd_turn__wrap_up","tool_input":{"ask":"unrelated answer"}})), json!({}));
            }
        }
    }
    // Resume and the retired handoff event neither attach chats nor clear old waits.
    assert_eq!(r.post("mcp__gil__gil_context", json!({})), json!({}));
    assert_eq!(r.post("mcp__gil__gil_await_user", json!({"question":"another question"})), json!({}));
    assert_eq!(fs::read(&registry).unwrap(), old_bytes);
    assert_eq!(fs::read(r.root.join(STATE_PATH)).unwrap(), state);
    assert!(denied(&r.pre("Edit", json!({"file_path":r.root.join("report.docx")}))));
}
#[test]
fn enrollment_needs_success_and_a_real_project_but_no_session_identity() {
    let r = Rig::new("no-session");
    for response in [json!({"isError":true}), json!({"ok":false}), json!({})] {
        assert_eq!(r.host_hook("claude", json!({"hook_event_name":"PostToolUse","session_id":null,
            "tool_name":"mcp__gil__gil_start","tool_input":{"project_root":r.root},"tool_response":response})), json!({}));
        assert!(!r.home.join(".gil-host-guard/registry.json").exists());
    }
    let result = r.host_hook("claude", json!({"hook_event_name":"PostToolUse","session_id":null,
        "tool_name":"mcp__gil__gil_context","tool_input":{"project_root":r.root},"tool_response":{"ok":true}}));
    assert!(result.to_string().contains("파일 보호 폴더 등록"));
    assert!(!result.to_string().contains("진행 검사"));
    assert!(denied(&r.pre("Write", json!({"file_path":r.root.join("x")}))));
    let registry: Value = serde_json::from_slice(&fs::read(r.home.join(".gil-host-guard/registry.json")).unwrap()).unwrap();
    assert_eq!(registry["sessions"], json!({}));
}
#[test]
fn failed_start_does_not_bind_or_register_and_registry_edits_are_denied() {
    let r = Rig::new("failed");
    r.hook(json!({"hook_event_name":"PostToolUse","tool_name":"mcp__gil__gil_start","tool_input":{"project_root":r.root},"tool_response":{"isError":true,"structuredContent":{"ok":false}}}));
    assert_eq!(r.stop("startup failed"), json!({}));
    r.post("mcp__gil__gil_context", json!({}));
    assert!(denied(&r.pre(
        "Write",
        json!({"file_path":r.home.join(".gil-host-guard/registry.json")})
    )));
}
#[cfg(target_os = "macos")]
#[test]
fn installer_preserves_settings_is_idempotent_and_never_writes_trust() {
    let r = Rig::new("install");
    fs::create_dir(r.home.join(".claude")).unwrap();
    fs::write(r.home.join(".claude/settings.json"),r#"{"permissions":{"allow":["Read"]},"hooks":{"Stop":[{"hooks":[{"type":"command","command":"unrelated-check"}]}]}}"#).unwrap();
    let install = || {
        let out = Command::new(env!("CARGO_BIN_EXE_gil"))
            .args(["write-guard", "install", "--auto", "--host", "both"])
            .env("HOME", &r.home)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    install();
    let before = fs::read(r.home.join(".claude/settings.json")).unwrap();
    install();
    assert_eq!(
        fs::read(r.home.join(".claude/settings.json")).unwrap(),
        before
    );
    let v: Value = serde_json::from_slice(&before).unwrap();
    assert_eq!(v["permissions"]["allow"], json!(["Read"]));
    assert_eq!(v["hooks"]["Stop"].as_array().unwrap().len(), 1);
    let c: Value =
        serde_json::from_slice(&fs::read(r.home.join(".codex/hooks.json")).unwrap()).unwrap();
    for event in ["PreToolUse", "PostToolUse"] {
        assert_eq!(c["hooks"][event].as_array().unwrap().len(), 1);
    }
    assert_eq!(fs::read_dir(r.home.join(".codex")).unwrap().count(), 1);
    assert!(c["hooks"].get("Stop").is_none());
    assert!(c["hooks"].get("UserPromptSubmit").is_none());
    assert!(!c["hooks"]["PreToolUse"][0]["matcher"].as_str().unwrap().contains("wrap_up"));
}

#[cfg(all(target_os = "macos", not(feature = "approved-plan")))]
#[test]
fn one_sandbox_protects_multiple_roots_and_only_the_open_verify_is_writable() {
    let r = Rig::new("multi");
    let second = r.base.join("second");
    fs::create_dir(&second).unwrap();
    ProjectSession::start(RuleSet::builtin().unwrap(), second.join(STATE_PATH))
        .unwrap()
        .commit()
        .unwrap();
    r.post("mcp__gil__gil_context", json!({}));
    r.hook(json!({"hook_event_name":"PostToolUse","tool_name":"mcp__gil__gil_start","tool_input":{"project_root":second},"tool_response":{"ok":true}}));
    let mut session =
        ProjectSession::open(RuleSet::builtin().unwrap(), r.root.join(STATE_PATH)).unwrap();
    *session.project_mut() = common::bootstrap_from(session.project().clone());
    common::up_to_verify(session.project_mut());
    session.commit().unwrap();
    drop(session);
    let run = |payload: &str| {
        Command::new(env!("CARGO_BIN_EXE_gil"))
            .args(["write-guard", "run", "--auto", "--", payload])
            .env("HOME", &r.home)
            .current_dir(&r.base)
            .output()
            .unwrap()
    };
    assert!(
        run("printf verified > project/report.docx")
            .status
            .success()
    );
    assert!(
        !run("printf forbidden > second/report.docx")
            .status
            .success()
    );
    assert!(run("printf unrelated > outside.txt").status.success());
    assert!(
        !run("printf forbidden > project/.gil/injected")
            .status
            .success()
    );
    assert!(
        !run("printf forbidden > home/.gil-host-guard/registry.json")
            .status
            .success()
    );
    assert_eq!(fs::read(r.root.join("report.docx")).unwrap(), b"verified");
    assert!(!second.join("report.docx").exists());
    let denial = r.pre("Bash", json!({"command":"echo x"}));
    let wrapped = denial["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .unwrap()
        .split('\n')
        .last()
        .unwrap();
    assert_eq!(r.pre("Bash", json!({"command":wrapped})), json!({}));
    assert!(denied(&r.pre(
        "Bash",
        json!({"command":format!("{wrapped}; touch second/report.docx")})
    )));
}

#[test]
fn host_identity_is_explicit_and_turn_scope_never_exempts_files() {
    let r = Rig::new("host-scope");
    r.post("mcp__gil__gil_context", json!({}));
    // Both hosts expose Bash; tool name, cwd and session id alone are not Host identity.
    assert_eq!(r.host_hook("claude", json!({"hook_event_name":"Stop"})), json!({}));
    assert_eq!(r.stop("unfinished"), json!({}));
    for host in ["claude", "codex"] {
        let external = r.host_hook(host, json!({"hook_event_name":"PreToolUse", "session_id":"unbound",
            "tool_name":"Write", "tool_input":{"file_path":r.base.join("external.txt")}}));
        assert_eq!(external, json!({}));
        let protected = r.host_hook(host, json!({"hook_event_name":"PreToolUse", "session_id":"unbound",
            "tool_name":"Write", "tool_input":{"file_path":r.root.join("artifact.txt")}}));
        assert!(denied(&protected));
        assert_eq!(r.host_hook(host, json!({"hook_event_name":"PreToolUse", "session_id":"unbound",
            "tool_name":"mcp__ccd_turn__wrap_up", "tool_input":{"ask":"done"}})), json!({}));
    }
    // No session id cannot turn off file protection.
    assert!(denied(&r.host_hook("claude", json!({"hook_event_name":"PreToolUse", "session_id":null,
        "tool_name":"Edit", "tool_input":{"file_path":r.root.join("artifact.txt")}}))));
    let claude = r.host_hook("claude", json!({"hook_event_name":"PreToolUse", "session_id":"unbound",
        "tool_name":"Bash", "tool_input":{"command":"printf 'hello'", "timeout":1234, "description":"ordinary build"}}));
    let out = &claude["hookSpecificOutput"];
    assert_eq!(out["permissionDecision"], "ask");
    assert_eq!(out["updatedInput"]["timeout"], 1234);
    assert_eq!(out["updatedInput"]["description"], "ordinary build");
    assert!(out["updatedInput"]["command"].as_str().unwrap().contains("'run' '--auto' '--host' 'claude' '--'"));
    assert!(!claude.to_string().contains("require_escalated"));
    assert!(out["updatedInput"].get("dangerouslyDisableSandbox").is_none());
    let codex = r.pre("Bash", json!({"command":"echo build"}));
    assert!(denied(&codex));
    assert!(codex.to_string().contains("require_escalated"));
}

#[test]
fn shared_registry_failure_does_not_impose_a_global_turn_gate() {
    let r = Rig::new("damaged-registry");
    r.post("mcp__gil__gil_context", json!({}));
    let registry = r.home.join(".gil-host-guard/registry.json");
    fs::write(&registry, b"damaged fixture").unwrap();
    for event in ["Stop", "UserPromptSubmit"] {
        let v = r.hook(json!({"hook_event_name":event,"session_id":"unrelated","prompt":"ordinary work"}));
        assert_eq!(v, json!({}));
    }
    assert!(denied(&r.pre("Bash", json!({"command":"echo x"}))));
    assert!(denied(&r.pre("Write", json!({"file_path":r.root.join("x")}))));
    assert!(!denied(&r.pre("mcp__ccd_turn__wrap_up", json!({"ask":"outside complete"}))));
    assert_eq!(r.pre("WebSearch", json!({"query":"no files"})), json!({}));
    assert_eq!(fs::read(&registry).unwrap(), b"damaged fixture");
}

#[test]
fn registry_lock_contention_preserves_scope_and_existing_records() {
    let r = Rig::new("registry-lock");
    r.post("mcp__gil__gil_context", json!({}));
    let lock = fs::OpenOptions::new().read(true).write(true)
        .open(r.home.join(".gil-host-guard/registry.lock")).unwrap();
    lock.lock().unwrap();
    assert!(denied(&r.pre("Bash", json!({"command":"echo x"}))));
    for event in ["Stop", "UserPromptSubmit"] {
        assert_eq!(r.hook(json!({"hook_event_name":event,"session_id":"s1"})), json!({}));
    }
    assert_eq!(r.pre("wrap_up", json!({})), json!({}));
    drop(lock);
    assert_eq!(r.stop("unfinished"), json!({}));
    assert_eq!(r.hook(json!({"hook_event_name":"Stop","session_id":"other"})), json!({}));
}

#[cfg(target_os = "macos")]
#[test]
fn claude_unbound_shell_runs_outside_work_but_cannot_reach_protected_paths() {
    let r = Rig::new("claude-shell");
    r.post("mcp__gil__gil_context", json!({}));
    std::os::unix::fs::symlink(&r.root, r.base.join("alias")).unwrap();
    let state = fs::read(r.root.join(STATE_PATH)).unwrap();
    let execute = |command: String| {
        let verdict = r.host_hook("claude", json!({"hook_event_name":"PreToolUse","session_id":"ordinary",
            "tool_name":"Bash", "tool_input":{"command":command}}));
        assert_eq!(verdict["hookSpecificOutput"]["permissionDecision"], "ask");
        // Synthetic permission acceptance; never claims real Claude approval/UI acceptance.
        Command::new("/bin/sh").args(["-c", verdict["hookSpecificOutput"]["updatedInput"]["command"].as_str().unwrap()])
            .env("HOME", &r.home).current_dir(&r.base).output().unwrap()
    };
    assert!(execute("mkdir -p build && printf package > build/prepared.txt".into()).status.success());
    for command in [format!("printf x > '{}/bad'", r.root.display()),
        "cd project && printf x > bad".into(), "printf x > alias/bad".into(),
        "mv project renamed".into()] {
        assert!(!execute(command).status.success());
        assert!(!r.root.join("bad").exists());
    }
    let held = ProjectSession::open_read_only(RuleSet::builtin().unwrap(), r.root.join(STATE_PATH)).unwrap();
    assert!(execute("printf outside > outside.txt".into()).status.success());
    assert!(!execute("printf x > project/bad".into()).status.success());
    drop(held);
    fs::write(r.root.join(STATE_PATH), b"damaged synthetic state").unwrap();
    assert!(execute("printf outside > outside.txt".into()).status.success());
    assert!(!execute("printf x > project/bad".into()).status.success());
    assert_eq!(r.stop("done"), json!({}));
    fs::write(r.root.join(STATE_PATH), state).unwrap();
}

#[cfg(target_os = "macos")]
#[test]
fn update_replaces_old_core_hooks_preserves_settings_and_backups() {
    let r = Rig::new("update");
    r.post("mcp__gil__gil_context", json!({}));
    let registry_path = r.home.join(".gil-host-guard/registry.json");
    let registry_before = fs::read(&registry_path).unwrap();
    let state_before = fs::read(r.root.join(STATE_PATH)).unwrap();
    let install = |exe: &std::path::Path| {
        let out = Command::new(exe).args(["write-guard","install","--auto","--host","both"])
            .env("HOME", &r.home).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    };
    fs::create_dir(r.home.join(".claude")).unwrap();
    let mut old = json!({"permissions":{"deny":["Read(secret)"]}, "sandbox":{"enabled":true},
        "hooks":{"PreToolUse":[{"matcher":"Bash", "hooks":[{"type":"command",
            "command":"'/old/core/gil' 'write-guard' 'hook' '--auto'"}]},
            {"hooks":[{"type":"command","command":"echo custom; '/old/core/gil' 'write-guard' 'hook' '--auto'"}]}]}});
    // Legacy generated commands coexist with unrelated commands in the same row.
    for event in ["Stop", "UserPromptSubmit"] {
        old["hooks"][event] = json!([
            {"matcher":"*", "hooks":[
                {"type":"command","command":"'/old/core/gil' 'write-guard' 'hook' '--auto' '--host' 'claude'"},
                {"type":"command","command":"unrelated-check"}]},
            {"hooks":[{"type":"command","command":"'/old/core/gil' 'write-guard' 'hook' '--auto'"}]}]);
    }
    let old_bytes = serde_json::to_vec(&old).unwrap();
    let settings_path = r.home.join(".claude/settings.json");
    fs::write(&settings_path, &old_bytes).unwrap();
    install(std::path::Path::new(env!("CARGO_BIN_EXE_gil")));
    let first = fs::read(&settings_path).unwrap();
    let next_dir = r.base.join("new core's version");
    fs::create_dir(&next_dir).unwrap();
    let next = next_dir.join("gil");
    fs::copy(env!("CARGO_BIN_EXE_gil"), &next).unwrap();
    install(&next);
    let updated = fs::read(&settings_path).unwrap();
    install(&next);
    assert_eq!(fs::read(&settings_path).unwrap(), updated);
    let v: Value = serde_json::from_slice(&updated).unwrap();
    assert_eq!(v["permissions"], old["permissions"]);
    assert_eq!(v["sandbox"], old["sandbox"]);
    for event in ["Stop", "UserPromptSubmit"] {
        assert_eq!(v["hooks"][event], json!([{"matcher":"*", "hooks":[{"type":"command","command":"unrelated-check"}]}]));
    }
    assert_eq!(v["hooks"]["PreToolUse"].as_array().unwrap().len(), 2);
    assert_eq!(v["hooks"]["PreToolUse"][0], old["hooks"]["PreToolUse"][1]);
    let backups: Vec<_> = fs::read_dir(r.home.join(".gil-host-guard")).unwrap()
        .flatten().filter(|e| e.file_name().to_string_lossy().starts_with("claude-settings-"))
        .map(|e| fs::read(e.path()).unwrap()).collect();
    assert!(backups.contains(&old_bytes));
    assert!(backups.contains(&first));
    let status = |exe: &std::path::Path| -> Value {
        let out = Command::new(exe).args(["write-guard","status","--auto"]).env("HOME", &r.home).output().unwrap();
        assert!(out.status.success()); serde_json::from_slice(&out.stdout).unwrap()
    };
    assert_eq!(fs::read(&registry_path).unwrap(), registry_before);
    assert_eq!(fs::read(r.root.join(STATE_PATH)).unwrap(), state_before);
    for host in ["codex", "claude"] {
        assert_eq!(status(&next)["installed_hooks"][host]["status"], "matches_current_core");
    }
    // A leftover generated turn hook makes status stale even if both file events match.
    let mut stale = v.clone();
    stale["hooks"]["Stop"].as_array_mut().unwrap().push(old["hooks"]["Stop"][1].clone());
    fs::write(&settings_path, serde_json::to_vec(&stale).unwrap()).unwrap();
    assert_eq!(status(&next)["installed_hooks"]["claude"]["status"], "refresh_required");
    install(&next);
    assert_eq!(status(&next)["installed_hooks"]["claude"]["status"], "matches_current_core");
    assert_eq!(status(std::path::Path::new(env!("CARGO_BIN_EXE_gil")))["installed_hooks"]["claude"]["status"], "refresh_required");
}

#[cfg(target_os = "macos")]
#[test]
fn nested_sandbox_failure_is_preexecution_and_host_specific() {
    let r = Rig::new("host-sandbox");
    r.post("mcp__gil__gil_context", json!({}));
    let profile = format!("(version 1)(allow default)(deny file-write* (subpath {}))",
        serde_json::to_string(r.root.to_str().unwrap()).unwrap());
    for host in ["claude", "codex"] {
        let out = Command::new("/usr/bin/sandbox-exec")
            .args(["-p", &profile, env!("CARGO_BIN_EXE_gil"), "write-guard", "run", "--auto", "--host", host, "--",
                "printf should-not-run > outside.txt"])
            .env("HOME", &r.home).current_dir(&r.base).output().unwrap();
        assert!(!out.status.success());
        let error = String::from_utf8_lossy(&out.stderr);
        assert!(error.contains("guard_sandbox_unavailable"));
        assert_eq!(error.contains("require_escalated"), host == "codex");
        assert!(!r.base.join("outside.txt").exists());
    }
}

#[test]
fn legacy_bindings_are_preserved_without_attaching_a_new_host_session() {
    let r = Rig::new("legacy-binding");
    r.post("mcp__gil__gil_context", json!({}));
    let registry = r.home.join(".gil-host-guard/registry.json");
    let old = json!({"roots":[r.root], "sessions":{"s1":{"root":r.root,"wait":null}}});
    fs::write(&registry, serde_json::to_vec(&old).unwrap()).unwrap();
    for host in ["codex", "claude"] {
        assert_eq!(r.host_hook(host, json!({"hook_event_name":"Stop"})), json!({}));
        assert!(denied(&r.host_hook(host, json!({"hook_event_name":"PreToolUse", "tool_name":"Write",
            "tool_input":{"file_path":r.root.join("x")}}))));
    }
    r.post("mcp__gil__gil_context", json!({}));
    assert_eq!(r.stop("resumed"), json!({}));
    let after: Value = serde_json::from_slice(&fs::read(&registry).unwrap()).unwrap();
    assert_eq!(after["sessions"]["s1"], old["sessions"]["s1"]);
}
