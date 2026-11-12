//! `gil install` · `gil doctor` · `gil uninstall` — the executable registers itself with the
//! Hosts on this PC (Claude Desktop, Codex) and reports the result in a fixed, machine-readable
//! form. This is the whole Windows installation story: one static `gil.exe`, one fixed path,
//! no installer, no launcher, no Companion.
//!
//! Decisions (2026-11-12 deployment decision record):
//! - the fixed path is `%LOCALAPPDATA%\Programs\gil\gil.exe`; Hosts register that path with
//!   `mcp --serve` (the existing MCP contract; `serve` is an alias);
//! - Claude Desktop's `claude_desktop_config.json` is written in **both** locations the Windows
//!   app may read (its MSIX package cache and `%APPDATA%`); Codex's `config.toml` is edited in
//!   place with its formatting preserved. Nothing else in either file changes;
//! - every write is backed up beside the file, parsed before it is touched, and replaced
//!   through a temporary file and a rename. A file that cannot be parsed is reported, never
//!   overwritten;
//! - `install` is idempotent: an entry that already matches leaves the file alone;
//! - no administrator rights, no registry, no security settings.
//!
//! A running Claude Desktop rewrites its own settings file and can revert an outside edit
//! (observed on macOS, 2026-10-31). `install` therefore refuses to write that file while the
//! app runs; by default it leaves a detached helper (`--after-desktop-exit`) that waits for the
//! app to quit, writes, and logs the result next to the executable.

mod claude;
mod codex;
mod doctor;
mod file;
mod paths;

pub use doctor::Status;
pub use paths::{Env, packaged_now};

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub const SERVER_NAME: &str = "gil";
pub const SERVE_ARGS: [&str; 2] = ["mcp", "--serve"];
const INSTALL_RECEIPT: &str = "install.json";
const LOG_FILE: &str = "install-log.txt";
const WAIT_FOR_DESKTOP: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// How a `gil` entry in a Host file relates to this installation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Registration {
    /// No `gil` entry.
    Missing,
    /// Exactly our entry: the fixed path, `mcp --serve`, nothing else.
    Ours,
    /// A `gil` entry that runs some GIL executable (an older launcher, a version folder, a
    /// Plugin cache) or ours with different arguments. `install` updates it.
    Stale,
    /// A `gil` entry that runs something that is not GIL. Left alone unless `--replace`.
    Foreign,
}

/// A GIL executable by name, whichever path separator the writing Host used.
pub(crate) fn is_gil_executable(command: &str) -> bool {
    let name = command
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(name.as_str(), "gil" | "gil.exe" | "gil-launcher" | "gil-launcher.exe")
}

