//! Closing Interview acceptance: no automatic verdict, exact approval provenance, durable close.
#![cfg(feature = "chain-core")]
mod common;
use common::{chain_report, closing_ready, scratch, spec};
use gil::{CycleKind, NodeKind, ProjectSession, load, save};
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn approved_interview_success_can_close_chain_with_either_verdict_without_growing_self() {
    for verdict in ["success", "failure"] {
        let root = scratch(&format!("chain-closing-{verdict}"));
        let path = root.join(gil::STATE_PATH);
        let mut p = closing_ready(verdict);
        assert_eq!(
            p.current_cycle().report().unwrap().get("verdict"),
            Some("success")
        );
        assert!(p.current_chain().unwrap().awaiting_report());
        let journey = p.current_existence().current_journey();
        let world = p.world_snapshot();
        save(&p, &path).unwrap();
        let s = ProjectSession::open_read_only(spec(), &path).unwrap();
        let view = gil::monitor_view_v1(&s.monitor().unwrap()).unwrap();
        assert!(
            view.next_actions
                .iter()
                .any(|a| a.kind.as_wire() == "close_chain")
        );
        drop(s);
        p.close_chain(chain_report(verdict)).unwrap();
        assert!(p.current_chain().unwrap().is_closed());
        assert_eq!(p.current_chain().unwrap().exit_snapshot(), Some(world));
        assert_eq!(p.current_chain().unwrap().journey(), Some(journey));
        assert_eq!(p.current_existence().current_journey(), journey);
        assert!(p.active_will().is_none());
        save(&p, &path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let mut p = load(spec(), &path).unwrap();
        assert_eq!(
            p.current_chain().unwrap().report(),
            Some(&chain_report(verdict))
        );
        assert!(p.close_chain(chain_report(verdict)).is_err());
        assert!(p.open_child_chain().is_err());
        assert!(p.open_child_cycle(CycleKind::Experiment).is_err());
        assert!(
            p.open_action_step(NodeKind::Question, common::plan(NodeKind::Question))
                .is_err()
        );
        save(&p, &path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let s = ProjectSession::open_read_only(spec(), &path).unwrap();
        let view = gil::monitor_view_v1(&s.monitor().unwrap()).unwrap();
        assert!(view.next_actions.is_empty());
        let chain = view.chain.unwrap();
        assert_eq!(chain.state.as_wire(), "closed");
        assert_eq!(chain.exit_snapshot_ref.as_deref(), Some("snapshot:A1"));
        assert_eq!(chain.report.unwrap()["verdict"], verdict);
        assert!(gil::context(s.project()).contains(&format!("Chain 판정: {verdict}")));
    }
}

#[test]
fn wrong_approval_verdict_empty_summary_and_extra_direction_leave_records_unchanged() {
    let root = scratch("chain-refusal");
    let path = root.join(gil::STATE_PATH);
    let mut p = closing_ready("failure");
    save(&p, &path).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let mut bad = vec![
        chain_report("success"),
        chain_report("failure").with("handoff_summary", "  "),
        chain_report("failure").with("next_direction.action", "open_child"),
    ];
    for target in [
        "step:C1/S3",
        "step:C2/S3",
        "step:C3/S1",
        "step:C3/S4",
        "step:C9/S3",
        "step:C03/S3",
        "",
    ] {
        bad.push(chain_report("failure").with("closing_synthesis_ref", target));
    }
    for report in bad {
        assert!(p.close_chain(report).is_err());
        save(&p, &path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
    for verdict in ["success", "failure"] {
        let mut experiment = common::graph_with_one_closed_cycle(verdict);
        assert!(experiment.close_chain(chain_report(verdict)).is_err());
        assert!(!experiment.current_chain().unwrap().is_closed());
    }
    let mut rejected = common::closing_interview(Some("failure"), "no");
    assert!(
        rejected
            .open_action_step(NodeKind::Outcome, common::plan(NodeKind::Outcome))
            .is_err()
    );
    assert!(rejected.close_chain(chain_report("failure")).is_err());
    let mut ordinary = common::closing_interview(None, "yes");
    let synthesis = ordinary
        .current_cycle()
        .step_ref(ordinary.current_cycle().steps().current().unwrap());
    let outcome = common::opened(&mut ordinary, NodeKind::Outcome);
    let report = common::full_report(
        ordinary.current_cycle().rules(),
        CycleKind::Interview,
        NodeKind::Outcome,
    )
    .with("synthesis_ref", synthesis.to_string());
    ordinary.close_action_step(report).unwrap();
    let report = common::cycle_report(
        ordinary.current_cycle(),
        "success",
        common::step_id(&ordinary, outcome),
    )
    .with("next_direction.action", "close_chain");
    assert!(ordinary.close_cycle(report).is_err());
    assert!(!ordinary.current_cycle().is_closed());
}

#[test]
fn forged_closed_records_are_rejected_on_load_without_repair() {
    let root = scratch("chain-forged-close");
    let path = root.join(gil::STATE_PATH);
    let mut p = closing_ready("success");
    p.close_chain(chain_report("success")).unwrap();
    save(&p, &path).unwrap();
    let good: serde_norway::Value =
        serde_norway::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    for (pointer, value) in [
        ("report.verdict", "failure"),
        ("report.closing_synthesis_ref", "step:C1/S3"),
        ("report.handoff_summary", ""),
        ("journey_ref", "journey:X1@J0"),
        ("state", "open"),
        ("exit_snapshot_ref", "snapshot:A99"),
        ("synthesis.approved", "no"),
        ("synthesis.chain_verdict", "failure"),
        ("cycle.next_direction.action", "open_child"),
    ] {
        let mut bad = good.clone();
        let chain = &mut bad["chains"]["nodes"][0];
        if let Some(key) = pointer.strip_prefix("report.") {
            chain["report"][key] = value.into();
        } else if let Some(key) = pointer.strip_prefix("synthesis.") {
            chain["cycles"]["nodes"][2]["steps"]["nodes"][2]["report"][key] = value.into();
        } else if let Some(key) = pointer.strip_prefix("cycle.") {
            chain["cycles"]["nodes"][2]["report"][key] = value.into();
        } else {
            chain[pointer] = value.into();
        }
        let bytes = serde_norway::to_string(&bad).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        assert!(load(spec(), &path).is_err(), "accepted {pointer}");
        assert_eq!(std::fs::read(&path).unwrap(), bytes.as_bytes());
    }
}

fn cli(root: &std::path::Path, args: &[&str], report: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_gil"))
        .args(args)
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(report.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn cli_close_dirty_gate_and_lost_response_restart_preserve_closed_records() {
    let root = scratch("chain-close-cli");
    let path = root.join(gil::STATE_PATH);
    save(&closing_ready("failure"), &path).unwrap();
    let before = std::fs::read(&path).unwrap();
    let report = "verdict: failure\nclosing_synthesis_ref: step:C3/S3\nhandoff_summary: Tested paths; unresolved question remains.\n";
    std::fs::write(root.join("unverified.txt"), "unconfirmed work").unwrap();
    assert!(!cli(&root, &["close"], report).status.success());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    std::fs::remove_file(root.join("unverified.txt")).unwrap();
    let help = cli(&root, &["close", "--help"], "");
    assert!(String::from_utf8_lossy(&help.stdout).contains("closing_synthesis_ref: step:C3/S3"));
    let closed = cli(&root, &["close"], report);
    assert!(
        closed.status.success(),
        "{}",
        String::from_utf8_lossy(&closed.stderr)
    );
    // Discard the receipt, reopen a new process and query before retrying.
    let bytes = std::fs::read(&path).unwrap();
    let context = cli(&root, &["context"], "");
    assert!(String::from_utf8_lossy(&context.stdout).contains("Chain 판정: failure"));
    assert!(!cli(&root, &["close"], report).status.success());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn malformed_closing_proposal_is_rejected_before_synthesis_becomes_immutable() {
    let root = scratch("chain-proposal-invalid");
    let path = root.join(gil::STATE_PATH);
    let mut p = common::graph_with_one_closed_cycle("success");
    p.open_child_cycle(CycleKind::Interview).unwrap();
    common::walked(&mut p, NodeKind::Question);
    common::walked(&mut p, NodeKind::Interpretation);
    common::opened(&mut p, NodeKind::Synthesis);
    save(&p, &path).unwrap();
    let before = std::fs::read(&path).unwrap();
    let report = common::full_report(
        p.current_cycle().rules(),
        CycleKind::Interview,
        NodeKind::Synthesis,
    )
    .with("approved", "yes")
    .with("basis_refs", "step:C3/S1\nstep:C3/S2")
    .with("chain_verdict", "partial");
    assert!(p.close_action_step(report).is_err());
    save(&p, &path).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(p.active_will().is_some());
    let hint = cli(&root, &["close", "--help"], "");
    assert!(String::from_utf8_lossy(&hint.stdout).contains("chain_verdict: success 또는 failure"));
}

#[test]
fn legacy_graph_does_not_gain_closing_grammar_or_synthetic_chain_approval() {
    let root = scratch("chain-closing-legacy");
    let path = root.join(gil::STATE_PATH);
    save(&common::bootstrap(), &path).unwrap();
    let mut legacy: serde_norway::Value =
        serde_norway::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    legacy["cycles"] = legacy["chains"]["nodes"][0]["cycles"].clone();
    legacy
        .as_mapping_mut()
        .unwrap()
        .remove(serde_norway::Value::from("chains"));
    legacy["format"] = 4.into();
    std::fs::write(&path, serde_norway::to_string(&legacy).unwrap()).unwrap();
    let mut p = load(spec(), &path).unwrap();
    save(&p, &path).unwrap();
    let before = std::fs::read(&path).unwrap();
    assert!(p.close_chain(chain_report("success")).is_err());
    let actions = &p
        .current_cycle()
        .rules()
        .cycle_rules(CycleKind::Interview)
        .unwrap()
        .field_constraints["next_direction.action"]
        .allowed_values;
    assert!(!actions.iter().any(|action| action == "close_chain"));
    save(&p, &path).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn approved_closing_proposal_cannot_redirect_to_another_experiment() {
    let mut p = common::closing_interview(Some("success"), "yes");
    assert!(
        p.close_chain(chain_report("success")).is_err(),
        "Synthesis alone cannot close Chain"
    );
    let outcome = common::opened(&mut p, NodeKind::Outcome);
    p.close_action_step(
        common::full_report(
            p.current_cycle().rules(),
            CycleKind::Interview,
            NodeKind::Outcome,
        )
        .with("synthesis_ref", "step:C3/S3"),
    )
    .unwrap();
    let report = common::cycle_report(p.current_cycle(), "success", common::step_id(&p, outcome));
    assert!(
        p.close_cycle(report.clone().with("next_direction.action", "open_child"))
            .is_err()
    );
    assert!(!p.current_cycle().is_closed());
    p.close_cycle(report.with("next_direction.action", "close_chain"))
        .unwrap();
    assert!(p.current_chain().unwrap().awaiting_report());
}
