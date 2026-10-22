//! Fixture tests for initialization and candidate preparation. Every test works in a fresh
//! isolated temp tree; the store itself is created by `prepare` (the product path), so most
//! tests are platform-independent. Links and protection breaks are platform-specific.
use super::hooks;
use super::*;
use crate::runtime_state::{Run, State};
use std::collections::BTreeMap;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier, Mutex};

const V1: &str = "0.2.1-dev.a";
const V2: &str = "0.2.2-dev.b";
fn sha(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
fn bytes_a() -> Vec<u8> { b"synthetic core A (never executed)\n".repeat(4000) }
fn bytes_b() -> Vec<u8> { b"synthetic core B (never executed)\n".repeat(4000) }

struct Fixture { base: PathBuf, root: PathBuf }
impl Fixture {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let tmp = std::env::temp_dir();
        #[cfg(unix)]
        let tmp = fs::canonicalize(tmp).unwrap();
        let base = tmp.join(format!("gil-prepare-{label}-{}-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir_all(&base).unwrap();
        #[cfg(unix)]
        { use std::os::unix::fs::PermissionsExt; fs::set_permissions(&base, fs::Permissions::from_mode(0o700)).unwrap(); }
        Self { root: base.join("runtime-v1"), base }
    }
    fn source(&self, name: &str, bytes: &[u8]) -> PathBuf { let p = self.base.join(name); fs::write(&p, bytes).unwrap(); p }
    /// Names, kinds, sizes, contents and (Unix) modes below the root.
    fn snapshot(&self) -> BTreeMap<String, (bool, u64, String, u32)> {
        fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, (bool, u64, String, u32)>) {
            let Ok(rd) = fs::read_dir(dir) else { return };
            for e in rd {
                let p = e.unwrap().path();
                let m = fs::symlink_metadata(&p).unwrap();
                let content = if m.is_file() { sha(&fs::read(&p).unwrap_or_default()) } else { String::new() };
                #[cfg(unix)]
                let mode = { use std::os::unix::fs::PermissionsExt; m.permissions().mode() & 0o7777 };
                #[cfg(not(unix))]
                let mode = 0;
                out.insert(p.strip_prefix(base).unwrap().display().to_string().replace('\\', "/"), (m.is_dir(), m.len(), content, mode));
                if m.is_dir() && !m.file_type().is_symlink() { walk(base, &p, out); }
            }
        }
        let mut out = BTreeMap::new();
        if self.root.exists() { walk(&self.base, &self.root, &mut out); }
        out
    }
    fn has(&self, rel: &str) -> bool { self.root.join(rel).exists() }
    fn complete_dirs_of(&self, version: &str) -> usize {
        fs::read_dir(self.root.join(VERSIONS_DIR)).map(|rd| rd.flatten().filter(|e| { let n = e.file_name().to_string_lossy().into_owned(); n.starts_with(&format!("{version}-")) && e.path().join(COMPLETE_FILE).exists() }).count()).unwrap_or(0)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        hooks::reset();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fn relax(p: &Path) { if let Ok(rd) = fs::read_dir(p) { for e in rd.flatten() { let q = e.path(); if fs::symlink_metadata(&q).map(|m| m.file_type().is_symlink()).unwrap_or(true) { continue; } let _ = fs::set_permissions(&q, fs::Permissions::from_mode(0o700)); if q.is_dir() { relax(&q); } } } }
            relax(&self.base);
        }
        let _ = fs::remove_dir_all(&self.base);
    }
}
fn vdir(version: &str, digest: &str) -> String { format!("{VERSIONS_DIR}/{}", version_dirname(version, digest)) }
fn reserve(version: &str) -> String { format!("{VERSIONS_DIR}/{}", reservation_name(version)) }
const RECORDED: &str = "store.json (record replaced: initializing → prepared)";
const SUPERSEDED: &str = "store.json (initializing record, replaced by this call)";
fn full_first_run(digest: &str) -> Vec<String> {
    vec!["<root>".into(), SUPERSEDED.into(), "versions".into(), reserve(V1), vdir(V1, digest), format!("{}/{CORE_FILE}", vdir(V1, digest)), format!("{}/{COMPLETE_FILE}", vdir(V1, digest)), RECORDED.into()]
}
/// The labels a run that stopped before the record replacement reports (`store.json` still the
/// initializing record).
fn partial_first_run(digest: &str, n: usize) -> Vec<String> {
    let mut v = full_first_run(digest)[..n].to_vec();
    if let Some(l) = v.iter_mut().find(|l| **l == SUPERSEDED) { *l = "store.json".into(); }
    v
}
/// A directory taken from under its name is refused at the final name re-check (Unix,
/// `Stage::Reobserve`) or already at the directory flush, which on Windows re-opens the pinned
/// directory by path and verifies its identity (`Stage::Complete`, reason "flush"). Both are
/// refusals with accurate object reports; the branch taken is printed for the log.
fn assert_refused_by_name_check(e: &Refusal, what: &str) {
    let ok = matches!(e.stage, Stage::Reobserve) || (matches!(e.stage, Stage::Complete) && e.reason.contains("flush"));
    assert!(ok, "{what}: {e:?}");
    println!("OBSERVED {what}: refused at {:?} ({})", e.stage, e.reason);
}
thread_local! { static SWAP_REFUSED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
fn presence_of<'a>(objects: &'a [Object], label: &str) -> &'a Presence { &objects.iter().find(|o| o.label == label).unwrap_or_else(|| panic!("{label} not reported in {objects:?}")).presence }
/// Everything created must be reported as the same object, except the initializing record this
/// call itself replaced.
fn assert_all_same(objects: &[Object]) {
    for o in objects {
        let expected = if o.label == SUPERSEDED { Presence::Replaced } else { Presence::SameObject };
        assert!(o.presence == expected, "{o:?} in {objects:?}");
    }
}

