//! Context — **새 세션이 지금 자리에서 이어 걷기 위한 것.**
//!
//! [`story`](crate::story) 와 같은 Graph 를 읽지만 독자와 책임이 다르다. story 는 사람이
//! 지금을 이해하기 위한 것이고, 여기는 **아무 대화도 물려받지 않은 Agent 가 다시 시작하기**
//! 위한 것이다(Context Model §8).
//!
//! # 해상도는 거리가 정한다
//!
//! ```text
//! 이전 Chain 들   → Chain Report 까지 (chain-core)
//! 이전 Cycle 들   → Cycle Report 까지
//! 현재 Cycle      → Step Report 까지
//! 서 있는 자리    → 상태 · 다음 행동 · 지금 지켜야 하는 규칙
//! ```
//!
//! Context Model §2 의 Context Resolution Rule 을 따른다. legacy 기록에는 Chain 절을 만들지 않는다.
//!
//! **지나온 Cycle 의 Step 은 여기 펼치지 않는다.** 그것이 이 투영의 존재 이유다 — 같은 Step
//! 기록을 Cycle 마다 다시 실어 나르면 프로젝트가 길어질수록 문맥이 그것만으로 찬다.
//! 다만 압축은 삭제가 아니다(§3). 접힌 Step Graph 는 원본 Graph 에 그대로 보존된다.
//! **어느 명령으로 그것을 펼치는지는 이 자리가 약속하지 않는다** — 전체 Graph 를 훑는
//! 책임은 장차 `gil history` 의 것이고, 아직 없는 명령을 출력에서 안내하지 않는다.
//!
//! # 아무것도 새로 저장하지 않는다
//!
//! 여기에는 상태가 없다. Cycle Report 를 다시 요약해 어딘가에 적어 두지 않고, **Graph 의
//! 참조와 Report 원본에서 그때그때 읽는다** — 요약본을 따로 두면 원본이 자라도 그쪽은
//! 낡은 채로 남는다.


use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use crate::cycle::{Cycle, CycleKind};
use crate::contract::CloseContract;
use crate::cycles::Cycles;
use crate::node::NodeKind;
use crate::project::Project;
use crate::report::Report;
use crate::rules::RuleSet;
use crate::locale::text;
use crate::tr;

/// 지금 자리에서 이어 걷기 위해 알아야 하는 것 전부.
///
/// 이전 Chain Report · 현재 Chain의 이전 Cycle · 현재 Cycle · 지금 자리. 없는 계층은 만들지 않는다.
pub fn context(project: &Project) -> String {
    let cycles = project.cycles();
    let mut out = String::new();
    write_preamble(&mut out);
    out.push_str(&format!("\n{}\n{}\n", text("context.next_call_heading"), next_call(project)));
    out.push_str(text("ctx.file_scope"));
    if let Some(plan) = project.approved_plan() {
        let chain = project.current_chain().unwrap();
        out.push_str(text("ctx.plan_profile"));
        out.push_str(&tr!("ctx.plan_state", revision = format!("{:?}", plan.active(chain).map(|p| p.revision)), pending = plan.pending(chain).iter().map(|i| i.id.as_str()).collect::<Vec<_>>().join(", "), executed = serde_json::to_string(&plan.executed_cycles).unwrap()));
        let cycle = project.current_cycle();
        if !plan.awaiting_interview(chain).is_empty() {
            out.push_str(&tr!("ctx.plan_reinterview_requests", requests = serde_json::to_string(&plan.awaiting_interview(chain)).unwrap()));
            if cycle.is_closed() {
                out.push_str(text("ctx.plan_reinterview_closed"));
            } else {
                out.push_str(text("ctx.plan_reinterview_open"));
            }
        }
        if let Some(p) = plan.current(project) { out.push_str(&tr!("ctx.plan_proposal", revision = p.revision, receipt = format!("{:?}", p.receipt))); }
        if let Some(item) = project.current_plan_item() {
            let label = if cycle.is_closed() { text("ctx.plan_label_closed") } else { text("ctx.plan_label_current") };
            out.push_str(&tr!("ctx.plan_item", label = label, id = item.id, question = item.question, done_when = item.done_when));
        }
        let verify_open = cycle.step_now_open().and_then(|id| cycle.steps().node(id))
            .is_some_and(|node| node.kind == NodeKind::Verify);
        if verify_open && project.plan_document().is_some() {
            out.push_str(text("ctx.plan_verify_doc"));
        }
    }
    if let Some(chain) = project.current_chain() {
        out.push_str(&tr!("ctx.chain_current", id = chain.id(), state = if chain.is_closed() { "closed" } else { "open" }, existence = chain.existence(), entry = chain.entry_snapshot(), cycle = chain.current_cycle()));
        if let Some(report) = chain.report() {
            out.push_str(&tr!("ctx.chain_report", verdict = report.get("verdict").unwrap(), exit = chain.exit_snapshot().unwrap(), basis = report.get("closing_synthesis_ref").unwrap(), handoff = report.get("handoff_summary").unwrap()));
        } else { out.push_str(text("ctx.chain_open_note")); }
    }
    for previous in project.chains().iter().filter(|chain| Some(chain.id()) != project.current_chain().map(|current| current.id())) {
        let report = previous.report().expect("historical Chains are closed");
        out.push_str(&tr!("ctx.chain_previous", id = previous.id(), verdict = report.get("verdict").unwrap(), basis = report.get("closing_synthesis_ref").unwrap(), handoff = report.get("handoff_summary").unwrap()));
    }
    write_previous_cycles(&mut out, cycles);
    write_failed_cycles(&mut out, cycles);
    write_current_cycle(&mut out, cycles);
    write_here(&mut out, project);
    out
}

