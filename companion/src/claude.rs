//! Explicit Claude Desktop registration. This never runs during setup or update.
//!
//! Claude Desktop — including its **Claude Code** tab — reads MCP servers from its own settings
//! file, `claude_desktop_config.json` (observed 2026-11-02: the Claude Code tab served the `gil`
//! server registered there, while the user-scope `~/.claude.json` carried no servers). Claude
//! Desktop offers no command-line interface for that file; its documented route is the
//! *Developer → Edit Config* button, which opens the same file. So this adapter edits the file
//! itself, under the same rules a careful person would follow:
//!
//! - only the `mcpServers.gil` entry changes; every other key is written back with its parsed value
//!   parsed, and a `gil` entry that is not GIL's is never touched;
//! - a backup is kept beside the file, the new content lands through a temporary file and a
//!   rename, and the file is re-read before the rename so a concurrent change aborts the write;
//! - nothing is written while Claude Desktop is running — the running app rewrites the file with
//!   its own settings and silently reverts external edits (observed 2026-10-31).
//!
//! The registered command is the stable GIL launcher, the same entry point Codex gets. Updates
//! replace versioned Core files and a small receipt, never this launcher, so both Hosts pick the
//! newly selected Core on their next connection without re-registration.

use crate::clients::{Registration, launcher_path, runtime_version};
use serde::Serialize;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const FILE_NAME: &str = "claude_desktop_config.json";
const MAX_CONFIG_BYTES: u64 = 1024 * 1024;
const KEEP_BACKUPS: usize = 5;
pub const SERVER_NAME: &str = "gil";

#[derive(Serialize)]
pub struct ClaudeCapability {
    supported: bool,
    runtime_installed: bool,
    config_found: bool,
    runtime_version: Option<String>,
    config_path: Option<String>,
    reason: Option<String>,
}

#[derive(Serialize)]
pub struct ClaudeConnection {
    state: &'static str,
    /// The command the existing `gil` entry runs, as written. Shown so a person can tell a
    /// stale version-folder Core from the stable launcher before approving a replacement.
    command: Option<String>,
    desktop_running: Option<bool>,
}

/// What `load` read: the parsed object, whether the file existed, and the digest of the exact
/// bytes read. `store` compares that digest again right before replacing the file.
pub(crate) struct Loaded {
    pub root: Map<String, Value>,
    pub existed: bool,
    pub digest: Option<[u8; 32]>,
}

pub(crate) fn config_path() -> Result<PathBuf, String> {
    #[cfg(windows)]
    {
        let appdata = std::env::var_os("APPDATA").ok_or("APPDATA가 없다")?;
        Ok(PathBuf::from(appdata).join("Claude").join(FILE_NAME))
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME").ok_or("HOME이 없다")?;
        Ok(PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("Claude")
            .join(FILE_NAME))
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    Err("Claude Desktop 연결은 Windows와 macOS Companion에서만 제공한다".into())
}

fn digest_of(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

pub(crate) fn load(path: &Path) -> Result<Loaded, String> {
    let parent = path
        .parent()
        .ok_or("Claude Desktop 설정 경로가 올바르지 않다")?;
    if !parent.is_dir() {
        return Err(
            "Claude Desktop 설정 폴더를 찾지 못했다. Claude Desktop을 설치하고 한 번 실행한 뒤 다시 시도한다."
                .into(),
        );
    }
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Loaded {
                root: Map::new(),
                existed: false,
                digest: None,
            });
        }
        Err(err) => return Err(format!("Claude Desktop 설정을 읽지 못했다: {err}")),
    };
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err("Claude Desktop 설정이 일반 파일이 아니라서 건드리지 않는다".into());
    }
    if metadata.len() > MAX_CONFIG_BYTES {
        return Err("Claude Desktop 설정이 예상보다 커서 건드리지 않는다".into());
    }
    let bytes =
        std::fs::read(path).map_err(|err| format!("Claude Desktop 설정을 읽지 못했다: {err}"))?;
    let root = parse(&bytes)?;
    Ok(Loaded {
        root,
        existed: true,
        digest: Some(digest_of(&bytes)),
    })
}

