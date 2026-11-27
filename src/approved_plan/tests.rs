use super::*;
use crate::plan_test_helpers as h;

pub(crate) fn synthesis(p: &mut Project, ids: &[&str]) -> Draft {
    for kind in [NodeKind::Question, NodeKind::Interpretation] {
        h::opened(p, kind);
        p.close_action_step(h::full_report(
            p.current_cycle().rules(),
            CycleKind::Interview,
            kind,
        ))
        .unwrap();
    }
    h::opened(p, NodeKind::Synthesis);
    let c = p.current_cycle();
    let mut report = h::full_report(c.rules(), CycleKind::Interview, NodeKind::Synthesis);
    report.remove("approved");
    report.insert("exploration_checklist", ids.join("\n"));
    report.insert(
        "basis_refs",
        c.steps()
            .nodes()
            .iter()
            .filter(|n| {
                n.is_closed() && matches!(n.kind, NodeKind::Question | NodeKind::Interpretation)
            })
            .map(|n| c.step_ref(n.id).to_string())
            .collect::<Vec<_>>()
            .join("\n"),
    );
    Draft {
        scope: Scope {
            goal: "Report".into(),
            in_scope: "Synthetic fixture".into(),
            out_of_scope: "Trading".into(),
            constraints: "No external data".into(),
            success_criteria: "Saved evidence".into(),
        },
        items: ids
            .iter()
            .map(|id| Item {
                id: (*id).into(),
                question: format!("Question {id}"),
                done_when: format!("Evidence {id}"),
                source: String::new(),
                command: String::new(), part_of: String::new(),
            })
            .collect(),
        synthesis: report
            .field_names()
            .map(|key| (key.into(), report.get(key).unwrap().into()))
            .collect(),
    }
}
pub(crate) fn approve_close(p: &mut Project, draft: Draft) {
    approve_close_as(p, draft, "open_child");
}
fn approve_close_as(p: &mut Project, draft: Draft, action: &str) {
    let proposal = p.propose_plan(draft).unwrap();
    let got = p
        .decide_plan(proposal.revision, &proposal.digest, Decision::Approve)
        .unwrap();
    let target = got.synthesis_ref.clone();
    p.close_action_step(got.report().unwrap()).unwrap();
    assert!(
        p.open_child_cycle(CycleKind::Experiment).is_err(),
        "even approved Synthesis is not a closed Interview"
    );
    let outcome = h::opened(p, NodeKind::Outcome);
    p.close_action_step(
        h::full_report(
            p.current_cycle().rules(),
            CycleKind::Interview,
            NodeKind::Outcome,
        )
        .with("synthesis_ref", target),
    )
    .unwrap();
    p.close_cycle(
        h::cycle_report(p.current_cycle(), "success", h::step_id(p, outcome))
            .with("next_direction.action", action),
    )
    .unwrap();
}
/// A Project whose first Chain carries the original item-queue contract.
///
/// An `exploration-flow` build opens new Chains under the exploration contract, yet it must keep
/// running Chains that were opened before it. Every test of the queue contract therefore starts
/// from this fixture, so the same suite holds in both builds.
pub(crate) fn project() -> Project {
    Project::start(h::spec(), h::first_world()).as_item_queue_for_test()
}

