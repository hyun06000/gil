//! Synthetic record/restart checks. App decisions below are fixture input, not human or Host acceptance.
mod common;
#[path = "guidance_support/mod.rs"]
mod support;
use gil::{CycleKind, Locale, NodeKind, Project, locale};
#[cfg(feature = "approved-plan")]
use gil::Report;
use serde_json::json;
use common::{full_report, opened, spec};
#[cfg(feature = "approved-plan")]
use common::walked;

/// Round-trip the real store, render every locale, and read with two fresh MCP processes.
/// Context and plan reads must preserve every stored byte, including receipts and bindings.
/// A synthetic external file change between processes cannot silently invalidate an approval.
fn checkpoint(p: &Project, label: &str, required: &[&str], forbidden: &[&str]) {
    let root = common::scratch(&format!("resume-{label}"));
    let settings = support::scratch(&format!("resume-settings-{label}"));
    let path = root.join(gil::STATE_PATH);
    gil::save(p, &path).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let restored = gil::load(spec(), &path).unwrap();
    for language in Locale::ALL {
        locale::with(language, || {
            let before = gil::context(p);
            let after = gil::context(&restored);
            assert_eq!(before, after, "{label}/{language}");
            for key in ["ctx.resume.heading", "ctx.resume.details"] {
                assert!(after.contains(locale::text(key)), "{label}/{language}: {after}");
            }
            let heading = format!("{}\n", locale::text("context.next_call_heading"));
            let next = after.split_once(&heading).unwrap().1.lines().next().unwrap();
            if language == Locale::Ko {
                for expected in required { assert!(after.contains(expected), "{label} missing {expected}: {after}"); }
                for excluded in forbidden { assert!(!next.contains(excluded), "{label} next includes {excluded}: {next}"); }
            }
            if let Some(proposal) = p.approved_plan().and_then(|plan| plan.current(p)) {
                assert!(after.contains(&proposal.digest) && after.contains(&proposal.synthesis_ref));
                let key = match proposal.receipt.as_ref().map(|r| &r.decision) {
                    None => "ctx.resume.waiting",
                    Some(gil::approved_plan::Decision::Approve) => "ctx.resume.approved",
                    Some(gil::approved_plan::Decision::RequestChanges) => "ctx.resume.changes",
                };
                assert!(after.contains(locale::text(key)), "{language}: {after}");
                assert!(!after.contains(locale::text("moves.close_here")), "generic close contradicts the plan state");
            }
        });
    }
    let mut previous = None;
    for attempt in 0..2 {
        let mut mcp = support::Mcp::serve(&settings);
        let response = support::ok(mcp.call("gil_context", &root, json!({})), label);
        let actual = support::said(&response);
        assert!(actual.starts_with(&gil::context(&restored)), "{label}: {actual}");
        if let Some(previous) = previous { assert_eq!(actual, previous, "{label}: process restart"); }
        previous = Some(actual);
        if p.approved_plan().is_some() {
            let read = support::ok(mcp.call("gil_plan_read", &root, json!({})), label);
            let next = read["structuredContent"]["next"].as_str().unwrap().lines().next().unwrap();
            assert!(read["content"][0]["text"].as_str().unwrap().starts_with(next), "{label}: {read}");
        }
        mcp.done();
        assert_eq!(std::fs::read(&path).unwrap(), bytes, "{label}: read mutated records");
        if attempt == 0 {
            std::fs::write(root.join("external-file-change.txt"), "synthetic change while disconnected").unwrap();
        }
    }
    gil::save(&restored, &path).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), bytes, "{label}: roundtrip changed records");
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(settings).unwrap();
}

#[test]
fn unanswered_question_resumes_without_inventing_delivery_or_requiring_plan_tools_on_legacy() {
    let mut p = support::legacy("resume-question");
    opened(&mut p, NodeKind::Question);
    checkpoint(&p, "question", &["응답 대상 step:C1/S1", "답변은 아직 기록되지 않았다", "실제 사용자 답변을 이미 받았다면", "아직 닫힌 결과가 없다"], &["gil_plan", "gil_close로"]);
    assert!(!gil::context(&p).contains("gil_plan"));
    assert!(!gil::context(&p).contains(locale::text("moves.close_here")));
    p.close_action_step(full_report(&spec(), CycleKind::Interview, NodeKind::Question).with("response", "A real synthetic user answer")).unwrap();
    checkpoint(&p, "answered", &["마지막 닫힌 결과: step:C1/S1", "A real synthetic user answer", "gil_open(kind: interpretation)"], &["기다린다"]);
}

