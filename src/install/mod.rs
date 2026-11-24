//! `gil install` · `gil doctor` · `gil uninstall` — the executable registers itself with the
//! Hosts on this PC (Claude Desktop, Codex) and reports the result in a fixed, machine-readable
//! form. Standalone installs copy the Core to a fixed path; Store installs register the OS
//! execution alias without copying an executable out of the package.
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
//! app to quit, writes, and logs the result in the installation's writable log location.

mod claude;
mod codex;
mod doctor;
mod file;
mod paths;
mod probe;
#[cfg(unix)]
mod user_place;

pub use doctor::Status;
pub use paths::{Env, packaged_now};

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub const SERVER_NAME: &str = "gil";
pub const SERVE_ARGS: [&str; 2] = ["mcp", "--serve"];
const INSTALL_RECEIPT: &str = "install.json";
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
            let verbose = match rest {
                [] => false,
                [flag] if flag == "--verbose" => true,
                _ => return refuse("usage: gil doctor [--verbose]"),
            };
            doctor(&env, verbose)
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
    print!("usage: gil install [--replace] [--no-wait]\n       gil doctor [--verbose]\n       gil uninstall [--remove-binary]\n");
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
        probe::check_alias(&fixed)?;
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
        let log = paths::install_log(env)?;
        lines.push(format!("Quit Claude Desktop completely (tray icon → Quit). A helper is waiting. Check {} for the registration result, then run `gil doctor` before reopening Claude Desktop.", log.display()));
        lines.push(format!("Claude Desktop을 트레이에서 완전히 종료하세요. 도우미가 등록한 결과는 {}에서 확인하고, `gil doctor`로 확인한 뒤 Claude Desktop을 다시 여세요.", log.display()));
    } else {
        lines.push("Restart the Hosts that were registered (Claude Desktop: quit from the tray icon, then reopen; Codex: restart). Tools appear only after a restart.".into());
        lines.push("등록된 Host를 다시 시작하세요(Claude Desktop은 트레이에서 완전 종료 후 다시 열기, Codex는 재시작). 재시작 전에는 도구가 보이지 않습니다.".into());
    }
    if cfg!(target_os = "linux") {
        // Only the Hosts listed above are registered. Nothing else on this PC is edited or assumed.
        lines.push(format!("Other MCP Hosts: gil install does not edit their settings. A Host that runs a local stdio MCP server can use the command {} with the arguments mcp --serve.", fixed.display()));
        lines.push(format!("다른 MCP Host의 설정은 gil install이 고치지 않습니다. 로컬 stdio MCP 서버를 실행하는 Host에는 명령 {} 와 인자 mcp --serve 를 직접 등록하세요.", fixed.display()));
    }
    lines.push("Check: gil doctor".into());
    lines.push("NOT_INSTALLED means no Host settings folder was found; PENDING is not completed registration. / NOT_INSTALLED는 Host 설정 폴더를 찾지 못했다는 뜻이며 PENDING은 등록 완료가 아닙니다.".into());
    Ok((lines.join("\n") + "\n", ExitCode::SUCCESS))
}

fn doctor(env: &Env, verbose: bool) -> Outcome {
    let report = doctor::report(env);
    let code = if report.healthy() { ExitCode::SUCCESS } else { ExitCode::FAILURE };
    if verbose {
        // Preserve the three machine-readable stdout lines, including for verbose callers.
        eprint!("{}", doctor::details(env));
    }
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
        #[cfg(unix)]
        if cfg!(target_os = "linux") {
            // Linux user install: only the file `install.json` records is removed, and a running
            // executable can remove its own file there.
            let removal = user_place::remove(&fixed)?;
            let complete = removal.complete() || !removal.found || removal.kept;
            lines.push(if !removal.found {
                format!("binary: NOT_INSTALLED {} (nothing recorded to remove)", fixed.display())
            } else if removal.kept {
                format!("binary: KEPT {} (not the file recorded by install.json; nothing was removed)", fixed.display())
            } else if removal.complete() {
                format!("binary: REMOVED {}", removal.removed.join(", "))
            } else {
                // Part of the removal did not happen. Both lists are what was actually done.
                format!("binary: PARTIAL removed [{}]; remaining [{}]", removal.removed.join(", "), removal.remaining.join(", "))
            });
            lines.push("Restart the Hosts for the change to take effect.".into());
            return Ok((lines.join("\n") + "\n", if complete { ExitCode::SUCCESS } else { ExitCode::FAILURE }));
        }
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
    #[cfg(unix)]
    if cfg!(target_os = "linux") {
        // Linux user install has its own placement: the ownership rule is asked before any write,
        // also when this very file or equal bytes are already at the fixed path (`user_place`).
        let my_bytes = std::fs::read(current).map_err(|err| format!("cannot read my own file: {err}"))?;
        let my_digest = hex(&Sha256::digest(&my_bytes));
        return user_place::place(fixed, &my_bytes, &my_digest, &receipt_text(&my_digest)?);
    }
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

fn receipt_text(sha256: &str) -> Result<String, String> {
    let receipt = serde_json::json!({
        "schema": 1,
        "version": crate::CORE_VERSION,
        "sha256": sha256,
        "installed_at": now_unix(),
    });
    Ok(serde_json::to_string_pretty(&receipt).map_err(|err| err.to_string())? + "\n")
}

fn write_receipt(dir: &Path, sha256: &str) -> Result<(), String> {
    let text = receipt_text(sha256)?;
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
            spawn_waiting_helper(env, fixed, current, options)?;
            Ok((format!("PENDING helper waits for Claude Desktop to quit ({})", join_paths(&paths)), true))
        }
        None => Err("could not tell whether Claude Desktop is running; quit it, then run `gil install` again".into()),
    }
}

