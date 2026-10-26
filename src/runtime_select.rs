//! Runtime update — **selection transition** of an already prepared and verified candidate
//! (Implementation Plan §1.1 select order: intent → selection → history; §1.2; §3.1).
//!
//! A request binds version, full digest and `expected_generation` (the completed history the
//! caller believes is current). The request is an input like any other: it does **not** prove
//! that a human approved anything; the approval surface (§4.1) is outside this layer.
//!
//! Under the OS exclusive selection lock (`runtime_lock`): the store is re-observed and judged
//! (only `prepared` with history 0 or `selected` at exactly `expected_generation`, not lagging,
//! may proceed), the candidate directory is re-verified with the same checks a preparation
//! gets (protection, links, execute right, digest), then:
//! 1. `intent.json.new` (exclusive) → rename over `intent.json` (intent recorded, durable),
//! 2. `selection.json.new` (exclusive) → rename over `selection.json` (published),
//! 3. `store.json.new` (exclusive) → rename over `store.json` (history = new generation).
//! `intent.json` is **kept**: clearing it would need a check-then-delete-by-name step, which is
//! not a conditional atomic delete; the judge reads the kept intent as `intent_pending_cleanup`
//! (still `selected`). Nothing is deleted by this layer; `selection.lock` is never deleted.
//!
//! Every interruption point maps to a judge state: after 1 → `damaged` (intent without a
//! matching published selection; the previous selection file is preserved byte for byte);
//! after 2 → `selected` with `history_lagging`; after 3 → `selected` with
//! `intent_pending_cleanup`. The report says which of the three records actually changed;
//! a failure after publication is never reported as "selection unchanged".
//!
//! Not in this layer: candidate execution, launcher, MCP tools, damage recovery, rollback,
//! garbage collection, completion of a lagging history left by an earlier interruption.
use crate::runtime_observe::{self as observe, INTENT_SCHEMA, SELECTION_SCHEMA, VERSIONS_DIR, observe_and_judge, platform, version_dirname};
use crate::runtime_prepare::{Candidate, Object, Presence, STORE_NEW, create_file_deletable, flush_dir_handle, flush_file_handle, replace_record, store_json, verify_complete_candidate};
use crate::runtime_lock::{self, Wait};
use crate::runtime_state::{Phase, Record, State, Verdict};
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
#[cfg(windows)]
use platform::StampSize as _;

pub const INTENT_NEW: &str = "intent.json.new";
pub const SELECTION_NEW: &str = "selection.json.new";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectRequest<'a> { pub version: &'a str, pub sha256: &'a str, pub expected_generation: u64 }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectStage { Request, Observe, Lock, Verify, Intent, Publish, History, Reobserve }

/// Boundaries at which a test can inject a failure (`hooks::FAIL_AFTER`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SelectPoint { IntentNewWritten, IntentRecorded, SelectionNewWritten, SelectionPublished, HistoryNewWritten, HistoryUpdated }

/// Which records this call actually changed (true only after the rename that made it so).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Progress { pub intent_recorded: bool, pub published: bool, pub history_updated: bool, pub intent_kept: bool }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectReport {
    pub generation: u64,
    pub version: String,
    pub sha256: String,
    pub progress: Progress,
    pub created: Vec<String>,
    pub objects: Vec<Object>,
    pub before: Verdict,
    pub after: Verdict,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectRefusal {
    pub stage: SelectStage,
    pub reason: String,
    /// The real state of the three records at the failure (never "unchanged" after a publication).
    pub progress: Progress,
    pub created: Vec<String>,
    pub objects: Vec<Object>,
    /// `Some(true)` when a previous `selection.json` existed and still holds the same bytes.
    pub previous_selection_preserved: Option<bool>,
    pub before: Option<Verdict>,
    pub after: Option<Verdict>,
}