#[test]
fn approval_requires_current_monitor_decision_preserves_journey_and_cannot_flip_receipt() {
    let mut p = project();
    let d = synthesis(&mut p, &["A", "B", "C"]);
    let before = p.current_existence().current_journey();
    let bare: Report = d.synthesis.clone().into_iter().collect();
    assert!(p.close_action_step(bare.with("approved", "yes")).is_err());
    let first = p.propose_plan(d.clone()).unwrap();
    assert_eq!(p.propose_plan(d.clone()).unwrap(), first);
    let mut edited = d.clone();
    edited.scope.goal = "Edited".into();
    let second = p.propose_plan(edited.clone()).unwrap();
    assert_eq!(second.revision, 2);
    assert!(
        p.decide_plan(first.revision, &first.digest, Decision::Approve)
            .is_err()
    );
    assert_eq!(p.current_existence().current_journey(), before);
    let accepted = p
        .decide_plan(second.revision, &second.digest, Decision::RequestChanges)
        .unwrap();
    assert_eq!(
        accepted,
        p.decide_plan(second.revision, &second.digest, Decision::RequestChanges)
            .unwrap()
    );
    assert!(
        p.decide_plan(second.revision, &second.digest, Decision::Approve)
            .is_err()
    );
    assert_eq!(p.propose_plan(edited).unwrap(), accepted);
    assert!(
        p.close_action_step(accepted.report().unwrap().with("statement", "changed"))
            .is_err()
    );
    p.close_action_step(accepted.report().unwrap()).unwrap();
    h::opened(&mut p, NodeKind::Question);
    assert!(p.open_child_cycle(CycleKind::Experiment).is_err());
}
#[test]
fn executed_a_then_reinterview_inserts_d_without_rewriting_a_or_old_approval() {
    let root = h::scratch("plan-queue");
    let mut p = project();
    let d = synthesis(&mut p, &["A", "B", "C"]);
    approve_close(&mut p, d);
    let old = p.approved_plan().unwrap().proposals[0].clone();
    p.open_child_cycle(CycleKind::Experiment).unwrap();
    let binding = p.approved_plan().unwrap().executed_cycles[0].clone();
    let here = p.current_cycle().id();
    assert!(p.revisit_cycle().is_err());
    assert_eq!(p.current_cycle().id(), here);
    assert!(p.cycles().pending_revisit().is_none());
    h::opened(&mut p, NodeKind::Define);
    let active = p.active_will().unwrap().target();
    p.request_plan_interview("D should come next".into())
        .unwrap();
    p.request_plan_interview("D should come next".into())
        .unwrap();
    assert_eq!(p.approved_plan().unwrap().reinterview_requests.len(), 1);
    assert_eq!(p.active_will().unwrap().target(), active);
    assert!(p.open_child_cycle(CycleKind::Interview).is_err());
    p.close_action_step(h::full_report(
        p.current_cycle().rules(),
        CycleKind::Experiment,
        NodeKind::Define,
    ))
    .unwrap();
    for kind in [
        NodeKind::Hypothesis,
        NodeKind::Verify,
        NodeKind::Analysis,
        NodeKind::Outcome,
    ] {
        h::walked(&mut p, kind);
    }
    let last = p.current_cycle().steps().current().unwrap();
    p.close_cycle(
        h::cycle_report(p.current_cycle(), "success", last)
            .with("next_direction.action", "open_child"),
    )
    .unwrap();
    assert!(p.open_child_cycle(CycleKind::Experiment).is_err());
    p.open_child_cycle(CycleKind::Interview).unwrap();
    let d = synthesis(&mut p, &["A", "D", "B", "C"]);
    let mut changed = d.clone();
    changed.items[0].question = "changed question".into();
    changed.items[0].done_when = "rewritten".into();
    let before = p.approved_plan().unwrap().clone();
    let error = p.propose_plan(changed).unwrap_err();
    for text in ["draft.items[id=\"A\"].question", "draft.items[id=\"A\"].done_when", "Question A", "changed question", "Evidence A", "rewritten", "active_plan.items"] {
        assert!(error.contains(text), "{error}");
    }
    assert_eq!(p.approved_plan().unwrap(), &before);
    let mut removed = d.clone();
    removed.items.retain(|i| i.id != "B");
    removed.synthesis.insert("exploration_checklist".into(), "A\nD\nC".into());
    let error = p.propose_plan(removed).unwrap_err();
    assert!(error.contains("draft.items[id=\"B\"]: 누락"), "{error}");
    assert!(error.contains(crate::locale::text("plan.err.no_cancel")), "dropping an item is named as the undecided cancel contract: {error}");
    assert_eq!(p.approved_plan().unwrap(), &before);
    let mut d = d;
    d.scope.in_scope = "New scope for inserted D".into();
    approve_close(&mut p, d);
    let state = p.approved_plan().unwrap();
    assert_eq!(state.proposals[0], old);
    assert_eq!(state.executed_cycles[0], binding);
    assert_eq!(
        state
            .pending(p.current_chain().unwrap())
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        vec!["D", "B", "C"]
    );
    p.open_child_cycle(CycleKind::Experiment).unwrap();
    assert_eq!(p.approved_plan().unwrap().executed_cycles[1].item_id, "D");
    let path = root.join(crate::STATE_PATH);
    crate::save(&p, &path).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let loaded = crate::load(h::spec(), &path).unwrap();
    crate::save(&loaded, &path).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_eq!(loaded.plan_document(), p.plan_document());
    let mut corrupt: serde_norway::Value = serde_norway::from_slice(&bytes).unwrap();
    corrupt["chains"]["nodes"][0]["approved_plan"]["executed_cycles"][1]["item_id"] =
        serde_norway::Value::String("unknown".into());
    std::fs::write(&path, serde_norway::to_string(&corrupt).unwrap()).unwrap();
    assert!(crate::load(h::spec(), &path).is_err());
}
#[test]
fn verify_requires_bound_document_and_separate_fresh_execution_evidence() {
    let root = h::scratch("plan-document");
    let mut p = project();
    let d = synthesis(&mut p, &["A"]);
    approve_close(&mut p, d);
    p.open_child_cycle(CycleKind::Experiment).unwrap();
    h::up_to_verify(&mut p);
    let path = root.join(crate::STATE_PATH);
    crate::save(&p, &path).unwrap();
    let mut session = crate::ProjectSession::open(h::spec(), &path).unwrap();
    let doc = session.project().plan_document().unwrap();
    std::fs::write(root.join("plan.md"), &doc).unwrap();
    let r = h::full_report(
        session.project().current_cycle().rules(),
        CycleKind::Experiment,
        NodeKind::Verify,
    )
    .with("plan_document", "plan.md")
    .with("evidence_files", "plan.md");
    assert!(
        session.close_step(r.clone()).is_err(),
        "plan alone is not execution evidence"
    );
    std::fs::write(root.join("report.md"), "Observed synthetic result").unwrap();
    std::fs::write(root.join("plan.md"), "Wrong revision").unwrap();
    let r = r.with("evidence_files", "report.md");
    assert!(session.close_step(r.clone()).is_err());
    std::fs::write(root.join("plan.md"), doc).unwrap();
    session.close_step(r).unwrap();
    session.commit().unwrap();
}

