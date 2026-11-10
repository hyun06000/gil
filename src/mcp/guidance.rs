//! Short host guidance shared by Plugin and direct-MCP installations.
//! These are instructions, not new Project facts or structured receipt fields.
//!
//! Each hint states **one next call or one wait first**, read from the receipt that Core already
//! produced (`structuredContent.said` for text actions, typed fields for plan tools). Nothing
//! here recomputes Project state or promises that a screen is visible. When a receipt shape is
//! not recognised, the generic sentence for that tool is used instead of guessing.
//!
//! Every sentence comes from the language tables (`src/i18n/*.json`, keys `guide.*`) in the
//! current locale. The receipt markers this module parses (`receipt.*`) come from the same
//! tables, so a receipt and its reader always speak the same language.
use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::Value;

use crate::locale::text;
use crate::tr;

// The Plugin skill embeds this exact text; CI rejects a stale generated copy.
pub const INSTRUCTIONS: &str = include_str!("workflow.md");

pub fn prepend(result: &mut CallToolResult, text: impl Into<String>) {
    result.content.insert(0, ContentBlock::text(text.into()));
}

pub fn after(name: &str, result: &mut CallToolResult) {
    if result.is_error == Some(true) {
        if let Some(hint) = refused(name) {
            let problem = problem(result);
            // A dirty refusal of a close has one more real recovery than "fix the input".
            let hint = if name == "gil_close" && (problem.contains(text("marker.dirty_world")) || problem.contains("dirty")) {
                format!("{hint} {}", dirty_close_recovery())
            } else { hint.to_string() };
            if name == "gil_plan_propose" {
                result.content.push(ContentBlock::text(hint));
            } else {
                prepend(result, hint);
            }
        }
        return;
    }
    let hint = match name {
        "gil_open" => opened(result),
        "gil_close" => closed(result),
        "gil_start" => tr!("guide.start"),
        "gil_context" => tr!("guide.context", heading = text("context.next_call_heading")),
        "gil_status" => tr!("guide.status"),
        "gil_revisit" => tr!("guide.revisit"),
        "show_gil_monitor" => tr!("guide.show_monitor"),
        "gil_plan_propose" | "gil_plan_read" | "gil_plan_request_interview" => match plan(name, result) {
            Some(hint) => hint,
            None => return,
        },
        _ => return,
    };
    if matches!(name, "gil_open" | "gil_close" | "gil_context" | "gil_status") {
        prepend(result, format!("{hint}\n{}", text("guide.file_boundary")));
    } else if name == "gil_start" {
        // The display handoff (exact show_gil_monitor call) stays the first block; this
        // imperative summary follows it so display recovery still precedes workflow guidance.
        let at = result.content.len().min(1);
        result.content.insert(at, ContentBlock::text(hint));
    } else {
        prepend(result, hint);
    }
}

/// A refused action moved nothing. Say so once and point at the one recovery call.
fn refused(name: &str) -> Option<&'static str> {
    Some(match name {
        "gil_close" => text("guide.refused.close"),
        "gil_open" => text("guide.refused.open"),
        "gil_revisit" => text("guide.refused.revisit"),
        "gil_start" => text("guide.refused.start"),
        "gil_plan_propose" => text("guide.refused.plan_propose"),
        "gil_plan_read" => text("guide.refused.plan_read"),
        "gil_plan_request_interview" => text("guide.refused.plan_request_interview"),
        _ => return None,
    })
}

/// One line for a close refused by the dirty gate — only what this build can actually do.
fn dirty_close_recovery() -> &'static str {
    if cfg!(feature = "product-safety") { text("guide.dirty_close.product") } else { text("guide.dirty_close.dev") }
}

fn problem(result: &CallToolResult) -> &str {
    result.structured_content.as_ref()
        .and_then(|v| v.get("problem")).and_then(Value::as_str).unwrap_or("")
}

fn said(result: &CallToolResult) -> &str {
    result.structured_content.as_ref()
        .and_then(|v| v.get("said")).and_then(Value::as_str).unwrap_or("")
}

/// `열었다: step:C1/S2 · hypothesis` → `hypothesis`; `열었다: cycle:C2 · experiment` → `experiment`.
fn first_line_kind<'a>(said: &'a str, lead: &str) -> Option<&'a str> {
    let line = said.lines().next()?.strip_prefix(lead)?;
    line.split(" · ").nth(1).map(str::trim)
}

