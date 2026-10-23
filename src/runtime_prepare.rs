//! Runtime update — **initialization and candidate preparation** of a runtime store at an
//! explicitly given path (Implementation Plan §1.1 prepare order, §3.1 protection table).
//!
//! What this layer does: create the protected root and `store.json{phase:"initializing"}` on a
//! verified first use; reserve the version (`versions/~reserve-<version>`, exclusive) so that
//! two concurrent preparations of the same version with different bytes cannot both complete;
//! create `versions/<version>-<sha12>/` exclusively; copy the candidate Core from a caller-given
//! regular file while hashing it; flush; re-read and re-hash the written file; publish the
//! `complete` marker only after that verification; record `store.json{phase:"prepared"}` by
//! exclusive `store.json.new` + rename; re-check that the held directories and the reservation
//! are still the objects at their names; re-observe with the read-only observer and the pure
//! judge. An existing complete preparation of the same version and digest is accepted as
//! `AlreadyPrepared` only after the same protection, link, execute-right and digest checks a
//! fresh preparation gets.
//!
//! Reservations are **never removed** by this layer (a Unix "check identity, then unlink" is
//! not a conditional atomic delete, so no "only my own object is deleted" guarantee could be
//! given; the conservative choice is to keep the file). Meaning of a reservation that is
//! present: together with a verified complete directory of that version it is the normal end
//! state of a preparation (the fast path still answers `AlreadyPrepared`); without such a
//! directory it marks an interrupted preparation, and the version stays refused until an
//! approved recovery removes it. Nothing is deleted at all by this layer: partial
//! preparations, reservations and `store.json.new` leftovers are kept and reported.
//!
//! What it does not do: download, archive extraction, candidate execution, selection or intent
//! records, launcher, MCP tools, product store path decision, deletion of anything,
//! modification of existing destinations, bytes or ACLs.
//!
//! The expected digest is a value the caller supplies and the written bytes are matched
//! against it. That is an integrity check of the copy — it is **not** an authentication of a
//! publisher and not a trust verdict on a product candidate (Plan §4.1 trust gate is separate).
use crate::runtime_observe::{self as observe, COMPLETE_FILE, CORE_FILE, STORE_SCHEMA, VERSIONS_DIR, observe_and_judge, platform, version_dirname};
use crate::runtime_state::{Phase, Record, State, Verdict};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, Read, Write};
pub(crate) use pfs::{create_file_deletable, flush_dir_handle, flush_file_handle, replace_record};
use std::path::{Path, PathBuf};
#[cfg(windows)]
use platform::StampSize as _;

pub const STORE_NEW: &str = "store.json.new";
/// Upper bound for a candidate Core copy (synthetic or real); larger sources are refused.
pub const MAX_CORE_BYTES: u64 = 256 * 1024 * 1024;
/// `versions/~reserve-<version>`: exclusive per-version reservation held while a directory for
/// that version is created. `~` cannot start a version identifier, so the name never parses as
/// a version directory.
pub fn reservation_name(version: &str) -> String { format!("~reserve-{version}") }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Request<'a> {
    pub version: &'a str,
    pub expected_sha256: &'a str,
    /// A regular file holding the candidate Core bytes (opened without following links).
    pub source: &'a Path,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome { Prepared, AlreadyPrepared }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage { Request, Observe, Root, StoreRecord, Versions, Reservation, Uniqueness, VersionDir, Copy, Verify, Complete, PreparedRecord, Reobserve }

/// Boundaries at which a test can inject a failure (`hooks::FAIL_AFTER`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Point { RootCreated, StoreWritten, VersionsCreated, Reserved, VersionDirCreated, CoreWritten, CoreFlushed, Verified, CompleteWritten, NewStoreWritten }

/// What a re-check through the held parent handle found for an object this call created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Presence {
    /// The entry at that name is the object this call created (same device/volume and inode/file id).
    SameObject,
    /// No entry at that name.
    Absent,
    /// An entry exists at that name but it is a different object.
    Replaced,
    /// The lookup itself failed (access denied, I/O error, no parent handle); not treated as absent.
    Unknown(String),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Object { pub label: String, pub presence: Presence }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub outcome: Outcome,
    pub version_dir: String,
    /// Objects this call created, in creation order, relative to the root (`<root>` for the root
    /// itself). A replaced record is listed as `store.json (record replaced: initializing → prepared)`
    /// and the initializing record it replaced as `store.json (initializing record, replaced by
    /// this call)` (presence `Replaced`). The reservation stays and is listed as created.
    pub created: Vec<String>,
    /// The same objects with the result of a re-check after the last step.
    pub objects: Vec<Object>,
    pub before: Verdict,
    pub after: Verdict,
    /// Informational notes (for example "record left alone: updated by another call").
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub stage: Stage,
    pub reason: String,
    /// Objects this call created before the failure, in creation order.
    pub created: Vec<String>,
    /// Those of `created` found as the same object after the failure. Nothing is deleted by
    /// this layer, so this normally equals `created` (minus a record this call itself replaced).
    pub remaining: Vec<String>,
    /// Every created object with its re-check result (same_object / absent / replaced / unknown),
    /// computed right before the return.
    pub objects: Vec<Object>,
    pub before: Option<Verdict>,
    pub after: Option<Verdict>,
    /// Informational notes (for example "record left alone: updated by another call").
    pub notes: Vec<String>,
}

