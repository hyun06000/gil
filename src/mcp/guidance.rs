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
            // An approved-plan refusal is an approval-state problem, not an input error: fixing
            // and resending the same call cannot succeed, so the one next call is gil_plan_read.
            let hint = if let Some(plan) = plan_refusal(name, problem) {
                plan.to_string()
            // A dirty refusal of a close has one more real recovery than "fix the input".
            } else if name == "gil_close" && (problem.contains(text("marker.dirty_world")) || problem.contains("dirty")) {
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
        prepend(result, format!("{hint}\n{}", file_boundary(name)));
    } else if name == "gil_start" {
        // The display handoff (exact show_gil_monitor call) stays the first block; this
        // imperative summary follows it so display recovery still precedes workflow guidance.
        let at = result.content.len().min(1);
        result.content.insert(at, ContentBlock::text(hint));
    } else {
        prepend(result, hint);
    }
}

/// The file-work note under the hint, in two parts. `guide.file_boundary.rules` says where
/// project files may change; `guide.file_boundary.applicability` says what that guidance is and
/// is not: Agent guidance for the selected GIL project, no OS write block, no hooks, nothing for
/// ordinary shells or other projects.
///
/// `gil_context` already carries the full rules in its body (`ctx.file_scope`, always written by
/// `context()`), so its head keeps only the applicability, which the body does not fully state.
/// `gil_status`, `gil_open` and `gil_close` have no such body and keep both parts, joined by the
/// locale's own `guide.file_boundary` template.
fn file_boundary(name: &str) -> String {
    let applicability = text("guide.file_boundary.applicability");
    if name == "gil_context" { return applicability.to_string(); }
    tr!("guide.file_boundary", rules = text("guide.file_boundary.rules"), applicability = applicability)
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

/// The approved-plan refusal marker (`plan.err.prefix` without its message), in the current
/// locale: the Core prefixes every approval-state reason with it, so the receipt and its reader
/// agree on the language.
fn plan_marker() -> &'static str {
    let prefix = text("plan.err.prefix");
    let marker = prefix.split("{message}").next().unwrap_or(prefix).trim_end();
    if marker.is_empty() { prefix } else { marker }
}

/// A `gil_close`/`gil_open` refused for an approval-state reason (no proposal, waiting for the
/// Monitor button, approved text differs, before approval, preserved re-interview request, no
/// pending item). Everything else keeps the ordinary "fix the input" recovery.
fn plan_refusal(name: &str, problem: &str) -> Option<&'static str> {
    if !problem.contains(plan_marker()) { return None; }
    match name {
        "gil_close" => Some(text("guide.refused.plan_close")),
        "gil_open" => Some(text("guide.refused.plan_open")),
        _ => None,
    }
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

/// A Chain receipt names the container, not a Step kind: `닫았다: chain:C1 · failure`,
/// `열었다: chain:C2 · Opening Interview cycle:C4`. Typed refs are locale-independent.
fn first_line_is_chain(said: &str, lead: &str) -> bool {
    said.lines().next().and_then(|line| line.strip_prefix(lead))
        .and_then(|line| line.split(" · ").next()).is_some_and(|r| r.trim().starts_with("chain:"))
}

