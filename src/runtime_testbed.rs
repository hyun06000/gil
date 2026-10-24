//! Test-only entry points behind the `runtime-testbed` feature: a **launcher driver** that
//! runs the crate's own launch layer for an explicitly given store (the same code path a
//! product `launch mcp` would use, minus the store-path decision and the trust gate), and a
//! **synthetic MCP Core** that answers MCP over stdio with its version, pid and instance id.
//!
//! Neither is reachable from the product binary, the product manifest or any public command:
//! the feature is off by default and the `gil-runtime-testbed` binary exists only with it. The
//! synthetic Core is not a GIL Core and the driver is not a trust decision about anything.
//!
//! Two ways a synthetic Core learns its version: an explicit `core <version>` argument (the
//! Unix fixtures are `#!/bin/sh` scripts that exec the testbed that way), or an **overlay
//! marker** appended to a copy of the testbed executable itself (the Windows fixtures: a PE
//! loader ignores trailing bytes, so each version is a distinct executable that reads its own
//! tail). With a marker the executable serves as a Core whatever its arguments are — which is
//! exactly how the launcher starts it: `<core> mcp --serve`.
use crate::runtime_launch::{self, FixedArgs, LaunchRefusal, TargetKind};
use std::io::{self, BufRead, Read, Seek, SeekFrom, Write};
use std::path::Path;

pub const OVERLAY_PREFIX: &str = "--gil-synthetic-version:";
pub const OVERLAY_SUFFIX: &str = "--";

/// Bytes to append to a copy of the testbed executable to make it a synthetic Core of `version`.
pub fn overlay_marker(version: &str) -> Vec<u8> { format!("\n{OVERLAY_PREFIX}{version}{OVERLAY_SUFFIX}\n").into_bytes() }

/// The version encoded in this executable's own tail, if any (last 4 KiB are searched).
pub fn version_from_own_exe() -> Option<String> { version_from_file(&std::env::current_exe().ok()?) }

/// The overlay version marker in the last 4 KiB of `path`, if any.
pub fn version_from_file(path: &Path) -> Option<String> {
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let start = len.saturating_sub(4096);
    f.seek(SeekFrom::Start(start)).ok()?;
    let mut tail = Vec::new();
    f.read_to_end(&mut tail).ok()?;
    let text = String::from_utf8_lossy(&tail);
    let i = text.rfind(OVERLAY_PREFIX)?;
    let rest = &text[i + OVERLAY_PREFIX.len()..];
    let j = rest.find(OVERLAY_SUFFIX)?;
    let version = &rest[..j];
    (!version.is_empty() && version.chars().all(|c| c.is_ascii_alphanumeric() || ".+_-".contains(c))).then(|| version.to_owned())
}

/// Decide, verify and run the store's current target with stdio inherited; the child's exit
/// status becomes this process's. Refusals go to stderr and exit 3; a child ended by a signal
/// exits 4; a child whose exit could not be confirmed exits 5.
pub fn launcher_main(root: &Path, bundled: &Path) -> i32 {
    match runtime_launch::launch(root, bundled, FixedArgs::McpServe, None) {
        Ok((kind, report)) => {
            let what = match &kind { TargetKind::Bundled => "bundled".to_owned(), TargetKind::Selected { generation, version, .. } => format!("selected generation {generation} version {version}") };
            eprintln!("gil-runtime-testbed launcher: ran {what} as pid {}; exit {:?}", report.pid, report.exit);
            match report.exit { runtime_launch::Exit::Code(c) => c, runtime_launch::Exit::Signal(_) => 4, runtime_launch::Exit::NotConfirmed { .. } => 5 }
        }
        Err(LaunchRefusal { stage, reason, verdict, spawned, child }) => {
            eprintln!("gil-runtime-testbed launcher: refused at {stage:?}: {reason}; verdict {verdict:?}; spawned {spawned:?}; child {child:?}");
            3
        }
    }
}

/// A minimal MCP server over stdio (newline-delimited JSON-RPC 2.0): `initialize`, `ping`,
/// `tools/list` and `tools/call` for one tool, `gil_synthetic_identity`, which reports the
/// version this process was started as, its pid, a per-process instance id, its executable
/// path and the argument vector it received. Exits 0 on stdin EOF.
pub fn synthetic_core_main(version: &str, argv: &[String]) -> i32 {
    let pid = std::process::id();
    let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default();
    let instance_id = format!("{:016x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0) ^ ((pid as u64) << 32));
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut calls = 0u64;
    for line in stdin.lock().lines() {
        let line = match line { Ok(l) => l, Err(_) => break };
        if line.trim().is_empty() { continue; }
        let msg: serde_json::Value = match serde_json::from_str(&line) { Ok(v) => v, Err(_) => continue };
        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let reply = match method {
            "initialize" => {
                let requested = msg.pointer("/params/protocolVersion").and_then(|v| v.as_str()).unwrap_or("2025-06-18").to_owned();
                serde_json::json!({"protocolVersion": requested, "capabilities": {"tools": {}}, "serverInfo": {"name": "gil-synthetic-core", "version": version}})
            }
            "ping" => serde_json::json!({}),
            "tools/list" => serde_json::json!({"tools": [{"name": "gil_synthetic_identity", "description": format!("Synthetic test Core {version}: reports version, pid and instance id (never a GIL Core)"), "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false}}]}),
            "tools/call" => {
                calls += 1;
                let identity = serde_json::json!({"version": version, "pid": pid, "instance_id": instance_id, "exe": exe, "argv": argv, "call": calls});
                serde_json::json!({"content": [{"type": "text", "text": identity.to_string()}], "structuredContent": identity, "isError": false})
            }
            _ => {
                if id.is_none() { continue; } // notification (e.g. notifications/initialized)
                let err = serde_json::json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": format!("method not found: {method}")}});
                if writeln!(stdout, "{err}").and_then(|_| stdout.flush()).is_err() { break; }
                continue;
            }
        };
        let Some(id) = id else { continue };
        let out = serde_json::json!({"jsonrpc": "2.0", "id": id, "result": reply});
        if writeln!(stdout, "{out}").and_then(|_| stdout.flush()).is_err() { break; }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_overlay_marker_is_found_only_at_the_tail_and_validated() {
        let dir = std::env::temp_dir().join(format!("gil-testbed-overlay-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("exe");
        std::fs::write(&f, b"MZ fake image bytes").unwrap();
        assert_eq!(version_from_file(&f), None, "no marker");
        let mut b = b"MZ fake image bytes".to_vec(); b.extend(overlay_marker("2.0.0-synthetic"));
        std::fs::write(&f, &b).unwrap();
        assert_eq!(version_from_file(&f).as_deref(), Some("2.0.0-synthetic"));
        // a later marker wins; an invalid version is refused; a marker far before the tail is not seen
        let mut c = b.clone(); c.extend(overlay_marker("3.1.0")); std::fs::write(&f, &c).unwrap();
        assert_eq!(version_from_file(&f).as_deref(), Some("3.1.0"));
        let mut d = b"MZ".to_vec(); d.extend(overlay_marker("bad version!")); std::fs::write(&f, &d).unwrap();
        assert_eq!(version_from_file(&f), None);
        let mut e = b"MZ".to_vec(); e.extend(overlay_marker("1.2.3")); e.extend(vec![0u8; 8192]); std::fs::write(&f, &e).unwrap();
        assert_eq!(version_from_file(&f), None, "only the last 4 KiB are searched");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