#[cfg(test)]
pub(crate) mod hooks {
    use super::Point;
    use std::cell::Cell;
    use std::path::Path;
    thread_local! {
        pub static FAIL_AFTER: Cell<Option<Point>> = const { Cell::new(None) };
        pub static FAIL_FLUSH: Cell<bool> = const { Cell::new(false) };
        pub static FAIL_WRITE: Cell<bool> = const { Cell::new(false) };
        /// The handle duplicate taken for reporting fails (the creation itself must still be recorded).
        pub static FAIL_NOTE_DUP: Cell<bool> = const { Cell::new(false) };
        /// Runs right after the version directory was created (before the copy).
        pub static AFTER_VERSION_DIR: Cell<Option<fn(&Path)>> = const { Cell::new(None) };
        /// Runs right after the reservation was taken (before the scan under it).
        pub static AFTER_RESERVED: Cell<Option<fn(&Path)>> = const { Cell::new(None) };
        /// Runs after the fast-path scan found nothing, before the reservation is taken.
        pub static BEFORE_RESERVATION: Cell<Option<fn(&Path)>> = const { Cell::new(None) };
        /// Runs right before the prepared-record step (after the version is complete, before the
        /// selection lock is taken for the record update).
        pub static BEFORE_RECORD: Cell<Option<fn(&Path)>> = const { Cell::new(None) };
        /// Runs inside the record step's lock lifetime, after store.json was re-read and before it
        /// is replaced.
        pub static UNDER_RECORD_LOCK: Cell<Option<fn(&Path)>> = const { Cell::new(None) };
    }
    pub fn reset() { FAIL_AFTER.with(|c| c.set(None)); FAIL_FLUSH.with(|c| c.set(false)); FAIL_WRITE.with(|c| c.set(false)); FAIL_NOTE_DUP.with(|c| c.set(false)); AFTER_VERSION_DIR.with(|c| c.set(None)); AFTER_RESERVED.with(|c| c.set(None)); BEFORE_RESERVATION.with(|c| c.set(None)); BEFORE_RECORD.with(|c| c.set(None)); UNDER_RECORD_LOCK.with(|c| c.set(None)); }
}

fn checkpoint(point: Point) -> io::Result<()> {
    #[cfg(test)]
    if hooks::FAIL_AFTER.with(|c| c.get()) == Some(point) { return Err(io::Error::other(format!("injected failure after {point:?}"))); }
    let _ = point;
    Ok(())
}
fn flush_file(file: &File) -> io::Result<()> {
    #[cfg(test)]
    if hooks::FAIL_FLUSH.with(|c| c.get()) { return Err(io::Error::other("injected flush failure")); }
    pfs::fsync_file(file)
}
fn flush_dir(dir: &Held) -> io::Result<()> {
    #[cfg(test)]
    if hooks::FAIL_FLUSH.with(|c| c.get()) { return Err(io::Error::other("injected flush failure")); }
    pfs::fsync_dir(&dir.dir, &dir.path)
}
fn write_all(file: &mut &File, bytes: &[u8]) -> io::Result<()> {
    #[cfg(test)]
    if hooks::FAIL_WRITE.with(|c| c.get()) { let half = bytes.len() / 2; file.write_all(&bytes[..half])?; return Err(io::Error::other("injected write failure")); }
    file.write_all(bytes)
}

/// A held directory with the path it was opened by (the path is used only for Windows
/// directory flushes and record renames, both of which re-check the pinned identity).
struct Held { dir: platform::Dir, path: PathBuf }

/// One object created by this call: its name under a held parent (duplicated for reporting;
/// `None` when that duplicate failed — the creation is still recorded) and its identity.
struct Created { label: String, parent: Option<platform::Dir>, name: String, id: Option<platform::ObjectId> }

struct Tx { created: Vec<Created>, before: Option<Verdict>, root: PathBuf, traces: Vec<String> }
impl Tx {
    /// Record a creation. The identity comes from the handle of the created object; the parent
    /// handle is duplicated for the later re-check. Neither failure drops the record.
    fn note(&mut self, label: &str, parent: &Held, name: &str, id: Option<platform::ObjectId>) {
        #[cfg(test)]
        let dup = if hooks::FAIL_NOTE_DUP.with(|c| c.get()) { Err(io::Error::other("injected duplicate failure")) } else { parent.dir.clone_handle() };
        #[cfg(not(test))]
        let dup = parent.dir.clone_handle();
        self.created.push(Created { label: label.into(), parent: dup.ok(), name: name.into(), id });
    }
    fn labels(&self) -> Vec<String> { self.created.iter().map(|c| c.label.clone()).collect() }
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
    fn remaining(&self) -> Vec<String> { self.objects().into_iter().filter(|o| o.presence == Presence::SameObject).map(|o| o.label).collect() }
    fn refuse(&self, stage: Stage, reason: impl Into<String>) -> Refusal {
        let after = if self.created.is_empty() && self.before.is_some() { None } else { Some(observe_and_judge(&self.root).1) };
        Refusal { stage, reason: reason.into(), created: self.labels(), remaining: self.remaining(), objects: self.objects(), before: self.before.clone(), after, notes: self.traces.clone() }
    }
}

