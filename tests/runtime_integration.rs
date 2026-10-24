//! Isolated integration of the runtime update layers with a real Codex **App Server** over
//! stdio: an isolated `CODEX_HOME` whose only MCP server is the crate's test-only launcher
//! driver (`gil-runtime-testbed launcher <store> <bundled>`), which runs the crate's own
//! judge → observe → verify → pin → spawn path for a synthetic Core that answers MCP with its
//! version, pid and instance id. Flow: connection on v1 → prepare and select v2 → the existing
//! connection still answers v1 → which later actions produce a v2 process is **recorded, not
//! assumed** (same thread, new thread, `config/mcpServer/reload`, app-server restart) → a new
//! app-server runs v2 with the fixed arguments.
//!
//! Outcomes are explicit: `PASS` (the flow ran and every assertion held), `FAIL`, or
//! `BLOCKED` (no Codex CLI: `GIL_CODEX_CLI`, or the ChatGPT-bundled path on macOS). BLOCKED
//! returns without failing only when `GIL_RUNTIME_INTEGRATION_REQUIRE` is unset; a required
//! run (CI) treats BLOCKED as a failure, so an absent CLI can never count as a pass. Whatever
//! happens mid-flow, the harness ends the app-servers **it holds a process handle for**, waits
//! for the synthetic Cores it learned about (grandchildren it has no handle for: never ended by
//! pid or image name — a Core still alive is recorded `Unconfirmed` and fails the run), records
//! how each ended (natural / forced / unconfirmed) and saves the evidence file; a failed save is
//! reported on stderr with the evidence dumped to stdout and never shown as PASS. Nothing under
//! the real `~/.codex`, GIL settings or projects is touched.
#![cfg(feature = "runtime-testbed")]
use gil::runtime_observe::observe_and_judge;
use gil::runtime_prepare::{self as prep, Request};
use gil::runtime_select::{self as sel, SelectRequest};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::{Duration, Instant};

const TESTBED: &str = env!("CARGO_BIN_EXE_gil-runtime-testbed");
/// The testbed executable: `GIL_RUNTIME_TESTBED` when set (a prebuilt bundle run outside the
/// build tree), else the one Cargo built next to this test. Must exist as a file.
fn testbed() -> String {
    let p = std::env::var_os("GIL_RUNTIME_TESTBED").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(TESTBED));
    assert!(p.is_file(), "testbed executable missing: {}", p.display());
    p.display().to_string()
}
#[cfg(target_os = "macos")]
const DEFAULT_CLI: Option<&str> = Some("/Applications/ChatGPT.app/Contents/Resources/codex-cli/bin/codex");
#[cfg(not(target_os = "macos"))]
const DEFAULT_CLI: Option<&str> = None;
const SERVER: &str = "gil_synthetic";
const TOOL: &str = "gil_synthetic_identity";
const V1: &str = "1.0.0-synthetic";
const V2: &str = "2.0.0-synthetic";

fn sha(b: &[u8]) -> String { format!("{:x}", Sha256::digest(b)) }

/// Synthetic Core bytes for a version: Unix a script that execs the testbed; Windows a copy of
/// the testbed executable with the overlay version marker (a distinct executable per version).
#[cfg(unix)]
fn core_bytes(version: &str) -> Vec<u8> { format!("#!/bin/sh\nexec '{}' core {version} \"$@\"\n", testbed()).into_bytes() }
#[cfg(windows)]
fn core_bytes(version: &str) -> Vec<u8> { let mut b = fs::read(testbed()).expect("testbed exe"); b.extend_from_slice(&gil::runtime_testbed::overlay_marker(version)); b }
fn make_executable(p: &Path) {
    #[cfg(unix)]
    { use std::os::unix::fs::PermissionsExt; fs::set_permissions(p, fs::Permissions::from_mode(0o500)).unwrap(); }
    #[cfg(windows)]
    { let _ = p; }
}
fn toml_literal(p: &Path) -> String { format!("'{}'", p.display()) }

