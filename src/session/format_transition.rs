//! The explicit storage-format transition **6 → 8 and 7 → 9** (Storage Transition Contract §7,
//! implementation order 1b). Development profile `will-end-history` only.
//!
//! ```text
//! check   read only    lock → no recovery, no cleanup → read state.yaml → full load
//!                      → candidate in memory → full format 8/9 load + value preservation
//!                      → the proposal and its digest
//! apply   under lock   approval + bound values → read again → same proposal?
//!                      → candidate again → original bytes to conversion/ (exclusive, synced,
//!                      re-read) → candidate through the exclusive, no-follow record writer
//!                      (new temporary file, flush, re-read, rename = the commit point) → read back
//! ```
//!
//! Nothing that already stands at `.gil/state.yaml.writing` or under `.gil/conversion/` is
//! overwritten, followed, moved or removed, on either format; such an entry stops the transition.
//!
//! # What the approval is, and what it is not
//!
//! The user decided (contract §12 O-1, 2026-11-21) that a project's transition is approved in the
//! conversation after the user reviewed the proposal. `user_approved` is the Agent's **relay** of
//! that consent. It is not an authentication of the user, nothing here stores it, and it binds to
//! the proposal digest: the exact project location, state file, formats, state bytes, copy
//! location and the records that change. A check result, a feature request, an install or this
//! development code is never that consent.
//!
//! # What this never does
//!
//! No transition on read, run, start or update; no recovery or cleanup of an unfinished restore;
//! no reverse transition, copy restore or deletion of a copy or a temporary file; no automatic
//! retry after an uncertain result; no Monitor button. Only `state.yaml` changes in the active
//! record — Artifact objects, Report text, working files and `.gilignore` are never touched.
use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{
    SessionError, gil_dir, preflight_state, private_storage_after_lock, private_storage_before_lock,
    private_visual_storage,
};
use crate::lock::{LOCK_FILE, ProjectLock};
use crate::project::Project;
use crate::restore;
use crate::rules::RuleSet;
use crate::store::{self, END_HISTORY_FORMAT, END_HISTORY_VISUAL_FORMAT, StoreError};

/// The proposal's schema; part of its digest.
pub(crate) const SCHEMA: &str = "gil-format-transition/1";
/// Beside the state file: `.gil/conversion/` (contract §7.3).
pub(crate) const COPY_DIR: &str = "conversion";

/// The only transitions this build applies: 6 → 8 and 7 → 9.
pub(crate) fn target_of(format: u32) -> Option<u32> {
    store::transition_target(format)
}

/// How far a refused or failed request got. The MCP answer says this, so an Agent never reads a
/// failure as "nothing happened" when the state may already be replaced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Class {
    /// Nothing was written.
    Refused,
    /// `state.yaml` is unchanged (the copy may exist).
    NotApplied,
    /// The state file may already be replaced: read the actual state, never re-apply blindly.
    Unconfirmed,
}

#[derive(Debug)]
pub(crate) struct Refusal {
    pub(crate) code: &'static str,
    pub(crate) class: Class,
    pub(crate) said: String,
    pub(crate) facts: Value,
}

fn refused(code: &'static str, said: impl Into<String>) -> Refusal {
    Refusal { code, class: Class::Refused, said: said.into(), facts: json!({}) }
}
fn not_applied(code: &'static str, said: impl Into<String>) -> Refusal {
    Refusal { code, class: Class::NotApplied, said: said.into(), facts: json!({}) }
}
fn unconfirmed(said: impl Into<String>) -> Refusal {
    Refusal { code: "apply_unconfirmed", class: Class::Unconfirmed, said: said.into(), facts: json!({}) }
}
impl Refusal {
    fn with(mut self, facts: Value) -> Self {
        self.facts = facts;
        self
    }
}

fn session_refusal(error: SessionError) -> Refusal {
    match error {
        SessionError::Busy { .. } => refused("busy", "Another GIL command holds this project. Nothing was read or changed; try again when it has finished."),
        SessionError::LockUnavailable { source, .. } => refused("lock_unavailable", format!("The project lock could not be taken: {source}")),
        SessionError::NeedsRecovery { found, .. } => refused("needs_recovery", format!(
            "An unfinished restore is left in .gil/restore ({found}). The transition does not recover or clean it up and changed nothing. Run an ordinary GIL command on this project first (it recovers), then check again.")),
        SessionError::Store(StoreError::NotFound { .. }) => refused("not_a_project", "No GIL record (.gil/state.yaml) at this project_root."),
        SessionError::Store(StoreError::UnknownFormat { found, known }) => refused("unknown_format", format!(
            "The record declares format {found}; this build reads up to {known}. Nothing was changed.")),
        SessionError::Store(error) => refused("record_invalid", format!("The record cannot be read as a valid GIL project: {error}. Nothing was changed.")),
        other => refused("record_unreadable", format!("The project could not be inspected: {other}. Nothing was changed.")),
    }
}

