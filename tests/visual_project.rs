mod common;

use gil::visual_report::project::{BoundaryError, MigrationBoundary, ProjectVisualGraph};
use gil::visual_report::registry::codec::{self, ClosedReportKind, CodecError, VisualGraph};
use gil::visual_report::registry::{
    ImageKind, NewVisualAsset, RegistryError, ReportOwner, ReportVisuals, SourceKind,
    VisualRegistry,
};
use gil::visual_report::{VisualDeclaration, VisualUse};
use gil::{CycleKind, CycleRef, NodeKind, StepRef};
use std::num::{NonZeroU32, NonZeroU64};

fn step(c: u32, s: u32) -> StepRef {
    StepRef::new(CycleRef::new(c).unwrap(), s).unwrap()
}
fn metadata() -> NewVisualAsset {
    NewVisualAsset {
        digest: [1; 32],
        image_kind: ImageKind::Png,
        byte_len: NonZeroU64::new(10).unwrap(),
        width: NonZeroU32::new(1).unwrap(),
        height: NonZeroU32::new(1).unwrap(),
        source_kind: SourceKind::Captured,
        source_description: "합성 자료".into(),
    }
}

#[test]
fn real_project_controls_kind_and_closed_state_not_callers_claims() {
    let mut project = common::bootstrap();
    let boundary = MigrationBoundary::capture(&project);
    common::up_to_verify(&mut project);
    let graph = ProjectVisualGraph::new(&project, &boundary).unwrap();
    assert_eq!(
        graph.closed_report_kind(ReportOwner::Step(step(2, 1))),
        Some(ClosedReportKind::Step(NodeKind::Define))
    );
    assert_eq!(
        graph.closed_report_kind(ReportOwner::Step(step(2, 3))),
        None
    );
    assert_eq!(
        graph.prepare_close(
            &VisualRegistry::default(),
            ReportOwner::Step(step(2, 1)),
            vec![],
            |_| panic!()
        ),
        Err(RegistryError::TargetNotOpen)
    );
    assert_eq!(
        graph.prepare_close(
            &VisualRegistry::default(),
            ReportOwner::Step(step(99, 3)),
            vec![],
            |_| panic!()
        ),
        Err(RegistryError::TargetNotOpen)
    );
}

#[test]
fn open_verify_at_migration_requires_declaration_when_it_later_closes() {
    let mut project = common::bootstrap();
    common::up_to_verify(&mut project);
    let boundary = MigrationBoundary::capture(&project);
    assert!(
        ProjectVisualGraph::new(&project, &boundary)
            .unwrap()
            .required_verify_reports()
            .is_empty()
    );
    let report = common::full_report(
        project.current_cycle().rules(),
        CycleKind::Experiment,
        NodeKind::Verify,
    );
    common::close_here(&mut project, report).unwrap();
    let graph = ProjectVisualGraph::new(&project, &boundary).unwrap();
    assert_eq!(graph.required_verify_reports(), vec![step(2, 3)]);
    assert_eq!(
        codec::encode(&VisualRegistry::default(), &graph),
        Err(CodecError::MissingDeclarationRecord)
    );
}

#[test]
fn capturing_legacy_boundary_does_not_modify_reports_or_project_and_future_boundary_is_refused() {
    let earlier = common::bootstrap();
    let later = common::graph_with_one_closed_cycle("success");
    let before = format!("{later:?}");
    let boundary = MigrationBoundary::capture(&later);
    let graph = ProjectVisualGraph::new(&later, &boundary).unwrap();
    assert!(graph.required_verify_reports().is_empty());
    assert!(codec::encode(&VisualRegistry::default(), &graph).is_ok());
    assert_eq!(format!("{later:?}"), before);
    assert!(matches!(
        ProjectVisualGraph::new(&earlier, &boundary),
        Err(BoundaryError::BeyondCurrentJourney)
    ));
}

#[test]
fn real_close_provenance_rejects_future_reference_and_accepts_same_verify_and_final_cycle() {
    let project = common::graph_with_one_closed_cycle("success");
    let boundary = MigrationBoundary::default();
    let graph = ProjectVisualGraph::new(&project, &boundary).unwrap();
    assert!(graph.reference_was_available(step(2, 3), ReportOwner::Step(step(2, 3))));
    assert!(graph.reference_was_available(step(2, 3), ReportOwner::Step(step(2, 5))));
    assert!(
        graph.reference_was_available(step(2, 3), ReportOwner::Cycle(CycleRef::new(2).unwrap()))
    );
    assert!(!graph.reference_was_available(step(2, 3), ReportOwner::Step(step(2, 1))));
    assert!(!graph.reference_was_available(step(2, 1), ReportOwner::Step(step(2, 5))));
    assert!(!graph.reference_was_available(step(99, 3), ReportOwner::Step(step(2, 5))));
}