#[test]
fn legacy_records_stay_opted_out_and_missing_or_invalid_ledger_is_refused() {
    let root = h::scratch("plan-compatibility");
    let path = root.join(crate::STATE_PATH);
    let mut p = project();
    // A synthetic, empty pre-M6-D Chain. Reading it must not activate the new profile.
    let crate::project::WorldGraph::Chains(chains) = &mut p.world else {
        unreachable!()
    };
    chains[0].approved_plan = None;
    crate::save(&p, &path).unwrap();
    let before = std::fs::read(&path).unwrap();
    let restored = crate::load(h::spec(), &path).unwrap();
    assert!(restored.approved_plan().is_none());
    crate::save(&restored, &path).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), before);

    let mut p = project();
    let d = synthesis(&mut p, &["A"]);
    approve_close(&mut p, d);
    crate::save(&p, &path).unwrap();
    let baseline: serde_norway::Value =
        serde_norway::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let mut missing = baseline.clone();
    missing["chains"]["nodes"][0]
        .as_mapping_mut()
        .unwrap()
        .remove("approved_plan");
    std::fs::write(&path, serde_norway::to_string(&missing).unwrap()).unwrap();
    assert!(crate::load(h::spec(), &path).is_err());
    let mut invalid = baseline.clone();
    invalid["chains"]["nodes"][0]["approved_plan"]["proposals"][0]["base_revision"] =
        serde_norway::Value::Number(0.into());
    std::fs::write(&path, serde_norway::to_string(&invalid).unwrap()).unwrap();
    assert!(crate::load(h::spec(), &path).is_err());
}

