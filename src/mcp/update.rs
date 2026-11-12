//! Update *guidance* for the Monitor: metadata only, never a download or an install.
//!
//! The Monitor App runs inside this Core, so the Core is the only place that can tell the
//! App that a newer GIL exists. The Companion contract (§4) allows a periodic check that reads
//! release metadata and nothing else, and requires a way to turn the notice off. This module:
//!
//! - reads the installed distribution version from where the Core actually lives (the Windows
//!   runtime's `versions/<v>/` directory, or the Plugin's `.codex-plugin/plugin.json` beside
//!   `core/<platform>/`). A development binary has neither and is never checked;
//! - fetches the small repository feed with the OS's own `curl` (the same tool
//!   `runtime_verify` already relies on; no new dependency, no new requirement for users) at
//!   most once a day, in a background thread, caching the result in the Monitor state
//!   directory so a tool call never waits on the network;
//! - compares strict SemVer with pre-release ordering and reports `available`, `up_to_date`,
//!   `unknown_installed`, `checking`, `unavailable` or `disabled`.
//!
//! Turning it off: `GIL_UPDATE_CHECK=off` in the Host's server environment, or a file named
//! `update-check.off` in the Monitor state directory. The payload is informational; the App
//! only shows guidance text and the existing Companion button. Nothing here runs an installer.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const FEED_URL: &str = "https://raw.githubusercontent.com/hyun06000/gil/main/distribution/update-feed.json";
const CACHE_FILE: &str = "update-check.json";
const OFF_FILE: &str = "update-check.off";
const MAX_FEED_BYTES: usize = 65_536;
const FRESH_OK: u64 = 24 * 60 * 60;
const FRESH_FAILED: u64 = 6 * 60 * 60;
const IN_FLIGHT_LIMIT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Latest {
    pub version: String,
    pub release_url: String,
    pub install_doc: String,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Cache {
    schema: u32,
    checked_at: u64,
    feed: Option<Value>,
    error: Option<String>,
}

pub fn platform_key() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Some("darwin-arm64"),
        ("windows", "x86_64") => Some("windows-x64"),
        ("windows", "aarch64") => Some("windows-arm64"),
        _ => None,
    }
}

/// The version of the installed distribution this executable belongs to, or `None` for a
/// development binary. Exact locations only; no search of the whole disk.
pub fn installed_version() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    installed_version_of(&exe)
}

pub(crate) fn installed_version_of(exe: &Path) -> Option<String> {
    if let Some(version) = version_from_install_receipt(exe) {
        return Some(version);
    }
    if let Some(version) = version_from_runtime_path(exe) {
        return Some(version);
    }
    version_from_plugin_manifest(exe)
}

/// `…\Programs\gil\gil.exe` beside the `install.json` that `gil install` writes — the
/// installer-free Windows layout (2026-11-12). Development binaries have no receipt.
fn version_from_install_receipt(exe: &Path) -> Option<String> {
    let version = crate::install::receipt_version(exe.parent()?)?;
    parse_version(&version).map(|_| version)
}

/// `…/GIL/runtime/versions/<version>/gil.exe` — the Windows product layout.
fn version_from_runtime_path(exe: &Path) -> Option<String> {
    let dir = exe.parent()?;
    let versions = dir.parent()?;
    if versions.file_name()?.to_str()? != "versions" {
        return None;
    }
    if versions.parent()?.file_name()?.to_str()? != "runtime" {
        return None;
    }
    let version = dir.file_name()?.to_str()?;
    parse_version(version).map(|_| version.to_owned())
}

