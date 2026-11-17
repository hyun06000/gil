//! A short index into existing records, not another state machine or a file observation.
use super::*;

pub(super) fn write(out: &mut String, project: &Project) {
    let cycle = project.current_cycle();
    let walk = cycle.steps();
    let here = walk.current().and_then(|id| walk.node(id));
    out.push_str(text("ctx.resume.heading"));
    if let Some(chain) = project.current_chain() {
        out.push_str(&tr!("ctx.resume.chain", target = chain.id(), state = state(chain.is_closed())));
    }
    out.push_str(&tr!("ctx.resume.cycle", target = cycle.id().to_ref(), kind = cycle.kind(), state = state(cycle.is_closed())));
    if let Some(node) = here {
        out.push_str(&tr!("ctx.resume.step", target = cycle.step_ref(node.id), kind = node.kind, state = state(node.is_closed())));
    }
    out.push('\n');

    // Only the current path: a later-created abandoned sibling is not this path's result.
    let last = project.current_chain().and_then(|c| c.report().map(|r| (c.id().to_string(), r)))
        .or_else(|| cycle.report().map(|r| (cycle.id().to_ref().to_string(), r)))
        .or_else(|| here.and_then(|n| walk.lineage(n.id).ok()).and_then(|path|
            path.into_iter().rev().find_map(|n| n.report.as_ref().map(|r| (cycle.step_ref(n.id).to_string(), r)))))
        .or_else(|| project.cycles().lineage(cycle.id()).ok().and_then(|path|
            path.into_iter().rev().find_map(|c| c.report().map(|r| (c.id().to_ref().to_string(), r)))));
    if let Some((target, report)) = last {
        let result = ["handoff_summary", "result", "interpretation", "statement", "response", "lesson", "problem", "hypothesis"]
            .into_iter().find_map(|key| report.get(key)).unwrap_or("");
        // Verdict-bearing reports already have a concise result. Keep their handoff/failure
        // evidence in the full report once, preserving the existing cumulative-history contract.
        let result = report.get("verdict").map(|v| format!("verdict: {v}")).unwrap_or_else(|| preview(result));
        out.push_str(&tr!("ctx.resume.result", target = target, result = result));
    } else {
        out.push_str(text("ctx.resume.no_result"));
    }

    if let Some(node) = here.filter(|n| !n.is_closed() && n.kind == NodeKind::Question) {
        let objective = project.active_will().map(|w| preview(w.objective())).unwrap_or_default();
        out.push_str(&tr!("ctx.resume.question", target = cycle.step_ref(node.id), objective = objective));
    }
    if let Some(plan) = project.approved_plan() {
        if let Some(proposal) = plan.current(project) {
            let decision = match proposal.receipt.as_ref().map(|r| &r.decision) {
                None => text("ctx.resume.waiting"),
                Some(crate::approved_plan::Decision::Approve) => text("ctx.resume.approved"),
                Some(crate::approved_plan::Decision::RequestChanges) => text("ctx.resume.changes"),
            };
            out.push_str(&tr!("ctx.resume.proposal", target = proposal.synthesis_ref, revision = proposal.revision,
                digest = proposal.digest, decision = decision, statement = preview(proposal.draft.synthesis.get("statement").map(String::as_str).unwrap_or(""))));
        }
        if let Some(binding) = plan.executed_cycles.iter().find(|e| e.cycle_ref == cycle.id().to_ref().to_string()) {
            out.push_str(&tr!("ctx.resume.binding", item = binding.item_id, revision = binding.revision));
        }
        out.push_str(text("ctx.resume.plan_details"));
    }
    out.push_str(text("ctx.resume.details"));
}

fn state(closed: bool) -> &'static str { text(if closed { "ctx.closed" } else { "ctx.open" }) }

// A clearly abbreviated, single-line excerpt; exact content stays in the existing read paths.
fn preview(value: &str) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = normalized.chars();
    let mut short: String = chars.by_ref().take(160).collect();
    if chars.next().is_some() { short.push('…'); }
    short
}