#[cfg(test)]
pub(crate) mod hooks {
    use super::SelectPoint;
    use std::cell::Cell;
    thread_local! {
        pub static FAIL_AFTER: Cell<Option<SelectPoint>> = const { Cell::new(None) };
        pub static FAIL_FLUSH: Cell<bool> = const { Cell::new(false) };
        pub static FAIL_WRITE: Cell<bool> = const { Cell::new(false) };
        /// Runs right after the selection lock was taken, before the observation under it.
        pub static AFTER_LOCK: Cell<Option<fn(&std::path::Path)>> = const { Cell::new(None) };
    }
    pub fn reset() { FAIL_AFTER.with(|c| c.set(None)); FAIL_FLUSH.with(|c| c.set(false)); FAIL_WRITE.with(|c| c.set(false)); AFTER_LOCK.with(|c| c.set(None)); }
}
fn checkpoint(point: SelectPoint) -> io::Result<()> {
    #[cfg(test)]
    if hooks::FAIL_AFTER.with(|c| c.get()) == Some(point) { return Err(io::Error::other(format!("injected failure after {point:?}"))); }
    let _ = point;
    Ok(())
}
fn flush_file(file: &File) -> io::Result<()> {
    #[cfg(test)]
    if hooks::FAIL_FLUSH.with(|c| c.get()) { return Err(io::Error::other("injected flush failure")); }
    flush_file_handle(file)
}
fn flush_root(root: &platform::Dir, path: &Path) -> io::Result<()> {
    #[cfg(test)]
    if hooks::FAIL_FLUSH.with(|c| c.get()) { return Err(io::Error::other("injected flush failure")); }
    flush_dir_handle(root, path)
}
fn write_all(file: &mut &File, bytes: &[u8]) -> io::Result<()> {
    #[cfg(test)]
    if hooks::FAIL_WRITE.with(|c| c.get()) { let half = bytes.len() / 2; file.write_all(&bytes[..half])?; return Err(io::Error::other("injected write failure")); }
    file.write_all(bytes)
}

struct Created { label: String, parent: Option<platform::Dir>, name: String, id: Option<platform::ObjectId> }
struct Ledger { created: Vec<Created>, progress: Progress, before: Option<Verdict>, root: PathBuf, previous_selection: Option<Vec<u8>>, root_dir: Option<platform::Dir> }
impl Ledger {
    fn note(&mut self, label: &str, parent: &platform::Dir, name: &str, file: &File) {
        let id = platform::stamp_file(file).ok().map(|s| s.object_id());
        self.created.push(Created { label: label.into(), parent: parent.clone_handle().ok(), name: name.into(), id });
    }
    fn relabel(&mut self, old_label: &str, new_label: &str, new_name: &str) {
        if let Some(c) = self.created.iter_mut().find(|c| c.label == old_label) { c.label = new_label.into(); c.name = new_name.into(); }
    }
    fn objects(&self) -> Vec<Object> {
        self.created.iter().map(|c| {
            let presence = match &c.parent {
                None => Presence::Unknown("no parent handle for the re-check (duplicate failed at creation)".into()),
                Some(parent) => match platform::stamp_entry(parent, &c.name) {
                    Ok(None) => Presence::Absent,
                    Ok(Some(now)) => match c.id { Some(id) if now.object_id() == id => Presence::SameObject, Some(_) => Presence::Replaced, None => Presence::Unknown("identity was not captured at creation".into()) },
                    Err(e) => Presence::Unknown(format!("lookup failed: {e}")),
                },
            };
            Object { label: c.label.clone(), presence }
        }).collect()
    }
    fn preserved(&self) -> Option<bool> {
        let before = self.previous_selection.as_ref()?;
        let root = self.root_dir.as_ref()?;
        let now = match platform::open_regular(root, "selection.json") { Ok(Some(f)) => { let mut b = Vec::new(); if (&f).take(observe::MAX_RECORD_BYTES + 1).read_to_end(&mut b).is_err() { return Some(false); } b } _ => return Some(false) };
        Some(&now == before)
    }
    fn refuse(&self, stage: SelectStage, reason: impl Into<String>) -> SelectRefusal {
        let after = if self.created.is_empty() && self.before.is_some() && !self.progress.intent_recorded { None } else { Some(observe_and_judge(&self.root).1) };
        SelectRefusal { stage, reason: reason.into(), progress: self.progress, created: self.created.iter().map(|c| c.label.clone()).collect(), objects: self.objects(), previous_selection_preserved: self.preserved(), before: self.before.clone(), after }
    }
}