/// `…/plugins/<name>/core/<platform>/gil` beside `…/plugins/<name>/.codex-plugin/plugin.json`
/// (Codex) or `.claude-plugin/plugin.json` (Claude Code): both carry the same version.
fn version_from_plugin_manifest(exe: &Path) -> Option<String> {
    let platform_dir = exe.parent()?;
    let core_dir = platform_dir.parent()?;
    if core_dir.file_name()?.to_str()? != "core" {
        return None;
    }
    let plugin_dir = core_dir.parent()?;
    for adapter in [".codex-plugin", ".claude-plugin"] {
        let path = plugin_dir.join(adapter).join("plugin.json");
        let Ok(metadata) = std::fs::metadata(&path) else { continue };
        if !metadata.is_file() || metadata.len() > 64 * 1024 {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else { continue };
        let Ok(value) = serde_json::from_slice::<Value>(&bytes) else { continue };
        if let Some(version) = value.get("version").and_then(Value::as_str) {
            // Development builds carry `+codex.<stamp>` build metadata; ordering ignores it.
            let bare = version.split('+').next().unwrap_or(version);
            if parse_version(bare).is_some() {
                return Some(bare.to_owned());
            }
        }
    }
    None
}

#[derive(Debug, PartialEq, Eq)]
struct Version {
    numbers: [u64; 3],
    pre: Option<Vec<String>>,
}

/// Strict SemVer 2.0 core + pre-release. Build metadata is not accepted here; callers strip it.
pub(crate) fn parse_version(text: &str) -> Option<Version> {
    if text.is_empty() || text.len() > 80 {
        return None;
    }
    let (core, pre) = match text.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (text, None),
    };
    let mut numbers = [0u64; 3];
    let mut parts = core.split('.');
    for slot in numbers.iter_mut() {
        let part = parts.next()?;
        if part.is_empty() || (part.len() > 1 && part.starts_with('0')) || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        *slot = part.parse().ok()?;
    }
    if parts.next().is_some() {
        return None;
    }
    let pre = match pre {
        None => None,
        Some(pre) => {
            let ids: Vec<String> = pre.split('.').map(str::to_owned).collect();
            for id in &ids {
                let numeric = id.bytes().all(|b| b.is_ascii_digit());
                if id.is_empty()
                    || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                    || (numeric && id.len() > 1 && id.starts_with('0'))
                {
                    return None;
                }
            }
            Some(ids)
        }
    };
    Some(Version { numbers, pre })
}

/// SemVer precedence: numbers, then a release outranks any pre-release, then identifiers.
pub(crate) fn compare_versions(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    use std::cmp::Ordering::*;
    let (a, b) = (parse_version(a)?, parse_version(b)?);
    for (x, y) in a.numbers.iter().zip(b.numbers.iter()) {
        if x != y {
            return Some(x.cmp(y));
        }
    }
    match (&a.pre, &b.pre) {
        (None, None) => Some(Equal),
        (None, Some(_)) => Some(Greater),
        (Some(_), None) => Some(Less),
        (Some(x), Some(y)) => {
            for (l, r) in x.iter().zip(y.iter()) {
                if l == r {
                    continue;
                }
                let (ln, rn) = (l.parse::<u64>().ok(), r.parse::<u64>().ok());
                return Some(match (ln, rn) {
                    (Some(l), Some(r)) => l.cmp(&r),
                    (Some(_), None) => Less,
                    (None, Some(_)) => Greater,
                    (None, None) => l.cmp(r),
                });
            }
            Some(x.len().cmp(&y.len()))
        }
    }
}

/// Pull this platform's entry out of a feed document, refusing anything that is not the shape
/// the repository publishes. A feed is remote content: shape-checked, never interpreted further.
pub(crate) fn latest_for(feed: &Value, platform: &str) -> Result<Option<Latest>, String> {
    if feed.get("schema").and_then(Value::as_u64) != Some(1) {
        return Err("feed schema is not 1".into());
    }
    let platforms = feed.get("platforms").and_then(Value::as_object).ok_or("feed has no platforms")?;
    let Some(entry) = platforms.get(platform) else { return Ok(None) };
    let latest: Latest = serde_json::from_value(entry.clone()).map_err(|_| "feed entry malformed".to_owned())?;
    let text = |s: &str| !s.is_empty() && s.len() <= 400;
    if parse_version(&latest.version).is_none()
        || !latest.release_url.starts_with("https://")
        || !latest.install_doc.starts_with("https://")
        || !text(&latest.release_url)
        || !text(&latest.install_doc)
        || latest.notes.len() > 2000
    {
        return Err("feed entry has an invalid version or URL".into());
    }
    Ok(Some(latest))
}

/// Pure decision: what the App should say, given what is known.
pub(crate) fn evaluate(installed: Option<&str>, latest: Option<&Latest>) -> &'static str {
    match (installed, latest) {
        (_, None) => "unavailable",
        (None, Some(_)) => "unknown_installed",
        (Some(have), Some(latest)) => match compare_versions(&latest.version, have) {
            Some(std::cmp::Ordering::Greater) => "available",
            Some(_) => "up_to_date",
            None => "unknown_installed",
        },
    }
}

fn now_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub(crate) fn is_fresh(cache: &Cache, now: u64) -> bool {
    let ttl = if cache.error.is_none() && cache.feed.is_some() { FRESH_OK } else { FRESH_FAILED };
    now.saturating_sub(cache.checked_at) < ttl
}

