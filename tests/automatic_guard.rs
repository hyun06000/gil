//! No installed Host acceptance: isolated settings, synthetic events and real Core records.
use gil::{ProjectSession, RuleSet, STATE_PATH};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Output, Stdio},
};

struct Rig {
    base: PathBuf,
    home: PathBuf,
    root: PathBuf,
}
impl Rig {
    fn new(label: &str) -> Self {
        let base = std::env::temp_dir().join(format!("gil-unhook-{label}-{}", std::process::id()));
        let home = base.join("home");
        let root = base.join("project");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&root).unwrap();
        ProjectSession::start(RuleSet::builtin().unwrap(), root.join(STATE_PATH))
            .unwrap()
            .commit()
            .unwrap();
        Self {
            base: base.canonicalize().unwrap(),
            home: home.canonicalize().unwrap(),
            root: root.canonicalize().unwrap(),
        }
    }
    fn command(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_gil"));
        c.arg("write-guard")
            .args(args)
            .env("HOME", &self.home)
            .current_dir(&self.base);
        c
    }
    fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().unwrap()
    }
    fn json(&self, args: &[&str]) -> Value {
        let out = self.run(args);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    }
    fn hook(&self, args: &[&str], input: &str) -> Value {
        let mut c = self
            .command(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        c.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
        let out = c.wait_with_output().unwrap();
        assert!(out.status.success());
        assert!(out.stderr.is_empty());
        serde_json::from_slice(&out.stdout).unwrap()
    }
    fn settings(&self, host: &str) -> PathBuf {
        self.home.join(if host == "codex" {
            ".codex/hooks.json"
        } else {
            ".claude/settings.json"
        })
    }
    fn write(&self, host: &str, value: &Value) -> Vec<u8> {
        let p = self.settings(host);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        let b = serde_json::to_vec_pretty(value).unwrap();
        fs::write(p, &b).unwrap();
        b
    }
    fn old_command(&self, host: Option<&str>) -> String {
        let exe = self.base.join("old core's").join("gil");
        let mut words = vec![exe.to_str().unwrap(), "write-guard", "hook", "--auto"];
        if let Some(h) = host {
            words.extend(["--host", h]);
        }
        quote(&words)
    }
    fn guard_dir(&self) -> PathBuf {
        let p = self.home.join(".gil-host-guard");
        fs::create_dir_all(&p).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
        }
        p
    }
}
impl Drop for Rig {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}
fn quote(words: &[&str]) -> String {
    words
        .iter()
        .map(|s| format!("'{}'", s.replace('\'', "'\\''")))
        .collect::<Vec<_>>()
        .join(" ")
}
fn entry(command: String) -> Value {
    json!({"matcher":"*","hooks":[{"type":"command","command":command}]})
}