pub(crate) fn store_json(phase: &str, last_generation: u64) -> Vec<u8> {
    let created = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!(r#"{{"schema":"{STORE_SCHEMA}","phase":"{phase}","last_generation":{last_generation},"owner":"{}","created":"{created}"}}"#, platform::owner_text()).into_bytes()
}

/// Parse `<version>-<sha12>`; `None` for names of another shape (reservations included).
fn split_dirname(name: &str) -> Option<(&str, &str)> {
    let (version, sha12) = name.rsplit_once('-')?;
    (sha12.len() == 12 && sha12.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()) && observe::valid_version(version)).then_some((version, sha12))
}
fn id_of_file(file: &File) -> Option<platform::ObjectId> { platform::stamp_file(file).ok().map(|s| s.object_id()) }
fn id_of_dir(dir: &platform::Dir) -> Option<platform::ObjectId> { platform::stamp_dir(dir).ok().map(|s| s.object_id()) }

/// Result of scanning `versions/` for the requested version.
enum Scan { Nothing, AlreadyPreparedVerified }

/// Initialize (on verified first use) and prepare one candidate version. See the module docs
/// for the scope and the refusal rules.
pub fn prepare(root: &Path, req: &Request) -> Result<Report, Refusal> {
    let mut tx = Tx { created: vec![], before: None, root: root.to_path_buf(), traces: vec![] };
    if !observe::valid_version(req.version) { return Err(tx.refuse(Stage::Request, "version identifier refused")); }
    if !observe::valid_sha256(req.expected_sha256) { return Err(tx.refuse(Stage::Request, "expected digest must be 64 lowercase hex characters")); }
    let target = version_dirname(req.version, req.expected_sha256);
    // 1. Observe; only verified first use or a healthy store may proceed.
    let (observed, before) = observe_and_judge(root);
    tx.before = Some(before.clone());
    let (init, phase, last_generation) = match (&before.state, &observed.observation.store) {
        (State::Blocked { .. }, _) => return Err(tx.refuse(Stage::Observe, format!("store blocked: {:?}", observed.observation.blocked))),
        (State::Damaged { .. }, _) => return Err(tx.refuse(Stage::Observe, "store damaged: diagnostic only, no bootstrap or repair in this layer")),
        (State::FirstUse, _) => (true, Phase::Initializing, 0),
        (_, Record::Valid(store)) => (false, store.phase, store.last_generation),
        _ => return Err(tx.refuse(Stage::Observe, "store record not valid")),
    };
    // 2. Root: exclusive creation on first use, otherwise the existing root through the walk.
    let root_held = if init {
        let parent = root.parent().ok_or_else(|| tx.refuse(Stage::Root, "root has no parent"))?;
        let name = root.file_name().and_then(|n| n.to_str()).ok_or_else(|| tx.refuse(Stage::Root, "root name is not valid UTF-8"))?;
        let parent_dir = match platform::walk(parent) { Ok(Some(d)) => d, Ok(None) => return Err(tx.refuse(Stage::Root, "parent directory not found")), Err(e) => return Err(tx.refuse(Stage::Root, format!("parent walk: {e}"))) };
        let parent_held = Held { dir: parent_dir, path: parent.to_path_buf() };
        let made = pfs::create_root_dir(&parent_held.dir, name).map_err(|e| tx.refuse(Stage::Root, format!("root creation: {e}")))?;
        let held = Held { dir: made, path: root.to_path_buf() };
        tx.note("<root>", &parent_held, name, id_of_dir(&held.dir));
        flush_dir(&parent_held).map_err(|e| tx.refuse(Stage::Root, format!("parent flush: {e}")))?;
        checkpoint(Point::RootCreated).map_err(|e| tx.refuse(Stage::Root, e.to_string()))?;
        platform::check_root(&held.dir).map_err(|e| tx.refuse(Stage::Root, format!("root protection after creation: {e}")))?;
        // store.json{initializing}
        let file = pfs::create_file(&held.dir, "store.json").map_err(|e| tx.refuse(Stage::StoreRecord, format!("store.json creation: {e}")))?;
        tx.note("store.json", &held, "store.json", id_of_file(&file));
        write_all(&mut &file, &store_json("initializing", 0)).map_err(|e| tx.refuse(Stage::StoreRecord, format!("store.json write: {e}")))?;
        flush_file(&file).map_err(|e| tx.refuse(Stage::StoreRecord, format!("store.json flush: {e}")))?;
        let stamp = platform::stamp_file(&file).map_err(|e| tx.refuse(Stage::StoreRecord, e.to_string()))?;
        platform::check_data(&file, &stamp).map_err(|e| tx.refuse(Stage::StoreRecord, format!("store.json protection after creation: {e}")))?;
        flush_dir(&held).map_err(|e| tx.refuse(Stage::StoreRecord, format!("root flush: {e}")))?;
        checkpoint(Point::StoreWritten).map_err(|e| tx.refuse(Stage::StoreRecord, e.to_string()))?;
        held
    } else {
        match platform::walk(root) { Ok(Some(d)) => Held { dir: d, path: root.to_path_buf() }, Ok(None) => return Err(tx.refuse(Stage::Root, "root vanished after observation")), Err(e) => return Err(tx.refuse(Stage::Root, format!("root walk: {e}"))) }
    };
    platform::check_root(&root_held.dir).map_err(|e| tx.refuse(Stage::Root, format!("root protection: {e}")))?;
    // 3. versions/
    let versions = match platform::open_subdir(&root_held.dir, VERSIONS_DIR) {
        Ok(Some(d)) => Held { dir: d, path: root.join(VERSIONS_DIR) },
        Ok(None) => match pfs::create_subdir(&root_held.dir, VERSIONS_DIR) {
            Ok(d) => {
                tx.note("versions", &root_held, VERSIONS_DIR, id_of_dir(&d));
                flush_dir(&root_held).map_err(|e| tx.refuse(Stage::Versions, format!("root flush: {e}")))?;
                checkpoint(Point::VersionsCreated).map_err(|e| tx.refuse(Stage::Versions, e.to_string()))?;
                Held { dir: d, path: root.join(VERSIONS_DIR) }
            }
            // versions/ is the shared container, not a candidate: one created concurrently is
            // opened (without following links) and checked like any existing one.
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => match platform::open_subdir(&root_held.dir, VERSIONS_DIR) {
                Ok(Some(d)) => Held { dir: d, path: root.join(VERSIONS_DIR) },
                Ok(None) => return Err(tx.refuse(Stage::Versions, "versions/ appeared and vanished concurrently")),
                Err(e) => return Err(tx.refuse(Stage::Versions, format!("versions/ (created concurrently): {e}"))),
            },
            Err(e) => return Err(tx.refuse(Stage::Versions, format!("versions/ creation: {e}"))),
        },
        Err(e) => return Err(tx.refuse(Stage::Versions, format!("versions/: {e}"))),
    };
    platform::check_dir(&versions.dir).map_err(|e| tx.refuse(Stage::Versions, format!("versions/ protection: {e}")))?;
    // 4. Fast path: an existing complete, verified preparation needs no reservation.
    let mut already = matches!(scan_versions(&mut tx, &versions, req, &target)?, Scan::AlreadyPreparedVerified);
    let mut held_version_dir: Option<Held> = None;
    let mut reservation_taken: Option<(String, Option<platform::ObjectId>)> = None;
    if !already {
        // 5. Per-version reservation: exclusive; an existing one (concurrent or interrupted) refuses.
        #[cfg(test)]
        if let Some(f) = hooks::BEFORE_RESERVATION.with(|c| c.get()) { f(root); }
        let reservation_name = reservation_name(req.version);
        let reservation = match pfs::create_file_deletable(&versions.dir, &reservation_name) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return Err(tx.refuse(Stage::Reservation, format!("{VERSIONS_DIR}/{reservation_name} exists: another preparation of version {} is in progress or was interrupted; kept, needs approved recovery", req.version))),
            Err(e) => return Err(tx.refuse(Stage::Reservation, format!("{VERSIONS_DIR}/{reservation_name} creation: {e}"))),
        };
        let reservation_label = format!("{VERSIONS_DIR}/{reservation_name}");
        let reservation_id = id_of_file(&reservation);
        tx.note(&reservation_label, &versions, &reservation_name, reservation_id);
        reservation_taken = Some((reservation_name.clone(), reservation_id));
        drop(reservation);
        flush_dir(&versions).map_err(|e| tx.refuse(Stage::Reservation, format!("versions/ flush: {e}")))?;
        checkpoint(Point::Reserved).map_err(|e| tx.refuse(Stage::Reservation, e.to_string()))?;
        #[cfg(test)]
        if let Some(f) = hooks::AFTER_RESERVED.with(|c| c.get()) { f(root); }
        // 6. Scan again under the reservation: the listing before the reservation may be stale.
        //    A refusal here keeps the reservation (reported as created), like every other object.
        match scan_versions(&mut tx, &versions, req, &target)? {
            Scan::AlreadyPreparedVerified => already = true,
            Scan::Nothing => {}
        }
        if !already {
            // 7. Exclusive version directory (a concurrent prepare for the same target loses here).
            let vdir = match pfs::create_subdir(&versions.dir, &target) {
                Ok(d) => Held { dir: d, path: versions.path.join(&target) },
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return Err(tx.refuse(Stage::VersionDir, format!("{VERSIONS_DIR}/{target} appeared concurrently (another preparation holds it); nothing replaced"))),
                Err(e) => return Err(tx.refuse(Stage::VersionDir, format!("{VERSIONS_DIR}/{target} creation: {e}"))),
            };
            let vlabel = format!("{VERSIONS_DIR}/{target}");
            tx.note(&vlabel, &versions, &target, id_of_dir(&vdir.dir));
            flush_dir(&versions).map_err(|e| tx.refuse(Stage::VersionDir, format!("versions/ flush: {e}")))?;
            platform::check_dir(&vdir.dir).map_err(|e| tx.refuse(Stage::VersionDir, format!("{vlabel} protection after creation: {e}")))?;
            checkpoint(Point::VersionDirCreated).map_err(|e| tx.refuse(Stage::VersionDir, e.to_string()))?;
            #[cfg(test)]
            if let Some(f) = hooks::AFTER_VERSION_DIR.with(|c| c.get()) { f(root); }
            // 8. Copy while hashing, flush, re-read and re-hash, then mark executable.
            let mut source = pfs::open_source(req.source).map_err(|e| tx.refuse(Stage::Copy, format!("source: {e}")))?;
            let size = source.metadata().map_err(|e| tx.refuse(Stage::Copy, e.to_string()))?.len();
            if size > MAX_CORE_BYTES { return Err(tx.refuse(Stage::Copy, format!("source of {size} bytes exceeds {MAX_CORE_BYTES}"))); }
            let core = pfs::create_file(&vdir.dir, CORE_FILE).map_err(|e| tx.refuse(Stage::Copy, format!("{vlabel}/{CORE_FILE} creation: {e}")))?;
            tx.note(&format!("{vlabel}/{CORE_FILE}"), &vdir, CORE_FILE, id_of_file(&core));
            let mut hasher = Sha256::new();
            let mut buf = vec![0u8; 1 << 16];
            let mut copied = 0u64;
            loop {
                let n = source.read(&mut buf).map_err(|e| tx.refuse(Stage::Copy, format!("source read: {e}")))?;
                if n == 0 { break; }
                copied += n as u64;
                if copied > MAX_CORE_BYTES { return Err(tx.refuse(Stage::Copy, "source grew beyond the size limit while copying")); }
                hasher.update(&buf[..n]);
                write_all(&mut &core, &buf[..n]).map_err(|e| tx.refuse(Stage::Copy, format!("{vlabel}/{CORE_FILE} write: {e}")))?;
            }
            checkpoint(Point::CoreWritten).map_err(|e| tx.refuse(Stage::Copy, e.to_string()))?;
            flush_file(&core).map_err(|e| tx.refuse(Stage::Copy, format!("{vlabel}/{CORE_FILE} flush: {e}")))?;
            checkpoint(Point::CoreFlushed).map_err(|e| tx.refuse(Stage::Copy, e.to_string()))?;
            let source_digest = format!("{:x}", hasher.finalize());
            if source_digest != req.expected_sha256 { return Err(tx.refuse(Stage::Verify, format!("source digest {} differs from the expected digest {}; partial copy kept without {COMPLETE_FILE}", &source_digest[..12], &req.expected_sha256[..12]))); }
            drop(core);
            let written = match platform::open_regular(&vdir.dir, CORE_FILE) { Ok(Some(f)) => f, Ok(None) => return Err(tx.refuse(Stage::Verify, format!("{vlabel}/{CORE_FILE} vanished before re-read"))), Err(e) => return Err(tx.refuse(Stage::Verify, format!("{vlabel}/{CORE_FILE} re-open: {e}"))) };
            let written_digest = hash_file(&written).map_err(|e| tx.refuse(Stage::Verify, format!("{vlabel}/{CORE_FILE} re-read: {e}")))?;
            if written_digest != req.expected_sha256 { return Err(tx.refuse(Stage::Verify, format!("re-read digest {} differs from the expected digest; partial copy kept without {COMPLETE_FILE}", &written_digest[..12]))); }
            pfs::finish_executable(&written).map_err(|e| tx.refuse(Stage::Verify, format!("{vlabel}/{CORE_FILE} mode: {e}")))?;
            let stamp = platform::stamp_file(&written).map_err(|e| tx.refuse(Stage::Verify, e.to_string()))?;
            platform::check_exec(&written, &stamp).map_err(|e| tx.refuse(Stage::Verify, format!("{vlabel}/{CORE_FILE} protection: {e}")))?;
            drop(written);
            checkpoint(Point::Verified).map_err(|e| tx.refuse(Stage::Verify, e.to_string()))?;
            // 9. complete marker, only now.
            let marker = pfs::create_file(&vdir.dir, COMPLETE_FILE).map_err(|e| tx.refuse(Stage::Complete, format!("{vlabel}/{COMPLETE_FILE} creation: {e}")))?;
            tx.note(&format!("{vlabel}/{COMPLETE_FILE}"), &vdir, COMPLETE_FILE, id_of_file(&marker));
            write_all(&mut &marker, req.expected_sha256.as_bytes()).map_err(|e| tx.refuse(Stage::Complete, format!("{vlabel}/{COMPLETE_FILE} write: {e}")))?;
            flush_file(&marker).map_err(|e| tx.refuse(Stage::Complete, format!("{vlabel}/{COMPLETE_FILE} flush: {e}")))?;
            drop(marker);
            flush_dir(&vdir).map_err(|e| tx.refuse(Stage::Complete, format!("{vlabel} flush: {e}")))?;
            checkpoint(Point::CompleteWritten).map_err(|e| tx.refuse(Stage::Complete, e.to_string()))?;
            held_version_dir = Some(vdir);
        }
    }
    // 10. initializing → prepared, by exclusive store.json.new and rename — under the selection
    //     lock, after re-reading the phase: a selection may have completed meanwhile (and raised
    //     last_generation), in which case the record is left alone. Without this, a preparation
    //     that observed `initializing` early could overwrite a newer history.
    #[cfg(test)]
    if let Some(f) = hooks::BEFORE_RECORD.with(|c| c.get()) { f(root); }
    let mut record_skipped = false;
    if phase == Phase::Initializing {
        // One lock lifetime covers the re-read and the replacement: nothing can complete a
        // selection (or another record) between them. The lock is not waited for: a held lock
        // refuses this step, and the objects already created stay and are reported.
        let _lock = match crate::runtime_lock::acquire(&root_held.dir, crate::runtime_lock::Wait::NonBlocking) {
            Ok(l) => l,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Err(tx.refuse(Stage::PreparedRecord, "selection lock is held by another selection or record update; the prepared version and the initializing record stay as created, the prepared record was not written")),
            Err(e) => return Err(tx.refuse(Stage::PreparedRecord, format!("selection lock: {e}"))),
        };
        let current = read_store(&root_held.dir).map_err(|e| tx.refuse(Stage::PreparedRecord, format!("store.json re-read under lock: {e}")))?;
        match current {
            Some((Phase::Initializing, 0)) => {}
            Some((p, g)) => { record_skipped = true; tx.traces.push(format!("store.json already {p:?} with last_generation {g} (updated by another call); record left alone")); }
            None => return Err(tx.refuse(Stage::PreparedRecord, "store.json not readable as a store record under lock; nothing replaced")),
        }
        #[cfg(test)]
        if let Some(f) = hooks::UNDER_RECORD_LOCK.with(|c| c.get()) { f(root); }
        if !record_skipped {
        let new = match pfs::create_file_deletable(&root_held.dir, STORE_NEW) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return Err(tx.refuse(Stage::PreparedRecord, format!("{STORE_NEW} exists from an interrupted record replacement; kept, needs approved recovery"))),
            Err(e) => return Err(tx.refuse(Stage::PreparedRecord, format!("{STORE_NEW} creation: {e}"))),
        };
        tx.note(STORE_NEW, &root_held, STORE_NEW, id_of_file(&new));
        write_all(&mut &new, &store_json("prepared", last_generation)).map_err(|e| tx.refuse(Stage::PreparedRecord, format!("{STORE_NEW} write: {e}")))?;
        flush_file(&new).map_err(|e| tx.refuse(Stage::PreparedRecord, format!("{STORE_NEW} flush: {e}")))?;
        checkpoint(Point::NewStoreWritten).map_err(|e| tx.refuse(Stage::PreparedRecord, e.to_string()))?;
        pfs::replace_record(&root_held.dir, &root_held.path, &new, STORE_NEW, "store.json").map_err(|e| tx.refuse(Stage::PreparedRecord, format!("record rename: {e}")))?;
        // The initializing record this call may have created is now the replaced object at that
        // name; the new record takes the name. Both stay in the report with accurate presence.
        if let Some(old) = tx.created.iter_mut().find(|c| c.label == "store.json") { old.label = "store.json (initializing record, replaced by this call)".into(); }
        if let Some(last) = tx.created.last_mut() { last.label = "store.json (record replaced: initializing → prepared)".into(); last.name = "store.json".into(); }
        drop(new);
        flush_dir(&root_held).map_err(|e| tx.refuse(Stage::PreparedRecord, format!("root flush: {e}")))?;
        }
    }
    // 11. The held directories and this call's reservation must still be the objects at their
    //     names (a link or another object planted meanwhile is refused; nothing was written
    //     through a planted path, and a reservation that was removed or replaced no longer
    //     serializes the version).
    match platform::open_subdir(&root_held.dir, VERSIONS_DIR) {
        Ok(Some(now)) => match platform::same_dir(&now, &versions.dir) { Ok(true) => {}, Ok(false) => return Err(tx.refuse(Stage::Reobserve, "versions/ replaced at its name during preparation; written objects kept under the held directory")), Err(e) => return Err(tx.refuse(Stage::Reobserve, format!("versions/ identity: {e}"))) },
        Ok(None) => return Err(tx.refuse(Stage::Reobserve, "versions/ vanished from its name during preparation")),
        Err(e) => return Err(tx.refuse(Stage::Reobserve, format!("versions/ at its name: {e}"))),
    }
    if let Some(held) = &held_version_dir {
        match platform::open_subdir(&versions.dir, &target) {
            Ok(Some(now)) => match platform::same_dir(&now, &held.dir) { Ok(true) => {}, Ok(false) => return Err(tx.refuse(Stage::Reobserve, format!("{VERSIONS_DIR}/{target} replaced at its name during preparation"))), Err(e) => return Err(tx.refuse(Stage::Reobserve, format!("{VERSIONS_DIR}/{target} identity: {e}"))) },
            Ok(None) => return Err(tx.refuse(Stage::Reobserve, format!("{VERSIONS_DIR}/{target} vanished from its name during preparation"))),
            Err(e) => return Err(tx.refuse(Stage::Reobserve, format!("{VERSIONS_DIR}/{target} at its name: {e}"))),
        }
    }
    if let Some((name, id)) = &reservation_taken {
        match platform::stamp_entry(&versions.dir, name) {
            Ok(Some(now)) if id.is_some_and(|id| now.object_id() == id) => {}
            Ok(Some(_)) => return Err(tx.refuse(Stage::Reobserve, format!("{VERSIONS_DIR}/{name} was replaced during preparation; the version is no longer serialized by this call's reservation"))),
            Ok(None) => return Err(tx.refuse(Stage::Reobserve, format!("{VERSIONS_DIR}/{name} vanished during preparation; the version is no longer serialized by this call's reservation"))),
            Err(e) => return Err(tx.refuse(Stage::Reobserve, format!("{VERSIONS_DIR}/{name} re-check: {e}"))),
        }
    }
    // 12. Re-observe through the held root handle (the path is only confirmed to still name
    //     it): the result must be what the state table predicts.
    let (_, after) = observe::observe_and_judge_held(root, &root_held.dir);
    let expected_ok = match (&before.state, &after.state) {
        (State::FirstUse | State::Initializing | State::Prepared, State::Prepared) => true,
        (State::Selected { generation: a, .. }, State::Selected { generation: b, .. }) => a == b,
        // another call completed the record and a selection ran meanwhile (record left alone)
        (State::FirstUse | State::Initializing, State::Selected { .. }) if record_skipped => true,
        _ => false,
    };
    if !expected_ok { return Err(tx.refuse(Stage::Reobserve, format!("post-observation {:?} is not the predicted state", after.state))); }
    Ok(Report { outcome: if already { Outcome::AlreadyPrepared } else { Outcome::Prepared }, version_dir: format!("{VERSIONS_DIR}/{target}"), created: tx.labels(), objects: tx.objects(), before, after, notes: tx.traces.clone() })
}