/// The project, locked, with the state bytes read exactly once under that lock.
struct Held {
    lock: ProjectLock,
    gil: PathBuf,
    state: PathBuf,
    /// Canonical project root and state file location: what the proposal binds to.
    root_named: String,
    state_named: String,
    text: String,
    digest: String,
    format: u32,
}

impl Held {
    /// The read-only opening of [`super::ProjectSession::open_read_only_at`]: the lock first, an
    /// unfinished restore refused (never recovered), temporary objects left alone.
    fn open(root: &Path, state: &Path) -> Result<Self, Refusal> {
        let gil = gil_dir(state).map_err(session_refusal)?;
        let declared = private_storage_before_lock(&gil, state).map_err(session_refusal)?;
        let lock = ProjectLock::acquire(&gil).map_err(|e| session_refusal(SessionError::from(e)))?;
        private_storage_after_lock(&gil, state, declared).map_err(session_refusal)?;
        preflight_state(state).map_err(session_refusal)?;
        if let Some(found) = restore::left_behind(&gil).map_err(|e| session_refusal(SessionError::Restore(e)))? {
            return Err(session_refusal(SessionError::NeedsRecovery { path: root.display().to_string(), found }));
        }
        let text = read_state(state).map_err(session_refusal)?;
        let format = store::declared_format(&text, state).map_err(|e| session_refusal(SessionError::Store(e)))?;
        let named = |path: &Path| path.canonicalize().map(|p| p.display().to_string());
        let root_named = named(root).map_err(|e| refused("record_unreadable", format!("project_root cannot be resolved: {e}")))?;
        let state_named = named(&gil)
            .map(|g| Path::new(&g).join(state.file_name().unwrap_or_default()).display().to_string())
            .map_err(|e| refused("record_unreadable", format!("the record folder cannot be resolved: {e}")))?;
        Ok(Held { lock, digest: sha256(text.as_bytes()), gil, state: state.to_path_buf(), root_named, state_named, text, format })
    }

    fn load(&self, rules: RuleSet) -> Result<Project, Refusal> {
        let project = store::load_text(rules, &self.text, &self.state).map_err(|e| session_refusal(SessionError::Store(e)))?;
        private_visual_storage(&self.gil, &project).map_err(session_refusal)?;
        Ok(project)
    }

    fn still_locked(&self) -> io::Result<()> {
        self.lock.validate(&self.gil.join(LOCK_FILE))
    }
}