/// **지금 바로 할 호출 하나** — 가벼운 모델이 긴 규칙을 추론하지 않아도 되도록.
///
/// 실제 기록된 위치에서 다음 호출(또는 기다림)을 한 줄로 말한다. 판정을 새로 만들지 않고
/// [`Cycles::openable_here`]·[`Cycles::can_close`]·Chain·승인 계획 기록을 그대로 읽는다.
/// 이미 성공한 동작(`gil_start`, 닫힌 Synthesis, 열린 Monitor)을 반복하라고 하지 않는다.
///
/// 승인 계획 프로필은 기록(`approved_plan`)으로만 판단한다 — OS 이름이나 설치 방식으로
/// 기능을 추측하지 않고, 옛 프로젝트에 `gil_plan_*`를 요구하지 않는다.
///
/// 보존된 재인터뷰 요청은 **알림**이다(Wave 1 검토 R4). 그것이 현재 자리의 합법적인 다음 호출을
/// 대신하지 않는다 — Define이 닫혔으면 Hypothesis, Hypothesis가 닫혔으면 Verify다. Cycle close는
/// 실제 `can_close`일 때만 권하고, 요청은 그동안 보존된다고 덧붙인다. 요청을 취소하거나
/// 현재 일을 건너뛰는 호출은 없고, 없는 중단 기능을 있는 것처럼 안내하지 않는다.
pub fn next_call(project: &Project) -> String {
    let line = next_call_here(project);
    match reinterview_notice(project) {
        Some(notice) => format!("{line} {notice}"),
        None => line,
    }
}

/// 열린 Experiment에 보존된 재인터뷰 요청이 있으면 그 사실만 한 문장으로. 닫힌 Cycle 뒤의
/// 안내는 [`plan_next_call_after_cycle`]이 이미 Interview를 가리키므로 여기서는 덧붙이지 않는다.
fn reinterview_notice(project: &Project) -> Option<&'static str> {
    let plan = project.approved_plan()?;
    let chain = project.current_chain()?;
    let cycle = project.current_cycle();
    if cycle.kind() != CycleKind::Experiment || cycle.is_closed() || plan.awaiting_interview(chain).is_empty() {
        return None;
    }
    Some(text("ctx.reinterview_notice"))
}

fn next_call_here(project: &Project) -> String {
    if let Some(chain) = project.current_chain() {
        if chain.is_closed() {
            return text("ctx.next.chain_closed").into();
        }
        if chain.awaiting_report() {
            return text("ctx.next.chain_report").into();
        }
    }
    let cycles = project.cycles();
    let cycle = cycles.current();
    let walk = cycle.steps();
    let here = walk.current().and_then(|id| walk.node(id)).filter(|node| !node.is_closed());

    if let Some(node) = here {
        if let Some(said) = plan_next_call(project, node.kind) {
            return said;
        }
        return match (cycle.kind(), node.kind) {
            (CycleKind::Interview, NodeKind::Question) => text("ctx.next.question").into(),
            (CycleKind::Interview, NodeKind::Interpretation) => text("ctx.next.interpretation").into(),
            (CycleKind::Interview, NodeKind::Synthesis) => text("ctx.next.synthesis").into(),
            (CycleKind::Interview, NodeKind::Outcome) => text("ctx.next.interview_outcome").into(),
            (CycleKind::Experiment, NodeKind::Define) => text("ctx.next.define").into(),
            (CycleKind::Experiment, NodeKind::Hypothesis) => text("ctx.next.hypothesis").into(),
            (CycleKind::Experiment, NodeKind::Verify) => text("ctx.next.verify").into(),
            (CycleKind::Experiment, NodeKind::Analysis) => text("ctx.next.analysis").into(),
            (CycleKind::Experiment, NodeKind::Outcome) => text("ctx.next.experiment_outcome").into(),
            (_, kind) => tr!("ctx.next.generic_open", kind = kind),
        };
    }

    if !cycle.is_closed() {
        if let Some(said) = plan_next_call_between_steps(project) {
            return said;
        }
        let openable = cycle.openable_here();
        if cycle.can_close() {
            return match cycle.kind() {
                CycleKind::Interview => text("ctx.next.cycle_close_interview").into(),
                CycleKind::Experiment => text("ctx.next.cycle_close_experiment").into(),
            };
        }
        if openable.is_empty() {
            return text("ctx.next.nothing").into();
        }
        let names: Vec<&str> = openable.iter().map(|kind| kind.as_str()).collect();
        return match (cycle.kind(), openable.as_slice()) {
            (CycleKind::Interview, [NodeKind::Outcome]) => text("ctx.next.synthesis_closed_outcome").into(),
            (CycleKind::Interview, [NodeKind::Question]) if walk.nodes().is_empty() => text("ctx.next.new_interview").into(),
            (CycleKind::Experiment, [NodeKind::Define]) => text("ctx.next.new_experiment").into(),
            (CycleKind::Experiment, [NodeKind::Verify]) => text("ctx.next.hypothesis_closed").into(),
            (CycleKind::Experiment, [NodeKind::Analysis]) => text("ctx.next.verify_closed").into(),
            (_, [only]) => tr!("ctx.next.open_only", kind = only.as_str()),
            _ => tr!("ctx.next.open_many", names = names.join(", ")),
        };
    }

    // 닫힌 Cycle — 밟을 수 있는 수만 말한다(`next_moves`와 같은 판정).
    if let Some(from) = cycles.pending_revisit() {
        if cfg!(feature = "product-safety") {
            return tr!("ctx.next.revisited_product", from = from.to_ref());
        }
        return tr!("ctx.next.revisited_dev", from = from.to_ref());
    }
    if cycles.can_revisit() {
        if cfg!(feature = "product-safety") {
            return tr!("ctx.next.can_revisit_product", cycle = cycle.id());
        }
        return tr!("ctx.next.can_revisit_dev", cycle = cycle.id());
    }
    if let Some(said) = plan_next_call_after_cycle(project) {
        return said;
    }
    match cycles.why_not_open_child() {
        None => match cycle.kind() {
            CycleKind::Interview => text("ctx.next.interview_closed").into(),
            CycleKind::Experiment => text("ctx.next.experiment_closed").into(),
        },
        Some(why) => tr!("ctx.next.blocked", why = why),
    }
}