fn state_dir() -> Option<PathBuf> {
    super::bindings::default_state_dir()
}

pub(crate) fn disabled_in(dir: Option<&Path>) -> bool {
    if std::env::var_os("GIL_UPDATE_CHECK").is_some_and(|v| v.eq_ignore_ascii_case("off") || v == "0") {
        return true;
    }
    dir.is_some_and(|dir| dir.join(OFF_FILE).exists())
}

fn read_cache(dir: &Path) -> Option<Cache> {
    let path = dir.join(CACHE_FILE);
    let metadata = std::fs::metadata(&path).ok()?;
    if !metadata.is_file() || metadata.len() > (MAX_FEED_BYTES as u64) * 2 {
        return None;
    }
    let cache: Cache = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    (cache.schema == 1).then_some(cache)
}

fn write_cache(dir: &Path, cache: &Cache) {
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let tmp = dir.join(format!("{CACHE_FILE}.{}.tmp", std::process::id()));
    let Ok(bytes) = serde_json::to_vec_pretty(cache) else { return };
    if std::fs::write(&tmp, bytes).is_ok() && std::fs::rename(&tmp, dir.join(CACHE_FILE)).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
}

/// `curl` from the OS, HTTPS only, bounded in time and size. Output is the feed bytes.
fn fetch_feed(url: &str) -> Result<Value, String> {
    let mut command = std::process::Command::new(if cfg!(windows) { "curl.exe" } else { "/usr/bin/curl" });
    #[cfg(windows)]
    { use std::os::windows::process::CommandExt; command.creation_flags(0x0800_0000); }
    let output = command
        .args(["-q", "-fsSL", "--proto", "=https", "--proto-redir", "=https", "--max-time", "8", "--max-redirs", "2",
            "--max-filesize", &MAX_FEED_BYTES.to_string(), url])
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|err| format!("curl unavailable: {err}"))?;
    if !output.status.success() {
        return Err(format!("feed request failed ({})", output.status.code().unwrap_or(-1)));
    }
    if output.stdout.len() > MAX_FEED_BYTES {
        return Err("feed too large".into());
    }
    serde_json::from_slice(&output.stdout).map_err(|_| "feed is not JSON".into())
}

fn in_flight() -> &'static Mutex<Option<Instant>> {
    static FLIGHT: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
    FLIGHT.get_or_init(|| Mutex::new(None))
}

fn refresh_in_background(dir: PathBuf) {
    let Ok(mut flight) = in_flight().lock() else { return };
    if flight.is_some_and(|since| since.elapsed() < IN_FLIGHT_LIMIT) {
        return;
    }
    *flight = Some(Instant::now());
    drop(flight);
    std::thread::Builder::new().name("gil-update-check".into()).spawn(move || {
        let result = fetch_feed(FEED_URL);
        let cache = Cache { schema: 1, checked_at: now_unix(),
            feed: result.as_ref().ok().cloned(), error: result.err() };
        write_cache(&dir, &cache);
        if let Ok(mut flight) = in_flight().lock() { *flight = None; }
    }).ok();
}

/// Assemble the App-facing payload from cached metadata only; never blocks on the network.
pub(crate) fn status_with(platform: Option<&str>, installed: Option<&str>, dir: Option<&Path>, now: u64) -> Value {
    let mut out = json!({ "schema": 1, "platform": platform, "installed": installed, "feed": FEED_URL,
        "state": "unavailable", "latest": Value::Null, "checked_at": Value::Null, "error": Value::Null });
    if disabled_in(dir) {
        out["state"] = json!("disabled");
        return out;
    }
    let (Some(platform), Some(dir)) = (platform, dir) else {
        out["error"] = json!("unsupported platform or no state directory");
        return out;
    };
    // A development binary is not an installation; it neither fetches nor reports a version.
    if installed.is_none() {
        out["state"] = json!("not_installed");
        return out;
    }
    let cache = read_cache(dir);
    let fresh = cache.as_ref().is_some_and(|c| is_fresh(c, now));
    if !fresh {
        refresh_in_background(dir.to_owned());
    }
    match cache {
        None => { out["state"] = json!("checking"); }
        Some(cache) => {
            out["checked_at"] = json!(cache.checked_at);
            if let Some(err) = &cache.error { out["error"] = json!(err); }
            match cache.feed.as_ref().map(|feed| latest_for(feed, platform)) {
                Some(Ok(latest)) => {
                    out["state"] = json!(evaluate(installed, latest.as_ref()));
                    if let Some(latest) = latest { out["latest"] = json!(latest); }
                }
                Some(Err(err)) => { out["error"] = json!(err); }
                None => {}
            }
        }
    }
    out
}