fn read_state(state: &Path) -> Result<String, SessionError> {
    #[cfg(unix)]
    let read = crate::internal_fs::read_state(state);
    #[cfg(windows)]
    let read = crate::windows_fs::read_state(state);
    #[cfg(not(any(unix, windows)))]
    let read = std::fs::read_to_string(state);
    read.map_err(|source| match source.kind() {
        io::ErrorKind::NotFound => SessionError::Store(StoreError::NotFound { path: store::said_path(state) }),
        _ => SessionError::Store(StoreError::Read { path: store::said_path(state), source }),
    })
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn copy_name(format: u32, digest: &str) -> String {
    format!("state-format-{format}-{}.yaml", &digest[..16])
}

/// What the user reviews, and the digest an approval binds to.
struct Proposal {
    from: u32,
    to: u32,
    copy: String,
    /// Ended Wills per Existence that move from `done_wills` to `ended_wills`.
    records: Vec<(String, usize)>,
    digest: String,
}

impl Proposal {
    fn of(held: &Held, project: &Project) -> Result<Self, Refusal> {
        let from = held.format;
        let to = target_of(from).ok_or_else(|| unsupported_format(from))?;
        let copy = copy_name(from, &held.digest);
        let records: Vec<(String, usize)> = project.existences()
            .map(|e| (e.id().to_string(), e.journey().ended_wills().len())).collect();
        // Every value JSON-quoted, one per line, in a fixed order: no path can forge a field.
        let q = |text: &str| serde_json::to_string(text).expect("string");
        let mut canonical = format!(
            "schema={}\nproject_root={}\nstate_file={}\ncurrent_format={from}\ntarget_format={to}\nstate_sha256={}\ncopy_file={}\nchange={}\nend_result={}\n",
            q(SCHEMA), q(&held.root_named), q(&held.state_named), q(&held.digest),
            q(&format!("{COPY_DIR}/{copy}")), q("done_wills->ended_wills"), q("achieved"));
        for (existence, count) in &records {
            canonical.push_str(&format!("ended_wills={}:{count}\n", q(existence)));
        }
        Ok(Proposal { from, to, copy, records, digest: sha256(canonical.as_bytes()) })
    }

    fn converted(&self) -> usize {
        self.records.iter().map(|(_, n)| n).sum()
    }

    /// What the user reads. Paths are said as the caller named the project and relative to it —
    /// never the canonical absolute record location (it may name the user's home or private app
    /// data). The digest still binds the canonical locations.
    fn body(&self, held: &Held, root: &Path, copy: &CopyState) -> Value {
        let inline = Path::new(&held.state_named).starts_with(&held.root_named);
        json!({
            "schema": SCHEMA,
            "project_root": root.display().to_string(),
            "state_file": if inline { format!(".gil/{}", held.state.file_name().unwrap_or_default().to_string_lossy()) }
                else { "the private GIL record of this project, kept outside project_root".to_owned() },
            "current_format": self.from,
            "target_format": self.to,
            "state_sha256": held.digest,
            "copy_file": format!(".gil/{COPY_DIR}/{}", self.copy),
            "copy_present": copy.word(),
            "changes": {
                "end_history": "done_wills becomes ended_wills (same records, same order)",
                "end_result": "every existing record gets end_result: achieved — a format 6/7 ended Will was an achieved one; no Report is read to reclassify it",
                "records": self.records.iter().map(|(x, n)| json!({"existence": x, "ended_wills": n})).collect::<Vec<_>>(),
                "format": format!("{} becomes {}", self.from, self.to),
            },
            "preserved": [
                "Will names, order and original text (objective, next_action, done_when, target)",
                "Journey revisions and will_head_ref",
                "Chains, Cycles, Steps and Reports",
                "Snapshot names and manifest addresses; Artifact objects are not copied or changed",
                if self.to == END_HISTORY_VISUAL_FORMAT { "the visual registry and its migration boundary" } else { "no visual registry is added (an even format stays even)" },
                "working files, Report text and .gilignore are not touched; only .gil/state.yaml is replaced",
            ],
            "compatibility": format!("After the transition a GIL build that does not know format {} refuses to open or change this project (released builds up to now do). Projects in formats 4–7 elsewhere are unaffected.", self.to),
            "reverse": "No reverse transition is provided. The original state bytes stay in the copy file; restoring them is a separate explicit decision, never automatic.",
        })
    }
}

fn unsupported_format(found: u32) -> Refusal {
    refused("unsupported_format", format!(
        "Format {found} is not a source of this transition: only 6 → 8 and 7 → 9 exist. Formats 4/5 (no Chain), adding the visual registry, formats 10/11 and any reverse transition are not provided. Nothing was changed."))
}

/// The candidate bytes for `text`, validated: a full format 8/9 load, the same domain state with
/// only the end-history flag changed, and every stored value except the allowed changes equal.
fn candidate(rules: &RuleSet, text: &str, state: &Path, project: Project, from: u32) -> Result<Vec<u8>, Refusal> {
    let to = target_of(from).ok_or_else(|| unsupported_format(from))?;
    let invalid = |why: String| refused("candidate_invalid", format!(
        "The converted record would not be valid in format {to}: {why}. Nothing was changed; the record stays in format {from}."));
    let converted = project.into_end_history_candidate();
    let bytes = store::encode(&converted).map_err(|e| invalid(e.to_string()))?;
    let written = std::str::from_utf8(&bytes).map_err(|e| invalid(e.to_string()))?;
    if store::declared_format(written, state).ok() != Some(to) {
        return Err(invalid("the candidate does not declare the target format".into()));
    }
    let reloaded = store::load_text(rules.clone(), written, state).map_err(|e| invalid(e.to_string()))?;
    if !reloaded.records_end_results() || format!("{converted:?}") != format!("{reloaded:?}") {
        return Err(invalid("the candidate reads back as a different project".into()));
    }
    preserved(text, written, from).map_err(|why| refused("not_preserved", format!(
        "The candidate changes more than the transition allows ({why}). Nothing was changed.")))?;
    Ok(bytes)
}

/// Every stored value of `original`, with only the allowed changes applied, equals `candidate`.
fn preserved(original: &str, candidate: &str, from: u32) -> Result<(), String> {
    use serde_norway::Value as Y;
    let mut expected: Y = serde_norway::from_str(original).map_err(|e| e.to_string())?;
    let got: Y = serde_norway::from_str(candidate).map_err(|e| e.to_string())?;
    let root = expected.as_mapping_mut().ok_or("state is not a mapping")?;
    if root.get("format") != Some(&Y::from(from)) {
        return Err("the original format changed".into());
    }
    root.insert(Y::from("format"), Y::from(from + 2));
    let existences = root.get_mut("existences").and_then(Y::as_mapping_mut).ok_or("no existences")?;
    for (name, existence) in existences.iter_mut() {
        let journey = existence.get_mut("journey").and_then(Y::as_mapping_mut)
            .ok_or_else(|| format!("{name:?}: no journey"))?;
        if journey.contains_key("ended_wills") {
            return Err(format!("{name:?}: already has ended_wills"));
        }
        let done = journey.remove("done_wills").ok_or_else(|| format!("{name:?}: no done_wills"))?;
        let mut ended = Vec::new();
        for record in done.as_sequence().ok_or_else(|| format!("{name:?}: done_wills is not a list"))? {
            let mut record = record.as_mapping().ok_or("an ended Will is not a mapping")?.clone();
            if record.contains_key("end_result") {
                return Err("a format 6/7 record already carries end_result".into());
            }
            record.insert(Y::from("end_result"), Y::from("achieved"));
            ended.push(Y::Mapping(record));
        }
        journey.insert(Y::from("ended_wills"), Y::Sequence(ended));
    }
    if expected != got {
        return Err("a stored value outside the end history differs".into());
    }
    Ok(())
}

// ── The original-state copy ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CopyState {
    Absent,
    /// Present with exactly the original bytes: reused, never rewritten.
    Same,
    /// Present with other bytes (a partial write, another state): never overwritten or deleted.
    Different,
}
impl CopyState {
    fn word(self) -> &'static str {
        match self {
            CopyState::Absent => "absent",
            CopyState::Same => "present_matching",
            CopyState::Different => "present_different",
        }
    }
}