fn join_paths(paths: &[PathBuf]) -> String {
    paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join("; ")
}

fn spawn_waiting_helper(env: &Env, fixed: &Path, current: &Path, options: InstallOptions) -> Result<(), String> {
    let log = paths::install_log(env)?;
    let mut log_file = open_log(&log)?;
    write_log(&mut log_file, &log, "helper requested; claude-desktop: PENDING\n")?;
    // Run the helper from the fixed path so the downloaded copy may be deleted right away.
    let exe = if fixed.is_file() { fixed } else { current };
    let mut command = std::process::Command::new(exe);
    command.arg("install").arg("--after-desktop-exit");
    if options.replace { command.arg("--replace"); }
    // Early helper errors must remain visible even though it has no console.
    let error_log = log_file.try_clone().map_err(|err| format!("cannot attach helper error log {}: {err}", log.display()))?;
    command.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(error_log);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    }
    command.spawn().map(|_| ()).map_err(|err| {
        let message = format!("cannot start the waiting helper: {err}");
        match write_log(&mut log_file, &log, &format!("claude-desktop: ERROR {message}\n")) {
            Ok(()) => message,
            Err(log_error) => format!("{message}; {log_error}"),
        }
    })
}

fn wait_then_register_claude(env: &Env, fixed: &Path, options: InstallOptions) -> Outcome {
    let log = paths::install_log(env)?;
    let mut log_file = open_log(&log)?;
    write_log(&mut log_file, &log, "helper started; claude-desktop: PENDING\n")?;
    let started = std::time::Instant::now();
    let mut running = claude::desktop_running();
    while running == Some(true) && started.elapsed() < WAIT_FOR_DESKTOP {
        std::thread::sleep(std::time::Duration::from_secs(2));
        running = claude::desktop_running();
    }
    let (text, code) = finish_claude_registration(env, fixed, options, running);
    write_log(&mut log_file, &log, &text).map_err(|why| format!("{text}{why}"))?;
    Ok((text, code))
}

fn finish_claude_registration(env: &Env, fixed: &Path, options: InstallOptions, running: Option<bool>) -> (String, ExitCode) {
    let mut lines = Vec::new();
    let mut code = ExitCode::SUCCESS;
    match running {
        Some(false) => {
            let configs = paths::claude_configs(env);
            if configs.is_empty() { lines.push("claude-desktop: NOT_INSTALLED".into()); }
            for path in configs {
                match claude::register(&path, fixed, options.replace) {
                    Ok(outcome) => lines.push(format!("claude-desktop: {outcome} ({})", path.display())),
                    Err(err) => {
                        code = ExitCode::FAILURE;
                        lines.push(format!("claude-desktop: ERROR {err} ({})", path.display()));
                    }
                }
            }
        }
        Some(true) => {
            code = ExitCode::FAILURE;
            lines.push("claude-desktop: PENDING Claude Desktop did not quit within 10 minutes; run `gil install` again after quitting it".into());
        }
        None => {
            code = ExitCode::FAILURE;
            lines.push("claude-desktop: ERROR could not tell whether Claude Desktop is running".into());
        }
    }
    (lines.join("\n") + "\n", code)
}

