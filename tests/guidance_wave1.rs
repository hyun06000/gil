//! Wave 1 / D (revised in Wave 2 / C) — 가벼운 모델용 다음 행동 안내 회귀.
//!
//! 상태별 fixture 가 **올바른 다음 호출 하나**를 포함하고 **금지된 지름길**을 제안하지 않는지
//! 잰다. 이것은 안내 계약 검증이지 실제 Luna 실행 결과가 아니다. 기존 legacy Context 의 절은 건드리지
//! 않고, 새로 더한 `[지금 바로 할 호출]` 블록과 tool receipt 안내만 본다.
//!
//! **어느 계약을 재는지 명시한다.** 이 파일의 in-memory 시험은 전부 **legacy 기록**(format 4 ·
//! Chain 없음 · 승인 계획 없음)을 fixture 로 쓴다. 그 기록에서만 Synthesis 의 `approved: yes` 가
//! 사람의 말이다. 승인 계획 프로필의 propose → Monitor 응답 → close_report 순서는
//! `src/context.rs` 의 Core 시험과 `tests/guidance_wave2.rs` 의 stdio 시험이 잰다 — 빌드 프로필이
//! 바뀌어도 여기 시험이 다른 계약을 몰래 재지 않는다.
//!
//! 두 층을 본다.
//! - 라이브러리: `gil::context` 의 첫 블록 (legacy Project · 승인 없는 걷기).
//! - 실제 binary: `gil mcp --serve` 의 receipt 에 붙는 한 줄 안내 (Plugin · 직접 MCP 가 같은 원문).
//!   이 층은 프로필을 기록에서 읽어 두 계약 중 실제 것을 따른다.
use gil::{CycleKind, NodeId, NodeKind, Project};

mod common;
use common::{ACTION, cycle_report, full_report, opened, spec, step_id, up_to_verify, walked};
#[path = "guidance_support/mod.rs"]
mod support;
use support::{Mcp, contract, first_call, hints, legacy, legacy_experiment, ok, refused, report_text, scratch};

/// context 의 첫 블록 — 한 줄이어야 한다.
fn next_call(project: &Project) -> String { first_call(&gil::context(project)) }

fn says(project: &Project, must: &[&str], must_not: &[&str]) {
    let line = next_call(project);
    for m in must { assert!(line.contains(m), "{m:?} 가 없다: {line}"); }
    for n in must_not { assert!(!line.contains(n), "{n:?} 를 제안했다: {line}"); }
}

fn close_cycle_as(project: &mut Project, verdict: &str, outcome: NodeId, action: &str) {
    let report = cycle_report(project.cycles().current(), verdict, outcome).with(ACTION, action);
    project.close_cycle(report).expect("Cycle 을 닫는다");
}

// ── Interview: 새 시작 → Question → Synthesis 대기 → Outcome → Cycle 닫기 ──

#[test]
fn a_fresh_interview_asks_for_the_first_question_not_another_start() {
    // 어느 프로필이든 새 프로젝트의 첫 수는 같다 — 승인 도구를 요구하지 않는다.
    let started = Project::start(spec(), common::first_world());
    says(&started, &["gil_open(kind: question)", "행동 계약"], &["gil_start를 호출", "gil_plan_propose", "experiment"]);
    let legacy = legacy("fresh");
    assert_eq!(next_call(&legacy), next_call(&started), "legacy 기록과 새 기록의 첫 수가 다르다");
}

#[test]
fn open_interview_steps_wait_for_the_user_and_never_research() {
    let mut project = legacy("interview-steps");
    opened(&mut project, NodeKind::Question);
    says(&project, &["질문하고 턴을 끝내", "gil_close"], &["gil_open", "조사한다"]);
    project.close_action_step(full_report(&spec(), CycleKind::Interview, NodeKind::Question)).unwrap();
    says(&project, &["gil_open(kind: interpretation)"], &["synthesis", "outcome"]);
    walked(&mut project, NodeKind::Interpretation);
    let line = next_call(&project);
    assert!(line.contains("question") && line.contains("synthesis"), "{line}");
    opened(&mut project, NodeKind::Synthesis);
    // legacy: 사람의 답을 기다린다 — 승인 계획 도구는 이 기록에 없다.
    says(&project, &["턴을 끝내 답을 기다린다", "명시적으로 동의했을 때만", "침묵"],
        &["gil_plan_propose", "gil_open(kind: outcome)", "조사를 시작"]);
}