/// Read-only look at `.gil/conversion/<name>`. A linked or special entry is an error.
fn inspect_copy(gil: &Path, name: &str, bytes: &[u8]) -> io::Result<CopyState> {
    match read_copy(gil, name, bytes.len())? {
        None => Ok(CopyState::Absent),
        Some(found) if found == bytes => Ok(CopyState::Same),
        Some(_) => Ok(CopyState::Different),
    }
}

#[cfg(unix)]
fn read_copy(gil: &Path, name: &str, limit: usize) -> io::Result<Option<Vec<u8>>> {
    use crate::internal_fs as fs;
    let at = fs::directory(gil, false)?;
    let dir = match fs::child_dir(&at, c"conversion") {
        Ok(dir) => dir,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let cname = std::ffi::CString::new(name).map_err(|_| io::Error::other("copy name"))?;
    match fs::regular_at(&dir, &cname) {
        Ok(file) => read_bounded(file, limit).map(Some),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

#[cfg(windows)]
fn read_copy(gil: &Path, name: &str, limit: usize) -> io::Result<Option<Vec<u8>>> {
    use crate::windows_fs as win;
    use std::ffi::OsStr;
    use windows_sys::Win32::Storage::FileSystem::OPEN_EXISTING;
    let root = gil.parent().ok_or_else(win::refused)?.canonicalize()?;
    let mut chain = win::Chain::open(&root)?;
    chain.push(OsStr::new(".gil"), false)?;
    match chain.push(OsStr::new(COPY_DIR), false) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    }
    match win::open_file(&chain.child(OsStr::new(name))?, win::READ, win::SHARE_ALL, OPEN_EXISTING) {
        Ok(file) => read_bounded(file, limit).map(Some),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// Reads at most `limit + 1` bytes: a longer copy is already "different" without reading it all.
fn read_bounded(mut file: std::fs::File, limit: usize) -> io::Result<Vec<u8>> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if file.metadata()?.nlink() != 1 {
            return Err(io::Error::other("a linked copy file is refused"));
        }
    }
    #[cfg(windows)]
    if !crate::windows_fs::Stamp::of(&file)?.is_single_regular() {
        return Err(io::Error::other("a linked or special copy file is refused"));
    }
    let mut out = Vec::new();
    (&mut file).take(limit as u64 + 1).read_to_end(&mut out)?;
    Ok(out)
}

/// Writes the original bytes as a new copy: exclusive creation, synced, read back and compared,
/// the folder synced. An existing equal copy is reused; anything else fails, and nothing that was
/// already there is overwritten or removed. Owner-only on every format.
#[cfg(unix)]
fn write_copy(gil: &Path, name: &str, bytes: &[u8]) -> io::Result<()> {
    use crate::internal_fs as fs;
    let at = fs::directory(gil, false)?;
    let dir = match fs::child_dir(&at, c"conversion") {
        Ok(dir) => dir,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let made = fs::create_dir_at(&at, c"conversion", 0o700)?;
            fs::fsync(&at)?;
            made
        }
        Err(e) => return Err(e),
    };
    let cname = std::ffi::CString::new(name).map_err(|_| io::Error::other("copy name"))?;
    {
        let mut file = fs::create_file_at(&dir, &cname, 0o600)?;
        file.write_all(bytes)?;
        file.flush()?;
        file.sync_all()?;
    }
    checkpoint("copy_synced")?;
    let again = read_bounded(fs::regular_at(&dir, &cname)?, bytes.len())?;
    if again != bytes {
        return Err(io::Error::other("the copy read back differs from the original state"));
    }
    fs::fsync(&dir)
}

#[cfg(windows)]
fn write_copy(gil: &Path, name: &str, bytes: &[u8]) -> io::Result<()> {
    use crate::windows_fs as win;
    use std::ffi::OsStr;
    use windows_sys::Win32::Storage::FileSystem::{CREATE_NEW, OPEN_EXISTING};
    let root = gil.parent().ok_or_else(win::refused)?.canonicalize()?;
    let mut chain = win::Chain::open(&root)?;
    chain.push(OsStr::new(".gil"), false)?;
    let created = match chain.push(OsStr::new(COPY_DIR), false) {
        Ok(()) => false,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            // Inherits the `.gil` DACL (owner-only on a visual-format record).
            chain.push(OsStr::new(COPY_DIR), true)?;
            true
        }
        Err(e) => return Err(e),
    };
    if created {
        let parent = chain.above(1).ok_or_else(win::refused)?;
        win::flush_dir(chain.path().parent().ok_or_else(win::refused)?, parent)?;
    }
    let at = chain.child(OsStr::new(name))?;
    {
        let mut file = win::open_file(&at, win::READ_WRITE, win::SHARE_ALL, CREATE_NEW)?;
        file.write_all(bytes)?;
        file.flush()?;
        win::flush(&file)?;
    }
    checkpoint("copy_synced")?;
    let again = read_bounded(win::open_file(&at, win::READ, win::SHARE_ALL, OPEN_EXISTING)?, bytes.len())?;
    if again != bytes {
        return Err(io::Error::other("the copy read back differs from the original state"));
    }
    win::flush_dir(chain.path(), chain.dir())
}