pub(crate) fn parse(bytes: &[u8]) -> Result<Map<String, Value>, String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| "Claude Desktop 설정이 UTF-8 텍스트가 아니다".to_owned())?;
    let value: Value = if text.trim().is_empty() {
        Value::Object(Map::new())
    } else {
        serde_json::from_str(text)
            .map_err(|err| format!("Claude Desktop 설정을 해석하지 못했다: {err}"))?
    };
    let Value::Object(root) = value else {
        return Err("Claude Desktop 설정의 최상위가 객체가 아니라서 건드리지 않는다".into());
    };
    if let Some(servers) = root.get("mcpServers")
        && !servers.is_object()
    {
        return Err("Claude Desktop 설정의 mcpServers가 객체가 아니라서 건드리지 않는다".into());
    }
    Ok(root)
}

pub(crate) fn entry(root: &Map<String, Value>) -> Option<&Value> {
    root.get("mcpServers")?.as_object()?.get(SERVER_NAME)
}

fn command_of(entry: &Value) -> Option<&str> {
    entry.get("command")?.as_str()
}

fn same_path(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

pub(crate) fn classify(root: &Map<String, Value>, launcher: &Path) -> Registration {
    let Some(entry) = entry(root) else {
        return Registration::Missing;
    };
    let Some(command) = command_of(entry) else {
        return Registration::Conflict;
    };
    let args_ok = entry
        .get("args")
        .and_then(Value::as_array)
        .is_some_and(|args| {
            args.iter()
                .map(Value::as_str)
                .eq(["mcp", "--serve"].map(Some))
        });
    // Codex and Claude differ in how a registration may be extended (`env`, `cwd`); anything
    // beyond command and args is not a registration this adapter made.
    let only_known_keys = entry
        .as_object()
        .is_some_and(|map| map.keys().all(|key| key == "command" || key == "args"));
    if same_path(command, &launcher.to_string_lossy()) && args_ok && only_known_keys {
        Registration::Ours
    } else {
        Registration::Conflict
    }
}

/// A `gil` entry that runs some GIL executable — an older direct registration pointing at a
/// version folder, a Plugin cache Core or the launcher with different arguments. Only these may
/// be replaced on repair; a `gil` entry running anything else belongs to the user.
pub(crate) fn is_gil_executable(entry: &Value) -> bool {
    let Some(command) = command_of(entry) else {
        return false;
    };
    // A registration written on Windows uses backslashes; classify by the last path component
    // whichever separator the writing Host used.
    let name = command
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        name.as_str(),
        "gil" | "gil.exe" | "gil-launcher" | "gil-launcher.exe"
    )
}

pub(crate) fn with_registration(
    mut root: Map<String, Value>,
    launcher: &Path,
) -> Map<String, Value> {
    let servers = root
        .entry("mcpServers")
        .or_insert_with(|| Value::Object(Map::new()));
    if let Some(servers) = servers.as_object_mut() {
        servers.insert(
            SERVER_NAME.into(),
            json!({"command": launcher.to_string_lossy(), "args": ["mcp", "--serve"]}),
        );
    }
    root
}

pub(crate) fn without_registration(mut root: Map<String, Value>) -> Map<String, Value> {
    if let Some(servers) = root.get_mut("mcpServers").and_then(Value::as_object_mut) {
        servers.remove(SERVER_NAME);
    }
    root
}

fn backup_name(id: impl std::fmt::Display) -> String {
    format!("{FILE_NAME}.gil-backup-{id}")
}

fn prune_backups(dir: &Path) {
    let prefix = format!("{FILE_NAME}.gil-backup-");
    let mut backups: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.file_name().is_some_and(|name| {
            let name = name.to_string_lossy();
            name.strip_prefix(&prefix).is_some_and(|id| !id.is_empty()
                && id.bytes().all(|b| b.is_ascii_digit() || b == b'-'))
        }))
        .collect();
    backups.sort();
    while backups.len() > KEEP_BACKUPS {
        let _ = std::fs::remove_file(backups.remove(0));
    }
}

/// Serialize GIL writers and check for external edits immediately before replacement.
/// Other editors do not honor our lock: Desktop must be stopped and the user must not edit
/// settings during this operation. Detected conflicts leave the current file untouched.
pub(crate) fn store(path: &Path, loaded: &Loaded, root: &Map<String, Value>) -> Result<(), String> {
    store_before_replace(path, loaded, root, || {})
}