fn record_json(schema: &str, generation: u64, version: &str, sha256: &str) -> Vec<u8> {
    let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!(r#"{{"schema":"{schema}","generation":{generation},"version":"{version}","sha256":"{sha256}","requested_at":"{at}"}}"#).into_bytes()
}

/// Write `name.new` exclusively, flush it, rename it over `name`, flush the root. The caller
/// places injection checkpoints around it through `before_rename` / the returned handle.
fn write_record(ledger: &mut Ledger, root: &platform::Dir, root_path: &Path, new_name: &str, final_name: &str, bytes: &[u8], stage: SelectStage, before_rename: SelectPoint, after_rename: SelectPoint) -> Result<(), SelectRefusal> {
    let new = match create_file_deletable(root, new_name) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return Err(ledger.refuse(stage, format!("{new_name} exists from an interrupted earlier transition; kept, needs approved recovery"))),
        Err(e) => return Err(ledger.refuse(stage, format!("{new_name} creation: {e}"))),
    };
    ledger.note(new_name, root, new_name, &new);
    write_all(&mut &new, bytes).map_err(|e| ledger.refuse(stage, format!("{new_name} write: {e}")))?;
    flush_file(&new).map_err(|e| ledger.refuse(stage, format!("{new_name} flush: {e}")))?;
    checkpoint(before_rename).map_err(|e| ledger.refuse(stage, e.to_string()))?;
    replace_record(root, root_path, &new, new_name, final_name).map_err(|e| ledger.refuse(stage, format!("{new_name} → {final_name} rename: {e}")))?;
    ledger.relabel(new_name, &format!("{final_name} (replaced by this call)"), final_name);
    drop(new);
    // the rename is what changed the record; a flush failure after it must not hide that
    match stage { SelectStage::Intent => ledger.progress.intent_recorded = true, SelectStage::Publish => ledger.progress.published = true, SelectStage::History => ledger.progress.history_updated = true, _ => {} }
    flush_root(root, root_path).map_err(|e| ledger.refuse(stage, format!("root flush after {final_name} rename: {e}")))?;
    checkpoint(after_rename).map_err(|e| ledger.refuse(stage, e.to_string()))?;
    Ok(())
}

