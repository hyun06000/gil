//! Fixture tests for the selection transition. Stores and candidates are built by the
//! preparation layer in fresh isolated temp trees; the synthetic Core bytes are never executed
//! (the layer has no process-spawning code, asserted below).
use super::hooks;
use super::*;
use crate::runtime_prepare::{self as prep, Outcome, Request};
use crate::runtime_state::{Run, State};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

const V1: &str = "0.2.1-dev.a";
const V2: &str = "0.2.2-dev.b";
const V3: &str = "0.2.3-dev.c";
fn sha(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
fn bytes_a() -> Vec<u8> { b"synthetic core A (never executed)\n".repeat(2000) }
fn bytes_b() -> Vec<u8> { b"synthetic core B (never executed)\n".repeat(2000) }
fn bytes_c() -> Vec<u8> { b"synthetic core C (never executed)\n".repeat(2000) }

struct Fixture { base: PathBuf, root: PathBuf }
impl Fixture {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let tmp = std::env::temp_dir();
        #[cfg(unix)]
        let tmp = fs::canonicalize(tmp).unwrap();
        let base = tmp.join(format!("gil-select-{label}-{}-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir_all(&base).unwrap();
        #[cfg(unix)]
        { use std::os::unix::fs::PermissionsExt; fs::set_permissions(&base, fs::Permissions::from_mode(0o700)).unwrap(); }
        Self { root: base.join("runtime-v1"), base }
    }
    /// A prepared store with V1 (bytes A) and V2 (bytes B); returns their digests.
    fn prepared(label: &str) -> (Self, String, String) {
        let fx = Self::new(label);
        let (a, b) = (bytes_a(), bytes_b());
        let src_a = fx.base.join("core-a"); fs::write(&src_a, &a).unwrap();
        let src_b = fx.base.join("core-b"); fs::write(&src_b, &b).unwrap();
        assert_eq!(prep::prepare(&fx.root, &Request { version: V1, expected_sha256: &sha(&a), source: &src_a }).unwrap().outcome, Outcome::Prepared);
        assert_eq!(prep::prepare(&fx.root, &Request { version: V2, expected_sha256: &sha(&b), source: &src_b }).unwrap().outcome, Outcome::Prepared);
        (fx, sha(&a), sha(&b))
    }
    fn read(&self, rel: &str) -> Option<Vec<u8>> { fs::read(self.root.join(rel)).ok() }
    fn has(&self, rel: &str) -> bool { self.root.join(rel).exists() }
    fn generation_in(&self, rel: &str) -> Option<u64> { let v: serde_json::Value = serde_json::from_slice(&self.read(rel)?).ok()?; v.get("generation").or(v.get("last_generation"))?.as_u64() }
    /// Bytes and (Unix) modes of everything below versions/.
    fn versions_snapshot(&self) -> BTreeMap<String, (u64, String, u32)> {
        fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, (u64, String, u32)>) {
            let Ok(rd) = fs::read_dir(dir) else { return };
            for e in rd {
                let p = e.unwrap().path();
                let m = fs::symlink_metadata(&p).unwrap();
                let content = if m.is_file() { sha(&fs::read(&p).unwrap_or_default()) } else { String::new() };
                #[cfg(unix)]
                let mode = { use std::os::unix::fs::PermissionsExt; m.permissions().mode() & 0o7777 };
                #[cfg(not(unix))]
                let mode = 0;
                out.insert(p.strip_prefix(base).unwrap().display().to_string().replace('\\', "/"), (m.len(), content, mode));
                if m.is_dir() { walk(base, &p, out); }
            }
        }
        let mut out = BTreeMap::new();
        walk(&self.root, &self.root.join(VERSIONS_DIR), &mut out);
        out
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        hooks::reset(); prep::hooks::reset();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fn relax(p: &Path) { if let Ok(rd) = fs::read_dir(p) { for e in rd.flatten() { let q = e.path(); if fs::symlink_metadata(&q).map(|m| m.file_type().is_symlink()).unwrap_or(true) { continue; } let _ = fs::set_permissions(&q, fs::Permissions::from_mode(0o700)); if q.is_dir() { relax(&q); } } } }
            relax(&self.base);
        }
        let _ = fs::remove_dir_all(&self.base);
    }
}
fn req<'a>(version: &'a str, sha256: &'a str, expected_generation: u64) -> SelectRequest<'a> { SelectRequest { version, sha256, expected_generation } }
fn selected(g: u64, v: &str) -> State { State::Selected { generation: g, version: v.into(), history_lagging: false } }
/// The report's progress must equal what the files say.
fn assert_progress_matches_disk(fx: &Fixture, progress: Progress, generation: u64) {
    assert_eq!(progress.intent_recorded, fx.generation_in("intent.json") == Some(generation), "intent_recorded vs disk");
    assert_eq!(progress.published, fx.generation_in("selection.json") == Some(generation), "published vs disk");
    assert_eq!(progress.history_updated, fx.generation_in("store.json") == Some(generation), "history_updated vs disk");
}