#[test]
fn closing_interview_requires_its_own_receipt_and_successor_cannot_reuse_it() {
    let root = h::scratch("plan-closing");
    let mut p = project();
    let d = synthesis(&mut p, &["A"]);
    approve_close(&mut p, d);
    p.open_child_cycle(CycleKind::Experiment).unwrap();
    for kind in [
        NodeKind::Define,
        NodeKind::Hypothesis,
        NodeKind::Verify,
        NodeKind::Analysis,
        NodeKind::Outcome,
    ] {
        h::walked(&mut p, kind);
    }
    let last = p.current_cycle().steps().current().unwrap();
    p.close_cycle(
        h::cycle_report(p.current_cycle(), "success", last)
            .with("next_direction.action", "open_child"),
    )
    .unwrap();
    p.open_child_cycle(CycleKind::Interview).unwrap();
    let mut closing = synthesis(&mut p, &["A"]);
    closing
        .synthesis
        .insert("chain_verdict".into(), "success".into());
    let proposal = p.propose_plan(closing.clone()).unwrap();
    assert!(
        p.close_action_step(proposal.draft.synthesis.clone().into_iter().collect())
            .is_err()
    );
    approve_close_as(&mut p, closing, "close_chain");
    p.close_chain(
        Report::new()
            .with("verdict", "success")
            .with("closing_synthesis_ref", &proposal.synthesis_ref)
            .with("handoff_summary", "Synthetic completed work"),
    )
    .unwrap();
    let first = serde_json::to_vec(p.approved_plan().unwrap()).unwrap();
    p.open_child_chain().unwrap();
    assert_eq!(
        serde_json::to_vec(p.chains()[0].approved_plan.as_ref().unwrap()).unwrap(),
        first
    );
    // An empty ledger under the contract this build gives the Chains it opens: the item queue
    // (no marker) in an `approved-plan` build, the exploration marker in an `exploration-flow` one.
    assert_eq!(p.approved_plan().unwrap(), &PlanState::for_new_chain());
    let draft = synthesis(&mut p, &["A"]);
    let next = p.propose_plan(draft).unwrap();
    assert_eq!(next.revision, 1);
    assert_ne!(next.synthesis_ref, proposal.synthesis_ref);
    assert!(
        p.decide_plan(next.revision, &proposal.digest, Decision::Approve)
            .is_err()
    );
    let path = root.join(crate::STATE_PATH);
    crate::save(&p, &path).unwrap();
    assert_eq!(crate::load(h::spec(), &path).unwrap().chains().len(), 2);
}

#[test]
fn editing_an_approved_open_proposal_preserves_old_receipt_but_requires_new_consent() {
    let mut p = project();
    let original = synthesis(&mut p, &["A"]);
    let first = p.propose_plan(original.clone()).unwrap();
    let approved = p
        .decide_plan(first.revision, &first.digest, Decision::Approve)
        .unwrap();
    let mut changed = original;
    changed.scope.goal = "Changed goal".into();
    let second = p.propose_plan(changed.clone()).unwrap();
    assert_eq!(second.revision, 2);
    assert!(second.receipt.is_none());
    assert_eq!(p.approved_plan().unwrap().proposals[0], approved);
    assert!(p.close_action_step(approved.report().unwrap()).is_err());
    assert!(
        p.decide_plan(first.revision, &first.digest, Decision::Approve)
            .is_err()
    );
    approve_close(&mut p, changed);
    assert_eq!(
        p.approved_plan()
            .unwrap()
            .active(p.current_chain().unwrap())
            .unwrap()
            .revision,
        2
    );
    p.open_child_cycle(CycleKind::Experiment).unwrap();
    assert_eq!(p.approved_plan().unwrap().executed_cycles[0].revision, 2);
    p.approved_plan()
        .unwrap()
        .validate(p.current_chain().unwrap())
        .unwrap();
}