/// 승인 계획 프로필에서 **열린 Step**의 다음 호출. 프로필이 아니면 `None`.
fn plan_next_call(project: &Project, kind: NodeKind) -> Option<String> {
    let plan = project.approved_plan()?;
    match kind {
        NodeKind::Synthesis => {
            let Some(proposal) = plan.current(project) else {
                return Some(text("ctx.plan.synthesis_propose").into());
            };
            Some(match proposal.receipt.as_ref().map(|r| &r.decision) {
                None => tr!("ctx.plan.waiting", revision = proposal.revision),
                Some(crate::approved_plan::Decision::Approve) => tr!("ctx.plan.approved", revision = proposal.revision),
                Some(crate::approved_plan::Decision::RequestChanges) => tr!("ctx.plan.changes", revision = proposal.revision),
            })
        }
        NodeKind::Verify if project.plan_document().is_some() => Some(text("ctx.plan.verify_doc").into()),
        // 다른 열린 Step의 다음 호출은 프로필과 무관하다. 보존된 재인터뷰 요청은 `next_call`이
        // 알림으로 덧붙이며, 그 Step의 합법적인 close를 가리지 않는다.
        _ => None,
    }
}

/// 승인 계획 프로필에서 **Step 사이**(열린 Step 없음, Cycle 열림)의 다음 호출.
fn plan_next_call_between_steps(project: &Project) -> Option<String> {
    let plan = project.approved_plan()?;
    let chain = project.current_chain()?;
    let cycle = project.current_cycle();
    if cycle.kind() == CycleKind::Interview && cycle.openable_here().as_slice() == [NodeKind::Outcome] {
        // Grammar only reaches this position through a closed `approved: yes` Synthesis, and in
        // this profile that close required the recorded Monitor receipt.
        return Some(text("ctx.plan.synthesis_closed").into());
    }
    // Experiment의 Step 사이에서는 실제 openable/can_close가 다음 호출을 정한다. 보존된
    // 재인터뷰 요청이 있어도 Cycle close를 앞당겨 권하지 않는다(R4).
    let _ = chain;
    None
}

/// 승인 계획 프로필에서 **닫힌 Cycle** 뒤의 다음 호출.
fn plan_next_call_after_cycle(project: &Project) -> Option<String> {
    let plan = project.approved_plan()?;
    let chain = project.current_chain()?;
    if !plan.awaiting_interview(chain).is_empty() {
        return Some(text("ctx.plan.reinterview_pending").into());
    }
    match plan.active(chain) {
        None => Some(text("ctx.plan.no_active").into()),
        Some(active) => {
            let pending = plan.pending(chain);
            match pending.first() {
                Some(item) => Some(tr!("ctx.plan.next_item", revision = active.revision, item = item.id, question = item.question, done_when = item.done_when)),
                None => Some(text("ctx.plan.all_done").into()),
            }
        }
    }
}

fn write_preamble(out: &mut String) {
    out.push_str(text("ctx.preamble"));
}

// ── ① 이전 Cycle — Cycle Report 해상도 ─────────────────────────────────────

/// 이 자리로 이어진 **조상** Cycle 들. 조상마다 **투영 하나**씩.
///
/// 대상은 `parent` 사슬이 지나온 Cycle 뿐이다([`Cycles::lineage`]) — 만든 순서도, 형제
/// 가지도 아니다. 이어받을 것이 있는 Cycle 은 계보 위에 있는 것들이다.
///
/// # "Cycle 해상도" 는 저장된 칸만 뜻하지 않는다
///
/// Cycle Report 는 `problem`·`success_condition`·`lesson` 을 **일부러 갖지 않는다**
/// (Cycle Model §11 — 두 자리에 같은 것을 적으면 한쪽이 낡는다). 그래서 저장된 칸만 실으면
/// 새 세션은 그 Cycle 이 무엇을 실험했는지조차 모른 채 판정만 받는다.
///
/// Context Model §7 이 정한 것은 그래서 **선택적 투영**이다: Cycle 을 대표하는 정보를
/// 원본 Graph 의 구조적 참조에서 읽되, 안의 Step Graph 를 다시 펼치지 않는다.
///
/// ```text
/// 실험 목적·성공 기준 ← 그 Cycle 의 유일하고 immutable 한 Define
/// 판정의 교훈         ← Cycle Report 의 outcome_ref 가 가리킨 Outcome
/// 판정·인수인계·다음 방향 ← Cycle Report
/// ```
///
/// **펼치는 것과 대표를 읽는 것은 다르다.** Step 의 이름도, Hypothesis·Verify·Analysis 도,
/// 되돌아가 버린 갈래의 판정도, 시간순 전개도 여기 오지 않는다. 시작점과 끝점만 읽는다.
///
/// 조상은 언제나 성공한 Cycle 이다(실패한 Cycle 은 자식을 두지 못한다). 되돌아오며 버린
/// 실패 Cycle 은 계보 위에 없으므로 이 절에 오지 않고, 바로 다음 절이 따로 싣는다 —
/// **같은 실패를 되풀이하지 않으려면 그 Report 가 반드시 전해져야 한다**(Cycle Model §10).
fn write_previous_cycles(out: &mut String, cycles: &Cycles) {
    let lineage = cycles
        .lineage(cycles.current_id())
        .expect("current 는 언제나 실재하는 Cycle 을 가리킨다");
    let ancestors = &lineage[..lineage.len() - 1];
    if ancestors.is_empty() {
        // 첫 Cycle 이면 이 절이 없다(§7). 빈 제목만 남기면 읽는 쪽은 무언가 빠졌다고 읽는다.
        return;
    }

    out.push_str(&tr!("ctx.previous_head", n = ancestors.len()));

    for cycle in ancestors {
        write_one_previous_cycle(out, cycle);
    }
}