fn store_before_replace(
    path: &Path, loaded: &Loaded, root: &Map<String, Value>, before_replace: impl FnOnce(),
) -> Result<(), String> {
    let dir = path
        .parent()
        .ok_or("Claude Desktop 설정 경로가 올바르지 않다")?;
    let text = serde_json::to_string_pretty(&Value::Object(root.clone()))
        .map_err(|err| format!("Claude Desktop 설정을 만들지 못했다: {err}"))?
        + "\n";

    // Keep this file in place: unlinking a held lock would let another writer lock a new inode.
    let lock = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false)
        .open(dir.join(format!("{FILE_NAME}.gil-lock")))
        .map_err(|err| format!("Claude Desktop 설정 잠금을 열지 못했다: {err}"))?;
    lock.try_lock().map_err(|_| "다른 GIL 연결 작업이 진행 중이다. 완료 후 다시 시도한다".to_owned())?;
    ensure_unchanged(path, loaded)?;
    let original = if loaded.existed {
        let bytes = std::fs::read(path).map_err(|err| format!("설정 백업 원본을 읽지 못했다: {err}"))?;
        if Some(digest_of(&bytes)) != loaded.digest { return Err("설정이 읽은 뒤에 바뀌어서 쓰지 않았다".into()); }
        Some(bytes)
    } else { None };
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let id = format!("{}-{}-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos()).unwrap_or(0), std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
    if let Some(bytes) = original {
        write_new_synced(&dir.join(backup_name(&id)), &bytes)?;
    }
    let temporary = dir.join(format!("{FILE_NAME}.gil-next-{id}"));
    write_new_synced(&temporary, text.as_bytes())?;
    before_replace();
    if let Err(err) = ensure_unchanged(path, loaded) {
        let _ = std::fs::remove_file(&temporary);
        return Err(err);
    }
    if let Err(err) = std::fs::rename(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(format!("Claude Desktop 설정을 교체하지 못했다: {err}"));
    }
    prune_backups(dir);
    Ok(())
}

fn ensure_unchanged(path: &Path, loaded: &Loaded) -> Result<(), String> {
    let now = load(path)?;
    if now.existed != loaded.existed || now.digest != loaded.digest {
        return Err("Claude Desktop 설정이 읽은 뒤에 바뀌어서 쓰지 않았다. 설정 편집을 마친 뒤 다시 시도한다.".into());
    }
    Ok(())
}

fn write_new_synced(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
    let mut file = options.open(path).map_err(|err| format!("설정 보존 파일을 만들지 못했다: {err}"))?;
    if let Err(err) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(format!("설정 보존 파일을 쓰지 못했다: {err}"));
    }
    Ok(())
}

/// Whether the Claude Desktop main process is running. `None` when the check itself failed.
pub(crate) fn desktop_running() -> Option<bool> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let output = std::process::Command::new("tasklist")
            .args(["/FI", "IMAGENAME eq Claude.exe", "/FO", "CSV", "/NH"])
            .creation_flags(0x0800_0000)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        Some(
            String::from_utf8_lossy(&output.stdout)
                .to_ascii_lowercase()
                .contains("\"claude.exe\""),
        )
    }
    #[cfg(target_os = "macos")]
    {
        // Only the main `Claude` process counts. Helper processes such as the browser
        // extension's native host survive a quit and must not block the edit (observed 2026-11-01).
        let status = std::process::Command::new("pgrep")
            .args(["-x", "Claude"])
            .stdout(std::process::Stdio::null())
            .status()
            .ok()?;
        match status.code() {
            Some(0) => Some(true),
            Some(1) => Some(false),
            _ => None,
        }
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    None
}

fn refuse_while_running() -> Result<(), String> {
    require_stopped(desktop_running())
}

fn require_stopped(running: Option<bool>) -> Result<(), String> {
    if running == Some(true) {
        return Err(
            "Claude Desktop이 실행 중이다. 실행 중에 바꾼 설정은 Desktop이 되돌릴 수 있으므로 Claude Desktop을 완전히 종료한 뒤 다시 누른다."
                .into(),
        );
    }
    if running.is_none() {
        return Err("Claude Desktop 실행 상태를 확인하지 못해 설정을 바꾸지 않았다. 확인 후 다시 시도한다.".into());
    }
    Ok(())
}

fn connection_state(root: &Map<String, Value>, launcher: &Path) -> (&'static str, Option<String>) {
    let command = entry(root).and_then(command_of).map(str::to_owned);
    let state = match classify(root, launcher) {
        Registration::Missing => "not_connected",
        Registration::Ours => "connected",
        Registration::Conflict => {
            if entry(root).is_some_and(is_gil_executable) {
                "needs_repair"
            } else {
                "conflict"
            }
        }
    };
    (state, command)
}

