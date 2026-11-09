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
            })
            .collect(),
        synthesis: report
            .field_names()
            .map(|key| (key.into(), report.get(key).unwrap().into()))
            .collect(),
    }
}
fn approve_close(p: &mut Project, draft: Draft) {
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
fn project() -> Project {
    Project::start(h::spec(), h::first_world())
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
    assert_eq!(p.approved_plan().unwrap(), &PlanState::default());
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
    let previous: Vec<_> = (0..12).map(|i| Item {id:i.to_string(),question:"한글\n\"".repeat(1000),done_when:"d".into()}).collect();
    let mut proposed = previous.clone();
    for item in &mut proposed { item.question = "changed".into(); }
    let error = retained_items(&previous, &proposed).unwrap_err();
    assert!(error.contains("…(생략)") && error.contains("그 밖의 차이 4개"));
    assert!(error.contains("\\n\\\""), "newlines and quotes are escaped");
    assert!(error.len() < 10000);
    assert!(retained_items(&previous, &previous).is_ok());
}
