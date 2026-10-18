//! Project가 소유하는 시각 선언: close 원자성, 명시적 format 전환, 재로딩.
mod common;

use gil::visual_report::VisualDeclaration;
use gil::visual_report::registry::{ReportOwner, ReportVisuals};
use gil::{CycleKind, CycleRef, NodeKind, Project, StepRef};
use serde_norway::Value;

fn owner(c: u32, s: u32) -> ReportOwner {
    ReportOwner::Step(StepRef::new(CycleRef::new(c).unwrap(), s).unwrap())
}
fn verify_report(project: &Project) -> gil::Report {
    common::full_report(
        project.current_cycle().rules(),
        CycleKind::Experiment,
        NodeKind::Verify,
    )
}
fn not_needed() -> ReportVisuals {
    ReportVisuals::Verify(VisualDeclaration::NotNeeded {
        reason: "문자열 반환값만 검사한 실험".into(),
    })
}
fn open_visual_verify() -> Project {
    let mut p = common::bootstrap();
    common::up_to_verify(&mut p);
    p.prepare_visual_upgrade().unwrap()
}
fn close_verify(p: &mut Project, visuals: ReportVisuals) {
    p.close_step_with_visuals(verify_report(p), Some(common::first_world()), visuals)
        .unwrap();
}
fn path(label: &str) -> std::path::PathBuf {
    let root = common::scratch(label);
    // Format-5 states are loaded under the explicit owner-only contract on Windows; the fixture
    // `.gil` therefore takes the owner-only descriptor (a no-op elsewhere and for format 4).
    common::private_storage(&root);
    root.join(gil::STATE_PATH)
}

#[test]
fn explicit_candidate_does_not_change_original_and_cannot_recapture_boundary() {
    let original = common::graph_with_one_closed_cycle("success");
    let before = format!("{original:?}");
    let upgraded = original.prepare_visual_upgrade().unwrap();
    assert!(original.visual_state().is_none());
    assert_eq!(format!("{original:?}"), before);
    assert!(upgraded.prepare_visual_upgrade().is_err());
    assert!(
        upgraded
            .visual_state()
            .unwrap()
            .registry()
            .report(owner(2, 3))
            .is_none()
    );
}

#[test]
fn format_four_roundtrip_stays_byte_identical_without_visual_field() {
    let p = common::graph_with_one_closed_cycle("success");
    let file = path("visual-store-legacy");
    gil::save(&p, &file).unwrap();
    let before = std::fs::read(&file).unwrap();
    let loaded = gil::load(common::spec(), &file).unwrap();
    gil::save(&loaded, &file).unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), before);
    assert!(!String::from_utf8_lossy(&before).contains("visuals:"));
}

#[test]
fn explicit_format_five_roundtrip_preserves_legacy_reports_and_boundary() {
    let old = common::graph_with_one_closed_cycle("success");
    let file = path("visual-store-upgrade");
    gil::save(&old.prepare_visual_upgrade().unwrap(), &file).unwrap();
    let before = std::fs::read(&file).unwrap();
    assert!(String::from_utf8_lossy(&before).starts_with("format: 5\n"));
    let mut loaded = gil::load(common::spec(), &file).unwrap();
    gil::save(&loaded, &file).unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), before);
    assert!(
        loaded
            .visual_state()
            .unwrap()
            .registry()
            .report(owner(2, 3))
            .is_none()
    );
    loaded.open_child_cycle(CycleKind::Experiment).unwrap();
    common::up_to_verify(&mut loaded);
    assert!(
        loaded
            .close_verify_step(verify_report(&loaded), common::first_world())
            .is_err()
    );
    close_verify(&mut loaded, not_needed());
    gil::save(&loaded, &file).unwrap();
    let loaded = gil::load(common::spec(), &file).unwrap();
    assert_eq!(
        loaded
            .visual_state()
            .unwrap()
            .registry()
            .report(owner(3, 3)),
        Some(&not_needed())
    );
}

#[test]
fn no_declaration_bad_declaration_and_bad_report_leave_entire_project_unchanged() {
    let mut p = open_visual_verify();
    let before = format!("{p:?}");
    assert!(
        p.close_verify_step(verify_report(&p), common::first_world())
            .is_err()
    );
    assert_eq!(format!("{p:?}"), before);
    assert!(
        p.close_step_with_visuals(
            verify_report(&p),
            Some(common::first_world()),
            ReportVisuals::Verify(VisualDeclaration::NotNeeded { reason: " ".into() })
        )
        .is_err()
    );
    assert_eq!(format!("{p:?}"), before);
    assert!(
        p.close_step_with_visuals(
            gil::Report::new(),
            Some(common::first_world()),
            not_needed()
        )
        .is_err()
    );
    assert_eq!(format!("{p:?}"), before);
    close_verify(&mut p, not_needed());
    let closed = format!("{p:?}");
    assert!(
        p.close_step_with_visuals(verify_report(&p), Some(common::first_world()), not_needed())
            .is_err()
    );
    assert_eq!(format!("{p:?}"), closed);
}