/// **되돌아오며 버린 실패 Cycle** — 계보 위에는 없지만 반드시 전해야 하는 것.
///
/// 실패 Cycle Report 는 늘 전달한다. 같은 실패를 되풀이할 수 있기 때문이다(Cycle Model §10).
/// 그러나 그 Cycle 은 **계보의 조상이 아니다** — 여기 실린다고 해서 `parent` 사슬에 낀 것이
/// 아니고, 그 사실을 절 이름과 한 줄로 함께 말한다.
///
/// Current와 pending 출처에서 parent/revisit_from을 따라 필요한 실패를 모두 읽는다.
/// parent는 구조적 계보이고 revisit_from은 경험의 출처다. 이 투영 때문에 둘을 같은
/// 계보로 바꾸지 않는다. 성공한 자식으로 진행하거나 더 먼 조상으로 돌아가도 경험은 남는다.
///
/// 조상과 **같은 해상도**로 싣는다 — 안의 Step Graph 는 펼치지 않는다.
fn write_failed_cycles(out: &mut String, cycles: &Cycles) {
    let failures = failure_context(cycles);
    if failures.is_empty() { return; }

    out.push_str(&tr!("ctx.failed_head", n = failures.len()));
    for cycle in failures {
        write_one_previous_cycle(out, cycle);
        out.push_str(&tr!("ctx.failed_source", cycle = cycle.id().to_ref()));
    }
}

/// Read-only causal closure, not a scan of every failure in storage. Dependencies precede
/// their consumers; neither ID magnitude nor serialized array order defines experience.
/// Iterative traversal avoids stack growth and repeated expansion of shared ancestors.
fn failure_context(cycles: &Cycles) -> Vec<&Cycle> {
    let nodes: BTreeMap<_, _> = cycles.nodes().iter().map(|c| (c.id(), c)).collect();
    let mut seen = BTreeSet::new();
    let mut stack = Vec::new();
    if let Some(from) = cycles.pending_revisit() { stack.push((from, false)); }
    stack.push((cycles.current_id(), false));
    let mut failures = Vec::new();
    while let Some((id, expanded)) = stack.pop() {
        let cycle = nodes[&id]; // restored Project already validates all structural references
        if expanded {
            // Current's full Report is already in the current-Cycle section.
            if id != cycles.current_id() && cycle.is_closed()
                && cycle.report().and_then(|r| r.get(VERDICT)) == Some("failure") {
                failures.push(cycle);
            }
        } else if seen.insert(id) {
            stack.push((id, true));
            if let Some(from) = cycle.revisit_from() { stack.push((from, false)); }
            if let Some(parent) = cycle.parent() { stack.push((parent, false)); }
        }
    }
    failures
}

/// 조상 하나의 투영 — **명세가 고른 자리만.**
///
/// 여기는 화이트리스트다. Cycle Report 에 칸이 늘어도 이 절은 자동으로 넓어지지 않는다 —
/// 새 칸이 story·context·history 중 어디에 필요한지는 명세가 정하고, 정해진 뒤에 이 자리에
/// 명시적으로 온다. 자동 노출은 편해 보이지만, 그러면 "새 세션이 무엇을 받는가" 를 정하는
/// 자리가 명세가 아니라 Report 를 적는 사람이 된다.
fn write_one_previous_cycle(out: &mut String, cycle: &Cycle) {
    let _ = write!(out, "\n{} · {}", cycle.id(), cycle.kind());
    let Some(report) = cycle.report() else {
        // 닫힌 Cycle 은 Report 를 지닌다(그것이 닫힘의 조건이다). 그래도 파일이 두 번째
        // 통로라 여기 닿을 수 있다 — 그때는 없다고 말한다. 조용히 건너뛰면 이어받을
        // 것이 하나 사라진 줄 모른다.
        out.push_str(text("ctx.no_cycle_report"));
        return;
    };
    match report.get(VERDICT) {
        Some(verdict) => {
            let _ = writeln!(out, " · {verdict}");
        }
        None => {
            let _ = writeln!(out);
        }
    }

    let mut projected: Vec<(&str, String)> = Vec::new();
    if let Some(define) = cycle.define() {
        if let Some(problem) = define.get(PROBLEM) {
            projected.push((text("ctx.label.purpose"), problem.to_string()));
        }
        if let Some(condition) = define.get(SUCCESS_CONDITION) {
            projected.push((text("ctx.label.success"), condition.to_string()));
        }
    }
    if let Some(lesson) = cycle.judged_lesson() {
        projected.push((text("ctx.label.lesson"), lesson.to_string()));
    }
    if let Some(summary) = report.get(HANDOFF_SUMMARY) {
        projected.push((text("ctx.label.handoff"), summary.to_string()));
    }
    if let Some(direction) = next_direction_of(report) {
        projected.push((text("ctx.label.direction"), direction));
    }
    write_block(out, &projected);
}