#[test]
fn an_approved_synthesis_leads_to_outcome_then_cycle_report_then_experiment() {
    let mut project = legacy("approved-synthesis");
    walked(&mut project, NodeKind::Question);
    walked(&mut project, NodeKind::Interpretation);
    opened(&mut project, NodeKind::Synthesis);
    let cycle = project.cycles().current();
    let basis: Vec<String> = cycle.steps().nodes().iter().filter(|n| n.is_closed())
        .map(|n| cycle.step_ref(n.id).to_string()).collect();
    // legacy 기록에서 `approved: yes` 는 사람의 답이다. 승인 계획 프로필이면 Core 가 이 close 를 거절한다.
    let report = full_report(&spec(), CycleKind::Interview, NodeKind::Synthesis)
        .with(common::APPROVED, common::APPROVAL).with(common::BASIS_REFS, basis.join("\n"));
    project.close_action_step(report).expect("legacy 기록의 approved: yes 는 받아들여진다");
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
    let mut project = legacy_experiment("experiment-walk");
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
    // 되돌아감은 Session 의 문이다(Artifact 복원이 따른다) — 격리 scratch 폴더의 legacy 기록에서만 밟는다.
    let dir = common::scratch(&format!("guidance-failed-revisit-{}", std::process::id()));
    let state = support::legacy_record(&dir);
    std::fs::write(dir.join("work.txt"), "처음").unwrap();
    let mut session = ProjectSession::open(spec(), &state).expect("legacy 기록을 연다");
    support::assert_legacy(session.project());
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
    let mut project = legacy_experiment("legacy-context");
    up_to_verify(&mut project);
    let told = gil::context(&project);
    for section in ["═══ 현재 Cycle ═══", "═══ 지금 자리 ═══", "[지금 하려는 행동]", "[다음에 할 수 있는 것]", "[닫는 데 필요한 칸]"] {
        assert!(told.contains(section), "{section} 절이 사라졌다:\n{told}");
    }
    assert!(told.contains(&gil::next_moves(project.cycles())), "기존 다음 수가 사라졌다");
    assert!(!told.contains("gil_plan"), "legacy 프로젝트에 승인 계획 도구를 요구했다:\n{told}");
    assert!(!told.contains("승인 계획 프로필"), "{told}");
    assert!(!told.contains("현재 Chain"), "legacy 기록에 Chain 절을 만들었다:\n{told}");
    assert_eq!(told.matches("[지금 바로 할 호출]").count(), 1);
}

// ── 실제 binary 의 receipt 안내 — Plugin 과 직접 MCP 가 같은 Core 를 쓴다 ──
//
// 프로필은 **기록과 도구 목록**에서 읽는다(OS 나 빌드 이름이 아니다). 승인 계획 프로필이면 Synthesis 는
// propose → 합성 Monitor 응답 → 정확한 close_report 로 닫히고, 그 밖에서는 `approved: yes` 로 닫힌다.
// 합성 Monitor 응답은 사람의 승인 증거가 아니다 — 안내 계약만 잰다.