#[test]
fn first_use_initializes_and_prepares_then_a_repeat_is_a_no_op() {
    let fx = Fixture::new("first");
    let a = bytes_a(); let d = sha(&a); let src = fx.source("core-a", &a);
    let r = prepare(&fx.root, &Request { version: V1, expected_sha256: &d, source: &src }).unwrap();
    assert_eq!(r.outcome, Outcome::Prepared);
    assert_eq!(r.before.state, State::FirstUse);
    assert_eq!(r.after.state, State::Prepared); assert_eq!(r.after.run, Run::Bundled);
    assert_eq!(r.created, full_first_run(&d));
    assert_all_same(&r.objects);
    assert_eq!(fs::read(fx.root.join(&r.version_dir).join(CORE_FILE)).unwrap(), a);
    assert!(fx.has(&format!("{}/{COMPLETE_FILE}", r.version_dir)) && !fx.has(STORE_NEW) && fx.has(&reserve(V1)), "the reservation stays");
    let (o, v) = observe_and_judge(&fx.root);
    assert!(o.observation.blocked.is_empty(), "{o:?}"); assert_eq!(v.state, State::Prepared);
    let before = fx.snapshot();
    let r2 = prepare(&fx.root, &Request { version: V1, expected_sha256: &d, source: &src }).unwrap();
    assert_eq!(r2.outcome, Outcome::AlreadyPrepared); assert!(r2.created.is_empty(), "{r2:?}");
    assert_eq!(fx.snapshot(), before);
}
#[test]
fn requests_are_validated_before_anything_is_touched() {
    let fx = Fixture::new("request");
    let src = fx.source("core", &bytes_a());
    let e = prepare(&fx.root, &Request { version: "../x", expected_sha256: &"0".repeat(64), source: &src }).unwrap_err();
    assert_eq!(e.stage, Stage::Request); assert!(e.created.is_empty() && !fx.root.exists());
    let e = prepare(&fx.root, &Request { version: V1, expected_sha256: "ABC", source: &src }).unwrap_err();
    assert_eq!(e.stage, Stage::Request); assert!(!fx.root.exists());
}
#[test]
fn the_same_version_with_different_bytes_is_refused_and_nothing_changes() {
    let fx = Fixture::new("bytes");
    let a = bytes_a(); let b = bytes_b();
    let src_a = fx.source("core-a", &a); let src_b = fx.source("core-b", &b);
    prepare(&fx.root, &Request { version: V1, expected_sha256: &sha(&a), source: &src_a }).unwrap();
    let before = fx.snapshot();
    let e = prepare(&fx.root, &Request { version: V1, expected_sha256: &sha(&b), source: &src_b }).unwrap_err();
    assert_eq!(e.stage, Stage::Uniqueness); assert!(e.reason.contains("different bytes"), "{e:?}");
    assert!(e.created.is_empty() && e.remaining.is_empty());
    assert_eq!(fx.snapshot(), before);
    // a complete directory whose Core no longer matches the expected digest is reported, kept
    let core = fx.root.join(vdir(V1, &sha(&a))).join(CORE_FILE);
    #[cfg(unix)]
    { use std::os::unix::fs::PermissionsExt; fs::set_permissions(&core, fs::Permissions::from_mode(0o600)).unwrap(); }
    fs::write(&core, &b).unwrap();
    #[cfg(unix)]
    { use std::os::unix::fs::PermissionsExt; fs::set_permissions(&core, fs::Permissions::from_mode(0o500)).unwrap(); }
    let before = fx.snapshot();
    let e = prepare(&fx.root, &Request { version: V1, expected_sha256: &sha(&a), source: &src_a }).unwrap_err();
    assert_eq!(e.stage, Stage::Uniqueness); assert!(e.reason.contains("digest differs"), "{e:?}");
    assert_eq!(fx.snapshot(), before);
}
#[test]
fn already_prepared_requires_the_full_checks_on_complete_and_core() {
    let fx = Fixture::new("already");
    let a = bytes_a(); let d = sha(&a); let src = fx.source("core-a", &a);
    prepare(&fx.root, &Request { version: V1, expected_sha256: &d, source: &src }).unwrap();
    let complete = fx.root.join(vdir(V1, &d)).join(COMPLETE_FILE);
    let core = fx.root.join(vdir(V1, &d)).join(CORE_FILE);
    let cases: Vec<(&str, Box<dyn Fn()>, Box<dyn Fn()>, &str)> = vec![
        ("complete protection", Box::new(|| break_protection(&complete)), Box::new(|| restore_protection(&complete, false)), "complete protection"),
        ("complete hard link", Box::new(|| fs::hard_link(&complete, fx.base.join("complete-hard")).unwrap()), Box::new(|| fs::remove_file(fx.base.join("complete-hard")).unwrap()), "link count"),
        ("core protection", Box::new(|| break_protection(&core)), Box::new(|| restore_protection(&core, true)), "gil"),
        ("core hard link", Box::new(|| fs::hard_link(&core, fx.base.join("core-hard")).unwrap()), Box::new(|| fs::remove_file(fx.base.join("core-hard")).unwrap()), "link count"),
    ];
    for (what, breaker, restorer, needle) in cases {
        breaker();
        let before = fx.snapshot();
        let r = prepare(&fx.root, &Request { version: V1, expected_sha256: &d, source: &src });
        let e = match r { Err(e) => e, Ok(r) => panic!("{what}: AlreadyPrepared returned on a broken preparation: {r:?}") };
        assert_eq!(e.stage, Stage::Uniqueness, "{what}: {e:?}");
        assert!(e.reason.contains(needle) && e.reason.contains("kept"), "{what}: {e:?}");
        assert!(e.created.is_empty(), "{what}: {e:?}");
        assert_eq!(fx.snapshot(), before, "{what}: bytes or permissions changed");
        restorer();
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // an unexecutable Core is not accepted either
        fs::set_permissions(&core, fs::Permissions::from_mode(0o400)).unwrap();
        let e = prepare(&fx.root, &Request { version: V1, expected_sha256: &d, source: &src }).unwrap_err();
        assert!(e.stage == Stage::Uniqueness && e.reason.contains("mode 400"), "{e:?}");
        fs::set_permissions(&core, fs::Permissions::from_mode(0o500)).unwrap();
    }
    let r = prepare(&fx.root, &Request { version: V1, expected_sha256: &d, source: &src }).unwrap();
    assert_eq!(r.outcome, Outcome::AlreadyPrepared);
}
#[test]
fn a_second_version_is_added_without_touching_the_first() {
    let fx = Fixture::new("second");
    let a = bytes_a(); let b = bytes_b();
    let src_a = fx.source("core-a", &a); let src_b = fx.source("core-b", &b);
    prepare(&fx.root, &Request { version: V1, expected_sha256: &sha(&a), source: &src_a }).unwrap();
    let first: BTreeMap<_, _> = fx.snapshot().into_iter().filter(|(k, _)| k.contains(&vdir(V1, &sha(&a))) || k.ends_with("store.json")).collect();
    let r = prepare(&fx.root, &Request { version: V2, expected_sha256: &sha(&b), source: &src_b }).unwrap();
    assert_eq!(r.outcome, Outcome::Prepared);
    assert_eq!(r.created, vec![reserve(V2), vdir(V2, &sha(&b)), format!("{}/{CORE_FILE}", vdir(V2, &sha(&b))), format!("{}/{COMPLETE_FILE}", vdir(V2, &sha(&b)))]);
    assert_all_same(&r.objects);
    assert_eq!(r.before.state, State::Prepared); assert_eq!(r.after.state, State::Prepared);
    let now: BTreeMap<_, _> = fx.snapshot().into_iter().filter(|(k, _)| k.contains(&vdir(V1, &sha(&a))) || k.ends_with("store.json")).collect();
    assert_eq!(now, first, "existing version and record changed");
}
#[test]
fn an_existing_selection_and_history_are_left_alone() {
    let fx = Fixture::new("selected");
    let a = bytes_a(); let b = bytes_b();
    let src_a = fx.source("core-a", &a); let src_b = fx.source("core-b", &b);
    prepare(&fx.root, &Request { version: V1, expected_sha256: &sha(&a), source: &src_a }).unwrap();
    let selection = format!(r#"{{"schema":"{}","generation":1,"version":"{V1}","sha256":"{}"}}"#, observe::SELECTION_SCHEMA, sha(&a));
    fs::write(fx.root.join("selection.json"), &selection).unwrap();
    fs::write(fx.root.join("store.json"), store_json("prepared", 1)).unwrap();
    #[cfg(unix)]
    { use std::os::unix::fs::PermissionsExt; fs::set_permissions(fx.root.join("selection.json"), fs::Permissions::from_mode(0o600)).unwrap(); }
    let (o, v) = observe_and_judge(&fx.root);
    assert_eq!(v.state, State::Selected { generation: 1, version: V1.into(), history_lagging: false }, "{o:?}");
    let store_before = fs::read(fx.root.join("store.json")).unwrap();
    let r = prepare(&fx.root, &Request { version: V2, expected_sha256: &sha(&b), source: &src_b }).unwrap();
    assert_eq!(r.outcome, Outcome::Prepared);
    assert_eq!(r.after.state, State::Selected { generation: 1, version: V1.into(), history_lagging: false });
    assert_eq!(fs::read(fx.root.join("selection.json")).unwrap(), selection.as_bytes());
    assert_eq!(fs::read(fx.root.join("store.json")).unwrap(), store_before);
    assert!(!r.created.iter().any(|c| c.contains("store.json")), "{r:?}");
}
#[test]
fn damaged_and_blocked_stores_are_refused_without_bootstrap() {
    let fx = Fixture::new("damaged");
    let a = bytes_a(); let src = fx.source("core-a", &a);
    make_protected_root(&fx.root);
    let before = fx.snapshot();
    let e = prepare(&fx.root, &Request { version: V1, expected_sha256: &sha(&a), source: &src }).unwrap_err();
    assert_eq!(e.stage, Stage::Observe); assert!(e.reason.contains("damaged"), "{e:?}");
    assert!(e.created.is_empty()); assert_eq!(fx.snapshot(), before);
    break_root_protection(&fx.root);
    let e = prepare(&fx.root, &Request { version: V1, expected_sha256: &sha(&a), source: &src }).unwrap_err();
    assert_eq!(e.stage, Stage::Observe); assert!(e.reason.contains("blocked"), "{e:?}");
    assert!(e.created.is_empty());
}
#[test]
fn failure_injection_at_every_boundary_keeps_partials_and_reports_them_exactly() {
    let a = bytes_a(); let d = sha(&a);
    let complete = format!("{}/{COMPLETE_FILE}", vdir(V1, &d));
    let base = |n: usize| -> Vec<String> { partial_first_run(&d, n) };
    let cases: Vec<(Point, Stage, Vec<String>, State)> = vec![
        (Point::RootCreated, Stage::Root, base(1), State::Damaged { reason: String::new() }),
        (Point::StoreWritten, Stage::StoreRecord, base(2), State::Initializing),
        (Point::VersionsCreated, Stage::Versions, base(3), State::Initializing),
        (Point::Reserved, Stage::Reservation, base(4), State::Initializing),
        (Point::VersionDirCreated, Stage::VersionDir, base(5), State::Initializing),
        (Point::CoreWritten, Stage::Copy, base(6), State::Initializing),
        (Point::CoreFlushed, Stage::Copy, base(6), State::Initializing),
        (Point::Verified, Stage::Verify, base(6), State::Initializing),
        (Point::CompleteWritten, Stage::Complete, base(7), State::Initializing),
        (Point::NewStoreWritten, Stage::PreparedRecord, { let mut v = base(7); v.push(STORE_NEW.into()); v }, State::Initializing),
    ];
    for (point, stage, created, state) in cases {
        let fx = Fixture::new("inject");
        let src = fx.source("core-a", &a);
        hooks::FAIL_AFTER.with(|c| c.set(Some(point)));
        let e = prepare(&fx.root, &Request { version: V1, expected_sha256: &d, source: &src }).unwrap_err();
        hooks::reset();
        assert_eq!(e.stage, stage, "{point:?}: {e:?}");
        assert!(e.reason.contains("injected failure"), "{point:?}: {e:?}");
        assert_eq!(e.created, created, "{point:?}");
        assert_eq!(e.remaining, e.created, "{point:?}: nothing may be deleted");
        assert_all_same(&e.objects);
        assert_eq!(fx.has(&complete), point >= Point::CompleteWritten, "{point:?}: complete marker presence");
        assert_eq!(fx.has(&reserve(V1)), point >= Point::Reserved, "{point:?}: a reservation, once created, stays");
        assert_eq!(std::mem::discriminant(&e.after.as_ref().unwrap().state), std::mem::discriminant(&state), "{point:?}: {:?}", e.after);
        let again = prepare(&fx.root, &Request { version: V1, expected_sha256: &d, source: &src });
        match point {
            Point::RootCreated => { let e = again.unwrap_err(); assert_eq!(e.stage, Stage::Observe); assert!(e.created.is_empty()); }
            Point::StoreWritten | Point::VersionsCreated => { let r = again.unwrap(); assert_eq!(r.outcome, Outcome::Prepared); assert_eq!(r.after.state, State::Prepared); assert!(r.created.ends_with(&[RECORDED.to_string()])); }
            Point::Reserved => { let e = again.unwrap_err(); assert_eq!(e.stage, Stage::Reservation); assert!(e.reason.contains("interrupted"), "{e:?}"); assert!(e.created.is_empty()); assert!(!fx.has(&vdir(V1, &d))); }
            Point::VersionDirCreated | Point::CoreWritten | Point::CoreFlushed | Point::Verified => { let e = again.unwrap_err(); assert_eq!(e.stage, Stage::Uniqueness); assert!(e.reason.contains("incomplete"), "{e:?}"); assert!(e.created.is_empty()); assert!(!fx.has(&complete)); assert!(fx.has(&reserve(V1)), "stale reservation must stay"); }
            Point::CompleteWritten => { let r = again.unwrap(); assert_eq!(r.outcome, Outcome::AlreadyPrepared); assert_eq!(r.created, vec![RECORDED.to_string()]); assert_eq!(r.after.state, State::Prepared); assert!(fx.has(&reserve(V1)), "the reservation is never reclaimed"); }
            Point::NewStoreWritten => { let e = again.unwrap_err(); assert_eq!(e.stage, Stage::PreparedRecord); assert!(e.reason.contains(STORE_NEW), "{e:?}"); assert!(e.created.is_empty()); assert!(fx.has(STORE_NEW)); }
        }
    }
}
#[test]
fn write_flush_and_digest_failures_keep_the_partial_without_complete() {
    let fx = Fixture::new("io");
    let a = bytes_a(); let b = bytes_b(); let d = sha(&a);
    let src_a = fx.source("core-a", &a);
    hooks::FAIL_WRITE.with(|c| c.set(true));
    let e = prepare(&fx.root, &Request { version: V1, expected_sha256: &d, source: &src_a }).unwrap_err();
    hooks::reset();
    assert_eq!(e.stage, Stage::StoreRecord); assert_eq!(e.created, vec!["<root>".to_string(), "store.json".into()]);
    assert!(matches!(e.after.as_ref().unwrap().state, State::Damaged { .. }), "{e:?}");
    let fx = Fixture::new("io2");
    let src_a = fx.source("core-a", &a); let src_b = fx.source("core-b", &b);
    prepare(&fx.root, &Request { version: V2, expected_sha256: &sha(&b), source: &src_b }).unwrap();
    hooks::FAIL_WRITE.with(|c| c.set(true));
    let e = prepare(&fx.root, &Request { version: V1, expected_sha256: &d, source: &src_a }).unwrap_err();
    hooks::reset();
    assert_eq!(e.stage, Stage::Copy); assert!(e.reason.contains("write"), "{e:?}");
    assert_eq!(e.created, vec![reserve(V1), vdir(V1, &d), format!("{}/{CORE_FILE}", vdir(V1, &d))]);
    assert_eq!(e.remaining, e.created);
    assert!(!fx.has(&format!("{}/{COMPLETE_FILE}", vdir(V1, &d))));
    assert_eq!(e.after.as_ref().unwrap().state, State::Prepared);
    let c = b"synthetic core C\n".repeat(100); let src_c = fx.source("core-c", &c);
    hooks::FAIL_FLUSH.with(|c| c.set(true));
    let e = prepare(&fx.root, &Request { version: "0.3.0", expected_sha256: &sha(&c), source: &src_c }).unwrap_err();
    hooks::reset();
    assert!(matches!(e.stage, Stage::Reservation | Stage::VersionDir | Stage::Copy) && e.reason.contains("flush"), "{e:?}");
    assert!(!fx.has(&format!("{}/{COMPLETE_FILE}", vdir("0.3.0", &sha(&c)))));
    let wrong = sha(b"other");
    let e = prepare(&fx.root, &Request { version: "0.4.0", expected_sha256: &wrong, source: &src_a }).unwrap_err();
    assert_eq!(e.stage, Stage::Verify); assert!(e.reason.contains("differs"), "{e:?}");
    assert!(fx.has(&format!("{}/{CORE_FILE}", vdir("0.4.0", &wrong))) && !fx.has(&format!("{}/{COMPLETE_FILE}", vdir("0.4.0", &wrong))));
    assert_eq!(e.remaining, e.created);
    let e = prepare(&fx.root, &Request { version: "0.4.0", expected_sha256: &d, source: &src_a }).unwrap_err();
    assert_eq!(e.stage, Stage::Uniqueness); assert!(e.reason.contains("different bytes"), "{e:?}");
    assert!(e.created.is_empty());
}
#[test]
fn two_prepares_for_the_same_target_never_overwrite_each_other() {
    let fx = Fixture::new("race");
    let a = bytes_a(); let d = sha(&a);
    let src = fx.source("core-a", &a);
    let b = bytes_b(); let src_b = fx.source("core-b", &b);
    prepare(&fx.root, &Request { version: V2, expected_sha256: &sha(&b), source: &src_b }).unwrap();
    let results: Vec<Result<Report, Refusal>> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..4).map(|_| s.spawn(|| prepare(&fx.root, &Request { version: V1, expected_sha256: &d, source: &src }))).collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let prepared = results.iter().filter(|r| matches!(r, Ok(Report { outcome: Outcome::Prepared, .. }))).count();
    assert_eq!(prepared, 1, "{results:?}");
    for r in &results {
        match r {
            Ok(Report { outcome: Outcome::Prepared, .. }) => {}
            Ok(Report { outcome: Outcome::AlreadyPrepared, created, .. }) => assert!(created.is_empty()),
            // Observe: saw the winner's writes during observation; Reservation: lost the
            // reservation; Uniqueness: saw the winner's incomplete or complete directory;
            // VersionDir: lost the exclusive creation. Nothing of the loser remains.
            Err(e) => { assert!(e.remaining.is_empty(), "{e:?}"); assert!(matches!(e.stage, Stage::Observe | Stage::Reservation | Stage::Uniqueness | Stage::VersionDir), "{e:?}"); }
        }
    }
    assert_eq!(fs::read(fx.root.join(vdir(V1, &d)).join(CORE_FILE)).unwrap(), a);
    assert_eq!(fx.complete_dirs_of(V1), 1);
    let (o, v) = observe_and_judge(&fx.root);
    assert!(o.observation.blocked.is_empty(), "{o:?}"); assert_eq!(v.state, State::Prepared);
    // first-use race: exactly one caller creates the root; the version may then be completed by
    // the initializer or by a caller that observed the freshly initialized store, never twice
    let fx = Fixture::new("race-init");
    let src = fx.source("core-a", &a);
    let results: Vec<Result<Report, Refusal>> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..4).map(|_| s.spawn(|| prepare(&fx.root, &Request { version: V1, expected_sha256: &d, source: &src }))).collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let created_of = |r: &Result<Report, Refusal>| match r { Ok(rep) => rep.created.clone(), Err(e) => e.created.clone() };
    let inits = results.iter().filter(|r| created_of(r).first().map(String::as_str) == Some("<root>")).count();
    assert_eq!(inits, 1, "{results:?}");
    let prepared = results.iter().filter(|r| matches!(r, Ok(Report { outcome: Outcome::Prepared, .. }))).count();
    assert_eq!(prepared, 1, "{results:?}");
    for r in &results {
        if let Err(e) = r {
            // the shared versions/ container may also have been created by a loser that observed
            // the freshly initialized store; it is not a partial preparation
            let allowed: &[&[&str]] = &[&[], &["versions"], &["<root>", "store.json"], &["<root>", "store.json", "versions"]];
            assert!(allowed.iter().any(|a| e.remaining.iter().map(String::as_str).eq(a.iter().copied())), "{e:?}");
        }
    }
    let (o, v) = observe_and_judge(&fx.root);
    assert!(o.observation.blocked.is_empty(), "{o:?}"); assert_eq!(v.state, State::Prepared);
    assert_eq!(fs::read(fx.root.join(vdir(V1, &d)).join(CORE_FILE)).unwrap(), a);
}
static GATE: Mutex<Option<Arc<Barrier>>> = Mutex::new(None);
fn wait_at_gate(_root: &Path) { let gate = GATE.lock().unwrap().clone(); if let Some(b) = gate { b.wait(); } }
#[test]
fn the_same_version_with_two_digests_at_once_yields_at_most_one_complete_preparation() {
    let fx = Fixture::new("race-digest");
    let a = bytes_a(); let b = bytes_b();
    let src_a = fx.source("core-a", &a); let src_b = fx.source("core-b", &b);
    let c = b"synthetic core C\n".repeat(100); let src_c = fx.source("core-c", &c);
    prepare(&fx.root, &Request { version: V2, expected_sha256: &sha(&c), source: &src_c }).unwrap();
    for round in 0..3 {
        let fx = if round == 0 { &fx } else { &fx };
        *GATE.lock().unwrap() = Some(Arc::new(Barrier::new(2)));
        let version = format!("1.{round}.0");
        let results: Vec<Result<Report, Refusal>> = std::thread::scope(|s| {
            let ha = s.spawn(|| { hooks::BEFORE_RESERVATION.with(|h| h.set(Some(wait_at_gate))); let r = prepare(&fx.root, &Request { version: &version, expected_sha256: &sha(&a), source: &src_a }); hooks::reset(); r });
            let hb = s.spawn(|| { hooks::BEFORE_RESERVATION.with(|h| h.set(Some(wait_at_gate))); let r = prepare(&fx.root, &Request { version: &version, expected_sha256: &sha(&b), source: &src_b }); hooks::reset(); r });
            vec![ha.join().unwrap(), hb.join().unwrap()]
        });
        *GATE.lock().unwrap() = None;
        // both passed the fast-path scan with nothing found; the reservation decides
        let prepared = results.iter().filter(|r| matches!(r, Ok(Report { outcome: Outcome::Prepared, .. }))).count();
        assert_eq!(prepared, 1, "{results:?}");
        let loser = results.iter().find_map(|r| r.as_ref().err()).expect("one loser");
        assert_eq!(loser.stage, Stage::Reservation, "{loser:?}");
        assert!(loser.created.is_empty(), "{loser:?}");
        assert_eq!(fx.complete_dirs_of(&version), 1, "two complete preparations of one version");
        assert!(fx.has(&reserve(&version)), "the winner's reservation stays");
    }
    let (o, v) = observe_and_judge(&fx.root);
    assert!(o.observation.blocked.is_empty(), "{o:?}"); assert_eq!(v.state, State::Prepared);
}
#[test]
fn a_swapped_versions_directory_is_refused_at_reobservation_and_everything_is_reported() {
    let fx = Fixture::new("swap");
    let a = bytes_a(); let d = sha(&a);
    let src = fx.source("core-a", &a);
    let b = bytes_b(); let src_b = fx.source("core-b", &b);
    prepare(&fx.root, &Request { version: V2, expected_sha256: &sha(&b), source: &src_b }).unwrap();
    SWAP_REFUSED.with(|c| c.set(false));
    hooks::AFTER_VERSION_DIR.with(|c| c.set(Some(swap_versions_for_link)));
    let r = prepare(&fx.root, &Request { version: V1, expected_sha256: &d, source: &src });
    hooks::reset();
    if SWAP_REFUSED.with(|c| c.get()) {
        // Windows refuses to rename versions/ while a handle is open beneath it: the swap never
        // happened, so the preparation must simply complete (branch printed for the log).
        println!("OBSERVED versions/ swap: the OS refused the rename while a handle was held beneath it; no swap happened");
        let r = r.unwrap();
        assert_eq!(r.outcome, Outcome::Prepared); assert_all_same(&r.objects);
        assert!(fx.has(&format!("{}/{COMPLETE_FILE}", vdir(V1, &d))));
        return;
    }
    let e = r.unwrap_err();
    assert_refused_by_name_check(&e, "versions/ swapped for a link");
    assert_eq!(e.created, vec![reserve(V1), vdir(V1, &d), format!("{}/{CORE_FILE}", vdir(V1, &d)), format!("{}/{COMPLETE_FILE}", vdir(V1, &d))]);
    assert_all_same(&e.objects);
    assert!(fx.base.join("versions-moved").join(version_dirname(V1, &d)).join(COMPLETE_FILE).exists());
    assert!(!fx.base.join("elsewhere").join(version_dirname(V1, &d)).exists());
    assert!(matches!(e.after.as_ref().unwrap().state, State::Blocked { .. }), "{e:?}");
}
#[test]
fn created_objects_are_reported_by_identity_not_by_name() {
    let fx = Fixture::new("presence");
    let a = bytes_a(); let d = sha(&a);
    let src = fx.source("core-a", &a);
    let b = bytes_b(); let src_b = fx.source("core-b", &b);
    prepare(&fx.root, &Request { version: V2, expected_sha256: &sha(&b), source: &src_b }).unwrap();
    // (a) the version directory is renamed away and another directory planted under its name
    //     after creation: the copy lands in the held (moved) directory, the name shows Replaced
    hooks::AFTER_VERSION_DIR.with(|c| c.set(Some(replace_version_dir_under_its_name)));
    let e = prepare(&fx.root, &Request { version: V1, expected_sha256: &d, source: &src }).unwrap_err();
    hooks::reset();
    assert_refused_by_name_check(&e, "version directory replaced under its name");
    assert_eq!(*presence_of(&e.objects, &vdir(V1, &d)), Presence::Replaced, "{e:?}");
    assert_eq!(*presence_of(&e.objects, &format!("{}/{CORE_FILE}", vdir(V1, &d))), Presence::SameObject);
    assert_eq!(*presence_of(&e.objects, &format!("{}/{COMPLETE_FILE}", vdir(V1, &d))), Presence::SameObject);
    assert_eq!(*presence_of(&e.objects, &reserve(V1)), Presence::SameObject);
    assert!(!e.remaining.contains(&vdir(V1, &d)) && e.remaining.contains(&format!("{}/{COMPLETE_FILE}", vdir(V1, &d))), "{e:?}");
    assert!(fx.base.join("moved-vdir").join(COMPLETE_FILE).exists() && !fx.root.join(vdir(V1, &d)).join(COMPLETE_FILE).exists());
    // (b) a created object taken away from under its name (no replacement): Absent, reported
    //     rather than silently dropped; the copy still lands in the held directory and the final
    //     name re-check refuses
    let fx = Fixture::new("presence-absent");
    let src = fx.source("core-a", &a);
    hooks::AFTER_VERSION_DIR.with(|c| c.set(Some(move_version_dir_away)));
    let e = prepare(&fx.root, &Request { version: V1, expected_sha256: &d, source: &src }).unwrap_err();
    hooks::reset();
    assert_refused_by_name_check(&e, "version directory moved away");
    assert_eq!(*presence_of(&e.objects, &vdir(V1, &d)), Presence::Absent, "{e:?}");
    assert_eq!(*presence_of(&e.objects, &format!("{}/{COMPLETE_FILE}", vdir(V1, &d))), Presence::SameObject);
    assert!(e.created.contains(&vdir(V1, &d)) && !e.remaining.contains(&vdir(V1, &d)));
    assert!(fx.base.join("moved-vdir").join(COMPLETE_FILE).exists());
    // (c) the reporting duplicate fails: every creation is still listed, presence is Unknown
    let fx = Fixture::new("presence-dup");
    let src = fx.source("core-a", &a);
    hooks::FAIL_NOTE_DUP.with(|c| c.set(true));
    let r = prepare(&fx.root, &Request { version: V1, expected_sha256: &d, source: &src }).unwrap();
    hooks::reset();
    assert_eq!(r.created, full_first_run(&d));
    for o in &r.objects { assert!(matches!(o.presence, Presence::Unknown(_)), "{o:?}"); }
    assert!(fx.has(&format!("{}/{COMPLETE_FILE}", vdir(V1, &d))) && fx.has("store.json"));
    #[cfg(unix)]
    {
        // (d) a lookup refused after creation: Unknown with the error, never Absent
        use std::os::unix::fs::PermissionsExt;
        if (unsafe { libc::geteuid() }) != 0 {
            let fx = Fixture::new("presence-denied");
            let src = fx.source("core-a", &a);
            prepare(&fx.root, &Request { version: V2, expected_sha256: &sha(&b), source: &fx.source("core-b", &b) }).unwrap();
            hooks::AFTER_VERSION_DIR.with(|c| c.set(Some(deny_versions_lookup)));
            let e = prepare(&fx.root, &Request { version: V1, expected_sha256: &d, source: &src }).unwrap_err();
            hooks::reset();
            fs::set_permissions(fx.root.join(VERSIONS_DIR), fs::Permissions::from_mode(0o700)).unwrap();
            assert!(matches!(presence_of(&e.objects, &vdir(V1, &d)), Presence::Unknown(m) if m.contains("lookup failed")), "{e:?}");
            assert!(matches!(presence_of(&e.objects, &reserve(V1)), Presence::Unknown(_)), "{e:?}");
            assert_eq!(*presence_of(&e.objects, &format!("{}/{CORE_FILE}", vdir(V1, &d))), Presence::SameObject);
            assert!(!e.remaining.contains(&vdir(V1, &d)) && e.created.contains(&vdir(V1, &d)));
            assert!(fx.has(&reserve(V1)) && fx.has(&format!("{}/{COMPLETE_FILE}", vdir(V1, &d))), "real state: reservation kept, complete written into the held directory");
        }
    }
}
#[test]
fn the_reservation_is_never_removed_and_must_still_be_this_calls_object_at_the_end() {
    // The release path does not exist: no unlink/delete of any object in the layer's source.
    const SRC: &str = include_str!("runtime_prepare.rs");
    for forbidden in ["unlink_at", "win::delete", "remove_file", "remove_dir", "release_reservation", "Released"] {
        assert!(!SRC.contains(forbidden), "{forbidden} must not appear in runtime_prepare.rs");
    }
    let fx = Fixture::new("reservation");
    let a = bytes_a(); let d = sha(&a); let src = fx.source("core-a", &a);
    let b = bytes_b(); let src_b = fx.source("core-b", &b);
    // success: the reservation stays and is reported as the same object; the verified fast path
    // still answers AlreadyPrepared next to it
    let r = prepare(&fx.root, &Request { version: V1, expected_sha256: &d, source: &src }).unwrap();
    assert_eq!(*presence_of(&r.objects, &reserve(V1)), Presence::SameObject);
    assert!(fx.has(&reserve(V1)));
    let r2 = prepare(&fx.root, &Request { version: V1, expected_sha256: &d, source: &src }).unwrap();
    assert_eq!(r2.outcome, Outcome::AlreadyPrepared); assert!(r2.created.is_empty()); assert!(fx.has(&reserve(V1)));
    // a different digest of the same version: refused by the directory scan, reservation untouched
    let e = prepare(&fx.root, &Request { version: V1, expected_sha256: &sha(&b), source: &src_b }).unwrap_err();
    assert_eq!(e.stage, Stage::Uniqueness); assert!(fx.has(&reserve(V1)));
    // a refusal under this call's fresh reservation keeps that reservation (reported, same object)
    hooks::AFTER_RESERVED.with(|c| c.set(Some(plant_other_digest_dir)));
    let e = prepare(&fx.root, &Request { version: V2, expected_sha256: &sha(&b), source: &src_b }).unwrap_err();
    hooks::reset();
    assert_eq!(e.stage, Stage::Uniqueness, "{e:?}"); assert!(e.reason.contains("different bytes"), "{e:?}");
    assert_eq!(e.created, vec![reserve(V2)]); assert_eq!(e.remaining, e.created);
    assert_eq!(*presence_of(&e.objects, &reserve(V2)), Presence::SameObject);
    assert!(fx.has(&reserve(V2)) && !fx.has(&vdir(V2, &sha(&b))));
    // the reservation renamed away after the version directory was created: the preparation
    // finishes its writes under held handles, then refuses because the version is no longer
    // serialized; everything written is kept and reported (reservation Absent)
    let fx = Fixture::new("reservation-vanish");
    let src = fx.source("core-a", &a);
    let b3 = b"synthetic core three
".repeat(50); let src3 = fx.source("core-3", &b3);
    prepare(&fx.root, &Request { version: V2, expected_sha256: &sha(&b3), source: &src3 }).unwrap();
    hooks::AFTER_VERSION_DIR.with(|c| c.set(Some(rename_reservation_away)));
    let e = prepare(&fx.root, &Request { version: V1, expected_sha256: &d, source: &src }).unwrap_err();
    hooks::reset();
    assert_eq!(e.stage, Stage::Reobserve, "{e:?}"); assert!(e.reason.contains("vanished"), "{e:?}");
    assert_eq!(*presence_of(&e.objects, &reserve(V1)), Presence::Absent);
    assert!(!e.remaining.contains(&reserve(V1)) && e.created.contains(&reserve(V1)));
    assert!(fx.has(&format!("{}/{COMPLETE_FILE}", vdir(V1, &d))), "written objects are kept");
    assert!(fx.base.join("reservation-moved").exists(), "nothing deleted the moved reservation either");
    // the reservation replaced under its name: Replaced, refused
    let fx = Fixture::new("reservation-replace");
    let src = fx.source("core-a", &a);
    let src3 = fx.source("core-3", &b3);
    prepare(&fx.root, &Request { version: V2, expected_sha256: &sha(&b3), source: &src3 }).unwrap();
    hooks::AFTER_VERSION_DIR.with(|c| c.set(Some(replace_reservation_under_its_name)));
    let e = prepare(&fx.root, &Request { version: V1, expected_sha256: &d, source: &src }).unwrap_err();
    hooks::reset();
    assert_eq!(e.stage, Stage::Reobserve, "{e:?}"); assert!(e.reason.contains("replaced"), "{e:?}");
    assert_eq!(*presence_of(&e.objects, &reserve(V1)), Presence::Replaced);
    assert!(fx.has(&reserve(V1)), "the planted file is not deleted");
}
#[cfg(unix)]
#[test]
fn a_linked_source_is_refused() {
    let fx = Fixture::new("source-link");
    let a = bytes_a(); let d = sha(&a);
    let real = fx.source("core-a", &a);
    let link = fx.base.join("core-link"); std::os::unix::fs::symlink(&real, &link).unwrap();
    let e = prepare(&fx.root, &Request { version: V1, expected_sha256: &d, source: &link }).unwrap_err();
    assert_eq!(e.stage, Stage::Copy); assert!(e.reason.contains("symlink"), "{e:?}");
    assert!(e.created.iter().all(|c| !c.ends_with(CORE_FILE)), "{e:?}");
    fs::hard_link(&real, fx.base.join("core-hard")).unwrap();
    let e = prepare(&fx.root, &Request { version: V2, expected_sha256: &d, source: &real }).unwrap_err();
    assert_eq!(e.stage, Stage::Copy); assert!(e.reason.contains("one link"), "{e:?}");
}

