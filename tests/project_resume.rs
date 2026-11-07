//! Filesystem/transaction acceptance; not actual Host instruction loading or model adherence.
mod common;
use gil::{ProjectSession, RuleSet, WorldState, STATE_PATH};
use std::{fs, path::{Path, PathBuf}, process::{Command, Stdio}, io::Write};
struct Rig { base: PathBuf, root: PathBuf, home: PathBuf }
impl Rig {
    fn new(label: &str) -> Self {
        let base=std::env::temp_dir().join(format!("gil-resume-integration-{label}-{}",std::process::id()));
        let root=base.join("project");let home=base.join("home");
        fs::create_dir_all(&root).unwrap();fs::create_dir_all(&home).unwrap();
        Self{base:base.canonicalize().unwrap(),root:root.canonicalize().unwrap(),home:home.canonicalize().unwrap()}
    }
    fn run(&self,args:&[&str],input:&str)->std::process::Output {
        let mut p=Command::new(env!("CARGO_BIN_EXE_gil")).args(args).current_dir(&self.root).env("HOME",&self.home)
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        p.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();p.wait_with_output().unwrap()
    }
    fn ok(&self,args:&[&str],input:&str)->String {
        let o=self.run(args,input);assert!(o.status.success(),"{}",String::from_utf8_lossy(&o.stderr));String::from_utf8(o.stdout).unwrap()
    }
}
impl Drop for Rig {fn drop(&mut self){let _=fs::remove_dir_all(&self.base);}}
fn files(at:&Path)->std::collections::BTreeMap<PathBuf,Vec<u8>> {
    fn walk(base:&Path,at:&Path,out:&mut std::collections::BTreeMap<PathBuf,Vec<u8>>) {
        for e in fs::read_dir(at).unwrap(){let p=e.unwrap().path();if p.is_dir(){walk(base,&p,out)}else{out.insert(p.strip_prefix(base).unwrap().into(),fs::read(p).unwrap());}}
    }
    let mut out=Default::default();walk(at,at,&mut out);out
}
#[test]
fn new_project_guides_are_in_the_first_snapshot_and_reconnect_never_rewrites_them() {
    let r=Rig::new("new");fs::write(r.root.join("CLAUDE.md"),b"Keep custom instructions\r\n").unwrap();
    assert!(r.ok(&["start"],"").contains("프로젝트 재개 안내: ready"));
    let session=ProjectSession::open(RuleSet::builtin().unwrap(),r.root.join(STATE_PATH)).unwrap();
    assert!(matches!(session.world_state().unwrap(),WorldState::Clean{..}));drop(session);
    assert!(fs::read(r.root.join("CLAUDE.md")).unwrap().starts_with(b"Keep custom instructions\r\n"));
    assert!(r.root.join("AGENTS.md").is_file());
    let before=files(&r.root);
    for _ in 0..2 {assert!(r.ok(&["context"],"").contains("프로젝트 재개 안내: ready"));}
    assert!(!r.run(&["start"],"").status.success());assert_eq!(files(&r.root),before);
    assert_eq!(fs::read_dir(&r.home).unwrap().count(),0);
    assert!(!r.base.join("AGENTS.md").exists());
}
#[test]
fn existing_project_reads_refusals_and_lock_contention_leave_guides_and_history_untouched() {
    let r=Rig::new("legacy");fs::write(r.root.join("CLAUDE.md"),b"custom").unwrap();
    ProjectSession::start(RuleSet::builtin().unwrap(),r.root.join(STATE_PATH)).unwrap().commit().unwrap();
    let before=files(&r.root);
    assert!(r.ok(&["context"],"").contains("pending/review_required"));
    assert!(!r.run(&["open","verify"],"objective: charts\nnext_action: plot\ndone_when: saved\n").status.success());
    let held=ProjectSession::open(RuleSet::builtin().unwrap(),r.root.join(STATE_PATH)).unwrap();
    assert!(!r.run(&["open","question"],"objective: ask\nnext_action: ask\ndone_when: answered\n").status.success());drop(held);
    assert_eq!(files(&r.root),before);assert!(!r.root.join("AGENTS.md").exists());
}
#[test]
fn plain_project_context_does_not_enroll_or_create_files() {
    let r=Rig::new("plain");fs::write(r.root.join("report.md"),"ordinary report").unwrap();
    let before=files(&r.root);assert!(!r.run(&["context"],"").status.success());
    assert_eq!(files(&r.root),before);assert_eq!(fs::read_dir(&r.home).unwrap().count(),0);
}
#[cfg(not(feature="approved-plan"))]
#[test]
fn legacy_guides_only_change_after_verify_is_committed_and_old_snapshots_survive() {
    use gil::{NodeKind,CycleKind};
    let r=Rig::new("verify");fs::write(r.root.join("CLAUDE.md"),b"old rules").unwrap();
    let mut s=ProjectSession::start(RuleSet::builtin().unwrap(),r.root.join(STATE_PATH)).unwrap();
    *s.project_mut()=common::bootstrap_from(s.project().clone());
    for k in [NodeKind::Define,NodeKind::Hypothesis] {common::walked(s.project_mut(),k);}
    s.commit().unwrap();let old=s.project().world_manifest(common::snapshot(1)).unwrap().clone();drop(s);
    let objects=files(&r.root.join(".gil/artifacts"));
    let out=r.ok(&["open","verify"],"objective: add a real chart\nnext_action: produce and check chart\ndone_when: chart checked\n");
    assert!(out.contains("added; original bytes preserved"));
    assert!(fs::read(r.root.join("CLAUDE.md")).unwrap().starts_with(b"old rules"));
    assert_eq!(files(&r.root.join(".gil/artifacts")),objects);
    let mut s=ProjectSession::open(RuleSet::builtin().unwrap(),r.root.join(STATE_PATH)).unwrap();
    assert!(s.can_confirm_artifact());assert_eq!(s.project().world_manifest(common::snapshot(1)),Some(&old));
    let report=common::evidence_report(&r.root,common::full_report(s.project().cycles().current().rules(),CycleKind::Experiment,NodeKind::Verify));
    s.close_step(report).unwrap();s.commit().unwrap();
    assert_eq!(s.project().world_manifest(common::snapshot(1)),Some(&old));
    assert!(matches!(s.world_state().unwrap(),WorldState::Clean{..}));drop(s);
    let now=files(&r.root.join(".gil/artifacts"));for (k,v) in objects {assert_eq!(now.get(&k),Some(&v));}
}
