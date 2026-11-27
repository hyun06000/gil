//! 형식 9 — **시각 등록부가 있는 새 프로젝트의 미달성 닫기**를 실제 표준 입출력 서버로 잰다.
//!
//! 형식 9 의 Verify 는 일반 텍스트 닫기가 아니라 typed 닫기(`expected_owner` + `visuals`)로만
//! 닫히므로, 미달성 표시도 그 경로를 지난다. 이 시험이 재는 것:
//!
//! - `unachieved_will: null` 은 입력 생략이 아니다. typed 닫기와 일반 닫기가 똑같이 `-32602` 로
//!   거절하고, 기록은 한 바이트도 바뀌지 않는다.
//! - 다른 약속을 지목하면 Core 가 거절한다. 바르게 지목하면 시각 선언·증거 검사를 그대로 거쳐
//!   미달성으로 끝나고, 같은 요청의 재전송은 끝난 자리와 결과로 답하며 아무것도 바꾸지 않는다.
//! - 그 뒤의 실험은 실패로만 닫히고, 새 프로세스의 상태·Monitor 가 미달성을 그대로 읽는다.
//!
//! 합성 프로젝트만 쓴다. 실제 Host 화면과 모델 호출은 이 시험의 범위가 아니다.
#![cfg(all(feature = "will-end-history", feature = "visual-authoring", not(feature = "approved-plan"), any(unix, windows)))]

#[path = "body_shim.rs"]
mod body_shim;
mod common;
use gil::{CycleKind, EndResult, NodeKind};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn scratch(name: &str) -> PathBuf {
    let at = std::env::temp_dir().join(format!("gil-end-visual-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&at);
    std::fs::create_dir_all(&at).unwrap();
    at
}

/// The product record of a visual project: inline on Unix, private app data on Windows.
fn product_state_path(root: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        use sha2::{Digest, Sha256};
        use std::os::windows::ffi::OsStrExt;
        let canonical = root.canonicalize().unwrap();
        let mut identity = Vec::new();
        for unit in canonical.as_os_str().encode_wide() {
            identity.extend_from_slice(&unit.to_le_bytes());
        }
        let key = format!("{:x}", Sha256::digest(&identity));
        PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap())
            .join("GIL/project-state-v1").join(key).join(gil::STATE_PATH)
    }
    #[cfg(not(windows))]
    { root.join(gil::STATE_PATH) }
}

fn state(root: &Path) -> Vec<u8> {
    std::fs::read(product_state_path(root)).unwrap()
}

struct Mcp {
    kid: std::process::Child,
    out: BufReader<std::process::ChildStdout>,
    next: i64,
}

impl Mcp {
    fn serve(settings: &Path) -> Self {
        let mut kid = Command::new(env!("CARGO_BIN_EXE_gil")).args(["mcp", "--serve"])
            .env("GIL_MONITOR_STATE_DIR", settings.join("bindings"))
            .current_dir(std::env::temp_dir())
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("gil mcp --serve");
        let out = BufReader::new(kid.stdout.take().unwrap());
        let mut mcp = Self { kid, out, next: 1 };
        mcp.request("initialize", json!({"protocolVersion": "2025-06-18", "capabilities": {},
            "clientInfo": {"name": "gil-test", "version": "0"}}));
        mcp.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        mcp
    }

    fn send(&mut self, frame: &Value) {
        let stdin = self.kid.stdin.as_mut().unwrap();
        writeln!(stdin, "{frame}").unwrap();
        stdin.flush().unwrap();
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let params = body_shim::tool_call(method, params);
        let id = self.next;
        self.next += 1;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let mut line = String::new();
        self.out.read_line(&mut line).unwrap();
        let got: Value = serde_json::from_str(&line).unwrap_or_else(|err| panic!("{line:?}: {err}"));
        assert_eq!(got["id"], json!(id));
        got
    }