/// 다음 방향 한 덩이 — 적힌 동작 그대로, 그 아래에 까닭.
///
/// 동작을 사람의 말로 옮기지 않는다. 이 글의 독자는 그 낱말 그대로 다음 Cycle Report 를
/// 적어야 하는 Agent 다(사람의 말로 옮기는 것은 `gil story` 의 몫이다).
fn next_direction_of(report: &Report) -> Option<String> {
    let action = report.get(NEXT_ACTION)?;
    Some(match report.get(NEXT_REASON) {
        Some(reason) if !reason.is_empty() => format!("{action}\n{reason}"),
        _ => action.to_string(),
    })
}

/// 이름표 붙은 값들 — 빈 줄로 갈리고, 값은 한 단 더 들여쓴다.
fn write_block(out: &mut String, fields: &[(&str, String)]) {
    for (label, value) in fields {
        let _ = writeln!(out, "\n  {label}");
        for line in value.lines() {
            let _ = writeln!(out, "    {line}");
        }
    }
}

// ── ② 현재 Cycle — Step Report 해상도 ──────────────────────────────────────

/// 지금 Cycle 안에서 지금까지 난 Step 들. **닫힌 것은 Report 째로, 열린 것은 상태로.**
fn write_current_cycle(out: &mut String, cycles: &Cycles) {
    let cycle = cycles.current();
    out.push_str(&tr!("ctx.current_head", id = cycle.id(), kind = cycle.kind(),
        state = if cycle.is_closed() { text("ctx.closed") } else { text("ctx.open") }));
    match cycle.parent() {
        Some(parent) => {
            out.push_str(&tr!("ctx.parent", parent = parent));
            // 이어받은 것을 여기 다시 적지 않는다 — 원본은 바로 위 절에 이미 있다.
            match cycles.inherited_report(cycle.id()).is_some() {
                true => out.push_str(&tr!("ctx.inherited", parent = parent)),
                false => out.push_str(&tr!("ctx.inherited_none", parent = parent)),
            };
        }
        None => {
            out.push_str(text("ctx.root"));
        }
    }

    let walk = cycle.steps();
    if walk.nodes().is_empty() {
        out.push_str(text("ctx.no_steps"));
        return;
    }

    for node in walk.nodes() {
        let _ = write!(out, "\n{} {} · {}", node.id, node.kind, node.status);
        if let Some(from) = node.revisit_from {
            out.push_str(&tr!("ctx.branched_from", from = from));
        }
        let _ = writeln!(out);
        match &node.report {
            Some(report) => {
                let declared = step_close_requires(cycle.rules(), cycle.kind(), node.kind);
                write_report_fields(out, &declared, report, "  ");
            }
            None => {
                out.push_str(text("ctx.no_report_yet"));
            }
        }
    }

    if let Some(report) = cycle.report() {
        out.push_str(text("ctx.cycle_report_head"));
        let declared = cycle_close_requires(cycle);
        write_report_fields(out, &declared, report, "  ");
    }
}

// ── ③ 지금 자리 — 상태 · 다음 행동 · 규칙 ──────────────────────────────────

fn write_here(out: &mut String, project: &Project) {
    let cycles = project.cycles();
    let cycle = cycles.current();
    let walk = cycle.steps();

    out.push_str(&tr!("ctx.here_head", id = cycle.id(), kind = cycle.kind(),
        state = if cycle.is_closed() { text("ctx.closed") } else { text("ctx.open") }));
    match walk.current().and_then(|id| walk.node(id)) {
        Some(node) => {
            out.push_str(&tr!("ctx.position", id = node.id, kind = node.kind, status = node.status));
        }
        None => {
            out.push_str(text("ctx.position_none"));
        }
    }
    out.push_str(&tr!("ctx.walked", cycles = cycles.nodes().len(), steps = walk.nodes().len(), closed = walk.history().count()));

    write_current_will(out, project);

    out.push_str(text("ctx.next_moves_head"));
    match project.current_chain() {
        Some(chain) if chain.is_closed() => out.push_str(text("ctx.chain_closed_moves")),
        Some(chain) if chain.awaiting_report() => out.push_str(&chain.close_hint()),
        _ => out.push_str(&next_moves(cycles)),
    }

    write_rules_here(out, cycles);
    write_report_principles(out);
}

/// **지금 무엇을 하려는가.**
///
/// 이 절이 없어서 값을 치렀다(Roadmap M2C): 상태와 가설은 정확히 복원했는데 열린 Verify 에서
/// 무엇을 먼저 해야 하는지 몰라, 검증하기도 전에 `gil close` 를 부른 Agent 가 있었다.
///
/// 지나온 행동들을 여기서 펼치지 않는다 — 이어 걷는 데 필요한 것은 **지금 걸린 하나**다.
/// 없으면 없다고 말한다. GIL 은 다음 행동을 추측하지 않는다(Will Model §10).
fn write_current_will(out: &mut String, project: &Project) {
    let existence = project.current_existence();
    out.push_str(&tr!("ctx.will_head", existence = existence.id(), journey = existence.current_journey()));
    let Some(will) = project.active_will() else {
        out.push_str(text("ctx.will_none"));
        return;
    };
    out.push_str(&tr!("ctx.will", will = will.id(), target = will.target(),
        objective = indent_more(will.objective()), action = indent_more(will.next_action()), done = indent_more(will.done_when())));
}

/// 여러 줄로 적힌 값이 다음 항목처럼 보이지 않게.
fn indent_more(value: &str) -> String {
    value.replace('\n', "\n    ")
}

