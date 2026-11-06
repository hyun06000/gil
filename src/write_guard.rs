//! Legacy hook retirement and explicit maintainer diagnostics. No Host hooks are installed.
use crate::RuleSet;
use serde_json::{Value, json};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitCode};

const LIMIT: u64 = 1024 * 1024;

pub fn cli(args: &[String]) -> ExitCode {
    // Compatibility with old settings, including malformed events/argv: never consult HOME,
    // a registry, cwd or project state, and never deny or rewrite a Host call.
    if args.first().is_some_and(|s| s == "hook") {
        let _ = io::copy(&mut io::stdin().take(LIMIT + 1), &mut io::sink());
        println!("{{}}");
        return ExitCode::SUCCESS;
    }
    if args.first().is_some_and(|s| s == "config") {
        println!("{}", json!({"hooks":{}}));
        return ExitCode::SUCCESS;
    }
    let result = dispatch(args);
    match result {
        Ok(code) => ExitCode::from(code),
        Err(error) if args.first().is_some_and(|s| s == "inspect") => {
            println!("{}", deny(&error));
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("GIL write guard: {error}");
            ExitCode::FAILURE
        }
    }
}

pub(crate) mod automatic;

fn dispatch(args: &[String]) -> Result<u8, String> {
    if args.get(1).is_some_and(|s| s == "--auto") {
        return automatic::dispatch(args);
    }
    if args.len() < 3 || args[1] != "--root" {
        return Err(
            "usage: gil-write-guard inspect|run --root ABSOLUTE_PROJECT [-- SHELL_COMMAND]"
                .into(),
        );
    }
    let root = project_root(Path::new(&args[2]))?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    match args[0].as_str() {
        "inspect" if args.len() == 3 => {
            let mut bytes = Vec::new();
            io::stdin()
                .take(LIMIT + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() as u64 > LIMIT {
                return Err("hook input too large".into());
            }
            let input: Value = serde_json::from_slice(&bytes).map_err(|_| "invalid hook JSON")?;
            println!("{}", check(&root, &exe, &input)?);
            Ok(0)
        }
        "run" if args.len() == 5 && args[3] == "--" => run(&root, &args[4]),
        _ => Err("invalid guard arguments".into()),
    }
}

fn project_root(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err("project root must be absolute".into());
    }
    let root = path.canonicalize().map_err(|e| e.to_string())?;
    if !root.is_dir() || root.parent().is_none() {
        return Err("select a project directory".into());
    }
    Ok(root)
}

fn session(root: &Path) -> Result<crate::ReadOnlySession, String> {
    let rules = RuleSet::builtin().map_err(|e| e.to_string())?;
    crate::open_project_root_read_only(rules, root).map_err(|e| e.to_string())
}

fn deny(reason: &str) -> Value {
    json!({"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":reason}})
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
fn shell_join(args: &[&str]) -> String {
    args.iter()
        .map(|s| shell_quote(s))
        .collect::<Vec<_>>()
        .join(" ")
}

fn invocation(exe: &Path, args: &[&str]) -> Result<String, String> {
    let mut words = vec![exe.to_str().ok_or("non-UTF8 executable")?];
    if exe.file_stem().is_some_and(|s| s == "gil") {
        words.push("write-guard");
    }
    words.extend_from_slice(args);
    Ok(shell_join(&words))
}

// Accept only our canonical single-quoted argv. Never guess whether an arbitrary shell command
// is read-only, nor whitelist a prefix that could be followed by `;`, redirects or substitutions.
fn wrapped(command: &str, exe: &Path, root: &Path) -> bool {
    let Some(exe) = exe.to_str() else {
        return false;
    };
    let Some(root) = root.to_str() else {
        return false;
    };
    let Ok(prefix) = invocation(Path::new(exe), &["run", "--root", root, "--"]) else {
        return false;
    };
    let prefix = format!("{prefix} ");
    let Some(arg) = command.strip_prefix(&prefix) else {
        return false;
    };
    let Some(body) = arg.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) else {
        return false;
    };
    let decoded = body.replace("'\\''", "'");
    !decoded.contains('\0')
        && shell_quote(&decoded) == arg
        && !body.replace("'\\''", "").contains('\'')
}

