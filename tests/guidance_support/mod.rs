//! Guidance stdio helpers; historical fixtures delegate to tests/common/legacy.rs.
//!
//! Two things live here.
//! - A **proven legacy fixture**: a format-4 record (no Chain, no approved-plan ledger) written to a
//!   scratch folder and loaded back. In that profile `approved: yes` in a Synthesis Report is the
//!   human's word; in the approved-plan profile Core refuses it, so the guidance tests must say
//!   which contract they measure instead of letting the build profile decide.
//! - A **stdio client** for the freshly built `gil mcp --serve`: every response has a deadline,
//!   the child is killed on timeout/panic/drop, the record goes to an isolated scratch folder
//!   (Windows: a scratch `LOCALAPPDATA`), and no user setting or record is touched.
#![allow(dead_code)]
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gil::{Project, Report};
use serde_json::{Value, json};

// The test file declares `mod common;` once; sharing it keeps `first_world()`'s one-time setup
// single (two copies of that static raced on the same temp folder).
use super::common;

/// Spec and empty world exactly as tests/common provides them.
pub fn spec() -> gil::RuleSet { common::spec() }

/// Write a **format-4** record of a freshly started project into `dir` and prove its shape.
///
/// Whatever the build profile wrote (format 4 or a Chain record), the stored file is rewritten to
/// the legacy container the way `tests/chain_closing.rs` does, then loaded back through the real
/// store. The loaded Project has no Chain and no approved-plan ledger, which is asserted here.
pub fn legacy_record(dir: &Path) -> PathBuf {
    common::legacy::record(dir, common::legacy::Layout::Cycles)
}

pub fn legacy(label: &str) -> Project {
    common::legacy::project(label, common::legacy::Layout::Cycles)
}

pub fn assert_legacy(project: &Project) {
    common::legacy::assert_unplanned(project, common::legacy::Layout::Cycles);
}

/// Interview walked and an Experiment opened — on the **legacy** record only.
pub fn legacy_experiment(label: &str) -> Project {
    common::bootstrap_from(legacy(label))
}

/// The binary Cargo built for this very test run; never PATH or an installed Core.
pub fn gil() -> PathBuf { PathBuf::from(env!("CARGO_BIN_EXE_gil")) }