fn open_log(path: &Path) -> Result<std::fs::File, String> {
    let dir = path.parent().ok_or("log path has no parent")?;
    std::fs::create_dir_all(dir).map_err(|err| format!("cannot create install log directory {}: {err}", dir.display()))?;
    std::fs::OpenOptions::new().create(true).append(true).open(path)
        .map_err(|err| format!("cannot open install log {}: {err}", path.display()))
}

fn write_log(file: &mut std::fs::File, path: &Path, text: &str) -> Result<(), String> {
    use std::io::Write;
    let entry = format!("[{} pid={}] {text}", now_unix(), std::process::id());
    file.write_all(entry.as_bytes()).and_then(|_| file.flush())
        .map_err(|err| format!("cannot write install log {}: {err}", path.display()))
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

    fn store_env(root: &Path) -> Env {
        Env { local_app_data: Some(root.join("Local")), app_data: Some(root.join("Roaming")), packaged: true, ..Env::default() }
    }

    #[test]
    fn store_helper_log_is_writable_user_data_and_preserves_previous_entries() {
        let root = scratch("store-log");
        let env = store_env(&root);
        let log = paths::install_log(&env).unwrap();
        assert_eq!(log, root.join("Local/GIL/logs/install-log.txt"));
        let mut file = open_log(&log).unwrap();
        write_log(&mut file, &log, "first\n").unwrap();
        drop(file);
        write_log(&mut open_log(&log).unwrap(), &log, "second\n").unwrap();
        let text = std::fs::read_to_string(&log).unwrap();
        assert!(text.contains("first\n") && text.contains("second\n"));
        assert!(!root.join("Local/Microsoft/WindowsApps").exists());
        let details = doctor::details(&env);
        assert!(details.contains(&log.display().to_string()));
        assert!(details.contains("present; historical results"));
        assert!(details.contains("does not verify MCP connection or Monitor display"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn standalone_install_keeps_its_existing_log_location() {
        let root = scratch("standalone-log");
        let env = Env { local_app_data: Some(root.join("Local")), home: Some(root.join("home")), ..Env::default() };
        if let Ok(fixed) = paths::fixed_exe(&env) {
            assert_eq!(paths::install_log(&env).unwrap(), fixed.parent().unwrap().join("install-log.txt"));
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn broken_store_alias_fails_before_any_host_registration() {
        let root = scratch("install-alias");
        let mut env = store_env(&root);
        env.codex_home = Some(root.join("codex"));
        std::fs::create_dir_all(root.join("codex")).unwrap();
        let error = install(&env, InstallOptions { no_wait: true, ..InstallOptions::default() }).unwrap_err();
        assert!(error.contains("app execution alias"));
        assert!(!root.join("codex/config.toml").exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unwritable_log_is_an_error_before_helper_changes_any_settings() {
        let root = scratch("blocked-log");
        let env = store_env(&root);
        std::fs::create_dir_all(root.join("Local")).unwrap();
        std::fs::write(root.join("Local/GIL"), b"not a directory").unwrap();
        let config = root.join("Roaming/Claude/claude_desktop_config.json");
        std::fs::create_dir_all(config.parent().unwrap()).unwrap();
        std::fs::write(&config, b"{}").unwrap();
        let err = wait_then_register_claude(&env, &paths::fixed_exe(&env).unwrap(), InstallOptions::default()).unwrap_err();
        assert!(err.contains("cannot create install log directory"));
        assert_eq!(std::fs::read(config).unwrap(), b"{}");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn log_write_failure_is_not_swallowed() {
        let root = scratch("log-write");
        let path = root.join("log.txt");
        std::fs::write(&path, b"existing").unwrap();
        let mut read_only = std::fs::File::open(&path).unwrap();
        assert!(write_log(&mut read_only, &path, "new\n").unwrap_err().contains("cannot write install log"));
        drop(read_only);
        assert_eq!(std::fs::read(&path).unwrap(), b"existing");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn helper_errors_and_timeouts_fail_without_touching_settings() {
        let root = scratch("helper-status");
        let env = store_env(&root);
        let fixed = paths::fixed_exe(&env).unwrap();
        let config = root.join("Roaming/Claude/claude_desktop_config.json");
        std::fs::create_dir_all(config.parent().unwrap()).unwrap();
        std::fs::write(&config, b"{ broken").unwrap();
        for running in [Some(true), None, Some(false)] {
            let (text, code) = finish_claude_registration(&env, &fixed, InstallOptions::default(), running);
            assert_eq!(code, ExitCode::FAILURE, "{text}");
            assert!(text.contains(if running == Some(true) { "PENDING" } else { "ERROR" }));
            assert_eq!(std::fs::read(&config).unwrap(), b"{ broken");
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn helper_reports_partial_failure_and_successful_idempotent_registration() {
        let root = scratch("helper-register");
        let env = store_env(&root);
        let fixed = paths::fixed_exe(&env).unwrap();
        let config = root.join("Roaming/Claude/claude_desktop_config.json");
        std::fs::create_dir_all(config.parent().unwrap()).unwrap();
        std::fs::write(&config, br#"{"theme":"dark"}"#).unwrap();
        let bad = root.join("Local/Packages/Claude_test/LocalCache/Roaming/Claude/claude_desktop_config.json");
        std::fs::create_dir_all(bad.parent().unwrap()).unwrap();
        std::fs::write(&bad, b"{ broken").unwrap();
        let (text, code) = finish_claude_registration(&env, &fixed, InstallOptions::default(), Some(false));
        assert_eq!(code, ExitCode::FAILURE);
        assert!(text.contains("ERROR") && text.contains("ADDED"), "{text}");
        assert_eq!(claude::load(&config).unwrap().root["theme"], "dark");
        std::fs::write(&bad, b"{}").unwrap();
        let (_, code) = finish_claude_registration(&env, &fixed, InstallOptions::default(), Some(false));
        assert_eq!(code, ExitCode::SUCCESS);
        let before = std::fs::read(&config).unwrap();
        let (text, code) = finish_claude_registration(&env, &fixed, InstallOptions::default(), Some(false));
        assert_eq!(code, ExitCode::SUCCESS);
        assert!(text.contains("claude-desktop: OK"));
        assert_eq!(std::fs::read(&config).unwrap(), before);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn helper_does_not_report_registration_when_no_host_was_found() {
        let root = scratch("helper-no-host");
        let env = store_env(&root);
        let (text, code) = finish_claude_registration(&env, &paths::fixed_exe(&env).unwrap(), InstallOptions::default(), Some(false));
        assert_eq!(text, "claude-desktop: NOT_INSTALLED\n");
        assert_eq!(code, ExitCode::SUCCESS);
        assert!(doctor::details(&env).contains("not created"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn gil_executables_are_recognised_by_name_only() {
        assert!(is_gil_executable(r"C:\Users\Me\AppData\Local\GIL\runtime\gil-launcher.exe"));
        assert!(is_gil_executable("/Users/me/plugins/core/darwin-arm64/gil"));
        assert!(is_gil_executable(r"C:\x\GIL.EXE"));
        assert!(!is_gil_executable(r"C:\x\node.exe"));
        assert!(!is_gil_executable(""));
    }

    // The parking flow of the Windows-style placement. The Linux user install never parks: its own
    // rules are in `user_place::tests` and in the Linux test below, so this one is not compiled there.
    #[cfg(not(target_os = "linux"))]
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

    /// Linux user install through the public entries: a foreign entry stops the install before any
    /// Host file is touched, and the installed executable removes exactly what the install recorded.
    #[cfg(target_os = "linux")]
    #[test]
    fn linux_install_never_takes_over_an_entry_it_did_not_place_and_uninstall_removes_only_its_own() {
        let root = scratch("linux-own");
        let home = root.join("home");
        std::fs::create_dir_all(home.join(".codex")).unwrap();
        let config = "# sentinel\n[mcp_servers.other]\ncommand = \"/usr/bin/true\"\nargs = []\n";
        std::fs::write(home.join(".codex/config.toml"), config).unwrap();
        let env = Env { home: Some(home.clone()), ..Env::default() };
        let fixed = paths::fixed_exe(&env).unwrap();
        let dir = fixed.parent().unwrap();
        std::fs::create_dir_all(dir).unwrap();
        let me = std::fs::read(std::env::current_exe().unwrap()).unwrap();

        // Equal bytes that no install recorded: refused, and Codex was not registered.
        std::fs::write(&fixed, &me).unwrap();
        let refused = install(&env, InstallOptions::default()).unwrap_err();
        assert!(refused.contains("Nothing was changed"), "{refused}");
        assert!(!dir.join(INSTALL_RECEIPT).exists());
        assert_eq!(std::fs::read_to_string(home.join(".codex/config.toml")).unwrap(), config, "a Host setting changed before the install could succeed");
        let (text, _) = uninstall(&env, true).unwrap();
        assert!(text.contains("binary: KEPT"), "{text}");
        assert_eq!(std::fs::read(&fixed).unwrap(), me);
        std::fs::remove_file(&fixed).unwrap();
        std::fs::write(home.join(".codex/config.toml"), config).unwrap();

        // A regular install.json with no executable beside it: not overwritten, nothing placed, no Host change.
        std::fs::write(dir.join(INSTALL_RECEIPT), b"the user's own file\n").unwrap();
        let refused = install(&env, InstallOptions::default()).unwrap_err();
        assert!(refused.contains("Nothing was changed"), "{refused}");
        assert_eq!(std::fs::read(dir.join(INSTALL_RECEIPT)).unwrap(), b"the user's own file\n");
        assert!(!fixed.exists());
        assert_eq!(std::fs::read_to_string(home.join(".codex/config.toml")).unwrap(), config);
        let (text, _) = uninstall(&env, true).unwrap();
        assert!(text.contains("binary: NOT_INSTALLED"), "{text}");
        assert_eq!(std::fs::read(dir.join(INSTALL_RECEIPT)).unwrap(), b"the user's own file\n");
        std::fs::remove_file(dir.join(INSTALL_RECEIPT)).unwrap();
        std::fs::write(home.join(".codex/config.toml"), config).unwrap();

        // Our own install, its repeat, and the removal of exactly the two recorded files. The two
        // folders were there before this installation (created above), so they stay, empty.
        let (text, _) = install(&env, InstallOptions::default()).unwrap();
        assert!(text.contains("binary: UPDATED") && text.contains("claude-desktop: NOT_INSTALLED"), "{text}");
        assert_eq!(std::fs::read(&fixed).unwrap(), me);
        assert!(std::fs::read_to_string(home.join(".codex/config.toml")).unwrap().contains("# sentinel"));
        let (text, _) = install(&env, InstallOptions::default()).unwrap();
        assert!(text.contains("binary: OK"), "{text}");
        let sentinel = home.join(".local/share/other-app.conf");
        std::fs::write(&sentinel, b"keep").unwrap();
        let (text, code) = uninstall(&env, true).unwrap();
        assert!(text.contains("binary: REMOVED") && code == ExitCode::SUCCESS, "{text}");
        assert!(!fixed.exists() && !dir.join(INSTALL_RECEIPT).exists());
        assert!(dir.is_dir() && std::fs::read_dir(dir).unwrap().count() == 0, "folders that were there before the installation stay");
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"keep");
        let after = std::fs::read_to_string(home.join(".codex/config.toml")).unwrap();
        assert!(after.contains("[mcp_servers.other]") && after.contains("# sentinel"));
        let (text, _) = uninstall(&env, true).unwrap();
        assert!(text.contains("binary: NOT_INSTALLED"), "{text}");

        // A home with no gil folders: the installation makes them and its removal takes them again.
        let fresh = root.join("fresh-home");
        std::fs::create_dir_all(&fresh).unwrap();
        let fresh_env = Env { home: Some(fresh.clone()), ..Env::default() };
        install(&fresh_env, InstallOptions::default()).unwrap();
        let (text, _) = uninstall(&fresh_env, true).unwrap();
        assert!(text.contains("binary: REMOVED"), "{text}");
        assert!(!fresh.join(".local/share/gil").exists() && fresh.join(".local/share").is_dir());
        let _ = std::fs::remove_dir_all(root);
    }

    /// The same four rules through the public entries on every Unix where the placement compiles,
    /// by handing `uninstall`'s Linux branch its result type directly is not possible off Linux;
    /// what is checked here is the report line of a partial removal, which must never read REMOVED.
    #[cfg(unix)]
    #[test]
    fn a_partial_removal_is_never_reported_as_removed() {
        let partial = user_place::Removal { found: true, kept: false, removed: vec!["/x/gil".into()], remaining: vec!["/x/install.json (denied)".into()] };
        assert!(!partial.complete());
        let whole = user_place::Removal { found: true, kept: false, removed: vec!["/x/gil".into(), "/x/install.json".into()], remaining: vec![] };
        assert!(whole.complete());
        assert!(!user_place::Removal::default().complete() && !user_place::Removal { found: true, kept: true, ..Default::default() }.complete());
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