#[test]
fn context_routes_file_observation_and_failed_reads_never_reuse_a_stale_success() {
    let root = common::scratch("resume-observation");
    let settings = support::scratch("resume-observation-settings");
    let path = support::legacy_record(&root);
    let bytes = std::fs::read(&path).unwrap();
    let mut mcp = support::Mcp::serve(&settings);
    let before = support::said(&support::ok(mcp.call("gil_context", &root, json!({})), "context"));
    std::fs::write(root.join("external-change.txt"), "synthetic external change").unwrap();
    let after = support::said(&support::ok(mcp.call("gil_context", &root, json!({})), "changed context"));
    assert_eq!(before, after, "context is a record projection, not a file watcher");
    assert!(after.contains(locale::text("ctx.resume.details")));
    let status = support::said(&support::ok(mcp.call("gil_status", &root, json!({})), "dirty status"));
    assert!(status.contains("상태: dirty"), "{status}");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("external-change.txt", root.join("unobservable-link")).unwrap();
        let status = support::said(&support::ok(mcp.call("gil_status", &root, json!({})), "unknown status"));
        assert!(status.contains("상태: 확인하지 못했다"), "{status}");
        assert!(!status.contains("· clean"), "{status}");
    }
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    // Corruption is synthetic and local to this fixture. It must not become a fresh resume.
    std::fs::write(&path, "synthetic unreadable record").unwrap();
    let failed = support::refused(mcp.call("gil_context", &root, json!({})), "failed context");
    assert!(!support::hints(&failed).contains("[저장 기록으로 재개]"));
    assert!(!support::hints(&failed).contains("GIL 재개:"));
    #[cfg(feature = "approved-plan")]
    {
        let failed = support::refused(mcp.call("gil_plan_read", &root, json!({})), "failed plan read");
        assert!(support::hints(&failed).contains(locale::text("guide.refused.plan_read")));
    }
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "synthetic unreadable record");
    mcp.done();
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(settings).unwrap();
}

#[cfg(feature = "approved-plan")]
fn draft(p: &Project, closing: bool) -> gil::approved_plan::Draft {
    let cycle = p.current_cycle();
    let basis = cycle.steps().nodes().iter().filter(|n| n.is_closed() && matches!(n.kind, NodeKind::Question | NodeKind::Interpretation))
        .map(|n| cycle.step_ref(n.id).to_string()).collect::<Vec<_>>();
    let mut value = support::draft(&["A"], &basis);
    if closing { value["synthesis"]["chain_verdict"] = json!("success"); }
    serde_json::from_value(value).unwrap()
}

#[cfg(feature = "approved-plan")]
fn synthesis(p: &mut Project) {
    for kind in [NodeKind::Question, NodeKind::Interpretation] { walked(p, kind); }
    opened(p, NodeKind::Synthesis);
}

#[cfg(feature = "approved-plan")]
fn close_cycle(p: &mut Project, action: &str) {
    let last = p.current_cycle().steps().current().unwrap();
    p.close_cycle(common::cycle_report(p.current_cycle(), "success", last).with("next_direction.action", action)).unwrap();
}

#[cfg(feature = "approved-plan")]
fn decide(p: &mut Project, proposal: &gil::approved_plan::Proposal, decision: gil::approved_plan::Decision) -> gil::approved_plan::Proposal {
    // Use the existing App-only wire path, not a new public approval API for this test.
    let root = common::scratch("resume-decision");
    let settings = support::scratch("resume-decision-settings");
    let path = root.join(gil::STATE_PATH);
    gil::save(p, &path).unwrap();
    let mut mcp = support::Mcp::serve(&settings);
    let prepared = support::ok(mcp.call("gil_monitor_prepare", &root, json!({})), "prepare");
    let scope = prepared["structuredContent"]["scope_id"].as_str().unwrap();
    let review = support::ok(mcp.call_raw("gil_plan_review", json!({"scope_id": scope})), "review");
    let token = review["_meta"]["gil/planApprovalToken"].as_str().unwrap();
    support::ok(mcp.call_raw("gil_plan_decide", json!({
        "scope_id": scope, "revision": proposal.revision, "digest": proposal.digest,
        "token": token, "decision": decision
    })), "synthetic decision");
    mcp.done();
    // The continuation gets no decision response: recover only the saved receipt.
    *p = gil::load(spec(), &path).unwrap();
    let saved = p.approved_plan().unwrap().current(p).unwrap().clone();
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(settings).unwrap();
    saved
}

