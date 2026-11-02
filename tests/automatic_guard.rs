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
    fn hook(&self, mut input: Value) -> Value {
        input["session_id"] = input.get("session_id").cloned().unwrap_or(json!("s1"));
        input["cwd"] = json!(self.home); // Selected project is outside the chat cwd.
        let mut child = Command::new(env!("CARGO_BIN_EXE_gil"))
            .args(["write-guard", "hook", "--auto"])
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
    assert_eq!(r.stop("report complete")["decision"], "block");
    assert!(denied(&r.pre(
        "mcp__ccd_turn__wrap_up",
        json!({"ask":"what goal?","blocked":true})
    )));
}
#[test]
fn waiting_requires_current_open_question_and_expires_on_next_user_message() {
    let r = Rig::new("turns");
    r.post("mcp__gil_companion__gil_context", json!({}));
    let invalid = r.post(
        "mcp__gil__gil_await_user",
        json!({"question":"What is the goal?"}),
    );
    assert!(invalid.to_string().contains("실패"));
    r.question();
    assert_eq!(r.stop("report complete")["decision"], "block");
    r.post(
        "mcp__gil__gil_await_user",
        json!({"question":"What is the goal?"}),
    );
    assert_eq!(r.stop("Question opened. What is the goal?"), json!({}));
    assert_eq!(r.stop("Here is the completed report.")["decision"], "block");
    r.hook(json!({"hook_event_name":"UserPromptSubmit","prompt":"a report please"}));
    assert_eq!(r.stop("What is the goal?")["decision"], "block");
    // A continuation never silently bypasses the gate merely because stop_hook_active is true.
    assert_eq!(r.hook(json!({"hook_event_name":"Stop","stop_hook_active":true,"last_assistant_message":"done"}))["decision"],"block");
    r.hook(json!({"hook_event_name":"UserPromptSubmit","prompt":"GIL 일시 중지"}));
    assert_eq!(r.stop("unrelated outside answer"), json!({}));
    assert!(denied(
        &r.pre("Edit", json!({"file_path":r.root.join("report.docx")}))
    ));
    r.post("mcp__gil__gil_context", json!({}));
    assert_eq!(r.stop("resumed but unfinished")["decision"], "block");
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
    assert_eq!(v["hooks"]["Stop"].as_array().unwrap().len(), 2);
    let c: Value =
        serde_json::from_slice(&fs::read(r.home.join(".codex/hooks.json")).unwrap()).unwrap();
    for event in ["PreToolUse", "PostToolUse", "UserPromptSubmit", "Stop"] {
        assert_eq!(c["hooks"][event].as_array().unwrap().len(), 1);
    }
    assert_eq!(fs::read_dir(r.home.join(".codex")).unwrap().count(), 1);
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
