//! Claude Desktop — `claude_desktop_config.json`, `mcpServers.gil`.
//!
//! Only that one entry changes. Every other key and every other server is written back with its
//! parsed value; keys come out sorted, because enabling `serde_json`'s `preserve_order` would
//! reorder every JSON value the Core emits (the Monitor wire fixtures are byte-compared).
//! Claude Desktop rewrites this file in its own order anyway. A `gil` entry that does not run
//! GIL is never replaced without `--replace`.

use super::{Registration, SERVER_NAME, SERVE_ARGS, file, is_gil_executable, same_path};
use serde_json::{Map, Value, json};
use std::path::Path;

const MAX_CONFIG_BYTES: u64 = 1024 * 1024;
const WHAT: &str = "Claude Desktop settings";

pub(crate) struct Loaded {
    pub root: Map<String, Value>,
    pub file: file::Loaded,
}

pub(crate) fn load(path: &Path) -> Result<Loaded, String> {
    let loaded = file::load(path, MAX_CONFIG_BYTES, WHAT)?;
    let root = parse(&loaded.bytes)?;
    Ok(Loaded { root, file: loaded })
}

pub(crate) fn parse(bytes: &[u8]) -> Result<Map<String, Value>, String> {
    let text = file::text_of(bytes, WHAT)?;
    let value: Value = if text.trim().is_empty() {
        Value::Object(Map::new())
    } else {
        serde_json::from_str(text).map_err(|err| format!("{WHAT} could not be parsed: {err}"))?
    };
    let Value::Object(root) = value else {
        return Err(format!("{WHAT} top level is not an object; left untouched"));
    };
    if root.get("mcpServers").is_some_and(|servers| !servers.is_object()) {
        return Err(format!("{WHAT} mcpServers is not an object; left untouched"));
    }
    Ok(root)
}

pub(crate) fn entry(root: &Map<String, Value>) -> Option<&Value> {
    root.get("mcpServers")?.as_object()?.get(SERVER_NAME)
}

pub(crate) fn command_of(entry: &Value) -> Option<&str> {
    entry.get("command")?.as_str()
}

pub(crate) fn classify(root: &Map<String, Value>, exe: &Path) -> Registration {
    let Some(entry) = entry(root) else { return Registration::Missing };
    let Some(command) = command_of(entry) else { return Registration::Foreign };
    let args_ok = entry.get("args").and_then(Value::as_array)
        .is_some_and(|args| args.iter().map(Value::as_str).eq(SERVE_ARGS.map(Some)));
    let only_known_keys = entry.as_object().is_some_and(|map| map.keys().all(|key| key == "command" || key == "args"));
    if same_path(command, &exe.to_string_lossy()) && args_ok && only_known_keys {
        Registration::Ours
    } else if is_gil_executable(command) {
        Registration::Stale
    } else {
        Registration::Foreign
    }
}

fn with_registration(mut root: Map<String, Value>, exe: &Path) -> Map<String, Value> {
    let servers = root.entry("mcpServers").or_insert_with(|| Value::Object(Map::new()));
    if let Some(servers) = servers.as_object_mut() {
        servers.insert(SERVER_NAME.into(), json!({"command": exe.to_string_lossy(), "args": SERVE_ARGS}));
    }
    root
}

fn without_registration(mut root: Map<String, Value>) -> Map<String, Value> {
    if let Some(servers) = root.get_mut("mcpServers").and_then(Value::as_object_mut) {
        servers.remove(SERVER_NAME);
    }
    root
}

fn render(root: &Map<String, Value>) -> Result<String, String> {
    serde_json::to_string_pretty(&Value::Object(root.clone()))
        .map(|text| text + "\n")
        .map_err(|err| format!("{WHAT} could not be serialized: {err}"))
}

/// Register at `path`. Returns `OK` (already ours), `ADDED`, `UPDATED` or an error.
pub(crate) fn register(path: &Path, exe: &Path, replace: bool) -> Result<&'static str, String> {
    let loaded = load(path)?;
    let outcome = match classify(&loaded.root, exe) {
        Registration::Ours => return Ok("OK"),
        Registration::Missing => "ADDED",
        Registration::Stale => "UPDATED",
        Registration::Foreign if replace => "UPDATED",
        Registration::Foreign => {
            return Err(format!(
                "the existing `gil` entry runs {:?}, which is not GIL; left untouched (use --replace to overwrite)",
                entry(&loaded.root).and_then(command_of).unwrap_or("<no command>")
            ));
        }
    };
    let next = with_registration(loaded.root.clone(), exe);
    file::store(path, &loaded.file, &render(&next)?, WHAT)?;
    if classify(&load(path)?.root, exe) != Registration::Ours {
        return Err("the Claude Desktop entry did not read back as expected".into());
    }
    Ok(outcome)
}

/// Remove our entry. Returns `REMOVED`, `MISSING`, or `KEPT` for a foreign entry.
pub(crate) fn unregister(path: &Path) -> Result<&'static str, String> {
    let loaded = load(path)?;
    match entry(&loaded.root) {
        None => Ok("MISSING"),
        Some(entry) if !command_of(entry).is_some_and(is_gil_executable) => Ok("KEPT (not a GIL entry)"),
        Some(_) => {
            let next = without_registration(loaded.root.clone());
            file::store(path, &loaded.file, &render(&next)?, WHAT)?;
            Ok("REMOVED")
        }
    }
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
        if !output.status.success() { return None; }
        Some(String::from_utf8_lossy(&output.stdout).to_ascii_lowercase().contains("\"claude.exe\""))
    }
    #[cfg(target_os = "macos")]
    {
        // Only the main `Claude` process counts; helper processes survive a quit.
        let status = std::process::Command::new("pgrep").args(["-x", "Claude"])
            .stdout(std::process::Stdio::null()).status().ok()?;
        match status.code() { Some(0) => Some(true), Some(1) => Some(false), _ => None }
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    None
}