fn opened(result: &CallToolResult) -> String {
    let said = said(result);
    // The next Chain: its Opening Interview is open with no Step, and nothing of the previous
    // Chain's approval carries over. The generic sentence would ask for a gil_close that does not exist.
    if first_line_is_chain(said, text("receipt.opened_lead")) { return tr!("guide.opened.chain"); }
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
    let specific = text(key);
    match kind {
        "synthesis" => {
            let here = said.lines().next().and_then(|line| line.strip_prefix(text("receipt.opened_lead")))
                .and_then(|line| line.split(" · ").next()).and_then(|r| r.parse::<crate::StepRef>().ok());
            let location = here.map(|r| tr!("guide.opened.synthesis_location", here = r, cycle = r.cycle())).unwrap_or_default();
            let cycle = here.map(|r| r.cycle().id().to_string()).unwrap_or_else(|| "Cn".into());
            let example = serde_json::json!({"basis_refs": format!("step:{cycle}/S1\nstep:{cycle}/S2")});
            // Cycle Model §16: the proposal is short, with small items, before the basis rules.
            let specific = format!("{specific} {}", text("guide.propose_brief"));
            tr!("guide.opened.synthesis_basis", specific = specific, location = location, rules = text("cycle.basis_refs_rules"), example = example)
        },
        "verify" => format!("{specific}\n{}", crate::visual_report::image::limits_guidance()),
        // Cycle Model §5.1: a Question deepens the interview; an Interpretation sorts what the
        // answers confirmed from the agent's reading and what is still unknown.
        "question" => format!("{specific} {}", text("guide.interview_deepen")),
        "interpretation" => format!("{specific} {} {}", text("guide.report_brief"), text("guide.interpretation_sort")),
        // Cycle Model §7.1: Define names the smallest problem performable now, aligned with the agreed item.
        "define" => format!("{specific} {}", text("guide.define_smallest")),
        // Report-writing Steps: the short-report rule rides on the receipt that opens them.
        "analysis" | "outcome" => format!("{specific} {}", text("guide.report_brief")),
        _ => specific.to_string(),
    }
}