#[test]
fn retained_item_diagnostics_bound_and_escape_values_without_weakening_equality() {
    let previous: Vec<_> = (0..12).map(|i| Item {id:i.to_string(),question:"한글\n\"".repeat(1000),done_when:"d".into(),source:String::new(),command:String::new(),part_of:String::new()}).collect();
    let mut proposed = previous.clone();
    for item in &mut proposed { item.question = "changed".into(); }
    let error = retained_items(&previous, &proposed).unwrap_err();
    assert!(error.contains("…(생략)") && error.contains("그 밖의 차이 4개"));
    assert!(error.contains("\\n\\\""), "newlines and quotes are escaped");
    assert!(error.len() < 10000);
    assert!(retained_items(&previous, &previous).is_ok());
}

/// M6-E preparation (2026-11-16): an approved item that cannot be finished stops **inside the
/// existing contract** — no new field, verdict value or transition. The record keeps the honest
/// interruption observation and the partial result; the direction stays `revisit`; no child of
/// either kind opens under the failed Cycle; the item returns to pending; the request to
/// re-interview survives; and the guidance names the preserved partial results before any restore.
/// The "continue on the partial result in place" path is deliberately absent (proposal D1).
#[test]
fn an_item_that_cannot_finish_closes_truthfully_keeps_partial_files_and_gets_no_shortcut() {
    let root = h::scratch("plan-interrupted");
    let mut p = project();
    let d = synthesis(&mut p, &["A", "B"]);
    approve_close(&mut p, d);
    p.open_child_cycle(CycleKind::Experiment).unwrap();
    h::up_to_verify(&mut p);
    let path = root.join(crate::STATE_PATH);
    crate::save(&p, &path).unwrap();
    let mut session = crate::ProjectSession::open(h::spec(), &path).unwrap();
    let failed = session.project().current_cycle().id();

    // The preserved request is the recorded signal; the next-call line now says how to stop honestly.
    session.project_mut().request_plan_interview("toolchain missing; the success condition cannot be evaluated".into()).unwrap();
    let context = crate::context(session.project());
    assert!(context.contains(crate::locale::text(crate::context::interrupt_honest_key())), "{context}");
    assert!(context.contains("evidence_files"), "the open Verify keeps its own close contract: {context}");

    // Partial result + interruption observation are the Verify evidence; the plan document stays separate.
    std::fs::write(root.join("plan.md"), session.project().plan_document().unwrap()).unwrap();
    std::fs::write(root.join("partial.txt"), "half-done partial result").unwrap();
    std::fs::write(root.join("interrupted.md"), "done: partial.txt drafted\nblocked: toolchain missing\nunverified: success condition not evaluated\n").unwrap();
    let rules = session.project().current_cycle().rules().clone();
    session.close_step(h::full_report(&rules, CycleKind::Experiment, NodeKind::Verify)
        .with("execution", "drafted partial.txt; the build could not run")
        .with("result", "partial file exists; success condition not evaluated; observation saved")
        .with("plan_document", "plan.md")
        .with("evidence_files", "interrupted.md\npartial.txt")).unwrap();
    h::opened(session.project_mut(), NodeKind::Analysis);
    session.project_mut().close_action_step(h::full_report(&rules, CycleKind::Experiment, NodeKind::Analysis)
        .with("interpretation", "the hypothesis is neither confirmed nor refuted: no test ran")).unwrap();
    let outcome = h::opened(session.project_mut(), NodeKind::Outcome);
    // The verdict value follows the current grammar; the text keeps "not evaluated" apart from a refutation.
    session.project_mut().close_action_step(h::full_report(&rules, CycleKind::Experiment, NodeKind::Outcome)
        .with("verdict", "failure")
        .with("lesson", "verification incomplete (toolchain missing), not a refutation of the hypothesis")
        .with(h::ACTION, "close_cycle")).unwrap();
    let last = h::step_id(session.project(), outcome);
    let report = h::cycle_report(session.project().current_cycle(), "failure", last)
        .with("handoff_summary", "A interrupted: partial.txt is half done and kept in the Verify Snapshot; success condition not evaluated");
    assert_eq!(report.get(h::ACTION), Some("revisit"), "failure still records the existing direction only");
    session.close_cycle(report).unwrap();
    session.commit().unwrap();

    // Nothing was restored or deleted: the partial result is still in the folder.
    assert_eq!(std::fs::read_to_string(root.join("partial.txt")).unwrap(), "half-done partial result");
    assert!(root.join("interrupted.md").is_file());
    let cycle = session.project().cycles().node(failed).unwrap();
    assert!(cycle.is_closed() && cycle.report().unwrap().get("verdict") == Some("failure"));
    let verify = cycle.steps().nodes().iter().find(|n| n.kind == NodeKind::Verify).unwrap();
    assert_eq!(verify.report.as_ref().unwrap().get("evidence_files"), Some("interrupted.md\npartial.txt"));

    // No child of either kind under a failed Experiment; the recorded direction is revisit.
    assert!(session.project_mut().open_child_cycle(CycleKind::Interview).is_err(), "D1 is not decided: no Interview under a failure");
    assert!(session.project_mut().open_child_cycle(CycleKind::Experiment).is_err());
    assert!(session.project().cycles().can_revisit());

    // The item returns to pending; the request survives; no Experiment binds before an Interview.
    let chain = session.project().current_chain().unwrap();
    let state = session.project().approved_plan().unwrap();
    assert_eq!(state.pending(chain).iter().map(|i| i.id.as_str()).collect::<Vec<_>>(), vec!["A", "B"]);
    assert_eq!(state.awaiting_interview(chain).len(), 1);
    assert!(session.project().next_plan_execution(None).is_err());

    // The next-call line shows the partial results before the restore and promises no keep-files shortcut.
    let context = crate::context(session.project());
    assert!(context.contains(crate::locale::text("guide.interrupt_partial_results")), "{context}");
    assert!(context.contains("partial.txt"), "the closed Cycle's evidence is readable in context: {context}");
    assert!(!context.contains("gil_open(kind: experiment)"), "{context}");

    // The record survives a reload byte for byte.
    let bytes = std::fs::read(&path).unwrap();
    let loaded = crate::load(h::spec(), &path).unwrap();
    crate::save(&loaded, &path).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), bytes);

    // Development profile only: the approved revisit removes the partial file from the folder (it stays in
    // the Snapshot), and the next plan may not drop the interrupted item — the refusal names the gap.
    if !cfg!(feature = "product-safety") {
        session.revisit_cycle().unwrap();
        assert!(!root.join("partial.txt").exists(), "revisit restores the ancestor world");
        session.open_branch_cycle(CycleKind::Interview).unwrap();
        let dropped = synthesis(session.project_mut(), &["B"]);
        let error = session.project_mut().propose_plan(dropped).unwrap_err();
        assert!(error.contains("draft.items[id=\"A\"]") && error.contains(crate::locale::text("plan.err.no_cancel")), "{error}");
    }
    std::fs::remove_dir_all(root).ok();
}