/// Scan `versions/` for the requested version. Any entry of another digest, any incomplete
/// entry and any complete entry that fails the checks a fresh preparation must pass (directory
/// protection; `complete` and Core protection, single link, execute right; Core digest equal to
/// the expected digest) is a refusal; everything found is kept untouched. Only a complete entry
/// that passes every check is `AlreadyPreparedVerified`.
fn scan_versions(tx: &mut Tx, versions: &Held, req: &Request, target: &str) -> Result<Scan, Refusal> {
    let names = platform::list_names(&versions.dir).map_err(|e| tx.refuse(Stage::Uniqueness, format!("versions/ listing: {e}")))?;
    let mut verified = false;
    for name in names.iter().filter_map(|n| String::from_utf8(n.clone()).ok()) {
        let Some((version, _)) = split_dirname(&name) else { continue };
        if version != req.version { continue; }
        if name != target { return Err(tx.refuse(Stage::Uniqueness, format!("version {} already has a preparation with different bytes ({VERSIONS_DIR}/{name}); kept, not replaced", req.version))); }
        match verify_complete_candidate(&versions.dir, &name, req.expected_sha256) {
            Ok(()) => verified = true,
            Err(Candidate::Absent) => continue,
            Err(Candidate::Incomplete) => return Err(tx.refuse(Stage::Uniqueness, format!("{VERSIONS_DIR}/{name} is an incomplete preparation (no {COMPLETE_FILE}); kept, needs approved recovery"))),
            Err(Candidate::DigestDiffers) => return Err(tx.refuse(Stage::Uniqueness, format!("{VERSIONS_DIR}/{name} is complete but its {CORE_FILE} digest differs from the expected digest; kept, needs approved recovery"))),
            Err(Candidate::Refused(why)) => return Err(tx.refuse(Stage::Uniqueness, format!("{VERSIONS_DIR}/{name} {why}; kept, not accepted as prepared"))),
        }
    }
    Ok(if verified { Scan::AlreadyPreparedVerified } else { Scan::Nothing })
}