fn target(cwd: &Path, name: &str) -> Result<PathBuf, String> {
    if name.is_empty() || name.contains('\0') {
        return Err("invalid file path".into());
    }
    let path = if Path::new(name).is_absolute() {
        PathBuf::from(name)
    } else {
        cwd.join(name)
    };
    if !path.is_absolute() {
        return Err("target path must resolve from an absolute cwd".into());
    }
    let mut resolved = PathBuf::new();
    for component in path.components() {
        match component {
            // A Windows drive prefix (including \\?\C:) is not a filesystem path
            // until its root separator has been appended. Looking it up alone can
            // return ERROR_INVALID_FUNCTION, or resolve against a drive-local cwd.
            Component::Prefix(_) => resolved.push(component.as_os_str()),
            Component::ParentDir => {
                resolved.pop();
            }
            Component::CurDir => {}
            other => {
                resolved.push(other.as_os_str());
                match std::fs::symlink_metadata(&resolved) {
                    Ok(_) => resolved = resolved.canonicalize().map_err(|e| e.to_string())?,
                    Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e.to_string()),
                }
            }
        }
    }
    Ok(resolved)
}

fn check(root: &Path, exe: &Path, input: &Value) -> Result<Value, String> {
    if input["hook_event_name"] != "PreToolUse" {
        return Err("expected PreToolUse".into());
    }
    let name = input["tool_name"].as_str().ok_or("missing tool_name")?;
    let args = &input["tool_input"];
    if matches!(name, "Bash" | "exec_command") {
        let command = args["command"]
            .as_str()
            .or_else(|| args["cmd"].as_str())
            .ok_or("missing shell command")?;
        if args["run_in_background"] == true {
            return Ok(deny(
                "GIL guarded commands must finish before Verify closes; background execution is unsupported.",
            ));
        }
        if wrapped(command, exe, root) {
            return Ok(json!({}));
        }
        let command = invocation(
            exe,
            &[
                "run",
                "--root",
                root.to_str().ok_or("non-UTF8 root")?,
                "--",
                command,
            ],
        )?;
        let host_note = if name == "exec_command" {
            " macOS의 제한된 Host sandbox 안에는 보호 sandbox를 중첩할 수 없다. Codex에서는 이 래퍼 명령만 sandbox_permissions=require_escalated와 구체적인 justification으로 정상 Host 승인을 요청한다. 승인이 거절되면 멈춘다. 전역 sandbox 설정을 바꾸거나 원래 명령만 재실행하지 않는다."
        } else {
            ""
        };
        return Ok(deny(&format!(
            "GIL 프로젝트 파일 보호: 다음 명령으로 다시 실행한다. 프로젝트 밖 작업과 읽기는 허용되며, 프로젝트 쓰기는 실제 열린 Verify에서만 허용된다. 기존 Host 승인은 그대로 적용된다.{host_note}\n{command}"
        )));
    }
    let fields: &[&str] = match name {
        "Write" | "Edit" | "MultiEdit" => &["file_path"],
        "NotebookEdit" => &["notebook_path"],
        "apply_patch" => &[],
        _ => return Ok(json!({})), // No claim about unregistered Host tools.
    };
    let cwd = Path::new(input["cwd"].as_str().ok_or("missing cwd")?);
    if !cwd.is_absolute() {
        return Err("cwd must be absolute".into());
    }
    let mut paths = Vec::new();
    if name == "apply_patch" {
        let patch = args
            .as_str()
            .or_else(|| args["command"].as_str())
            .or_else(|| args["input"].as_str())
            .ok_or("missing patch")?;
        if !patch.starts_with("*** Begin Patch\n") || !patch.trim_end().ends_with("*** End Patch") {
            return Err("unrecognized patch format".into());
        }
        for line in patch.lines() {
            for prefix in [
                "*** Add File: ",
                "*** Update File: ",
                "*** Delete File: ",
                "*** Move to: ",
            ] {
                if let Some(path) = line.strip_prefix(prefix) {
                    if path != path.trim() {
                        return Err("ambiguous whitespace in patch path".into());
                    }
                    paths.push(target(cwd, path)?);
                }
            }
        }
        if paths.is_empty() {
            return Err("patch has no recognized paths".into());
        }
    } else {
        for field in fields {
            paths.push(target(
                cwd,
                args[field].as_str().ok_or("missing target path")?,
            )?);
        }
    }
    // Also refuse an edit to an ancestor which could replace/remove the project directory.
    if paths
        .iter()
        .any(|p| p.starts_with(root.join(".gil")) || root.starts_with(p) && p != root)
    {
        return Ok(deny(
            "GIL 기록과 프로젝트 상위 경로는 직접 편집하지 않는다. GIL 기록·승인·복구는 기존 GIL 도구를 사용한다.",
        ));
    }
    if !paths.iter().any(|p| p.starts_with(root)) {
        return Ok(json!({}));
    }
    let s = session(root)?;
    if s.can_confirm_artifact() {
        Ok(json!({}))
    } else {
        Ok(deny(
            "프로젝트 파일 변경은 실제로 열린 Experiment Verify에서만 허용된다. gil_context의 현재 다음 호출을 따라 Verify를 연다. Closing Interview·승인 대기에서는 파일을 만들거나 가져오지 않는다.",
        ))
    }
}

