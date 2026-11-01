//! Wave 1 / D — 가벼운 모델용 다음 행동 안내 회귀.
//!
//! 상태별 fixture 가 **올바른 다음 호출 하나**를 포함하고 **금지된 지름길**을 제안하지 않는지
//! 잰다. 이것은 지침 회귀이지 실제 Luna 실행 결과가 아니다. 기존 legacy Context 의 절은 건드리지
//! 않고, 새로 더한 `[지금 바로 할 호출]` 블록과 tool receipt 안내만 본다.
//!
//! 두 층을 본다.
//! - 라이브러리: `gil::context` 의 첫 블록 (in-memory Project · 승인 없는 legacy 걷기).
//! - 실제 binary: `gil mcp --serve` 의 receipt 에 붙는 한 줄 안내 (Plugin · 직접 MCP 가 같은 원문).
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use gil::{CycleKind, NodeKind, Project, Report};

mod common;
use common::{ACTION, bootstrap, cycle_report, full_report, opened, spec, step_id, up_to_verify, walked};

/// context 의 첫 블록 — 한 줄이어야 한다.
fn next_call(project: &Project) -> String {
    let told = gil::context(project);
    let (_, rest) = told.split_once("[지금 바로 할 호출]\n").expect("첫 블록이 없다");
    let line = rest.lines().next().expect("한 줄");
    assert!(!line.trim().is_empty(), "빈 안내:\n{told}");
    line.to_string()
}

fn says(project: &Project, must: &[&str], must_not: &[&str]) {
    let line = next_call(project);
    for m in must { assert!(line.contains(m), "{m:?} 가 없다: {line}"); }
    for n in must_not { assert!(!line.contains(n), "{n:?} 를 제안했다: {line}"); }
}

fn close_cycle_as(project: &mut Project, verdict: &str, outcome: NodeId, action: &str) {
    let report = cycle_report(project.cycles().current(), verdict, outcome).with(ACTION, action);
    project.close_cycle(report).expect("Cycle 을 닫는다");
}
use gil::NodeId;

// ── Interview: 새 시작 → Question → Synthesis 대기 → Outcome → Cycle 닫기 ──

#[test]
fn a_fresh_interview_asks_for_the_first_question_not_another_start() {
    let project = Project::start(spec(), common::first_world());
    says(&project, &["gil_open(kind: question)", "행동 계약"], &["gil_start를 호출", "gil_plan_propose", "experiment"]);
}

#[test]
fn open_interview_steps_wait_for_the_user_and_never_research() {
    let mut project = Project::start(spec(), common::first_world());
    opened(&mut project, NodeKind::Question);
    says(&project, &["질문하고 턴을 끝내", "gil_close"], &["gil_open", "조사한다"]);
    project.close_action_step(full_report(&spec(), CycleKind::Interview, NodeKind::Question)).unwrap();
    says(&project, &["gil_open(kind: interpretation)"], &["synthesis", "outcome"]);
    walked(&mut project, NodeKind::Interpretation);
    let line = next_call(&project);
    assert!(line.contains("question") && line.contains("synthesis"), "{line}");
    opened(&mut project, NodeKind::Synthesis);
    says(&project, &["턴을 끝내 답을 기다린다", "명시적으로 동의했을 때만", "침묵"],
        &["gil_plan_propose", "gil_open(kind: outcome)", "조사를 시작"]);
}