/// Why a complete candidate directory is not accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Candidate { Absent, Incomplete, DigestDiffers, Refused(String) }

/// The checks an existing `versions/<name>` must pass to count as a prepared candidate — the
/// same ones a fresh preparation gets: directory protection; `complete` present, protected,
/// single link; Core present, protected, single link, execute right (Windows: FILE_EXECUTE
/// open), digest equal to `expected_sha256`. Shared by the preparation and selection layers.
pub(crate) fn verify_complete_candidate(versions: &platform::Dir, name: &str, expected_sha256: &str) -> Result<(), Candidate> {
    let existing = match platform::open_subdir(versions, name) { Ok(Some(d)) => d, Ok(None) => return Err(Candidate::Absent), Err(e) => return Err(Candidate::Refused(format!("open: {e}"))) };
    platform::check_dir(&existing).map_err(|e| Candidate::Refused(format!("protection: {e}")))?;
    let marker = match platform::open_regular(&existing, COMPLETE_FILE) { Ok(Some(f)) => f, Ok(None) => return Err(Candidate::Incomplete), Err(e) => return Err(Candidate::Refused(format!("{COMPLETE_FILE}: {e}"))) };
    let stamp = platform::stamp_file(&marker).map_err(|e| Candidate::Refused(format!("{COMPLETE_FILE} identity: {e}")))?;
    platform::check_data(&marker, &stamp).map_err(|e| Candidate::Refused(format!("{COMPLETE_FILE} protection: {e}")))?;
    drop(marker);
    let core = match platform::open_executable(&existing, CORE_FILE) { Ok(Some(f)) => f, Ok(None) => return Err(Candidate::Refused(format!("{CORE_FILE} absent although complete is present"))), Err(e) => return Err(Candidate::Refused(format!("{CORE_FILE}: {e}"))) };
    let stamp = platform::stamp_file(&core).map_err(|e| Candidate::Refused(format!("{CORE_FILE} identity: {e}")))?;
    platform::check_exec(&core, &stamp).map_err(|e| Candidate::Refused(format!("{CORE_FILE} protection: {e}")))?;
    let digest = hash_file(&core).map_err(|e| Candidate::Refused(format!("{CORE_FILE} read: {e}")))?;
    if digest != expected_sha256 { return Err(Candidate::DigestDiffers); }
    Ok(())
}