fn opened(result: &CallToolResult) -> String {
    let said = said(result);
    let Some(kind) = first_line_kind(said, text("receipt.opened_lead")) else { return tr!("guide.opened.generic"); };
    let key = match kind {
        "question" => "guide.opened.question",
        "interpretation" => "guide.opened.interpretation",
        "synthesis" => "guide.opened.synthesis",
        "outcome" if said.contains("Interview") => "guide.opened.interview_outcome",
        "outcome" => "guide.opened.outcome",
        "define" => "guide.opened.define",
        "hypothesis" => "guide.opened.hypothesis",
        "verify" => "guide.opened.verify",
        "analysis" => "guide.opened.analysis",
        "interview" => "guide.opened.interview",
        "experiment" => "guide.opened.experiment",
        _ => "guide.opened.generic",
    };
    text(key).to_string()
}

fn closed(result: &CallToolResult) -> String {
    let said = said(result);
    let Some(kind) = first_line_kind(said, text("receipt.closed_lead")) else { return tr!("guide.closed.generic"); };
    let next = next_from_receipt(said);
    let specific = match (kind, next.as_ref()) {
        ("hypothesis", _) => tr!("guide.closed.hypothesis"),
        ("synthesis", Some(Next::Open(k))) if k == "outcome" => tr!("guide.closed.synthesis_outcome"),
        ("synthesis", Some(Next::Open(k))) if k == "question" => tr!("guide.closed.synthesis_question"),
        ("verify", _) => tr!("guide.closed.verify"),
        ("outcome", Some(Next::CloseCycle)) if said.contains("Interview") => tr!("guide.closed.interview_outcome_cycle"),
        ("outcome", Some(Next::CloseCycle)) => tr!("guide.closed.outcome_cycle"),
        ("interview", Some(Next::OpenCycle)) => tr!("guide.closed.interview_open_cycle"),
        ("interview", Some(Next::ChainReport)) | ("experiment", Some(Next::ChainReport)) => tr!("guide.closed.chain_report"),
        ("experiment", Some(Next::Revisit)) if cfg!(feature = "product-safety") => tr!("guide.closed.experiment_revisit_product"),
        ("experiment", Some(Next::Revisit)) => tr!("guide.closed.experiment_revisit_dev"),
        ("experiment", Some(Next::OpenCycle)) => tr!("guide.closed.experiment_open_cycle"),
        (_, Some(Next::Open(k))) => tr!("guide.closed.open_kind", k = k),
        (_, Some(Next::OpenMany(kinds))) => tr!("guide.closed.open_many", kinds = kinds),
        (_, Some(Next::CloseCycle)) => tr!("guide.closed.close_cycle"),
        (_, Some(Next::Blocked)) => tr!("guide.closed.blocked"),
        _ => tr!("guide.closed.generic"),
    };
    tr!("guide.closed.lead", specific = specific)
}

/// What the Core receipt's `다음` block says can actually be stepped. Read, not recomputed.
#[derive(Debug, PartialEq)]
enum Next {
    Open(String),
    OpenMany(String),
    CloseCycle,
    OpenCycle,
    ChainReport,
    Revisit,
    Blocked,
}

fn next_from_receipt(said: &str) -> Option<Next> {
    // Order matters: a Cycle-close skeleton may quote `gil revisit` inside a value description.
    if said.contains(text("receipt.marker.chain_report")) { return Some(Next::ChainReport); }
    if said.contains(text("receipt.marker.close_cycle")) { return Some(Next::CloseCycle); }
    if said.contains(text("receipt.marker.revisit")) { return Some(Next::Revisit); }
    if said.contains(text("receipt.marker.blocked")) { return Some(Next::Blocked); }
    let mut lines = said.lines().map(str::trim);
    while let Some(line) = lines.next() {
        if let Some(rest) = line.strip_prefix(text("receipt.openable_prefix")) {
            return Some(Next::Open(rest.trim().to_string()));
        }
        if line == text("receipt.openable_heading") {
            // `gil open <종류>`: one `name  description` line per kind (two spaces after the name).
            let kinds: Vec<&str> = lines.by_ref()
                .take_while(|l| !l.is_empty() && !l.starts_with(text("receipt.run_heading")))
                .filter_map(|l| l.split_whitespace().next())
                .collect();
            if kinds.iter().any(|k| *k == "interview" || *k == "experiment") { return Some(Next::OpenCycle); }
            return Some(Next::OpenMany(kinds.join(", ")));
        }
    }
    None
}