#[cfg(not(any(unix, windows)))]
fn read_copy(_: &Path, _: &str, _: usize) -> io::Result<Option<Vec<u8>>> {
    Err(io::Error::other("the transition copy is not supported on this platform"))
}
#[cfg(not(any(unix, windows)))]
fn write_copy(_: &Path, _: &str, _: &[u8]) -> io::Result<()> {
    Err(io::Error::other("the transition copy is not supported on this platform"))
}

/// The state writer's temporary name beside the state file.
fn temporary_state(state: &Path) -> PathBuf {
    let mut temp = state.as_os_str().to_owned();
    temp.push(".writing");
    PathBuf::from(temp)
}

/// Whatever already stands at the temporary name — a file, a folder, a link or a special entry,
/// on every format — is kept exactly as it is: the transition never overwrites, follows, moves or
/// removes it, and does not apply while it is there. The entry itself is examined, never what a
/// link points to; an entry that cannot be examined stops the transition the same way.
/// Check and apply say the same thing. The writer below refuses such an entry again at creation
/// time, so this early answer is a message, not the safety guarantee.
fn temporary_blocker(state: &Path) -> Option<String> {
    match std::fs::symlink_metadata(temporary_state(state)) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Ok(_) => Some("An entry already exists at the state writer's temporary name (.gil/state.yaml.writing), for example from an earlier failed save. GIL keeps it exactly as it is — it is not overwritten, followed, moved or deleted — and the transition is not applied while it is there. The user decides what to do with it.".into()),
        Err(e) => Some(format!("The state writer's temporary name (.gil/state.yaml.writing) cannot be examined safely ({e}). GIL changes nothing there and the transition is not applied.")),
    }
}

