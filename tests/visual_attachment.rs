//! Real locked Session close using synthetic Project files, never the source checkout.
#![cfg(unix)]
mod common;
use gil::visual_report::image::VisualAttachment;
use gil::visual_report::registry::{ReportOwner, ReportVisuals, SourceKind};
use gil::visual_report::{VisualAssetId, VisualDeclaration, VisualUse};
use gil::{CycleKind, CycleRef, NodeKind, ProjectSession, StepRef};
use std::{fs, path::PathBuf};

fn png() -> Vec<u8> {
    let mut bytes = Vec::new();
    png::Encoder::new(&mut bytes, 1, 1)
        .write_header()
        .unwrap()
        .write_image_data(&[127])
        .unwrap();
    bytes
}
fn setup(label: &str) -> (PathBuf, ProjectSession) {
    let root = common::scratch(label);
    let mut p = common::bootstrap();
    common::up_to_verify(&mut p);
    gil::save(
        &p.prepare_visual_upgrade().unwrap(),
        root.join(gil::STATE_PATH),
    )
    .unwrap();
    fs::write(root.join("screen.png"), png()).unwrap();
    let session = ProjectSession::open(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    (root, session)
}
fn input(path: &str) -> Vec<VisualAttachment> {
    vec![VisualAttachment {
        path: path.into(),
        source_kind: SourceKind::Generated,
        source_description: "합성 그림".into(),
    }]
}
fn report(session: &ProjectSession) -> gil::Report {
    common::full_report(
        session.project().current_cycle().rules(),
        CycleKind::Experiment,
        NodeKind::Verify,
    )
}
fn attached(ids: &[VisualAssetId]) -> VisualDeclaration {
    VisualDeclaration::Attached {
        uses: ids
            .iter()
            .map(|id| VisualUse {
                asset: *id,
                caption: "실험 화면".into(),
                alt: "회색 픽셀".into(),
                purpose: "출력 확인".into(),
            })
            .collect(),
    }
}
fn owner() -> ReportOwner {
    ReportOwner::Step(StepRef::new(CycleRef::new(2).unwrap(), 3).unwrap())
}

fn committed(label: &str) -> (PathBuf, ProjectSession, VisualAssetId, PathBuf) {
    let (root, mut session) = setup(label);
    session
        .close_verify_with_images_and_commit(report(&session), input("screen.png"), attached)
        .unwrap();
    let registry = session.project().visual_state().unwrap().registry();
    let ReportVisuals::Verify(VisualDeclaration::Attached { uses }) =
        registry.report(owner()).unwrap()
    else {
        panic!()
    };
    let id = uses[0].asset;
    let hex: String = registry
        .asset_for_report(owner(), id)
        .unwrap()
        .metadata()
        .digest
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect();
    let blob = root
        .join(".gil/artifacts/blobs/sha256")
        .join(&hex[..2])
        .join(&hex[2..]);
    (root, session, id, blob)
}

#[test]
fn display_projection_survives_reopen_and_source_deletion() {
    use gil::visual_report::display::VisualStateV1;
    let (root, session, id, _) = committed("visual-display-reopen");
    let before = fs::read(root.join(gil::STATE_PATH)).unwrap();
    let view = session.project().report_visuals_v1(owner()).unwrap();
    assert_eq!(view.state, VisualStateV1::Attached);
    assert_eq!(view.uses.len(), 1);
    let item = &view.uses[0];
    assert_eq!(item.asset_id, "1");
    assert_eq!(item.origin_step_ref, "step:C2/S3");
    assert_eq!(item.caption, "실험 화면");
    assert_eq!(item.alt, "회색 픽셀");
    assert_eq!(item.source_kind, "generated");
    assert_eq!(item.mime_type, "image/png");
    assert_eq!((item.width, item.height, item.byte_length), (1, 1, png().len() as u64));
    use sha2::{Digest, Sha256};
    assert_eq!(item.sha256, format!("{:x}", Sha256::digest(png())));
    let json = serde_json::to_value(&view).unwrap();
    assert_eq!(json["state"], "attached");
    assert_eq!(json["owner"], serde_json::json!({"kind":"step","ref":"step:C2/S3"}));
    assert!(json["uses"][0]["asset_id"].is_string());
    assert!(json.get("reason").is_none());
    drop(session);
    fs::remove_file(root.join("screen.png")).unwrap();
    let readonly = ProjectSession::open_read_only(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    assert_eq!(readonly.project().report_visuals_v1(owner()).unwrap(), view);
    assert_eq!(readonly.read_report_image(owner(), id).unwrap().bytes(), png());
    assert_eq!(fs::read(root.join(gil::STATE_PATH)).unwrap(), before);
}

#[test]
fn display_projection_distinguishes_open_legacy_unknown_and_boundary() {
    use gil::visual_report::display::{VisualDetailError, VisualStateV1};
    let (_, session) = setup("visual-display-states");
    let p = session.project();
    assert_eq!(p.report_visuals_v1(owner()).unwrap().state, VisualStateV1::OpenReport);
    let c = CycleRef::new(2).unwrap();
    assert_eq!(p.report_visuals_v1(ReportOwner::Cycle(c)).unwrap().state, VisualStateV1::OpenReport);
    assert_eq!(p.report_visuals_v1(ReportOwner::Step(StepRef::new(c, 1).unwrap())).unwrap().state,
        VisualStateV1::LegacyUnspecified);
    assert_eq!(p.report_visuals_v1(ReportOwner::Cycle(CycleRef::new(99).unwrap())), Err(VisualDetailError::NotFound));
    for node in p.current_cycle().steps().nodes() {
        if matches!(node.kind, NodeKind::CycleEntry | NodeKind::CycleExit) {
            assert_eq!(p.report_visuals_v1(ReportOwner::Step(p.current_cycle().step_ref(node.id))), Err(VisualDetailError::NotFound));
        }
    }
    let legacy = common::graph_with_one_closed_cycle("success");
    assert_eq!(legacy.report_visuals_v1(owner()).unwrap().state, VisualStateV1::LegacyUnspecified);
    assert!(legacy.visual_state().is_none());
}

#[test]
fn display_projection_partial_missing_preserves_multiple_use_order() {
    use gil::visual_report::display::VisualStateV1;
    let (root, mut session) = setup("visual-display-partial");
    fs::write(root.join("second.png"), png()).unwrap();
    let mut files = input("screen.png");
    files.extend(input("second.png"));
    session.close_verify_with_images_and_commit(report(&session), files, |ids| {
        let VisualDeclaration::Attached { mut uses } = attached(ids) else { unreachable!() };
        uses.reverse();
        VisualDeclaration::Missing { reason: "세 번째 화면 누락".into(), follow_up: "후속 Cycle".into(), uses }
    }).unwrap();
    let view = session.project().report_visuals_v1(owner()).unwrap();
    assert_eq!(view.state, VisualStateV1::Missing { reason: "세 번째 화면 누락".into(), follow_up: "후속 Cycle".into() });
    assert_eq!(view.uses.iter().map(|u| u.asset_id.as_str()).collect::<Vec<_>>(), vec!["2", "1"]);
    assert_eq!(view.uses[0].sha256, view.uses[1].sha256);
}

#[test]
fn display_projection_preserves_missing_and_not_needed_without_inventing_images() {
    use gil::visual_report::display::VisualStateV1;
    for missing in [false, true] {
        let mut p = common::bootstrap();
        common::up_to_verify(&mut p);
        let mut p = p.prepare_visual_upgrade().unwrap();
        let declaration = if missing {
            VisualDeclaration::Missing { reason: "추가 확인 필요".into(), follow_up: "다음 실험".into(), uses: vec![] }
        } else { VisualDeclaration::NotNeeded { reason: "문자 비교 실험".into() } };
        let report = common::full_report(p.current_cycle().rules(), CycleKind::Experiment, NodeKind::Verify);
        p.close_step_with_visuals(report, Some(common::first_world()), ReportVisuals::Verify(declaration)).unwrap();
        let view = p.report_visuals_v1(owner()).unwrap();
        assert!(view.uses.is_empty());
        assert_eq!(view.state, if missing {
            VisualStateV1::Missing { reason: "추가 확인 필요".into(), follow_up: "다음 실험".into() }
        } else { VisualStateV1::NotNeeded { reason: "문자 비교 실험".into() } });
    }
}

#[test]
fn display_projection_reuse_preserves_asset_and_report_specific_caption() {
    use gil::visual_report::display::VisualStateV1;
    let (root, mut session, id, _) = committed("visual-display-reuse");
    let original = session.project().report_visuals_v1(owner()).unwrap().uses[0].clone();
    let p = session.project_mut();
    let analysis = common::opened(p, NodeKind::Analysis);
    let report = common::full_report(p.current_cycle().rules(), CycleKind::Experiment, NodeKind::Analysis);
    p.close_step_with_visuals(report, None, ReportVisuals::References(vec![VisualUse {
        asset: id, caption: "<script>자료 해석</script>".into(), alt: "다시 쓰는 그림".into(), purpose: "해석".into(),
    }])).unwrap();
    let reused = p.report_visuals_v1(ReportOwner::Step(analysis)).unwrap();
    assert_eq!(reused.state, VisualStateV1::References);
    assert_eq!(reused.uses[0].caption, "<script>자료 해석</script>");
    assert_eq!(reused.uses[0].sha256, original.sha256);
    assert_eq!(reused.uses[0].origin_step_ref, original.origin_step_ref);
    assert_eq!(p.report_visuals_v1(owner()).unwrap().uses[0], original);
    let usage = |caption: &str| VisualUse { asset: id, caption: caption.into(), alt: "회색".into(), purpose: "인용".into() };
    let outcome = common::opened(p, NodeKind::Outcome);
    let report = common::full_report(p.current_cycle().rules(), CycleKind::Experiment, NodeKind::Outcome)
        .with("verdict", "success");
    p.close_step_with_visuals(report, None, ReportVisuals::References(vec![usage("최종 판정")])).unwrap();
    let report = common::cycle_report(p.current_cycle(), "success", common::step_id(p, outcome));
    let cycle_owner = ReportOwner::Cycle(p.current_cycle().id().to_ref());
    p.close_cycle_with_visuals(report, vec![usage("Cycle 요약")]).unwrap();
    assert_eq!(p.report_visuals_v1(cycle_owner).unwrap().uses[0].caption, "Cycle 요약");
    p.open_child_cycle(CycleKind::Experiment).unwrap();
    let define = common::opened(p, NodeKind::Define);
    let report = common::full_report(p.current_cycle().rules(), CycleKind::Experiment, NodeKind::Define);
    p.close_step_with_visuals(report, None, ReportVisuals::References(vec![usage("다음 문제의 근거")])).unwrap();
    for target in [ReportOwner::Step(outcome), cycle_owner, ReportOwner::Step(define)] {
        let view = p.report_visuals_v1(target).unwrap();
        assert_eq!(view.state, VisualStateV1::References);
        assert_eq!(view.uses[0].origin_step_ref, original.origin_step_ref);
        assert_eq!(view.uses[0].sha256, original.sha256);
    }
    session.commit().unwrap();
    drop(session);
    let readonly = ProjectSession::open_read_only(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    assert_eq!(readonly.project().report_visuals_v1(ReportOwner::Step(analysis)).unwrap(), reused);
    assert_eq!(readonly.project().report_visuals_v1(cycle_owner).unwrap().uses[0].caption, "Cycle 요약");
    assert_eq!(readonly.project().report_visuals_v1(ReportOwner::Step(define)).unwrap().uses[0].caption, "다음 문제의 근거");
}

#[test]
fn report_scoped_read_survives_source_deletion_and_does_not_mutate_state() {
    let (root, session, id, _) = committed("visual-read-scope");
    let before = fs::read(root.join(gil::STATE_PATH)).unwrap();
    fs::remove_file(root.join("screen.png")).unwrap();
    assert_eq!(
        session.read_report_image(owner(), id).unwrap().bytes(),
        png()
    );
    let wrong = ReportOwner::Step(StepRef::new(CycleRef::new(2).unwrap(), 2).unwrap());
    assert!(session.read_report_image(wrong, id).is_err());
    assert!(
        session
            .read_report_image(
                owner(),
                VisualAssetId::new(std::num::NonZeroU64::new(999).unwrap())
            )
            .is_err()
    );
    let (_, other) = setup("visual-read-other-project");
    assert!(other.read_report_image(owner(), id).is_err());
    assert_eq!(fs::read(root.join(gil::STATE_PATH)).unwrap(), before);
}

#[test]
fn stored_read_refuses_links_at_every_internal_component() {
    use std::os::unix::fs::symlink;
    let (root, session, id, blob) = committed("visual-read-links");
    let mut paths = vec![blob.clone()];
    let mut parent = blob.parent().unwrap();
    while parent != root {
        paths.push(parent.to_path_buf());
        parent = parent.parent().unwrap();
    }
    for path in paths {
        let moved = path.with_extension("held");
        fs::rename(&path, &moved).unwrap();
        symlink(&moved, &path).unwrap();
        assert!(session.read_report_image(owner(), id).is_err(), "{path:?}");
        fs::remove_file(&path).unwrap();
        fs::rename(&moved, &path).unwrap();
    }
    let link = root.join("duplicate");
    fs::hard_link(&blob, &link).unwrap();
    assert!(session.read_report_image(owner(), id).is_err());
    fs::remove_file(link).unwrap();
    assert!(session.read_report_image(owner(), id).is_ok());
}

#[test]
fn stored_read_rejects_missing_corrupt_and_valid_but_wrong_bytes() {
    let (_, session, id, blob) = committed("visual-read-integrity");
    fs::write(&blob, b"not an image").unwrap();
    assert!(session.read_report_image(owner(), id).is_err());
    let mut different = Vec::new();
    png::Encoder::new(&mut different, 1, 1)
        .write_header()
        .unwrap()
        .write_image_data(&[42])
        .unwrap();
    fs::write(&blob, different).unwrap();
    assert!(session.read_report_image(owner(), id).is_err());
    fs::remove_file(&blob).unwrap();
    assert!(session.read_report_image(owner(), id).is_err());
    fs::create_dir(&blob).unwrap();
    assert!(session.read_report_image(owner(), id).is_err());
}

#[test]
fn read_only_session_reads_preserved_image_without_recovery_or_state_write() {
    let (root, session, id, _) = committed("visual-read-only-session");
    drop(session);
    let before = fs::read(root.join(gil::STATE_PATH)).unwrap();
    let session =
        ProjectSession::open_read_only(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    assert_eq!(
        session.read_report_image(owner(), id).unwrap().bytes(),
        png()
    );
    assert_eq!(fs::read(root.join(gil::STATE_PATH)).unwrap(), before);
}

#[test]
fn visual_state_temp_never_truncates_preexisting_files_or_links() {
    use std::os::unix::fs::symlink;
    let (root, session) = setup("visual-state-temp-boundary");
    let state = root.join(gil::STATE_PATH);
    let before = fs::read(&state).unwrap();
    let temp = root.join(".gil/state.yaml.writing");
    let victim = root.join("keep.txt");
    fs::write(&victim, b"keep").unwrap();
    for kind in 0..3 {
        match kind {
            0 => fs::write(&temp, b"unfinished").unwrap(),
            1 => symlink(&victim, &temp).unwrap(),
            _ => fs::hard_link(&victim, &temp).unwrap(),
        }
        assert!(gil::save(session.project(), &state).is_err());
        assert_eq!(fs::read(&victim).unwrap(), b"keep");
        assert_eq!(fs::read(&state).unwrap(), before);
        if kind == 0 {
            assert_eq!(fs::read(&temp).unwrap(), b"unfinished");
        }
        fs::remove_file(&temp).unwrap();
    }
}

#[test]
fn visual_state_refuses_linked_internal_parent_without_writing_target() {
    use std::os::unix::fs::symlink;
    let (root, session) = setup("visual-state-parent-boundary");
    let moved = root.join("held-gil");
    fs::rename(root.join(".gil"), &moved).unwrap();
    symlink(&moved, root.join(".gil")).unwrap();
    let before = fs::read(moved.join("state.yaml")).unwrap();
    assert!(gil::save(session.project(), root.join(gil::STATE_PATH)).is_err());
    assert_eq!(fs::read(moved.join("state.yaml")).unwrap(), before);
    assert!(!moved.join("state.yaml.writing").exists());
}

#[test]
fn visual_state_replaces_target_link_not_its_referent() {
    use std::os::unix::fs::symlink;
    let (root, session) = setup("visual-state-target-boundary");
    let victim = root.join("keep.txt");
    fs::write(&victim, b"keep").unwrap();
    let state = root.join(gil::STATE_PATH);
    fs::remove_file(&state).unwrap();
    symlink(&victim, &state).unwrap();
    gil::save(session.project(), &state).unwrap();
    assert_eq!(fs::read(victim).unwrap(), b"keep");
    assert!(
        !fs::symlink_metadata(&state)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(gil::load(common::spec(), &state).is_ok());
}

#[test]
fn attachment_close_persists_report_provenance_and_bytes_before_receipt() {
    let (root, mut session) = setup("visual-attach-success");
    session
        .close_verify_with_images_and_commit(report(&session), input("screen.png"), attached)
        .unwrap();
    let loaded = gil::load(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    let registry = loaded.visual_state().unwrap().registry();
    let ReportVisuals::Verify(VisualDeclaration::Attached { uses }) =
        registry.report(owner()).unwrap()
    else {
        panic!()
    };
    let asset = registry.asset_for_report(owner(), uses[0].asset).unwrap();
    assert_eq!(
        asset.origin(),
        StepRef::new(CycleRef::new(2).unwrap(), 3).unwrap()
    );
    let hex: String = asset
        .metadata()
        .digest
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect();
    let blob = root
        .join(".gil/artifacts/blobs/sha256")
        .join(&hex[..2])
        .join(&hex[2..]);
    fs::remove_file(root.join("screen.png")).unwrap();
    assert_eq!(fs::read(blob).unwrap(), png());
    assert!(
        session
            .close_verify_with_images_and_commit(report(&session), input("screen.png"), attached)
            .is_err()
    );
}

#[test]
fn invalid_report_and_declaration_leave_state_and_project_unchanged() {
    let (root, mut session) = setup("visual-attach-bad-report");
    let before = fs::read(root.join(gil::STATE_PATH)).unwrap();
    let memory = format!("{:?}", session.project());
    assert!(
        session
            .close_verify_with_images_and_commit(gil::Report::new(), input("screen.png"), attached)
            .is_err()
    );
    assert!(
        session
            .close_verify_with_images_and_commit(report(&session), input("screen.png"), |_| {
                VisualDeclaration::NotNeeded {
                    reason: "첨부와 모순".into(),
                }
            })
            .is_err()
    );
    assert_eq!(fs::read(root.join(gil::STATE_PATH)).unwrap(), before);
    assert_eq!(format!("{:?}", session.project()), memory);
}

#[test]
fn file_changed_during_preparation_refuses_without_logical_close() {
    let (root, mut session) = setup("visual-attach-race");
    let before = fs::read(root.join(gil::STATE_PATH)).unwrap();
    assert!(
        session
            .close_verify_with_images_and_commit(report(&session), input("screen.png"), |ids| {
                fs::write(root.join("screen.png"), b"changed by another writer").unwrap();
                attached(ids)
            })
            .is_err()
    );
    assert_eq!(fs::read(root.join(gil::STATE_PATH)).unwrap(), before);
    assert!(session.can_confirm_artifact());
}

#[test]
fn replacing_file_with_identical_bytes_still_changes_identity() {
    let (root, mut session) = setup("visual-attach-replaced");
    let before = fs::read(root.join(gil::STATE_PATH)).unwrap();
    assert!(
        session
            .close_verify_with_images_and_commit(report(&session), input("screen.png"), |ids| {
                fs::rename(root.join("screen.png"), root.join("old.png")).unwrap();
                fs::write(root.join("screen.png"), png()).unwrap();
                attached(ids)
            })
            .is_err()
    );
    assert_eq!(fs::read(root.join(gil::STATE_PATH)).unwrap(), before);
}

#[test]
fn source_links_parent_links_hardlinks_and_unsafe_names_are_refused() {
    use std::os::unix::fs::symlink;
    let (root, mut session) = setup("visual-attach-boundary");
    fs::create_dir(root.join("images")).unwrap();
    fs::write(root.join("images/normal.png"), png()).unwrap();
    symlink(root.join("screen.png"), root.join("linked.png")).unwrap();
    symlink(root.join("images"), root.join("linked-dir")).unwrap();
    fs::hard_link(root.join("screen.png"), root.join("hard.png")).unwrap();
    let before = fs::read(root.join(gil::STATE_PATH)).unwrap();
    for path in [
        "../screen.png",
        "/tmp/screen.png",
        ".gil/state.yaml",
        "images/../screen.png",
        "linked.png",
        "linked-dir/normal.png",
        "hard.png",
        "NUL.png",
        "C:screen.png",
        "images\\normal.png",
    ] {
        assert!(
            session
                .close_verify_with_images_and_commit(report(&session), input(path), attached)
                .is_err(),
            "{path}"
        );
    }
    assert_eq!(fs::read(root.join(gil::STATE_PATH)).unwrap(), before);
}

#[test]
fn directory_fifo_and_duplicate_input_do_not_become_attachments() {
    let (root, mut session) = setup("visual-attach-special");
    fs::create_dir(root.join("directory.png")).unwrap();
    let path =
        std::ffi::CString::new(root.join("pipe.png").as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
    for name in ["directory.png", "pipe.png"] {
        assert!(
            session
                .close_verify_with_images_and_commit(report(&session), input(name), attached)
                .is_err()
        );
    }
    let mut duplicates = input("screen.png");
    duplicates.extend(input("screen.png"));
    assert!(
        session
            .close_verify_with_images_and_commit(report(&session), duplicates, attached)
            .is_err()
    );
}

#[test]
fn failed_state_write_does_not_publish_success_or_mutate_in_memory_project() {
    let (root, mut session) = setup("visual-attach-save-failure");
    let before = fs::read(root.join(gil::STATE_PATH)).unwrap();
    let memory = format!("{:?}", session.project());
    fs::create_dir(root.join(".gil/state.yaml.writing")).unwrap();
    assert!(
        session
            .close_verify_with_images_and_commit(report(&session), input("screen.png"), attached)
            .is_err()
    );
    assert_eq!(fs::read(root.join(gil::STATE_PATH)).unwrap(), before);
    assert_eq!(format!("{:?}", session.project()), memory);
}

#[test]
fn not_needed_without_files_can_close_and_persist_explicitly() {
    let (_, mut session) = setup("visual-attach-not-needed");
    session
        .close_verify_with_images_and_commit(report(&session), vec![], |_| {
            VisualDeclaration::NotNeeded {
                reason: "문자열만 검사했다".into(),
            }
        })
        .unwrap();
    assert!(matches!(
        session
            .project()
            .visual_state()
            .unwrap()
            .registry()
            .report(owner()),
        Some(ReportVisuals::Verify(VisualDeclaration::NotNeeded { .. }))
    ));
}

#[test]
fn non_verify_and_legacy_projects_refuse_before_reading_files() {
    let root = common::scratch("visual-attach-preflight");
    let mut p = common::bootstrap();
    gil::save(
        &p.prepare_visual_upgrade().unwrap(),
        root.join(gil::STATE_PATH),
    )
    .unwrap();
    let mut session = ProjectSession::open(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    assert!(
        session
            .close_verify_with_images_and_commit(
                gil::Report::new(),
                input("nonexistent.png"),
                attached
            )
            .unwrap_err()
            .to_string()
            .contains("열린 Verify")
    );
    drop(session);
    common::up_to_verify(&mut p);
    gil::save(&p, root.join(gil::STATE_PATH)).unwrap();
    let mut session = ProjectSession::open(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    assert!(
        session
            .close_verify_with_images_and_commit(
                report(&session),
                input("nonexistent.png"),
                attached
            )
            .unwrap_err()
            .to_string()
            .contains("이관")
    );
}

#[test]
fn too_many_inputs_are_refused_before_opening_any_file() {
    let (_, mut session) = setup("visual-attach-count-limit");
    let inputs = (0..9)
        .flat_map(|n| input(&format!("missing-{n}.png")))
        .collect();
    assert!(
        session
            .close_verify_with_images_and_commit(report(&session), inputs, attached)
            .unwrap_err()
            .to_string()
            .contains("8개")
    );
}