pub(crate) fn require_stopped(running: Option<bool>) -> Result<(), String> {
    match running {
        Some(true) => Err("Claude Desktop is running; a running app can revert this edit. Quit it completely (tray icon → Quit), then retry".into()),
        None => Err("could not tell whether Claude Desktop is running; quit it, then retry".into()),
        Some(false) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn exe() -> PathBuf {
        if cfg!(windows) { PathBuf::from(r"C:\Users\Me\AppData\Local\Programs\gil\gil.exe") } else { PathBuf::from("/Users/me/gil/bin/gil") }
    }

    fn config(tag: &str, text: &str) -> PathBuf {
        let dir = super::super::tests::scratch(&format!("claude-{tag}"));
        let path = dir.join("claude_desktop_config.json");
        std::fs::write(&path, text).unwrap();
        path
    }

    #[test]
    fn register_adds_our_entry_and_keeps_everything_else() {
        let path = config("add", "{\n  \"preferences\": {\"z\": 1, \"a\": 2},\n  \"mcpServers\": {\"other\": {\"command\": \"node\"}}\n}\n");
        assert_eq!(register(&path, &exe(), false).unwrap(), "ADDED");
        let text = std::fs::read_to_string(&path).unwrap();
        let root = parse(text.as_bytes()).unwrap();
        assert_eq!(classify(&root, &exe()), Registration::Ours);
        assert_eq!(root["mcpServers"]["other"]["command"], "node");
        assert_eq!(root["preferences"]["z"], 1, "other top-level keys survive: {text}");
        assert_eq!(root["preferences"]["a"], 2);
        // Idempotent.
        let before = std::fs::read(&path).unwrap();
        assert_eq!(register(&path, &exe(), false).unwrap(), "OK");
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(unregister(&path).unwrap(), "REMOVED");
        let root = parse(std::fs::read(&path).unwrap().as_slice()).unwrap();
        assert_eq!(classify(&root, &exe()), Registration::Missing);
        assert_eq!(root["mcpServers"]["other"]["command"], "node");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn register_creates_a_missing_or_empty_file() {
        let path = config("empty", "");
        assert_eq!(register(&path, &exe(), false).unwrap(), "ADDED");
        let missing = path.parent().unwrap().join("fresh").join("claude_desktop_config.json");
        std::fs::create_dir_all(missing.parent().unwrap()).unwrap();
        assert_eq!(register(&missing, &exe(), false).unwrap(), "ADDED");
        assert_eq!(classify(&load(&missing).unwrap().root, &exe()), Registration::Ours);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn stale_gil_entries_are_updated_but_foreign_ones_are_kept() {
        let old = if cfg!(windows) { r"C:\\Users\\Me\\AppData\\Local\\GIL\\runtime\\gil-launcher.exe" } else { "/old/gil-launcher" };
        let path = config("stale", &format!("{{\"mcpServers\": {{\"gil\": {{\"command\": \"{old}\", \"args\": [\"mcp\", \"--serve\"]}}}}}}"));
        assert_eq!(classify(&load(&path).unwrap().root, &exe()), Registration::Stale);
        assert_eq!(register(&path, &exe(), false).unwrap(), "UPDATED");
        assert_eq!(classify(&load(&path).unwrap().root, &exe()), Registration::Ours);

        let foreign = config("foreign", "{\"mcpServers\": {\"gil\": {\"command\": \"node\", \"args\": [\"x.js\"]}}}");
        assert_eq!(classify(&load(&foreign).unwrap().root, &exe()), Registration::Foreign);
        let err = register(&foreign, &exe(), false).unwrap_err();
        assert!(err.contains("not GIL"), "{err}");
        assert_eq!(load(&foreign).unwrap().root["mcpServers"]["gil"]["command"], "node");
        assert_eq!(unregister(&foreign).unwrap(), "KEPT (not a GIL entry)");
        assert_eq!(register(&foreign, &exe(), true).unwrap(), "UPDATED");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
        let _ = std::fs::remove_dir_all(foreign.parent().unwrap());
    }

    #[test]
    fn broken_files_are_reported_not_overwritten() {
        let path = config("broken", "{ not json");
        let err = register(&path, &exe(), false).unwrap_err();
        assert!(err.contains("could not be parsed"), "{err}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ not json");
        let array = config("array", "[]");
        assert!(register(&array, &exe(), false).unwrap_err().contains("not an object"));
        let bom = config("bom", "\u{FEFF}{\"mcpServers\": {}}");
        assert_eq!(register(&bom, &exe(), false).unwrap(), "ADDED");
        for p in [path, array, bom] { let _ = std::fs::remove_dir_all(p.parent().unwrap()); }
    }

    #[test]
    fn require_stopped_reads_the_three_answers() {
        assert!(require_stopped(Some(false)).is_ok());
        assert!(require_stopped(Some(true)).unwrap_err().contains("running"));
        assert!(require_stopped(None).unwrap_err().contains("could not tell"));
    }
}
