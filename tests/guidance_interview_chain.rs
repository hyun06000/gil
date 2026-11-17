//! Chain first (2026-11-17, Cycle Model §5.1 · §16 · §7.1) over the real binary: the Question,
//! Interpretation, Synthesis and Define receipts, the Host hints prepended to them and the fresh
//! `gil_context` first line carry the deep-interview, short-proposal and smallest-problem guidance,
//! in the approved-plan profile and in the legacy Synthesis alike. The profile is read from the
//! connected tool catalog and the record, never from the OS or the build name.
//!
//! This is a guidance-contract check: it proves the sentences reach the model at the Step where
//! they apply and that the recorded next call is unchanged. It does not measure question depth,
//! proposal clarity or experiment size — those need a model/user evaluation — and it adds no Core
//! rule about problem size (a long Define contract is still accepted).
use gil::{CycleKind, Locale, NodeKind, locale::text_in};
use serde_json::json;

mod common;
use common::{full_report, spec};
#[path = "guidance_support/mod.rs"]
mod support;
use support::{Mcp, contract, draft, first_call, hints, ok, report_text, said, scratch, scope_from_start};

fn ko(key: &str) -> &'static str { text_in(Locale::Ko, key) }

/// The Core note, indented in the receipt body, as one contiguous text.
fn note(key: &str) -> String { ko(key).trim().to_string() }