#[test]
fn an_approved_synthesis_leads_to_outcome_then_cycle_report_then_experiment() {
    let mut project = Project::start(spec(), common::first_world());
    walked(&mut project, NodeKind::Question);
    walked(&mut project, NodeKind::Interpretation);
    opened(&mut project, NodeKind::Synthesis);
    let cycle = project.cycles().current();
    let basis: Vec<String> = cycle.steps().nodes().iter().filter(|n| n.is_closed())
        .map(|n| cycle.step_ref(n.id).to_string()).collect();
    let report = full_report(&spec(), CycleKind::Interview, NodeKind::Synthesis)
        .with(common::APPROVED, common::APPROVAL).with(common::BASIS_REFS, basis.join("\n"));
    project.close_action_step(report).unwrap();
    // 승인된 Synthesis 만 닫혔다 — Outcome 을 연다. 다시 묻지 않는다.
    says(&project, &["gil_open(kind: outcome)", "다시 묻지 않는다"], &["gil_open(kind: experiment)", "gil_close"]);
    let outcome = opened(&mut project, NodeKind::Outcome);
    says(&project, &["판정해 gil_close", "별도로 닫는다"], &["gil_open(kind: experiment)"]);
    let synthesis = project.cycles().current().steps().nodes().iter()
        .find(|n| n.kind == NodeKind::Synthesis).map(|n| project.cycles().current().step_ref(n.id).to_string()).unwrap();
    project.close_action_step(full_report(&spec(), CycleKind::Interview, NodeKind::Outcome)
        .with(common::SYNTHESIS_REF, synthesis)).unwrap();
    // Outcome 까지 닫혔고 Cycle 은 열려 있다 — Cycle Report 가 다음이고, Experiment 는 그 뒤다.
    says(&project, &["Interview Cycle은 아직 열려 있다", "gil_close로 Cycle Report"], &["gil_open(kind: experiment)", "gil_open(kind: outcome)"]);
    let outcome = step_id(&project, outcome);
    close_cycle_as(&mut project, "success", outcome, "open_child");
    says(&project, &["gil_open(kind: experiment)"], &["gil_plan", "gil_close"]);
}

// ── Experiment: Define → Hypothesis → Verify(파일 증거) → Analysis → Outcome → Cycle 닫기 ──

#[test]
fn an_experiment_walks_define_hypothesis_verify_analysis_outcome_in_order() {
    let mut project = bootstrap();
    says(&project, &["gil_open(kind: define)", "질문 하나"], &["verify", "조사"]);
    opened(&mut project, NodeKind::Define);
    says(&project, &["판정 기준만 적어 gil_close"], &["자료 조회를 시작", "gil_open"]);
    project.close_action_step(full_report(&spec(), CycleKind::Experiment, NodeKind::Define)).unwrap();
    says(&project, &["gil_open(kind: hypothesis)"], &["verify", "evidence_files"]);
    opened(&mut project, NodeKind::Hypothesis);
    says(&project, &["gil_close", "Verify의 gil_open", "그 성공 전에는"], &["evidence_files"]);
    project.close_action_step(full_report(&spec(), CycleKind::Experiment, NodeKind::Hypothesis)).unwrap();
    says(&project, &["gil_open(kind: verify)", "열기 성공 전에는"], &["gil_close", "evidence_files"]);
    opened(&mut project, NodeKind::Verify);
    says(&project, &["프로젝트 파일로 저장", "evidence_files", "채팅만으로는 닫히지 않는다"], &["gil_open", "plan_document"]);
    common::close_here(&mut project, full_report(&spec(), CycleKind::Experiment, NodeKind::Verify)).unwrap();
    says(&project, &["gil_open(kind: analysis)"], &["outcome", "gil_close"]);
    opened(&mut project, NodeKind::Analysis);
    says(&project, &["interpretation", "gil_close"], &["새 조사를 한다", "gil_open"]);
    project.close_action_step(full_report(&spec(), CycleKind::Experiment, NodeKind::Analysis)).unwrap();
    let line = next_call(&project);
    assert!(line.contains("gil_open") && line.contains("outcome"), "{line}");
    let outcome = opened(&mut project, NodeKind::Outcome);
    says(&project, &["verdict", "lesson", "gil_close", "Cycle Report를 별도로"], &["gil_open(kind: experiment)"]);
    project.close_action_step(full_report(&spec(), CycleKind::Experiment, NodeKind::Outcome)).unwrap();
    says(&project, &["Experiment Cycle은 아직 열려 있다", "gil_close로 Cycle Report"], &["gil_open"]);
    let outcome = step_id(&project, outcome);
    close_cycle_as(&mut project, "success", outcome, "open_child");
    says(&project, &["gil_open(kind: experiment)", "gil_open(kind: interview)"], &["gil_revisit", "gil_close"]);
}

