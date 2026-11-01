//! Serial Chain handoff: preserve worlds, reports, identity and history across process boundaries.
#![cfg(feature = "chain-core")]
mod common;
use common::{chain_report, closing_ready, scratch, spec};
use gil::{CycleKind, NodeKind, Project, ProjectSession, Report, load, save};
use serde_norway::Value;
use std::path::Path;
use std::process::Command;

fn yaml(path: &Path) -> Value {
    serde_norway::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn closed_first(verdict: &str) -> Project {
    let mut p = closing_ready(verdict);
    p.close_chain(chain_report(verdict)).unwrap();
    p
}

// Synthetic consent for this test only; walk the same public action/close boundaries.
fn close_opening_as_final(p: &mut Project, verdict: &str) {
    for kind in [
        NodeKind::Question,
        NodeKind::Interpretation,
        NodeKind::Synthesis,
    ] {
        common::opened(p, kind);
        let cycle = p.current_cycle();
        let mut report = common::full_report(cycle.rules(), CycleKind::Interview, kind);
        if kind == NodeKind::Synthesis {
            report.insert("approved", "yes");
            report.insert("chain_verdict", verdict);
            report.insert(
                "basis_refs",
                cycle
                    .steps()
                    .nodes()
                    .iter()
                    .filter(|n| matches!(n.kind, NodeKind::Question | NodeKind::Interpretation))
                    .map(|n| cycle.step_ref(n.id).to_string())
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
        p.close_action_step(report).unwrap();
    }
    let synthesis = p
        .current_cycle()
        .step_ref(p.current_cycle().steps().current().unwrap());
    let outcome = common::opened(p, NodeKind::Outcome);
    p.close_action_step(
        common::full_report(
            p.current_cycle().rules(),
            CycleKind::Interview,
            NodeKind::Outcome,
        )
        .with("synthesis_ref", synthesis.to_string()),
    )
    .unwrap();
    p.close_cycle(
        common::cycle_report(p.current_cycle(), "success", common::step_id(p, outcome))
            .with("next_direction.action", "close_chain"),
    )
    .unwrap();
    p.close_chain(
        Report::new()
            .with("verdict", verdict)
            .with("closing_synthesis_ref", synthesis.to_string())
            .with(
                "handoff_summary",
                "Second Chain: new conclusion; one remaining question.",
            ),
    )
    .unwrap();
}

#[test]
fn serial_chains_keep_global_cycle_ids_worlds_and_self_and_compress_prior_context() {
    for verdict in ["success", "failure"] {
        let root = scratch(&format!("chain-handoff-{verdict}"));
        let path = root.join(gil::STATE_PATH);
        let mut p = closed_first(verdict);
        save(&p, &path).unwrap();
        let first = yaml(&path)["chains"]["nodes"][0].clone();
        let journey = p.current_existence().current_journey();
        let world = p.world_snapshot();
        let will_id = p.next_will_id();
        let snapshots = p.next_snapshot_id();
        assert_eq!(p.open_child_chain().unwrap().to_string(), "chain:C2");
        assert_eq!(p.current_cycle().id().to_ref().to_string(), "cycle:C4");
        assert_eq!(p.current_cycle().kind(), CycleKind::Interview);
        assert!(p.current_cycle().parent().is_none());
        assert!(p.active_will().is_none());
        assert_eq!(p.current_existence().current_journey(), journey);
        assert_eq!(p.next_will_id(), will_id);
        assert_eq!(p.next_snapshot_id(), snapshots);
        assert_eq!(p.world_snapshot(), world);
        assert!(p.open_child_chain().is_err());
        assert!(p.open_child_cycle(CycleKind::Experiment).is_err());
        save(&p, &path).unwrap();
        assert_eq!(yaml(&path)["chains"]["nodes"][0], first);
        let bytes = std::fs::read(&path).unwrap();
        let mut p = load(spec(), &path).unwrap();
        save(&p, &path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let context = gil::context(&p);
        assert!(context.contains(&format!("이전 Chain: chain:C1 · {verdict}")));
        assert!(context.contains(chain_report(verdict).get("handoff_summary").unwrap()));
        assert!(
            !context.contains("step:C1/S1"),
            "old Step details are not expanded"
        );
        close_opening_as_final(&mut p, "success");
        let latest_journey = p.current_existence().current_journey();
        assert!(latest_journey.revision() > journey.revision());
        assert_eq!(p.open_child_chain().unwrap().to_string(), "chain:C3");
        assert_eq!(p.current_cycle().id().to_ref().to_string(), "cycle:C5");
        save(&p, &path).unwrap();
        let mut p = load(spec(), &path).unwrap();
        assert_eq!(p.chains().len(), 3);
        assert_eq!(yaml(&path)["chains"]["nodes"][0], first);
        assert_eq!(p.chains()[0].journey(), Some(journey));
        assert_eq!(p.current_existence().current_journey(), latest_journey);
        // New work after two handoffs must retain old Will/provenance references.
        common::opened(&mut p, NodeKind::Question);
        save(&p, &path).unwrap();
        let loaded = load(spec(), &path).unwrap();
        assert_eq!(
            loaded.active_will().unwrap().target().to_string(),
            "step:C5/S1"
        );
        assert!(gil::context(&loaded).contains("Second Chain: new conclusion"));
    }
}

#[test]
fn load_refuses_forged_lineage_allocators_and_historical_records_without_repair() {
    let root = scratch("chain-handoff-forged");
    let path = root.join(gil::STATE_PATH);
    let mut p = closed_first("failure");
    p.open_child_chain().unwrap();
    save(&p, &path).unwrap();
    let good = yaml(&path);
    for case in 0..13 {
        let mut v = good.clone();
        match case {
            0 => v["chains"]["current"] = "chain:C1".into(),
            1 => v["chains"]["next_id"] = 2.into(),
            2 => v["chains"]["nodes"][1]["parent_chain_ref"] = Value::Null,
            3 => v["chains"]["nodes"][1]["parent_chain_ref"] = "chain:C2".into(),
            4 => v["chains"]["nodes"][1]["id"] = "C1".into(),
            5 => v["chains"]["nodes"][0]["state"] = "open".into(),
            6 => v["chains"]["nodes"][1]["entry_snapshot_ref"] = "snapshot:A99".into(),
            7 => v["chains"]["nodes"][0]["cycles"]["next_id"] = 5.into(),
            8 => v["chains"]["nodes"][1]["cycles"]["nodes"][0]["id"] = 1.into(),
            9 => v["chains"]["nodes"][0]["report"]["verdict"] = "success".into(),
            10 => v["chains"]["nodes"][0]["journey_ref"] = "journey:X1@J999".into(),
            11 => {
                v["chains"]["nodes"][0]["cycles"]["nodes"][1]["entry_snapshot_ref"] =
                    "snapshot:A99".into()
            }
            _ => {
                v["chains"]["nodes"][0]["cycles"]["nodes"][1]["steps"]["nodes"][0]["journey_ref"] =
                    "journey:X1@J999".into()
            }
        }
        let bytes = serde_norway::to_string(&v).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        assert!(load(spec(), &path).is_err(), "accepted forgery {case}");
        assert_eq!(std::fs::read(&path).unwrap(), bytes.as_bytes());
    }
}

#[test]
fn cli_handoff_requires_clean_world_and_lost_response_retry_cannot_duplicate_chain() {
    let root = scratch("chain-handoff-cli");
    let path = root.join(gil::STATE_PATH);
    save(&closed_first("failure"), &path).unwrap();
    let before = std::fs::read(&path).unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_gil"))
            .args(args)
            .current_dir(&root)
            .output()
            .unwrap()
    };
    std::fs::write(root.join("unconfirmed.txt"), "preserve").unwrap();
    let refused = run(&["open", "chain"]);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("다음 Chain을 열 수 없다"));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(
        std::fs::read(root.join("unconfirmed.txt")).unwrap(),
        b"preserve"
    );
    std::fs::remove_file(root.join("unconfirmed.txt")).unwrap();
    let opened = run(&["open", "chain"]);
    assert!(
        opened.status.success(),
        "{}",
        String::from_utf8_lossy(&opened.stderr)
    );
    let bytes = std::fs::read(&path).unwrap();
    assert!(
        String::from_utf8_lossy(&run(&["context"]).stdout)
            .contains("이전 Chain: chain:C1 · failure")
    );
    assert!(String::from_utf8_lossy(&run(&["status"]).stdout).contains("chain:C2"));
    let story = run(&["story"]);
    let story = String::from_utf8_lossy(&story.stdout);
    assert!(story.contains("chain:C1 · closed") && story.contains("chain:C2 · open"));
    assert!(!run(&["open", "chain"]).status.success());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn monitor_and_visual_history_remain_readable_in_the_next_chain() {
    let root = scratch("chain-handoff-monitor");
    common::private_storage(&root);
    let path = root.join(gil::STATE_PATH);
    let mut p = closed_first("success").prepare_visual_upgrade().unwrap();
    p.open_child_chain().unwrap();
    save(&p, &path).unwrap();
    let s = ProjectSession::open_read_only(spec(), &path).unwrap();
    let view = gil::monitor_view_v1(&s.monitor().unwrap()).unwrap();
    assert_eq!(
        view.chain.as_ref().unwrap().parent_chain_ref.as_deref(),
        Some("chain:C1")
    );
    assert_eq!(view.chains.len(), 2);
    assert_eq!(view.timeline.len(), 4);
    assert_eq!(
        view.chains[0].report.as_ref().unwrap()["verdict"],
        "success"
    );
    assert!(s.node_detail_v1("step:C2/S3".parse().unwrap()).is_ok());
    use gil::visual_report::registry::ReportOwner;
    assert!(
        s.project()
            .report_visuals_v1(ReportOwner::Step("step:C2/S3".parse().unwrap()))
            .is_ok()
    );
}

#[test]
fn attached_evidence_stays_readable_after_handoff_and_source_removal() {
    use gil::visual_report::image::VisualAttachment;
    use gil::visual_report::registry::{ReportOwner, ReportVisuals, SourceKind};
    use gil::visual_report::{VisualDeclaration, VisualUse};
    let root = scratch("chain-handoff-image");
    let path = root.join(gil::STATE_PATH);
    let mut p = common::bootstrap();
    common::up_to_verify(&mut p);
    save(&p.prepare_visual_upgrade().unwrap(), &path).unwrap();
    common::private_storage(&root);
    let mut png = Vec::new();
    png::Encoder::new(&mut png, 1, 1)
        .write_header()
        .unwrap()
        .write_image_data(&[127])
        .unwrap();
    std::fs::write(root.join("screen.png"), &png).unwrap();
    let mut s = ProjectSession::open(spec(), &path).unwrap();
    let report = common::full_report(
        s.project().current_cycle().rules(),
        CycleKind::Experiment,
        NodeKind::Verify,
    )
    .with("evidence_files", "screen.png");
    s.close_verify_with_images_and_commit(
        report,
        vec![VisualAttachment {
            path: "screen.png".into(),
            source_kind: SourceKind::Generated,
            source_description: "Synthetic pixel".into(),
        }],
        |ids| VisualDeclaration::Attached {
            uses: ids
                .iter()
                .map(|id| VisualUse {
                    asset: *id,
                    caption: "Evidence".into(),
                    alt: "Gray pixel".into(),
                    purpose: "Synthetic observation".into(),
                })
                .collect(),
        },
    )
    .unwrap();
    let owner = ReportOwner::Step("step:C2/S3".parse().unwrap());
    let ReportVisuals::Verify(VisualDeclaration::Attached { uses }) = s
        .project()
        .visual_state()
        .unwrap()
        .registry()
        .report(owner)
        .unwrap()
    else {
        panic!()
    };
    let image = uses[0].asset;
    let before = s.project().report_visuals_v1(owner).unwrap();
    let p = s.project_mut();
    // The Verify attached an asset, so Analysis and the Cycle Report must cite it (visual evidence policy).
    let cite = || VisualUse {
        asset: image,
        caption: "Cited evidence".into(),
        alt: "Gray pixel".into(),
        purpose: "Interpretation basis".into(),
    };
    common::opened(p, NodeKind::Analysis);
    p.close_step_with_visuals(
        common::full_report(p.current_cycle().rules(), CycleKind::Experiment, NodeKind::Analysis),
        None,
        ReportVisuals::References(vec![cite()]),
    )
    .unwrap();
    let outcome = common::opened(p, NodeKind::Outcome);
    p.close_action_step(
        common::full_report(
            p.current_cycle().rules(),
            CycleKind::Experiment,
            NodeKind::Outcome,
        )
        .with("verdict", "success")
        .with("next_direction.action", "close_cycle"),
    )
    .unwrap();
    p.close_cycle_with_visuals(
        common::cycle_report(p.current_cycle(), "success", common::step_id(p, outcome)),
        vec![cite()],
    )
    .unwrap();
    p.open_child_cycle(CycleKind::Interview).unwrap();
    close_opening_as_final(p, "success");
    s.commit().unwrap();
    s.open_child_chain().unwrap();
    s.commit().unwrap();
    drop(s);
    std::fs::remove_file(root.join("screen.png")).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let s = ProjectSession::open_read_only(spec(), &path).unwrap();
    assert_eq!(s.project().report_visuals_v1(owner).unwrap(), before);
    assert_eq!(s.read_report_image(owner, image).unwrap().bytes(), png);
    assert!(s.node_detail_v1("step:C2/S3".parse().unwrap()).is_ok());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn new_chain_requires_its_own_interview_before_experiment_and_new_learning() {
    let root = scratch("chain-handoff-new-work");
    let path = root.join(gil::STATE_PATH);
    let mut p = closed_first("failure");
    let old_journey = p.current_existence().current_journey();
    p.open_child_chain().unwrap();
    assert!(p.open_child_cycle(CycleKind::Experiment).is_err());
    // Synthetic fresh approval is collected by this helper, not reused from Chain C1.
    let mut p = common::bootstrap_from(p);
    assert_eq!(p.current_cycle().id().to_ref().to_string(), "cycle:C5");
    assert_eq!(
        p.current_cycle().parent().unwrap().to_ref().to_string(),
        "cycle:C4"
    );
    assert_eq!(p.chains()[0].cycles().nodes().len(), 3);
    let outcome = common::walk_to_the_exit(&mut p, "success");
    p.close_cycle(common::cycle_report(p.current_cycle(), "success", outcome))
        .unwrap();
    assert!(!p.current_chain().unwrap().is_closed());
    assert!(p.current_existence().current_journey().revision() > old_journey.revision());
    assert_eq!(p.chains()[0].journey(), Some(old_journey));
    save(&p, &path).unwrap();
    let restored = load(spec(), &path).unwrap();
    assert_eq!(restored.all_cycles().count(), 5);
    assert!(gil::context(&restored).contains("이전 Chain: chain:C1 · failure"));
    assert!(gil::context(&restored).contains("cycle:C5"));
}