pub(crate) fn same_path(a: &str, b: &str) -> bool {
    if cfg!(windows) { a.eq_ignore_ascii_case(b) } else { a == b }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct InstallOptions {
    /// Replace a `gil` entry even when it does not run a GIL executable.
    pub replace: bool,
    /// Do not leave a helper waiting for Claude Desktop to quit; only report.
    pub no_wait: bool,
    /// Internal: this process *is* the helper. Wait for Claude Desktop to quit, then write.
    pub after_desktop_exit: bool,
}

/// CLI entry: prints its own output and returns the exit code. Called from `main` before the
/// project-bound commands, because none of these needs a project root.
pub fn cli(args: &[String]) -> ExitCode {
    let (command, rest) = match args.split_first() {
        Some((command, rest)) => (command.as_str(), rest),
        None => return usage(),
    };
    let env = Env::from_process();
    let outcome = match command {
        "install" => {
            let mut options = InstallOptions::default();
            for flag in rest {
                match flag.as_str() {
                    "--replace" => options.replace = true,
                    "--no-wait" => options.no_wait = true,
                    "--after-desktop-exit" => options.after_desktop_exit = true,
                    other => return refuse(&format!("unknown install option {other:?}")),
                }
            }
            install(&env, options)
        }
        "doctor" => {
            if !rest.is_empty() { return refuse("doctor takes no arguments"); }
            doctor(&env)
        }
        "uninstall" => {
            let mut remove_binary = false;
            for flag in rest {
                match flag.as_str() {
                    "--remove-binary" => remove_binary = true,
                    other => return refuse(&format!("unknown uninstall option {other:?}")),
                }
            }
            uninstall(&env, remove_binary)
        }
        _ => return usage(),
    };
    match outcome {
        Ok((text, code)) => { print!("{text}"); code }
        Err(message) => { eprintln!("gil {command}: {message}"); ExitCode::FAILURE }
    }
}

fn usage() -> ExitCode {
    print!("usage: gil install [--replace] [--no-wait]\n       gil doctor\n       gil uninstall [--remove-binary]\n");
    ExitCode::FAILURE
}

fn refuse(message: &str) -> ExitCode {
    eprintln!("gil: {message}");
    ExitCode::FAILURE
}

type Outcome = Result<(String, ExitCode), String>;

fn install(env: &Env, options: InstallOptions) -> Outcome {
    let fixed = paths::fixed_exe(env)?;
    if options.after_desktop_exit {
        return wait_then_register_claude(env, &fixed, options);
    }
    let current = std::env::current_exe().map_err(|err| format!("cannot find my own path: {err}"))?;
    let mut lines = if env.packaged {
        // Store channel: nothing to copy. The package folder is read-only and the alias is the
        // registered path; a missing alias means the package registration itself is broken.
        if !fixed.exists() {
            return Err(format!("the app execution alias is missing at {}; reinstall the GIL app from the Microsoft Store", fixed.display()));
        }
        vec![format!("binary: OK {} (v{}) [store]", fixed.display(), crate::CORE_VERSION)]
    } else {
        let placed = place_binary(&current, &fixed)?;
        vec![format!("binary: {placed} {} (v{})", fixed.display(), crate::CORE_VERSION)]
    };

    lines.push(format!("codex: {}", codex_step(env, &fixed, options)?));
    let (claude_line, claude_waits) = claude_step(env, &fixed, &current, options)?;
    lines.push(format!("claude-desktop: {claude_line}"));

    lines.push(String::new());
    if claude_waits {
        lines.push("Quit Claude Desktop completely (tray icon → Quit). A helper is waiting; it writes the Claude Desktop entry as soon as the app has quit and records the result in install-log.txt next to gil.exe. Then open Claude Desktop again.".into());
        lines.push("Claude Desktop을 트레이에서 완전히 종료하세요. 도우미가 종료를 기다렸다가 등록을 마치고 gil.exe 옆 install-log.txt에 결과를 남깁니다. 그 뒤 Claude Desktop을 다시 여세요.".into());
    } else {
        lines.push("Restart the Hosts that were registered (Claude Desktop: quit from the tray icon, then reopen; Codex: restart). Tools appear only after a restart.".into());
        lines.push("등록된 Host를 다시 시작하세요(Claude Desktop은 트레이에서 완전 종료 후 다시 열기, Codex는 재시작). 재시작 전에는 도구가 보이지 않습니다.".into());
    }
    lines.push("Check: gil doctor".into());
    Ok((lines.join("\n") + "\n", ExitCode::SUCCESS))
}

fn doctor(env: &Env) -> Outcome {
    let report = doctor::report(env);
    let code = if report.healthy() { ExitCode::SUCCESS } else { ExitCode::FAILURE };
    Ok((report.render(), code))
}

fn uninstall(env: &Env, remove_binary: bool) -> Outcome {
    let fixed = paths::fixed_exe(env)?;
    let mut lines = Vec::new();
    lines.push(format!("codex: {}", match paths::codex_config(env) {
        None => "NOT_INSTALLED".to_owned(),
        Some(path) => codex::unregister(&path)?,
    }));
    let claude_paths = paths::claude_configs(env);
    if claude_paths.is_empty() {
        lines.push("claude-desktop: NOT_INSTALLED".into());
    } else {
        claude::require_stopped(claude::desktop_running())?;
        for path in &claude_paths {
            lines.push(format!("claude-desktop: {} ({})", claude::unregister(path)?, path.display()));
        }
    }
    if remove_binary && env.packaged {
        lines.push("binary: KEPT (Microsoft Store app; remove it from Settings → Apps)".into());
    } else if remove_binary {
        let dir = fixed.parent().ok_or("fixed path has no parent")?;
        let current = std::env::current_exe().ok();
        if current.as_deref().is_some_and(|me| same_file(me, &fixed)) {
            lines.push(format!("binary: KEPT {} (cannot delete the running executable; delete the folder after this process ends)", fixed.display()));
        } else {
            std::fs::remove_file(&fixed).map_err(|err| format!("cannot remove {}: {err}", fixed.display()))?;
            let _ = std::fs::remove_file(dir.join(INSTALL_RECEIPT));
            lines.push(format!("binary: REMOVED {}", fixed.display()));
        }
    }
    lines.push("Restart the Hosts for the change to take effect.".into());
    Ok((lines.join("\n") + "\n", ExitCode::SUCCESS))
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => same_path(&a.to_string_lossy(), &b.to_string_lossy()),
    }
}

