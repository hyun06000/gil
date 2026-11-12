//! Wave 2 / C — R4 over the real binary: a preserved re-interview request must not hide the
//! legal next call, and `gil_context`'s first line must agree with the receipt that preceded it.
//!
//! The profile is read from the connected tool catalog and the record, never from the OS or the
//! build name. In the approved-plan profile the Interview is approved the only way Core accepts:
//! `gil_plan_propose` → synthetic Monitor decision (App tool + `_meta` capability) → the exact
//! `close_report` → Outcome → Interview Cycle Report. Elsewhere the human's `approved: yes` is the
//! Report. Both branches check the same thing: after every close, context and receipt name the
//! same next call. The synthetic decision is harness input, never evidence of a person's consent
//! or of a visible screen. This is a guidance-contract check, not a model run.
use gil::{CycleKind, NodeKind};
use serde_json::json;

mod common;
use common::{full_report, spec};
#[path = "guidance_support/mod.rs"]
mod support;
use support::{Mcp, contract, draft, first_call, hints, ok, refused, report_text, said, scratch, scope_from_start};

/// The receipt's `다음 행동` and the fresh context's first line must name the same `gil_open` kind.
fn agree(mcp: &mut Mcp, root: &std::path::Path, receipt: &serde_json::Value, kind: &str) -> String {
    let hint = hints(receipt);
    assert!(hint.contains(&format!("gil_open(kind: {kind})")), "receipt does not name {kind}: {hint}");
    let context = ok(mcp.call("gil_context", root, json!({})), "gil_context");
    let line = first_call(&said(&context));
    assert!(line.contains(&format!("gil_open(kind: {kind})")), "context first line disagrees with the receipt ({kind}): {line}");
    for other in ["question", "interpretation", "synthesis", "outcome", "define", "hypothesis", "verify", "analysis", "experiment", "interview"] {
        if other != kind { assert!(!line.contains(&format!("gil_open(kind: {other})")), "context offers a second call: {line}"); }
    }
    line
}