pub fn status() -> Value {
    let dir = state_dir();
    status_with(platform_key(), installed_version().as_deref(), dir.as_deref(), now_unix())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering::*;

    fn latest(v: &str) -> Latest {
        Latest { version: v.into(), release_url: "https://example.invalid/r".into(),
            install_doc: "https://example.invalid/i".into(), notes: String::new() }
    }

    #[test]
    fn semver_precedence_follows_the_release_policy() {
        assert_eq!(compare_versions("0.2.1-preview.9", "0.2.1-preview.8"), Some(Greater));
        assert_eq!(compare_versions("0.2.1-preview.10", "0.2.1-preview.9"), Some(Greater));
        assert_eq!(compare_versions("0.2.1", "0.2.1-preview.8"), Some(Greater));
        assert_eq!(compare_versions("0.1.1-owner.7", "0.1.1-owner.7"), Some(Equal));
        assert_eq!(compare_versions("0.1.2-owner.1", "0.1.1-owner.7"), Some(Greater));
        assert_eq!(compare_versions("0.2.1-alpha", "0.2.1-alpha.1"), Some(Less));
        assert_eq!(compare_versions("1.0.0-1", "1.0.0-a"), Some(Less));
        assert_eq!(compare_versions("1.0", "1.0.0"), None);
        assert_eq!(compare_versions("01.0.0", "1.0.0"), None);
        assert_eq!(compare_versions("1.0.0+codex.1", "1.0.0"), None, "build metadata must be stripped by the caller");
    }

    #[test]
    fn evaluation_distinguishes_available_current_and_unknown() {
        assert_eq!(evaluate(Some("0.2.1-preview.8"), Some(&latest("0.2.1-preview.9"))), "available");
        assert_eq!(evaluate(Some("0.2.1-preview.9"), Some(&latest("0.2.1-preview.9"))), "up_to_date");
        assert_eq!(evaluate(Some("0.2.1-preview.10"), Some(&latest("0.2.1-preview.9"))), "up_to_date");
        assert_eq!(evaluate(None, Some(&latest("0.2.1-preview.9"))), "unknown_installed");
        assert_eq!(evaluate(Some("weird"), Some(&latest("0.2.1-preview.9"))), "unknown_installed");
        assert_eq!(evaluate(Some("0.2.1-preview.8"), None), "unavailable");
    }

    #[test]
    fn the_feed_is_shape_checked_per_platform() {
        let feed = json!({"schema": 1, "platforms": {
            "darwin-arm64": {"version": "0.2.1-preview.8", "release_url": "https://x/r", "install_doc": "https://x/i", "notes": "n"},
            "windows-x64": {"version": "0.1.1-owner.7", "release_url": "http://x/r", "install_doc": "https://x/i"}}});
        assert_eq!(latest_for(&feed, "darwin-arm64").unwrap().unwrap().version, "0.2.1-preview.8");
        assert!(latest_for(&feed, "windows-x64").is_err(), "http URL refused");
        assert_eq!(latest_for(&feed, "linux-x64").unwrap(), None);
        assert!(latest_for(&json!({"schema": 2, "platforms": {}}), "darwin-arm64").is_err());
        assert!(latest_for(&json!({"schema": 1}), "darwin-arm64").is_err());
        let actual: Value = serde_json::from_str(include_str!("../../distribution/update-feed.json")).unwrap();
        for platform in ["darwin-arm64", "windows-x64"] {
            assert!(latest_for(&actual, platform).unwrap().is_some(), "repository feed covers {platform}");
        }
    }

    #[test]
    fn installed_version_reads_the_install_receipt_beside_the_executable() {
        let dir = std::env::temp_dir().join(format!("gil-update-receipt-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("gil.exe");
        std::fs::write(&exe, b"x").unwrap();
        assert_eq!(installed_version_of(&exe), None);
        std::fs::write(dir.join("install.json"), r#"{"schema":1,"version":"0.3.0","sha256":"ab"}"#).unwrap();
        assert_eq!(installed_version_of(&exe).as_deref(), Some("0.3.0"));
        std::fs::write(dir.join("install.json"), r#"{"schema":1,"version":"not-semver"}"#).unwrap();
        assert_eq!(installed_version_of(&exe), None, "a receipt that is not SemVer is ignored");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn installed_version_comes_only_from_the_known_layouts() {
        let base = std::env::temp_dir().join(format!("gil-update-{}-{}", std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        let runtime = base.join("GIL/runtime/versions/0.1.1-owner.7");
        std::fs::create_dir_all(&runtime).unwrap();
        assert_eq!(installed_version_of(&runtime.join("gil.exe")).as_deref(), Some("0.1.1-owner.7"));
        assert_eq!(installed_version_of(&base.join("GIL/runtime/versions/not a version/gil.exe")), None);

        let plugin = base.join("plugins/gil-companion-prototype");
        std::fs::create_dir_all(plugin.join("core/darwin-arm64")).unwrap();
        std::fs::create_dir_all(plugin.join(".codex-plugin")).unwrap();
        std::fs::write(plugin.join(".codex-plugin/plugin.json"), br#"{"name":"gil-companion-prototype","version":"0.2.1-preview.8+codex.20261102"}"#).unwrap();
        assert_eq!(installed_version_of(&plugin.join("core/darwin-arm64/gil")).as_deref(), Some("0.2.1-preview.8"));
        std::fs::remove_dir_all(plugin.join(".codex-plugin")).unwrap();
        std::fs::create_dir_all(plugin.join(".claude-plugin")).unwrap();
        std::fs::write(plugin.join(".claude-plugin/plugin.json"), br#"{"version":"0.2.1-preview.9"}"#).unwrap();
        assert_eq!(installed_version_of(&plugin.join("core/darwin-arm64/gil")).as_deref(), Some("0.2.1-preview.9"));

        assert_eq!(installed_version_of(&base.join("target/debug/gil")), None, "a development binary is not installed");
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn status_reads_the_cache_and_never_fetches_for_development_or_disabled_runs() {
        let dir = std::env::temp_dir().join(format!("gil-update-state-{}-{}", std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&dir).unwrap();
        // Development binary: no network, explicit state.
        let dev = status_with(Some("darwin-arm64"), None, Some(&dir), 1_800_000_000);
        assert_eq!(dev["state"], "not_installed");
        assert!(!dir.join(CACHE_FILE).exists());
        // Off switch by file.
        std::fs::write(dir.join(OFF_FILE), b"").unwrap();
        assert_eq!(status_with(Some("darwin-arm64"), Some("0.2.1-preview.8"), Some(&dir), 1_800_000_000)["state"], "disabled");
        std::fs::remove_file(dir.join(OFF_FILE)).unwrap();
        // A fresh cache answers without any fetch.
        let feed = json!({"schema": 1, "platforms": {"darwin-arm64": {"version": "0.2.1-preview.9",
            "release_url": "https://x/r", "install_doc": "https://x/i", "notes": "new"}}});
        write_cache(&dir, &Cache { schema: 1, checked_at: 1_800_000_000, feed: Some(feed), error: None });
        let got = status_with(Some("darwin-arm64"), Some("0.2.1-preview.8"), Some(&dir), 1_800_000_100);
        assert_eq!(got["state"], "available");
        assert_eq!(got["latest"]["version"], "0.2.1-preview.9");
        assert_eq!(got["installed"], "0.2.1-preview.8");
        let same = status_with(Some("darwin-arm64"), Some("0.2.1-preview.9"), Some(&dir), 1_800_000_100);
        assert_eq!(same["state"], "up_to_date");
        // A cached failure is reported, not hidden.
        write_cache(&dir, &Cache { schema: 1, checked_at: 1_800_000_000, feed: None, error: Some("feed request failed (22)".into()) });
        let failed = status_with(Some("windows-x64"), Some("0.1.1-owner.7"), Some(&dir), 1_800_000_100);
        assert_eq!(failed["state"], "unavailable");
        assert_eq!(failed["error"], "feed request failed (22)");
        assert!(is_fresh(&Cache { schema: 1, checked_at: 100, feed: Some(json!({})), error: None }, 100 + FRESH_OK - 1));
        assert!(!is_fresh(&Cache { schema: 1, checked_at: 100, feed: Some(json!({})), error: None }, 100 + FRESH_OK));
        assert!(!is_fresh(&Cache { schema: 1, checked_at: 100, feed: None, error: Some("x".into()) }, 100 + FRESH_FAILED));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