#[test]
fn receipts_name_one_next_call_through_interview_and_into_an_experiment() {
    let root = scratch("receipts"); let settings = scratch("receipts-settings");
    let mut mcp = Mcp::serve(&settings);
    let rules = spec();

    let started = ok(mcp.call("gil_start", &root, serde_json::json!({})), "gil_start");
    assert!(hints(&started).contains("show_gil_monitor("), "{}", hints(&started));
    let scope = support::scope_from_start(&started);
    ok(mcp.call_raw("show_gil_monitor", serde_json::json!({"scope_id": scope})), "show_gil_monitor");

    let duplicate = mcp.call("gil_start", &root, serde_json::json!({}));
    assert_eq!(duplicate["isError"], true);
    let hint = hints(&duplicate);
    assert!(hint.contains("gil_start를 반복하지 않는다") && hint.contains("gil_context"), "{hint}");

    // 프로필은 도구 목록과 기록이 함께 말한다 — 둘이 어긋나면 안내도 어긋난다.
    let tools = mcp.tool_names();
    let plan_tools = tools.iter().any(|t| t == "gil_plan_propose");
    let plan_record = plan_tools && mcp.call("gil_plan_read", &root, serde_json::json!({}))["structuredContent"]["supported"] == serde_json::json!(true);
    assert_eq!(plan_tools, plan_record, "도구 목록과 기록의 프로필이 다르다: tools={tools:?}");

    let resumed = mcp.call("gil_context", &root, serde_json::json!({}));
    assert_eq!(resumed["structuredContent"]["ok"], true, "{resumed}");
    assert!(hints(&resumed).contains("[지금 바로 할 호출] 한 줄을 먼저"), "{}", hints(&resumed));
    assert!(first_call(&support::said(&resumed)).contains("gil_open(kind: question)"));

    let opened = mcp.call("gil_open", &root, contract("question"));
    assert_eq!(opened["structuredContent"]["ok"], true, "{opened}");
    let hint = hints(&opened);
    assert!(hint.contains("턴을 끝내 답을 기다린다") && hint.contains("gil_close"), "{hint}");

    let refused_close = mcp.call("gil_close", &root, serde_json::json!({"report": ""}));
    assert_eq!(refused_close["isError"], true);
    let hint = hints(&refused_close);
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
    let basis = vec!["step:C1/S1".to_string(), "step:C1/S2".to_string()];
    let synthesis_report = if plan_record {
        // 승인 계획 프로필: 맨손 approved: yes 는 거절되고 아무것도 닫히지 않는다.
        let bare = full_report(&rules, CycleKind::Interview, NodeKind::Synthesis)
            .with(common::APPROVED, common::APPROVAL).with(common::BASIS_REFS, basis.join("\n"));
        let r = refused(mcp.call("gil_close", &root, serde_json::json!({"report": report_text(&bare)})), "bare approved: yes");
        assert!(hints(&r).contains("닫힌 것은 없다"), "{}", hints(&r));
        let proposed = ok(mcp.call("gil_plan_propose", &root, serde_json::json!({"draft": support::draft(&["A", "B"], &basis)})), "propose");
        let hint = hints(&proposed);
        assert!(hint.contains("제안 판 1") && hint.contains("Monitor 버튼 응답을 기다린다"), "{hint}");
        assert!(!hint.contains("gil_close(Synthesis)"), "{hint}");
        let context = mcp.call("gil_context", &root, serde_json::json!({}));
        let line = first_call(&support::said(&context));
        assert!(line.contains("제안 판 1") && line.contains("기다린다"), "context 와 receipt 가 어긋난다: {line}");
        // 합성 Monitor 응답 — App 전용 도구와 _meta 능력으로만 기록된다.
        let review = ok(mcp.call_raw("gil_plan_review", serde_json::json!({"scope_id": scope})), "review");
        let token = review["_meta"]["gil/planApprovalToken"].as_str().expect("capability in _meta").to_owned();
        let digest = review["structuredContent"]["proposal"]["digest"].clone();
        ok(mcp.call_raw("gil_plan_decide", serde_json::json!({"scope_id": scope, "revision": 1, "digest": digest, "token": token, "decision": "approve"})), "approve");
        let read = ok(mcp.call("gil_plan_read", &root, serde_json::json!({})), "read");
        let hint = hints(&read);
        assert!(hint.contains("판 1이(가) 승인됐다") && hint.contains("close_report를 글자 그대로"), "{hint}");
        read["structuredContent"]["close_report"].as_str().expect("close_report after approval").to_owned()
    } else {
        // legacy / Chain-without-plan: 사람의 답이 Report 에 적힌다.
        report_text(&full_report(&rules, CycleKind::Interview, NodeKind::Synthesis)
            .with(common::APPROVED, common::APPROVAL).with(common::BASIS_REFS, basis.join("\n")))
    };
    let closed = mcp.call("gil_close", &root, serde_json::json!({"report": synthesis_report}));
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