#[derive(Debug, Clone)]
struct Identity { version: String, pid: u32, instance: String, exe: String, argv: Vec<String> }
impl Identity {
    fn parse(v: &Value) -> Identity {
        let s = v.get("structuredContent").cloned().unwrap_or_else(|| { let text = v["content"][0]["text"].as_str().unwrap_or("{}"); serde_json::from_str(text).unwrap_or(Value::Null) });
        Identity { version: s["version"].as_str().unwrap_or("?").into(), pid: s["pid"].as_u64().unwrap_or(0) as u32, instance: s["instance_id"].as_str().unwrap_or("?").into(), exe: s["exe"].as_str().unwrap_or("?").into(), argv: s["argv"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default() }
    }
    fn brief(&self) -> String { format!("version={} pid={} instance={} exe={} argv={:?}", self.version, self.pid, self.instance, self.exe, self.argv) }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum How { Natural, Forced, Unconfirmed }
#[derive(Debug, Clone)]
struct Termination { kind: &'static str, pid: u32, how: How, detail: String }

/// A stdio app-server driven over newline-delimited JSON-RPC.
struct AppServer { child: Child, lines: Receiver<String>, next_id: u64, notifications: Vec<Value>, label: &'static str }
impl AppServer {
    fn start(cli: &Path, codex_home: &Path, label: &'static str) -> Result<AppServer, String> {
        let mut cmd = Command::new(cli);
        cmd.args(["app-server", "--stdio"]).env_clear().env("CODEX_HOME", codex_home).env("TMPDIR", std::env::temp_dir()).env("TEMP", std::env::temp_dir()).env("TMP", std::env::temp_dir());
        for key in ["HOME", "PATH", "USERPROFILE", "SystemRoot", "SystemDrive", "APPDATA", "LOCALAPPDATA", "ProgramData", "COMSPEC", "USERNAME"] { if let Some(v) = std::env::var_os(key) { cmd.env(key, v); } }
        let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| format!("app-server spawn: {e}"))?;
        let (tx, lines) = channel();
        let out = child.stdout.take().unwrap();
        std::thread::spawn(move || { for l in BufReader::new(out).lines() { match l { Ok(l) => { if tx.send(l).is_err() { break; } } Err(_) => break } } });
        let err = child.stderr.take().unwrap();
        std::thread::spawn(move || { for l in BufReader::new(err).lines().map_while(Result::ok) { eprintln!("[app-server {label} stderr] {l}"); } });
        Ok(AppServer { child, lines, next_id: 1, notifications: Vec::new(), label })
    }
    fn pid(&self) -> u32 { self.child.id() }
    fn request(&mut self, method: &str, params: Value, timeout: Duration) -> Result<Value, String> {
        let id = self.next_id; self.next_id += 1;
        let msg = if params.is_null() { json!({"jsonrpc": "2.0", "id": id, "method": method}) } else { json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}) };
        let stdin = self.child.stdin.as_mut().ok_or("stdin closed")?;
        writeln!(stdin, "{msg}").and_then(|_| stdin.flush()).map_err(|e| format!("write: {e}"))?;
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() { return Err(format!("{method}: no response within {timeout:?}")); }
            match self.lines.recv_timeout(left) {
                Ok(line) => {
                    let v: Value = match serde_json::from_str(&line) { Ok(v) => v, Err(_) => continue };
                    if v.get("id").and_then(|i| i.as_u64()) == Some(id) {
                        if let Some(e) = v.get("error") { return Err(format!("{method}: error {e}")); }
                        return Ok(v.get("result").cloned().unwrap_or(Value::Null));
                    }
                    if v.get("method").is_some() { self.notifications.push(v); }
                }
                Err(RecvTimeoutError::Timeout) => return Err(format!("{method}: no response within {timeout:?}")),
                Err(RecvTimeoutError::Disconnected) => return Err(format!("{method}: app-server stdout closed")),
            }
        }
    }
    /// stdin EOF, then wait up to 20 s; a kill afterwards is recorded as forced, never natural.
    fn close(mut self) -> Termination {
        let pid = self.child.id();
        drop(self.child.stdin.take());
        let started = Instant::now();
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => return Termination { kind: "app-server", pid, how: How::Natural, detail: format!("{}: exited {status} after stdin EOF in {:?}", self.label, started.elapsed()) },
                Ok(None) if started.elapsed() > Duration::from_secs(20) => {
                    let killed = self.child.kill();
                    let waited = self.child.wait();
                    return Termination { kind: "app-server", pid, how: if waited.is_ok() { How::Forced } else { How::Unconfirmed }, detail: format!("{}: no exit within 20 s after stdin EOF; kill {killed:?}, wait {waited:?}", self.label) };
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                Err(e) => return Termination { kind: "app-server", pid, how: How::Unconfirmed, detail: format!("{}: wait error {e}", self.label) },
            }
        }
    }
}