/// A fresh empty folder under the test temp root (the Wave 2 queue sets `TMPDIR` per command).
pub fn scratch(name: &str) -> PathBuf {
    let at = std::env::temp_dir().join(format!("gil-guidance-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&at);
    std::fs::create_dir_all(&at).expect("scratch dir");
    at
}

pub const RESPONSE_DEADLINE: Duration = Duration::from_secs(30);

pub struct Mcp {
    kid: Child,
    lines: Receiver<std::io::Result<String>>,
    stderr: Arc<Mutex<String>>,
    next: i64,
    pub noise: Vec<String>,
    finished: bool,
}

impl Mcp {
    /// Serve from an isolated record root. `settings` holds Monitor bindings and, on Windows, the
    /// private app-data store, so the real user's `LOCALAPPDATA`/settings are never written.
    pub fn serve(settings: &Path) -> Self {
        let mut command = Command::new(gil());
        command.args(["mcp", "--serve"]).env_clear();
        // Keep only what the child needs to run; everything that names a record location is ours.
        for key in ["HOME", "USERPROFILE", "SYSTEMROOT", "SystemRoot", "WINDIR", "COMSPEC", "PATHEXT", "APPDATA", "PROGRAMDATA", "TEMP", "TMP", "USERNAME"] {
            if let Some(value) = std::env::var_os(key) { command.env(key, value); }
        }
        if cfg!(windows) {
            if let Some(path) = std::env::var_os("PATH") { command.env("PATH", path); }
            let local = settings.join("LocalAppData");
            std::fs::create_dir_all(&local).expect("scratch LOCALAPPDATA");
            command.env("LOCALAPPDATA", &local);
        } else {
            command.env("PATH", "/usr/bin:/bin");
        }
        let mut kid = command
            .env("TMPDIR", std::env::temp_dir())
            .env("GIL_MONITOR_STATE_DIR", settings.join("bindings"))
            .current_dir(std::env::temp_dir())
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
            .spawn().expect("gil mcp --serve");
        let stdout = kid.stdout.take().expect("stdout");
        let (tx, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if tx.send(line).is_err() { break; }
            }
        });
        let stderr = Arc::new(Mutex::new(String::new()));
        let sink = stderr.clone();
        let mut err = kid.stderr.take().expect("stderr");
        std::thread::spawn(move || {
            let mut text = String::new();
            let _ = err.read_to_string(&mut text);
            if let Ok(mut s) = sink.lock() { s.push_str(&text); }
        });
        let mut me = Self { kid, lines, stderr, next: 1, noise: Vec::new(), finished: false };
        let init = me.request("initialize", json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "guidance-test", "version": "0"}}));
        assert!(init.get("error").is_none(), "initialize failed: {init}");
        me.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        me
    }
    fn send(&mut self, frame: &Value) {
        let stdin = self.kid.stdin.as_mut().expect("stdin");
        writeln!(stdin, "{}", serde_json::to_string(frame).unwrap()).expect("write");
        stdin.flush().expect("flush");
    }
    fn fail(&mut self, why: &str) -> ! {
        let _ = self.kid.kill();
        let _ = self.kid.wait();
        self.finished = true;
        let stderr = self.stderr.lock().map(|s| s.clone()).unwrap_or_default();
        panic!("{why}\n--- gil stderr ---\n{stderr}");
    }
    /// One JSON frame within the deadline. Non-JSON lines are recorded as noise, not ignored.
    fn read(&mut self) -> Value {
        let deadline = Instant::now() + RESPONSE_DEADLINE;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.lines.recv_timeout(left) {
                Ok(Ok(line)) => {
                    if line.trim().is_empty() { continue; }
                    match serde_json::from_str::<Value>(&line) {
                        Ok(v) => return v,
                        Err(_) => self.noise.push(line.chars().take(200).collect()),
                    }
                }
                Ok(Err(e)) => self.fail(&format!("stdout read error: {e}")),
                Err(RecvTimeoutError::Timeout) => self.fail(&format!("no JSON-RPC frame within {RESPONSE_DEADLINE:?}")),
                Err(RecvTimeoutError::Disconnected) => self.fail("Core closed stdout before answering"),
            }
        }
    }
    pub fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next; self.next += 1;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let got = self.read();
        if got["id"] != json!(id) { let why = format!("id mismatch: {got}"); self.fail(&why); }
        got
    }
    /// `tools/call` with `project_root` added; the whole frame (result or error).
    pub fn call_frame(&mut self, name: &str, root: &Path, extra: Value) -> Value {
        let mut args = json!({"project_root": root});
        for (k, v) in extra.as_object().unwrap() { args[k] = v.clone(); }
        self.request("tools/call", json!({"name": name, "arguments": args}))
    }
    /// `tools/call` with `project_root` added; the `result` only (panics on an RPC error).
    pub fn call(&mut self, name: &str, root: &Path, extra: Value) -> Value {
        let frame = self.call_frame(name, root, extra);
        if frame.get("error").is_some() { let why = format!("{name}: rpc error {frame}"); self.fail(&why); }
        frame["result"].clone()
    }
    /// A tool call whose arguments are given whole (App tools take `scope_id`, not a root).
    pub fn call_raw(&mut self, name: &str, arguments: Value) -> Value {
        let frame = self.request("tools/call", json!({"name": name, "arguments": arguments}));
        if frame.get("error").is_some() { let why = format!("{name}: rpc error {frame}"); self.fail(&why); }
        frame["result"].clone()
    }
    pub fn tool_names(&mut self) -> Vec<String> {
        let listed = self.request("tools/list", json!({}));
        listed["result"]["tools"].as_array().expect("tools").iter().filter_map(|t| t["name"].as_str().map(str::to_owned)).collect()
    }
    /// Close stdin and expect a clean exit within the deadline.
    pub fn done(mut self) {
        drop(self.kid.stdin.take());
        let deadline = Instant::now() + RESPONSE_DEADLINE;
        loop {
            match self.kid.try_wait() {
                Ok(Some(status)) => {
                    self.finished = true;
                    assert!(status.success(), "gil mcp --serve exit: {status:?}");
                    assert!(self.noise.is_empty(), "non-JSON on stdout: {:?}", self.noise);
                    return;
                }
                Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
                Ok(None) => self.fail("gil mcp --serve did not exit after stdin closed"),
                Err(e) => self.fail(&format!("wait: {e}")),
            }
        }
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.kid.kill();
            let _ = self.kid.wait();
        }
    }
}