#[test]
fn real_verify_close_and_later_define_share_asset_through_codec() {
    let mut project = common::bootstrap();
    let boundary = MigrationBoundary::capture(&project);
    common::up_to_verify(&mut project);
    let graph = ProjectVisualGraph::new(&project, &boundary).unwrap();
    let mut asset_id = None;
    let candidate = graph
        .prepare_close(
            &VisualRegistry::default(),
            ReportOwner::Step(step(2, 3)),
            vec![metadata()],
            |ids| {
                asset_id = Some(ids[0]);
                ReportVisuals::Verify(VisualDeclaration::Attached {
                    uses: vec![VisualUse {
                        asset: ids[0],
                        caption: "화면".into(),
                        alt: "계산기".into(),
                        purpose: "관측".into(),
                    }],
                })
            },
        )
        .unwrap();
    // registry 후보만 있고 실제 Graph Verify는 아직 열려 있으므로 저장 검증을 통과할 수 없다.
    assert_eq!(
        codec::encode(&candidate, &graph),
        Err(CodecError::MissingClosedReport)
    );
    let report = common::full_report(
        project.current_cycle().rules(),
        CycleKind::Experiment,
        NodeKind::Verify,
    );
    common::close_here(&mut project, report).unwrap();
    common::walked(&mut project, NodeKind::Analysis);
    let outcome = common::opened(&mut project, NodeKind::Outcome);
    let report = common::full_report(
        project.current_cycle().rules(),
        CycleKind::Experiment,
        NodeKind::Outcome,
    )
    .with("verdict", "success")
    .with("next_direction.action", "close_cycle");
    common::close_here(&mut project, report).unwrap();
    let report = common::cycle_report(
        project.current_cycle(),
        "success",
        common::step_id(&project, outcome),
    );
    project.close_cycle(report).unwrap();
    project.open_child_cycle(CycleKind::Experiment).unwrap();
    common::opened(&mut project, NodeKind::Define);
    let graph = ProjectVisualGraph::new(&project, &boundary).unwrap();
    let candidate = graph
        .prepare_close(&candidate, ReportOwner::Step(step(3, 1)), vec![], |_| {
            ReportVisuals::References(vec![VisualUse {
                asset: asset_id.unwrap(),
                caption: "이전 화면".into(),
                alt: "계산기".into(),
                purpose: "문제 정의".into(),
            }])
        })
        .unwrap();
    let report = common::full_report(
        project.current_cycle().rules(),
        CycleKind::Experiment,
        NodeKind::Define,
    );
    common::close_here(&mut project, report).unwrap();
    let graph = ProjectVisualGraph::new(&project, &boundary).unwrap();
    let restored = codec::decode(&codec::encode(&candidate, &graph).unwrap(), &graph).unwrap();
    assert_eq!(restored, candidate);
    assert_eq!(
        restored
            .asset_for_report(ReportOwner::Step(step(3, 1)), asset_id.unwrap())
            .unwrap()
            .origin(),
        step(2, 3)
    );
}

#[test]
fn format_four_read_and_boundary_planning_leave_disk_bytes_unchanged() {
    let project = common::graph_with_one_closed_cycle("success");
    let dir = common::scratch("visual-project-boundary-read-only");
    let path = dir.join(gil::STATE_PATH);
    gil::save(&project, &path).unwrap();
    let before = std::fs::read(&path).unwrap();
    assert!(String::from_utf8_lossy(&before).starts_with("format: 4\n"));
    let loaded = gil::load(common::spec(), &path).unwrap();
    let boundary = MigrationBoundary::capture(&loaded);
    let graph = ProjectVisualGraph::new(&loaded, &boundary).unwrap();
    let empty = VisualRegistry::default();
    assert_eq!(
        codec::decode(&codec::encode(&empty, &graph).unwrap(), &graph).unwrap(),
        empty
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn previous_verify_is_legacy_but_next_cycle_verify_is_required() {
    let mut project = common::graph_with_one_closed_cycle("success");
    let boundary = MigrationBoundary::capture(&project);
    project.open_child_cycle(CycleKind::Experiment).unwrap();
    common::up_to_verify(&mut project);
    let report = common::full_report(
        project.current_cycle().rules(),
        CycleKind::Experiment,
        NodeKind::Verify,
    );
    common::close_here(&mut project, report).unwrap();
    let graph = ProjectVisualGraph::new(&project, &boundary).unwrap();
    assert_eq!(graph.required_verify_reports(), vec![step(3, 3)]);
    assert!(graph.reference_was_available(step(2, 3), ReportOwner::Step(step(3, 1))));
    assert!(!graph.reference_was_available(step(3, 3), ReportOwner::Step(step(2, 5))));
}
