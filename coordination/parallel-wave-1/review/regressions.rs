//! Review-only regression probes. Compile as a crate unit-test module; not production code.
use crate::artifact::{EntryPath, Manifest, ManifestEntry};
use crate::restore::preview::{self, LstatProbe, Readiness, Request, TargetOs};

fn manifest(path: &str) -> Manifest {
    Manifest::new(vec![ManifestEntry {
        path: EntryPath::parse(path).unwrap(),
        content: crate::artifact::digest_from_raw([1; 32]),
    }])
}

#[test]
#[cfg(unix)]
fn review_preview_refuses_symlink_root() {
    let temp = std::env::temp_dir().join(format!("gil-review-root-{}", std::process::id()));
    std::fs::create_dir_all(temp.join("outside")).unwrap();
    std::os::unix::fs::symlink(temp.join("outside"), temp.join("project")).unwrap();
    let before = Manifest::new(vec![]);
    let target = manifest("new.txt");
    let value = preview::preview(&Request { before: &before, target: &target,
        before_policy: Some("policy"), target_policy: Some("policy"), os: TargetOs::host() },
        |_| Ok(true), &LstatProbe::at(&temp.join("project")));
    std::fs::remove_dir_all(&temp).unwrap();
    assert!(matches!(value.readiness(), Readiness::Refused(_)), "symlink root was accepted: {value}");
}

#[test]
#[cfg(target_os = "macos")]
fn review_preview_refuses_existing_normalized_alias() {
    let temp = std::env::temp_dir().join(format!("gil-review-normalization-{}", std::process::id()));
    std::fs::create_dir_all(&temp).unwrap();
    let nfd = "\u{1100}\u{1161}.txt";
    let nfc = "\u{ac00}.txt";
    std::fs::write(temp.join(nfd), b"untracked local file").unwrap();
    assert!(temp.join(nfc).exists(), "probe requires a normalization-insensitive volume");
    let before = Manifest::new(vec![]);
    let target = manifest(nfc);
    let value = preview::preview(&Request { before: &before, target: &target,
        before_policy: Some("policy"), target_policy: Some("policy"), os: TargetOs::MacOs },
        |_| Ok(true), &LstatProbe::at(&temp));
    std::fs::remove_dir_all(&temp).unwrap();
    assert!(matches!(value.readiness(), Readiness::Refused(_)), "existing normalization alias was accepted: {value}");
}

#[cfg(feature = "approved-plan")]
fn planned_experiment() -> crate::Project {
    use crate::{CycleKind, NodeKind};
    use crate::plan_test_helpers as h;
    let mut p = crate::Project::start(h::spec(), h::first_world());
    let draft = crate::approved_plan::tests::synthesis(&mut p, &["A", "B"]);
    let proposal = p.propose_plan(draft).unwrap();
    let receipt = p.decide_plan(proposal.revision, &proposal.digest, crate::approved_plan::Decision::Approve).unwrap();
    let synthesis = receipt.synthesis_ref.clone();
    p.close_action_step(receipt.report().unwrap()).unwrap();
    let outcome = h::opened(&mut p, NodeKind::Outcome);
    p.close_action_step(h::full_report(p.current_cycle().rules(), CycleKind::Interview, NodeKind::Outcome)
        .with("synthesis_ref", synthesis)).unwrap();
    p.close_cycle(h::cycle_report(p.current_cycle(), "success", h::step_id(&p, outcome))
        .with("next_direction.action", "open_child")).unwrap();
    p.open_child_cycle(CycleKind::Experiment).unwrap();
    p
}

#[test]
#[cfg(feature = "approved-plan")]
fn review_reinterview_between_steps_names_a_legal_next_step() {
    use crate::plan_test_helpers as h;
    let mut p = planned_experiment();
    h::opened(&mut p, crate::NodeKind::Define);
    p.request_plan_interview("Change scope after this experiment".into()).unwrap();
    p.close_action_step(h::full_report(p.current_cycle().rules(), crate::CycleKind::Experiment, crate::NodeKind::Define)).unwrap();
    assert!(!p.current_cycle().can_close());
    assert_eq!(p.current_cycle().openable_here(), vec![crate::NodeKind::Hypothesis]);
    let hint = crate::context::next_call(&p);
    assert!(hint.contains("gil_open(kind: hypothesis)"), "the proposed cycle close is not executable: {hint}");
}

#[test]
#[cfg(feature = "approved-plan")]
fn review_closed_cycle_still_has_current_item_binding() {
    use crate::plan_test_helpers as h;
    let mut p = planned_experiment();
    for kind in [crate::NodeKind::Define, crate::NodeKind::Hypothesis, crate::NodeKind::Verify,
        crate::NodeKind::Analysis, crate::NodeKind::Outcome] { h::walked(&mut p, kind); }
    let outcome = p.current_cycle().steps().current().unwrap();
    p.close_cycle(h::cycle_report(p.current_cycle(), "success", outcome)
        .with("next_direction.action", "open_child")).unwrap();
    assert!(p.current_cycle().is_closed());
    assert_eq!(p.current_plan_item().unwrap().id, "A");
    println!("closed cycle retains current_item=A: this binding is not an open-state witness");
}