// ───────────── platform-specific fixture helpers ─────────────
fn plant_other_digest_dir(root: &Path) {
    let dir = root.join(VERSIONS_DIR).join(version_dirname(V2, &"f".repeat(64)));
    fs::create_dir(&dir).unwrap();
    #[cfg(unix)]
    { use std::os::unix::fs::PermissionsExt; fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap(); }
}
fn rename_reservation_away(root: &Path) {
    fs::rename(root.join(VERSIONS_DIR).join(reservation_name(V1)), root.parent().unwrap().join("reservation-moved")).unwrap();
}
fn replace_reservation_under_its_name(root: &Path) {
    let name = root.join(VERSIONS_DIR).join(reservation_name(V1));
    fs::rename(&name, root.parent().unwrap().join("reservation-moved")).unwrap();
    fs::write(&name, b"planted").unwrap();
}
fn replace_version_dir_under_its_name(root: &Path) {
    let versions = root.join(VERSIONS_DIR);
    let created = fs::read_dir(&versions).unwrap().flatten().map(|e| e.path()).find(|p| p.is_dir() && p.file_name().unwrap().to_string_lossy().starts_with(&format!("{V1}-"))).unwrap();
    let moved = root.parent().unwrap().join("moved-vdir");
    fs::rename(&created, &moved).unwrap();
    fs::create_dir(&created).unwrap();
    #[cfg(unix)]
    { use std::os::unix::fs::PermissionsExt; fs::set_permissions(&created, fs::Permissions::from_mode(0o700)).unwrap(); }
}
fn move_version_dir_away(root: &Path) {
    let versions = root.join(VERSIONS_DIR);
    let created = fs::read_dir(&versions).unwrap().flatten().map(|e| e.path()).find(|p| p.is_dir() && p.file_name().unwrap().to_string_lossy().starts_with(&format!("{V1}-"))).unwrap();
    fs::rename(&created, root.parent().unwrap().join("moved-vdir")).unwrap();
}
#[cfg(unix)]
fn deny_versions_lookup(root: &Path) { use std::os::unix::fs::PermissionsExt; fs::set_permissions(root.join(VERSIONS_DIR), fs::Permissions::from_mode(0o000)).unwrap(); }
#[cfg(unix)]
fn make_protected_root(root: &Path) { use std::os::unix::fs::PermissionsExt; fs::create_dir(root).unwrap(); fs::set_permissions(root, fs::Permissions::from_mode(0o700)).unwrap(); }
#[cfg(unix)]
fn break_root_protection(root: &Path) { use std::os::unix::fs::PermissionsExt; fs::set_permissions(root, fs::Permissions::from_mode(0o755)).unwrap(); }
#[cfg(unix)]
fn break_protection(path: &Path) { use std::os::unix::fs::PermissionsExt; fs::set_permissions(path, fs::Permissions::from_mode(0o644)).unwrap(); }
#[cfg(unix)]
fn restore_protection(path: &Path, executable: bool) { use std::os::unix::fs::PermissionsExt; fs::set_permissions(path, fs::Permissions::from_mode(if executable { 0o500 } else { 0o600 })).unwrap(); }
#[cfg(unix)]
fn swap_versions_for_link(root: &Path) {
    let v = root.join(VERSIONS_DIR); let base = root.parent().unwrap().to_path_buf();
    fs::rename(&v, base.join("versions-moved")).unwrap();
    fs::create_dir(base.join("elsewhere")).unwrap();
    std::os::unix::fs::symlink(base.join("elsewhere"), &v).unwrap();
}
#[cfg(windows)]
fn icacls(args: &[&str], target: &Path) {
    let icacls = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/icacls.exe");
    let out = std::process::Command::new(icacls).arg(target).args(args).output().unwrap();
    assert!(out.status.success(), "icacls {args:?}: {} {}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}
#[cfg(windows)]
fn make_protected_root(root: &Path) {
    use crate::windows_fs as win;
    let parent = win::open_dir(root.parent().unwrap(), win::SHARE_ALL, false).unwrap();
    let made = win::create_private_dir_at(&parent, root.file_name().unwrap()).unwrap();
    win::private_dir(&made).unwrap();
}
#[cfg(windows)]
fn break_root_protection(root: &Path) { icacls(&["/inheritance:e"], root); }
/// An extra explicit ACE: the DACL is no longer the two inherited ACEs (icacls also writes
/// explicit copies of the inherited ones; `/reset` restores the inherited-only state).
#[cfg(windows)]
fn break_protection(path: &Path) { icacls(&["/grant", "*S-1-1-0:(R)"], path); }
#[cfg(windows)]
fn restore_protection(path: &Path, _executable: bool) { icacls(&["/reset"], path); }
#[cfg(windows)]
fn swap_versions_for_link(root: &Path) {
    let v = root.join(VERSIONS_DIR); let base = root.parent().unwrap().to_path_buf();
    if let Err(e) = fs::rename(&v, base.join("versions-moved")) {
        // CI run 39873558592: "Access is denied" — the OS refuses to rename a directory with an
        // open handle beneath it. Recorded; the test asserts the no-swap outcome.
        println!("OBSERVED versions/ rename refused by the OS: {e}");
        SWAP_REFUSED.with(|c| c.set(true));
        return;
    }
    fs::create_dir(base.join("elsewhere")).unwrap();
    let powershell = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let made = std::process::Command::new(powershell)
        .args(["-NoProfile", "-NonInteractive", "-Command", "$ErrorActionPreference='Stop'; New-Item -ItemType Junction -Path $env:GIL_TEST_LINK -Target $env:GIL_TEST_TARGET | Out-Null"])
        .env("GIL_TEST_LINK", &v).env("GIL_TEST_TARGET", base.join("elsewhere")).output().unwrap();
    assert!(made.status.success(), "junction fixture creation failed: {}", String::from_utf8_lossy(&made.stderr));
}