#[test]
fn a_preserved_reinterview_request_never_hides_the_legal_next_call() {
    let root = scratch("wave2-root"); let settings = scratch("wave2-settings");
    let mut mcp = Mcp::serve(&settings);
    let rules = spec();

    let started = ok(mcp.call("gil_start", &root, json!({})), "gil_start");
    let scope = scope_from_start(&started);
    ok(mcp.call_raw("show_gil_monitor", json!({"scope_id": scope})), "show_gil_monitor");
    let plan_profile = mcp.tool_names().iter().any(|t| t == "gil_plan_propose");
    if plan_profile {
        let read = ok(mcp.call("gil_plan_read", &root, json!({})), "gil_plan_read");
        assert_eq!(read["structuredContent"]["supported"], json!(true), "tools offer gil_plan_* but the record is not a plan profile: {read}");
        let context = ok(mcp.call("gil_context", &root, json!({})), "gil_context");
        assert!(said(&context).contains("승인 계획 프로필"), "{}", said(&context));
    }

    // Interview to an approved Synthesis.
    let mut basis = Vec::new();
    for (kind, node) in [("question", NodeKind::Question), ("interpretation", NodeKind::Interpretation)] {
        let opened = ok(mcp.call("gil_open", &root, contract(kind)), kind);
        basis.push(support::step_ref(&opened, kind));
        ok(mcp.call("gil_close", &root, json!({"report": report_text(&full_report(&rules, CycleKind::Interview, node))})), kind);
    }
    let synthesis = support::step_ref(&ok(mcp.call("gil_open", &root, contract("synthesis")), "synthesis"), "synthesis");
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
    let closed = ok(mcp.call("gil_close", &root, json!({"report": synthesis_report})), "close synthesis");
    agree(&mut mcp, &root, &closed, "outcome");
    let outcome = support::step_ref(&ok(mcp.call("gil_open", &root, contract("outcome")), "outcome"), "outcome");
    let closed = ok(mcp.call("gil_close", &root, json!({"report": report_text(&full_report(&rules, CycleKind::Interview, NodeKind::Outcome).with(common::SYNTHESIS_REF, &synthesis))})), "close outcome");
    assert!(hints(&closed).contains("Interview Cycle Report"), "{}", hints(&closed));
    let line = first_call(&said(&ok(mcp.call("gil_context", &root, json!({})), "context")));
    assert!(line.contains("Interview Cycle은 아직 열려 있다") && line.contains("gil_close"), "{line}");
    let closed = ok(mcp.call("gil_close", &root, json!({"report": format!("verdict: success\noutcome_ref: {outcome}\nhandoff_summary: 합의한 범위를 실행한다\nnext_direction.action: open_child\nnext_direction.reason: 첫 항목을 확인한다\n")})), "close interview cycle");
    agree(&mut mcp, &root, &closed, "experiment");

    // Experiment: Define open.
    let opened = ok(mcp.call("gil_open", &root, json!({"kind": "experiment"})), "open experiment");
    agree(&mut mcp, &root, &opened, "define");
    if plan_profile {
        let read = ok(mcp.call("gil_plan_read", &root, json!({})), "read bound item");
        assert_eq!(read["structuredContent"]["current_item"]["id"], "A", "{read}");
        assert!(hints(&read).contains("항목 A"), "{}", hints(&read));
    }
    ok(mcp.call("gil_open", &root, contract("define")), "define");

    if plan_profile {
        // The request is preserved and closes nothing; the open Define still has its own next call.
        let requested = ok(mcp.call("gil_plan_request_interview", &root, json!({"reason": "범위가 바뀌었다"})), "request interview");
        let hint = hints(&requested);
        assert!(hint.contains("재인터뷰 요청을 보존했다") && hint.contains("아무것도 닫히거나 열리지 않았다"), "{hint}");
        assert!(!hint.contains("gil_open(kind: experiment)"), "{hint}");
        let line = first_call(&said(&ok(mcp.call("gil_context", &root, json!({})), "context")));
        assert!(line.contains("열린 Define") && line.contains("gil_close"), "the open Define lost its next call: {line}");
        assert!(line.contains("재인터뷰 요청이 보존"), "the preserved request is not announced: {line}");
        assert!(!line.contains("gil_open(kind: interview)") && !line.contains("gil_open(kind: experiment)"), "{line}");
        refused(mcp.call("gil_open", &root, json!({"kind": "interview"})), "interview while Define is open");
    }

    // R4: Define closed → the only legal call is Hypothesis; the Cycle cannot be closed yet.
    let closed = ok(mcp.call("gil_close", &root, json!({"report": report_text(&full_report(&rules, CycleKind::Experiment, NodeKind::Define))})), "close define");
    let line = agree(&mut mcp, &root, &closed, "hypothesis");
    assert!(!line.contains("Cycle Report") && !line.contains("Cycle을 사실대로 닫는다"), "proposes a Cycle close that Core would refuse: {line}");
    if plan_profile {
        assert!(line.contains("재인터뷰 요청이 보존"), "{line}");
        let read = ok(mcp.call("gil_plan_read", &root, json!({})), "read with request");
        let hint = hints(&read);
        assert!(hint.contains("재인터뷰 요청이 보존") && !hint.contains("gil_open(kind: experiment)"), "{hint}");
        // Refuse the shortcut the field model took: a Cycle Report here is not accepted.
        let r = refused(mcp.call("gil_close", &root, json!({"report": "verdict: success\noutcome_ref: step:C2/S1\nhandoff_summary: x\nnext_direction.action: open_child\nnext_direction.reason: y\n"})), "cycle close before Outcome");
        assert!(hints(&r).contains("닫힌 것은 없다"), "{}", hints(&r));
    } else {
        assert!(!line.contains("재인터뷰") && !line.contains("gil_plan"), "{line}");
    }

    ok(mcp.call("gil_open", &root, contract("hypothesis")), "hypothesis");
    let closed = ok(mcp.call("gil_close", &root, json!({"report": report_text(&full_report(&rules, CycleKind::Experiment, NodeKind::Hypothesis))})), "close hypothesis");
    let line = agree(&mut mcp, &root, &closed, "verify");
    assert!(line.contains("열기 성공 전에는"), "{line}");
    let opened = ok(mcp.call("gil_open", &root, contract("verify")), "verify");
    assert!(hints(&opened).contains("evidence_files"), "{}", hints(&opened));
    let line = first_call(&said(&ok(mcp.call("gil_context", &root, json!({})), "context")));
    assert!(line.contains("열린 Verify") && line.contains("evidence_files"), "{line}");
    if plan_profile {
        assert!(line.contains("plan_document") && line.contains("재인터뷰 요청이 보존"), "{line}");
        refused(mcp.call("gil_open", &root, json!({"kind": "interview"})), "interview while Verify is open");
    } else {
        assert!(!line.contains("plan_document"), "{line}");
    }
    mcp.done();
    std::fs::remove_dir_all(root).ok(); std::fs::remove_dir_all(settings).ok();
}

