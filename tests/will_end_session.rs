//! 미달성 종료의 **실제 파일 transaction** — 세션·CLI·MCP stdio (Storage Transition Contract §4.5).
//!
//! 여기서 재는 것은 저장을 지나야만 보이는 것들이다.
//!
//! - 닫기의 기존 조건(Verify 증거·변경 검사)은 미달성이라는 이유로 면제되지 않는다.
//! - 거절과 저장 실패 뒤에는 `state.yaml` 이 한 바이트도 달라지지 않는다.
//! - 확정 뒤 응답을 잃은 쪽은 새 프로세스에서 저장된 이력을 읽어 판단할 수 있고, 같은 요청은
//!   두 번째 기록을 만들지 않는다.
//! - `gil close --unachieved will:W<n>` 와 MCP `gil_close.unachieved_will` 은 같은 판정을 쓴다.
//! - 상태·문맥·이야기·Monitor 읽기 모델이 달성과 미달성을 구분한다.
//!
//! 합성 프로젝트만 쓴다. 실제 모델 호출과 Host 화면은 이 시험의 범위가 아니다.
#![cfg(all(feature = "will-end-history", not(feature = "approved-plan")))]

#[path = "body_shim.rs"]
mod body_shim;
mod common;
use gil::{ActionError, CycleKind, EndResult, NodeKind, ProjectSession, Report, SessionError, WillRef};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn setup(name: &str) -> (PathBuf, ProjectSession) {
    let root = common::scratch(name);
    std::fs::write(root.join("old.txt"), "existing source").unwrap();
    let mut session = ProjectSession::start(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    *session.project_mut() = common::bootstrap_from(session.project().clone());
    common::up_to_verify(session.project_mut());
    session.commit().unwrap();
    (root, session)
}

fn state(root: &Path) -> Vec<u8> {
    std::fs::read(root.join(gil::STATE_PATH)).unwrap()
}

fn active(session: &ProjectSession) -> WillRef {
    session.project().active_will().expect("걸린 행동이 있다").id()
}

/// 시도의 기록 — 수행 범위·막힌 이유·미검증을 본문에 구분해 적는다.
fn attempt(evidence: &str) -> Report {
    common::full_report(&common::spec(), CycleKind::Experiment, NodeKind::Verify)
        .with("execution", "수행: 입력 3건 중 1건을 변환했다. 막힘: 원본 2건이 손상돼 읽히지 않았다.")
        .with("result", "미검증: 전체 변환 결과와 실패 목록. 성공 조건은 평가하지 못했다.")
        .with("evidence_files", evidence)
}

fn report(kind: NodeKind) -> Report {
    common::full_report(&common::spec(), CycleKind::Experiment, kind)
}

#[test]
fn an_unachieved_verify_still_needs_real_evidence_and_survives_a_restart() {
    let (root, mut session) = setup("end-session-evidence");
    let will = active(&session);
    let promised = session.project().active_will().unwrap().clone();
    let saved = state(&root);
    let memory = format!("{:?}", session.project());

    // 증거 없이는 닫히지 않는다 — 미달성도 시도 기록을 남겨야 한다.
    for paths in ["", "missing.md", "old.txt"] {
        let err = session.close_step_unachieved(attempt(paths), will).unwrap_err();
        assert!(matches!(err, SessionError::Evidence(_)), "{paths:?}: {err}");
        assert_eq!(format!("{:?}", session.project()), memory);
        assert_eq!(state(&root), saved);
    }

    std::fs::write(root.join("attempt.md"), "변환 1/3 수행. 원본 2건 손상으로 중단. 전체 결과 미검증.").unwrap();
    let closed = session.close_step_unachieved(attempt("attempt.md"), will).expect("증거가 있으면 닫힌다");
    assert_eq!(closed.result, EndResult::Unachieved);
    session.commit().unwrap();
    drop(session);

    // 새 프로세스가 읽는 것은 저장된 이력뿐이다.
    let again = ProjectSession::open(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    let project = again.project();
    let last = project.current_existence().journey().last_ended_will().unwrap();
    assert_eq!(last.will(), &promised, "원래 약속이 저장을 넘으며 달라졌다");
    assert_eq!(last.result(), EndResult::Unachieved);
    assert!(project.active_will().is_none());
    let node = project.current_cycle().steps().nodes().iter().find(|node| node.kind == NodeKind::Verify).unwrap();
    assert!(node.is_closed() && node.snapshot.is_some());
    assert_eq!(node.report.as_ref().unwrap().get("evidence_files"), Some("attempt.md"));
    assert_eq!(node.journey, Some(project.current_existence().current_journey()));

    // 같은 요청의 재전송은 종료된 자리와 결과를 알려 주며 거절된다 — 파일은 그대로다.
    let durable = state(&root);
    drop(again);
    let mut again = ProjectSession::open(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    let err = again.close_step_unachieved(attempt("attempt.md"), will).unwrap_err();
    assert!(matches!(err, SessionError::Action(ActionError::WillAlreadyEnded { result: EndResult::Unachieved, .. })), "{err}");
    assert_eq!(state(&root), durable);
}

#[test]
fn a_dirty_world_blocks_an_unachieved_close_outside_verify() {
    let (root, mut session) = setup("end-session-dirty");
    std::fs::write(root.join("attempt.md"), "관측: 합성 입력을 검사했다.").unwrap();
    session.close_step(attempt("attempt.md")).unwrap();
    common::opened(session.project_mut(), NodeKind::Analysis);
    session.commit().unwrap();
    let will = active(&session);
    let saved = state(&root);
    let memory = format!("{:?}", session.project());

    // Verify 밖에서 바뀐 세계는 미달성 닫기로 확정되지 않는다.
    std::fs::write(root.join("old.txt"), "changed outside Verify").unwrap();
    let err = session.close_step_unachieved(report(NodeKind::Analysis), will).unwrap_err();
    assert!(matches!(err, SessionError::Dirty { .. }), "{err}");
    assert_eq!(format!("{:?}", session.project()), memory);
    assert_eq!(state(&root), saved);
}

#[test]
fn a_failed_save_leaves_the_step_open_and_the_will_active() {
    let (root, mut session) = setup("end-session-save-failure");
    let will = active(&session);
    let saved = state(&root);
    std::fs::write(root.join("attempt.md"), "변환 1/3 수행 뒤 중단.").unwrap();

    // 임시 파일이 설 자리를 디렉터리로 막는다 — 교체 전에 저장이 실패한다.
    let blocker = root.join(".gil/state.yaml.writing");
    std::fs::create_dir(&blocker).unwrap();
    session.close_step_unachieved(attempt("attempt.md"), will).expect("도메인은 닫는다");
    assert!(session.commit().is_err(), "막힌 자리에 저장이 섰다");
    assert_eq!(state(&root), saved, "확정 전 실패가 상태 파일을 바꿨다");
    drop(session);
    std::fs::remove_dir(&blocker).unwrap();

    // 다시 연 프로세스에는 열린 Verify 와 진행 중 약속이 그대로 있다. 종료 기록은 없다.
    let mut again = ProjectSession::open(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    assert!(again.can_confirm_artifact(), "Verify 가 열려 있어야 한다");
    assert_eq!(active(&again), will);
    assert!(again.project().ended_will(will).is_none());
    // 그리고 같은 요청을 다시 보내면 이번에는 확정된다 — 한 번만.
    again.close_step_unachieved(attempt("attempt.md"), will).unwrap();
    again.commit().unwrap();
    let ended = again.project().current_existence().journey().ended_wills().iter()
        .filter(|ended| ended.will().id() == will).count();
    assert_eq!(ended, 1);
}

// ── 실패 뒤의 길 ───────────────────────────────────────────────────────────

/// 미달성 Verify 뒤 Analysis·Outcome 을 거쳐 실패로 닫은 실험 — 저장까지 마친 상태.
fn failed_after_unachieved(name: &str) -> (PathBuf, ProjectSession) {
    let (root, mut session) = setup(name);
    let will = active(&session);
    std::fs::write(root.join("attempt.md"), "변환 1/3 수행 뒤 중단. 전체 결과 미검증.").unwrap();
    session.close_step_unachieved(attempt("attempt.md"), will).unwrap();
    common::opened(session.project_mut(), NodeKind::Analysis);
    session.close_step(report(NodeKind::Analysis)).unwrap();
    let outcome = common::opened(session.project_mut(), NodeKind::Outcome);
    session.close_step(report(NodeKind::Outcome).with("verdict", "failure").with(common::ACTION, "close_cycle")).unwrap();
    let failure = common::cycle_report(session.project().current_cycle(), "failure", common::step_id(session.project(), outcome));
    session.close_cycle(failure).unwrap();
    session.commit().unwrap();
    (root, session)
}

/// 기존 실패 보고 → 되돌아가기 → 전체 복원 → 다음 실험. 미달성 종료는 그 길을 바꾸지 않는다.
#[cfg(not(feature = "product-safety"))]
#[test]
fn a_failed_unachieved_experiment_goes_through_revisit_and_restore_to_the_next_experiment() {
    let (root, mut session) = failed_after_unachieved("end-session-next");
    let failed = session.project().current_cycle().id();
    assert!(root.join("attempt.md").exists());
    assert!(session.project().cycles().can_revisit());
    assert!(!session.project().current_chain().unwrap().is_closed(), "미달성 종료가 Chain 을 닫았다");

    session.revisit_cycle().expect("기존 되돌아가기가 그대로 동작한다");
    // 세계 전체가 대상의 세계로 돌아간다 — 시도의 파일은 폴더에서 치워지고 기록에는 남는다.
    assert!(!root.join("attempt.md").exists(), "부분 결과가 현재 세계에 남았다");
    assert_eq!(std::fs::read_to_string(root.join("old.txt")).unwrap(), "existing source");

    session.open_branch_cycle(CycleKind::Experiment).expect("복원 뒤 다음 실험을 연다");
    session.commit().unwrap();
    drop(session);

    let again = ProjectSession::open(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    let project = again.project();
    let next = project.current_cycle();
    assert_eq!(next.revisit_from(), Some(failed));
    // 다음 실험은 앞 실험의 미달성 종료에 묶이지 않는다. 앞 실험의 기록은 그대로다.
    assert!(project.unachieved_in(next.id().to_ref()).is_none());
    assert_eq!(project.openable_here(), vec![NodeKind::Define]);
    let kept = project.unachieved_in(failed.to_ref()).expect("실패한 실험의 미달성 기록");
    assert_eq!(kept.result(), EndResult::Unachieved);
    let verify = project.all_cycles().find(|cycle| cycle.id() == failed).unwrap()
        .steps().nodes().iter().find(|node| node.kind == NodeKind::Verify).unwrap();
    assert_eq!(verify.report.as_ref().unwrap().get("evidence_files"), Some("attempt.md"), "미수행 이유는 원본 보고서에 남는다");
}

/// 제품 보호 구성: 실패 판정은 복구 승인이 아니다. 승인 없는 되돌아가기와 일반 복원은 거절된다.
#[cfg(feature = "product-safety")]
#[test]
fn a_failure_after_an_unachieved_end_does_not_approve_the_recovery() {
    let (root, session) = failed_after_unachieved("end-session-product");
    drop(session);
    let before = state(&root);
    for command in ["revisit", "restore"] {
        let refused = gil(&root, &[command], "");
        assert!(!refused.status.success(), "{command} 이 승인 없이 실행됐다: {}", said(&refused));
    }
    assert_eq!(state(&root), before, "거절된 복구가 기록을 바꿨다");
    assert!(root.join("attempt.md").exists(), "승인 없이 파일이 치워졌다");
    let session = ProjectSession::open_read_only(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    assert!(session.project().cycles().pending_revisit().is_none(), "승인 없이 되돌아갔다");
}

// ── CLI ────────────────────────────────────────────────────────────────────

fn gil(root: &Path, args: &[&str], body: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_gil")).args(args).current_dir(root)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(body.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
}

fn said(output: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr))
}

const ATTEMPT: &str = "execution: 수행 1/3, 원본 2건 손상으로 중단\nresult: 미검증 — 전체 변환 결과\nevidence_files: attempt.md\n";

#[test]
fn the_cli_marker_names_the_will_and_the_read_surfaces_show_the_result() {
    let (root, session) = setup("end-session-cli");
    let will = active(&session).to_string();
    drop(session);
    std::fs::write(root.join("attempt.md"), "변환 1/3 수행. 원본 2건 손상.").unwrap();
    let before = state(&root);

    // 값이 없거나, 이름이 아니거나, 다른 약속이면 닫지 않는다.
    for args in [vec!["close", "--unachieved"], vec!["close", "--unachieved", "W3"], vec!["close", "--unachieved", "will:W99"], vec!["close", "--partial"]] {
        let refused = gil(&root, &args, ATTEMPT);
        assert!(!refused.status.success(), "{args:?} 가 닫혔다: {}", said(&refused));
        assert_eq!(state(&root), before, "{args:?}");
    }
    // 열린 자리의 문맥은 지목할 이름과 닫는 법을 말한다.
    let context = said(&gil(&root, &["context"], ""));
    assert!(context.contains(&format!("gil close --unachieved {will}")), "{context}");

    let closed = gil(&root, &["close", "--unachieved", &will], ATTEMPT);
    assert!(closed.status.success(), "{}", said(&closed));
    let receipt = said(&closed);
    assert!(receipt.contains("end_result: unachieved") && receipt.contains(&will), "{receipt}");
    let after = state(&root);
    assert_ne!(after, before);

    // 응답을 잃었다고 보고 같은 명령을 다시 보낸다.
    let resent = gil(&root, &["close", "--unachieved", &will], ATTEMPT);
    assert!(!resent.status.success());
    assert!(said(&resent).contains("end_result: unachieved"), "종료된 결과를 알려 줘야 한다: {}", said(&resent));
    assert_eq!(state(&root), after, "재전송이 기록을 바꿨다");

    // 읽기 화면 — 상태·문맥·이야기. 조회는 저장하지 않는다.
    let status = said(&gil(&root, &["status"], ""));
    assert!(status.contains(&will) && status.contains("미달성"), "{status}");
    let context = said(&gil(&root, &["context"], ""));
    assert!(context.contains("약속 미달성") && context.contains("약속 달성"), "{context}");
    assert!(context.contains("failure") && context.contains("close_cycle"), "남은 길을 말해야 한다: {context}");
    let story = said(&gil(&root, &["story"], ""));
    assert!(story.contains("미달성"), "{story}");
    assert_eq!(state(&root), after);

    // 남은 단계를 건너뛰지 않고, 같은 실험 안에서 다시 시도하지 않는다.
    let open = |kind: &str| gil(&root, &["open", kind], "objective: 다음\nnext_action: 한다\ndone_when: 됐다\n");
    assert!(!open("outcome").status.success(), "Analysis 를 건너뛰었다");
    assert!(open("analysis").status.success());
    let analysis = "hypothesis_fit: 판단 불가\nproblem_solved: 아니오\nsuccess_condition_met: 평가하지 못했다\nguardrail_triggered: 아니오\ninterpretation: 수행하지 못해 미검증이다. 질문이 반증된 것은 아니다.\n";
    assert!(gil(&root, &["close"], analysis).status.success());
    let retry = open("hypothesis");
    assert!(!retry.status.success(), "같은 실험 안에서 새 가설이 열렸다");
    assert!(said(&retry).contains(&will), "어느 미달성 종료 때문인지 말해야 한다: {}", said(&retry));
    assert!(open("outcome").status.success());
    let success = "verdict: success\nlesson: 없음\nnext_direction.action: close_cycle\nnext_direction.reason: 끝\n";
    assert!(!gil(&root, &["close"], success).status.success(), "미달성 실험의 Outcome 이 success 로 닫혔다");
    let failure = "verdict: failure\nlesson: 원본 손상 여부를 먼저 확인해야 한다\nnext_direction.action: close_cycle\nnext_direction.reason: 약속한 변환을 수행하지 못했다\n";
    assert!(gil(&root, &["close"], failure).status.success());
}

#[test]
fn a_stale_cli_request_cannot_be_read_as_the_next_report() {
    // Outcome 을 미달성으로 닫은 뒤 서 있는 자리는 Cycle Report 다. 재전송이 그 닫기로 읽히면 안 된다.
    let (root, mut session) = setup("end-session-stale");
    std::fs::write(root.join("attempt.md"), "관측: 합성 입력.").unwrap();
    session.close_step(attempt("attempt.md")).unwrap();
    common::walked(session.project_mut(), NodeKind::Analysis);
    common::opened(session.project_mut(), NodeKind::Outcome);
    session.commit().unwrap();
    let will = active(&session).to_string();
    drop(session);

    let outcome = "verdict: failure\nlesson: 판정을 내릴 근거가 없었다\nnext_direction.action: close_cycle\nnext_direction.reason: 판정하지 못했다\n";
    assert!(gil(&root, &["close", "--unachieved", &will], outcome).status.success());
    let after = state(&root);
    let resent = gil(&root, &["close", "--unachieved", &will], outcome);
    assert!(!resent.status.success());
    assert_eq!(state(&root), after);
    let session = ProjectSession::open_read_only(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    assert!(!session.project().current_cycle().is_closed(), "오래된 요청이 Cycle 을 닫았다");
}

// ── MCP stdio ──────────────────────────────────────────────────────────────

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
        mcp.request("initialize", serde_json::json!({"protocolVersion": "2025-06-18", "capabilities": {},
            "clientInfo": {"name": "gil-test", "version": "0"}}));
        mcp.send(&serde_json::json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        mcp
    }

    fn send(&mut self, frame: &serde_json::Value) {
        let stdin = self.kid.stdin.as_mut().unwrap();
        writeln!(stdin, "{frame}").unwrap();
        stdin.flush().unwrap();
    }

    fn request(&mut self, method: &str, params: serde_json::Value) -> serde_json::Value {
        let params = body_shim::tool_call(method, params);
        let id = self.next;
        self.next += 1;
        self.send(&serde_json::json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let mut line = String::new();
        self.out.read_line(&mut line).unwrap();
        let got: serde_json::Value = serde_json::from_str(&line).unwrap_or_else(|err| panic!("{line:?}: {err}"));
        assert_eq!(got["id"], serde_json::json!(id));
        got
    }

    fn call(&mut self, name: &str, arguments: serde_json::Value) -> serde_json::Value {
        self.request("tools/call", serde_json::json!({"name": name, "arguments": arguments}))
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        drop(self.kid.stdin.take());
        let _ = self.kid.wait();
    }
}

#[test]
fn the_mcp_close_offers_the_marker_and_applies_the_same_designation_check() {
    let (root, session) = setup("end-session-mcp");
    let will = active(&session).to_string();
    drop(session);
    std::fs::write(root.join("attempt.md"), "변환 1/3 수행. 원본 2건 손상.").unwrap();
    let settings = common::scratch("end-session-mcp-settings");
    let mut mcp = Mcp::serve(&settings);

    // 이 빌드의 도구 표면은 입력을 내놓고, 그 설명은 이유를 본문에 적으라고 말한다.
    let tools = mcp.request("tools/list", serde_json::json!({}));
    let close = tools["result"]["tools"].as_array().unwrap().iter().find(|tool| tool["name"] == "gil_close").unwrap().clone();
    let field = &close["inputSchema"]["properties"]["unachieved_will"];
    assert_eq!(field["type"], "string", "{close}");
    assert!(field["description"].as_str().unwrap().contains("Report body"));

    let before = state(&root);
    let arguments = |will: &str| serde_json::json!({"project_root": root, "report": ATTEMPT, "unachieved_will": will});

    // 이름이 아닌 값은 wire 에서 거절된다(-32602). 다른 약속은 Core 가 거절한다. 어느 쪽도 닫지 않는다.
    let malformed = mcp.call("gil_close", arguments("W3"));
    assert_eq!(malformed["error"]["code"], -32602, "{malformed}");
    assert!(malformed["error"]["message"].as_str().unwrap().contains("unachieved_will"));
    let other = mcp.call("gil_close", arguments("will:W99"));
    assert_eq!(other["result"]["isError"], true, "{other}");
    assert_eq!(state(&root), before);

    let closed = mcp.call("gil_close", arguments(&will));
    assert_eq!(closed["result"]["structuredContent"]["ok"], true, "{closed}");
    assert!(closed["result"]["structuredContent"]["said"].as_str().unwrap().contains("end_result: unachieved"));
    let after = state(&root);
    assert_ne!(after, before);

    // 재전송: 이미 끝난 자리와 결과로 답하고 아무것도 바꾸지 않는다.
    let resent = mcp.call("gil_close", arguments(&will));
    assert_eq!(resent["result"]["isError"], true);
    assert!(resent["result"]["structuredContent"]["problem"].as_str().unwrap().contains("end_result: unachieved"), "{resent}");
    assert_eq!(state(&root), after);

    // 문맥의 다음 호출은 남은 단계를 건너뛰지 않고, 실험이 실패로만 닫힌다는 것을 말한다.
    let context = mcp.call("gil_context", serde_json::json!({"project_root": root}));
    let text = context["result"]["structuredContent"]["said"].as_str().unwrap().to_owned();
    assert!(text.contains("analysis") && text.contains("failure") && text.contains(&will), "{text}");

    // Monitor 읽기 모델: 닫힌 자리마다 종료 결과가 실린다. 판정이 아니라 기록의 표지다.
    let prepared = mcp.call("gil_monitor_prepare", serde_json::json!({"project_root": root}));
    let scope = prepared["result"]["structuredContent"]["scope_id"].as_str().expect("scope").to_owned();
    let view = mcp.call("gil_monitor_read", serde_json::json!({"scope_id": scope}));
    let view = &view["result"]["structuredContent"]["view"];
    let steps: Vec<(String, String)> = view["timeline"].as_array().expect("timeline").iter()
        .flat_map(|cycle| cycle["steps"].as_array().unwrap().iter())
        .filter_map(|step| Some((step["kind"].as_str()?.to_owned(), step["will_end"].as_str()?.to_owned())))
        .collect();
    assert!(steps.contains(&("verify".into(), "unachieved".into())), "{steps:?}");
    assert!(steps.contains(&("define".into(), "achieved".into())), "{steps:?}");
    assert_eq!(steps.iter().filter(|(_, result)| result == "unachieved").count(), 1);
    assert_eq!(state(&root), after, "조회가 기록을 바꿨다");
}