#[cfg(feature = "approved-plan")]
#[test]
fn saved_decisions_execution_and_chain_boundaries_survive_new_sessions() {
    use gil::approved_plan::Decision;
    let mut p = support::item_queue(Project::start(spec(), common::first_world()));
    synthesis(&mut p);
    let proposal = p.propose_plan(draft(&p, false)).unwrap();
    checkpoint(&p, "waiting", &["Monitor 결정을 기다림", "제안 판 1", "턴을 끝내고"], &["gil_close", "gil_plan_propose"]);
    let first_approval = decide(&mut p, &proposal, Decision::Approve);
    // Lost approval response: a fresh reader sees the persisted receipt, never another wait.
    checkpoint(&p, "approved-open", &["승인 저장됨", "close_report를 그대로 gil_close", &first_approval.digest], &["Monitor 버튼 응답을 기다린다", "gil_plan_propose"]);
    // Editing creates a new revision; the saved approval remains in history.
    let mut edited = draft(&p, false); edited.scope.goal = "Explicitly revised scope".into();
    let proposal = p.propose_plan(edited).unwrap();
    assert_eq!(p.approved_plan().unwrap().proposals[0], first_approval);
    checkpoint(&p, "edited-waiting", &["제안 판 2", "Monitor 결정을 기다림"], &["승인됐다", "close_report"]);
    let changed = decide(&mut p, &proposal, Decision::RequestChanges);
    checkpoint(&p, "changes", &["수정 요청 저장됨", "approved: no", "다음 Question"], &["gil_open(kind: outcome)"]);
    p.close_action_step(changed.report().unwrap()).unwrap();
    synthesis(&mut p);
    let proposal = p.propose_plan(draft(&p, false)).unwrap();
    let approved = decide(&mut p, &proposal, Decision::Approve);
    let target = approved.synthesis_ref.clone();
    p.close_action_step(approved.report().unwrap()).unwrap();
    checkpoint(&p, "synthesis-closed", &["gil_open(kind: outcome)", "승인을 다시 묻거나"], &["gil_plan_propose로", "gil_close"]);
    opened(&mut p, NodeKind::Outcome);
    p.close_action_step(full_report(&spec(), CycleKind::Interview, NodeKind::Outcome).with("synthesis_ref", target)).unwrap();
    checkpoint(&p, "interview-open", &["Interview Cycle은 아직 열려 있다", "gil_close로 Cycle Report"], &["gil_open(kind: experiment)"]);
    close_cycle(&mut p, "open_child");
    p.open_child_cycle(CycleKind::Experiment).unwrap();
    checkpoint(&p, "experiment-open", &["실행 바인딩: 항목 A", "gil_open(kind: define)"], &["gil_plan_propose", "gil_close"]);
    opened(&mut p, NodeKind::Define);
    checkpoint(&p, "define-open", &["열린 Define", "gil_close", "실행 바인딩: 항목 A"], &["gil_open", "재인터뷰 요청이 보존"]);
    p.request_plan_interview("Synthetic scope clarification".into()).unwrap();
    checkpoint(&p, "reinterview-open", &["열린 Define", "재인터뷰 요청이 보존", "Synthetic scope clarification"], &["gil_open(kind: interview)", "gil_open(kind: experiment)"]);
    p.close_action_step(full_report(&spec(), CycleKind::Experiment, NodeKind::Define)).unwrap();
    checkpoint(&p, "reinterview-between", &["gil_open(kind: hypothesis)", "재인터뷰 요청이 보존"], &["gil_open(kind: interview)", "gil_close"]);
    walked(&mut p, NodeKind::Hypothesis);
    opened(&mut p, NodeKind::Verify);
    checkpoint(&p, "verify", &["열린 Verify", "plan_document", "evidence_files"], &["gil_open"]);
    p.close_verify_step(full_report(&spec(), CycleKind::Experiment, NodeKind::Verify), common::first_world()).unwrap();
    checkpoint(&p, "verify-closed", &["gil_open(kind: analysis)"], &["gil_close", "gil_open(kind: verify)"]);
    for kind in [NodeKind::Analysis, NodeKind::Outcome] { walked(&mut p, kind); }
    close_cycle(&mut p, "open_child");
    checkpoint(&p, "experiment-closed", &["gil_open(kind: interview)", "닫힌 Experiment의 실행 연결"], &["gil_open(kind: experiment)", "열린 Verify"]);
    p.open_child_cycle(CycleKind::Interview).unwrap();
    synthesis(&mut p);
    let proposal = p.propose_plan(draft(&p, true)).unwrap();
    let closing = decide(&mut p, &proposal, Decision::Approve);
    let target = closing.synthesis_ref.clone();
    p.close_action_step(closing.report().unwrap()).unwrap();
    opened(&mut p, NodeKind::Outcome);
    p.close_action_step(full_report(&spec(), CycleKind::Interview, NodeKind::Outcome).with("synthesis_ref", &target)).unwrap();
    checkpoint(&p, "closing-cycle-open", &["gil_close로 Cycle Report", "Closing Interview 뒤에는 Chain Report"], &["Experiment는 그 뒤에"]);
    close_cycle(&mut p, "close_chain");
    checkpoint(&p, "chain-report", &["Chain Report", "closing_synthesis_ref"], &["gil_open"]);
    p.close_chain(Report::new().with("verdict", "success").with("closing_synthesis_ref", target).with("handoff_summary", "Synthetic final handoff")).unwrap();
    checkpoint(&p, "chain-closed", &["마지막 닫힌 결과: chain:C1", "Synthetic final handoff", "gil_open(kind: chain)"], &["gil_close", "gil_open(kind: experiment)", "gil_open(kind: interview)"]);
    assert_eq!(p.approved_plan().unwrap().proposals[0], first_approval);
}