/// `store.json` phase and last_generation read through the held root (None when the record is
/// absent or not a valid store record).
pub(crate) fn read_store(root: &platform::Dir) -> io::Result<Option<(Phase, u64)>> {
    let file = match platform::open_regular(root, "store.json")? { Some(f) => f, None => return Ok(None) };
    let mut bytes = Vec::new();
    (&file).take(observe::MAX_RECORD_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > observe::MAX_RECORD_BYTES { return Ok(None); }
    let v: serde_json::Value = match serde_json::from_slice(&bytes) { Ok(v) => v, Err(_) => return Ok(None) };
    if v.get("schema").and_then(|s| s.as_str()) != Some(STORE_SCHEMA) { return Ok(None); }
    let phase = match v.get("phase").and_then(|s| s.as_str()) { Some("initializing") => Phase::Initializing, Some("prepared") => Phase::Prepared, _ => return Ok(None) };
    let Some(g) = v.get("last_generation").and_then(|g| g.as_u64()) else { return Ok(None) };
    Ok(Some((phase, g)))
}

pub(crate) fn hash_file(file: &File) -> io::Result<String> {
    let mut hasher = Sha256::new();
    let mut reader = file;
    let mut buf = vec![0u8; 1 << 16];
    loop { match reader.read(&mut buf)? { 0 => break, n => hasher.update(&buf[..n]) } }
    Ok(format!("{:x}", hasher.finalize()))
}

// ───────────────────────── platform primitives (creating) ─────────────────────────
#[cfg(unix)]
mod pfs {
    use crate::internal_fs as ifs;
    use crate::runtime_observe::platform::Dir;
    use std::ffi::CString;
    use std::fs::File;
    use std::io;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    use std::path::Path;

    fn cstr(name: &str) -> io::Result<CString> { CString::new(name).map_err(|_| io::Error::other("name contains NUL")) }
    pub fn create_root_dir(parent: &Dir, name: &str) -> io::Result<Dir> { Ok(Dir::from_file(ifs::create_dir_at(parent.as_file(), &cstr(name)?, 0o700)?)) }
    pub fn create_subdir(parent: &Dir, name: &str) -> io::Result<Dir> { Ok(Dir::from_file(ifs::create_dir_at(parent.as_file(), &cstr(name)?, 0o700)?)) }
    pub fn create_file(parent: &Dir, name: &str) -> io::Result<File> { ifs::create_file_at(parent.as_file(), &cstr(name)?, 0o600) }
    pub fn create_file_deletable(parent: &Dir, name: &str) -> io::Result<File> { create_file(parent, name) }
    pub fn finish_executable(file: &File) -> io::Result<()> { ifs::chmod(file, 0o500) }
    pub fn fsync_file(file: &File) -> io::Result<()> { file.sync_all() }
    pub fn fsync_dir(dir: &Dir, _path: &Path) -> io::Result<()> { ifs::fsync(dir.as_file()) }
    pub fn flush_file_handle(file: &File) -> io::Result<()> { fsync_file(file) }
    pub fn flush_dir_handle(dir: &Dir, path: &Path) -> io::Result<()> { fsync_dir(dir, path) }
    /// `from` → `to` inside the held root (POSIX replace); `new` is the handle of `from`.
    pub fn replace_record(root: &Dir, _root_path: &Path, _new: &File, from: &str, to: &str) -> io::Result<()> { ifs::rename_at(root.as_file(), &cstr(from)?, &cstr(to)?) }
    /// The source is opened without following links and must be a regular file with one link.
    pub fn open_source(path: &Path) -> io::Result<File> {
        let file = std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC).open(path)
            .map_err(|e| if matches!(e.raw_os_error(), Some(libc::ELOOP) | Some(libc::EMLINK)) { io::Error::other(format!("{} is a symlink", path.display())) } else { e })?;
        let m = file.metadata()?;
        if !m.is_file() || m.nlink() != 1 { return Err(io::Error::other(format!("{} is not a regular file with one link", path.display()))); }
        Ok(file)
    }
}

