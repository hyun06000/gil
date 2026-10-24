//! Windows fixture tests for the read-only observer. Runs only on a Windows host; a Mac
//! cross-compile of this file is not evidence of its behaviour. Fixtures are built here
//! (separately from the observer): the root is created the product's way, exclusively and with
//! the owner-only protected DACL (`windows_fs::create_private_dir_at`), and children inherit
//! it; protection failures, links,
//! junctions and denies are made with `icacls`, `fs::hard_link` and a PowerShell junction, and
//! removed again before the fixture is deleted. A snapshot of names, attributes, sizes and
//! contents is compared before and after each observation.
use super::hooks;
use super::*;
use crate::runtime_state::{Run, State};
use crate::windows_fs as win;
use std::collections::BTreeMap;
use std::fs;
use std::os::windows::fs::MetadataExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const V1: &str = "0.2.1-dev.a";
fn sha(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
fn core_bytes() -> Vec<u8> { b"MZ gil-fixture (never executed)\n".to_vec() }
fn system32(tool: &str) -> PathBuf { PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32").join(tool) }
fn icacls(args: &[&str], target: &Path) {
    let out = std::process::Command::new(system32("icacls.exe")).arg(target).args(args).output().unwrap();
    assert!(out.status.success(), "icacls {args:?} on {}: {} {}", target.display(), String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}
/// `icacls` listing of one entry, quoted in assertion messages so a CI failure shows the DACL
/// the fixture actually produced. Observed on the runners (CI run 39866451531): `/grant
/// Everyone:(R)` on a file below a protected-DACL root writes the extra ACE *and* explicit
/// copies of the two inherited ACEs (5 ACEs: Everyone, user, SYSTEM, user(I), SYSTEM(I)), and
/// `/remove:g` takes only the Everyone ACE back out (4 left, CI run 39865476127). The product
/// check refuses any count other than the two inherited ACEs, so the fixture asserts that
/// refusal and restores the inherited-only state with `/reset`.
fn acl(path: &Path) -> String {
    let out = std::process::Command::new(system32("icacls.exe")).arg(path).output().unwrap();
    format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
}
/// Junction fixture (no privilege required). Paths travel as environment values, never as
/// shell source.
fn junction(link: &Path, target: &Path) {
    let powershell = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let made = std::process::Command::new(powershell)
        .args(["-NoProfile", "-NonInteractive", "-Command",
            "$ErrorActionPreference='Stop'; New-Item -ItemType Junction -Path $env:GIL_TEST_LINK -Target $env:GIL_TEST_TARGET | Out-Null"])
        .env("GIL_TEST_LINK", link)
        .env("GIL_TEST_TARGET", target)
        .output()
        .unwrap();
    assert!(made.status.success(), "junction fixture creation failed: {}", String::from_utf8_lossy(&made.stderr));
}
fn owner() -> String { platform::owner_text() }
/// Exclusive creation with the owner-only protected DACL, exactly as the product reserves a
/// root (`create_private_dir_at`: the handle it returns carries WRITE_DAC/WRITE_OWNER; a
/// read-only `open_dir` handle cannot set a DACL, CI run 39864263398).
fn protected_dir(dir: &Path) {
    let parent = win::open_dir(dir.parent().unwrap(), win::SHARE_ALL, false).unwrap();
    let made = win::create_private_dir_at(&parent, dir.file_name().unwrap()).unwrap();
    win::private_dir(&made).unwrap();
}
/// Re-apply the protected DACL to an existing directory through a handle opened with the
/// rights `SetSecurityInfo` needs (fixture cleanup only).
fn reprotect(dir: &Path) {
    use windows_sys::Win32::Storage::FileSystem::{FILE_FLAG_BACKUP_SEMANTICS, OPEN_EXISTING, WRITE_DAC, WRITE_OWNER};
    let handle = win::open_raw(dir, win::READ | WRITE_DAC | WRITE_OWNER, win::SHARE_ALL, OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS).unwrap();
    win::set_private_dacl_on(&handle).unwrap();
}
fn store_json(phase: &str, last_generation: u64) -> String {
    format!(r#"{{"schema":"{STORE_SCHEMA}","phase":"{phase}","last_generation":{last_generation},"owner":"{}","created":"t"}}"#, owner())
}

struct Fixture { base: PathBuf, root: PathBuf }
impl Fixture {
    fn new(label: &str) -> Self { Self::under(label, "") }
    fn under(label: &str, sub: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let base = std::env::temp_dir().join(format!("gil-observe-{label}-{}-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir_all(&base).unwrap();
        let parent = if sub.is_empty() { base.clone() } else { let p = base.join(sub); fs::create_dir(&p).unwrap(); p };
        Self { root: parent.join("runtime-v1"), base }
    }
    /// The root with the product's owner-only protected DACL; children inherit it.
    fn root_dir(&self) -> &Self { protected_dir(&self.root); self }
    fn write(&self, rel: &str, bytes: &[u8]) -> PathBuf {
        let p = self.root.join(rel);
        if let Some(d) = p.parent() { fs::create_dir_all(d).unwrap(); }
        if p.exists() { fs::remove_file(&p).unwrap(); }
        fs::write(&p, bytes).unwrap();
        p
    }
    fn store(&self, phase: &str, last_generation: u64) -> &Self { self.write("store.json", store_json(phase, last_generation).as_bytes()); self }
    fn intent(&self, generation: u64, version: &str, sha256: &str) -> &Self { self.write("intent.json", format!(r#"{{"schema":"{INTENT_SCHEMA}","generation":{generation},"version":"{version}","sha256":"{sha256}"}}"#).as_bytes()); self }
    fn selection(&self, generation: u64, version: &str, sha256: &str) -> &Self { self.write("selection.json", format!(r#"{{"schema":"{SELECTION_SCHEMA}","generation":{generation},"version":"{version}","sha256":"{sha256}"}}"#).as_bytes()); self }
    fn version(&self, version: &str, core: &[u8]) -> String {
        let digest = sha(core);
        let dir = format!("versions/{version}-{}", &digest[..12]);
        self.write(&format!("{dir}/{CORE_FILE}"), core);
        self.write(&format!("{dir}/complete"), digest.as_bytes());
        digest
    }
    fn version_dir(&self, digest: &str) -> PathBuf { self.root.join(format!("versions/{V1}-{}", &digest[..12])) }
    fn snapshot(&self) -> BTreeMap<String, (u32, u64, String)> {
        fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, (u32, u64, String)>) {
            let Ok(rd) = fs::read_dir(dir) else { return };
            for e in rd {
                let p = e.unwrap().path();
                let m = fs::symlink_metadata(&p).unwrap();
                let content = if m.is_file() { sha(&fs::read(&p).unwrap_or_default()) } else { String::new() };
                out.insert(p.strip_prefix(base).unwrap().display().to_string(), (m.file_attributes(), m.file_size(), content));
                if m.is_dir() && !m.file_type().is_symlink() && m.file_attributes() & 0x400 == 0 { walk(base, &p, out); }
            }
        }
        let mut out = BTreeMap::new();
        if self.root.exists() { walk(&self.base, &self.root, &mut out); }
        out
    }
}
impl Drop for Fixture {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.base); }
}
fn observe_checked(fx: &Fixture) -> (Observed, crate::runtime_state::Verdict) {
    let before = fx.snapshot();
    let out = observe_and_judge(&fx.root);
    assert_eq!(fx.snapshot(), before, "observation must not change names, attributes, sizes or contents");
    out
}
fn blocked_with(o: &Observed, needle: &str) -> bool { o.observation.blocked.iter().any(|b| b.contains(needle)) }
fn checked(o: &Observed, needle: &str) -> bool { o.checks.iter().any(|c| c.contains(needle)) }

#[test]
fn w_first_use_only_when_the_root_is_not_found() {
    let fx = Fixture::new("first");
    let (o, v) = observe_checked(&fx);
    assert!(!o.observation.root_exists && o.observation.blocked.is_empty(), "{o:?}");
    assert_eq!(v.state, State::FirstUse); assert_eq!(v.run, Run::Bundled);
    assert!(checked(&o, "root not found"));
}
#[test]
fn w_prepared_selected_and_history_lagging_from_real_files() {
    let fx = Fixture::new("normal"); fx.root_dir().store("prepared", 0);
    let (o, v) = observe_checked(&fx);
    assert_eq!(v.state, State::Prepared, "{o:?}");
    let d = fx.version(V1, &core_bytes());
    fx.store("prepared", 1).selection(1, V1, &d);
    let (o, v) = observe_checked(&fx);
    assert_eq!(v.state, State::Selected { generation: 1, version: V1.into(), history_lagging: false }, "{o:?}");
    for needle in ["path walk", "root protection", "store.json protection", "selection.json protection", "versions/ protection", "complete protection", "gil.exe protection", "FILE_EXECUTE", "digest equals selection", "re-check"] {
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
fn w_damaged_cases_are_damaged_not_blocked() {
    let fx = Fixture::new("damaged"); fx.root_dir();
    let (o, v) = observe_checked(&fx);
    assert!(matches!(v.state, State::Damaged { .. }) && o.observation.blocked.is_empty(), "{o:?}");
    fx.write("store.json", b"{ not json");
    let (o, v) = observe_checked(&fx);
    assert_eq!(o.observation.store, Record::Invalid); assert!(matches!(v.state, State::Damaged { .. }));
    fx.store("prepared", 1);
    let big = format!(r#"{{"schema":"{SELECTION_SCHEMA}","generation":1,"version":"v","sha256":"{}","pad":"{}"}}"#, "0".repeat(64), "x".repeat(MAX_RECORD_BYTES as usize));
    fx.write("selection.json", big.as_bytes());
    let (o, v) = observe_checked(&fx);
    assert_eq!(o.observation.selection, Record::Invalid, "{o:?}"); assert!(matches!(v.state, State::Damaged { .. }));
    let d = fx.version(V1, &core_bytes());
    fx.write(&format!("versions/{V1}-{}/{CORE_FILE}", &d[..12]), b"tampered");
    fx.selection(1, V1, &d);
    let (o, v) = observe_checked(&fx);
    assert_eq!(o.observation.selection, Record::Valid(Selection { generation: 1, version: V1.into(), sha256: d.clone(), version_complete: false }));
    assert!(matches!(v.state, State::Damaged { .. }) && o.observation.blocked.is_empty(), "{o:?}");
    fx.write(&format!("versions/{V1}-{}/{CORE_FILE}", &d[..12]), &core_bytes());
    fs::remove_file(fx.version_dir(&d).join("complete")).unwrap();
    let (o, _) = observe_checked(&fx);
    assert!(matches!(o.observation.selection, Record::Valid(Selection { version_complete: false, .. })) && o.observation.blocked.is_empty(), "{o:?}");
    for version in ["..\\x", "a\\b", "a/b", "..", "con:"] {
        fx.write("selection.json", format!(r#"{{"schema":"{SELECTION_SCHEMA}","generation":1,"version":"{}","sha256":"{}"}}"#, version.replace('\\', "\\\\"), "a".repeat(64)).as_bytes());
        assert_eq!(observe(&fx.root).observation.selection, Record::Invalid, "{version:?}");
    }
}
#[test]
fn w_protection_failures_block_and_stop_the_descent() {
    let fx = Fixture::new("protect"); fx.root_dir().store("prepared", 1);
    let d = fx.version(V1, &core_bytes()); fx.selection(1, V1, &d);
    assert!(observe(&fx.root).observation.blocked.is_empty(), "{:?}", observe(&fx.root));
    // an extra explicit ACE on a record: DACL structure check fails, record not read
    icacls(&["/grant", "*S-1-1-0:(R)"], &fx.root.join("store.json"));
    let (o, v) = observe_checked(&fx);
    assert!(blocked_with(&o, "store.json protection") && blocked_with(&o, "ACEs instead of the two inherited ones") && blocked_with(&o, "(not read)"), "{o:?}\n{}", acl(&fx.root.join("store.json")));
    assert!(matches!(v.state, State::Blocked { .. })); assert_eq!(v.run, Run::None);
    icacls(&["/reset"], &fx.root.join("store.json")); // back to the inherited-only ACEs
    let o = observe(&fx.root);
    assert!(o.observation.blocked.is_empty(), "{o:?}\n{}", acl(&fx.root.join("store.json")));
    // versions/ with a protected (non-inherited) DACL: the candidate below is not read
    icacls(&["/inheritance:d"], &fx.root.join("versions"));
    let o = observe(&fx.root);
    assert!(blocked_with(&o, "versions/ protection") && blocked_with(&o, "nothing below was read") && !checked(&o, "complete") && !checked(&o, "digest equals"), "{o:?}");
    icacls(&["/reset"], &fx.root.join("versions")); // back to inherited-only ACEs
    // complete marker with an extra ACE: Core not read
    icacls(&["/grant", "*S-1-1-0:(R)"], &fx.version_dir(&d).join("complete"));
    let o = observe(&fx.root);
    assert!(blocked_with(&o, "complete protection") && blocked_with(&o, "ACEs instead of the two inherited ones") && blocked_with(&o, "(Core not read)") && !checked(&o, "digest equals"), "{o:?}\n{}", acl(&fx.version_dir(&d).join("complete")));
    icacls(&["/reset"], &fx.version_dir(&d).join("complete"));
    let o = observe(&fx.root);
    assert!(o.observation.blocked.is_empty(), "{o:?}\n{}", acl(&fx.version_dir(&d).join("complete")));
    // Core with an extra ACE: not hashed
    icacls(&["/grant", "*S-1-1-0:(RX)"], &fx.version_dir(&d).join(CORE_FILE));
    let o = observe(&fx.root);
    assert!(blocked_with(&o, "gil.exe protection") && blocked_with(&o, "ACEs instead of the two inherited ones") && blocked_with(&o, "(not hashed)"), "{o:?}\n{}", acl(&fx.version_dir(&d).join(CORE_FILE)));
    icacls(&["/reset"], &fx.version_dir(&d).join(CORE_FILE));
    let o = observe(&fx.root);
    assert!(o.observation.blocked.is_empty(), "{o:?}\n{}", acl(&fx.version_dir(&d).join(CORE_FILE)));
    // the root loses its protected DACL: nothing below is read (last: inheritance from the temp
    // directory propagates into the tree, so the fixture is only re-protected for cleanup)
    icacls(&["/inheritance:e"], &fx.root);
    let o = observe(&fx.root);
    assert!(blocked_with(&o, "root protection") && blocked_with(&o, "nothing below the root was read") && !checked(&o, "store.json"), "{o:?}");
    reprotect(&fx.root);
}
#[test]
fn w_reparse_points_and_hard_links_are_blocked() {
    let fx = Fixture::new("links"); fx.root_dir().store("prepared", 1);
    let d = fx.version(V1, &core_bytes()); fx.selection(1, V1, &d);
    // hard-linked record and hard-linked complete marker
    fs::hard_link(fx.root.join("selection.json"), fx.base.join("hard.json")).unwrap();
    let o = observe(&fx.root);
    assert!(blocked_with(&o, "selection.json protection") && blocked_with(&o, "link count"), "{o:?}");
    fs::remove_file(fx.base.join("hard.json")).unwrap();
    fs::hard_link(fx.version_dir(&d).join("complete"), fx.base.join("complete-hard")).unwrap();
    let o = observe(&fx.root);
    assert!(blocked_with(&o, "complete protection") && blocked_with(&o, "link count") && !checked(&o, "digest equals"), "{o:?}");
    fs::remove_file(fx.base.join("complete-hard")).unwrap();
    // junction in place of the version directory
    let vdir = fx.version_dir(&d);
    let moved = fx.base.join("vdir"); fs::rename(&vdir, &moved).unwrap();
    junction(&vdir, &moved);
    let o = observe(&fx.root);
    assert!(blocked_with(&o, &format!("versions/{V1}")) && !checked(&o, "digest equals"), "{o:?}");
    fs::remove_dir(&vdir).unwrap(); fs::rename(&moved, &vdir).unwrap();
    // junction as the root and as an ancestor
    let link_root = fx.base.join("link-root"); junction(&link_root, &fx.root);
    let o = observe(&link_root);
    assert!(blocked_with(&o, "path walk") && !o.observation.root_exists, "{o:?}");
    fs::remove_dir(&link_root).unwrap();
    let link_base = fx.base.join("link-base"); junction(&link_base, &fx.base);
    let o = observe(&link_base.join("runtime-v1"));
    assert!(blocked_with(&o, "path walk") && o.checks.is_empty(), "{o:?}");
    fs::remove_dir(&link_base).unwrap();
    assert!(observe(&fx.root).observation.blocked.is_empty(), "{:?}", observe(&fx.root));
}
#[test]
fn w_access_denied_is_blocked_never_absent_or_first_use() {
    let fx = Fixture::new("deny"); fx.root_dir().store("prepared", 0);
    icacls(&["/deny", "*S-1-1-0:(R)"], &fx.root.join("store.json"));
    // precondition: the deny is effective for this token (a backup-privileged token would read
    // through it and make the test meaningless)
    let probe = fs::read(fx.root.join("store.json"));
    assert!(matches!(&probe, Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied), "deny fixture not effective for this token: {probe:?}");
    let o = observe(&fx.root);
    assert!(o.observation.root_exists && blocked_with(&o, "store.json") && !checked(&o, "store.json"), "{o:?}");
    assert!(matches!(crate::runtime_state::judge(&o.observation).state, State::Blocked { .. }));
    icacls(&["/remove:d", "*S-1-1-0"], &fx.root.join("store.json"));
    // denied root: blocked at the walk, not first use
    icacls(&["/deny", "*S-1-1-0:(RD)"], &fx.root);
    let o = observe(&fx.root);
    assert!(blocked_with(&o, "path walk") || blocked_with(&o, "root listing") || blocked_with(&o, "root protection"), "{o:?}");
    assert!(!matches!(crate::runtime_state::judge(&o.observation).state, State::FirstUse), "{o:?}");
    icacls(&["/remove:d", "*S-1-1-0"], &fx.root);
}
#[test]
fn w_a_change_during_observation_is_detected_and_blocks() {
    let fx = Fixture::under("race", "a"); fx.root_dir().store("prepared", 1);
    let d = fx.version(V1, &core_bytes()); fx.selection(1, V1, &d);
    fn rewrite(root: &Path) { std::thread::sleep(std::time::Duration::from_millis(20)); fs::write(root.join("store.json"), store_json("prepared", 2)).unwrap(); }
    hooks::set(Some(rewrite));
    let o = observe(&fx.root);
    hooks::set(None);
    assert!(blocked_with(&o, "store.json changed during observation"), "{o:?}");
    fn add(root: &Path) { fs::write(root.join("stray.json"), b"{}").unwrap(); }
    hooks::set(Some(add));
    let o = observe(&fx.root);
    hooks::set(None);
    assert!(blocked_with(&o, "listing changed"), "{o:?}");
    fs::remove_file(fx.root.join("stray.json")).unwrap();
    // a DACL change after the read is detected (change time and protection re-check)
    fn grant(root: &Path) { icacls(&["/grant", "*S-1-1-0:(R)"], &root.join("store.json")); }
    hooks::set(Some(grant));
    let o = observe(&fx.root);
    hooks::set(None);
    assert!(blocked_with(&o, "store.json changed during observation") && blocked_with(&o, "store.json protection changed during observation"), "{o:?}\n{}", acl(&fx.root.join("store.json")));
    icacls(&["/reset"], &fx.root.join("store.json"));
    let o = observe(&fx.root);
    assert!(o.observation.blocked.is_empty(), "{o:?}\n{}", acl(&fx.root.join("store.json")));
    // versions/ swapped for a junction between the store read and the version read: the
    // junction is refused and the root-entry identity recorded first differs
    fn swap(root: &Path) {
        let v = root.join("versions"); let base = root.parent().unwrap().parent().unwrap().to_path_buf();
        fs::rename(&v, base.join("versions-moved")).unwrap();
        junction(&v, &base.join("versions-moved"));
    }
    hooks::set(Some(swap));
    let o = observe(&fx.root);
    hooks::set(None);
    assert!(blocked_with(&o, "versions/") && blocked_with(&o, "versions re-check") , "{o:?}");
    assert!(!checked(&o, "digest equals"));
    fs::remove_dir(fx.root.join("versions")).unwrap();
    fs::rename(fx.base.join("versions-moved"), fx.root.join("versions")).unwrap();
    let o = observe(&fx.root);
    assert!(o.observation.blocked.is_empty(), "{o:?}\n{}", acl(&fx.root.join("versions")));
}
#[test]
fn w_an_ancestor_swapped_during_observation_never_reads_the_other_tree() {
    // A at <base>\a\runtime-v1 (prepared, 7); B at <base>\b\runtime-v1 with an invalid intent.
    let fx = Fixture::under("swap-ancestor", "a"); fx.root_dir().store("prepared", 7);
    let b_root = fx.base.join("b").join("runtime-v1");
    fs::create_dir(fx.base.join("b")).unwrap();
    protected_dir(&b_root);
    fs::write(b_root.join("store.json"), store_json("prepared", 9)).unwrap();
    fs::write(b_root.join("intent.json"), b"not json: reading B would make intent Invalid").unwrap();
    thread_local! { static SWAPPED: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) }; }
    fn swap(root: &Path) {
        let a = root.parent().unwrap().to_path_buf(); let base = a.parent().unwrap().to_path_buf();
        // Renaming a directory with open handles beneath it may be refused by the OS; both
        // outcomes are recorded and asserted below.
        let done = fs::rename(&a, base.join("a-moved")).is_ok() && fs::rename(base.join("b"), &a).is_ok();
        SWAPPED.with(|c| c.set(Some(done)));
    }
    hooks::set(Some(swap));
    let o = observe(&fx.root);
    hooks::set(None);
    let swapped = SWAPPED.with(|c| c.get()).expect("hook ran");
    // Printed so a --nocapture run shows which branch this host actually exercised; neither
    // branch alone is evidence for the other.
    println!("OBSERVED ancestor-swap branch: {} (blocked = {:?})", if swapped { "rename succeeded, re-walk must block" } else { "OS refused the rename of a directory with open handles beneath it" }, o.observation.blocked);
    assert_eq!(o.observation.store, Record::Valid(Store { phase: Phase::Prepared, last_generation: 7 }), "{o:?}");
    assert_eq!(o.observation.intent, Record::Absent, "B's intent.json must not have been read: {o:?}");
    if swapped {
        assert!(blocked_with(&o, "root replaced at its path during observation") || blocked_with(&o, "root re-walk"), "{o:?}");
    } else {
        assert!(o.observation.blocked.is_empty(), "rename refused by the OS, so nothing changed: {o:?}");
    }
}
#[test]
fn w_a_failed_handle_duplicate_is_blocked_without_panic() {
    let fx = Fixture::new("w_dup"); fx.root_dir().store("prepared", 1);
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
fn w_compared_items_and_limits_are_stated() {
    assert!(COMPARED.iter().any(|l| l.contains("change time")));
    assert!(LIMITS.iter().any(|l| l.contains("not permission to run")));
}