/// Plan tools answer with typed fields; the next call follows the recorded decision, not the OS.
fn plan(name: &str, result: &CallToolResult) -> Option<String> {
    let view = result.structured_content.as_ref()?;
    if view.get("supported") == Some(&Value::Bool(false)) {
        return Some(tr!("guide.plan.unsupported"));
    }
    let proposal = view.get("proposal").filter(|p| !p.is_null());
    let decision = proposal.and_then(|p| p.pointer("/receipt/decision")).and_then(Value::as_str);
    let revision = proposal.and_then(|p| p.get("revision")).and_then(Value::as_u64);
    let reinterview = view.get("reinterview_requests").and_then(Value::as_array).is_some_and(|r| !r.is_empty());
    let pending: Vec<String> = view.get("pending").and_then(Value::as_array).map(|items|
        items.iter().filter_map(|i| i.get("id").and_then(Value::as_str)).map(str::to_string).collect()).unwrap_or_default();
    let current = view.get("current_item").filter(|i| !i.is_null()).and_then(|i| i.get("id")).and_then(Value::as_str);
    // A binding survives Cycle closure. B's read-only state is the witness; without it
    // an older server's binding cannot justify telling the model to resume Verify.
    let cycle_state = view.pointer("/current_cycle/state").and_then(Value::as_str);
    if name == "gil_plan_read"
        && (cycle_state == Some("closed") || (current.is_some() && cycle_state != Some("open")))
    {
        return Some(if cycle_state == Some("closed") { tr!("guide.plan.cycle_closed") } else { tr!("guide.plan.cycle_unknown") });
    }
    Some(match name {
        "gil_plan_request_interview" => tr!("guide.plan.request_interview"),
        _ => match (proposal, decision) {
            (Some(_), None) => tr!("guide.plan.waiting", revision = revision.unwrap_or(0)),
            (Some(_), Some("approve")) => tr!("guide.plan.approved", revision = revision.unwrap_or(0)),
            (Some(_), Some(_)) => tr!("guide.plan.changes", revision = revision.unwrap_or(0)),
            (None, _) if reinterview => tr!("guide.plan.reinterview"),
            (None, _) => match current {
                Some(item) => tr!("guide.plan.bound", item = item),
                None if !pending.is_empty() => tr!("guide.plan.pending", pending = pending.join(", ")),
                None => tr!("guide.plan.none"),
            },
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn receipt(said: &str) -> CallToolResult {
        let mut out = CallToolResult::success(vec![ContentBlock::text(said.to_string())]);
        out.structured_content = Some(json!({"ok": true, "exit_code": 0, "said": said, "problem": ""}));
        out
    }
    fn text(result: &CallToolResult) -> String {
        result.content.iter().filter_map(|c| c.as_text().map(|t| t.text.clone())).collect::<Vec<_>>().join("\n")
    }

    fn refusal_of(problem: &str) -> CallToolResult {
        let mut out = CallToolResult::success(vec![ContentBlock::text(problem.to_string())]);
        out.structured_content = Some(json!({"ok": false, "exit_code": 1, "said": "", "problem": problem}));
        out.is_error = Some(true);
        out
    }

    #[test]
    fn a_dirty_close_refusal_adds_the_recovery_this_build_allows() {
        let mut dirty = refusal_of("거절: cycle:C2 을(를) 닫을 수 없다.\n\n이유\n  기준 세계 snapshot:A1 이후 프로젝트 파일이 바뀌었다.\n    새로 생김  report.md\n");
        after("gil_close", &mut dirty);
        let hint = text(&dirty);
        assert!(hint.starts_with("GIL 거절: 닫힌 것은 없다."), "{hint}");
        assert!(hint.contains("dirty 거절이면:"), "{hint}");
        if cfg!(feature = "product-safety") {
            assert!(!hint.contains("gil restore") && !hint.contains("gil_restore"), "{hint}");
            assert!(hint.contains("프로젝트 밖의 새 보존 위치로 옮기고") && hint.contains("다음 Experiment의 Verify"), "{hint}");
        } else {
            assert!(hint.contains("gil_restore"), "{hint}");
        }
        // An ordinary input refusal gets no dirty line.
        let mut plain = refusal_of("Report 에 verdict 가 없다\n");
        after("gil_close", &mut plain);
        assert!(!text(&plain).contains("dirty 거절이면"), "{}", text(&plain));
    }

    #[test]
    fn a_closed_hypothesis_names_verify_open_first_and_nothing_else() {
        let mut r = receipt("닫았다: step:C2/S2 · hypothesis\n기록됨: journey:J3\n\n다음\n  열 수 있는 것: verify\n\n실행\n  gil open\n");
        after("gil_close", &mut r);
        let hint = text(&r);
        assert!(hint.starts_with("GIL 진행: 이 닫기가 성공했다. 다음 행동: gil_open(kind: verify)"), "{hint}");
        assert!(!hint.contains("gil_cycle"), "{hint}");
    }

    #[test]
    fn a_closed_synthesis_follows_the_recorded_approval_not_the_sequence() {
        let mut yes = receipt("닫았다: step:C1/S3 · synthesis\n기록됨: journey:J4\n\nInterview 마무리\n  승인된 Synthesis만 닫혔다.\n\n다음\n  열 수 있는 것: outcome\n\n실행\n  gil open\n");
        after("gil_close", &mut yes);
        assert!(text(&yes).contains("gil_open(kind: outcome)"), "{}", text(&yes));
        assert!(!text(&yes).contains("gil_open(kind: question)"));
        let mut no = receipt("닫았다: step:C1/S3 · synthesis\n기록됨: journey:J4\n\n다음\n  열 수 있는 것: question\n\n실행\n  gil open\n");
        after("gil_close", &mut no);
        assert!(text(&no).contains("gil_open(kind: question)") && text(&no).contains("재사용하지 않는다"), "{}", text(&no));
    }

    #[test]
    fn interview_outcome_then_cycle_report_then_experiment_are_three_distinct_calls() {
        let mut outcome = receipt("닫았다: step:C1/S4 · outcome\n기록됨: journey:J5\n\nInterview 마무리\n  Outcome Step만 닫혔고 Interview Cycle은 아직 열려 있다.\n\n다음\n  이 Cycle 의 Report 를 적어 닫는다. MCP에서는 gil_close(project_root, report)를 사용한다.\n\n실행\n  gil close\n");
        after("gil_close", &mut outcome);
        assert!(text(&outcome).contains("Interview Cycle Report"), "{}", text(&outcome));
        assert!(!text(&outcome).contains("gil_open(kind: experiment)"));
        let mut cycle = receipt("닫았다: cycle:C1 · interview · success\n기록됨: journey:J6 (판은 늘지 않는다)\n\n다음\n  Interview Cycle까지 닫혔다.\n  열 수 있는 것\n  interview   사용자에게 질문해 의도와 판단을 확인한다\n  experiment  명확한 가설을 실행하고 결과를 검증한다\n\n실행\n  gil open <종류>\n");
        after("gil_close", &mut cycle);
        assert!(text(&cycle).contains("gil_open(kind: experiment)") && text(&cycle).contains("gil_start를 다시 부르지 않는다"), "{}", text(&cycle));
    }

    #[test]
    fn a_failed_experiment_is_told_to_revisit_once_and_warn_the_user() {
        let mut r = receipt("닫았다: cycle:C3 · experiment · failure\n기록됨: journey:J9 (판은 늘지 않는다)\n\n다음\n  적어 둔 조상으로 되돌아간다 — 그 자리의 Artifact 세계가 함께 복원된다\n\n실행\n  gil revisit\n");
        after("gil_close", &mut r);
        let hint = text(&r);
        if cfg!(feature = "product-safety") { assert!(hint.contains("버튼을 기다린다"), "{hint}"); }
        else { assert!(hint.contains("gil_revisit(인수 없음)") && hint.contains("사용자에게 알린"), "{hint}"); }
    }

    #[test]
    fn an_unknown_receipt_shape_falls_back_to_the_generic_sentence() {
        let mut r = receipt("typed close receipt without the usual first line\n");
        after("gil_close", &mut r);
        assert!(text(&r).contains("다음 행동") && text(&r).contains("gil_status"), "{}", text(&r));
        let mut o = receipt("something else\n");
        after("gil_open", &mut o);
        assert!(text(&o).contains("gil_close"), "{}", text(&o));
    }

    #[test]
    fn opened_steps_say_wait_or_act_without_shortcuts() {
        for (kind, must, must_not) in [
            ("question", "답을 기다린다", "조사한다"),
            ("synthesis", "명시적 동의", "approved: yes로 바로"),
            ("verify", "evidence_files", "채팅만으로 닫"),
            ("define", "Verify에서 한다", "검색"),
            ("experiment", "gil_open(kind: define)", "verify를 먼저"),
        ] {
            let lead = if kind == "experiment" { "열었다: cycle:C2 · experiment · 부모 cycle:C1\n" } else { "열었다: step:C1/S1 · " };
            let mut r = receipt(&format!("{lead}{}\n\n지금 할 일\n  …\n", if kind == "experiment" { "" } else { kind }));
            after("gil_open", &mut r);
            let hint = text(&r);
            assert!(hint.contains(must), "{kind}: {hint}");
            assert!(!hint.contains(must_not), "{kind}: {hint}");
            assert!(hint.contains("gil_close") || kind == "experiment", "{kind}: {hint}");
            if matches!(kind, "question" | "synthesis") {
                assert!(hint.contains("gil_await_user(project_root, question)") && hint.contains("선택적 읽기 확인이며 턴 종료 조건이 아니다"), "{kind}: {hint}");
            }
        }
    }

    #[test]
    fn a_successful_start_names_the_monitor_call_before_any_chat_answer() {
        let mut r = receipt("시작했다: cycle:C1 · interview\n\n다음\n  show_gil_monitor({\"scope_id\":\"project:abc\"})\n");
        after("gil_start", &mut r);
        let hint = text(&r);
        assert!(hint.contains("GIL 시작:"), "{hint}");
        assert!(r.content[0].as_text().unwrap().text.starts_with("시작했다"), "handoff block stays first");
        assert!(hint.contains("채팅으로 답하기 전에") && hint.contains("show_gil_monitor"), "{hint}");
        assert!(hint.contains("gil_open(kind: question)") && hint.contains("Verify"), "{hint}");
        assert!(hint.contains("gil_await_user") && hint.contains("파일 없는 웹 조사·기존 증거 읽기·무관한 외부 작업은 허용"), "{hint}");
        let mut shown = receipt("Monitor 표시를 요청했다\n");
        after("show_gil_monitor", &mut shown);
        assert!(text(&shown).contains("요청문을 답으로 대신 적고 넘어가지 않는다"), "{}", text(&shown));
        assert!(text(&shown).contains("gil_await_user"), "{}", text(&shown));
        let mut q = receipt("열었다: step:C1/S1 · question\n\n지금 할 일\n  …\n");
        after("gil_open", &mut q);
        assert!(text(&q).contains("response로 대신 적어 닫지 않는다"), "{}", text(&q));
        assert!(text(&q).contains("파일 없는 웹 조사·기존 증거 읽기·무관한 외부 작업은 허용"), "{}", text(&q));
    }

    #[test]
    fn refusals_say_nothing_closed_and_never_claim_success() {
        let mut r = CallToolResult::error(vec![ContentBlock::text("Report 에 다음 칸이 있어야 한다".to_string())]);
        r.structured_content = Some(json!({"ok": false, "exit_code": 1, "said": "", "problem": "Report 에 다음 칸이 있어야 한다\n"}));
        after("gil_close", &mut r);
        let hint = text(&r);
        assert!(hint.contains("닫힌 것은 없다") && !hint.contains("이 닫기가 성공"), "{hint}");
        let mut start = CallToolResult::error(vec![ContentBlock::text("이미 프로젝트가 있다".to_string())]);
        after("gil_start", &mut start);
        assert!(text(&start).contains("gil_start를 반복하지 않는다") && !text(&start).contains("show_gil_monitor"));
        let mut plain = CallToolResult::error(vec![ContentBlock::text("x".to_string())]);
        after("gil_status", &mut plain);
        assert_eq!(text(&plain), "x", "unlisted refusals keep their exact text");
    }

    #[test]
    fn plan_refusal_keeps_the_specific_cause_before_recovery_guidance() {
        let mut result = super::super::actions::refusal("draft.items[id=A].question: mismatch");
        after("gil_plan_propose", &mut result);
        assert!(text(&result).starts_with("draft.items[id=A].question"));
        assert!(text(&result).contains("active_plan.items"));
        assert!(!text(&result).contains("scope 5칸"));
    }
    #[test]
    fn plan_hints_follow_the_recorded_decision_and_never_require_plans_on_legacy_projects() {
        let mut legacy = CallToolResult::structured(json!({"supported": false}));
        after("gil_plan_read", &mut legacy);
        assert!(text(&legacy).contains("승인 계획 프로필이 아니다") && !text(&legacy).contains("gil_plan_propose로 제안"), "{}", text(&legacy));

        let waiting = json!({"supported": true, "proposal": {"revision": 2, "receipt": null}, "pending": [], "reinterview_requests": [], "current_item": null});
        let mut w = CallToolResult::structured(waiting);
        after("gil_plan_propose", &mut w);
        let hint = text(&w);
        assert!(hint.contains("판 2") && hint.contains("턴을 끝내고 기다린다") && !hint.contains("gil_close(Synthesis)"), "{hint}");
        assert!(hint.contains("gil_await_user(project_root, question)"), "{hint}");

        let approved = json!({"supported": true, "proposal": {"revision": 2, "receipt": {"decision": "approve", "source": "monitor_button"}}, "pending": [], "reinterview_requests": [], "current_item": null});
        let mut a = CallToolResult::structured(approved);
        after("gil_plan_read", &mut a);
        let hint = text(&a);
        assert!(hint.contains("close_report를 글자 그대로") && hint.contains("gil_open(kind: outcome)") && hint.contains("Interview Cycle Report"), "{hint}");

        let changes = json!({"supported": true, "proposal": {"revision": 3, "receipt": {"decision": "request_changes", "source": "monitor_button"}}, "pending": [], "reinterview_requests": [], "current_item": null});
        let mut c = CallToolResult::structured(changes);
        after("gil_plan_read", &mut c);
        assert!(text(&c).contains("approved: no") && text(&c).contains("이전 승인은 재사용되지 않는다"), "{}", text(&c));

        let bound = json!({"supported": true, "proposal": null, "pending": [{"id": "B", "question": "q", "done_when": "d"}], "reinterview_requests": [], "current_item": {"id": "A", "question": "q", "done_when": "d"}, "current_cycle": {"ref": "cycle:C2", "state": "open"}, "document": "# GIL approved plan"});
        let mut b = CallToolResult::structured(bound);
        after("gil_plan_read", &mut b);
        assert!(text(&b).contains("항목 A") && text(&b).contains("plan_document") && text(&b).contains("evidence_files"), "{}", text(&b));

        let blocked = json!({"supported": true, "proposal": null, "pending": [{"id": "B", "question": "q", "done_when": "d"}], "reinterview_requests": [{"cycle_ref": "cycle:C2", "reason": "scope"}], "current_item": null});
        let mut r = CallToolResult::structured(blocked);
        after("gil_plan_request_interview", &mut r);
        assert!(text(&r).contains("아무것도 닫히거나 열리지 않았다") && text(&r).contains("gil_open(kind: interview)"), "{}", text(&r));
        let mut r2 = CallToolResult::structured(json!({"supported": true, "proposal": null, "pending": [], "reinterview_requests": [{"cycle_ref": "cycle:C2", "reason": "scope"}], "current_item": null}));
        after("gil_plan_read", &mut r2);
        assert!(text(&r2).contains("재인터뷰 요청이 보존") && !text(&r2).contains("gil_open(kind: experiment)"), "{}", text(&r2));
    }

    #[test]
    fn wave2_closed_or_unknown_cycle_never_directs_work_into_its_old_verify() {
        for state in [Some("closed"), None] {
            for requested in [false, true] {
                let mut view = json!({"supported": true, "proposal": null,
                    "current_item": {"id": "A"}, "pending": [{"id": "B"}],
                    "reinterview_requests": if requested {json!([{"reason":"scope"}])} else {json!([])}});
                if let Some(state) = state { view["current_cycle"] = json!({"ref":"cycle:C2", "state":state}); }
                let mut result = CallToolResult::structured(view);
                after("gil_plan_read", &mut result);
                let hint = text(&result);
                assert!(hint.contains("gil_context"), "{state:?}, request={requested}: {hint}");
                assert!(!hint.contains("Verify에서") && !hint.contains("남은 Step"), "{hint}");
            }
        }
    }

}