// ── liveness of a pid we learned about (never a reason to end it) ────────────────────────
#[derive(Debug, Clone, PartialEq, Eq)]
enum Liveness { Alive, Gone, Unknown(String) }
#[cfg(unix)]
fn liveness(pid: u32) -> Liveness {
    if unsafe { libc::kill(pid as i32, 0) } == 0 { return Liveness::Alive; }
    liveness_from_errno(std::io::Error::last_os_error().raw_os_error())
}
#[cfg(unix)]
fn liveness_from_errno(errno: Option<i32>) -> Liveness {
    match errno { Some(libc::ESRCH) => Liveness::Gone, Some(libc::EPERM) => Liveness::Alive, other => Liveness::Unknown(format!("kill(pid, 0) failed with errno {other:?}")) }
}
#[cfg(windows)]
fn liveness(pid: u32) -> Liveness {
    let out = Command::new(PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_default()).join("System32").join("tasklist.exe")).args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"]).output();
    liveness_from_tasklist(out.map(|o| (o.status.success(), String::from_utf8_lossy(&o.stdout).into_owned())).map_err(|e| e.to_string()))
}
/// `tasklist` answers: a CSV row = alive; its "no tasks" notice = gone; anything else (command
/// failure, access denied, unparsable text) = unknown.
#[cfg(windows)]
fn liveness_from_tasklist(out: Result<(bool, String), String>) -> Liveness {
    match out {
        Err(e) => Liveness::Unknown(format!("tasklist could not run: {e}")),
        Ok((false, text)) => Liveness::Unknown(format!("tasklist failed: {}", text.trim())),
        Ok((true, text)) => {
            let text = text.trim();
            if text.starts_with('"') && text.matches(',').count() >= 4 { Liveness::Alive }
            else if text.to_ascii_lowercase().contains("no tasks") { Liveness::Gone }
            else { Liveness::Unknown(format!("tasklist output not understood: {text}")) }
        }
    }
}
/// Poll until gone or the limit; the last observation is returned (Unknown is not Gone).
fn wait_until_gone(pid: u32, limit: Duration) -> Liveness {
    let deadline = Instant::now() + limit;
    loop {
        let now = liveness(pid);
        if now == Liveness::Gone || Instant::now() >= deadline { return now; }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Everything the test created, with cleanup and evidence on drop (also on a panic).
struct Harness {
    cli: PathBuf, base: PathBuf, codex_home: PathBuf, root: PathBuf,
    log: Vec<String>, servers: Vec<AppServer>, cores: Vec<Identity>, terminations: Vec<Termination>,
    outcome: &'static str, error: Option<String>, evidence_dir: PathBuf, saved: Option<Result<PathBuf, String>>,
}
impl Harness {
    fn note(&mut self, s: String) { println!("OBSERVED {s}"); self.log.push(s); }
    fn start_server(&mut self, label: &'static str) -> Result<usize, String> {
        let s = AppServer::start(&self.cli, &self.codex_home, label)?;
        self.note(format!("app-server {label} started pid {}", s.pid()));
        self.servers.push(s);
        Ok(self.servers.len() - 1)
    }
    fn call(&mut self, server: usize, thread: &str) -> Result<Identity, String> {
        let v = self.servers[server].request("mcpServer/tool/call", json!({"server": SERVER, "threadId": thread, "tool": TOOL, "arguments": {}}), Duration::from_secs(60))?;
        let id = Identity::parse(&v);
        if id.pid > 0 && !self.cores.iter().any(|c| c.pid == id.pid) { self.cores.push(id.clone()); }
        Ok(id)
    }
    fn close_server(&mut self, server: usize) {
        if server < self.servers.len() {
            let s = self.servers.remove(server);
            let t = s.close();
            self.note(format!("{} pid {} -> {:?}: {}", t.kind, t.pid, t.how, t.detail));
            self.terminations.push(t);
        }
    }
    /// Idempotent. App-servers are ended through the handles this test holds. Synthetic Cores
    /// are grandchildren (spawned by the app-server through the launcher): no handle, so they
    /// are only waited for — never ended by pid or image name — and a Core still alive, or one
    /// whose state cannot be queried, is `Unconfirmed` (which fails the run).
    fn cleanup(&mut self) {
        while !self.servers.is_empty() { self.close_server(0); }
        let cores = std::mem::take(&mut self.cores);
        for c in cores {
            let t = match wait_until_gone(c.pid, Duration::from_secs(15)) {
                Liveness::Gone => Termination { kind: "synthetic-core", pid: c.pid, how: How::Natural, detail: format!("{} gone within 15 s of its app-server's end", c.version) },
                Liveness::Alive => Termination { kind: "synthetic-core", pid: c.pid, how: How::Unconfirmed, detail: format!("{} still alive 15 s after its app-server's end; not ended by this test (no process handle: pid/image name are not ownership)", c.version) },
                Liveness::Unknown(why) => Termination { kind: "synthetic-core", pid: c.pid, how: How::Unconfirmed, detail: format!("{} state unknown: {why}; not ended by this test", c.version) },
            };
            self.note(format!("{} pid {} -> {:?}: {}", t.kind, t.pid, t.how, t.detail));
            self.terminations.push(t);
        }
    }
    fn evidence(&self) -> Value {
        let terminations: Vec<Value> = self.terminations.iter().map(|t| json!({"kind": t.kind, "pid": t.pid, "how": format!("{:?}", t.how), "detail": t.detail})).collect();
        json!({"outcome": self.outcome, "error": self.error, "cli": self.cli.display().to_string(), "codex_home": self.codex_home.display().to_string(), "store": self.root.display().to_string(), "observed": self.log, "terminations": terminations, "verdict_after": if self.root.exists() { format!("{:?}", observe_and_judge(&self.root).1.state) } else { "no store".into() }})
    }
    /// Save the evidence once. A failure is returned and printed (stderr: the error; stdout: the
    /// evidence itself) and never turned into "written" or PASS.
    fn save_evidence(&mut self) -> Result<PathBuf, String> {
        if let Some(done) = &self.saved { return done.clone(); }
        let evidence = self.evidence();
        let result = (|| -> Result<PathBuf, String> {
            fs::create_dir_all(&self.evidence_dir).map_err(|e| format!("create_dir_all {}: {e}", self.evidence_dir.display()))?;
            let path = self.evidence_dir.join(format!("run-{}-{}.json", std::process::id(), self.outcome));
            fs::write(&path, serde_json::to_string_pretty(&evidence).unwrap()).map_err(|e| format!("write {}: {e}", path.display()))?;
            Ok(path)
        })();
        match &result {
            Ok(path) => { println!("OUTCOME: {}", self.outcome); println!("OBSERVED evidence written to {}", path.display()); }
            Err(e) => { eprintln!("EVIDENCE NOT WRITTEN: {e}"); println!("OUTCOME: {} (evidence not saved: {e})", if self.outcome == "PASS" { "FAIL" } else { self.outcome }); println!("EVIDENCE (not saved) {}", evidence); }
        }
        self.saved = Some(result.clone());
        result
    }
}
impl Drop for Harness {
    fn drop(&mut self) {
        if self.outcome == "RUNNING" { self.outcome = "FAIL"; if self.error.is_none() { self.error = Some("flow ended without a verdict (panic or early exit)".into()); } }
        self.cleanup();
        let _ = self.save_evidence(); // reported inside; no panic from a destructor
        let _ = fs::remove_dir_all(&self.base);
    }
}
fn evidence_dir() -> PathBuf { std::env::var_os("GIL_RUNTIME_INTEGRATION_EVIDENCE_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target").join("runtime-integration-evidence")) }
fn new_harness(cli: PathBuf, base: PathBuf) -> Harness {
    let codex_home = base.join("codex-home"); fs::create_dir_all(&codex_home).unwrap();
    Harness { cli, root: base.join("runtime-v1"), codex_home, base, log: vec![], servers: vec![], cores: vec![], terminations: vec![], outcome: "RUNNING", error: None, evidence_dir: evidence_dir(), saved: None }
}
fn fresh_base(label: &str) -> PathBuf {
    let base = fs::canonicalize(std::env::temp_dir()).unwrap().join(format!("gil-runtime-integration-{label}-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&base).unwrap();
    #[cfg(unix)]
    { use std::os::unix::fs::PermissionsExt; fs::set_permissions(&base, fs::Permissions::from_mode(0o700)).unwrap(); }
    base
}

fn thread_start(h: &mut Harness, server: usize) -> Result<String, String> {
    let cwd = h.base.to_str().unwrap().to_owned();
    Ok(h.servers[server].request("thread/start", json!({"cwd": cwd, "ephemeral": true}), Duration::from_secs(30))?["thread"]["id"].as_str().ok_or("thread id")?.to_owned())
}

/// The flow; every failure is an `Err` (or a panic) and the harness still cleans up.
fn flow(h: &mut Harness) -> Result<(), String> {
    let root = h.root.clone(); let base = h.base.clone();
    let bundled = base.join(if cfg!(windows) { "bundled-core.exe" } else { "bundled-core" });
    fs::write(&bundled, core_bytes("bundled")).unwrap(); make_executable(&bundled);
    fs::write(h.codex_home.join("config.toml"), format!("# isolated runtime-integration home; the only MCP server is the test-only launcher driver\n[mcp_servers.{SERVER}]\ncommand = {}\nargs = [\"launcher\", {}, {}]\n", toml_literal(Path::new(&testbed())), toml_literal(&root), toml_literal(&bundled))).unwrap();
    h.note(format!("isolated CODEX_HOME {} ; store {} ; testbed {}", h.codex_home.display(), root.display(), testbed()));
    // v1 through the crate's own layers
    let v1 = core_bytes(V1); let src1 = base.join("src-v1"); fs::write(&src1, &v1).unwrap();
    prep::prepare(&root, &Request { version: V1, expected_sha256: &sha(&v1), source: &src1 }).map_err(|e| format!("prepare v1: {e:?}"))?;
    if sel::select(&root, &SelectRequest { version: V1, sha256: &sha(&v1), expected_generation: 0 }).map_err(|e| format!("select v1: {e:?}"))?.generation != 1 { return Err("select v1 generation".into()); }
    h.note("store: v1 prepared and selected (generation 1)".into());
    // app-server A, thread T1
    let a = h.start_server("A")?;
    let init = h.servers[a].request("initialize", json!({"clientInfo": {"name": "gil-runtime-integration", "version": "0"}}), Duration::from_secs(30))?;
    h.note(format!("initialize: codexHome={} userAgent={}", init["codexHome"], init["userAgent"]));
    // The same directory, not the same spelling: Windows reports the home without the `\\?\`
    // verbatim prefix that our canonicalized temp path carries, so both sides are canonicalized.
    let reported = init["codexHome"].as_str().map(PathBuf::from).ok_or_else(|| format!("initialize returned no codexHome: {init}"))?;
    let reported_id = fs::canonicalize(&reported).map_err(|e| format!("canonicalize reported codexHome {}: {e}", reported.display()))?;
    let isolated_id = fs::canonicalize(&h.codex_home).map_err(|e| format!("canonicalize isolated codex_home {}: {e}", h.codex_home.display()))?;
    if reported_id != isolated_id { return Err(format!("the app-server must run on the isolated home {}, got {}", h.codex_home.display(), reported.display())); }
    let t1 = thread_start(h, a)?;
    let status = h.servers[a].request("mcpServerStatus/list", json!({"threadId": t1, "serverName": SERVER}), Duration::from_secs(60))?;
    let entry = status["data"].as_array().and_then(|d| d.iter().find(|s| s["name"] == SERVER)).cloned().unwrap_or(Value::Null);
    h.note(format!("T1 {t1}: runtimeStatus={} serverInfo={} tools={:?} toolsError={}", entry["runtimeStatus"], entry["serverInfo"], entry["tools"].as_object().map(|t| t.keys().cloned().collect::<Vec<_>>()), entry["toolsError"]));
    if entry["serverInfo"]["name"] != "gil-synthetic-core" { return Err(format!("the launcher must have started the synthetic Core: {entry}")); }
    let id1 = h.call(a, &t1)?;
    h.note(format!("T1 call #1: {}", id1.brief()));
    if id1.version != V1 || id1.pid == 0 { return Err(format!("first connection must run v1: {}", id1.brief())); }
    if id1.argv != ["mcp", "--serve"] { return Err(format!("fixed arguments must reach the Core: {:?}", id1.argv)); }
    if std::env::var_os("GIL_RUNTIME_INTEGRATION_INJECT_FAIL").is_some() { return Err("injected mid-flow failure (harness cleanup check)".into()); }
    // v2 while T1 lives
    let v2 = core_bytes(V2); let src2 = base.join("src-v2"); fs::write(&src2, &v2).unwrap();
    prep::prepare(&root, &Request { version: V2, expected_sha256: &sha(&v2), source: &src2 }).map_err(|e| format!("prepare v2: {e:?}"))?;
    if sel::select(&root, &SelectRequest { version: V2, sha256: &sha(&v2), expected_generation: 1 }).map_err(|e| format!("select v2: {e:?}"))?.generation != 2 { return Err("select v2 generation".into()); }
    h.note("store: v2 prepared and selected (generation 2) while T1 is connected".into());
    let id2 = h.call(a, &t1)?;
    h.note(format!("T1 call #2 after v2 selection: {}", id2.brief()));
    if id2.version != V1 || id2.pid != id1.pid || id2.instance != id1.instance { return Err(format!("the existing connection must keep the same v1 process: {} vs {}", id2.brief(), id1.brief())); }
    // recorded, not assumed
    let t2 = thread_start(h, a)?;
    let id3 = h.call(a, &t2)?;
    h.note(format!("T2 {t2} (new thread, same app-server): {} -> {}", id3.brief(), if id3.pid == id1.pid { "SAME process as T1" } else if id3.version == V2 { "NEW process running v2" } else { "new process, still v1" }));
    let reload = h.servers[a].request("config/mcpServer/reload", Value::Null, Duration::from_secs(60));
    h.note(format!("config/mcpServer/reload -> {reload:?}"));
    let id4 = h.call(a, &t1);
    h.note(format!("T1 call after reload: {}", match &id4 { Ok(i) => format!("{} -> {}", i.brief(), if i.pid == id1.pid { "SAME v1 process" } else { "DIFFERENT process" }), Err(e) => format!("error: {e}") }));
    let t3 = thread_start(h, a)?;
    let id5 = h.call(a, &t3);
    h.note(format!("T3 {t3} (new thread after reload): {}", match &id5 { Ok(i) => format!("{} -> {}", i.brief(), if i.pid == id1.pid { "SAME v1 process" } else if i.version == V2 { "NEW v2 process" } else { "new process, v1" }), Err(e) => format!("error: {e}") }));
    let notes: Vec<String> = h.servers[a].notifications.iter().filter(|n| n["method"] == "mcpServerStatus/updated").map(|n| n["params"].to_string()).collect();
    for n in notes { h.note(format!("notification A: {n}")); }
    h.close_server(a);
    // restart → v2 with the fixed arguments
    let b = h.start_server("B")?;
    h.servers[b].request("initialize", json!({"clientInfo": {"name": "gil-runtime-integration", "version": "0"}}), Duration::from_secs(30))?;
    let t4 = thread_start(h, b)?;
    let id6 = h.call(b, &t4)?;
    h.note(format!("B/T4 {t4} (app-server restart): {}", id6.brief()));
    if id6.version != V2 || id6.pid == id1.pid { return Err(format!("a new app-server's connection must run the selected v2 in a new process: {}", id6.brief())); }
    if id6.argv != ["mcp", "--serve"] { return Err(format!("fixed arguments must reach the v2 Core: {:?}", id6.argv)); }
    h.close_server(b);
    Ok(())
}

#[test]
fn existing_connection_keeps_v1_while_v2_is_selected_and_the_origin_of_v2_processes_is_recorded() {
    let cli = std::env::var_os("GIL_CODEX_CLI").map(PathBuf::from).or_else(|| DEFAULT_CLI.map(PathBuf::from));
    let required = std::env::var_os("GIL_RUNTIME_INTEGRATION_REQUIRE").is_some();
    let mut h = new_harness(cli.clone().unwrap_or_default(), fresh_base("flow"));
    match &cli {
        Some(c) if c.is_file() => {}
        other => {
            h.outcome = "BLOCKED";
            h.error = Some(format!("Codex CLI not found ({other:?}); set GIL_CODEX_CLI. The App Server integration did not run."));
            let msg = h.error.clone().unwrap();
            drop(h);
            assert!(!required, "BLOCKED must not pass in a required run: {msg}");
            return;
        }
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| flow(&mut h)));
    let verdict = match result { Ok(Ok(())) => Ok(()), Ok(Err(e)) => Err(e), Err(p) => Err(format!("panic in flow: {}", p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default())) };
    h.outcome = if verdict.is_ok() { "PASS" } else { "FAIL" };
    h.error = verdict.clone().err();
    h.cleanup();
    let forced: Vec<String> = h.terminations.iter().filter(|t| t.how != How::Natural).map(|t| format!("{} pid {} {:?}: {}", t.kind, t.pid, t.how, t.detail)).collect();
    if verdict.is_ok() && !forced.is_empty() { h.outcome = "FAIL"; h.error = Some(format!("flow passed but termination was not natural for: {forced:?}")); }
    let saved = h.save_evidence();
    let outcome = h.outcome; let error = h.error.clone();
    drop(h);
    assert!(saved.is_ok(), "evidence must be saved: {saved:?}");
    assert_eq!(outcome, "PASS", "{error:?}");
}

// ── regression tests for the harness itself (no app-server, no unrelated process is ended) ──
#[cfg(unix)]
#[test]
fn liveness_distinguishes_alive_gone_and_unknown() {
    assert_eq!(liveness(std::process::id()), Liveness::Alive);
    assert_eq!(liveness_from_errno(Some(libc::ESRCH)), Liveness::Gone);
    assert_eq!(liveness_from_errno(Some(libc::EPERM)), Liveness::Alive, "a process we may not signal still exists");
    assert!(matches!(liveness_from_errno(Some(libc::EINVAL)), Liveness::Unknown(_)), "a query failure is not Gone");
    assert!(matches!(liveness_from_errno(None), Liveness::Unknown(_)));
}
#[cfg(windows)]
#[test]
fn liveness_distinguishes_alive_gone_and_unknown() {
    assert_eq!(liveness(std::process::id()), Liveness::Alive);
    assert_eq!(liveness_from_tasklist(Ok((true, "\"gil.exe\",\"1234\",\"Console\",\"1\",\"8,000 K\"".into()))), Liveness::Alive);
    assert_eq!(liveness_from_tasklist(Ok((true, "INFO: No tasks are running which match the specified criteria.".into()))), Liveness::Gone);
    assert!(matches!(liveness_from_tasklist(Ok((false, "ERROR: Access denied".into()))), Liveness::Unknown(_)), "a failed query is not Gone");
    assert!(matches!(liveness_from_tasklist(Ok((true, "garbage".into()))), Liveness::Unknown(_)));
    assert!(matches!(liveness_from_tasklist(Err("spawn failed".into())), Liveness::Unknown(_)));
}
#[test]
fn a_core_the_test_does_not_hold_a_handle_for_is_never_ended_even_with_the_same_image_name() {
    // A process this test *does* own, running the very same image a synthetic Core would (the
    // testbed in core mode), stands in for a stranger with the same image name: the harness
    // learns its pid as if it were a Core, finds it alive after the wait, records Unconfirmed
    // and leaves it running. The test ends it afterwards through its own handle.
    let mut stranger = Command::new(testbed()).args(["core", "stranger"]).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().expect("spawn testbed core");
    let pid = stranger.id();
    let mut h = new_harness(PathBuf::new(), fresh_base("stranger"));
    h.cores.push(Identity { version: "stranger".into(), pid, instance: "-".into(), exe: testbed(), argv: vec![] });
    let started = Instant::now();
    h.cleanup();
    assert!(started.elapsed() >= Duration::from_secs(15), "the harness must have waited the full window");
    let t = h.terminations.iter().find(|t| t.pid == pid).expect("recorded");
    assert_eq!(t.how, How::Unconfirmed, "{t:?}");
    assert!(t.detail.contains("not ended by this test"), "{t:?}");
    assert_eq!(liveness(pid), Liveness::Alive, "the look-alike process must still be running");
    h.outcome = "FAIL"; h.error = Some("harness regression: look-alike process".into());
    let _ = h.save_evidence(); drop(h);
    drop(stranger.stdin.take()); // EOF: our own child exits by itself
    let status = stranger.wait().unwrap();
    assert!(status.success() || status.code().is_some(), "{status:?}");
}
#[test]
fn an_unsaveable_evidence_location_is_reported_and_never_shown_as_pass() {
    let base = fresh_base("evidence");
    let blocker = base.join("not-a-directory"); fs::write(&blocker, b"file in the way").unwrap();
    let mut h = new_harness(PathBuf::new(), base.clone());
    h.evidence_dir = blocker.join("evidence"); // create_dir_all must fail: a parent is a file
    h.outcome = "PASS"; h.log.push("harness regression: evidence save failure".into());
    let saved = h.save_evidence();
    assert!(saved.is_err(), "{saved:?}");
    assert!(saved.as_ref().unwrap_err().contains("create_dir_all"), "{saved:?}");
    assert!(matches!(&h.saved, Some(Err(_))));
    // a second call returns the same failure (no retry loop, no panic in drop)
    assert_eq!(h.save_evidence(), saved);
    drop(h);
    assert!(!blocker.join("evidence").exists());
}