    /// The whole JSON-RPC reply: `error` for a wire refusal, `result` for an executed call.
    fn call(&mut self, name: &str, arguments: Value) -> Value {
        self.request("tools/call", json!({"name": name, "arguments": arguments}))
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        drop(self.kid.stdin.take());
        let _ = self.kid.wait();
    }
}

fn said(reply: &Value) -> String {
    let content = &reply["result"]["structuredContent"];
    format!("{}{}", content["said"].as_str().unwrap_or(""), content["problem"].as_str().unwrap_or(""))
}

fn executed_ok(reply: &Value) -> bool {
    reply["result"]["isError"] != json!(true) && reply["result"]["structuredContent"]["ok"] == json!(true)
}

const ATTEMPT: &str = "execution: 수행 1/3, 원본 2건 손상으로 중단\nresult: 미검증 — 전체 변환 결과\nevidence_files: evidence/attempt.md\n";
const CONTRACT: &str = "objective: 다음\nnext_action: 한다\ndone_when: 됐다\n";

#[test]
fn a_format_9_verify_ends_unachieved_through_the_typed_close_and_null_is_never_an_omission() {
    let root = scratch("root");
    let settings = scratch("settings");
    let mut mcp = Mcp::serve(&settings);
    let started = mcp.call("gil_start", json!({"project_root": root}));
    assert_eq!(started["result"]["structuredContent"]["storage_format"], gil::END_HISTORY_VISUAL_FORMAT, "{started}");

    // Synthetic walk to an open Verify, through the domain API on the real product record.
    let mut project = common::bootstrap_from(gil::load(common::spec(), product_state_path(&root)).unwrap());
    common::up_to_verify(&mut project);
    gil::save(&project, product_state_path(&root)).unwrap();
    assert!(String::from_utf8_lossy(&state(&root)).starts_with("format: 9\n"));
    let promised = project.active_will().unwrap().clone();
    let will = promised.id().to_string();
    let verify = promised.target().to_string();
    std::fs::create_dir_all(root.join("evidence")).unwrap();
    std::fs::write(root.join("evidence/attempt.md"), "변환 1/3 수행. 원본 2건 손상으로 중단. 전체 결과 미검증.").unwrap();

    let typed = |marker: Option<Value>| {
        let mut input = json!({"project_root": root, "report": ATTEMPT,
            "expected_owner": {"kind": "step", "ref": verify},
            "visuals": {"attachments": [], "uses": [], "declaration": {"state": "not_needed", "reason": "변환 건수만 세는 검증이라 그림이 대신할 것이 없다"}}});
        if let Some(marker) = marker { input["unachieved_will"] = marker; }
        input
    };
    let before = state(&root);

    // `null` is not an omission: both close shapes refuse it at the wire, and nothing is written.
    for input in [typed(Some(Value::Null)), json!({"project_root": root, "report": ATTEMPT, "unachieved_will": null})] {
        let refused = mcp.call("gil_close", input);
        assert_eq!(refused["error"]["code"], -32602, "{refused}");
        let message = refused["error"]["message"].as_str().unwrap();
        assert!(message.contains("unachieved_will") && message.contains("Nothing was closed"), "{message}");
        assert_eq!(state(&root), before, "a refused null marker touched the record");
    }

    // Another Will is refused by Core before the owner check; nothing is written.
    let other = mcp.call("gil_close", typed(Some(json!("will:W99"))));
    assert_eq!(other["result"]["isError"], true, "{other}");
    assert_eq!(other["result"]["structuredContent"]["code"], "unachieved_refused", "{other}");
    assert_eq!(state(&root), before);

    // The named active Will ends as unachieved; the visual declaration and evidence are kept.
    let closed = mcp.call("gil_close", typed(Some(json!(will))));
    assert!(executed_ok(&closed), "{closed}");
    assert_eq!(closed["result"]["structuredContent"]["confirmed"], true);
    assert_eq!(closed["result"]["structuredContent"]["visuals"]["state"], "not_needed");
    let after = state(&root);
    assert_ne!(after, before);
    assert!(String::from_utf8_lossy(&after).starts_with("format: 9\n"));
    let reread = gil::load(common::spec(), product_state_path(&root)).unwrap();
    assert!(reread.visual_state().is_some() && reread.records_end_results());
    let ended = reread.current_existence().journey().last_ended_will().unwrap();
    assert_eq!(ended.will(), &promised, "the promise changed while ending");
    assert_eq!(ended.result(), EndResult::Unachieved);
    let node = reread.current_cycle().steps().nodes().iter().find(|node| node.kind == NodeKind::Verify).unwrap();
    assert_eq!(node.report.as_ref().unwrap().get("evidence_files"), Some("evidence/attempt.md"));

    // A resend is answered with the ended Step and result; a null resend is still a wire error.
    let resent = mcp.call("gil_close", typed(Some(json!(will))));
    assert_eq!(resent["result"]["isError"], true, "{resent}");
    assert_eq!(resent["result"]["structuredContent"]["code"], "unachieved_refused");
    assert!(said(&resent).contains("end_result: unachieved"), "{resent}");
    assert_eq!(mcp.call("gil_close", typed(Some(Value::Null)))["error"]["code"], -32602);
    assert_eq!(state(&root), after, "a resend changed the record");

    // The Experiment now closes only as a failure; no new hypothesis inside it.
    assert!(executed_ok(&mcp.call("gil_open", json!({"project_root": root, "kind": "analysis", "contract": CONTRACT}))));
    let analysis = common::full_report(&common::spec(), CycleKind::Experiment, NodeKind::Analysis);
    let analysis: String = analysis.field_names().map(|k| format!("{k}: {}\n", analysis.get(k).unwrap())).collect();
    let analyzed = mcp.call("gil_close", json!({"project_root": root, "report": analysis}));
    assert!(executed_ok(&analyzed), "{analyzed}");
    let retry = mcp.call("gil_open", json!({"project_root": root, "kind": "hypothesis", "contract": CONTRACT}));
    assert_eq!(retry["result"]["isError"], true, "{retry}");
    assert!(said(&retry).contains(&will), "{retry}");
    assert!(executed_ok(&mcp.call("gil_open", json!({"project_root": root, "kind": "outcome", "contract": CONTRACT}))));
    let at_outcome = state(&root);
    let success = mcp.call("gil_close", json!({"project_root": root,
        "report": "verdict: success\nlesson: 없음\nnext_direction.action: close_cycle\nnext_direction.reason: 끝\n"}));
    assert_eq!(success["result"]["isError"], true, "{success}");
    assert_eq!(state(&root), at_outcome);
    let failure = mcp.call("gil_close", json!({"project_root": root,
        "report": "verdict: failure\nlesson: 원본 손상 여부를 먼저 확인해야 한다\nnext_direction.action: close_cycle\nnext_direction.reason: 약속한 변환을 수행하지 못했다\n"}));
    assert!(executed_ok(&failure), "{failure}");
    drop(mcp);

    // A new process reads the same history: status and the Monitor read model.
    let mut fresh = Mcp::serve(&settings);
    let status = fresh.call("gil_status", json!({"project_root": root}));
    assert!(said(&status).contains("미달성") && said(&status).contains(&will), "{status}");
    let prepared = fresh.call("gil_monitor_prepare", json!({"project_root": root}));
    let scope = prepared["result"]["structuredContent"]["scope_id"].as_str().expect("scope").to_owned();
    let view = fresh.call("gil_monitor_read", json!({"scope_id": scope}));
    let ends: Vec<(String, String)> = view["result"]["structuredContent"]["view"]["timeline"].as_array().expect("timeline").iter()
        .flat_map(|cycle| cycle["steps"].as_array().unwrap().iter())
        .filter_map(|step| Some((step["step_ref"].as_str()?.to_owned(), step["will_end"].as_str()?.to_owned())))
        .collect();
    assert!(ends.contains(&(verify.clone(), "unachieved".into())), "{ends:?}");
    assert_eq!(ends.iter().filter(|(_, result)| result == "unachieved").count(), 1, "{ends:?}");
    drop(fresh);
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&settings);
}
