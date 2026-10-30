//! M6-A acceptance: actual ownership, persistence, legacy preservation, public surfaces.
use gil::load;
mod common;
use common::{scratch, spec};

#[cfg(not(feature = "chain-core"))]
#[test]
fn ordinary_writer_refuses_chain_format_without_changing_it() {
    let dir = scratch("chain-old-writer");
    let path = dir.join(".gil/state.yaml");
    for format in [6, 7] {
        let bytes = format!("format: {format}\nchains: {{}}\n");
        std::fs::write(&path, &bytes).unwrap();
        assert!(
            matches!(load(spec(), &path), Err(gil::StoreError::UnknownFormat { found, .. }) if found == format)
        );
        assert_eq!(std::fs::read(&path).unwrap(), bytes.as_bytes());
    }
}

#[cfg(feature = "chain-core")]
mod enabled {
    use super::*;
    use common::first_world;
    use gil::{ChainError, CycleKind, NodeKind, Project, ProjectSession, save};
    use serde_norway::Value;
    fn yaml(path: &std::path::Path) -> Value {
        serde_norway::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }
    fn write(path: &std::path::Path, value: &Value) {
        std::fs::write(path, serde_norway::to_string(value).unwrap()).unwrap();
    }
    #[test]
    fn chain_owns_interview_then_experiment_and_restores_current_and_self() {
        let dir = scratch("chain-lifetime");
        let path = dir.join(gil::STATE_PATH);
        let mut project = Project::start(spec(), first_world());
        let chain = project.current_chain().unwrap();
        assert_eq!(chain.id().to_string(), "chain:C1");
        assert_eq!(chain.current_cycle().to_string(), "cycle:C1");
        assert_eq!(chain.entry_snapshot(), project.world_snapshot());
        assert!(chain.exit_snapshot().is_none());
        assert_eq!(chain.cycles().current().kind(), CycleKind::Interview);
        assert!(project.active_will().is_none());
        assert_eq!(project.current_existence().current_journey().revision(), 0);
        assert!(
            project.open_child_cycle(CycleKind::Experiment).is_err(),
            "opening Interview must close first"
        );
        assert!(matches!(
            project.open_child_chain(),
            Err(ChainError::ParentOpen(_))
        ));
        save(&project, &path).unwrap();
        let first = yaml(&path);
        assert_eq!(first["format"], Value::from(6));
        assert!(first.get("cycles").is_none(), "no orphan root Cycle graph");
        project = common::bootstrap_from(load(spec(), &path).unwrap());
        let open = project
            .open_action_step(NodeKind::Define, common::plan(NodeKind::Define))
            .unwrap();
        assert_eq!(
            open.step.to_string(),
            "step:C2/S1",
            "typed IDs keep their existing scope"
        );
        let chain = project.current_chain().unwrap();
        assert!(chain.owns(project.current_cycle().id().to_ref()));
        assert_eq!(chain.cycles().nodes().len(), 2);
        assert_eq!(
            project
                .current_cycle()
                .parent()
                .unwrap()
                .to_ref()
                .to_string(),
            "cycle:C1"
        );
        assert!(!chain.is_closed(), "Interview success is not Chain close");
        let journey = project.current_existence().current_journey();
        let will = project.active_will().unwrap().id();
        let world = project.world_snapshot();
        save(&project, &path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let restored = load(spec(), &path).unwrap();
        assert_eq!(
            restored
                .current_chain()
                .unwrap()
                .current_cycle()
                .to_string(),
            "cycle:C2"
        );
        assert_eq!(restored.current_existence().current_journey(), journey);
        assert_eq!(restored.active_will().unwrap().id(), will);
        assert_eq!(restored.world_snapshot(), world);
        assert!(gil::context(&restored).contains("현재 Chain: chain:C1"));
        save(&restored, &path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let session = ProjectSession::open_read_only(spec(), &path).unwrap();
        let seen = session.monitor().unwrap();
        let view = gil::monitor_view_v1(&seen).unwrap();
        assert_eq!(view.current.chain_ref.as_deref(), Some("chain:C1"));
        assert_eq!(view.chain.unwrap().cycle_refs, ["cycle:C1", "cycle:C2"]);
        assert!(gil::render_monitor_text(&seen).contains("Chain: chain:C1"));
        assert_eq!(
            std::fs::read(&path).unwrap(),
            bytes,
            "read models do not save"
        );
    }

    #[test]
    fn cycle_close_keeps_chain_open_and_refused_transitions_preserve_every_record() {
        let dir = scratch("chain-close-cycle");
        let path = dir.join(gil::STATE_PATH);
        let mut p = common::graph_with_one_closed_cycle("success");
        assert!(p.current_cycle().is_closed());
        assert!(!p.current_chain().unwrap().is_closed());
        assert!(p.current_chain().unwrap().exit_snapshot().is_none());
        let exit = p.current_cycle().exit_snapshot().unwrap();
        let journey = p.current_existence().current_journey();
        save(&p, &path).unwrap();
        let before = std::fs::read(&path).unwrap();
        assert!(p.open_child_chain().is_err());
        save(&p, &path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let opened = p.open_child_cycle(CycleKind::Experiment).unwrap();
        assert_eq!(opened.to_ref().to_string(), "cycle:C3");
        assert_eq!(p.current_cycle().entry_snapshot(), exit);
        assert_eq!(p.current_existence().current_journey(), journey);
        assert!(p.active_will().is_none());
        save(&p, &path).unwrap();
        let before = std::fs::read(&path).unwrap();
        assert!(p.open_child_cycle(CycleKind::Experiment).is_err());
        save(&p, &path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let restored = load(spec(), &path).unwrap();
        assert_eq!(restored.current_chain().unwrap().cycles().nodes().len(), 3);
    }

    #[test]
    fn forged_containment_boundaries_and_current_are_rejected_without_write() {
        let dir = scratch("chain-forgery");
        let path = dir.join(gil::STATE_PATH);
        save(&common::bootstrap(), &path).unwrap();
        let valid = yaml(&path);
        for case in 0..16 {
            let mut v = valid.clone();
            match case {
                0 => v["chains"]["current"] = Value::from("chain:C2"),
                1 => v["chains"]["nodes"][0]["id"] = Value::from("C2"),
                2 => v["chains"]["nodes"][0]["existence_ref"] = Value::from("existence:X2"),
                3 => v["chains"]["nodes"][0]["entry_snapshot_ref"] = Value::from("snapshot:A999"),
                4 => v["chains"]["nodes"][0]["exit_snapshot_ref"] = Value::from("snapshot:A1"),
                5 => v["chains"]["nodes"][0]["parent_chain_ref"] = Value::from("chain:C1"),
                6 => v["chains"]["nodes"][0]["state"] = Value::from("closed"),
                7 => v["cycles"] = v["chains"]["nodes"][0]["cycles"].clone(),
                8 => {
                    let extra = v["chains"]["nodes"][0].clone();
                    v["chains"]["nodes"].as_sequence_mut().unwrap().push(extra);
                }
                9 => v["chains"]["nodes"][0]["cycles"]["current"] = Value::from(999),
                10 => v["chains"]["nodes"][0]["cycles"]["nodes"][1]["parent"] = Value::from(999),
                11 => {
                    v["chains"]["nodes"][0]["cycles"]["nodes"][1]["chain_ref"] =
                        Value::from("chain:C2")
                }
                12 => v["chains"]["next_id"] = Value::from(1),
                13 => {
                    v.as_mapping_mut().unwrap().remove(Value::from("chains"));
                }
                14 => v["format"] = Value::from(4),
                _ => v["chains"]["nodes"][0]["report"] = Value::from("invented approval"),
            }
            write(&path, &v);
            let before = std::fs::read(&path).unwrap();
            assert!(load(spec(), &path).is_err(), "accepted forged case {case}");
            assert_eq!(std::fs::read(&path).unwrap(), before);
        }
    }

    #[test]
    fn visual_chain_format_cannot_silently_lose_its_registry_or_storage_contract() {
        let dir = scratch("chain-visual-contract");
        common::private_storage(&dir);
        let path = dir.join(gil::STATE_PATH);
        let p = common::bootstrap().prepare_visual_upgrade().unwrap();
        save(&p, &path).unwrap();
        let valid = yaml(&path);
        assert_eq!(valid["format"], Value::from(7));
        assert!(load(spec(), &path).unwrap().visual_state().is_some());
        for variant in 0..2 {
            let mut forged = valid.clone();
            if variant == 0 { forged["format"] = Value::from(6); }
            else { forged.as_mapping_mut().unwrap().remove(Value::from("visuals")); }
            write(&path, &forged);
            let before = std::fs::read(&path).unwrap();
            assert!(matches!(load(spec(), &path), Err(gil::StoreError::VisualState(_))));
            assert_eq!(std::fs::read(&path).unwrap(), before);
        }
    }

    #[test]
    fn old_formats_stay_unwrapped_and_keep_their_records_on_commit() {
        for visual in [false, true] {
            let dir = scratch(if visual {
                "chain-old-five"
            } else {
                "chain-old-four"
            });
            let path = dir.join(gil::STATE_PATH);
            common::private_storage(&dir);
            let project = common::bootstrap();
            let project = if visual {
                project.prepare_visual_upgrade().unwrap()
            } else {
                project
            };
            save(&project, &path).unwrap();
            // Synthetic historical shape, never a migration of user data.
            let mut v = yaml(&path);
            v["cycles"] = v["chains"]["nodes"][0]["cycles"].clone();
            v.as_mapping_mut().unwrap().remove(Value::from("chains"));
            v["format"] = Value::from(if visual { 5 } else { 4 });
            write(&path, &v);
            let mut old = load(spec(), &path).unwrap();
            assert!(old.current_chain().is_none());
            assert_eq!(old.visual_state().is_some(), visual);
            assert!(matches!(
                old.open_child_chain(),
                Err(ChainError::LegacyProject)
            ));
            save(&old, &path).unwrap();
            let before = std::fs::read(&path).unwrap();
            let old = load(spec(), &path).unwrap();
            save(&old, &path).unwrap();
            assert_eq!(std::fs::read(&path).unwrap(), before);
            assert_eq!(
                yaml(&path),
                v,
                "all historical fields, reports, worlds and Journey survive"
            );
            assert!(!gil::context(&old).contains("현재 Chain"));
        }
    }

    #[test]
    fn cli_start_and_restart_expose_the_same_chain_and_refuse_child() {
        let root = std::env::temp_dir().join(format!("gil-chain-cli-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let run = |args: &[&str]| {
            std::process::Command::new(env!("CARGO_BIN_EXE_gil"))
                .args(args)
                .current_dir(&root)
                .output()
                .unwrap()
        };
        let start = run(&["start"]);
        assert!(
            start.status.success(),
            "{}",
            String::from_utf8_lossy(&start.stderr)
        );
        assert!(String::from_utf8_lossy(&start.stdout).contains("chain:C1"));
        let before = std::fs::read(root.join(gil::STATE_PATH)).unwrap();
        assert!(String::from_utf8_lossy(&run(&["context"]).stdout).contains("chain:C1"));
        let denied = run(&["open", "chain"]);
        assert!(!denied.status.success());
        assert!(String::from_utf8_lossy(&denied.stderr).contains("자식 Chain"));
        assert_eq!(std::fs::read(root.join(gil::STATE_PATH)).unwrap(), before);
        assert!(
            !run(&["start"]).status.success(),
            "restart cannot reinitialize a Chain"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