#[test]
fn legacy_hook_events_never_deny_rewrite_or_register_any_session() {
    let r = Rig::new("events");
    let before = fs::read(r.root.join(STATE_PATH)).unwrap();
    for host in ["codex", "claude"] {
        for session in [json!("bound"), json!("unrelated"), Value::Null] {
            for name in [
                "Bash",
                "exec_command",
                "Write",
                "Edit",
                "apply_patch",
                "MultiEdit",
                "NotebookEdit",
                "mcp__ccd_turn__wrap_up",
                "WebSearch",
            ] {
                let input = json!({"hook_event_name":"PreToolUse","session_id":session,"cwd":r.home,"tool_name":name,
                "tool_input":{"command":format!("cd '{}' && printf x > output",r.root.display()),"file_path":r.root.join("x"),"run_in_background":true}});
                assert_eq!(
                    r.hook(&["hook", "--auto", "--host", host], &input.to_string()),
                    json!({})
                );
            }
            for event in ["Stop", "UserPromptSubmit", "PostToolUse"] {
                let input = json!({"hook_event_name":event,"tool_name":"mcp__gil__gil_start","tool_input":{"project_root":r.root},"tool_response":{"ok":true},"prompt":"continue","stop_hook_active":true});
                assert_eq!(
                    r.hook(&["hook", "--auto", "--host", host], &input.to_string()),
                    json!({})
                );
            }
        }
    }
    assert!(!r.home.join(".gil-host-guard").exists());
    assert_eq!(fs::read(r.root.join(STATE_PATH)).unwrap(), before);
    assert!(!r.root.join(".codex").exists());
    assert!(!r.root.join(".claude").exists());
}
#[test]
fn malformed_old_hooks_are_inert_even_without_home_or_a_project() {
    let r = Rig::new("malformed");
    for args in [
        vec!["hook"],
        vec!["hook", "--auto", "--host", "unknown"],
        vec!["hook", "--root", "/missing-project"],
    ] {
        for input in ["not JSON", "", "{\"tool_name\":null}"] {
            assert_eq!(r.hook(&args, input), json!({}));
        }
    }
    let out = r
        .command(&["hook", "--auto"])
        .env_remove("HOME")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap(),
        json!({})
    );
    assert!(!r.home.join(".gil-host-guard").exists());
}
#[test]
fn damaged_locked_registry_and_project_never_affect_host_calls_or_cleanup() {
    let r = Rig::new("damage");
    let dir = r.guard_dir();
    let registry = dir.join("registry.json");
    fs::write(&registry, b"damaged registry").unwrap();
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join("registry.lock"))
        .unwrap();
    lock.lock().unwrap();
    fs::write(r.root.join(STATE_PATH), b"damaged project fixture").unwrap();
    assert_eq!(r.hook(&["hook","--auto"],&json!({"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"echo normal"}}).to_string()),json!({}));
    r.write(
        "claude",
        &json!({"hooks":{"PreToolUse":[entry(r.old_command(Some("claude")))]}}),
    );
    r.json(&["uninstall", "--auto", "--host", "claude"]);
    assert_eq!(fs::read(&registry).unwrap(), b"damaged registry");
    assert_eq!(
        fs::read(r.root.join(STATE_PATH)).unwrap(),
        b"damaged project fixture"
    );
}
#[test]
fn fresh_install_creates_no_hooks_or_registry_and_retired_installer_cannot_restore_them() {
    let r = Rig::new("fresh");
    assert!(
        !r.run(&["install", "--auto", "--host", "both"])
            .status
            .success()
    );
    for args in [
        vec!["config", "--auto", "--host", "codex"],
        vec!["config", "--root", "/missing"],
    ] {
        assert_eq!(r.json(&args), json!({"hooks":{}}));
    }
    r.json(&["uninstall", "--auto", "--host", "both"]);
    let status = r.json(&["status", "--auto"]);
    for host in ["codex", "claude"] {
        assert!(!r.settings(host).exists());
        assert_eq!(status["installed_hooks"][host]["status"], "not_installed");
    }
    assert!(!r.home.join(".gil-host-guard").exists());
}
#[test]
fn uninstall_preserves_mixed_rows_settings_original_backups_and_registry_bytes() {
    let r = Rig::new("remove");
    let dir = r.guard_dir();
    let registry = dir.join("registry.json");
    let record = fs::read(r.root.join(STATE_PATH)).unwrap();
    let bytes=serde_json::to_vec(&json!({"roots":[r.root],"sessions":{"claude:s1":{"root":r.root,"wait":{"question":"old"}}}})).unwrap();
    fs::write(&registry, &bytes).unwrap();
    let mut originals = Vec::new();
    for host in ["codex", "claude"] {
        let old = r.old_command(Some(host));
        let standalone = r.base.join("gil-write-guard");
        let root_cmd = quote(&[
            standalone.to_str().unwrap(),
            "hook",
            "--root",
            r.root.to_str().unwrap(),
        ]);
        let value = json!({"permissions":{"deny":["Read(secret)"]},"sandbox":{"enabled":true},"enabledPlugins":{"unrelated":true},
            "hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":old},{"type":"command","command":"unrelated"}]}],
            "PostToolUse":[entry(r.old_command(None))],"Stop":[entry(root_cmd)],"UserPromptSubmit":[],"SessionStart":[entry("unrelated-start".into())]}});
        originals.push((host, r.write(host, &value)));
    }
    let status = r.json(&["status", "--auto"]);
    assert_eq!(
        status["installed_hooks"]["codex"]["status"],
        "removal_required"
    );
    let result = r.json(&["uninstall", "--auto", "--host", "both"]);
    for (host, old) in originals {
        let new: Value = serde_json::from_slice(&fs::read(r.settings(host)).unwrap()).unwrap();
        let previous: Value = serde_json::from_slice(&old).unwrap();
        for key in ["permissions", "sandbox", "enabledPlugins"] {
            assert_eq!(new[key], previous[key]);
        }
        assert_eq!(
            new["hooks"]["PreToolUse"],
            json!([{"matcher":"Bash","hooks":[{"type":"command","command":"unrelated"}]}])
        );
        assert_eq!(
            new["hooks"]["SessionStart"],
            previous["hooks"]["SessionStart"]
        );
        assert_eq!(new["hooks"]["UserPromptSubmit"], json!([]));
        assert!(new["hooks"].get("Stop").is_none());
        assert!(new["hooks"].get("PostToolUse").is_none());
        let item = result["updated"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["host"] == host)
            .unwrap();
        assert_eq!(fs::read(item["backup"].as_str().unwrap()).unwrap(), old);
        assert_eq!(item["removed_commands"], 3);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(item["backup"].as_str().unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
    let before: Vec<_> = ["codex", "claude"]
        .iter()
        .map(|h| fs::read(r.settings(h)).unwrap())
        .collect();
    let second = r.json(&["uninstall", "--auto", "--host", "both"]);
    assert!(
        second["updated"]
            .as_array()
            .unwrap()
            .iter()
            .all(|v| v["changed"] == false)
    );
    assert_eq!(
        before,
        ["codex", "claude"]
            .iter()
            .map(|h| fs::read(r.settings(h)).unwrap())
            .collect::<Vec<_>>()
    );
    assert_eq!(fs::read(&registry).unwrap(), bytes);
    assert_eq!(fs::read(r.root.join(STATE_PATH)).unwrap(), record);
}
#[test]
fn arbitrary_shell_combinations_are_preserved_and_require_review() {
    let r = Rig::new("custom");
    let original=r.write("claude",&json!({"hooks":{"PreToolUse":[entry(format!("{}; echo custom",r.old_command(Some("claude")))),entry(format!("echo before; {}",r.old_command(None))) ]}}));
    r.json(&["uninstall", "--auto", "--host", "claude"]);
    assert_eq!(fs::read(r.settings("claude")).unwrap(), original);
    assert_eq!(
        r.json(&["status", "--auto"])["installed_hooks"]["claude"]["status"],
        "custom_review_required"
    );
    assert!(!r.home.join(".gil-host-guard").exists());
}
#[test]
fn malformed_second_host_is_preflighted_before_changing_first() {
    let r = Rig::new("preflight");
    let original = r.write(
        "codex",
        &json!({"hooks":{"PreToolUse":[entry(r.old_command(Some("codex")))]}}),
    );
    r.write("claude", &json!({"hooks":"damaged fixture"}));
    assert!(
        !r.run(&["uninstall", "--auto", "--host", "both"])
            .status
            .success()
    );
    assert_eq!(fs::read(r.settings("codex")).unwrap(), original);
    assert!(!r.home.join(".gil-host-guard").exists());
}
#[test]
fn cleanup_lock_contention_preserves_settings_and_does_not_gate_other_tools() {
    let r = Rig::new("lock");
    let dir = r.guard_dir();
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join("cleanup.lock"))
        .unwrap();
    lock.lock().unwrap();
    let original = r.write(
        "codex",
        &json!({"hooks":{"PreToolUse":[entry(r.old_command(Some("codex")))]}}),
    );
    assert!(
        !r.run(&["uninstall", "--auto", "--host", "codex"])
            .status
            .success()
    );
    assert_eq!(fs::read(r.settings("codex")).unwrap(), original);
    assert_eq!(r.hook(&["hook", "--auto"], "{}"), json!({}));
}
#[cfg(unix)]
#[test]
fn symlinked_host_settings_are_not_followed_or_modified() {
    let r = Rig::new("symlink");
    let target = r.base.join("other.json");
    fs::write(&target, b"{}").unwrap();
    let path = r.settings("claude");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&target, &path).unwrap();
    assert!(
        !r.run(&["uninstall", "--auto", "--host", "claude"])
            .status
            .success()
    );
    assert_eq!(fs::read(&target).unwrap(), b"{}");
}
#[test]
fn selecting_one_host_does_not_modify_another() {
    let r = Rig::new("selection");
    let old = r.write(
        "claude",
        &json!({"hooks":{"PreToolUse":[entry(r.old_command(Some("claude")))]}}),
    );
    r.json(&["uninstall", "--auto", "--host", "codex"]);
    assert_eq!(fs::read(r.settings("claude")).unwrap(), old);
}
#[test]
fn automatic_shell_wrapper_is_retired_without_executing_or_reading_registry() {
    let r = Rig::new("run-retired");
    let out = r.run(&["run", "--auto", "--", "printf bad > executed.txt"]);
    assert!(!out.status.success());
    assert!(!r.base.join("executed.txt").exists());
    assert!(!r.home.join(".gil-host-guard").exists());
}
#[test]
fn ordinary_shell_work_uses_the_host_input_unchanged_after_hook_retirement() {
    let r = Rig::new("normal-shell");
    let command = "echo ordinary > ordinary.txt";
    assert_eq!(r.hook(&["hook","--auto","--host","claude"],&json!({"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":command}}).to_string()),json!({}));
    #[cfg(unix)]
    let out = Command::new("/bin/sh")
        .args(["-c", command])
        .current_dir(&r.base)
        .output()
        .unwrap();
    #[cfg(windows)]
    let out = Command::new("cmd.exe")
        .args(["/C", command])
        .current_dir(&r.base)
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(
        fs::read_to_string(r.base.join("ordinary.txt"))
            .unwrap()
            .contains("ordinary")
    );
}

#[test]
fn malformed_hook_rows_are_reported_without_claiming_absence_or_overwriting() {
    let r = Rig::new("invalid-row");
    for row in [
        json!({"hooks": "invalid"}),
        json!({"matcher":"*"}),
        json!({"hooks":[null]}),
    ] {
        let original = r.write("claude", &json!({"hooks":{"PreToolUse":[row]}}));
        assert!(
            !r.run(&["uninstall", "--auto", "--host", "claude"])
                .status
                .success()
        );
        assert_eq!(fs::read(r.settings("claude")).unwrap(), original);
        assert_eq!(
            r.json(&["status", "--auto"])["installed_hooks"]["claude"]["status"],
            "unreadable_or_invalid_settings"
        );
    }
    assert!(!r.home.join(".gil-host-guard").exists());
}