/// Select a prepared candidate as the next generation. See the module docs.
pub fn select(root: &Path, req: &SelectRequest) -> Result<SelectReport, SelectRefusal> {
    let mut ledger = Ledger { created: vec![], progress: Progress::default(), before: None, root: root.to_path_buf(), previous_selection: None, root_dir: None };
    if !observe::valid_version(req.version) { return Err(ledger.refuse(SelectStage::Request, "version identifier refused")); }
    if !observe::valid_sha256(req.sha256) { return Err(ledger.refuse(SelectStage::Request, "digest must be 64 lowercase hex characters")); }
    let Some(generation) = req.expected_generation.checked_add(1) else { return Err(ledger.refuse(SelectStage::Request, "expected_generation is u64::MAX: no next generation exists (overflow refused)")) };
    // 1. Observe before taking the lock (cheap refusal of blocked/damaged/first-use stores).
    let (_, before) = observe_and_judge(root);
    ledger.before = Some(before.clone());
    if !matches!(before.state, State::Prepared | State::Selected { .. }) { return Err(ledger.refuse(SelectStage::Observe, format!("store is {:?}; only prepared or selected stores accept a selection", before.state))); }
    // 2. Root handle and the OS exclusive lock.
    let root_dir = match platform::walk(root) { Ok(Some(d)) => d, Ok(None) => return Err(ledger.refuse(SelectStage::Lock, "root vanished after observation")), Err(e) => return Err(ledger.refuse(SelectStage::Lock, format!("root walk: {e}"))) };
    platform::check_root(&root_dir).map_err(|e| ledger.refuse(SelectStage::Lock, format!("root protection: {e}")))?;
    ledger.root_dir = root_dir.clone_handle().ok();
    let _lock = match runtime_lock::acquire(&root_dir, Wait::NonBlocking) {
        Ok(l) => l,
        Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Err(ledger.refuse(SelectStage::Lock, "selection lock is held by another selection or record update; nothing changed")),
        Err(e) => return Err(ledger.refuse(SelectStage::Lock, format!("selection lock: {e}"))),
    };
    #[cfg(test)]
    if let Some(f) = hooks::AFTER_LOCK.with(|c| c.get()) { f(root); }
    // 3. Under the lock: re-observe **through the held root handle** (the same object the lock
    //    and the writes refer to; the path is only confirmed to still name it), bind the
    //    generation, re-verify the candidate.
    let (observed, under_lock) = observe::observe_and_judge_held(root, &root_dir);
    if let State::Blocked { reasons } = &under_lock.state { return Err(ledger.refuse(SelectStage::Observe, format!("store blocked under the lock: {reasons:?}; nothing changed"))); }
    let (phase, last_generation) = match &observed.observation.store { Record::Valid(s) => (s.phase, s.last_generation), _ => return Err(ledger.refuse(SelectStage::Observe, "store record not valid under the lock")) };
    if phase != Phase::Prepared { return Err(ledger.refuse(SelectStage::Observe, "store is still initializing under the lock; nothing changed")); }
    match &under_lock.state {
        State::Prepared => {}
        State::Selected { history_lagging: true, .. } => return Err(ledger.refuse(SelectStage::Observe, "an earlier selection was published but its history update is pending (history_lagging); completing it is not in this layer; nothing changed")),
        State::Selected { generation: g, .. } if *g != req.expected_generation => return Err(ledger.refuse(SelectStage::Observe, format!("generation conflict: current selection is generation {g}, request expected {}; nothing changed", req.expected_generation))),
        State::Selected { .. } => {}
        other => return Err(ledger.refuse(SelectStage::Observe, format!("store is {other:?} under the lock; nothing changed"))),
    }
    if last_generation != req.expected_generation { return Err(ledger.refuse(SelectStage::Observe, format!("generation conflict: history is at {last_generation}, request expected {}; nothing changed", req.expected_generation))); }
    let versions = match platform::open_subdir(&root_dir, VERSIONS_DIR) { Ok(Some(d)) => d, Ok(None) => return Err(ledger.refuse(SelectStage::Verify, "versions/ absent")), Err(e) => return Err(ledger.refuse(SelectStage::Verify, format!("versions/: {e}"))) };
    platform::check_dir(&versions).map_err(|e| ledger.refuse(SelectStage::Verify, format!("versions/ protection: {e}")))?;
    let dirname = version_dirname(req.version, req.sha256);
    match verify_complete_candidate(&versions, &dirname, req.sha256) {
        Ok(()) => {}
        Err(Candidate::Absent) => return Err(ledger.refuse(SelectStage::Verify, format!("{VERSIONS_DIR}/{dirname} is not prepared"))),
        Err(Candidate::Incomplete) => return Err(ledger.refuse(SelectStage::Verify, format!("{VERSIONS_DIR}/{dirname} is an incomplete preparation; not selectable"))),
        Err(Candidate::DigestDiffers) => return Err(ledger.refuse(SelectStage::Verify, format!("{VERSIONS_DIR}/{dirname} Core digest differs from the requested digest; not selectable"))),
        Err(Candidate::Refused(why)) => return Err(ledger.refuse(SelectStage::Verify, format!("{VERSIONS_DIR}/{dirname} {why}; not selectable"))),
    }
    drop(versions);
    // previous selection bytes, to report whether a failure before publication preserved them
    if let Ok(Some(f)) = platform::open_regular(&root_dir, "selection.json") { let mut b = Vec::new(); if (&f).take(observe::MAX_RECORD_BYTES + 1).read_to_end(&mut b).is_ok() { ledger.previous_selection = Some(b); } }
    // 4. intent → selection → history.
    write_record(&mut ledger, &root_dir, root, INTENT_NEW, "intent.json", &record_json(INTENT_SCHEMA, generation, req.version, req.sha256), SelectStage::Intent, SelectPoint::IntentNewWritten, SelectPoint::IntentRecorded)?;
    write_record(&mut ledger, &root_dir, root, SELECTION_NEW, "selection.json", &record_json(SELECTION_SCHEMA, generation, req.version, req.sha256), SelectStage::Publish, SelectPoint::SelectionNewWritten, SelectPoint::SelectionPublished)?;
    write_record(&mut ledger, &root_dir, root, STORE_NEW, "store.json", &store_json("prepared", generation), SelectStage::History, SelectPoint::HistoryNewWritten, SelectPoint::HistoryUpdated)?;
    // 5. intent.json is kept (no conditional atomic delete available); the judge reports
    //    intent_pending_cleanup and the next selection replaces it by rename.
    ledger.progress.intent_kept = true;
    // 6. Re-observe through the held root handle, still inside the lock lifetime.
    let (_, after) = observe::observe_and_judge_held(root, &root_dir);
    match &after.state {
        State::Selected { generation: g, version, history_lagging: false } if *g == generation && version == req.version => {}
        other => return Err(ledger.refuse(SelectStage::Reobserve, format!("post-observation {other:?} is not the predicted selected generation {generation}"))),
    }
    drop(_lock);
    let notes = vec!["intent.json kept after the history update (intent_pending_cleanup); no delete-by-name is performed".to_owned()];
    Ok(SelectReport { generation, version: req.version.to_owned(), sha256: req.sha256.to_owned(), progress: ledger.progress, created: ledger.created.iter().map(|c| c.label.clone()).collect(), objects: ledger.objects(), before, after, notes })
}

#[cfg(test)]
#[path = "runtime_select_tests.rs"]
mod tests;