/// Put this executable at the fixed path. A running `gil.exe` at that path cannot be
/// overwritten or deleted on Windows, but it can be renamed: the old file keeps serving the
/// connections that already run it, and new connections start the new file.
fn place_binary(current: &Path, fixed: &Path) -> Result<&'static str, String> {
    let dir = fixed.parent().ok_or("fixed path has no parent")?;
    std::fs::create_dir_all(dir).map_err(|err| format!("cannot create {}: {err}", dir.display()))?;
    let my_bytes = std::fs::read(current).map_err(|err| format!("cannot read my own file: {err}"))?;
    let my_digest = hex(&Sha256::digest(&my_bytes));
    let outcome = if same_file(current, fixed) {
        "OK"
    } else if std::fs::read(fixed).ok().is_some_and(|bytes| hex(&Sha256::digest(&bytes)) == my_digest) {
        "OK"
    } else {
        if fixed.exists() {
            let previous = receipt_version(dir).unwrap_or_else(|| format!("{}", now_unix()));
            let parked = dir.join(format!("gil.exe.old-{previous}"));
            let _ = std::fs::remove_file(&parked);
            std::fs::rename(fixed, &parked)
                .map_err(|err| format!("cannot move the previous gil.exe aside: {err}"))?;
        }
        std::fs::copy(current, fixed).map_err(|err| format!("cannot copy to {}: {err}", fixed.display()))?;
        prune_parked(dir);
        "UPDATED"
    };
    write_receipt(dir, &my_digest)?;
    Ok(outcome)
}