/// The commit point for **both** formats: the record writer that creates its temporary file
/// exclusively under the held `.gil` directory without following links (an entry that appeared
/// after the early answer fails the creation and stays untouched), flushes and re-reads it, then
/// renames it over the state file relative to that same directory. It keeps a failed temporary
/// file for inspection. This is the visual-format writer; the ordinary even-format `save` path is
/// not used here and is not changed. Durability is what that writer provides, no more.
fn publish_exclusive(state: &Path, bytes: &[u8]) -> io::Result<()> {
    #[cfg(any(unix, windows))]
    {
        crate::visual_report::state_file::write(state, bytes)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (state, bytes);
        Err(io::Error::other("the transition writer is not supported on this platform"))
    }
}

// ── Test-only interruption points ───────────────────────────────────────────────────────

#[cfg(test)]
pub(crate) mod hooks {
    use std::cell::Cell;
    thread_local! {
        /// Called at `copy_synced`, `before_publish` and `after_publish`; an error stops there,
        /// exactly as a crash or a failed step would.
        pub(crate) static AT: Cell<Option<fn(&str) -> std::io::Result<()>>> = const { Cell::new(None) };
    }
}

fn checkpoint(stage: &str) -> io::Result<()> {
    #[cfg(test)]
    if let Some(act) = hooks::AT.with(|c| c.get()) {
        return act(stage);
    }
    let _ = stage;
    Ok(())
}

// ── The two operations ──────────────────────────────────────────────────────────────────

/// Read-only check. Never writes, recovers or cleans up; a lock file is only created where the
/// ordinary read-only opening would create it.
pub(crate) fn check(rules: RuleSet, root: &Path, state: &Path) -> Result<Value, Refusal> {
    let held = Held::open(root, state)?;
    let project = held.load(rules.clone())?;
    if matches!(held.format, END_HISTORY_FORMAT | END_HISTORY_VISUAL_FORMAT) {
        return Ok(json!({
            "ok": true, "status": "already_transitioned", "storage_format": held.format,
            "project_root": root.display().to_string(), "state_sha256": held.digest,
            "copies": copies(&held.gil),
            "said": format!("This project is already in format {}. There is nothing to apply; do not call the apply tool again. A copy listed here is the original state of an earlier transition; it is kept, never restored automatically.", held.format),
        }));
    }
    let proposal = Proposal::of(&held, &project)?;
    candidate(&rules, &held.text, state, project, held.format)?;
    let copy = inspect_copy(&held.gil, &proposal.copy, held.text.as_bytes())
        .map_err(|e| refused("copy_unreadable", format!("The copy location .gil/{COPY_DIR}/{} cannot be inspected safely: {e}. Nothing was changed.", proposal.copy)))?;
    let mut blocked = Vec::new();
    if copy == CopyState::Different {
        blocked.push(format!("A different file already exists at .gil/{COPY_DIR}/{}. GIL never overwrites or deletes it; the user decides what to do with it before a transition can be applied.", proposal.copy));
    }
    if let Some(why) = temporary_blocker(state) {
        blocked.push(why);
    }
    let body = proposal.body(&held, root, &copy);
    Ok(json!({
        "ok": true,
        "status": if blocked.is_empty() { "ready" } else { "blocked" },
        "blocked_by": blocked,
        "development_only": true,
        "proposal": body,
        "proposal_sha256": proposal.digest,
        "apply_with": {
            "project_root": root.display().to_string(),
            "current_format": proposal.from,
            "target_format": proposal.to,
            "state_sha256": held.digest,
            "proposal_sha256": proposal.digest,
            "user_approved": "true only after the user reviewed THIS proposal and explicitly agreed in the conversation; false if the user declined",
        },
        "said": format!("Show the user this proposal (project, format {} → {}, what changes and what is kept, older builds can no longer open it, no reverse transition, copy at .gil/{COPY_DIR}/{}). This check is not an approval. Apply only after the user explicitly agrees in the conversation to this exact proposal; the approval you relay is not an authentication of the user. If the state or the proposal changes, check and ask again.", proposal.from, proposal.to, proposal.copy),
    }))
}