#[tauri::command]
pub fn claude_capability() -> ClaudeCapability {
    let launcher = launcher_path();
    let config = config_path();
    let supported = launcher.is_ok() && config.is_ok();
    let reason = match (&launcher, &config) {
        (Err(why), _) => Some(why.clone()),
        (_, Err(why)) => Some(why.clone()),
        _ => None,
    };
    ClaudeCapability {
        supported,
        runtime_installed: launcher.as_ref().is_ok_and(|one| one.is_file()),
        config_found: config.as_ref().is_ok_and(|one| one.is_file()),
        runtime_version: runtime_version(),
        config_path: config.ok().map(|one| one.display().to_string()),
        reason,
    }
}

#[tauri::command]
pub async fn claude_connection() -> Result<ClaudeConnection, String> {
    let launcher = launcher_path()?;
    let path = config_path()?;
    tauri::async_runtime::spawn_blocking(move || {
        let loaded = load(&path)?;
        let (state, command) = connection_state(&loaded.root, &launcher);
        Ok(ClaudeConnection {
            state,
            command,
            desktop_running: desktop_running(),
        })
    })
    .await
    .map_err(|err| format!("Claude Desktop 연결 상태를 확인하지 못했다: {err}"))?
}

#[tauri::command]
pub async fn connect_claude() -> Result<String, String> {
    let launcher = launcher_path()?;
    if !launcher.is_file() {
        return Err("GIL 런타임이 설치되지 않았다. Companion 설치를 다시 실행한다.".into());
    }
    let path = config_path()?;
    tauri::async_runtime::spawn_blocking(move || register(&path, &launcher))
        .await
        .map_err(|err| format!("Claude Desktop 연결 작업을 마치지 못했다: {err}"))?
}

fn register(path: &Path, launcher: &Path) -> Result<String, String> {
    register_with_check(path, launcher, refuse_while_running)
}

fn register_with_check(path: &Path, launcher: &Path, check: impl FnOnce() -> Result<(), String>) -> Result<String, String> {
    let loaded = load(path)?;
    match classify(&loaded.root, launcher) {
        Registration::Ours => return Ok("Claude Desktop에 GIL이 이미 연결되어 있다.".into()),
        Registration::Conflict => {
            return Err(if entry(&loaded.root).is_some_and(is_gil_executable) {
                "Claude Desktop에 다른 GIL 실행 파일을 가리키는 gil 등록이 있다. 자동으로 덮어쓰지 않는다 — 이전 GIL 연결 교체를 사용한다.".into()
            } else {
                "Claude Desktop에 이름이 같은 다른 gil 등록이 있다. 자동으로 덮어쓰지 않는다."
                    .into()
            });
        }
        Registration::Missing => {}
    }
    check()?;
    let next = with_registration(loaded.root.clone(), launcher);
    store(path, &loaded, &next)?;
    if classify(&load(path)?.root, launcher) != Registration::Ours {
        return Err("Claude Desktop 설정을 썼지만 gil 등록이 확인되지 않는다".into());
    }
    Ok("Claude Desktop에 GIL을 연결했다. Claude Desktop을 열면 새 대화부터 적용된다.".into())
}

#[tauri::command]
pub async fn disconnect_claude() -> Result<String, String> {
    let launcher = launcher_path()?;
    let path = config_path()?;
    tauri::async_runtime::spawn_blocking(move || disconnect(&path, &launcher))
        .await
        .map_err(|err| format!("Claude Desktop 연결 해제 작업을 마치지 못했다: {err}"))?
}

fn disconnect(path: &Path, launcher: &Path) -> Result<String, String> {
    disconnect_with_check(path, launcher, refuse_while_running)
}

fn disconnect_with_check(path: &Path, launcher: &Path, check: impl FnOnce() -> Result<(), String>) -> Result<String, String> {
    let loaded = load(path)?;
    match classify(&loaded.root, launcher) {
        Registration::Missing => {
            return Ok("Claude Desktop에서 GIL 연결이 이미 해제되어 있다.".into());
        }
        Registration::Conflict => {
            return Err(
                "이름이 같은 gil 등록이 GIL 고정 런처를 가리키지 않아 삭제하지 않는다.".into(),
            );
        }
        Registration::Ours => {}
    }
    check()?;
    let next = without_registration(loaded.root.clone());
    store(path, &loaded, &next)?;
    if classify(&load(path)?.root, launcher) != Registration::Missing {
        return Err("Claude Desktop 설정을 썼지만 gil 등록이 남아 있다".into());
    }
    Ok("Claude Desktop에서 GIL 연결을 해제했다. 이미 열린 대화는 닫은 뒤 제거를 계속한다.".into())
}