fn prune_parked(dir: &Path) {
    for entry in std::fs::read_dir(dir).into_iter().flatten().filter_map(Result::ok) {
        let name = entry.file_name();
        if name.to_string_lossy().starts_with("gil.exe.old-") {
            // Still in use by a running connection → the delete fails and the file stays.
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

fn write_receipt(dir: &Path, sha256: &str) -> Result<(), String> {
    let receipt = serde_json::json!({
        "schema": 1,
        "version": crate::CORE_VERSION,
        "sha256": sha256,
        "installed_at": now_unix(),
    });
    let text = serde_json::to_string_pretty(&receipt).map_err(|err| err.to_string())? + "\n";
    let temporary = dir.join(format!("{INSTALL_RECEIPT}.next"));
    std::fs::write(&temporary, text).map_err(|err| format!("cannot write {INSTALL_RECEIPT}: {err}"))?;
    std::fs::rename(&temporary, dir.join(INSTALL_RECEIPT)).map_err(|err| format!("cannot replace {INSTALL_RECEIPT}: {err}"))
}

/// The version recorded beside an installed executable, if the receipt is sane.
pub fn receipt_version(dir: &Path) -> Option<String> {
    let path = dir.join(INSTALL_RECEIPT);
    let metadata = std::fs::metadata(&path).ok()?;
    if !metadata.is_file() || metadata.len() > 4096 { return None; }
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    if value.get("schema")?.as_u64()? != 1 { return None; }
    let version = value.get("version")?.as_str()?;
    (!version.is_empty() && version.len() <= 80 && version.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'+'))
        .then(|| version.to_owned())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn now_unix() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn codex_step(env: &Env, fixed: &Path, options: InstallOptions) -> Result<String, String> {
    match paths::codex_config(env) {
        None => Ok("NOT_INSTALLED".into()),
        Some(path) => codex::register(&path, fixed, options.replace).map(|outcome| format!("{outcome} ({})", path.display())),
    }
}

/// Returns the report line and whether a helper now waits for Claude Desktop to quit.
fn claude_step(env: &Env, fixed: &Path, current: &Path, options: InstallOptions) -> Result<(String, bool), String> {
    let paths = paths::claude_configs(env);
    if paths.is_empty() {
        return Ok(("NOT_INSTALLED".into(), false));
    }
    // Nothing to do when every target already carries our entry: no write, no wait.
    let all_ours = paths.iter().all(|path| {
        claude::load(path).is_ok_and(|loaded| claude::classify(&loaded.root, fixed) == Registration::Ours)
    });
    if all_ours {
        return Ok((format!("OK ({})", join_paths(&paths)), false));
    }
    match claude::desktop_running() {
        Some(false) => {
            let mut parts = Vec::new();
            for path in &paths {
                parts.push(format!("{} ({})", claude::register(path, fixed, options.replace)?, path.display()));
            }
            Ok((parts.join("; "), false))
        }
        Some(true) if options.no_wait => Ok((format!("PENDING Claude Desktop is running; quit it completely, then run `gil install` again ({})", join_paths(&paths)), false)),
        Some(true) => {
            spawn_waiting_helper(fixed, current, options)?;
            Ok((format!("PENDING helper waits for Claude Desktop to quit ({})", join_paths(&paths)), true))
        }
        None => Err("could not tell whether Claude Desktop is running; quit it, then run `gil install` again".into()),
    }
}

fn join_paths(paths: &[PathBuf]) -> String {
    paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join("; ")
}

fn spawn_waiting_helper(fixed: &Path, current: &Path, options: InstallOptions) -> Result<(), String> {
    // Run the helper from the fixed path so the downloaded copy may be deleted right away.
    let exe = if fixed.is_file() { fixed } else { current };
    let mut command = std::process::Command::new(exe);
    command.arg("install").arg("--after-desktop-exit");
    if options.replace { command.arg("--replace"); }
    command.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    }
    command.spawn().map(|_| ()).map_err(|err| format!("cannot start the waiting helper: {err}"))
}

fn wait_then_register_claude(env: &Env, fixed: &Path, options: InstallOptions) -> Outcome {
    let dir = fixed.parent().ok_or("fixed path has no parent")?;
    let log = dir.join(LOG_FILE);
    let started = std::time::Instant::now();
    let mut running = claude::desktop_running();
    while running == Some(true) && started.elapsed() < WAIT_FOR_DESKTOP {
        std::thread::sleep(std::time::Duration::from_secs(2));
        running = claude::desktop_running();
    }
    let mut lines = vec![format!("[{}] helper", now_unix())];
    match running {
        Some(false) => {
            for path in paths::claude_configs(env) {
                match claude::register(&path, fixed, options.replace) {
                    Ok(outcome) => lines.push(format!("claude-desktop: {outcome} ({})", path.display())),
                    Err(err) => lines.push(format!("claude-desktop: ERROR {err} ({})", path.display())),
                }
            }
        }
        Some(true) => lines.push("claude-desktop: PENDING Claude Desktop did not quit within 10 minutes; run `gil install` again after quitting it".into()),
        None => lines.push("claude-desktop: ERROR could not tell whether Claude Desktop is running".into()),
    }
    let text = lines.join("\n") + "\n";
    append_log(&log, &text);
    Ok((text, ExitCode::SUCCESS))
}

fn append_log(path: &Path, text: &str) {
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(text.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "gil-install-{tag}-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn gil_executables_are_recognised_by_name_only() {
        assert!(is_gil_executable(r"C:\Users\Me\AppData\Local\GIL\runtime\gil-launcher.exe"));
        assert!(is_gil_executable("/Users/me/plugins/core/darwin-arm64/gil"));
        assert!(is_gil_executable(r"C:\x\GIL.EXE"));
        assert!(!is_gil_executable(r"C:\x\node.exe"));
        assert!(!is_gil_executable(""));
    }

    #[test]
    fn placing_the_binary_parks_a_different_previous_file_and_keeps_an_identical_one() {
        let dir = scratch("place");
        let fixed = dir.join("gil.exe");
        let download = dir.join("gil-new.exe");
        std::fs::write(&download, b"new bytes").unwrap();
        assert_eq!(place_binary(&download, &fixed).unwrap(), "UPDATED");
        assert_eq!(std::fs::read(&fixed).unwrap(), b"new bytes");
        assert_eq!(receipt_version(&dir).as_deref(), Some(crate::CORE_VERSION));
        // Same bytes again: nothing moves.
        assert_eq!(place_binary(&download, &fixed).unwrap(), "OK");
        assert!(!dir.join(format!("gil.exe.old-{}", crate::CORE_VERSION)).exists());
        // Different bytes: the previous file is parked under the recorded version, then pruned
        // when nothing holds it open.
        std::fs::write(&download, b"newer bytes").unwrap();
        assert_eq!(place_binary(&download, &fixed).unwrap(), "UPDATED");
        assert_eq!(std::fs::read(&fixed).unwrap(), b"newer bytes");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn receipt_rejects_odd_shapes() {
        let dir = scratch("receipt");
        std::fs::write(dir.join(INSTALL_RECEIPT), r#"{"schema":2,"version":"1.0.0"}"#).unwrap();
        assert_eq!(receipt_version(&dir), None);
        std::fs::write(dir.join(INSTALL_RECEIPT), r#"{"schema":1,"version":"1.0.0-rc.1"}"#).unwrap();
        assert_eq!(receipt_version(&dir).as_deref(), Some("1.0.0-rc.1"));
        std::fs::write(dir.join(INSTALL_RECEIPT), r#"{"schema":1,"version":"../x"}"#).unwrap();
        assert_eq!(receipt_version(&dir), None);
        let _ = std::fs::remove_dir_all(dir);
    }
}