/// The bound approval, as relayed by the Agent.
pub(crate) struct Approval<'a> {
    pub(crate) current_format: u32,
    pub(crate) target_format: u32,
    pub(crate) state_sha256: &'a str,
    pub(crate) proposal_sha256: &'a str,
    pub(crate) user_approved: bool,
}

pub(crate) fn apply(rules: RuleSet, root: &Path, state: &Path, approval: &Approval) -> Result<Value, Refusal> {
    if !approval.user_approved {
        return Err(refused("approval_required", "user_approved is false: the user has not approved this proposal (or declined it). Nothing was read or changed. Do not apply without the user's explicit agreement in the conversation."));
    }
    if target_of(approval.current_format) != Some(approval.target_format) {
        return Err(refused("unsupported_transition", format!(
            "{} → {} is not a transition this build applies: only 6 → 8 and 7 → 9. Formats 4/5, adding the visual registry, formats 10/11 and reverse transitions are not provided. Nothing was read or changed.",
            approval.current_format, approval.target_format)));
    }
    let held = Held::open(root, state)?;
    if held.format == approval.target_format {
        return Err(already_transitioned(&rules, &held, approval));
    }
    if held.format != approval.current_format {
        return Err(refused("target_mismatch", format!(
            "The record is in format {}, not {}. Nothing was changed. Check again and show the user the new proposal.", held.format, approval.current_format))
            .with(json!({"storage_format": held.format})));
    }
    if held.digest != approval.state_sha256 {
        return Err(refused("state_changed", "The state file changed since the approved check (or the approval was for another project). Nothing was changed. Check again, show the new proposal and ask the user again.")
            .with(json!({"state_sha256": held.digest})));
    }
    let project = held.load(rules.clone())?;
    let proposal = Proposal::of(&held, &project)?;
    if proposal.digest != approval.proposal_sha256 {
        return Err(refused("proposal_mismatch", "The approved proposal is not the proposal for this project and state (another project, location or proposal). Nothing was changed. Check this project, show its proposal and ask the user again.")
            .with(json!({"proposal_sha256": proposal.digest})));
    }
    if let Some(why) = temporary_blocker(state) {
        return Err(refused("temporary_state_present", format!("{why} Nothing was changed.")));
    }
    let bytes = candidate(&rules, &held.text, state, project, held.format)?;

    // The original bytes, beside the state file. An equal copy (an earlier interrupted attempt)
    // is reused after its content is compared; a different one stops everything.
    let copy_file = format!(".gil/{COPY_DIR}/{}", proposal.copy);
    let reused = match inspect_copy(&held.gil, &proposal.copy, held.text.as_bytes()) {
        Ok(CopyState::Same) => true,
        Ok(CopyState::Absent) => {
            held.still_locked().map_err(|e| not_applied("copy_failed", format!("The project lock could not be confirmed before the copy: {e}. state.yaml is unchanged.")))?;
            write_copy(&held.gil, &proposal.copy, held.text.as_bytes()).map_err(|e| not_applied("copy_failed", format!(
                "The original state could not be saved and confirmed at {copy_file}: {e}. state.yaml is unchanged (format {}). Anything written there is kept; GIL does not delete it. Check again before any new request.", held.format))
                .with(json!({"copy_file": copy_file})))?;
            false
        }
        Ok(CopyState::Different) => return Err(not_applied("copy_conflict", format!(
            "A different file already exists at {copy_file}. GIL does not overwrite or delete it. state.yaml is unchanged; the user decides what to do with that file.")).with(json!({"copy_file": copy_file}))),
        Err(e) => return Err(refused("copy_unreadable", format!("The copy location {copy_file} cannot be inspected safely: {e}. Nothing was changed."))),
    };

    let failed_before = |e: io::Error| not_applied("apply_interrupted", format!(
        "Stopped before the state file was replaced ({e}). state.yaml is unchanged (format {}); the copy at {copy_file} is kept. Check again before any new request.", held.format))
        .with(json!({"copy_file": copy_file}));
    checkpoint("before_publish").map_err(failed_before)?;
    held.still_locked().map_err(failed_before)?;

    // The commit point: exclusive temporary creation, flush, re-read, rename (both formats).
    if let Err(error) = publish_exclusive(state, &bytes) {
        return Err(match read_state(state) {
            Ok(now) if now == held.text => not_applied("candidate_not_saved", format!(
                "The converted state could not be saved ({error}). state.yaml is unchanged (format {}); the copy at {copy_file} is kept. Any entry at the temporary name is kept as it is; GIL does not delete it. Check again before any new request.", held.format)),
            Ok(now) if now.as_bytes() == bytes.as_slice() => unconfirmed(format!(
                "state.yaml was replaced with format {}, but a later step reported an error ({error}). Do not apply again; read the project with the check tool.", proposal.to)),
            _ => unconfirmed(format!("The save reported an error ({error}) and the state file could not be confirmed. Do not apply again; read the project with the check tool.")),
        }.with(json!({"copy_file": copy_file})));
    }
    let after = (|| -> Result<(), String> {
        checkpoint("after_publish").map_err(|e| e.to_string())?;
        held.still_locked().map_err(|e| e.to_string())?;
        let now = read_state(state).map_err(|e| e.to_string())?;
        if now.as_bytes() != bytes.as_slice() {
            return Err("the state file does not read back as the converted state".into());
        }
        let reread = store::load_text(rules.clone(), &now, state).map_err(|e| e.to_string())?;
        if !reread.records_end_results() || store::declared_format(&now, state).ok() != Some(proposal.to) {
            return Err("the state file does not read back in the target format".into());
        }
        Ok(())
    })();
    if let Err(why) = after {
        return Err(unconfirmed(format!(
            "state.yaml was replaced, but reading it back failed ({why}). Do not apply again; read the project with the check tool.")).with(json!({"copy_file": copy_file})));
    }
    Ok(json!({
        "ok": true, "stage": "applied",
        "previous_format": proposal.from, "storage_format": proposal.to,
        "converted_records": proposal.converted(),
        "copy_file": copy_file, "copy": if reused { "reused" } else { "created" },
        "state_sha256_before": held.digest, "state_sha256_after": sha256(&bytes),
        "reverse_available": false, "automatic_retry": false,
        "said": format!("Applied: this project is now format {}. Every earlier ended Will is recorded as achieved. The original state is kept at {copy_file}; no reverse transition is provided.", proposal.to),
    }))
}