/// 지금 할 수 있는 것들 — **판정을 여기서 새로 쓰지 않는다.**
///
/// 실행하는 그 자리에게 묻는다([`Cycles::openable_here`]·[`Cycles::can_close`]·
/// [`Cycles::why_not_open_child`]). 안내와 실행이 갈리면 안내를 믿은 Agent 가 한 번
/// 실패하고서야 옳은 수를 알게 된다(실사용 보고 #123).
///
/// `gil status` 와 `gil context` 가 **같은 이 함수**를 읽는다. 두 자리에 적으면 한쪽이 낡는다.
pub fn next_moves(cycles: &Cycles) -> String {
    let cycle = cycles.current();

    if !cycle.is_closed() {
        let here = cycle.steps().current().and_then(|id| cycle.steps().node(id));
        if here.is_some_and(|node| !node.is_closed()) {
            return text("moves.close_here").to_string();
        }

        let mut out = String::new();
        let openable = cycle.openable_here();
        let names: Vec<&str> = openable.iter().map(|kind| kind.as_str()).collect();
        if !names.is_empty() {
            out.push_str(&tr!("moves.openable", names = names.join(", ")));
        }
        if cycle.can_close() {
            let lead = match names.is_empty() {
                true => text("moves.next"),
                false => text("moves.or"),
            };
            out.push_str(&tr!("moves.close_cycle", lead = lead));
        }
        if out.is_empty() {
            out.push_str(text("moves.nothing"));
        }
        return out;
    }

    // **되돌아온 자리** — 여기서 할 일은 하나뿐이다.
    if let Some(from) = cycles.pending_revisit() {
        return tr!("moves.revisited", from = from.to_ref());
    }

    // 닫힌 Cycle 이 되돌아가겠다고 적었는가 — **밟아 보고 답한다.**
    if cycles.can_revisit() {
        if cfg!(feature = "product-safety") {
            return text("moves.revisit_product").into();
        }
        return tr!("moves.revisit_dev", cycle = cycle.id());
    }

    // 닫힌 Cycle — 적어 둔 방향을 밟을 수 있는가.
    match cycles.why_not_open_child() {
        None => tr!("moves.open_next", cycle = cycle.id()),
        Some(why) => tr!("moves.blocked", why = why),
    }
}

/// 지금 무언가를 닫으려면 무엇이 있어야 하는가 — **명세에서 그대로 읽는다.**
///
/// 칸의 목록도, 값의 제약도 여기 옮겨 적지 않는다. `gil-spec.yaml` 이 자라면 이 절도 함께
/// 자란다 — 옮겨 적으면 명세가 바뀐 날부터 새 세션이 낡은 규칙을 받는다.
fn write_rules_here(out: &mut String, cycles: &Cycles) {
    let cycle = cycles.current();
    if cycle.is_closed() {
        // 닫힌 Cycle 에서는 아무 Report 도 쓰지 않는다. 다음 수는 Cycle 을 여는 것이고,
        // 그것은 바로 위 절이 이미 말했다.
        return;
    }

    let walk = cycle.steps();
    let here = walk.current().and_then(|id| walk.node(id));

    // 계약을 읽는 자리는 하나다([`CloseContract`]) — Receipt·help·거절과 같은 것을 본다.
    let mut blocks: Vec<CloseContract> = Vec::new();
    match here.filter(|node| !node.is_closed()) {
        // 열려 있는 자리가 있으면 지금 필요한 규칙은 그 자리를 닫는 규칙 하나뿐이다.
        Some(node) => blocks.extend(cycle.step_contract(node.id, node.kind)),
        None => {
            for kind in cycle.openable_here() {
                blocks.extend(cycle.contract_of_kind(kind));
            }
            if cycle.can_close() {
                blocks.extend(cycle.close_contract());
            }
        }
    }

    if blocks.is_empty() {
        return;
    }
    out.push_str(text("ctx.rules_head"));
    for (index, contract) in blocks.into_iter().enumerate() {
        // 블록은 빈 줄로 갈린다 — 색에 기대지 않고 눈으로 갈려야 한다.
        if index > 0 {
            out.push('\n');
        }
        let _ = writeln!(out, "{}", contract.subject());
        out.push_str(&contract.constraints("  "));
    }
}

/// Report 를 쓰는 원칙.
///
/// 문법이 판정하지 못하는 **의미의 불변식**이라 `gil-spec.yaml` 에 없다
/// (GIL Specification v0.1 「Report 작성 불변식」이 원본이고, 여기는 그 요약이다).
/// 새 세션에는 이것이 필요하다 — 없이 걸은 실제 dogfood 가 정의되지 않은 내부 단계명
/// `S1` 을 Report 에 남겼고, 그 Cycle 에 없던 사람은 그 Report 를 읽지 못했다.
fn write_report_principles(out: &mut String) {
    out.push_str(text("ctx.report_principles"));
}

// ── 명세에서 읽어 오는 것들 ────────────────────────────────────────────────

/// 이전 Cycle 절이 원본에서 읽어 오는 칸의 이름 — `gil-spec.yaml` 이 부르는 그대로.
use crate::report::field::{
    HANDOFF_SUMMARY, NEXT_ACTION, NEXT_REASON, PROBLEM, SUCCESS_CONDITION, VERDICT,
};

fn step_close_requires(rules: &RuleSet, cycle: CycleKind, kind: NodeKind) -> Vec<String> {
    rules
        .rules(cycle, kind)
        .map(|rules| rules.close_requires.clone())
        .unwrap_or_default()
}

fn cycle_close_requires(cycle: &Cycle) -> Vec<String> {
    cycle
        .rules()
        .cycle_rules(cycle.kind())
        .map(|rules| rules.close_requires.clone())
        .unwrap_or_default()
}

// ── Report 를 적는 꼴 ──────────────────────────────────────────────────────

