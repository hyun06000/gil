//! Public Core entry only, not CLI/MCP routing or Host rendering acceptance.
#![cfg(all(
    feature = "visual-authoring",
    any(all(target_os = "macos", target_arch = "aarch64"), windows)
))]
mod common;
use gil::visual_report::{VisualDeclaration, registry::ReportVisuals};
use gil::{CycleKind, NodeKind, ProjectSession};

#[test]
fn new_visual_project_requires_declaration_at_its_first_verify_and_reopens() {
    let root = std::env::temp_dir().join(format!(
        "gil-new-visual-public-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let mut session = ProjectSession::start_visual(common::spec(), &root).unwrap();
    *session.project_mut() = common::bootstrap_from(session.project().clone());
    common::up_to_verify(session.project_mut());
    session.commit().unwrap();
    let path = root.join(gil::STATE_PATH);
    let before = std::fs::read(&path).unwrap();
    let report = common::full_report(
        session.project().current_cycle().rules(),
        CycleKind::Experiment,
        NodeKind::Verify,
    );
    let world = session
        .project()
        .world_manifest(common::snapshot(1))
        .unwrap()
        .clone();
    assert!(
        session
            .project_mut()
            .close_verify_step(report.clone(), world.clone())
            .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    session
        .project_mut()
        .close_step_with_visuals(
            report,
            Some(world),
            ReportVisuals::Verify(VisualDeclaration::NotNeeded {
                reason: "合成 텍스트 반환값 검사이며 이미지가 필요하지 않다".into(),
            }),
        )
        .unwrap();
    session.commit().unwrap();
    drop(session);
    let saved = std::fs::read(&path).unwrap();
    let read = ProjectSession::open_read_only(common::spec(), &path).unwrap();
    assert!(read.project().visual_state().is_some());
    assert!(String::from_utf8_lossy(&saved).contains("合成 텍스트 반환값"));
    drop(read);
    assert!(ProjectSession::start_visual(common::spec(), &root).is_err());
    assert_eq!(std::fs::read(path).unwrap(), saved);
}