/// Chain execution flow (2026-11-17): a re-interview request, the Closing approval and the
/// Chain Report stay apart from the actual close, a closed Chain binds nothing more, and the
/// next Chain starts with an empty ledger — no request, binding or approval carries over.
/// Every position survives a reload with the same next-call line. Synthetic consent only.
#[test]
fn closed_chain_binds_nothing_and_the_next_chain_starts_with_an_empty_ledger() {
    let root = h::scratch("plan-chain-flow");
    let path = root.join(crate::STATE_PATH);
    let first_line = |p: &Project| crate::context(p).split_once("[지금 바로 할 호출]\n").unwrap().1.lines().next().unwrap().to_string();
    let reloaded = |p: &Project| { crate::save(p, &path).unwrap(); crate::load(h::spec(), &path).unwrap() };
    let mut p = project();
    let d = synthesis(&mut p, &["A", "B"]);
    let proposal = p.propose_plan(d.clone()).unwrap();
    assert_eq!(first_line(&reloaded(&p)), first_line(&p), "waiting position survives reload");
    assert!(p.open_child_cycle(CycleKind::Experiment).is_err(), "no execution before approval");
    let _ = proposal;
    approve_close(&mut p, d);
    p.open_child_cycle(CycleKind::Experiment).unwrap();
    for kind in [NodeKind::Define, NodeKind::Hypothesis] { h::walked(&mut p, kind); }
    p.request_plan_interview("B must change".into()).unwrap();
    h::opened(&mut p, NodeKind::Verify);
    p.close_verify_step(h::full_report(p.current_cycle().rules(), CycleKind::Experiment, NodeKind::Verify), h::first_world()).unwrap();
    for kind in [NodeKind::Analysis, NodeKind::Outcome] { h::walked(&mut p, kind); }
    let last = p.current_cycle().steps().current().unwrap();
    p.close_cycle(h::cycle_report(p.current_cycle(), "success", last).with(h::ACTION, "open_child")).unwrap();
    assert!(p.open_child_cycle(CycleKind::Experiment).is_err(), "preserved request blocks execution");
    assert_eq!(first_line(&reloaded(&p)), first_line(&p));

    // Closing Interview: the approval of the closing proposal is not the close of the Chain.
    p.open_child_cycle(CycleKind::Interview).unwrap();
    let mut closing = synthesis(&mut p, &["A", "B"]);
    closing.synthesis.insert("chain_verdict".into(), "failure".into());
    let closing_proposal = p.propose_plan(closing.clone()).unwrap();
    assert_eq!(closing_proposal.revision, 2);
    approve_close_as(&mut p, closing, "close_chain");
    let chain = p.current_chain().unwrap();
    assert!(chain.awaiting_report() && !chain.is_closed());
    assert!(p.approved_plan().unwrap().awaiting_interview(chain).is_empty(), "the new approval answers the request");
    assert_eq!(p.approved_plan().unwrap().pending(chain).iter().map(|i| i.id.as_str()).collect::<Vec<_>>(), vec!["B"]);
    assert!(p.open_child_cycle(CycleKind::Experiment).is_err(), "Chain Report is due; B stays pending");
    assert!(p.open_child_cycle(CycleKind::Interview).is_err());
    let line = first_line(&p);
    assert!(line.contains("Chain Report") && !line.contains("gil_open"), "{line}");
    assert_eq!(first_line(&reloaded(&p)), line);
    assert!(p.close_chain(Report::new().with("verdict", "success").with("closing_synthesis_ref", &closing_proposal.synthesis_ref).with("handoff_summary", "x")).is_err());
    p.close_chain(Report::new().with("verdict", "failure").with("closing_synthesis_ref", &closing_proposal.synthesis_ref).with("handoff_summary", "B remains; A done")).unwrap();

    // Closed Chain: nothing opens, nothing binds, the ledger is byte-identical.
    let ledger = serde_json::to_vec(p.approved_plan().unwrap()).unwrap();
    assert!(p.open_child_cycle(CycleKind::Experiment).is_err());
    assert!(p.open_child_cycle(CycleKind::Interview).is_err());
    assert!(p.request_plan_interview("late".into()).is_err(), "no request on a closed Chain");
    assert!(p.propose_plan(synthesis_draft_only(&["Z"])).is_err());
    assert_eq!(serde_json::to_vec(p.approved_plan().unwrap()).unwrap(), ledger);
    let line = first_line(&p);
    assert!(line.contains("gil_open(kind: chain)") && !line.contains("experiment"), "{line}");
    assert_eq!(first_line(&reloaded(&p)), line);

    // Next Chain: empty ledger, no request, no pending; a fresh approval binds its own item.
    p.open_child_chain().unwrap();
    let chain = p.current_chain().unwrap();
    let state = p.approved_plan().unwrap();
    assert!(state.awaiting_interview(chain).is_empty() && state.pending(chain).is_empty() && state.reinterview_requests.is_empty());
    assert!(p.open_child_cycle(CycleKind::Experiment).is_err(), "no approval in the new Chain");
    let line = first_line(&p);
    assert!(line.contains("gil_open(kind: question)"), "{line}");
    assert_eq!(first_line(&reloaded(&p)), line);
    let x = synthesis(&mut p, &["X"]);
    let next = p.propose_plan(x.clone()).unwrap();
    assert_eq!((next.revision, next.base_revision), (1, None));
    approve_close(&mut p, x);
    if p.exploration_contract() {
        // The successor this build opens links its Experiment explicitly; nothing is auto-bound.
        assert!(p.open_child_cycle(CycleKind::Experiment).is_err());
        let active = p.approved_plan().unwrap().active(p.current_chain().unwrap()).unwrap();
        let link = ExperimentLink { revision: active.revision, digest: active.digest.clone(), requirement_id: "X".into(),
            selection: Selection { relevance: "X".into(), readiness: "ready".into(), uncertainty: "one".into() } };
        p.open_child_cycle_linked(CycleKind::Experiment, Some(&link)).unwrap();
    } else {
        p.open_child_cycle(CycleKind::Experiment).unwrap();
    }
    assert_eq!(p.current_plan_item().unwrap().id, "X");
    assert_eq!(p.approved_plan().unwrap().executed_cycles.len(), 1);
    assert_eq!(serde_json::to_vec(p.chains()[0].approved_plan.as_ref().unwrap()).unwrap(), ledger, "the first Chain's ledger, request included, is untouched");
    assert_eq!(p.chains()[0].approved_plan.as_ref().unwrap().reinterview_requests.len(), 1);
    let bytes = { crate::save(&p, &path).unwrap(); std::fs::read(&path).unwrap() };
    crate::save(&crate::load(h::spec(), &path).unwrap(), &path).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    std::fs::remove_dir_all(root).ok();
}