#[test]
fn first_selection_then_the_next_generation_follow_the_contract() {
    let (fx, da, db) = Fixture::prepared("first");
    let versions_before = fx.versions_snapshot();
    let r = select(&fx.root, &req(V1, &da, 0)).unwrap();
    assert_eq!(r.generation, 1);
    assert_eq!(r.before.state, State::Prepared);
    assert_eq!(r.after.state, selected(1, V1)); assert_eq!(r.after.run, Run::Selected);
    assert!(r.after.interrupted.iter().any(|t| t.contains("intent_pending_cleanup")), "{:?}", r.after);
    assert_eq!(r.progress, Progress { intent_recorded: true, published: true, history_updated: true, intent_kept: true });
    assert_eq!(r.created, vec!["intent.json (replaced by this call)", "selection.json (replaced by this call)", "store.json (replaced by this call)"]);
    for o in &r.objects { assert_eq!(o.presence, Presence::SameObject, "{o:?}"); }
    assert_progress_matches_disk(&fx, r.progress, 1);
    assert!(fx.has(runtime_lock::LOCK_FILE) && !fx.has(INTENT_NEW) && !fx.has(SELECTION_NEW) && !fx.has(STORE_NEW));
    let sel: serde_json::Value = serde_json::from_slice(&fx.read("selection.json").unwrap()).unwrap();
    assert_eq!(sel["version"], V1); assert_eq!(sel["sha256"], da.as_str()); assert_eq!(sel["generation"], 1);
    // next generation: the kept intent is replaced by rename, history moves to 2
    let r2 = select(&fx.root, &req(V2, &db, 1)).unwrap();
    assert_eq!(r2.generation, 2); assert_eq!(r2.before.state, selected(1, V1)); assert_eq!(r2.after.state, selected(2, V2));
    assert_progress_matches_disk(&fx, r2.progress, 2);
    assert_eq!(fx.generation_in("intent.json"), Some(2));
    // the observer agrees and nothing below versions/ changed (bytes, sizes, modes)
    let (o, v) = observe_and_judge(&fx.root);
    assert!(o.observation.blocked.is_empty(), "{o:?}"); assert_eq!(v.state, selected(2, V2));
    assert_eq!(fx.versions_snapshot(), versions_before, "candidate bytes or permissions changed");
    assert!(fx.has(runtime_lock::LOCK_FILE), "the lock file is never deleted");
}
#[test]
fn requests_bind_version_digest_and_generation_and_refuse_boundaries() {
    let (fx, da, db) = Fixture::prepared("bind");
    let unchanged = |fx: &Fixture| (fx.read("selection.json"), fx.read("store.json"), fx.read("intent.json"));
    let before = unchanged(&fx);
    // overflow: no generation after u64::MAX
    let e = select(&fx.root, &req(V1, &da, u64::MAX)).unwrap_err();
    assert_eq!(e.stage, SelectStage::Request); assert!(e.reason.contains("overflow"), "{e:?}");
    // bad identifiers
    assert_eq!(select(&fx.root, &req("../x", &da, 0)).unwrap_err().stage, SelectStage::Request);
    assert_eq!(select(&fx.root, &req(V1, "ABC", 0)).unwrap_err().stage, SelectStage::Request);
    // generation conflict on a prepared store (history is 0)
    let e = select(&fx.root, &req(V1, &da, 3)).unwrap_err();
    assert_eq!(e.stage, SelectStage::Observe); assert!(e.reason.contains("generation conflict"), "{e:?}");
    // candidate not prepared with this digest / this version
    let e = select(&fx.root, &req(V1, &db, 0)).unwrap_err();
    assert_eq!(e.stage, SelectStage::Verify); assert!(e.reason.contains("not prepared"), "{e:?}");
    let e = select(&fx.root, &req(V3, &da, 0)).unwrap_err();
    assert_eq!(e.stage, SelectStage::Verify);
    assert_eq!(unchanged(&fx), before, "a refusal must not touch the records");
    assert_eq!(e.progress, Progress::default());
    // incomplete candidate (preparation interrupted before complete) is not selectable
    let c = bytes_c(); let src_c = fx.base.join("core-c"); fs::write(&src_c, &c).unwrap();
    prep::hooks::FAIL_AFTER.with(|h| h.set(Some(prep::Point::Verified)));
    prep::prepare(&fx.root, &Request { version: V3, expected_sha256: &sha(&c), source: &src_c }).unwrap_err();
    prep::hooks::reset();
    let e = select(&fx.root, &req(V3, &sha(&c), 0)).unwrap_err();
    assert_eq!(e.stage, SelectStage::Verify); assert!(e.reason.contains("incomplete"), "{e:?}");
    // a candidate whose protection fails is not selectable, and is not modified
    let core = fx.root.join(VERSIONS_DIR).join(version_dirname(V2, &db)).join(observe::CORE_FILE);
    break_protection(&core);
    let e = select(&fx.root, &req(V2, &db, 0)).unwrap_err();
    assert_eq!(e.stage, SelectStage::Verify); assert!(e.reason.contains("not selectable"), "{e:?}");
    restore_protection(&core);
    // after a first selection, the old generation is refused (stale expected_generation)
    select(&fx.root, &req(V1, &da, 0)).unwrap();
    let e = select(&fx.root, &req(V2, &db, 0)).unwrap_err();
    assert_eq!(e.stage, SelectStage::Observe); assert!(e.reason.contains("generation conflict"), "{e:?}");
    assert_eq!(fx.generation_in("selection.json"), Some(1));
    // stores that are not prepared/selected are refused before the lock
    let fresh = Fixture::new("bind-fresh");
    let e = select(&fresh.root, &req(V1, &da, 0)).unwrap_err();
    assert_eq!(e.stage, SelectStage::Observe); assert!(!fresh.root.exists());
}
#[test]
fn a_held_lock_refuses_selection_and_blocks_the_prepared_record_update() {
    let (fx, da, _) = Fixture::prepared("lock");
    let root_dir = platform::walk(&fx.root).unwrap().unwrap();
    let held = runtime_lock::acquire(&root_dir, Wait::NonBlocking).unwrap();
    let e = select(&fx.root, &req(V1, &da, 0)).unwrap_err();
    assert_eq!(e.stage, SelectStage::Lock); assert!(e.reason.contains("held"), "{e:?}");
    assert_eq!(e.progress, Progress::default()); assert!(e.created.is_empty());
    assert_eq!(fx.generation_in("store.json"), Some(0)); assert!(!fx.has("selection.json"));
    drop(held);
    assert_eq!(select(&fx.root, &req(V1, &da, 0)).unwrap().generation, 1);
    // the preparation layer's initializing → prepared record does not wait for the lock: it is
    // refused, the objects already created stay and are reported, and a later call completes it
    let fresh = Fixture::new("lock-prepare");
    let c = bytes_c(); let src_c = fresh.base.join("core-c"); fs::write(&src_c, &c).unwrap();
    thread_local! { static HELD: std::cell::RefCell<Option<runtime_lock::SelectionLock>> = const { std::cell::RefCell::new(None) }; }
    fn take_lock(root: &Path) {
        // runs right before the record step: the root exists and the version is complete
        let root_dir = platform::walk(root).unwrap().unwrap();
        HELD.with(|h| *h.borrow_mut() = Some(runtime_lock::acquire(&root_dir, Wait::NonBlocking).unwrap()));
    }
    prep::hooks::BEFORE_RECORD.with(|h| h.set(Some(take_lock)));
    let e = prep::prepare(&fresh.root, &Request { version: V3, expected_sha256: &sha(&c), source: &src_c }).unwrap_err();
    prep::hooks::reset();
    assert_eq!(e.stage, prep::Stage::PreparedRecord, "{e:?}"); assert!(e.reason.contains("held"), "{e:?}");
    assert!(!e.reason.contains("nothing changed"), "objects were created; the refusal must not claim an unchanged store: {e:?}");
    assert!(e.created.iter().any(|c| c.ends_with(observe::COMPLETE_FILE)) && e.created.contains(&"store.json".to_string()), "{e:?}");
    assert_eq!(e.remaining, e.created);
    let phase: serde_json::Value = serde_json::from_slice(&fresh.read("store.json").unwrap()).unwrap();
    assert_eq!(phase["phase"], "initializing", "record not replaced while the lock is held");
    assert!(!fresh.has(STORE_NEW));
    HELD.with(|h| *h.borrow_mut() = None); // release
    let r = prep::prepare(&fresh.root, &Request { version: V3, expected_sha256: &sha(&c), source: &src_c }).unwrap();
    assert_eq!(r.outcome, Outcome::AlreadyPrepared); assert_eq!(r.after.state, State::Prepared);
}
#[test]
fn the_prepared_record_re_read_and_replacement_share_one_lock_lifetime() {
    // Inside the record step, between the re-read and the replacement, no selection and no
    // second lock holder can get in: the lock is held across both (deterministic, no timing).
    let fx = Fixture::new("one-lock");
    let a = bytes_a(); let src_a = fx.base.join("core-a"); fs::write(&src_a, &a).unwrap();
    thread_local! { static SEEN: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) }; static DIGEST: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) }; }
    fn under_lock(root: &Path) {
        let root_dir = platform::walk(root).unwrap().unwrap();
        let second = runtime_lock::acquire(&root_dir, Wait::NonBlocking);
        SEEN.with(|s| s.borrow_mut().push(format!("second lock: {:?}", second.as_ref().err().map(|e| e.kind()))));
        let d = DIGEST.with(|d| d.borrow().clone());
        let s = select(root, &SelectRequest { version: V1, sha256: &d, expected_generation: 0 });
        SEEN.with(|v| v.borrow_mut().push(format!("select: {:?}", s.as_ref().err().map(|e| e.stage))));
        // the record is still `initializing` here, so the selection is already refused by its
        // pre-lock observation; had it reached the lock it would be refused there (WouldBlock)
        assert!(matches!(&s, Err(SelectRefusal { stage: SelectStage::Observe | SelectStage::Lock, progress, .. }) if *progress == Progress::default()), "a selection must be refused, unchanged, while the record step holds the lock: {s:?}");
        assert!(matches!(second, Err(ref e) if e.kind() == io::ErrorKind::WouldBlock), "the lock must be held across re-read and replacement: {second:?}");
    }
    DIGEST.with(|d| *d.borrow_mut() = sha(&a));
    prep::hooks::UNDER_RECORD_LOCK.with(|h| h.set(Some(under_lock)));
    let r = prep::prepare(&fx.root, &Request { version: V1, expected_sha256: &sha(&a), source: &src_a }).unwrap();
    prep::hooks::reset();
    let seen = SEEN.with(|s| s.borrow().clone());
    assert_eq!(seen, vec!["second lock: Some(WouldBlock)", "select: Some(Observe)"], "{seen:?}");
    assert_eq!(r.outcome, Outcome::Prepared); assert_eq!(r.after.state, State::Prepared);
    assert_eq!(fx.generation_in("store.json"), Some(0));
    // and a selection afterwards works on the record this call wrote
    assert_eq!(select(&fx.root, &req(V1, &sha(&a), 0)).unwrap().generation, 1);
    assert_eq!(fx.generation_in("store.json"), Some(1));
}
#[test]
fn a_root_swapped_at_its_path_after_locking_never_updates_the_held_store_from_another_stores_values() {
    let (fx, da, _) = Fixture::prepared("root-swap");
    let original_before = (fx.read("store.json"), fx.read("selection.json"));
    SWAP_REFUSED.with(|c| c.set(false));
    hooks::AFTER_LOCK.with(|h| h.set(Some(plant_other_store_at_root)));
    let r = select(&fx.root, &req(V1, &da, 0));
    hooks::reset();
    if SWAP_REFUSED.with(|c| c.get()) {
        println!("OBSERVED root swap: the OS refused to rename the root while handles were held beneath it; no swap happened");
        assert_eq!(r.unwrap().generation, 1);
        return;
    }
    let e = r.unwrap_err();
    assert_eq!(e.stage, SelectStage::Observe, "{e:?}"); assert!(e.reason.contains("root replaced at its path"), "{e:?}");
    assert_eq!(e.progress, Progress::default()); assert!(e.created.is_empty());
    // the original store (now at <base>/original) is untouched; the impostor at the path too
    let original = fx.base.join("original");
    assert_eq!((fs::read(original.join("store.json")).ok(), fs::read(original.join("selection.json")).ok()), original_before);
    assert!(!original.join("intent.json").exists() && !original.join(INTENT_NEW).exists());
    let impostor: serde_json::Value = serde_json::from_slice(&fx.read("store.json").unwrap()).unwrap();
    assert_eq!(impostor["last_generation"], 9); assert!(!fx.has("intent.json") && !fx.has("selection.json"));
}
#[test]
fn an_existing_lock_file_is_verified_but_never_modified() {
    let (fx, da, _) = Fixture::prepared("lockfile");
    let lock = fx.root.join(runtime_lock::LOCK_FILE);
    let root_dir = platform::walk(&fx.root).unwrap().unwrap();
    // created by the first acquisition with the data-file protection
    drop(runtime_lock::acquire(&root_dir, Wait::NonBlocking).unwrap());
    assert!(lock.exists());
    #[cfg(unix)]
    { use std::os::unix::fs::PermissionsExt; assert_eq!(fs::metadata(&lock).unwrap().permissions().mode() & 0o7777, 0o600); }
    fs::write(&lock, b"existing bytes").unwrap();
    // wrong protection: refused, and the file's bytes and protection are left exactly as found
    break_lock_protection(&lock);
    let before = lock_state(&lock);
    let e = runtime_lock::acquire(&root_dir, Wait::NonBlocking).unwrap_err();
    assert!(e.to_string().contains("protection"), "{e}");
    assert_eq!(lock_state(&lock), before, "an existing lock file must not be modified");
    let s = select(&fx.root, &req(V1, &da, 0)).unwrap_err();
    assert_eq!(s.stage, SelectStage::Lock); assert_eq!(lock_state(&lock), before);
    restore_lock_protection(&lock);
    // hard-linked: refused; the link target keeps its bytes and protection
    let target = fx.base.join("lock-target");
    fs::hard_link(&lock, &target).unwrap();
    let before = (lock_state(&lock), lock_state(&target));
    let e = runtime_lock::acquire(&root_dir, Wait::NonBlocking).unwrap_err();
    assert!(e.to_string().contains("link count"), "{e}");
    assert_eq!((lock_state(&lock), lock_state(&target)), before);
    fs::remove_file(&target).unwrap();
    #[cfg(unix)]
    {
        // a symlink in place of the lock file is refused (never followed)
        fs::rename(&lock, fx.base.join("lock-moved")).unwrap();
        std::os::unix::fs::symlink(fx.base.join("lock-moved"), &lock).unwrap();
        let e = runtime_lock::acquire(&root_dir, Wait::NonBlocking).unwrap_err();
        assert!(e.to_string().contains("symlink") || e.raw_os_error() == Some(libc::ELOOP) || e.to_string().contains("regular"), "{e}");
        fs::remove_file(&lock).unwrap(); fs::rename(fx.base.join("lock-moved"), &lock).unwrap();
    }
    assert!(runtime_lock::acquire(&root_dir, Wait::NonBlocking).is_ok());
    assert_eq!(fs::read(&lock).unwrap(), b"existing bytes", "acquiring never rewrites the file");
}
thread_local! { static SWAP_REFUSED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
fn lock_state(p: &Path) -> (Vec<u8>, u32, u64) {
    let m = fs::metadata(p).unwrap();
    #[cfg(unix)]
    let mode = { use std::os::unix::fs::PermissionsExt; m.permissions().mode() & 0o7777 };
    #[cfg(not(unix))]
    let mode = 0;
    (fs::read(p).unwrap(), mode, m.len())
}
fn plant_other_store_at_root(root: &Path) {
    let base = root.parent().unwrap().to_path_buf();
    if let Err(e) = fs::rename(root, base.join("original")) { println!("OBSERVED root rename refused by the OS: {e}"); SWAP_REFUSED.with(|c| c.set(true)); return; }
    make_protected_dir(root);
    fs::write(root.join("store.json"), crate::runtime_prepare::store_json("prepared", 9)).unwrap();
    #[cfg(unix)]
    { use std::os::unix::fs::PermissionsExt; fs::set_permissions(root.join("store.json"), fs::Permissions::from_mode(0o600)).unwrap(); }
}
#[cfg(unix)]
fn make_protected_dir(p: &Path) { use std::os::unix::fs::PermissionsExt; fs::create_dir(p).unwrap(); fs::set_permissions(p, fs::Permissions::from_mode(0o700)).unwrap(); }
#[cfg(windows)]
fn make_protected_dir(p: &Path) {
    use crate::windows_fs as win;
    let parent = win::open_dir(p.parent().unwrap(), win::SHARE_ALL, false).unwrap();
    let made = win::create_private_dir_at(&parent, p.file_name().unwrap()).unwrap();
    win::private_dir(&made).unwrap();
}
#[cfg(unix)]
fn break_lock_protection(p: &Path) { use std::os::unix::fs::PermissionsExt; fs::set_permissions(p, fs::Permissions::from_mode(0o644)).unwrap(); }
#[cfg(unix)]
fn restore_lock_protection(p: &Path) { use std::os::unix::fs::PermissionsExt; fs::set_permissions(p, fs::Permissions::from_mode(0o600)).unwrap(); }
#[cfg(windows)]
fn break_lock_protection(p: &Path) { icacls(&["/grant", "*S-1-1-0:(R)"], p); }
#[cfg(windows)]
fn restore_lock_protection(p: &Path) { icacls(&["/reset"], p); }
#[test]
fn two_writers_with_the_same_expected_generation_yield_exactly_one_selection() {
    for round in 0..3 {
        let (fx, da, db) = Fixture::prepared("writers");
        let results: Vec<Result<SelectReport, SelectRefusal>> = std::thread::scope(|s| {
            let a = s.spawn(|| select(&fx.root, &req(V1, &da, 0)));
            let b = s.spawn(|| select(&fx.root, &req(V2, &db, 0)));
            vec![a.join().unwrap(), b.join().unwrap()]
        });
        let ok: Vec<&SelectReport> = results.iter().filter_map(|r| r.as_ref().ok()).collect();
        assert_eq!(ok.len(), 1, "round {round}: {results:?}");
        let loser = results.iter().find_map(|r| r.as_ref().err()).unwrap();
        assert!(matches!(loser.stage, SelectStage::Lock | SelectStage::Observe), "{loser:?}");
        assert_eq!(loser.progress, Progress::default()); assert!(loser.created.is_empty());
        assert_eq!(fx.generation_in("selection.json"), Some(1)); assert_eq!(fx.generation_in("store.json"), Some(1));
        let (o, v) = observe_and_judge(&fx.root);
        assert!(o.observation.blocked.is_empty(), "{o:?}");
        assert_eq!(v.state, selected(1, &ok[0].version));
    }
}
#[test]
fn prepare_and_select_do_not_overwrite_each_others_records() {
    // (a) concurrent: a selection and a preparation of another version
    for round in 0..3 {
        let (fx, da, _) = Fixture::prepared("race");
        let c = bytes_c(); let src_c = fx.base.join("core-c"); fs::write(&src_c, &c).unwrap();
        let (s, p) = std::thread::scope(|sc| {
            let s = sc.spawn(|| select(&fx.root, &req(V1, &da, 0)));
            let p = sc.spawn(|| prep::prepare(&fx.root, &Request { version: V3, expected_sha256: &sha(&c), source: &src_c }));
            (s.join().unwrap(), p.join().unwrap())
        });
        // the selection either completed, or was refused conservatively because its observation
        // under the lock saw the preparation writing (nothing changed); never half-done
        let s = match s {
            Ok(s) => { assert_eq!(s.generation, 1); Some(s) }
            Err(e) => { assert_eq!(e.stage, SelectStage::Observe, "round {round}: {e:?}"); assert_eq!(e.progress, Progress::default()); assert!(e.created.is_empty()); assert_eq!(fx.generation_in("store.json"), Some(0)); assert!(!fx.has("selection.json")); None }
        };
        match &p { Ok(r) => assert!(matches!(r.outcome, Outcome::Prepared)), Err(e) => assert!(matches!(e.stage, prep::Stage::Observe | prep::Stage::Reobserve), "{e:?}") }
        if s.is_none() {
            // once the preparation is over, the same request goes through and nothing was lost
            let s = select(&fx.root, &req(V1, &da, 0)).unwrap_or_else(|e| panic!("round {round}: {e:?}"));
            assert_eq!(s.generation, 1);
        }
        assert_eq!(fx.generation_in("store.json"), Some(1), "round {round}: history overwritten by the preparation: {p:?}");
        assert_eq!(fx.generation_in("selection.json"), Some(1));
        let (o, v) = observe_and_judge(&fx.root);
        assert!(o.observation.blocked.is_empty(), "{o:?}"); assert_eq!(v.state, selected(1, V1));
    }
    // (b) the dangerous interleaving: a preparation that observed `initializing` reaches its
    //     record step only after another call completed the record and a selection ran
    let fx = Fixture::new("race-late-record");
    let a = bytes_a(); let src_a = fx.base.join("core-a"); fs::write(&src_a, &a).unwrap();
    prep::hooks::FAIL_AFTER.with(|h| h.set(Some(prep::Point::CompleteWritten)));
    prep::prepare(&fx.root, &Request { version: V1, expected_sha256: &sha(&a), source: &src_a }).unwrap_err();
    prep::hooks::reset();
    assert_eq!(observe_and_judge(&fx.root).1.state, State::Initializing);
    thread_local! { static DIGEST: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) }; }
    fn complete_and_select_meanwhile(root: &Path) {
        // runs inside the outer prepare, right before its record step
        prep::hooks::BEFORE_RECORD.with(|h| h.set(None));
        let d = DIGEST.with(|d| d.borrow().clone());
        let src = root.parent().unwrap().join("core-a");
        let r = prep::prepare(root, &Request { version: V1, expected_sha256: &d, source: &src }).unwrap();
        assert_eq!(r.outcome, Outcome::AlreadyPrepared); assert_eq!(r.after.state, State::Prepared);
        assert_eq!(select(root, &SelectRequest { version: V1, sha256: &d, expected_generation: 0 }).unwrap().generation, 1);
    }
    DIGEST.with(|d| *d.borrow_mut() = sha(&a));
    prep::hooks::BEFORE_RECORD.with(|h| h.set(Some(complete_and_select_meanwhile)));
    let r = prep::prepare(&fx.root, &Request { version: V1, expected_sha256: &sha(&a), source: &src_a }).unwrap();
    prep::hooks::reset();
    assert_eq!(r.outcome, Outcome::AlreadyPrepared);
    assert!(r.notes.iter().any(|n| n.contains("record left alone")), "{r:?}");
    assert_eq!(r.after.state, selected(1, V1));
    assert_eq!(fx.generation_in("store.json"), Some(1), "history overwritten by the late record");
    assert!(!fx.has(STORE_NEW));
}
#[test]
fn failure_injection_at_every_boundary_matches_the_judge_and_the_report_matches_the_disk() {
    for subsequent in [false, true] {
        for point in [SelectPoint::IntentNewWritten, SelectPoint::IntentRecorded, SelectPoint::SelectionNewWritten, SelectPoint::SelectionPublished, SelectPoint::HistoryNewWritten, SelectPoint::HistoryUpdated] {
            let (fx, da, db) = Fixture::prepared("inject");
            let (version, digest, expected) = if subsequent { select(&fx.root, &req(V1, &da, 0)).unwrap(); (V2, db.clone(), 1) } else { (V1, da.clone(), 0) };
            let new_gen = expected + 1;
            let previous_selection = fx.read("selection.json");
            let versions_before = fx.versions_snapshot();
            hooks::FAIL_AFTER.with(|h| h.set(Some(point)));
            let e = select(&fx.root, &req(version, &digest, expected)).unwrap_err();
            hooks::reset();
            let ctx = format!("subsequent={subsequent} point={point:?}");
            assert!(e.reason.contains("injected failure"), "{ctx}: {e:?}");
            let after = e.after.clone().expect("re-observation after an interruption");
            // expected judge state per interruption point
            match point {
                SelectPoint::IntentNewWritten => {
                    assert_eq!(e.stage, SelectStage::Intent, "{ctx}");
                    assert_eq!(e.progress, Progress::default(), "{ctx}");
                    assert_eq!(after.state, if subsequent { selected(1, V1) } else { State::Prepared }, "{ctx}: {after:?}");
                    assert_eq!(e.previous_selection_preserved, previous_selection.as_ref().map(|_| true), "{ctx}");
                    assert!(fx.has(INTENT_NEW) && e.created == vec![INTENT_NEW.to_string()], "{ctx}: {e:?}");
                    // the leftover blocks the next attempt (kept, no recovery here)
                    let again = select(&fx.root, &req(version, &digest, expected)).unwrap_err();
                    assert_eq!(again.stage, SelectStage::Intent); assert!(again.reason.contains("exists"), "{again:?}");
                }
                SelectPoint::IntentRecorded | SelectPoint::SelectionNewWritten => {
                    assert_eq!(e.stage, if point == SelectPoint::IntentRecorded { SelectStage::Intent } else { SelectStage::Publish }, "{ctx}");
                    assert_eq!(e.progress, Progress { intent_recorded: true, ..Progress::default() }, "{ctx}");
                    assert!(matches!(after.state, State::Damaged { .. }), "{ctx}: {after:?}");
                    assert_eq!(after.run, Run::None);
                    assert_eq!(e.previous_selection_preserved, previous_selection.as_ref().map(|_| true), "{ctx}: the previous selection file must be byte-identical");
                    assert_eq!(fx.read("selection.json"), previous_selection, "{ctx}");
                    assert_eq!(fx.generation_in("intent.json"), Some(new_gen), "{ctx}");
                }
                SelectPoint::SelectionPublished | SelectPoint::HistoryNewWritten => {
                    assert_eq!(e.stage, if point == SelectPoint::SelectionPublished { SelectStage::Publish } else { SelectStage::History }, "{ctx}");
                    assert_eq!(e.progress, Progress { intent_recorded: true, published: true, ..Progress::default() }, "{ctx}: a failure after publication reports the publication");
                    assert_eq!(after.state, State::Selected { generation: new_gen, version: version.into(), history_lagging: true }, "{ctx}: {after:?}");
                    assert_eq!(after.run, Run::Selected);
                    assert_eq!(e.previous_selection_preserved, previous_selection.as_ref().map(|_| false), "{ctx}: the previous selection was replaced");
                    assert_eq!(fx.generation_in("selection.json"), Some(new_gen)); assert_eq!(fx.generation_in("store.json"), Some(expected));
                    // a further selection is refused until the lagging history is completed elsewhere
                    let again = select(&fx.root, &req(version, &digest, new_gen)).unwrap_err();
                    assert_eq!(again.stage, SelectStage::Observe); assert!(again.reason.contains("history_lagging"), "{again:?}");
                }
                SelectPoint::HistoryUpdated => {
                    assert_eq!(e.stage, SelectStage::History, "{ctx}");
                    assert_eq!(e.progress, Progress { intent_recorded: true, published: true, history_updated: true, intent_kept: false }, "{ctx}");
                    assert_eq!(after.state, selected(new_gen, version), "{ctx}: {after:?}");
                    assert!(after.interrupted.iter().any(|t| t.contains("intent_pending_cleanup")), "{after:?}");
                }
            }
            assert_progress_matches_disk(&fx, e.progress, new_gen);
            for o in &e.objects { assert!(matches!(o.presence, Presence::SameObject), "{ctx}: {o:?}"); }
            assert_eq!(fx.versions_snapshot(), versions_before, "{ctx}: candidate bytes changed");
            assert!(fx.has(runtime_lock::LOCK_FILE));
        }
    }
    // write and flush failures stop before any record is replaced
    let (fx, da, _) = Fixture::prepared("io");
    hooks::FAIL_WRITE.with(|h| h.set(true));
    let e = select(&fx.root, &req(V1, &da, 0)).unwrap_err();
    hooks::reset();
    assert_eq!(e.stage, SelectStage::Intent); assert!(e.reason.contains("write"), "{e:?}"); assert_eq!(e.progress, Progress::default());
    assert!(fx.has(INTENT_NEW) && !fx.has("intent.json") && !fx.has("selection.json"));
    let (fx, da, _) = Fixture::prepared("io-flush");
    hooks::FAIL_FLUSH.with(|h| h.set(true));
    let e = select(&fx.root, &req(V1, &da, 0)).unwrap_err();
    hooks::reset();
    assert_eq!(e.stage, SelectStage::Intent); assert!(e.reason.contains("flush"), "{e:?}"); assert_eq!(e.progress, Progress::default());
}
#[test]
fn the_layer_never_executes_a_candidate_or_deletes_anything() {
    const SRC: &str = include_str!("runtime_select.rs");
    for forbidden in ["Command::new", "spawn(", "execv", "CreateProcess", "unlink", "delete(", "remove_file", "remove_dir"] {
        assert!(!SRC.contains(forbidden), "{forbidden} must not appear in runtime_select.rs");
    }
    const LOCK_SRC: &str = include_str!("runtime_lock.rs");
    for forbidden in ["unlink", "delete(", "remove_file"] {
        assert!(!LOCK_SRC.contains(forbidden), "{forbidden} must not appear in runtime_lock.rs");
    }
}

#[cfg(unix)]
fn break_protection(path: &Path) { use std::os::unix::fs::PermissionsExt; fs::set_permissions(path, fs::Permissions::from_mode(0o400)).unwrap(); }
#[cfg(unix)]
fn restore_protection(path: &Path) { use std::os::unix::fs::PermissionsExt; fs::set_permissions(path, fs::Permissions::from_mode(0o500)).unwrap(); }
#[cfg(windows)]
fn icacls(args: &[&str], target: &Path) {
    let icacls = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/icacls.exe");
    let out = std::process::Command::new(icacls).arg(target).args(args).output().unwrap();
    assert!(out.status.success(), "icacls {args:?}: {} {}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}
#[cfg(windows)]
fn break_protection(path: &Path) { icacls(&["/grant", "*S-1-1-0:(RX)"], path); }
#[cfg(windows)]
fn restore_protection(path: &Path) { icacls(&["/reset"], path); }