#[tauri::command]
pub async fn repair_claude_connection() -> Result<String, String> {
    let launcher = launcher_path()?;
    if !launcher.is_file() {
        return Err("GIL 런타임이 설치되지 않았다. Companion 설치를 다시 실행한다.".into());
    }
    let path = config_path()?;
    tauri::async_runtime::spawn_blocking(move || repair(&path, &launcher))
        .await
        .map_err(|err| format!("Claude Desktop 연결 교체 작업을 마치지 못했다: {err}"))?
}

fn repair(path: &Path, launcher: &Path) -> Result<String, String> {
    repair_with_check(path, launcher, refuse_while_running)
}

fn repair_with_check(path: &Path, launcher: &Path, check: impl FnOnce() -> Result<(), String>) -> Result<String, String> {
    let loaded = load(path)?;
    match classify(&loaded.root, launcher) {
        Registration::Ours => return Ok("Claude Desktop에 GIL이 이미 연결되어 있다.".into()),
        Registration::Missing => return register_with_check(path, launcher, check),
        Registration::Conflict => {}
    }
    if !entry(&loaded.root).is_some_and(is_gil_executable) {
        return Err(
            "Claude Desktop의 gil 등록이 GIL 실행 파일이 아니라서 교체하지 않는다. 사용자가 직접 만든 등록으로 본다.".into(),
        );
    }
    check()?;
    let next = with_registration(loaded.root.clone(), launcher);
    store(path, &loaded, &next)?;
    if classify(&load(path)?.root, launcher) != Registration::Ours {
        return Err("Claude Desktop 연결 교체가 끝까지 확인되지 않았다".into());
    }
    Ok("이전 GIL 연결을 설치된 GIL 고정 런처로 교체했다. Claude Desktop을 열면 새 대화부터 적용된다.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn launcher() -> PathBuf {
        if cfg!(windows) {
            PathBuf::from(r"C:\Users\Me\AppData\Local\GIL\runtime\gil-launcher.exe")
        } else {
            PathBuf::from("/Users/me/Library/Application Support/GIL/runtime/gil-launcher")
        }
    }

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "gil-claude-config-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(dir.join("Claude")).unwrap();
        dir.join("Claude").join(FILE_NAME)
    }

    #[test]
    fn only_the_exact_launcher_registration_is_owned() {
        let launcher = launcher();
        let ours = with_registration(Map::new(), &launcher);
        assert_eq!(classify(&ours, &launcher), Registration::Ours);
        assert_eq!(classify(&Map::new(), &launcher), Registration::Missing);

        let mut other_args = ours.clone();
        other_args["mcpServers"]["gil"]["args"] = json!(["mcp"]);
        assert_eq!(classify(&other_args, &launcher), Registration::Conflict);

        let mut with_env = ours.clone();
        with_env["mcpServers"]["gil"]["env"] = json!({"X": "1"});
        assert_eq!(classify(&with_env, &launcher), Registration::Conflict);

        let foreign =
            parse(br#"{"mcpServers":{"gil":{"command":"node","args":["server.mjs"]}}}"#).unwrap();
        assert_eq!(classify(&foreign, &launcher), Registration::Conflict);
        assert!(!is_gil_executable(entry(&foreign).unwrap()));
        assert_eq!(connection_state(&foreign, &launcher).0, "conflict");
    }

    #[test]
    fn stale_direct_registrations_of_a_gil_core_need_repair() {
        let launcher = launcher();
        let stale = parse(br#"{"mcpServers":{"gil":{"command":"/Users/me/Library/Application Support/GIL/dev-20261101-local/plugins/gil-companion-prototype/core/darwin-arm64/gil","args":["mcp","--serve"]}}}"#).unwrap();
        assert_eq!(classify(&stale, &launcher), Registration::Conflict);
        assert!(is_gil_executable(entry(&stale).unwrap()));
        let (state, command) = connection_state(&stale, &launcher);
        assert_eq!(state, "needs_repair");
        assert!(command.unwrap().ends_with("/gil"));

        let windows_core = parse(br#"{"mcpServers":{"gil":{"command":"C:\\Users\\Me\\AppData\\Local\\GIL\\runtime\\versions\\0.1.1\\gil.exe","args":["mcp","--serve"]}}}"#).unwrap();
        assert!(is_gil_executable(entry(&windows_core).unwrap()));
    }

    #[test]
    fn other_servers_and_settings_survive_connect_and_disconnect() {
        let launcher = launcher();
        let original = br#"{
  "coworkUserFilesPath": "/Users/me/Documents/Claude",
  "mcpServers": {
    "map": {"command": "npx", "args": ["-y", "@modelcontextprotocol/server-map", "--stdio"]}
  },
  "preferences": {"theme": "dark"}
}"#;
        let root = parse(original).unwrap();
        let connected = with_registration(root.clone(), &launcher);
        assert_eq!(
            connected["coworkUserFilesPath"],
            root["coworkUserFilesPath"]
        );
        assert_eq!(connected["preferences"], root["preferences"]);
        assert_eq!(connected["mcpServers"]["map"], root["mcpServers"]["map"]);
        assert_eq!(classify(&connected, &launcher), Registration::Ours);
        let restored = without_registration(connected);
        assert_eq!(restored, root);
    }

    #[test]
    fn malformed_settings_are_refused_rather_than_rewritten() {
        assert!(parse(b"[]").is_err());
        assert!(parse(br#"{"mcpServers": []}"#).is_err());
        assert!(parse(b"{not json").is_err());
        assert!(parse(b"   ").unwrap().is_empty());
        assert!(parse(&[0xff, 0xfe]).is_err());
    }

    #[test]
    fn a_missing_file_registers_cleanly_and_disconnect_leaves_the_file() {
        let path = scratch("fresh");
        let launcher = launcher();
        assert!(matches!(load(&path), Ok(Loaded { existed: false, .. })));
        register_with_check(&path, &launcher, || Ok(())).unwrap();
        let loaded = load(&path).unwrap();
        assert!(loaded.existed);
        assert_eq!(classify(&loaded.root, &launcher), Registration::Ours);
        assert!(std::fs::read_dir(path.parent().unwrap()).unwrap().count() >= 1);
        disconnect_with_check(&path, &launcher, || Ok(())).unwrap();
        let after = load(&path).unwrap();
        assert_eq!(classify(&after.root, &launcher), Registration::Missing);
        assert_eq!(after.root["mcpServers"], json!({}));
        std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap()).unwrap();
    }

    #[test]
    fn a_concurrent_change_aborts_the_write_and_keeps_a_backup_otherwise() {
        let path = scratch("conflict");
        let launcher = launcher();
        std::fs::write(
            &path,
            br#"{"mcpServers":{"map":{"command":"npx","args":[]}}}"#,
        )
        .unwrap();
        let loaded = load(&path).unwrap();
        std::fs::write(
            &path,
            br#"{"mcpServers":{"map":{"command":"npx","args":["-y"]}}}"#,
        )
        .unwrap();
        let next = with_registration(loaded.root.clone(), &launcher);
        let refused = store(&path, &loaded, &next).unwrap_err();
        assert!(refused.contains("바뀌어서"));
        let kept = std::fs::read(&path).unwrap();
        assert!(kept.ends_with(br#"["-y"]}}}"#));

        let loaded = load(&path).unwrap();
        store(
            &path,
            &loaded,
            &with_registration(loaded.root.clone(), &launcher),
        )
        .unwrap();
        let dir = path.parent().unwrap();
        let backups: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().contains(".gil-backup-"))
            .collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(std::fs::read(backups[0].path()).unwrap(), kept);
        assert!(!dir.join(format!("{FILE_NAME}.gil-next")).exists());
        let now = load(&path).unwrap();
        assert_eq!(classify(&now.root, &launcher), Registration::Ours);
        assert_eq!(now.root["mcpServers"]["map"]["args"], json!(["-y"]));
        std::fs::remove_dir_all(dir.parent().unwrap()).unwrap();
    }

    #[test]
    fn repair_replaces_only_a_gil_executable_entry() {
        let path = scratch("repair");
        let launcher = launcher();
        std::fs::write(&path, br#"{"mcpServers":{"gil":{"command":"/old/plugins/gil-companion-prototype/core/darwin-arm64/gil","args":["mcp","--serve"]},"map":{"command":"npx","args":[]}}}"#).unwrap();
        assert!(
            register_with_check(&path, &launcher, || Ok(()))
                .unwrap_err()
                .contains("이전 GIL 연결 교체")
        );
        { // Isolated fixture: do not inspect or stop the developer's actual Desktop.
            repair_with_check(&path, &launcher, || Ok(())).unwrap();
            let now = load(&path).unwrap();
            assert_eq!(classify(&now.root, &launcher), Registration::Ours);
            assert!(now.root["mcpServers"]["map"].is_object());
        }

        std::fs::write(
            &path,
            br#"{"mcpServers":{"gil":{"command":"node","args":["mine.mjs"]}}}"#,
        )
        .unwrap();
        assert!(
            repair_with_check(&path, &launcher, || Ok(()))
                .unwrap_err()
                .contains("교체하지 않는다")
        );
        assert!(
            disconnect_with_check(&path, &launcher, || Ok(()))
                .unwrap_err()
                .contains("삭제하지 않는다")
        );
        let untouched = load(&path).unwrap();
        assert_eq!(
            untouched.root["mcpServers"]["gil"]["command"],
            json!("node")
        );
        std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap()).unwrap();
    }

    #[test]
    fn late_external_edit_is_kept_and_our_temporary_is_removed() {
        let path = scratch("late-conflict");
        std::fs::write(&path, br#"{"preferences":{"theme":"dark"}}"#).unwrap();
        let loaded = load(&path).unwrap();
        let next = with_registration(loaded.root.clone(), &launcher());
        let newer = br#"{"preferences":{"theme":"light"}}"#;
        let err = store_before_replace(&path, &loaded, &next, || {
            std::fs::write(&path, newer).unwrap();
        }).unwrap_err();
        assert!(err.contains("바뀌어서"));
        assert_eq!(std::fs::read(&path).unwrap(), newer);
        assert!(!std::fs::read_dir(path.parent().unwrap()).unwrap().filter_map(Result::ok)
            .any(|e| e.file_name().to_string_lossy().contains(".gil-next-")));
        std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap()).unwrap();
    }

    #[test]
    fn simultaneous_gil_writer_is_refused_and_rapid_backups_are_distinct() {
        let path = scratch("serialized");
        let first = br#"{"preferences":{"theme":"dark"}}"#;
        std::fs::write(&path, first).unwrap();
        let loaded = load(&path).unwrap();
        let next = with_registration(loaded.root.clone(), &launcher());
        store_before_replace(&path, &loaded, &next, || {
            assert!(store(&path, &loaded, &next).unwrap_err().contains("진행 중"));
        }).unwrap();
        let second = std::fs::read(&path).unwrap();
        let now = load(&path).unwrap();
        store(&path, &now, &without_registration(now.root.clone())).unwrap();
        let backups: Vec<_> = std::fs::read_dir(path.parent().unwrap()).unwrap().filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().contains(".gil-backup-"))
            .map(|e| std::fs::read(e.path()).unwrap()).collect();
        assert_eq!(backups.len(), 2);
        assert!(backups.iter().any(|b| b == first));
        assert!(backups.contains(&second));
        std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap()).unwrap();
    }

    #[test]
    fn an_unknown_desktop_process_state_does_not_authorize_a_write() {
        assert!(require_stopped(Some(false)).is_ok());
        assert!(require_stopped(Some(true)).is_err());
        assert!(require_stopped(None).is_err());
        let path = scratch("unknown-process");
        let before = br#"{"preferences":{"theme":"dark"}}"#;
        std::fs::write(&path, before).unwrap();
        assert!(register_with_check(&path, &launcher(), || require_stopped(None)).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(std::fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
        std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap()).unwrap();
    }

    #[test]
    fn old_backups_are_pruned_to_a_small_number() {
        let path = scratch("prune");
        let dir = path.parent().unwrap();
        for i in 0..(KEEP_BACKUPS + 3) {
            std::fs::write(dir.join(backup_name(1000 + i as u64)), b"{}").unwrap();
        }
        prune_backups(dir);
        let left: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        assert_eq!(left.len(), KEEP_BACKUPS);
        assert!(
            left.iter()
                .all(|e| !e.file_name().to_string_lossy().ends_with("-1000"))
        );
        std::fs::remove_dir_all(dir.parent().unwrap()).unwrap();
    }
}
