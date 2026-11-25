//! Public Core entry only, not CLI/MCP routing or Host rendering acceptance.
#![cfg(all(
    feature = "visual-authoring",
    any(all(target_os = "macos", target_arch = "aarch64"), all(target_os = "linux", target_arch = "x86_64"), windows)
))]
mod common;
use gil::visual_report::{VisualDeclaration, registry::ReportVisuals};
use gil::{CycleKind, NodeKind, ProjectSession};

/// The creation entry in every profile that builds it: one committed image-report project (an
/// Interview, no Step, the visual registry present), a fresh reader sees the same record, and a
/// second creation in that folder is refused without changing a byte. The first-Verify declaration
/// below is walked with the bare-approval fixture, which an approved-plan build refuses by contract;
/// there the declaration rules are checked over the real App path (`tests/exploration_flow.rs`,
/// `tests/exploration_evidence.rs`).
#[test]
fn the_creation_entry_commits_one_visual_project_and_refuses_a_second_creation() {
    let root = std::env::temp_dir().join(format!(
        "gil-new-visual-entry-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("notes.txt"), "synthetic\n").unwrap();
    let session = ProjectSession::start_visual(common::spec(), &root).unwrap();
    assert!(session.project().visual_state().is_some());
    assert_eq!(session.project().current_cycle().kind(), CycleKind::Interview);
    drop(session);
    let path = root.join(gil::STATE_PATH);
    let saved = std::fs::read(&path).unwrap();
    let head = String::from_utf8_lossy(&saved[..saved.iter().position(|b| *b == b'\n').unwrap()]).into_owned();
    let format: u32 = head.strip_prefix("format: ").expect("the record starts with its format").parse().unwrap();
    assert!(gil::is_visual_format(format), "the creation wrote a visual storage format, not {format}");
    // The creation never writes the tracked-scope formats: that kind starts only through its approval.
    assert!(format != gil::TRACKING_FORMAT && format != gil::TRACKING_VISUAL_FORMAT, "format {format}");
    let read = ProjectSession::open_read_only(common::spec(), &path).unwrap();
    assert!(read.project().visual_state().is_some());
    assert_eq!(read.project().current_cycle().kind(), CycleKind::Interview);
    assert!(read.project().current_cycle().step_now_open().is_none(), "one Interview, no Step");
    drop(read);
    assert!(ProjectSession::start_visual(common::spec(), &root).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), saved);
    assert_eq!(std::fs::read_to_string(root.join("notes.txt")).unwrap(), "synthetic\n");
}

#[cfg(not(feature = "approved-plan"))]
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