#[cfg(windows)]
mod pfs {
    use crate::runtime_observe::platform::Dir;
    use crate::windows_fs as win;
    use std::ffi::OsStr;
    use std::fs::File;
    use std::io;
    use std::path::Path;
    use windows_sys::Win32::Foundation::GENERIC_WRITE;
    use windows_sys::Win32::Storage::FileSystem::{DELETE, OPEN_EXISTING};

    pub fn create_root_dir(parent: &Dir, name: &str) -> io::Result<Dir> { Ok(Dir::from_file(win::create_private_dir_at(parent.as_file(), OsStr::new(name))?)) }
    pub fn create_subdir(parent: &Dir, name: &str) -> io::Result<Dir> { Ok(Dir::from_file(win::create_dir_at(parent.as_file(), OsStr::new(name))?)) }
    pub fn create_file(parent: &Dir, name: &str) -> io::Result<File> { win::create_file_at(parent.as_file(), OsStr::new(name), win::READ | GENERIC_WRITE, win::SHARE_PINNED) }
    /// The record file is renamed over `store.json` through its handle later, which needs
    /// DELETE on the handle. (Reservations are created with it too but never deleted.)
    pub fn create_file_deletable(parent: &Dir, name: &str) -> io::Result<File> { win::create_file_at(parent.as_file(), OsStr::new(name), win::READ | GENERIC_WRITE | DELETE, win::SHARE_PINNED) }
    /// Execute right comes from the inherited full-access ACE; nothing to set.
    pub fn finish_executable(_file: &File) -> io::Result<()> { Ok(()) }
    pub fn fsync_file(file: &File) -> io::Result<()> { win::flush(file) }
    pub fn fsync_dir(dir: &Dir, path: &Path) -> io::Result<()> { win::flush_dir(path, dir.as_file()) }
    pub fn flush_file_handle(file: &File) -> io::Result<()> { fsync_file(file) }
    pub fn flush_dir_handle(dir: &Dir, path: &Path) -> io::Result<()> { fsync_dir(dir, path) }
    /// Rename the open `new` (which must carry DELETE) over `to` inside the pinned root.
    pub fn replace_record(root: &Dir, root_path: &Path, new: &File, _from: &str, to: &str) -> io::Result<()> { win::rename(new, root.as_file(), root_path, OsStr::new(to), true) }
    pub fn open_source(path: &Path) -> io::Result<File> {
        let file = win::open_file(path, win::READ, win::SHARE_ALL, OPEN_EXISTING)?;
        if !win::Stamp::of(&file)?.is_single_regular() { return Err(io::Error::other(format!("{} is not a regular file with one link", path.display()))); }
        Ok(file)
    }
}

#[cfg(test)]
#[path = "runtime_prepare_tests.rs"]
mod tests;