#[test]
fn missing_and_not_needed_remain_distinct_across_disk_reload() {
    for (label, visuals) in [
        ("visual-store-none", not_needed()),
        (
            "visual-store-missing",
            ReportVisuals::Verify(VisualDeclaration::Missing {
                reason: "화면 캡처 실패".into(),
                follow_up: "다음 Cycle의 Verify에서 캡처".into(),
                uses: vec![],
            }),
        ),
    ] {
        let mut p = open_visual_verify();
        close_verify(&mut p, visuals.clone());
        let file = path(label);
        gil::save(&p, &file).unwrap();
        let loaded = gil::load(common::spec(), &file).unwrap();
        assert_eq!(
            loaded
                .visual_state()
                .unwrap()
                .registry()
                .report(owner(2, 3)),
            Some(&visuals)
        );
    }
}

#[test]
fn legacy_close_does_not_silently_discard_visuals_or_upgrade() {
    let mut p = common::bootstrap();
    common::up_to_verify(&mut p);
    let before = format!("{p:?}");
    assert!(
        p.close_step_with_visuals(verify_report(&p), Some(common::first_world()), not_needed())
            .is_err()
    );
    assert_eq!(format!("{p:?}"), before);
    assert!(p.visual_state().is_none());
}

#[test]
fn stored_visual_state_rejects_inconsistent_format_missing_declaration_and_bad_boundaries() {
    let mut p = open_visual_verify();
    close_verify(&mut p, not_needed());
    let file = path("visual-store-corruption");
    gil::save(&p, &file).unwrap();
    let original: Value = serde_norway::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    for case in 0..7 {
        let mut edited = original.clone();
        match case {
            0 => {
                edited
                    .as_mapping_mut()
                    .unwrap()
                    .remove(Value::from("visuals"));
            }
            1 => edited["format"] = Value::from(4),
            2 => edited["visuals"]["registry"]["reports"] = Value::Sequence(vec![]),
            3 => edited["visuals"]["boundary"][0]["revision"] = Value::from(u32::MAX),
            4 => {
                let entry = edited["visuals"]["boundary"][0].clone();
                edited["visuals"]["boundary"]
                    .as_sequence_mut()
                    .unwrap()
                    .push(entry);
            }
            5 => edited["visuals"]["boundary"][0]["existence_ref"] = Value::from("existence:X999"),
            _ => edited["visuals"]["registry"]["version"] = Value::from(999),
        }
        let bytes = serde_norway::to_string(&edited).unwrap();
        std::fs::write(&file, &bytes).unwrap();
        assert!(gil::load(common::spec(), &file).is_err(), "case {case}");
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            bytes,
            "reader must not repair"
        );
    }
}

#[test]
fn outcome_and_final_cycle_close_keep_visual_state_and_reject_parallel_cycle() {
    let mut p = open_visual_verify();
    close_verify(&mut p, not_needed());
    assert!(p.open_child_cycle(CycleKind::Experiment).is_err());
    common::walked(&mut p, NodeKind::Analysis);
    let outcome = common::opened(&mut p, NodeKind::Outcome);
    let report = common::full_report(
        p.current_cycle().rules(),
        CycleKind::Experiment,
        NodeKind::Outcome,
    )
    .with("verdict", "success")
    .with("next_direction.action", "close_cycle");
    common::close_here(&mut p, report).unwrap();
    p.close_cycle(common::cycle_report(
        p.current_cycle(),
        "success",
        common::step_id(&p, outcome),
    ))
    .unwrap();
    let file = path("visual-store-cycle");
    gil::save(&p, &file).unwrap();
    let mut loaded = gil::load(common::spec(), &file).unwrap();
    assert_eq!(
        loaded
            .visual_state()
            .unwrap()
            .registry()
            .report(owner(2, 3)),
        Some(&not_needed())
    );
    assert_eq!(
        loaded
            .visual_state()
            .unwrap()
            .registry()
            .report(ReportOwner::Cycle(CycleRef::new(2).unwrap())),
        Some(&ReportVisuals::References(vec![]))
    );
    loaded.open_child_cycle(CycleKind::Experiment).unwrap();
}