/// Report 의 칸들을 **명세가 정한 순서**로, 그 뒤에 명세가 모르는 칸을 이어서.
///
/// 이름을 사람의 말로 옮기지 않는다 — 이 글의 독자는 이 칸 이름 그대로 다음 Report 를
/// 적어야 하는 Agent 다(사람의 말로 읽는 것은 `gil story` 의 몫이다).
fn write_report_fields(out: &mut String, declared: &[String], report: &Report, indent: &str) {
    let extra: Vec<&str> = report
        .field_names()
        .filter(|name| !declared.iter().any(|field| field == name))
        .collect();

    for field in declared.iter().map(String::as_str).chain(extra) {
        let Some(value) = report.get(field) else {
            continue;
        };
        match value.contains('\n') {
            // 여러 줄은 이름만 두고 아래로 들여쓴다 — 어디까지가 한 칸의 값인지 눈으로 갈린다.
            true => {
                let _ = writeln!(out, "{indent}{field}:");
                for line in value.lines() {
                    let _ = writeln!(out, "{indent}  {line}");
                }
            }
            false => {
                let _ = writeln!(out, "{indent}{field}: {value}");
            }
        }
    }
}

// ── 승인 계획 프로필의 다음 호출 — Core 의 합성 fixture 로 잰다 ──────────────
//
// Monitor 버튼 응답은 `decide_plan` 의 합성 입력이며 실제 사람의 승인 증거가 아니다.
#[cfg(all(test, feature = "approved-plan"))]
mod plan_guidance_tests {
    use super::*;
    use crate::approved_plan::{Decision, tests::synthesis};
    use crate::plan_test_helpers as h;

    fn first_line(project: &Project) -> String {
        let told = context(project);
        let (_, rest) = told.split_once("[지금 바로 할 호출]\n").expect("첫 블록이 없다");
        rest.lines().next().unwrap().to_string()
    }
    fn says(project: &Project, must: &[&str], must_not: &[&str]) {
        let line = first_line(project);
        for m in must { assert!(line.contains(m), "{m:?} 가 없다: {line}"); }
        for n in must_not { assert!(!line.contains(n), "{n:?} 를 제안했다: {line}"); }
    }
    fn close_interview_after_approval(p: &mut Project, approved: crate::approved_plan::Proposal) {
        let target = approved.synthesis_ref.clone();
        p.close_action_step(approved.report().unwrap()).unwrap();
        says(p, &["gil_open(kind: outcome)", "gil_plan_propose를 반복하지 않는다"], &["gil_plan_propose로", "gil_close"]);
        let outcome = h::opened(p, NodeKind::Outcome);
        p.close_action_step(h::full_report(p.current_cycle().rules(), CycleKind::Interview, NodeKind::Outcome).with("synthesis_ref", target)).unwrap();
        says(p, &["Interview Cycle은 아직 열려 있다", "gil_close"], &["gil_open(kind: experiment)"]);
        p.close_cycle(h::cycle_report(p.current_cycle(), "success", h::step_id(p, outcome)).with("next_direction.action", "open_child")).unwrap();
    }

    #[test]
    fn synthesis_waits_for_the_monitor_button_and_follows_the_recorded_decision() {
        let mut p = Project::start(h::spec(), h::first_world());
        let draft = synthesis(&mut p, &["A", "B"]);
        says(&p, &["gil_plan_propose({project_root, draft:{scope, items, synthesis}})", "scope 5칸", "draft.synthesis.exploration_checklist", "draft.synthesis.chain_verdict", "approved · plan_revision · plan_digest는 적지 않는다"], &["approved: yes", "gil_close"]);
        let first = p.propose_plan(draft.clone()).unwrap();
        says(&p, &["제안 판 1", "Monitor 버튼 응답을 기다린다", "턴을 끝내고"], &["gil_close", "gil_plan_propose", "조사를 시작"]);
        // 승인 후 제안을 고치면 새 판이다 — 이전 승인은 재사용되지 않는다.
        p.decide_plan(first.revision, &first.digest, Decision::Approve).unwrap();
        says(&p, &["제안 판 1이(가) 승인됐다", "close_report를 그대로 gil_close", "다시 묻지 않는다"], &["gil_plan_propose"]);
        let mut edited = draft.clone();
        edited.scope.goal = "Edited".into();
        let second = p.propose_plan(edited).unwrap();
        says(&p, &["제안 판 2", "Monitor 버튼 응답을 기다린다"], &["승인됐다", "close_report"]);
        p.decide_plan(second.revision, &second.digest, Decision::RequestChanges).unwrap();
        says(&p, &["수정 요청", "approved: no", "다음 Question", "이전 승인은 재사용하지 않는다"], &["gil_open(kind: outcome)"]);
    }

    #[test]
    fn approved_interview_binds_the_first_pending_item_and_verify_separates_plan_from_evidence() {
        let mut p = Project::start(h::spec(), h::first_world());
        let draft = synthesis(&mut p, &["A", "B"]);
        let proposal = p.propose_plan(draft).unwrap();
        let approved = p.decide_plan(proposal.revision, &proposal.digest, Decision::Approve).unwrap();
        close_interview_after_approval(&mut p, approved);
        says(&p, &["승인 판 1", "다음 항목 A", "gil_open(kind: experiment)", "Question A"], &["gil_start", "gil_plan_propose"]);
        p.open_child_cycle(CycleKind::Experiment).unwrap();
        says(&p, &["gil_open(kind: define)"], &["plan_document"]);
        h::up_to_verify(&mut p);
        says(&p, &["bytes 그대로", "plan_document", "evidence_files", "계획 문서만으로는 닫히지 않는다"], &["gil_open"]);
        let told = context(&p);
        assert!(told.contains("승인 계획 프로필") && told.contains("이번 Experiment: A"), "{told}");
    }