#[test]
fn interview_proposal_and_define_receipts_carry_the_deep_interview_and_smallest_problem_guidance() {
    let root = scratch("interview-chain-root"); let settings = scratch("interview-chain-settings");
    let mut mcp = Mcp::serve(&settings);
    let rules = spec();

    let started = ok(mcp.call("gil_start", &root, json!({})), "gil_start");
    support::as_item_queue(&root, &settings);
    let scope = scope_from_start(&started);
    ok(mcp.call_raw("show_gil_monitor", json!({"scope_id": scope})), "show_gil_monitor");
    let plan_profile = mcp.tool_names().iter().any(|t| t == "gil_plan_propose");

    // Question: receipt body, Host hint and fresh context all deepen the interview.
    let opened = ok(mcp.call("gil_open", &root, contract("question")), "question");
    assert!(said(&opened).contains(&note("cmd.opened_question_note")), "{}", said(&opened));
    assert!(hints(&opened).contains(ko("guide.interview_deepen")), "{}", hints(&opened));
    let line = first_call(&said(&ok(mcp.call("gil_context", &root, json!({})), "context")));
    assert!(line.contains("열린 Question") && line.contains(ko("guide.interview_deepen")), "{line}");
    let mut basis = vec![support::step_ref(&opened, "question")];
    ok(mcp.call("gil_close", &root, json!({"report": report_text(&full_report(&rules, CycleKind::Interview, NodeKind::Question))})), "close question");

    // Interpretation: confirmed facts, the agent's reading and the unknown are kept apart; a
    // remaining ambiguity leads back to a Question, which the grammar allows.
    let opened = ok(mcp.call("gil_open", &root, contract("interpretation")), "interpretation");
    assert!(said(&opened).contains(&note("cmd.opened_interpretation_note")), "{}", said(&opened));
    assert!(hints(&opened).contains(ko("guide.interpretation_sort")), "{}", hints(&opened));
    let line = first_call(&said(&ok(mcp.call("gil_context", &root, json!({})), "context")));
    assert!(line.contains("열린 Interpretation") && line.contains(ko("guide.interpretation_sort")), "{line}");
    basis.push(support::step_ref(&opened, "interpretation"));
    let closed = ok(mcp.call("gil_close", &root, json!({"report": report_text(&full_report(&rules, CycleKind::Interview, NodeKind::Interpretation))})), "close interpretation");
    let after = hints(&closed);
    assert!(after.contains("question") && after.contains("synthesis"), "Interpretation → Question stays a legal move: {after}");

    // Synthesis: the proposal is short with small items, in the receipt, the hint and the context.
    let opened = ok(mcp.call("gil_open", &root, contract("synthesis")), "synthesis");
    let synthesis = support::step_ref(&opened, "synthesis");
    assert!(said(&opened).contains(&note("cmd.opened_synthesis_note")), "{}", said(&opened));
    let hint = hints(&opened);
    assert!(hint.contains(ko("guide.propose_brief")), "{hint}");
    assert!(hint.find(ko("guide.propose_brief")).unwrap() < hint.find("$.draft.synthesis.basis_refs").unwrap(), "the short proposal precedes the basis rules: {hint}");
    let line = first_call(&said(&ok(mcp.call("gil_context", &root, json!({})), "context")));
    assert!(line.contains(ko("guide.propose_brief")), "{line}");
    if plan_profile {
        assert!(line.contains("gil_plan_propose") && line.contains(ko("cycle.basis_refs_rules")), "{line}");
    } else {
        assert!(!line.contains("gil_plan_propose"), "{line}");
    }
    let synthesis_report = if plan_profile {
        ok(mcp.call("gil_plan_propose", &root, json!({"draft": draft(&["A", "B"], &basis)})), "propose");
        let review = ok(mcp.call_raw("gil_plan_review", json!({"scope_id": scope})), "review");
        let token = review["_meta"]["gil/planApprovalToken"].as_str().expect("capability in _meta").to_owned();
        let digest = review["structuredContent"]["proposal"]["digest"].clone();
        ok(mcp.call_raw("gil_plan_decide", json!({"scope_id": scope, "revision": 1, "digest": digest, "token": token, "decision": "approve"})), "approve");
        let read = ok(mcp.call("gil_plan_read", &root, json!({})), "read");
        read["structuredContent"]["close_report"].as_str().expect("close_report").to_owned()
    } else {
        report_text(&full_report(&rules, CycleKind::Interview, NodeKind::Synthesis)
            .with(common::APPROVED, common::APPROVAL).with(common::BASIS_REFS, basis.join("\n")))
    };
    ok(mcp.call("gil_close", &root, json!({"report": synthesis_report})), "close synthesis");
    let outcome = support::step_ref(&ok(mcp.call("gil_open", &root, contract("outcome")), "outcome"), "outcome");
    ok(mcp.call("gil_close", &root, json!({"report": report_text(&full_report(&rules, CycleKind::Interview, NodeKind::Outcome).with(common::SYNTHESIS_REF, &synthesis))})), "close outcome");
    ok(mcp.call("gil_close", &root, json!({"report": format!("verdict: success\noutcome_ref: {outcome}\nhandoff_summary: 합의한 범위를 실행한다\nnext_direction.action: open_child\nnext_direction.reason: 첫 항목을 확인한다\n")})), "close interview cycle");

    // Experiment: the bound item (approved-plan profile) is what the Define must point at.
    let opened = ok(mcp.call("gil_open", &root, json!({"kind": "experiment"})), "open experiment");
    if plan_profile {
        assert!(said(&opened).contains("Define은 항목 A과 같은 문제·완료 범위를 가리킨다"), "{}", said(&opened));
        let read = ok(mcp.call("gil_plan_read", &root, json!({})), "read bound item");
        assert_eq!(read["structuredContent"]["current_item"]["id"], "A", "{read}");
        assert!(hints(&read).contains("Define은 항목 A과 같은 문제·완료 범위"), "{}", hints(&read));
    } else {
        assert!(!said(&opened).contains("승인 계획 실행 연결"), "{}", said(&opened));
    }
    assert!(hints(&opened).contains("gil_open(kind: define)"), "{}", hints(&opened));

    // Define: the smallest problem performable now, in the receipt, the hint and the context.
    let opened = ok(mcp.call("gil_open", &root, contract("define")), "define");
    assert!(said(&opened).contains(&note("cmd.opened_define_note")), "{}", said(&opened));
    assert!(hints(&opened).contains(ko("guide.define_smallest")), "{}", hints(&opened));
    let line = first_call(&said(&ok(mcp.call("gil_context", &root, json!({})), "context")));
    assert!(line.contains("열린 Define") && line.contains(ko("guide.define_smallest")) && line.contains("gil_close"), "{line}");
    // No size rule was added: the Define closes on its grammar alone.
    ok(mcp.call("gil_close", &root, json!({"report": report_text(&full_report(&rules, CycleKind::Experiment, NodeKind::Define))})), "close define");

    mcp.done();
    std::fs::remove_dir_all(root).ok(); std::fs::remove_dir_all(settings).ok();
}