fn synthesis_draft_only(ids: &[&str]) -> Draft {
    let mut report: BTreeMap<String, String> = BTreeMap::new();
    report.insert("statement".into(), "s".into());
    report.insert("basis_refs".into(), "step:C1/S1".into());
    report.insert("exploration_checklist".into(), ids.join("\n"));
    Draft {
        scope: Scope { goal: "g".into(), in_scope: "i".into(), out_of_scope: "o".into(), constraints: "c".into(), success_criteria: "s".into() },
        items: ids.iter().map(|id| Item { id: (*id).into(), question: "q".into(), done_when: "d".into(), source: String::new(), command: String::new(), part_of: String::new() }).collect(),
        synthesis: report,
    }
}

/// A build that cannot honour requirement links must refuse a record that carries them — the
/// way the approval ledger itself is refused by a build without `approved-plan` — and must not
/// read the links as queue bindings. The marker here is written by hand because this build never
/// writes one; `tests/exploration_flow.rs` produces real ones in an `exploration-flow` build.
#[cfg(not(feature = "exploration-flow"))]
#[test]
fn a_record_under_the_exploration_contract_is_refused_and_left_as_it_is() {
    let root = h::scratch("plan-exploration-record");
    let path = root.join(crate::STATE_PATH);
    let p = project();
    assert_eq!(p.approved_plan().unwrap().contract, None, "this build opens item-queue Chains only");
    crate::save(&p, &path).unwrap();
    assert!(!std::fs::read_to_string(&path).unwrap().contains("contract"), "and writes no marker");
    let mut file: serde_norway::Value = serde_norway::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    file["chains"]["nodes"][0]["approved_plan"].as_mapping_mut().unwrap().insert("contract".into(), "exploration".into());
    let marked = serde_norway::to_string(&file).unwrap();
    std::fs::write(&path, &marked).unwrap();
    let refused = crate::load(h::spec(), &path).unwrap_err().to_string();
    assert!(refused.contains(crate::locale::text("plan.err.exploration_core_required")), "{refused}");
    assert!(crate::ProjectSession::open(h::spec(), &path).is_err(), "no session, so nothing can be opened, closed or rewritten");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), marked, "the refused record is untouched");
    std::fs::remove_dir_all(root).ok();
}