    /// R4 — a preserved re-interview request is announced, but the first line still names the
    /// call Core will actually accept at each position (Hypothesis, Verify, …, Cycle Report).
    #[test]
    fn a_reinterview_request_is_preserved_without_hiding_the_legal_next_call() {
        let mut p = Project::start(h::spec(), h::first_world());
        let draft = synthesis(&mut p, &["A", "B"]);
        let proposal = p.propose_plan(draft).unwrap();
        let approved = p.decide_plan(proposal.revision, &proposal.digest, Decision::Approve).unwrap();
        close_interview_after_approval(&mut p, approved);
        p.open_child_cycle(CycleKind::Experiment).unwrap();
        h::opened(&mut p, NodeKind::Define);
        p.request_plan_interview("scope changed".into()).unwrap();
        // Open Define: its own close is the call; the request is a notice, not a replacement.
        says(&p, &["열린 Define", "gil_close", "재인터뷰 요청이 보존"], &["gil_open(kind: interview)", "gil_open(kind: experiment)", "Cycle을 사실대로 닫는다"]);
        p.close_action_step(h::full_report(p.current_cycle().rules(), CycleKind::Experiment, NodeKind::Define)).unwrap();
        assert!(!p.current_cycle().can_close());
        assert_eq!(p.current_cycle().openable_here(), vec![NodeKind::Hypothesis]);
        says(&p, &["gil_open(kind: hypothesis)", "재인터뷰 요청이 보존"], &["gil_open(kind: interview)", "Cycle Report", "Cycle을 사실대로 닫는다"]);
        h::walked(&mut p, NodeKind::Hypothesis);
        says(&p, &["gil_open(kind: verify)", "재인터뷰 요청이 보존"], &["gil_open(kind: interview)", "Cycle Report"]);
        h::opened(&mut p, NodeKind::Verify);
        says(&p, &["열린 Verify", "plan_document", "evidence_files", "재인터뷰 요청이 보존"], &["gil_open"]);
        p.close_verify_step(h::full_report(p.current_cycle().rules(), CycleKind::Experiment, NodeKind::Verify), h::first_world()).unwrap();
        says(&p, &["gil_open(kind: analysis)", "재인터뷰 요청이 보존"], &["Cycle Report", "gil_open(kind: interview)"]);
        h::walked(&mut p, NodeKind::Analysis);
        h::walked(&mut p, NodeKind::Outcome);
        // Only now can the Cycle close — and only now is that the advice.
        assert!(p.current_cycle().can_close());
        says(&p, &["Experiment Cycle은 아직 열려 있다", "gil_close로 Cycle Report", "재인터뷰 요청이 보존"], &["gil_open"]);
        let last = p.current_cycle().steps().current().unwrap();
        p.close_cycle(h::cycle_report(p.current_cycle(), "success", last).with("next_direction.action", "open_child")).unwrap();
        says(&p, &["gil_open(kind: interview)", "다시 승인받는다"], &["gil_open(kind: experiment)"]);
        let told = context(&p);
        assert!(told.contains("보존된 재인터뷰 요청"), "{told}");
        assert!(!told.contains("현재 Cycle을 사실대로 닫은 후"), "the body still tells the model to close the Cycle first:\n{told}");
    }

    /// After the last approved item, the Chain still needs a Closing Interview and a Chain Report —
    /// chat alone is never the completion.
    #[test]
    fn all_items_executed_leads_to_a_closing_interview_not_a_chat_report() {
        let mut p = Project::start(h::spec(), h::first_world());
        let draft = synthesis(&mut p, &["A"]);
        let proposal = p.propose_plan(draft).unwrap();
        let approved = p.decide_plan(proposal.revision, &proposal.digest, Decision::Approve).unwrap();
        close_interview_after_approval(&mut p, approved);
        p.open_child_cycle(CycleKind::Experiment).unwrap();
        for kind in [NodeKind::Define, NodeKind::Hypothesis] { h::walked(&mut p, kind); }
        h::opened(&mut p, NodeKind::Verify);
        p.close_verify_step(h::full_report(p.current_cycle().rules(), CycleKind::Experiment, NodeKind::Verify), h::first_world()).unwrap();
        for kind in [NodeKind::Analysis, NodeKind::Outcome] { h::walked(&mut p, kind); }
        let last = p.current_cycle().steps().current().unwrap();
        p.close_cycle(h::cycle_report(p.current_cycle(), "success", last).with("next_direction.action", "open_child")).unwrap();
        assert!(p.approved_plan().unwrap().pending(p.current_chain().unwrap()).is_empty());
        says(&p, &["모두 실행했다", "gil_open(kind: interview)", "Closing Interview", "Chain Report", "채팅 보고만으로"], &["gil_open(kind: experiment)", "완료 보고는 사용자에게 한다"]);
    }

    #[test]
    fn a_plan_project_without_an_active_plan_is_sent_to_interview_not_experiment() {
        let mut p = Project::start(h::spec(), h::first_world());
        // Synthesis 를 승인 없이 닫을 수 없으므로 Interview 는 열린 채 Question 으로 돌아간다.
        let draft = synthesis(&mut p, &["A"]);
        let proposal = p.propose_plan(draft).unwrap();
        p.decide_plan(proposal.revision, &proposal.digest, Decision::RequestChanges).unwrap();
        p.close_action_step(proposal.report().unwrap_or_else(|_| p.approved_plan().unwrap().current(&p).unwrap().report().unwrap())).unwrap();
        let line = first_line(&p);
        assert!(line.contains("gil_open") && line.contains("question"), "{line}");
        assert!(!line.contains("experiment"), "{line}");
    }
}
