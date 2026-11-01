//! Historical read-model compatibility and fresh-project approval are different contracts.
//! All records here are synthetic; no installed project is converted or migrated.
mod common;
use common::{legacy::{self, Layout}, spec};
use gil::{CycleKind, NodeKind, Project};

#[test]
fn historical_context_read_and_resume_preserve_format_and_absent_approval_ledger() {
    let mut layouts = vec![Layout::Cycles];
    if cfg!(feature = "chain-core") { layouts.push(Layout::Chain); }
    for layout in layouts {
        let root = common::scratch(&format!("context-profile-{layout:?}-{}", std::process::id()));
        let path = legacy::record(&root, layout);
        let before = std::fs::read(&path).unwrap();
        let mut project = gil::load(spec(), &path).unwrap();
        let context = gil::context(&project);
        assert!(!context.contains("승인 계획 프로필") && !context.contains("gil_plan_propose"), "{context}");
        assert_eq!(context.contains("현재 Chain:"), matches!(layout, Layout::Chain), "{context}");
        assert_eq!(std::fs::read(&path).unwrap(), before);

        // Continuing a historical record does not silently enroll it in approved-plan.
        common::walked(&mut project, NodeKind::Question);
        gil::save(&project, &path).unwrap();
        let resumed = gil::load(spec(), &path).unwrap();
        legacy::assert_unplanned(&resumed, layout);
        assert!(gil::context(&resumed).contains("gil_open(kind: interpretation)"));
        let stored: serde_norway::Value = serde_norway::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let expected = match layout { Layout::Cycles => gil::FORMAT, Layout::Chain => gil::CHAIN_FORMAT };
        assert_eq!(stored["format"].as_u64(), Some(expected.into()));
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn fresh_projects_keep_their_real_approval_contract_while_context_is_read_only() {
    let root = common::scratch(&format!("context-fresh-profile-{}", std::process::id()));
    let path = root.join(gil::STATE_PATH);
    let mut project = Project::start(spec(), common::first_world());
    let mut basis = Vec::new();
    for kind in [NodeKind::Question, NodeKind::Interpretation] {
        let id = common::walked(&mut project, kind);
        basis.push(id.to_string());
    }
    common::opened(&mut project, NodeKind::Synthesis);
    gil::save(&project, &path).unwrap();
    let before = std::fs::read(&path).unwrap();
    let context = gil::context(&project);
    assert_eq!(project.approved_plan().is_some(), cfg!(feature = "approved-plan"));
    assert_eq!(context.contains("승인 계획 프로필"), cfg!(feature = "approved-plan"), "{context}");
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let result = project.close_action_step(common::full_report(&spec(), CycleKind::Interview, NodeKind::Synthesis)
        .with(common::APPROVED, common::APPROVAL).with(common::BASIS_REFS, basis.join("\n")));
    if cfg!(feature = "approved-plan") {
        let error = result.expect_err("a bare yes must not create an approval").to_string();
        assert!(error.contains("gil_plan_propose"), "{error}");
        gil::save(&project, &path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), before, "refusal must preserve the record");
    } else {
        result.expect("plain Synthesis approval remains valid outside approved-plan");
        assert!(gil::context(&project).contains("gil_open(kind: outcome)"));
    }
    std::fs::remove_dir_all(root).unwrap();
}
