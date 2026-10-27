//! Real filesystem transactions: evidence and learning must survive reopening; refusals close nothing.
mod common;
use gil::{CycleKind, NodeKind, ProjectSession, Report};
use std::{fs, path::PathBuf};

fn setup(name: &str) -> (PathBuf, ProjectSession) {
    let root = common::scratch(name);
    fs::write(root.join("old.txt"), "existing source").unwrap();
    let mut s = ProjectSession::start(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    *s.project_mut() = common::bootstrap_from(s.project().clone());
    common::up_to_verify(s.project_mut());
    s.commit().unwrap();
    (root, s)
}
fn report(paths: &str) -> Report {
    common::full_report(&common::spec(), CycleKind::Experiment, NodeKind::Verify)
        .with("execution", "가상 입력을 검사했다")
        .with("result", "검사 결과를 파일로 남겼다")
        .with("evidence_files", paths)
}

#[test]
fn missing_empty_unchanged_and_unsafe_evidence_never_close_or_advance_journey() {
    let (root, mut s) = setup("evidence-rejections");
    fs::write(root.join("empty.txt"), "").unwrap();
    fs::create_dir(root.join("directory")).unwrap();
    let saved = fs::read(root.join(gil::STATE_PATH)).unwrap();
    let memory = format!("{:?}", s.project());
    for paths in ["", "missing.md", "empty.txt", "old.txt", "directory", "../outside.txt",
        "/absolute", "C:/outside", ".gil/state.yaml", "old.txt\nold.txt"] {
        assert!(s.close_step(report(paths)).is_err(), "accepted {paths}");
        assert_eq!(format!("{:?}", s.project()), memory);
        assert_eq!(fs::read(root.join(gil::STATE_PATH)).unwrap(), saved);
    }
    // An unrelated changed file cannot validate a citation of an unchanged source.
    fs::write(root.join("unrelated.txt"), "changed").unwrap();
    assert!(s.close_step(report("old.txt")).is_err());
    fs::write(root.join("evidence.md"), "Observation: synthetic test failed; expected 2, got 3.").unwrap();
    assert!(s.close_step(report("evidence.md\nmissing.md")).is_err());
    s.close_step(report("evidence.md")).unwrap();
    s.commit().unwrap();
    assert_ne!(fs::read(root.join(gil::STATE_PATH)).unwrap(), saved);
}

#[test]
fn changed_evidence_snapshot_and_learning_links_survive_reload() {
    let (root, mut s) = setup("evidence-learning");
    fs::write(root.join("old.txt"), "Observed new result: 20 percent, source SAMPLE.").unwrap();
    s.close_step(report("old.txt")).unwrap();
    s.commit().unwrap();
    let cycle = s.project().current_cycle();
    let verify = cycle.step_ref(cycle.steps().current().unwrap()).to_string();
    let world = s.project().world_snapshot();
    common::opened(s.project_mut(), NodeKind::Analysis);
    let analysis = common::full_report(&common::spec(), CycleKind::Experiment, NodeKind::Analysis);
    assert!(s.close_step(analysis.clone().with("interpretation", " ")).is_err());
    assert!(s.close_step(analysis.clone().with("evidence.verify_ref", "step:C999/S3")).is_err());
    s.close_step(analysis).unwrap();
    let cycle = s.project().current_cycle();
    let analysis = cycle.step_ref(cycle.steps().current().unwrap()).to_string();
    common::opened(s.project_mut(), NodeKind::Outcome);
    let outcome = common::full_report(&common::spec(), CycleKind::Experiment, NodeKind::Outcome)
        .with("verdict", "success").with(common::ACTION, "close_cycle");
    assert!(s.close_step(outcome.clone().with("lesson", "")).is_err());
    assert!(s.close_step(outcome.clone().with("learning.analysis_ref", "step:C999/S4")).is_err());
    s.close_step(outcome.with("lesson", "동일 기준의 자료를 비교해야 한다")).unwrap();
    s.commit().unwrap(); drop(s);
    let p = gil::load(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    // Working-file deletion cannot erase the captured evidence or its learning lineage.
    let durable = fs::read(root.join(gil::STATE_PATH)).unwrap();
    fs::remove_file(root.join("old.txt")).unwrap();
    let session = ProjectSession::open(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    session.restore().unwrap();
    assert_eq!(fs::read_to_string(root.join("old.txt")).unwrap(), "Observed new result: 20 percent, source SAMPLE.");
    assert_eq!(fs::read(root.join(gil::STATE_PATH)).unwrap(), durable);
    let nodes = p.current_cycle().steps().nodes();
    let v = nodes.iter().find(|n| n.kind == NodeKind::Verify).unwrap();
    assert_eq!(v.snapshot, Some(world));
    assert_eq!(v.report.as_ref().unwrap().get("evidence_files"), Some("old.txt"));
    let a = nodes.iter().find(|n| n.kind == NodeKind::Analysis).unwrap();
    assert_eq!(a.report.as_ref().unwrap().get("evidence.verify_ref"), Some(verify.as_str()));
    let o = nodes.iter().find(|n| n.kind == NodeKind::Outcome).unwrap();
    assert_eq!(o.report.as_ref().unwrap().get("learning.analysis_ref"), Some(analysis.as_str()));
    assert_eq!(o.journey, Some(p.current_existence().current_journey()));
    assert_ne!(v.journey, a.journey);
    assert_ne!(a.journey, o.journey);
}

#[test]
fn historical_verify_without_evidence_stays_readable() {
    let (root, mut s) = setup("evidence-legacy");
    // Explicit pre-policy fixture through the low-level domain API, not a current write path.
    let world = s.project().world_manifest(s.project().world_snapshot()).unwrap().clone();
    let legacy = common::full_report(&common::spec(), CycleKind::Experiment, NodeKind::Verify);
    s.project_mut().close_verify_step(legacy, world).unwrap();
    s.commit().unwrap(); drop(s);
    let before = fs::read(root.join(gil::STATE_PATH)).unwrap();
    let s = ProjectSession::open_read_only(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    assert!(s.project().current_cycle().steps().nodes().iter().any(|n| n.kind == NodeKind::Verify && n.is_closed()));
    assert_eq!(before, fs::read(root.join(gil::STATE_PATH)).unwrap());
}

#[test]
fn raw_cli_cannot_close_chat_only_verify_and_can_recover_with_a_real_document() {
    use std::{io::Write, process::{Command, Stdio}};
    let (root, s) = setup("evidence-cli"); drop(s);
    let before = fs::read(root.join(gil::STATE_PATH)).unwrap();
    let close = |body: &str| {
        let mut child = Command::new(env!("CARGO_BIN_EXE_gil")).arg("close").current_dir(&root)
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        child.stdin.take().unwrap().write_all(body.as_bytes()).unwrap();
        child.wait_with_output().unwrap()
    };
    let refused = close("execution: inspected synthetic numbers\nresult: two\n");
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("verify_evidence_required"));
    assert_eq!(fs::read(root.join(gil::STATE_PATH)).unwrap(), before);
    fs::write(root.join("observed.txt"), "Synthetic input 1+1, observed 2.").unwrap();
    let accepted = close("execution: inspected synthetic numbers\nresult: two\nevidence_files: observed.txt\n");
    assert!(accepted.status.success(), "{}", String::from_utf8_lossy(&accepted.stderr));
    let s = ProjectSession::open_read_only(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    assert!(s.project().current_cycle().step_now_open().is_none());
}