#[test]
fn a_failed_experiment_is_told_revisit_once_then_open_and_never_restore_silently() {
    use gil::ProjectSession;
    // 되돌아감은 Session 의 문이다(Artifact 복원이 따른다) — 격리 scratch 폴더에서만 밟는다.
    let dir = scratch("failed-revisit");
    std::fs::write(dir.join("work.txt"), "처음").unwrap();
    let mut session = ProjectSession::start(spec(), dir.join(gil::STATE_PATH)).expect("시작한다");
    *session.project_mut() = common::bootstrap_from(session.project().clone());
    up_to_verify(session.project_mut());
    std::fs::write(dir.join("work.txt"), "실패한 시도가 남긴 세계").unwrap();
    session.close_step(full_report(&spec(), CycleKind::Experiment, NodeKind::Verify).with("evidence_files", "work.txt")).expect("Verify 를 닫는다");
    walked(session.project_mut(), NodeKind::Analysis);
    opened(session.project_mut(), NodeKind::Outcome);
    session.close_step(full_report(&spec(), CycleKind::Experiment, NodeKind::Outcome)
        .with("verdict", "failure").with(ACTION, "close_cycle")).expect("판정을 닫는다");
    let last = session.project().cycles().current().steps().current().unwrap();
    let report = cycle_report(session.project().cycles().current(), "failure", last);
    assert_eq!(report.get(ACTION), Some("revisit"));
    session.close_cycle(report).expect("실패로 닫는다");
    session.commit().expect("눕힌다");
    assert!(session.project().cycles().can_revisit(), "이 시험은 되돌아갈 수 있는 자리를 전제한다");
    says(session.project(), &["gil_revisit(인수 없음)", "한 번", "사용자에게 먼저 알린다"], &["gil_restore", "gil_open(kind: experiment)을 바로"]);
    session.revisit_cycle().expect("적어 둔 대로 되돌아간다");
    says(session.project(), &["되돌아왔다", "gil_open(kind: interview|experiment)", "gil_revisit을 반복하지 않는다"], &["gil_close"]);
    drop(session);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn legacy_context_keeps_its_sections_and_never_mentions_plan_tools() {
    let mut project = bootstrap();
    up_to_verify(&mut project);
    let told = gil::context(&project);
    for section in ["═══ 현재 Cycle ═══", "═══ 지금 자리 ═══", "[지금 하려는 행동]", "[다음에 할 수 있는 것]", "[닫는 데 필요한 칸]"] {
        assert!(told.contains(section), "{section} 절이 사라졌다:\n{told}");
    }
    assert!(told.contains(&gil::next_moves(project.cycles())), "기존 다음 수가 사라졌다");
    assert!(!told.contains("gil_plan"), "legacy 프로젝트에 승인 계획 도구를 요구했다:\n{told}");
    assert!(!told.contains("승인 계획 프로필"), "{told}");
    assert_eq!(told.matches("[지금 바로 할 호출]").count(), 1);
}

// ── 실제 binary 의 receipt 안내 — Plugin 과 직접 MCP 가 같은 Core 를 쓴다 ──

fn gil() -> PathBuf {
    let mut at = std::env::current_exe().expect("시험 실행 파일");
    at.pop();
    if at.ends_with("deps") { at.pop(); }
    at.join(format!("gil{}", std::env::consts::EXE_SUFFIX))
}

fn scratch(name: &str) -> PathBuf {
    let at = std::env::temp_dir().join(format!("gil-guidance-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&at);
    std::fs::create_dir_all(&at).expect("자리");
    at
}

struct Mcp { kid: Child, out: BufReader<std::process::ChildStdout>, next: i64 }
impl Mcp {
    fn serve(settings: &Path) -> Self {
        let mut kid = Command::new(gil()).args(["mcp", "--serve"]).env("PATH", "/usr/bin:/bin")
            .env("GIL_MONITOR_STATE_DIR", settings.join("bindings")).current_dir(std::env::temp_dir())
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("gil mcp --serve");
        let out = BufReader::new(kid.stdout.take().unwrap());
        let mut me = Self { kid, out, next: 1 };
        let init = me.request("initialize", serde_json::json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "guidance-test", "version": "0"}}));
        assert!(init["result"]["instructions"].as_str().unwrap().contains("[지금 바로 할 호출]"), "instructions 가 새 원문이 아니다");
        me.send(&serde_json::json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        me
    }
    fn send(&mut self, frame: &serde_json::Value) {
        let stdin = self.kid.stdin.as_mut().unwrap();
        writeln!(stdin, "{}", serde_json::to_string(frame).unwrap()).unwrap();
        stdin.flush().unwrap();
    }
    fn request(&mut self, method: &str, params: serde_json::Value) -> serde_json::Value {
        let id = self.next; self.next += 1;
        self.send(&serde_json::json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let mut line = String::new();
        self.out.read_line(&mut line).unwrap();
        let got: serde_json::Value = serde_json::from_str(&line).unwrap_or_else(|e| panic!("{line:?} ({e})"));
        assert_eq!(got["id"], serde_json::json!(id));
        got
    }
    fn call(&mut self, name: &str, root: &Path, extra: serde_json::Value) -> serde_json::Value {
        let mut args = serde_json::json!({"project_root": root});
        for (k, v) in extra.as_object().unwrap() { args[k] = v.clone(); }
        self.request("tools/call", serde_json::json!({"name": name, "arguments": args}))["result"].clone()
    }
    fn done(mut self) {
        drop(self.kid.stdin.take());
        assert!(self.kid.wait().unwrap().success());
    }
}

/// 모든 text content 를 잇는다 — 안내는 content 의 앞에 붙는다.
fn hints(result: &serde_json::Value) -> String {
    result["content"].as_array().unwrap().iter().filter_map(|c| c["text"].as_str()).collect::<Vec<_>>().join("\n")
}

fn report_text(report: &Report) -> String {
    report.field_names().map(|k| format!("{k}: {}\n", report.get(k).unwrap())).collect()
}

fn contract(kind: &str) -> serde_json::Value {
    serde_json::json!({"kind": kind, "contract": format!("objective: {kind} 를 확인한다\nnext_action: {kind} 자리에서 할 일을 한다\ndone_when: 관측한 결과를 얻는다\n")})
}

#[test]
fn receipts_name_one_next_call_through_interview_and_into_an_experiment() {
    let root = scratch("receipts"); let settings = scratch("receipts-settings");
    let started = Command::new(gil()).arg("start").current_dir(&root).output().unwrap();
    assert!(started.status.success(), "{}", String::from_utf8_lossy(&started.stderr));
    let mut mcp = Mcp::serve(&settings);
    let rules = spec();

    let duplicate = mcp.call("gil_start", &root, serde_json::json!({}));
    assert_eq!(duplicate["isError"], true);
    let hint = hints(&duplicate);
    assert!(hint.contains("gil_start를 반복하지 않는다") && hint.contains("gil_context"), "{hint}");

    let resumed = mcp.call("gil_context", &root, serde_json::json!({}));
    assert_eq!(resumed["structuredContent"]["ok"], true, "{resumed}");
    assert!(hints(&resumed).contains("[지금 바로 할 호출] 한 줄을 먼저"), "{}", hints(&resumed));
    assert!(resumed["structuredContent"]["said"].as_str().unwrap().contains("gil_open(kind: question)"));

    let opened = mcp.call("gil_open", &root, contract("question"));
    assert_eq!(opened["structuredContent"]["ok"], true, "{opened}");
    let hint = hints(&opened);
    assert!(hint.contains("턴을 끝내 답을 기다린다") && hint.contains("gil_close"), "{hint}");

    let refused = mcp.call("gil_close", &root, serde_json::json!({"report": ""}));
    assert_eq!(refused["isError"], true);
    let hint = hints(&refused);
    assert!(hint.contains("닫힌 것은 없다") && !hint.contains("이 닫기가 성공"), "{hint}");

    let closed = mcp.call("gil_close", &root, serde_json::json!({"report": report_text(&full_report(&rules, CycleKind::Interview, NodeKind::Question))}));
    assert_eq!(closed["structuredContent"]["ok"], true, "{closed}");
    assert!(hints(&closed).contains("다음 행동: gil_open(kind: interpretation)"), "{}", hints(&closed));

    mcp.call("gil_open", &root, contract("interpretation"));
    let closed = mcp.call("gil_close", &root, serde_json::json!({"report": report_text(&full_report(&rules, CycleKind::Interview, NodeKind::Interpretation))}));
    let hint = hints(&closed);
    assert!(hint.contains("다음 중 하나를 연다") && hint.contains("question") && hint.contains("synthesis"), "{hint}");

    let opened = mcp.call("gil_open", &root, contract("synthesis"));
    let hint = hints(&opened);
    assert!(hint.contains("명시적 동의가 있을 때만 gil_close") && hint.contains("기다리는 동안 조사"), "{hint}");
    let synthesis = full_report(&rules, CycleKind::Interview, NodeKind::Synthesis)
        .with(common::APPROVED, common::APPROVAL).with(common::BASIS_REFS, "step:C1/S1\nstep:C1/S2");
    let closed = mcp.call("gil_close", &root, serde_json::json!({"report": report_text(&synthesis)}));
    assert_eq!(closed["structuredContent"]["ok"], true, "{closed}");
    let hint = hints(&closed);
    assert!(hint.contains("다음 행동: gil_open(kind: outcome)") && hint.contains("승인을 다시 묻거나"), "{hint}");
    assert!(!hint.contains("gil_open(kind: experiment)"), "{hint}");

    let opened = mcp.call("gil_open", &root, contract("outcome"));
    assert!(hints(&opened).contains("Interview Outcome을 열었다"), "{}", hints(&opened));
    let outcome = full_report(&rules, CycleKind::Interview, NodeKind::Outcome).with(common::SYNTHESIS_REF, "step:C1/S3");
    let closed = mcp.call("gil_close", &root, serde_json::json!({"report": report_text(&outcome)}));
    assert_eq!(closed["structuredContent"]["ok"], true, "{closed}");
    let hint = hints(&closed);
    assert!(hint.contains("Interview Cycle Report를 제출한다") && !hint.contains("gil_open(kind: experiment)"), "{hint}");

    let cycle = "verdict: success\noutcome_ref: step:C1/S4\nhandoff_summary: 합의한 범위를 실행한다\nnext_direction.action: open_child\nnext_direction.reason: 첫 질문을 검증한다\n";
    let closed = mcp.call("gil_close", &root, serde_json::json!({"report": cycle}));
    assert_eq!(closed["structuredContent"]["ok"], true, "{closed}");
    let hint = hints(&closed);
    assert!(hint.contains("gil_open(kind: experiment)") && hint.contains("gil_start를 다시 부르지 않는다"), "{hint}");

    let opened = mcp.call("gil_open", &root, serde_json::json!({"kind": "experiment"}));
    assert_eq!(opened["structuredContent"]["ok"], true, "{opened}");
    assert!(hints(&opened).contains("gil_open(kind: define)"), "{}", hints(&opened));
    mcp.call("gil_open", &root, contract("define"));
    let closed = mcp.call("gil_close", &root, serde_json::json!({"report": report_text(&full_report(&rules, CycleKind::Experiment, NodeKind::Define))}));
    assert!(hints(&closed).contains("다음 행동: gil_open(kind: hypothesis)"), "{}", hints(&closed));
    let opened = mcp.call("gil_open", &root, contract("hypothesis"));
    assert!(hints(&opened).contains("Verify의 gil_open"), "{}", hints(&opened));
    let closed = mcp.call("gil_close", &root, serde_json::json!({"report": report_text(&full_report(&rules, CycleKind::Experiment, NodeKind::Hypothesis))}));
    let hint = hints(&closed);
    assert!(hint.contains("다음 행동: gil_open(kind: verify)") && hint.contains("열기 성공 전에는"), "{hint}");
    assert!(!hint.contains("evidence_files"), "Verify 를 열기 전에 증거를 말했다: {hint}");

    let status = mcp.call("gil_status", &root, serde_json::json!({}));
    assert!(hints(&status).contains("'다음:' 줄 하나"), "{}", hints(&status));
    mcp.done();
    std::fs::remove_dir_all(root).unwrap(); std::fs::remove_dir_all(settings).unwrap();
}