fn closed(result: &CallToolResult) -> String {
    let said = said(result);
    let Some(kind) = first_line_kind(said, text("receipt.closed_lead")) else { return tr!("guide.closed.generic"); };
    // A closed Chain: the second segment is its verdict, not a Step kind. The Core receipt
    // preserves the Report and names no next move; only an explicit next Chain can follow.
    if first_line_is_chain(said, text("receipt.closed_lead")) {
        return tr!("guide.closed.lead", specific = tr!("guide.closed.chain", verdict = kind));
    }
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
        ("experiment", Some(Next::Revisit)) if cfg!(feature = "product-safety") => format!("{} {}", tr!("guide.closed.experiment_revisit_product"), text("guide.interrupt_partial_results")),
        ("experiment", Some(Next::Revisit)) => format!("{} {}", tr!("guide.closed.experiment_revisit_dev"), text("guide.interrupt_partial_results")),
        ("experiment", Some(Next::OpenCycle)) => tr!("guide.closed.experiment_open_cycle"),
        (_, Some(Next::Open(k))) => tr!("guide.closed.open_kind", k = k),
        (_, Some(Next::OpenMany(kinds))) => tr!("guide.closed.open_many", kinds = kinds),
        (_, Some(Next::CloseCycle)) => tr!("guide.closed.close_cycle"),
        (_, Some(Next::Blocked)) => tr!("guide.closed.blocked"),
        // The generic sentence already carries the lead; wrapping it again repeated the lead.
        _ => return tr!("guide.closed.generic"),
    };
    // A Cycle or Chain Report comes next: write it short. Execution finished: show the result first.
    let specific = match (kind, next.as_ref()) {
        (_, Some(Next::CloseCycle) | Some(Next::ChainReport)) => format!("{specific} {}", text("guide.report_brief")),
        ("experiment", Some(Next::OpenCycle)) => format!("{specific} {}", text("guide.closing_results_first")),
        _ => specific,
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
    // Modern Core already projects the exact Step/Chain next call on the first line of next.
    // Its longer reference notes stay in the payload. Do not infer the action again from a
    // pending item or a binding that survives closure. Older replies fall back below.
    if let Some(next) = view.get("next").and_then(Value::as_str).and_then(|n| n.lines().next())
        .filter(|n| !n.trim().is_empty()) {
        let mut hint = next.to_string();
        if name == "gil_plan_request_interview" {
            hint.push_str(&format!("\n{}", text("guide.plan.request_saved")));
        }
        if view.pointer("/current_cycle/state").and_then(Value::as_str) == Some("open") {
            if let Some(item) = view.pointer("/current_item/id").and_then(Value::as_str) {
                hint.push_str(&format!("\n{}", tr!("guide.plan.bound", item = item)));
            }
        }
        return Some(hint);
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
        // The request is the one recorded signal that the item may not finish: say how to stop truthfully.
        "gil_plan_request_interview" => format!("{} {}", tr!("guide.plan.request_interview"), text("guide.interrupt_honest")),
        _ => match (proposal, decision) {
            (Some(_), None) => tr!("guide.plan.waiting", revision = revision.unwrap_or(0)),
            (Some(_), Some("approve")) => tr!("guide.plan.approved", revision = revision.unwrap_or(0)),
            (Some(_), Some(_)) => tr!("guide.plan.changes", revision = revision.unwrap_or(0)),
            (None, _) if reinterview => tr!("guide.plan.reinterview"),
            (None, _) => match current {
                Some(item) => tr!("guide.plan.bound", item = item),
                None if !pending.is_empty() => tr!("guide.plan.pending", pending = pending.join(", ")),
                None => format!("{} {}", tr!("guide.plan.none"), text("guide.closing_results_first")),
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

    /// 2026-11-16 roadmap item 2: an approval-state refusal must not be answered with
    /// "fix the input and resend". Guidance contract only — not evidence of a model run.
    #[test]
    fn an_approved_plan_refusal_is_a_state_problem_not_an_input_error() {
        let waiting = format!("거절: step:C1/S3 · synthesis 를 닫을 수 없다.\n\n이유\n  {}\n", tr!("plan.err.prefix", message = crate::locale::text("plan.err.decide_in_monitor")));
        let mut close = refusal_of(&waiting);
        after("gil_close", &mut close);
        let hint = text(&close);
        assert!(hint.starts_with("GIL 거절: 닫힌 것은 없다."), "{hint}");
        assert!(hint.contains("gil_plan_read"), "{hint}");
        assert!(hint.contains("승인 대기") && hint.contains("턴을 끝내고"), "{hint}");
        for forbidden in ["입력 오류를 고쳐", "같은 gil_close를 한 번 다시 보낸다", "dirty 거절이면", "gil_help"] {
            assert!(!hint.contains(forbidden), "{forbidden}: {hint}");
        }
        assert!(hint.contains("approved") && hint.contains("직접 적거나"), "{hint}");

        let before_approval = format!("거절: experiment Cycle 을 열 수 없다.\n\n이유\n  {}\n", tr!("plan.err.prefix", message = crate::locale::text("plan.err.before_approval")));
        let mut open = refusal_of(&before_approval);
        after("gil_open", &mut open);
        let hint = text(&open);
        assert!(hint.starts_with("GIL 거절: 열린 것은 없다."), "{hint}");
        assert!(hint.contains("gil_plan_read"), "{hint}");
        assert!(!hint.contains("행동 계약(objective") && !hint.contains("한 번 다시 보낸다"), "{hint}");

        // An ordinary input refusal keeps the input recovery; a reason that merely mentions a
        // plan in prose without the Core's marker is not an approval-state refusal.
        let mut plain = refusal_of("거절: step:C2/S3 · verify 를 닫을 수 없다.\n\n이유\n  Report 에 다음 칸이 있어야 한다 (빠진 것: result)\n");
        after("gil_close", &mut plain);
        assert!(text(&plain).contains("입력 골격대로 고쳐") && !text(&plain).contains("gil_plan_read"), "{}", text(&plain));
        // Other refused tools never get the plan hint.
        let mut revisit = refusal_of(&waiting);
        after("gil_revisit", &mut revisit);
        assert!(!text(&revisit).contains("gil_plan_read"), "{}", text(&revisit));
    }

    /// 2026-11-16 roadmap item 2: Verify records observations, Analysis interprets them.
    #[test]
    fn verify_and_analysis_openings_keep_observation_apart_from_interpretation() {
        let mut verify = receipt("열었다: step:C2/S3 · verify\n\n지금 할 일\n  …\n");
        after("gil_open", &mut verify);
        let hint = text(&verify);
        assert!(hint.contains("result에는") && hint.contains("관측") && hint.contains("interpretation") && hint.contains("verdict"), "{hint}");
        let mut analysis = receipt("열었다: step:C2/S4 · analysis\n\n지금 할 일\n  …\n");
        after("gil_open", &mut analysis);
        let hint = text(&analysis);
        assert!(hint.contains("되풀이하지 않") && hint.contains("가설") && hint.contains("불확실"), "{hint}");
    }

    /// 2026-11-17 Chain first, context size: `gil_context` repeated the file-work rules twice,
    /// once in the head and once in its body (`ctx.file_scope`). The head now keeps only what the
    /// guidance applies to; the body and the other receipts are untouched. Guidance contract
    /// only — not a token, cost or accuracy claim, and a Host that reads only structuredContent
    /// sees no difference.
    #[test]
    fn the_context_head_keeps_only_the_applicability_and_other_receipts_keep_the_full_boundary() {
        use crate::locale::{Locale, text as t};
        // (language, what the applicability must still say, what the context body must still say)
        let preserved: [(Locale, [&str; 4], [&str; 4]); 5] = [
            (Locale::Ko,
                ["선택한 GIL 프로젝트의 Agent 지침", "OS 쓰기 차단은 아니다", "GIL 훅은 설치하지 않으며", "일반 셸·다른 프로젝트에는 적용하지 않는다"],
                ["실제로 열린 Experiment Verify에서만", "DOCX·PNG·PDF", "프로젝트 파일을 남기지 않는 웹 조사", "OS 쓰기 차단을 뜻하지 않는다"]),
            (Locale::En,
                ["Agent guidance for the selected GIL project", "not an OS write block", "GIL installs no hooks", "does not apply to ordinary shells or other projects"],
                ["only in an Experiment Verify that is actually open", "DOCX, PNG and PDF", "web research that leaves no project file", "not an OS write block"]),
            (Locale::Ja,
                ["選んだ GIL プロジェクトへの Agent 指針", "OS の書き込み遮断ではない", "GIL はフックを入れず", "一般のシェルや他のプロジェクトには適用しない"],
                ["実際に開いている Experiment Verify でのみ", "DOCX・PNG・PDF", "プロジェクトファイルを残さないウェブ調査", "OS の書き込み遮断ではない"]),
            (Locale::ZhHans,
                ["所选 GIL 项目的 Agent 指引", "不是操作系统的写入阻止", "GIL 不安装钩子", "不适用于普通 shell 或其他项目"],
                ["只在实际打开的 Experiment Verify 中", "DOCX、PNG、PDF", "不留下项目文件的网络调研", "不是操作系统的写入阻止"]),
            (Locale::ZhHant,
                ["所選 GIL 專案的 Agent 指引", "不是作業系統的寫入封鎖", "GIL 不安裝 hook", "不適用於一般 shell 或其他專案"],
                ["只在實際開啟的 Experiment Verify 中", "DOCX、PNG、PDF", "不留下專案檔案的網路調查", "不是作業系統的寫入封鎖"]),
        ];
        assert_eq!(preserved.len(), Locale::ALL.len());
        for (language, applies, body_rules) in preserved {
            crate::locale::with(language, || {
                let rules = t("guide.file_boundary.rules");
                let applicability = t("guide.file_boundary.applicability");
                let full = tr!("guide.file_boundary", rules = rules, applicability = applicability);
                // The full note is the two parts and the language's own separator, nothing else.
                assert!(full.starts_with(rules) && full.ends_with(applicability), "{language}: {full}");
                assert!(full[rules.len()..full.len() - applicability.len()].trim().is_empty(), "{language}: {full}");
                assert!(!full.contains('{') && !rules.contains(applicability) && !applicability.contains(rules), "{language}: {full}");
                for needle in applies { assert!(applicability.contains(needle), "{language} applicability lost {needle}: {applicability}"); }
                let body = t("ctx.file_scope");
                for needle in body_rules { assert!(body.contains(needle), "{language} body lost {needle}: {body}"); }

                // gil_status / gil_open / gil_close: rules and applicability, as before.
                for name in ["gil_status", "gil_open", "gil_close"] {
                    let mut r = receipt("receipt\n");
                    after(name, &mut r);
                    let head = &r.content[0].as_text().unwrap().text;
                    assert!(head.ends_with(&format!("\n{full}")), "{language} {name}: {head}");
                }

                // gil_context: the head is the resume hint and the applicability only. The body
                // (`said`) with the full rules, and structuredContent, pass through untouched.
                let said = format!("preamble\n{body}\nreport\n");
                let mut context = receipt(&said);
                let structured = context.structured_content.clone();
                after("gil_context", &mut context);
                let head = &context.content[0].as_text().unwrap().text;
                assert_eq!(*head, format!("{}\n{applicability}", tr!("guide.context", heading = t("context.next_call_heading"))), "{language}");
                assert!(!head.contains(rules), "{language}: {head}");
                assert_eq!(context.content.len(), 2, "{language}");
                assert_eq!(context.content[1].as_text().unwrap().text, said, "{language}");
                assert_eq!(context.structured_content, structured, "{language}");
                assert!(said.contains(body), "{language}");

                // A refused read gets no file note at all, exactly as before.
                let mut refused = refusal_of("x");
                after("gil_context", &mut refused);
                assert_eq!(text(&refused), "x", "{language}");
            });
        }
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
        // M6-E: partial results are shown before the restore; no keep-files transition is promised.
        assert!(hint.contains(crate::locale::text("guide.interrupt_partial_results")), "{hint}");
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
    fn synthesis_and_verify_open_expose_authoring_constraints_before_close() {
        let mut synthesis = receipt("열었다: step:C7/S9 · synthesis\n");
        after("gil_open", &mut synthesis);
        let hint = text(&synthesis);
        for needle in ["$.draft.synthesis.basis_refs", "step:C7/S9", "cycle:C7", "부모 사슬", "Question", "Interpretation", "중복 없이", "한 줄에 하나씩", "statement", "자동 이동"] {
            assert!(hint.contains(needle), "{hint}");
        }
        let mut verify = receipt("열었다: step:C8/S3 · verify\n");
        after("gil_open", &mut verify);
        assert!(text(&verify).contains(&crate::visual_report::image::limits_guidance()));
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

    /// 2026-11-15 roadmap item 1: reports are written short, and the end of execution shows the
    /// result before asking to close. Guidance contract only — not evidence of a model run.
    #[test]
    fn report_steps_are_told_to_write_short_and_the_closing_shows_the_result_first() {
        for kind in ["interpretation", "analysis", "outcome"] {
            let mut r = receipt(&format!("열었다: step:C2/S4 · {kind}\n\n지금 할 일\n  …\n"));
            after("gil_open", &mut r);
            assert!(text(&r).contains("Report는 짧게 쓴다") && text(&r).contains("하위 보고서를 반복하지 않으며"), "{kind}: {}", text(&r));
        }
        for kind in ["question", "define", "hypothesis", "verify", "synthesis"] {
            let mut r = receipt(&format!("열었다: step:C2/S1 · {kind}\n\n지금 할 일\n  …\n"));
            after("gil_open", &mut r);
            assert!(!text(&r).contains("Report는 짧게 쓴다"), "{kind}: {}", text(&r));
        }
        let mut cycle = receipt("닫았다: step:C2/S5 · outcome\n기록됨: journey:J7\n\n다음\n  이 Cycle 의 Report 를 적어 닫는다. MCP에서는 gil_close(project_root, report)를 사용한다.\n\n실행\n  gil close\n");
        after("gil_close", &mut cycle);
        assert!(text(&cycle).contains("Report는 짧게 쓴다") && text(&cycle).contains("실패 원인·불확실성·미검증 범위는 줄이지 않는다"), "{}", text(&cycle));
        let mut step = receipt("닫았다: step:C2/S2 · hypothesis\n기록됨: journey:J3\n\n다음\n  열 수 있는 것: verify\n\n실행\n  gil open\n");
        after("gil_close", &mut step);
        assert!(!text(&step).contains("Report는 짧게 쓴다"), "{}", text(&step));
        let mut done = receipt("닫았다: cycle:C3 · experiment · success\n기록됨: journey:J9 (판은 늘지 않는다)\n\n다음\n  열 수 있는 것\n  interview   사용자에게 질문해 의도와 판단을 확인한다\n  experiment  명확한 가설을 실행하고 결과를 검증한다\n\n실행\n  gil open <종류>\n");
        after("gil_close", &mut done);
        let hint = text(&done);
        assert!(hint.contains("결과물을 먼저 보여 준다") && hint.contains("종료 여부를 묻는다") && hint.contains("새 파일을 만들지 않는다"), "{hint}");
        assert!(hint.contains("gil_open(kind: experiment)"), "the legal next calls stay: {hint}");
        let mut none = CallToolResult::structured(json!({"supported": true, "proposal": null, "pending": [], "reinterview_requests": [], "current_item": null}));
        after("gil_plan_read", &mut none);
        assert!(text(&none).contains("Closing Interview") && text(&none).contains("결과물을 먼저 보여 준다"), "{}", text(&none));
    }

    /// 2026-11-17 Chain first (Cycle Model §5.1 · §16 · §7.1): the Question deepens the interview,
    /// the Interpretation sorts confirmed facts from the agent's reading, the Synthesis proposes
    /// short with small items, and the Define names the smallest problem performable now, aligned
    /// with the agreed item. Guidance contract only — not evidence of a model run, and no size rule.
    #[test]
    fn interview_steps_deepen_the_synthesis_proposes_short_and_the_define_is_the_smallest_problem() {
        let open = |kind: &str| {
            let mut r = receipt(&format!("열었다: step:C1/S1 · {kind}\n\n지금 할 일\n  …\n"));
            after("gil_open", &mut r);
            text(&r)
        };
        let question = open("question");
        for needle in ["심층 인터뷰", "예와 원하지 않는 반례", "완료 조건", "이미 받은 답은 재사용", "다음 결정에 영향을 주는", "질문을 더하지 않는다"] {
            assert!(question.contains(needle), "{needle}: {question}");
        }
        assert!(question.contains("답을 기다린다") && question.contains("gil_close"), "the wait and the close stay: {question}");

        let interpretation = open("interpretation");
        for needle in ["확인된 사실", "에이전트의 해석", "아직 모르는 것", "Synthesis 전에 Question을 다시 연다", "Report는 짧게 쓴다"] {
            assert!(interpretation.contains(needle), "{needle}: {interpretation}");
        }

        let synthesis = open("synthesis");
        for needle in ["제안은 짧게", "목표·이번 범위·중요 제약", "수행·검증 가능한 문제 하나", "관측 가능한 완료 조건", "첫 항목이 된다", "basis_refs", "scope/items/synthesis 모양을 유지"] {
            assert!(synthesis.contains(needle), "{needle}: {synthesis}");
        }
        // The short-proposal rule precedes the basis_refs input rules; neither replaces the other.
        assert!(synthesis.find("제안은 짧게").unwrap() < synthesis.find("$.draft.synthesis.basis_refs").unwrap(), "{synthesis}");
        assert!(synthesis.contains("gil_plan_propose") && synthesis.contains("명시적 동의"), "{synthesis}");

        let define = open("define");
        for needle in ["가장 작은 문제 하나", "current_item", "Chain 전체 목표가 아니라", "멈출 지점", "한 Experiment에 묶지 않고", "단순 준비 동작", "일부만 수행하면 완료가 아니", "현재 제품에서 지원하지 않는다", "승인 범위를 바꾸지 않는다", "예/아니오로 답할 단일 명제", "새 설계는 아직 활성화되지 않았다"] {
            assert!(define.contains(needle), "{needle}: {define}");
        }
        // No size heuristic: nothing asks to count characters, conjunctions or questions.
        for hint in [&question, &interpretation, &synthesis, &define] {
            for forbidden in ["글자 수", "접속사", "질문 수", "단어 수"] {
                assert!(!hint.contains(forbidden), "{forbidden}: {hint}");
            }
        }
        // Other Steps keep their own hints.
        for kind in ["hypothesis", "verify", "analysis", "outcome"] {
            let other = open(kind);
            for stray in ["심층 인터뷰", "제안은 짧게", "가장 작은 문제 하나"] {
                assert!(!other.contains(stray), "{kind} carries {stray}: {other}");
            }
        }
        // The bound-item hint ties the Define to that item without redefining it.
        let bound = json!({"supported": true, "proposal": null, "pending": [{"id": "B"}], "reinterview_requests": [], "current_item": {"id": "A"}, "current_cycle": {"ref": "cycle:C2", "state": "open"}});
        let mut b = CallToolResult::structured(bound);
        after("gil_plan_read", &mut b);
        assert!(text(&b).contains("Define은 항목 A과 같은 문제·완료 범위") && text(&b).contains("일부만 수행하면 항목은 완료가 아니다"), "{}", text(&b));
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
        assert!(text(&r).contains(crate::locale::text("guide.interrupt_honest")), "the request hint says how to stop truthfully: {}", text(&r));
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

    #[test]
    fn plan_hints_follow_the_existing_next_field_even_with_pending_items_or_old_bindings() {
        for language in crate::locale::Locale::ALL {
            crate::locale::with(language, || {
                for key in ["ctx.plan.synthesis_closed", "ctx.next.chain_report", "ctx.next.chain_closed"] {
                    let next = crate::locale::text(key);
                    let mut result = CallToolResult::structured(json!({
                        "supported": true, "proposal": null, "pending": [{"id":"B"}],
                        "current_item": {"id":"A"}, "current_cycle": {"state":"closed"},
                        "reinterview_requests": [{"reason":"preserved"}],
                        "next": format!("{next}\nRead-only detail kept in the payload")
                    }));
                    after("gil_plan_read", &mut result);
                    assert_eq!(result.content[0].as_text().unwrap().text, next);
                }
                let mut failure = refusal_of("synthetic read failure");
                // Even a stale successful payload must not supply guidance after a failed read.
                failure.structured_content.as_mut().unwrap()["next"] = json!("stale next action");
                after("gil_plan_read", &mut failure);
                let hint = &failure.content[0].as_text().unwrap().text;
                assert_eq!(hint, crate::locale::text("guide.refused.plan_read"));
                assert!(!hint.contains("stale next action"));
            });
        }
    }

    /// Chain execution flow (2026-11-17): the Chain-close receipt and the next-Chain-open receipt
    /// are container receipts, not Step receipts. Before this, both fell back to the generic
    /// sentence (a doubled lead, "Hypothesis → Verify", "gil_close when the condition is met"),
    /// so a light model had no next call at the one boundary the context line also marks.
    #[test]
    fn chain_close_and_next_chain_open_name_the_explicit_next_chain_and_its_own_approval() {
        for verdict in ["success", "failure"] {
            let mut closed = receipt(&format!("닫았다: chain:C1 · {verdict}\nChain Report와 Exit을 보존했다. Will·Journey 판은 늘지 않는다.\n"));
            after("gil_close", &mut closed);
            let hint = text(&closed);
            assert!(hint.starts_with("GIL 진행: 이 닫기가 성공했다. 다음 행동: Chain이 닫혔다"), "{hint}");
            assert_eq!(hint.matches("이 닫기가 성공했다").count(), 1, "{hint}");
            assert!(hint.contains(verdict) && hint.contains("gil_open(kind: chain)") && hint.contains("새 주제가 있을 때만"), "{hint}");
            assert!(hint.contains("재사용하지 않는다") && hint.contains("gil_start를 부르지 않는다"), "{hint}");
            for forbidden in ["Hypothesis를 닫았다면", "gil_status를 한 번 읽는다", "gil_open(kind: experiment)", "gil_open(kind: interview)"] {
                assert!(!hint.contains(forbidden), "{forbidden}: {hint}");
            }
        }
        let mut opened = receipt("열었다: chain:C2 · Opening Interview cycle:C4\n이전 Chain Report를 gil context로 읽고 새 주제를 사용자와 확인한다.\n");
        after("gil_open", &mut opened);
        let hint = text(&opened);
        assert!(hint.starts_with("GIL 진행: 다음 Chain을 열었다"), "{hint}");
        assert!(hint.contains("gil_context") && hint.contains("gil_open(kind: question)") && hint.contains("gil_plan_propose"), "{hint}");
        assert!(hint.contains("재사용되지 않는다") && hint.contains("gil_start를 부르지 않고"), "{hint}");
        for forbidden in ["완료 조건을 충족하면", "gil_close한 뒤", "gil_open(kind: experiment)"] {
            assert!(!hint.contains(forbidden), "{forbidden}: {hint}");
        }
        // A Step or Cycle receipt is untouched by the Chain branch.
        let mut cycle = receipt("열었다: cycle:C2 · experiment · 부모 cycle:C1\n");
        after("gil_open", &mut cycle);
        assert!(text(&cycle).contains("gil_open(kind: define)"), "{}", text(&cycle));
        // The generic fallback no longer repeats its lead.
        let mut unknown = receipt("typed close receipt without the usual first line\n");
        after("gil_close", &mut unknown);
        assert_eq!(text(&unknown).matches("이 닫기가 성공했다").count(), 1, "{}", text(&unknown));
    }
}