/// An apply request that arrives when the record is already in the target format (a resend after
/// a lost response, or a stale approval): nothing is written and no copy is created. The answer
/// says what is actually there.
fn already_transitioned(rules: &RuleSet, held: &Held, approval: &Approval) -> Refusal {
    // The wire accepts only a 64-digit lowercase digest, so the name is always well formed.
    let name = copy_name(approval.current_format, approval.state_sha256);
    let original = read_copy(&held.gil, &name, 512 * 1024 * 1024).ok().flatten();
    let copy = match &original {
        None => "absent",
        Some(bytes) if sha256(bytes) == approval.state_sha256 => "present_matching",
        Some(_) => "present_different",
    };
    // Is the record now exactly that copy, converted? Only then did this request's transition
    // produce what is there and nothing was written since.
    let same = original.as_deref()
        .filter(|bytes| sha256(bytes) == approval.state_sha256)
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .and_then(|text| store::load_text(rules.clone(), text, &held.state).ok().map(|p| (text, p)))
        .and_then(|(text, project)| candidate(rules, text, &held.state, project, approval.current_format).ok())
        .map(|bytes| bytes == held.text.as_bytes());
    refused("already_transitioned", format!(
        "This project is already in format {}. Nothing was written and no copy was created. Do not apply again.", held.format))
        .with(json!({
            "storage_format": held.format, "wrote": false,
            "copy_file": format!(".gil/{COPY_DIR}/{name}"), "copy_for_request": copy,
            "state_is_that_transition": same,
        }))
}

/// Names under `.gil/conversion/` (read-only listing; nothing is opened or judged).
fn copies(gil: &Path) -> Value {
    let dir = gil.join(COPY_DIR);
    match std::fs::symlink_metadata(&dir) {
        Ok(meta) if meta.is_dir() => {}
        Ok(_) => return json!({"error": "the conversion entry is not a directory"}),
        Err(e) if e.kind() == io::ErrorKind::NotFound => return json!([]),
        Err(e) => return json!({"error": e.to_string()}),
    }
    match std::fs::read_dir(&dir) {
        Ok(entries) => {
            let mut names: Vec<String> = entries.filter_map(|e| e.ok())
                .map(|e| format!(".gil/{COPY_DIR}/{}", e.file_name().to_string_lossy())).collect();
            names.sort();
            json!(names)
        }
        Err(e) => json!({"error": e.to_string()}),
    }
}

#[cfg(test)]
mod tests;