fn run(root: &Path, command: &str) -> Result<u8, String> {
    run_roots(&[root.to_path_buf()], command, &[], "legacy")
}

fn run_roots(roots: &[PathBuf], command: &str, extra_readonly: &[PathBuf], host: &str) -> Result<u8, String> {
    if !cfg!(target_os = "macos") {
        return Err("guarded shell currently supports macOS only".into());
    }
    // Hold the normal GIL lock for the entire writable foreground command. If state cannot
    // be read/locked, deny the entire project instead: external work needs no GIL permission.
    // This is a stricter sandbox, never a fallback to an unguarded command.
    let sessions: Vec<_> = roots.iter().map(|root| session(root).ok()).collect();
    let mut profile = "(version 1)(allow default)".to_owned();
    let literal = |path: &Path| -> Result<String, String> {
        let text = path.to_str().ok_or("non-UTF8 root")?;
        if text.chars().any(char::is_control) {
            return Err("control character in root".into());
        }
        Ok(format!(
            "\"{}\"",
            text.replace('\\', "\\\\").replace('"', "\\\"")
        ))
    };
    for (root, s) in roots.iter().zip(&sessions) {
        let denied = if s.as_ref().is_some_and(|s| s.can_confirm_artifact()) {
            root.join(".gil")
        } else {
            root.clone()
        };
        profile.push_str(&format!(
            "(deny file-write* (subpath {}))",
            literal(&denied)?
        ));
        for ancestor in root.ancestors() {
            profile.push_str(&format!(
                "(deny file-write-unlink (literal {}))",
                literal(ancestor)?
            ));
        }
    }
    for path in extra_readonly {
        profile.push_str(&format!("(deny file-write* (subpath {}))", literal(path)?));
    }
    // Establish that this process can install the protection before starting any user command.
    // macOS rejects tightening an already restrictive Host sandbox. Do not run an unprotected
    // fallback, change Host permissions, or mistake a payload's exit 71 for sandbox failure.
    let probe = Command::new("/usr/bin/sandbox-exec")
        .args(["-p", &profile, "/usr/bin/true"])
        .output()
        .map_err(|e| e.to_string())?;
    if !probe.status.success() {
        let guidance = match host {
            "codex" => "Codex: 동일한 래퍼 전체에 sandbox_permissions=require_escalated와 justification으로 정상 Host 승인을 요청한다.",
            "claude" => "Claude Code: 이 Host가 제공하는 래퍼 단일 명령의 sandbox 밖 실행 승인 경로만 사용할 수 있다. 지원하지 않거나 정책으로 금지됐으면 이 명령은 실행할 수 없다.",
            _ => "현재 Host의 단일 명령 승인 기능을 확인한다. 다른 Host의 승인 필드를 추측하지 않는다.",
        };
        return Err(format!(
            "[guard_sandbox_unavailable] 보호 sandbox를 적용하지 못해 요청한 명령을 실행하지 않았다. {guidance} 승인 거절 시 중단한다. 전역 sandbox 설정 변경이나 보호 없는 재실행은 하지 않는다. {}",
            String::from_utf8_lossy(&probe.stderr).trim()
        ));
    }
    let status = Command::new("/usr/bin/sandbox-exec")
        .args(["-p", &profile, "/bin/sh", "-c", command])
        .status()
        .map_err(|e| e.to_string())?;
    drop(sessions);
    Ok(status
        .code()
        .and_then(|c| u8::try_from(c).ok())
        .unwrap_or(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wrapper_is_exact_argv_not_a_shell_prefix_allowlist() {
        let exe = Path::new("/tmp/gil guard");
        let root = Path::new("/tmp/project");
        for payload in [
            "printf hello",
            "python3 -c 'print(1)'",
            "echo $(whoami)",
            "printf 'a\nb'",
        ] {
            let cmd = shell_join(&[
                exe.to_str().unwrap(),
                "run",
                "--root",
                root.to_str().unwrap(),
                "--",
                payload,
            ]);
            assert!(wrapped(&cmd, exe, root));
            for suffix in ["; touch bad", " > bad", " && echo bad", " $(echo bad)"] {
                assert!(!wrapped(&(cmd.clone() + suffix), exe, root));
            }
            assert!(!wrapped(&cmd, exe, Path::new("/tmp/other")));
        }
        assert!(!wrapped(
            "'/tmp/gil guard' 'run' '--root' '/tmp/project' '--' 'a';'b'",
            exe,
            root
        ));
    }
}
