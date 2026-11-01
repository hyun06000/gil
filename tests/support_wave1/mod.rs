//! Wave 1 E support: a minimal stdio JSON-RPC client around the freshly built `gil` binary.
//! Dedicated to tests/approved_plan_wire_wave1.rs; the shared tests/common is read-only for E.
#![allow(dead_code)]
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// The binary Cargo built for this very test run; never PATH or an installed Core.
pub fn gil() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_gil"))
}

pub fn scratch(name: &str) -> PathBuf {
    let at = std::env::temp_dir().join(format!(
        "gil-wave1-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&at);
    std::fs::create_dir_all(&at).expect("scratch dir");
    at
}

pub struct Client {
    kid: Child,
    out: BufReader<std::process::ChildStdout>,
    next: i64,
    pub noise: Vec<String>,
}

impl Client {
    pub fn serve(settings: &Path) -> Self {
        let mut kid = Command::new(gil())
            .args(["mcp", "--serve"])
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", std::env::var_os("HOME").unwrap_or_default())
            .env("TMPDIR", std::env::temp_dir())
            .env("GIL_MONITOR_STATE_DIR", settings.join("bindings"))
            .current_dir(std::env::temp_dir())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("gil mcp --serve");
        let out = BufReader::new(kid.stdout.take().expect("stdout"));
        let mut c = Self { kid, out, next: 1, noise: Vec::new() };
        let init = c.request("initialize", json!({"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"wave1-wire","version":"0"}}));
        assert!(init.get("error").is_none(), "initialize failed: {init}");
        c.send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
        c
    }
    fn send(&mut self, frame: &Value) {
        let stdin = self.kid.stdin.as_mut().expect("stdin");
        writeln!(stdin, "{}", serde_json::to_string(frame).unwrap()).expect("write");
        stdin.flush().expect("flush");
    }
    /// One response frame. Non-JSON stdout lines are collected as noise (a defect) instead of panicking.
    fn read(&mut self) -> Value {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            assert!(Instant::now() < deadline, "timeout waiting for a JSON-RPC frame");
            let mut line = String::new();
            let n = self.out.read_line(&mut line).expect("read");
            assert!(n > 0, "Core closed stdout before answering");
            if line.trim().is_empty() { continue; }
            match serde_json::from_str::<Value>(&line) {
                Ok(v) => return v,
                Err(_) => self.noise.push(line.chars().take(200).collect()),
            }
        }
    }
    pub fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next;
        self.next += 1;
        self.send(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}));
        let got = self.read();
        assert_eq!(got["id"], json!(id), "id mismatch: {got}");
        got
    }
    /// tools/call → whole frame (result or error).
    pub fn call(&mut self, name: &str, arguments: Value) -> Value {
        self.request("tools/call", json!({"name":name,"arguments":arguments}))
    }
    pub fn eof(mut self) -> (std::process::ExitStatus, Vec<String>) {
        drop(self.kid.stdin.take());
        let mut rest = String::new();
        use std::io::Read as _;
        self.out.read_to_string(&mut rest).ok();
        for line in rest.lines().filter(|l| !l.trim().is_empty()) {
            if serde_json::from_str::<Value>(line).is_err() { self.noise.push(line.chars().take(200).collect()); }
        }
        let status = self.kid.wait().expect("exit");
        (status, std::mem::take(&mut self.noise))
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.kid.kill();
        let _ = self.kid.wait();
    }
}

pub fn is_ok(frame: &Value) -> bool {
    frame.get("error").is_none()
        && frame["result"]["isError"] != json!(true)
        && frame["result"]["structuredContent"]["ok"] != json!(false)
}
pub fn structured(frame: &Value) -> &Value { &frame["result"]["structuredContent"] }
pub fn said(frame: &Value) -> String {
    if let Some(s) = frame["result"]["structuredContent"]["said"].as_str().filter(|s| !s.is_empty()) { return s.to_owned(); }
    frame["result"]["content"].as_array().map(|c| c.iter().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("\n")).unwrap_or_default()
}
pub fn token(frame: &Value) -> Option<String> { frame["result"]["_meta"]["gil/planApprovalToken"].as_str().map(str::to_owned) }

#[track_caller]
pub fn ok(frame: Value, what: &str) -> Value {
    assert!(is_ok(&frame), "{what}: expected success, got {}", frame);
    frame
}
#[track_caller]
pub fn refused(frame: Value, what: &str) -> Value {
    assert!(frame.get("error").is_none(), "{what}: expected a Core refusal, got JSON-RPC error {frame}");
    assert!(!is_ok(&frame), "{what}: expected a refusal but the call succeeded: {frame}");
    frame
}
#[track_caller]
pub fn rpc_error(frame: Value, what: &str) -> Value {
    assert!(frame.get("error").is_some(), "{what}: expected a JSON-RPC schema error, got {frame}");
    frame
}

pub fn draft(ids: &[&str], basis: &[String], extra: &[(&str, &str)]) -> Value {
    let mut synthesis = serde_json::Map::new();
    synthesis.insert("statement".into(), json!("합성 자료로 항목을 순서대로 확인하고 파일 증거를 남긴다"));
    synthesis.insert("basis_refs".into(), json!(basis.join("\n")));
    synthesis.insert("exploration_checklist".into(), json!(ids.join("\n")));
    for (k, v) in extra { synthesis.insert((*k).into(), json!(v)); }
    json!({
        "scope": {"goal":"합성 자료 기반 항목별 보고서","in_scope":"제공된 합성 자료의 비교","out_of_scope":"실시간 데이터·실제 개인정보","constraints":"외부 네트워크 없음","success_criteria":"항목마다 파일 증거가 남는다"},
        "items": ids.iter().map(|id| json!({"id":id,"question":format!("항목 {id}을 합성 자료로 확인한다"),"done_when":format!("evidence 파일에 항목 {id} 결과가 저장된다")})).collect::<Vec<_>>(),
        "synthesis": synthesis,
    })
}

pub const CONTRACT: &str = "objective: 합성 작업 범위를 합의한다\nnext_action: 사용자에게 묻는다\ndone_when: 사용자의 답을 받았다\n";
pub const QUESTION: &str = "question: 어떤 합성 작업인가\nchoices: 보고서 / 비교\nresponse: 합성 자료로 항목별 보고서를 작성해줘\n";
pub const INTERPRETATION: &str = "interpretation: 항목별 파일 증거를 남기며 보고서를 만든다\nunresolved: 없음\n";

pub fn step_ref(frame: &Value, kind: &str) -> String {
    let text = said(frame);
    let line = text.lines().find(|l| l.starts_with("열었다: step:") && l.contains(&format!(" · {kind}"))).unwrap_or_else(|| panic!("no open receipt for {kind}: {text}"));
    line.trim_start_matches("열었다: ").split(" · ").next().unwrap().to_owned()
}
pub fn cycle_number(frame: &Value) -> u32 {
    let text = said(frame);
    let line = text.lines().find(|l| l.starts_with("열었다: cycle:C")).unwrap_or_else(|| panic!("no cycle open receipt: {text}"));
    line.trim_start_matches("열었다: cycle:C").split(' ').next().unwrap().parse().unwrap()
}

/// Typed Verify close in visual builds, ordinary text close otherwise.
pub fn verify_args(root: &Path, step: &str, report: &str) -> Value {
    if cfg!(feature = "visual-authoring") {
        json!({"project_root":root,"report":report,"expected_owner":{"kind":"step","ref":step},"visuals":{"attachments":[],"uses":[],"declaration":{"state":"not_needed","reason":"문서 증거만 필요한 합성 시험"}}})
    } else {
        json!({"project_root":root,"report":report})
    }
}