// Both macOS and Windows CI already execute this guidance suite in the approval profile.
// Language is presentation only, including after a real MCP process restart.
mod locale_preservation {
use super::{common, support};
use support::{Mcp, hints, ok, said, scope_from_start, scratch};
use std::path::{Path, PathBuf};
use std::collections::BTreeMap;
use gil::{Locale, locale::text_in};
use serde_json::{Value, json};

fn speaks(result: &Value, locale: Locale) -> bool {
    let all = format!("{}\n{}", hints(result), said(result));
    all.contains(text_in(locale, "guide.file_boundary")) || all.contains(text_in(locale, "guide.start"))
}

fn files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(path: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        if !path.exists() { return; }
        for entry in std::fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            assert!(!entry.file_type().unwrap().is_symlink());
            if path.is_dir() { visit(&path, out); }
            else if path.file_name().unwrap() != gil::locale::MEMORY_FILE {
                out.insert(path.clone(), std::fs::read(path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new(); visit(root, &mut out); out
}

fn project_bytes(root: &Path, settings: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = files(root);
    out.extend(files(&settings.join("LocalAppData")));
    out
}

#[test]
fn locale_switch_and_restart_preserve_historical_and_current_records_snapshots_and_guides() {
    for historical in [true, false] {
        let settings = scratch(if historical { "preserve-old-settings" } else { "preserve-new-settings" });
        let root = scratch(if historical { "preserve-old-project" } else { "preserve-new-project" });
        if historical {
            // Real empty Snapshot objects plus a synthetic format-4 record; no user data.
            common::sow_world(&root);
            support::legacy_record(&root);
        }
        std::fs::write(root.join("report.md"), "Existing report bytes.\n").unwrap();
        std::fs::write(root.join("AGENTS.md"), "Existing Codex guidance.\n").unwrap();
        std::fs::write(root.join("CLAUDE.md"), "Existing Claude guidance.\n").unwrap();
        let mut mcp = Mcp::serve(&settings);
        let scope = if historical {
            let prepared = ok(mcp.call("gil_monitor_prepare", &root, json!({})), "prepare legacy");
            prepared["structuredContent"]["scope_id"].as_str().unwrap().to_owned()
        } else { scope_from_start(&ok(mcp.call("gil_start", &root, json!({})), "start")) };
        let before = project_bytes(&root, &settings);
        assert!(before.keys().any(|p| p.file_name().unwrap() == "state.yaml"));
        assert!(before.keys().any(|p| p.components().any(|c| c.as_os_str() == "manifests")), "real Snapshot objects");
        for (n, tag, language) in [(1_u32, "en-US", Locale::En), (2, "zh-TW", Locale::ZhHant), (3, "ja-JP", Locale::Ja)] {
            ok(mcp.call_raw("gil_monitor_poll", json!({"scope_id": scope, "instance_id": "locale-preservation", "sequence": n.to_string(), "presentation": "inline", "locale": tag})), "locale poll");
            let context = ok(mcp.call("gil_context", &root, json!({})), "context");
            assert!(speaks(&context, language));
            assert_eq!(project_bytes(&root, &settings), before, "locale must not rewrite records, objects or user guidance");
        }
        mcp.done();
        let mut restarted = Mcp::serve(&settings);
        assert!(speaks(&ok(restarted.call("gil_context", &root, json!({})), "restart context"), Locale::Ja));
        assert_eq!(project_bytes(&root, &settings), before);
        restarted.done();
    }
}

}