/// All text content joined — guidance is prepended to the content list.
pub fn hints(result: &Value) -> String {
    result["content"].as_array().unwrap_or(&Vec::new()).iter().filter_map(|c| c["text"].as_str()).collect::<Vec<_>>().join("\n")
}
pub fn said(result: &Value) -> String {
    result["structuredContent"]["said"].as_str().unwrap_or("").to_string()
}
pub fn is_ok(result: &Value) -> bool {
    result["isError"] != json!(true) && result["structuredContent"]["ok"] != json!(false)
}
pub fn ok(result: Value, what: &str) -> Value {
    assert!(is_ok(&result), "{what} was refused: {result}");
    result
}
pub fn refused(result: Value, what: &str) -> Value {
    assert!(!is_ok(&result), "{what} was accepted: {result}");
    result
}
pub fn report_text(report: &Report) -> String {
    report.field_names().map(|k| format!("{k}: {}\n", report.get(k).unwrap())).collect()
}
pub fn contract(kind: &str) -> Value {
    json!({"kind": kind, "contract": format!("objective: {kind} 를 확인한다\nnext_action: {kind} 자리에서 할 일을 한다\ndone_when: 관측한 결과를 얻는다\n")})
}
/// `열었다: step:C1/S3 · synthesis` → `step:C1/S3`.
pub fn step_ref(result: &Value, kind: &str) -> String {
    let text = said(result);
    let line = text.lines().find(|l| l.starts_with("열었다: step:") && l.contains(&format!(" · {kind}"))).unwrap_or_else(|| panic!("no open receipt for {kind}: {text}"));
    line.trim_start_matches("열었다: ").split(" · ").next().unwrap().to_owned()
}
/// The `scope_id` a successful `gil_start` tells the Host to show.
pub fn scope_from_start(result: &Value) -> String {
    let text = hints(result);
    let at = text.find("show_gil_monitor(").expect("start names show_gil_monitor") + "show_gil_monitor(".len();
    let end = text[at..].find(')').unwrap();
    serde_json::from_str::<Value>(&text[at..at + end]).unwrap()["scope_id"].as_str().unwrap().to_owned()
}
/// First line of `[지금 바로 할 호출]` in a context text.
pub fn first_call(context: &str) -> String {
    let (_, rest) = context.split_once("[지금 바로 할 호출]\n").expect("첫 블록이 없다");
    let line = rest.lines().next().expect("한 줄");
    assert!(!line.trim().is_empty(), "빈 안내:\n{context}");
    line.to_string()
}
/// Synthetic approved-plan draft: `items` ordered as `ids`, Synthesis fields from the spec.
pub fn draft(ids: &[&str], basis: &[String]) -> Value {
    json!({
        "scope": {"goal": "합성 자료 기반 항목별 보고서", "in_scope": "제공된 합성 자료의 비교", "out_of_scope": "실시간 데이터·실제 개인정보", "constraints": "외부 네트워크 없음", "success_criteria": "항목마다 파일 증거가 남는다"},
        "items": ids.iter().map(|id| json!({"id": id, "question": format!("항목 {id}을 합성 자료로 확인한다"), "done_when": format!("evidence 파일에 항목 {id} 결과가 저장된다")})).collect::<Vec<_>>(),
        "synthesis": {"statement": "합성 자료로 항목을 순서대로 확인하고 파일 증거를 남긴다", "basis_refs": basis.join("\n"), "exploration_checklist": ids.join("\n")},
    })
}
