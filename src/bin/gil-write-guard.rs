//! Opt-in local Host adapter. Does not change GIL storage or install Host settings.
use gil::RuleSet;
use serde_json::{Value, json};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitCode};

const LIMIT: u64 = 1024 * 1024;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = dispatch(&args);
    match result {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            // A malformed hook must deny, not exit 1 (Hosts may fail open on hook errors).
            if args.first().is_some_and(|s| s == "hook") {
                println!("{}", deny(&error));
                ExitCode::SUCCESS
            } else {
                eprintln!("GIL write guard: {error}");
                ExitCode::FAILURE
            }
        }
    }
}

fn dispatch(args: &[String]) -> Result<u8, String> {
    if args.len() < 3 || args[1] != "--root" {
        return Err(
            "usage: gil-write-guard hook|config|run --root ABSOLUTE_PROJECT [-- SHELL_COMMAND]"
                .into(),
        );
    }
    let root = project_root(Path::new(&args[2]))?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    match args[0].as_str() {
        "config" if args.len() == 3 => {
            let command = shell_join(&[
                exe.to_str().ok_or("non-UTF8 executable")?,
                "hook",
                "--root",
                root.to_str().ok_or("non-UTF8 root")?,
            ]);
            println!(
                "{}",
                json!({"hooks":{"PreToolUse":[{"matcher":"^(Bash|exec_command|apply_patch|Write|Edit|MultiEdit|NotebookEdit)$","hooks":[{"type":"command","command":command}]}]}})
            );
            Ok(0)
        }
        "hook" if args.len() == 3 => {
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

fn session(root: &Path) -> Result<gil::ReadOnlySession, String> {
    let rules = RuleSet::builtin().map_err(|e| e.to_string())?;
    gil::open_project_root_read_only(rules, root).map_err(|e| e.to_string())
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

// Accept only our canonical single-quoted argv. Never guess whether an arbitrary shell command
// is read-only, nor whitelist a prefix that could be followed by `;`, redirects or substitutions.
fn wrapped(command: &str, exe: &Path, root: &Path) -> bool {
    let Some(exe) = exe.to_str() else {
        return false;
    };
    let Some(root) = root.to_str() else {
        return false;
    };
    let prefix = format!("{} ", shell_join(&[exe, "run", "--root", root, "--"]));
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
    let mut resolved = PathBuf::new();
    for component in path.components() {
        match component {
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
        let command = shell_join(&[
            exe.to_str().ok_or("non-UTF8 executable")?,
            "run",
            "--root",
            root.to_str().ok_or("non-UTF8 root")?,
            "--",
            command,
        ]);
        return Ok(deny(&format!(
            "GIL 프로젝트 파일 보호: 다음 명령으로 다시 실행한다. 프로젝트 밖 작업과 읽기는 허용되며, 프로젝트 쓰기는 실제 열린 Verify에서만 허용된다. 기존 Host 승인은 그대로 적용된다.\n{command}"
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
    if !cfg!(target_os = "macos") {
        return Err("guarded shell currently supports macOS only".into());
    }
    // Hold the normal GIL lock for the entire foreground command: a close cannot race this run.
    let s = session(root)?;
    let denied = if s.can_confirm_artifact() {
        root.join(".gil")
    } else {
        root.to_path_buf()
    };
    let path = denied.to_str().ok_or("non-UTF8 root")?;
    if path.chars().any(char::is_control) {
        return Err("control character in root".into());
    }
    let literal = format!("\"{}\"", path.replace('\\', "\\\\").replace('"', "\\\""));
    let mut profile = format!("(version 1)(allow default)(deny file-write* (subpath {literal}))");
    // Keep the configured boundary in place: renaming a parent must not move its descendants
    // outside the protected pathname while the command is running.
    for ancestor in root.ancestors() {
        let path = ancestor.to_str().ok_or("non-UTF8 ancestor")?;
        let literal = format!("\"{}\"", path.replace('\\', "\\\\").replace('"', "\\\""));
        profile.push_str(&format!("(deny file-write-unlink (literal {literal}))"));
    }
    let status = Command::new("/usr/bin/sandbox-exec")
        .args(["-p", &profile, "/bin/sh", "-c", command])
        .status()
        .map_err(|e| e.to_string())?;
    drop(s);
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
