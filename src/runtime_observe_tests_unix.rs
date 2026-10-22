//! Unix fixture tests for the read-only observer. Fixtures are built here (separately from the
//! observer) in a fresh isolated temp tree with the exact modes the contract requires; the
//! observer only reads. A snapshot of names, modes, link counts, owners, sizes and contents is
//! compared before and after each observation (access times are not part of the claim).
use super::hooks;
use super::*;
use crate::runtime_state::{Run, State};
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::PathBuf;

const V1: &str = "0.2.1-dev.a";
fn sha(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
fn core_bytes() -> Vec<u8> { b"#!/bin/sh\necho gil-fixture\n".to_vec() }
fn nanos() -> u128 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() }
fn is_root() -> bool { (unsafe { libc::geteuid() }) == 0 }
fn euid() -> u32 { unsafe { libc::geteuid() } }

struct Fixture { base: PathBuf, root: PathBuf }
impl Fixture {
    fn new(label: &str) -> Self { Self::under(label, "") }
    /// Root at `<base>/<sub>/runtime-v1`; `sub` may be empty or one directory name.
    fn under(label: &str, sub: &str) -> Self {
        // canonicalized: the observer refuses symlinked ancestors (macOS /var -> /private/var)
        let base = fs::canonicalize(std::env::temp_dir()).unwrap().join(format!("gil-observe-{label}-{}-{}", std::process::id(), nanos()));
        fs::create_dir_all(&base).unwrap();
        fs::set_permissions(&base, fs::Permissions::from_mode(0o700)).unwrap();
        let parent = if sub.is_empty() { base.clone() } else { let p = base.join(sub); fs::create_dir(&p).unwrap(); p };
        Self { root: parent.join("runtime-v1"), base }
    }
    fn root_dir(&self) -> &Self { fs::create_dir(&self.root).unwrap(); fs::set_permissions(&self.root, fs::Permissions::from_mode(0o700)).unwrap(); self }
    fn write(&self, rel: &str, bytes: &[u8], mode: u32) -> PathBuf {
        let p = self.root.join(rel);
        if let Some(d) = p.parent() { fs::create_dir_all(d).unwrap(); }
        if p.exists() { fs::remove_file(&p).unwrap(); }
        fs::write(&p, bytes).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(mode)).unwrap();
        p
    }
    fn store(&self, phase: &str, last_generation: u64) -> &Self { self.write("store.json", store_json(phase, last_generation).as_bytes(), 0o600); self }
    fn intent(&self, generation: u64, version: &str, sha256: &str) -> &Self { self.write("intent.json", format!(r#"{{"schema":"{INTENT_SCHEMA}","generation":{generation},"version":"{version}","sha256":"{sha256}"}}"#).as_bytes(), 0o600); self }
    fn selection(&self, generation: u64, version: &str, sha256: &str) -> &Self { self.write("selection.json", format!(r#"{{"schema":"{SELECTION_SCHEMA}","generation":{generation},"version":"{version}","sha256":"{sha256}"}}"#).as_bytes(), 0o600); self }
    fn version(&self, version: &str, core: &[u8]) -> String {
        let digest = sha(core);
        let dir = format!("versions/{version}-{}", &digest[..12]);
        fs::create_dir_all(self.root.join("versions")).unwrap();
        fs::set_permissions(self.root.join("versions"), fs::Permissions::from_mode(0o700)).unwrap();
        self.write(&format!("{dir}/{CORE_FILE}"), core, 0o500);
        self.write(&format!("{dir}/complete"), digest.as_bytes(), 0o600);
        fs::set_permissions(self.root.join(&dir), fs::Permissions::from_mode(0o700)).unwrap();
        digest
    }
    fn version_dir(&self, digest: &str) -> PathBuf { self.root.join(format!("versions/{V1}-{}", &digest[..12])) }
    fn snapshot(&self) -> BTreeMap<String, (u32, u64, u32, u64, String)> {
        fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, (u32, u64, u32, u64, String)>) {
            let Ok(rd) = fs::read_dir(dir) else { return };
            for e in rd {
                let p = e.unwrap().path();
                let m = fs::symlink_metadata(&p).unwrap();
                let content = if m.is_file() { sha(&fs::read(&p).unwrap_or_default()) } else { String::new() };
                out.insert(p.strip_prefix(base).unwrap().display().to_string(), (m.mode() & 0o7777, m.nlink(), m.uid(), m.size(), content));
                if m.is_dir() { walk(base, &p, out); }
            }
        }
        let mut out = BTreeMap::new();
        if self.root.exists() { walk(&self.base, &self.root, &mut out); }
        out
    }
}
fn store_json(phase: &str, last_generation: u64) -> String {
    format!(r#"{{"schema":"{STORE_SCHEMA}","phase":"{phase}","last_generation":{last_generation},"owner":"uid:{}","created":"t"}}"#, euid())
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fn relax(p: &Path) { if let Ok(rd) = fs::read_dir(p) { for e in rd.flatten() { let q = e.path(); if fs::symlink_metadata(&q).map(|m| m.file_type().is_symlink()).unwrap_or(true) { continue; } let _ = fs::set_permissions(&q, fs::Permissions::from_mode(0o700)); if q.is_dir() { relax(&q); } } } }
        relax(&self.base);
        let _ = fs::remove_dir_all(&self.base);
    }
}

/// Observe with the invariance check around it.
fn observe_checked(fx: &Fixture) -> (Observed, crate::runtime_state::Verdict) {
    let before = fx.snapshot();
    let out = observe_and_judge(&fx.root);
    assert_eq!(fx.snapshot(), before, "observation must not change names, modes, links, owners, sizes or contents");
    out
}
fn blocked_with(o: &Observed, needle: &str) -> bool { o.observation.blocked.iter().any(|b| b.contains(needle)) }
fn checked(o: &Observed, needle: &str) -> bool { o.checks.iter().any(|c| c.contains(needle)) }

#[test]
fn first_use_only_when_the_root_is_not_found() {
    let fx = Fixture::new("first");
    let (o, v) = observe_checked(&fx);
    assert!(!o.observation.root_exists && o.observation.blocked.is_empty(), "{o:?}");
    assert_eq!(v.state, State::FirstUse); assert_eq!(v.run, Run::Bundled);
    assert!(checked(&o, "root not found"));
}
#[test]
fn prepared_and_selected_and_history_lagging_from_real_files() {
    let fx = Fixture::new("normal"); fx.root_dir().store("prepared", 0);
    let (o, v) = observe_checked(&fx);
    assert_eq!(v.state, State::Prepared, "{o:?}");
    let d = fx.version(V1, &core_bytes());
    fx.store("prepared", 1).selection(1, V1, &d);
    let (o, v) = observe_checked(&fx);
    assert_eq!(v.state, State::Selected { generation: 1, version: V1.into(), history_lagging: false }, "{o:?}");
    for needle in ["path walk", "root protection", "store.json protection", "selection.json protection", "versions/ protection", "complete protection", "gil protection", "digest equals selection", "re-check"] {
        assert!(checked(&o, needle), "missing check {needle}: {:?}", o.checks);
    }
    fx.store("prepared", 0).intent(1, V1, &d);
    let (_, v) = observe_checked(&fx);
    assert_eq!(v.state, State::Selected { generation: 1, version: V1.into(), history_lagging: true });
    fs::remove_file(fx.root.join("selection.json")).unwrap();
    let (_, v) = observe_checked(&fx);
    assert!(matches!(v.state, State::Damaged { .. })); assert_eq!(v.run, Run::None);
}
#[test]
fn damaged_cases_are_damaged_not_blocked() {
    let fx = Fixture::new("damaged"); fx.root_dir();
    let (o, v) = observe_checked(&fx);
    assert!(matches!(v.state, State::Damaged { .. }) && o.observation.blocked.is_empty(), "{o:?}");
    fx.write("store.json", b"{ not json", 0o600);
    let (o, v) = observe_checked(&fx);
    assert_eq!(o.observation.store, Record::Invalid); assert!(matches!(v.state, State::Damaged { .. }));
    fx.write("store.json", br#"{"schema":"other/1","phase":"prepared","last_generation":0,"owner":"uid:0"}"#, 0o600);
    assert_eq!(observe(&fx.root).observation.store, Record::Invalid);
    fx.write("store.json", format!(r#"{{"schema":"{STORE_SCHEMA}","phase":"prepared","last_generation":0,"owner":"uid:999999"}}"#).as_bytes(), 0o600);
    assert_eq!(observe(&fx.root).observation.store, Record::Invalid);
    fx.store("prepared", 1);
    let big = format!(r#"{{"schema":"{SELECTION_SCHEMA}","generation":1,"version":"v","sha256":"{}","pad":"{}"}}"#, "0".repeat(64), "x".repeat(MAX_RECORD_BYTES as usize));
    fx.write("selection.json", big.as_bytes(), 0o600);
    let (o, v) = observe_checked(&fx);
    assert_eq!(o.observation.selection, Record::Invalid, "{o:?}"); assert!(matches!(v.state, State::Damaged { .. }));
    let d = fx.version(V1, &core_bytes());
    fx.write(&format!("versions/{V1}-{}/{CORE_FILE}", &d[..12]), b"tampered", 0o500);
    fx.selection(1, V1, &d);
    let (o, v) = observe_checked(&fx);
    assert_eq!(o.observation.selection, Record::Valid(Selection { generation: 1, version: V1.into(), sha256: d.clone(), version_complete: false }));
    assert!(matches!(v.state, State::Damaged { .. }) && o.observation.blocked.is_empty(), "{o:?}");
    fx.selection(1, V1, &sha(b"other"));
    let (o, _) = observe_checked(&fx);
    assert!(matches!(o.observation.selection, Record::Valid(Selection { version_complete: false, .. })));
    // complete missing → incomplete, not blocked
    fx.write(&format!("versions/{V1}-{}/{CORE_FILE}", &d[..12]), &core_bytes(), 0o500);
    fs::remove_file(fx.version_dir(&d).join("complete")).unwrap();
    fx.selection(1, V1, &d);
    let (o, _) = observe_checked(&fx);
    assert!(matches!(o.observation.selection, Record::Valid(Selection { version_complete: false, .. })) && o.observation.blocked.is_empty(), "{o:?}");
    assert!(o.observation.traces.iter().any(|t| t.contains("complete absent")));
}
#[test]
fn path_traversal_and_bad_identifiers_are_invalid_records() {
    let fx = Fixture::new("traversal"); fx.root_dir().store("prepared", 1);
    for version in ["../x", "a/b", "..", " v", ".hidden"] {
        let sha = "a".repeat(64);
        fx.write("selection.json", format!(r#"{{"schema":"{SELECTION_SCHEMA}","generation":1,"version":"{version}","sha256":"{sha}"}}"#).as_bytes(), 0o600);
        let o = observe(&fx.root);
        assert_eq!(o.observation.selection, Record::Invalid, "{version:?}: {o:?}");
    }
    for sha in ["ABCDEF".repeat(11), "0".repeat(63), "zz".repeat(32)] {
        fx.write("selection.json", format!(r#"{{"schema":"{SELECTION_SCHEMA}","generation":1,"version":"v","sha256":"{sha}"}}"#).as_bytes(), 0o600);
        assert_eq!(observe(&fx.root).observation.selection, Record::Invalid);
    }
    assert!(blocked_with(&observe(Path::new("relative/root")), "refused"));
    assert!(blocked_with(&observe(&fx.root.join("..").join("runtime-v1")), "refused"));
}
#[test]
fn links_are_refused_as_blocked() {
    let fx = Fixture::new("links"); fx.root_dir().store("prepared", 1);
    let d = fx.version(V1, &core_bytes());
    fx.selection(1, V1, &d);
    let real = fx.base.join("elsewhere.json"); fs::copy(fx.root.join("selection.json"), &real).unwrap();
    fs::remove_file(fx.root.join("selection.json")).unwrap();
    std::os::unix::fs::symlink(&real, fx.root.join("selection.json")).unwrap();
    let (o, v) = observe_checked(&fx);
    assert!(o.observation.blocked.iter().any(|b| b.starts_with("selection.json")), "{o:?}");
    assert!(matches!(v.state, State::Blocked { .. }));
    fs::remove_file(fx.root.join("selection.json")).unwrap(); fx.selection(1, V1, &d);
    fs::hard_link(fx.root.join("selection.json"), fx.base.join("hard.json")).unwrap();
    let o = observe(&fx.root);
    assert!(blocked_with(&o, "selection.json protection") && blocked_with(&o, "link count"), "{o:?}");
    fs::remove_file(fx.base.join("hard.json")).unwrap();
    // hard-linked complete marker
    fs::hard_link(fx.version_dir(&d).join("complete"), fx.base.join("complete-hard")).unwrap();
    let o = observe(&fx.root);
    assert!(blocked_with(&o, "complete protection") && blocked_with(&o, "link count") && !checked(&o, "digest equals"), "{o:?}");
    fs::remove_file(fx.base.join("complete-hard")).unwrap();
    // symlinked version directory
    let vdir = fx.version_dir(&d);
    let moved = fx.base.join("vdir"); fs::rename(&vdir, &moved).unwrap();
    std::os::unix::fs::symlink(&moved, &vdir).unwrap();
    let o = observe(&fx.root);
    assert!(blocked_with(&o, "versions/") && !checked(&o, "digest equals"), "{o:?}");
    fs::remove_file(&vdir).unwrap(); fs::rename(&moved, &vdir).unwrap();
    // symlinked root
    let link_root = fx.base.join("link-root"); std::os::unix::fs::symlink(&fx.root, &link_root).unwrap();
    let o = observe(&link_root);
    assert!(blocked_with(&o, "path walk") && blocked_with(&o, "symlink"), "{o:?}");
    assert!(observe(&fx.root).observation.blocked.is_empty());
}
#[test]
fn protection_failures_and_access_denial_are_blocked_not_absent() {
    if is_root() { return; }
    let fx = Fixture::new("protect"); fx.root_dir().store("prepared", 0);
    fs::set_permissions(fx.root.join("store.json"), fs::Permissions::from_mode(0o644)).unwrap();
    let (o, v) = observe_checked(&fx);
    assert!(blocked_with(&o, "store.json protection") && blocked_with(&o, "mode 644") && blocked_with(&o, "(not read)"), "{o:?}");
    assert!(matches!(v.state, State::Blocked { .. })); assert_eq!(v.run, Run::None);
    fs::set_permissions(fx.root.join("store.json"), fs::Permissions::from_mode(0o600)).unwrap();
    // group-writable root: nothing below is read
    fs::set_permissions(&fx.root, fs::Permissions::from_mode(0o720)).unwrap();
    let o = observe(&fx.root);
    assert!(blocked_with(&o, "root protection") && blocked_with(&o, "nothing below the root was read") && !checked(&o, "store.json"), "{o:?}");
    fs::set_permissions(&fx.root, fs::Permissions::from_mode(0o700)).unwrap();
    // mode 000 record: blocked, never absent or first use
    fs::set_permissions(fx.root.join("store.json"), fs::Permissions::from_mode(0o000)).unwrap();
    let o = observe(&fx.root);
    assert!(o.observation.root_exists && blocked_with(&o, "store.json"), "{o:?}");
    fs::set_permissions(fx.root.join("store.json"), fs::Permissions::from_mode(0o600)).unwrap();
    // mode 000 root: blocked at the walk, not first use
    fs::set_permissions(&fx.root, fs::Permissions::from_mode(0o000)).unwrap();
    let o = observe(&fx.root);
    assert!(blocked_with(&o, "path walk") && !o.observation.root_exists, "{o:?}");
    assert!(matches!(crate::runtime_state::judge(&o.observation).state, State::Blocked { .. }));
    fs::set_permissions(&fx.root, fs::Permissions::from_mode(0o700)).unwrap();
    // executable without the execute bit
    let d = fx.version(V1, &core_bytes()); fx.store("prepared", 1).selection(1, V1, &d);
    let core = fx.version_dir(&d).join(CORE_FILE);
    fs::set_permissions(&core, fs::Permissions::from_mode(0o400)).unwrap();
    let o = observe(&fx.root);
    assert!(blocked_with(&o, CORE_FILE) && blocked_with(&o, "mode 400") && blocked_with(&o, "(not hashed)"), "{o:?}");
    fs::set_permissions(&core, fs::Permissions::from_mode(0o500)).unwrap();
    // versions/ with the wrong mode: the candidate below is not read
    fs::set_permissions(fx.root.join("versions"), fs::Permissions::from_mode(0o755)).unwrap();
    let o = observe(&fx.root);
    assert!(blocked_with(&o, "versions/ protection") && blocked_with(&o, "nothing below was read") && !checked(&o, "complete") && !checked(&o, "digest equals"), "{o:?}");
    fs::set_permissions(fx.root.join("versions"), fs::Permissions::from_mode(0o700)).unwrap();
    // version directory with the wrong mode
    fs::set_permissions(fx.version_dir(&d), fs::Permissions::from_mode(0o750)).unwrap();
    let o = observe(&fx.root);
    assert!(blocked_with(&o, &format!("versions/{V1}")) && !checked(&o, "complete"), "{o:?}");
    fs::set_permissions(fx.version_dir(&d), fs::Permissions::from_mode(0o700)).unwrap();
    // complete marker with the wrong mode: Core not read
    fs::set_permissions(fx.version_dir(&d).join("complete"), fs::Permissions::from_mode(0o644)).unwrap();
    let o = observe(&fx.root);
    assert!(blocked_with(&o, "complete protection") && blocked_with(&o, "(Core not read)") && !checked(&o, "digest equals"), "{o:?}");
    fs::set_permissions(fx.version_dir(&d).join("complete"), fs::Permissions::from_mode(0o600)).unwrap();
    assert!(observe(&fx.root).observation.blocked.is_empty());
}
#[cfg(target_os = "macos")]
#[test]
fn an_extended_acl_is_a_protection_failure() {
    if is_root() { return; }
    let fx = Fixture::new("acl"); fx.root_dir().store("prepared", 1);
    let d = fx.version(V1, &core_bytes()); fx.selection(1, V1, &d);
    for (target, needle) in [(fx.root.join("store.json"), "store.json protection"), (fx.root.join("versions"), "versions/ protection"), (fx.version_dir(&d).join("complete"), "complete protection")] {
        let out = std::process::Command::new("/bin/chmod").args(["+a", "everyone allow read"]).arg(&target).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let o = observe(&fx.root);
        assert!(blocked_with(&o, needle) && blocked_with(&o, "extended ACL"), "{needle}: {o:?}");
        let out = std::process::Command::new("/bin/chmod").args(["-a", "everyone allow read"]).arg(&target).output().unwrap();
        assert!(out.status.success());
    }
    assert!(observe(&fx.root).observation.blocked.is_empty());
}
#[test]
fn a_change_during_observation_is_detected_and_blocks() {
    let fx = Fixture::new("race"); fx.root_dir().store("prepared", 1);
    let d = fx.version(V1, &core_bytes()); fx.selection(1, V1, &d);
    fn rewrite(root: &Path) { std::thread::sleep(std::time::Duration::from_millis(20)); fs::write(root.join("store.json"), store_json("prepared", 2)).unwrap(); }
    hooks::set(Some(rewrite));
    let o = observe(&fx.root);
    hooks::set(None);
    assert!(blocked_with(&o, "store.json changed during observation"), "{o:?}");
    assert!(matches!(crate::runtime_state::judge(&o.observation).state, State::Blocked { .. }));
    fn add(root: &Path) { fs::write(root.join("stray.json"), b"{}").unwrap(); }
    hooks::set(Some(add));
    let o = observe(&fx.root);
    hooks::set(None);
    assert!(blocked_with(&o, "listing changed"), "{o:?}");
    fs::remove_file(fx.root.join("stray.json")).unwrap();
    // a permission change after the read is detected (identity and protection re-check)
    fn relax(root: &Path) { fs::set_permissions(root.join("store.json"), fs::Permissions::from_mode(0o644)).unwrap(); }
    hooks::set(Some(relax));
    let o = observe(&fx.root);
    hooks::set(None);
    assert!(blocked_with(&o, "store.json changed during observation") && blocked_with(&o, "store.json protection changed during observation"), "{o:?}");
    fs::set_permissions(fx.root.join("store.json"), fs::Permissions::from_mode(0o600)).unwrap();
    // selection.json replaced before it is opened: root-entry identity recorded first catches it
    fn reselect(root: &Path) { let p = root.join("selection.json"); let bytes = fs::read(&p).unwrap(); fs::remove_file(&p).unwrap(); fs::write(&p, bytes).unwrap(); fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).unwrap(); }
    hooks::set(Some(reselect));
    let o = observe(&fx.root);
    hooks::set(None);
    assert!(blocked_with(&o, "selection.json changed during observation (root entry identity)"), "{o:?}");
    assert!(observe(&fx.root).observation.blocked.is_empty());
}
#[test]
fn an_intermediate_directory_swapped_during_observation_is_refused_without_reading_outside() {
    let fx = Fixture::under("swap-versions", "a"); fx.root_dir().store("prepared", 1);
    let d = fx.version(V1, &core_bytes()); fx.selection(1, V1, &d);
    // elsewhere: a tree whose Core is unreadable; if the observer followed the link it would
    // report a permission error naming it
    let elsewhere = fx.base.join("elsewhere");
    fs::create_dir_all(elsewhere.join(format!("{V1}-{}", &d[..12]))).unwrap();
    fs::write(elsewhere.join(format!("{V1}-{}/complete", &d[..12])), b"x").unwrap();
    fs::write(elsewhere.join(format!("{V1}-{}/{CORE_FILE}", &d[..12])), b"canary").unwrap();
    if !is_root() { fs::set_permissions(elsewhere.join(format!("{V1}-{}/{CORE_FILE}", &d[..12])), fs::Permissions::from_mode(0o000)).unwrap(); }
    fn swap(root: &Path) {
        let v = root.join("versions"); let moved = root.parent().unwrap().parent().unwrap().join("versions-moved");
        fs::rename(&v, &moved).unwrap();
        std::os::unix::fs::symlink(root.parent().unwrap().parent().unwrap().join("elsewhere"), &v).unwrap();
    }
    hooks::set(Some(swap));
    let o = observe(&fx.root);
    hooks::set(None);
    assert!(blocked_with(&o, "versions/") && blocked_with(&o, "versions changed during observation (root entry identity)"), "{o:?}");
    assert!(!o.observation.blocked.iter().any(|b| b.contains("denied") || b.contains("canary")), "read outside the tree: {o:?}");
    assert!(!checked(&o, "digest equals"));
    fs::remove_file(fx.root.join("versions")).unwrap();
    fs::rename(fx.base.join("versions-moved"), fx.root.join("versions")).unwrap();
    assert!(observe(&fx.root).observation.blocked.is_empty());
}
#[test]
fn an_ancestor_swapped_during_observation_keeps_reading_the_opened_tree_and_blocks() {
    // A at <base>/a/runtime-v1 (prepared, last_generation 7); B at <base>/b/runtime-v1.
    let fx = Fixture::under("swap-ancestor", "a"); fx.root_dir().store("prepared", 7);
    let b_root = fx.base.join("b").join("runtime-v1");
    fs::create_dir_all(&b_root).unwrap();
    fs::set_permissions(fx.base.join("b"), fs::Permissions::from_mode(0o700)).unwrap();
    fs::set_permissions(&b_root, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(b_root.join("store.json"), store_json("prepared", 9)).unwrap();
    fs::set_permissions(b_root.join("store.json"), fs::Permissions::from_mode(0o600)).unwrap();
    fs::write(b_root.join("intent.json"), b"not json: reading B would make intent Invalid").unwrap();
    fs::set_permissions(b_root.join("intent.json"), fs::Permissions::from_mode(0o600)).unwrap();
    // variant 1: <base>/a becomes a symlink to <base>/b
    fn to_symlink(root: &Path) {
        let a = root.parent().unwrap().to_path_buf(); let base = a.parent().unwrap().to_path_buf();
        fs::rename(&a, base.join("a-moved")).unwrap();
        std::os::unix::fs::symlink(base.join("b"), &a).unwrap();
    }
    hooks::set(Some(to_symlink));
    let o = observe(&fx.root);
    hooks::set(None);
    assert_eq!(o.observation.store, Record::Valid(Store { phase: Phase::Prepared, last_generation: 7 }), "{o:?}");
    assert_eq!(o.observation.intent, Record::Absent, "B's intent.json must not have been read: {o:?}");
    assert!(blocked_with(&o, "root re-walk") && blocked_with(&o, "symlink"), "{o:?}");
    fs::remove_file(fx.base.join("a")).unwrap();
    fs::rename(fx.base.join("a-moved"), fx.base.join("a")).unwrap();
    // variant 2: <base>/a becomes a real directory holding B's tree
    fn to_dir(root: &Path) {
        let a = root.parent().unwrap().to_path_buf(); let base = a.parent().unwrap().to_path_buf();
        fs::rename(&a, base.join("a-moved")).unwrap();
        fs::rename(base.join("b"), &a).unwrap();
    }
    hooks::set(Some(to_dir));
    let o = observe(&fx.root);
    hooks::set(None);
    assert_eq!(o.observation.store, Record::Valid(Store { phase: Phase::Prepared, last_generation: 7 }), "{o:?}");
    assert_eq!(o.observation.intent, Record::Absent, "{o:?}");
    assert!(blocked_with(&o, "root replaced at its path during observation"), "{o:?}");
    assert!(o.observation.blocked.len() == 1, "only the re-walk may block: {o:?}");
}
#[test]
fn a_symlinked_ancestor_blocks_before_anything_is_read() {
    let fx = Fixture::new("ancestor"); fx.root_dir().store("prepared", 0);
    let linked_base = fx.base.join("link-base");
    std::os::unix::fs::symlink(&fx.base, &linked_base).unwrap();
    let o = observe(&linked_base.join("runtime-v1"));
    assert!(blocked_with(&o, "path walk") && blocked_with(&o, "symlink"), "{o:?}");
    assert!(!o.observation.root_exists && o.checks.is_empty(), "{o:?}");
    if fs::symlink_metadata("/var").map(|m| m.file_type().is_symlink()).unwrap_or(false) {
        let o = observe(&std::env::temp_dir().join("gil-observe-never-created"));
        assert!(blocked_with(&o, "/var is a symlink"), "{o:?}");
    }
}
#[test]
fn a_failed_handle_duplicate_is_blocked_without_panic() {
    let fx = Fixture::new("dup"); fx.root_dir().store("prepared", 1);
    let d = fx.version(V1, &core_bytes()); fx.selection(1, V1, &d);
    hooks::FAIL_DUP.with(|c| c.set(true));
    let (o, v) = observe_checked(&fx);
    hooks::FAIL_DUP.with(|c| c.set(false));
    assert!(blocked_with(&o, "root handle duplicate") && blocked_with(&o, "injected duplicate failure"), "{o:?}");
    assert!(o.observation.root_exists && o.observation.store == Record::Absent && !checked(&o, "store.json"), "{o:?}");
    assert!(matches!(v.state, State::Blocked { .. })); assert_eq!(v.run, Run::None);
    assert!(observe(&fx.root).observation.blocked.is_empty());
}
#[test]
fn a_linked_or_unprotected_versions_directory_blocks_even_without_a_selection() {
    let fx = Fixture::under("versions-nosel", "a"); fx.root_dir().store("prepared", 0);
    let elsewhere = fx.base.join("elsewhere"); fs::create_dir(&elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, fx.root.join("versions")).unwrap();
    let (o, v) = observe_checked(&fx);
    assert!(blocked_with(&o, "versions/"), "{o:?}"); assert!(matches!(v.state, State::Blocked { .. }));
    fs::remove_file(fx.root.join("versions")).unwrap();
    fs::create_dir(fx.root.join("versions")).unwrap();
    fs::set_permissions(fx.root.join("versions"), fs::Permissions::from_mode(0o755)).unwrap();
    let o = observe(&fx.root);
    assert!(blocked_with(&o, "versions/ protection"), "{o:?}");
    fs::set_permissions(fx.root.join("versions"), fs::Permissions::from_mode(0o700)).unwrap();
    let (o, v) = observe_checked(&fx);
    assert!(o.observation.blocked.is_empty() && checked(&o, "versions/ protection"), "{o:?}"); assert_eq!(v.state, State::Prepared);
}
#[test]
fn compared_items_and_limits_are_stated() {
    assert!(COMPARED.iter().any(|l| l.contains("root directory listing")));
    assert!(COMPARED.iter().any(|l| l.contains("protection checks")));
    assert!(LIMITS.iter().any(|l| l.contains("not an atomic snapshot")));
    assert!(LIMITS.iter().any(|l| l.contains("not permission to run")));
    assert!(LIMITS.iter().any(|l| l.contains("size, mtime and ctime")));
}
