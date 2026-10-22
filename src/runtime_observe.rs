//! Runtime update — **read-only observation** of a runtime store at an explicitly given path,
//! producing a [`crate::runtime_state::Observation`] for the pure judge.
//!
//! Scope (Implementation Plan §1.1, §3, §3.1; 2026-10-23): this layer only reads. It creates
//! nothing, changes no ACL or mode, takes no lock, removes nothing and never executes a
//! candidate. It does not decide the product store path either: the caller hands in a path
//! (tests use isolated fixtures). A verdict derived from one observation is **not** a
//! permission to run anything: the files can change the moment the observation ends.
//!
//! Path safety, three distinct mechanisms with different handle lifetimes:
//! 1. **Step-by-step handle-relative walk** from the volume root (Unix `openat` with
//!    `O_NOFOLLOW|O_DIRECTORY`, Windows `NtCreateFile` with `RootDirectory`): each ancestor is
//!    opened relative to the previous handle and checked on its own handle, then that previous
//!    handle is dropped; only the handle of the root itself survives the walk. Ancestors are
//!    therefore *not* pinned for the duration of the observation.
//! 2. **Held observation handles**: the root and every entry read below it (records,
//!    `versions/`, the version directory, `complete`, the Core file) stay open until the final
//!    re-check, and every entry is opened relative to its held parent. No path string is
//!    re-resolved during the observation, so an ancestor or intermediate directory swapped in
//!    the meantime cannot redirect a read outside the originally opened tree.
//! 3. **Final root re-walk**: the root path is walked once more (mechanism 1) and must name the
//!    same directory as the held root handle, otherwise the observation is blocked.
//!
//! Classification:
//! - a record is `Absent` only on "not found"; every other read failure (access denied, I/O
//!   error, a non-regular entry, a link) is a **blocked** reason, never "absent" and never first
//!   use; the root "does not exist" only when its parent answers "not found" for it;
//! - JSON records are size-limited (`MAX_RECORD_BYTES`), must carry the expected `schema`,
//!   generations are `u64`, version identifiers match `VERSION_RE`, digests are 64 lowercase hex
//!   characters; anything else makes the record `Invalid`;
//! - file link counts must be 1, directories are opened as directories, version identifiers are
//!   validated so a selection can never point outside `versions/`;
//! - a protection check passes only when it was actually performed and succeeded; after a
//!   failed check nothing below that entry is read; a check that cannot be performed on this
//!   platform is a blocked reason, not a pass;
//! - `complete` is an existence marker (contract §1.1 prepare step): its protection is checked,
//!   its content is not interpreted;
//! - see [`COMPARED`] and [`LIMITS`] for what change detection covers and what it misses.
use crate::runtime_state::{Intent, Observation, Phase, Record, Selection, Store};
use regex::Regex;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::LazyLock;

pub const MAX_RECORD_BYTES: u64 = 64 * 1024;
pub const STORE_SCHEMA: &str = "gil-runtime-store/1";
pub const INTENT_SCHEMA: &str = "gil-runtime-intent/1";
pub const SELECTION_SCHEMA: &str = "gil-runtime-selection/1";
pub const VERSIONS_DIR: &str = "versions";
pub const COMPLETE_FILE: &str = "complete";
#[cfg(windows)]
pub const CORE_FILE: &str = "gil.exe";
#[cfg(not(windows))]
pub const CORE_FILE: &str = "gil";
/// Root entries whose identity is recorded before the first read and compared after the last.
const ROOT_ENTRIES: [&str; 4] = ["store.json", "intent.json", "selection.json", VERSIONS_DIR];

/// What change detection actually compares (all through held handles, no path re-resolution).
pub const COMPARED: &[&str] = &[
    "root directory listing (names) before the first read and after the last",
    "identity of store.json, intent.json, selection.json and versions/ as root entries, recorded before the first read and compared after the last (device/volume, inode/file id, size, link count, mode or attributes, mtime, ctime/change time)",
    "identity of every opened handle (root, records, versions/, the version directory, complete, the Core file) at open time and after the last read",
    "protection checks (owner, mode or DACL structure, link count, extended ACL) re-run on every held handle after the last read",
    "the root path walked again from the volume root after the last read must name the same directory",
];

/// What this observation does **not** guarantee.
pub const LIMITS: &[&str] = &[
    "not an atomic snapshot: files are read one after another; a change inside the window is detected only through the identities above and the final re-walk",
    "a content change that leaves size, mtime and ctime/change time unchanged is not detected (requires owner-level timestamp manipulation)",
    "entries that are not read (other version directories, unrelated files) are not compared",
    "changes after the final re-check are not detected; a verdict is not permission to run anything; the launcher must re-verify under its own pinned handles",
    "ancestor directories are checked for links, owner and world/group writability (Unix) or reparse points (Windows) only; their ACLs are not inspected",
    "owner and protection checks describe the account this process runs as, not the person in the chat",
    "the product store path is not decided here; the caller passes an explicit path",
];

static VERSION_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9.+_-]{0,119}$").unwrap());
static SHA256_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[0-9a-f]{64}$").unwrap());

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observed {
    pub observation: Observation,
    /// Checks that were actually performed and passed, in order.
    pub checks: Vec<String>,
    /// Owner identity of the process, as compared against `store.json.owner`.
    pub owner: String,
}

pub fn observe(root: &Path) -> Observed {
    let mut ob = Observer { blocked: vec![], checks: vec![], traces: vec![], owner: platform::owner_text(), held: vec![], pre: vec![] };
    let observation = ob.run(root);
    Observed { observation, checks: ob.checks, owner: ob.owner }
}

/// Observation followed by the pure judge. Still not an execution permission.
pub fn observe_and_judge(root: &Path) -> (Observed, crate::runtime_state::Verdict) {
    let observed = observe(root);
    let verdict = crate::runtime_state::judge(&observed.observation);
    (observed, verdict)
}

/// Test-only hook: called after `store.json` was read, before the other records, so a test can
/// change the store (or its ancestors) in between and prove the change is detected or ignored
/// as the contract requires.
#[cfg(test)]
pub(crate) mod hooks {
    use std::cell::Cell;
    use std::path::Path;
    thread_local! {
        pub static BETWEEN: Cell<Option<fn(&Path)>> = const { Cell::new(None) };
        /// When set, every directory-handle duplicate fails (simulates EMFILE / handle limits).
        pub static FAIL_DUP: Cell<bool> = const { Cell::new(false) };
    }
    pub fn run(root: &Path) { if let Some(f) = BETWEEN.with(|c| c.get()) { f(root); } }
    pub fn set(f: Option<fn(&Path)>) { BETWEEN.with(|c| c.set(f)); }
}

#[derive(Clone, Copy)]
enum Kind { Root, Dir, Data, Exec }
enum Handle { Dir(platform::Dir), File(File) }
struct Held { name: String, handle: Handle, stamp: platform::Stamp, kind: Kind }

struct Observer {
    blocked: Vec<String>,
    checks: Vec<String>,
    traces: Vec<String>,
    owner: String,
    held: Vec<Held>,
    pre: Vec<(String, Option<platform::Stamp>)>,
}

impl Observer {
    /// Duplicate a directory handle so one copy stays held for the re-check while the other is
    /// used for relative opens; a failed duplicate is a blocked reason (test hook can inject it).
    fn dup(&self, dir: &platform::Dir) -> std::io::Result<platform::Dir> {
        #[cfg(test)]
        if hooks::FAIL_DUP.with(|c| c.get()) { return Err(std::io::Error::other("injected duplicate failure")); }
        dir.clone_handle()
    }
    fn block(&mut self, what: impl Into<String>) { self.blocked.push(what.into()); }
    fn pass(&mut self, what: impl Into<String>) { self.checks.push(what.into()); }
    fn finish(&self, root_exists: bool, store: Record<Store>, intent: Record<Intent>, selection: Record<Selection>) -> Observation {
        Observation { blocked: self.blocked.clone(), root_exists, store, intent, selection, traces: self.traces.clone() }
    }
    fn empty(&self, root_exists: bool) -> Observation { self.finish(root_exists, Record::Absent, Record::Absent, Record::Absent) }

    fn run(&mut self, root: &Path) -> Observation {
        // 0. Path shape (absolute, no `.`/`..`; Windows: drive-letter path only).
        if let Err(e) = platform::check_path_shape(root) {
            self.block(format!("root path refused ({e}): {}", root.display()));
            return self.empty(false);
        }
        // 1. Handle-relative walk from the volume root; ancestors checked on their handles.
        let root_dir = match platform::walk(root) {
            Ok(Some(d)) => d,
            Ok(None) => { self.pass("path walk: step-by-step handle-relative, no links followed; root not found (verified first-use candidate)"); return self.empty(false); }
            Err(e) => { self.block(format!("path walk: {e}")); return self.empty(false); }
        };
        self.pass("path walk: step-by-step handle-relative, no links followed; root opened as a directory and held");
        let root_stamp = match platform::stamp_dir(&root_dir) { Ok(s) => s, Err(e) => { self.block(format!("root identity: {e}")); return self.empty(true); } };
        let listing_before = match platform::list_names(&root_dir) { Ok(l) => l, Err(e) => { self.block(format!("root listing: {e}")); return self.empty(true); } };
        // 2. Identity of the root entries before anything is read.
        for name in ROOT_ENTRIES {
            match platform::stamp_entry(&root_dir, name) {
                Ok(s) => self.pre.push((name.to_owned(), s)),
                Err(e) => { self.block(format!("{name} pre-check: {e}")); return self.empty(true); }
            }
        }
        // 3. Root protection; on failure nothing below is read.
        match platform::check_root(&root_dir) {
            Ok(desc) => self.pass(format!("root protection: {desc}")),
            Err(e) => { self.block(format!("root protection: {e} (nothing below the root was read)")); return self.empty(true); }
        }
        let root_dir = match self.dup(&root_dir) {
            Ok(d) => { self.held.push(Held { name: "<root>".into(), handle: Handle::Dir(root_dir), stamp: root_stamp, kind: Kind::Root }); d }
            Err(e) => { self.block(format!("root handle duplicate: {e} (nothing below the root was read)")); return self.empty(true); }
        };
        // 4. Records.
        let store = self.record(&root_dir, "store.json", parse_store);
        #[cfg(test)]
        hooks::run(root);
        let intent = self.record(&root_dir, "intent.json", parse_intent);
        let selection = self.record(&root_dir, "selection.json", parse_selection_raw);
        let store = match store {
            Record::Valid((s, owner)) if owner == self.owner => Record::Valid(s),
            Record::Valid((_, owner)) => { self.traces.push(format!("store.json owner differs from the process owner ({owner} vs {})", self.owner)); Record::Invalid }
            Record::Absent => Record::Absent,
            Record::Invalid => Record::Invalid,
        };
        // 5. Referenced version: protection of versions/ and the version directory, complete
        //    marker, Core digest.
        let selection = match selection {
            Record::Valid(raw) => {
                let complete = self.version_complete(&root_dir, &raw);
                Record::Valid(Selection { generation: raw.generation, version: raw.version, sha256: raw.sha256, version_complete: complete })
            }
            Record::Absent => Record::Absent,
            Record::Invalid => Record::Invalid,
        };
        // 6. Change detection.
        self.recheck(root, &root_dir, &listing_before);
        self.finish(true, store, intent, selection)
    }

    /// Open a record relative to the root without following links; absent only on not-found;
    /// identity recorded and protection checked before any byte is read.
    fn record<T>(&mut self, dir: &platform::Dir, name: &str, parse: fn(&Value) -> Option<T>) -> Record<T> {
        let file = match platform::open_regular(dir, name) {
            Ok(Some(f)) => f,
            Ok(None) => return Record::Absent,
            Err(e) => { self.block(format!("{name}: {e}")); return Record::Absent; }
        };
        let stamp = match platform::stamp_file(&file) { Ok(s) => s, Err(e) => { self.block(format!("{name} identity: {e}")); return Record::Absent; } };
        let size = stamp.size();
        let protection = platform::check_data(&file, &stamp);
        self.held.push(Held { name: name.into(), handle: Handle::File(file), stamp, kind: Kind::Data });
        match protection {
            Ok(desc) => self.pass(format!("{name} protection: {desc}")),
            Err(e) => { self.block(format!("{name} protection: {e} (not read)")); return Record::Absent; }
        }
        if size > MAX_RECORD_BYTES { self.traces.push(format!("{name}: {size} bytes exceeds {MAX_RECORD_BYTES}")); return Record::Invalid; }
        let file = match &self.held.last().unwrap().handle { Handle::File(f) => f, Handle::Dir(_) => unreachable!() };
        let mut bytes = Vec::with_capacity(size as usize);
        if let Err(e) = file.take(MAX_RECORD_BYTES + 1).read_to_end(&mut bytes) { self.block(format!("{name} read: {e}")); return Record::Absent; }
        if bytes.len() as u64 > MAX_RECORD_BYTES { return Record::Invalid; }
        let value: Value = match serde_json::from_slice(&bytes) { Ok(v) => v, Err(e) => { self.traces.push(format!("{name}: invalid JSON: {e}")); return Record::Invalid; } };
        match parse(&value) { Some(t) => Record::Valid(t), None => { self.traces.push(format!("{name}: schema or field validation failed")); Record::Invalid } }
    }

    fn open_checked_dir(&mut self, parent: &platform::Dir, name: &str, label: &str) -> Option<platform::Dir> {
        let dir = match platform::open_subdir(parent, name) {
            Ok(Some(d)) => d,
            Ok(None) => { self.traces.push(format!("{label} absent")); return None; }
            Err(e) => { self.block(format!("{label}: {e}")); return None; }
        };
        let stamp = match platform::stamp_dir(&dir) { Ok(s) => s, Err(e) => { self.block(format!("{label} identity: {e}")); return None; } };
        let protection = platform::check_dir(&dir);
        let handle = self.dup(&dir);
        self.held.push(Held { name: label.into(), handle: Handle::Dir(dir), stamp, kind: Kind::Dir });
        match (protection, handle) {
            (Ok(d), Ok(handle)) => { self.pass(format!("{label} protection: {d}")); Some(handle) }
            (Err(e), _) => { self.block(format!("{label} protection: {e} (nothing below was read)")); None }
            (Ok(_), Err(e)) => { self.block(format!("{label} handle duplicate: {e} (nothing below was read)")); None }
        }
    }

    /// `versions/<version>-<sha12>/{complete, CORE_FILE}` with the Core digest equal to the
    /// selection's digest. Missing pieces are "incomplete" (→ damaged); access or protection
    /// failures block and stop the descent.
    fn version_complete(&mut self, root: &platform::Dir, sel: &RawSelection) -> bool {
        let dirname = format!("{}-{}", sel.version, &sel.sha256[..12]);
        let Some(versions) = self.open_checked_dir(root, VERSIONS_DIR, "versions/") else { return false };
        let label = format!("versions/{dirname}");
        let Some(vdir) = self.open_checked_dir(&versions, &dirname, &label) else { return false };
        // complete: existence marker; protection checked, content not interpreted.
        match platform::open_regular(&vdir, COMPLETE_FILE) {
            Ok(Some(marker)) => {
                let stamp = match platform::stamp_file(&marker) { Ok(s) => s, Err(e) => { self.block(format!("{label}/{COMPLETE_FILE} identity: {e}")); return false; } };
                let protection = platform::check_data(&marker, &stamp);
                self.held.push(Held { name: format!("{label}/{COMPLETE_FILE}"), handle: Handle::File(marker), stamp, kind: Kind::Data });
                match protection {
                    Ok(d) => self.pass(format!("{label}/{COMPLETE_FILE} protection: {d}")),
                    Err(e) => { self.block(format!("{label}/{COMPLETE_FILE} protection: {e} (Core not read)")); return false; }
                }
            }
            Ok(None) => { self.traces.push(format!("{label}/{COMPLETE_FILE} absent")); return false; }
            Err(e) => { self.block(format!("{label}/{COMPLETE_FILE}: {e}")); return false; }
        }
        let core = match platform::open_executable(&vdir, CORE_FILE) {
            Ok(Some(f)) => f,
            Ok(None) => { self.traces.push(format!("{label}/{CORE_FILE} absent")); return false; }
            Err(e) => { self.block(format!("{label}/{CORE_FILE}: {e}")); return false; }
        };
        let stamp = match platform::stamp_file(&core) { Ok(s) => s, Err(e) => { self.block(format!("{label}/{CORE_FILE} identity: {e}")); return false; } };
        let protection = platform::check_exec(&core, &stamp);
        self.held.push(Held { name: format!("{label}/{CORE_FILE}"), handle: Handle::File(core), stamp, kind: Kind::Exec });
        match protection {
            Ok(d) => self.pass(format!("{label}/{CORE_FILE} protection: {d}")),
            Err(e) => { self.block(format!("{label}/{CORE_FILE} protection: {e} (not hashed)")); return false; }
        }
        let core = match &self.held.last().unwrap().handle { Handle::File(f) => f, Handle::Dir(_) => unreachable!() };
        let mut hasher = Sha256::new();
        let mut reader = core;
        let mut buf = [0u8; 1 << 16];
        loop {
            match reader.read(&mut buf) { Ok(0) => break, Ok(n) => hasher.update(&buf[..n]), Err(e) => { self.block(format!("{label}/{CORE_FILE} read: {e}")); return false; } }
        }
        let digest = format!("{:x}", hasher.finalize());
        if digest != sel.sha256 { self.traces.push(format!("{label}/{CORE_FILE} digest {} != selection {}", &digest[..12], &sel.sha256[..12])); return false; }
        self.pass(format!("{label}: complete marker present, {CORE_FILE} digest equals selection"));
        true
    }

    /// Listing, root-entry identities, held-handle identities and protections, and the root
    /// path itself are compared again after the last read.
    fn recheck(&mut self, root_path: &Path, root_dir: &platform::Dir, listing_before: &[Vec<u8>]) {
        match platform::list_names(root_dir) {
            Ok(after) if after == listing_before => {}
            Ok(_) => self.block("root listing changed during observation"),
            Err(e) => self.block(format!("root re-listing: {e}")),
        }
        let pre = std::mem::take(&mut self.pre);
        for (name, before) in &pre {
            match platform::stamp_entry(root_dir, name) {
                Ok(now) if &now == before => {}
                Ok(_) => self.block(format!("{name} changed during observation (root entry identity)")),
                Err(e) => self.block(format!("{name} re-check: {e}")),
            }
        }
        self.pre = pre;
        let held = std::mem::take(&mut self.held);
        for h in &held {
            let now = match &h.handle { Handle::Dir(d) => platform::stamp_dir(d), Handle::File(f) => platform::stamp_file(f) };
            match now {
                Ok(n) if n == h.stamp => {}
                Ok(_) => self.block(format!("{} changed during observation (handle identity)", h.name)),
                Err(e) => self.block(format!("{} re-check: {e}", h.name)),
            }
            let again = match (&h.handle, h.kind) {
                (Handle::Dir(d), Kind::Root) => platform::check_root(d),
                (Handle::Dir(d), _) => platform::check_dir(d),
                (Handle::File(f), Kind::Exec) => platform::check_exec(f, &h.stamp),
                (Handle::File(f), _) => platform::check_data(f, &h.stamp),
            };
            if let Err(e) = again { self.block(format!("{} protection changed during observation: {e}", h.name)); }
        }
        self.held = held;
        match platform::walk(root_path) {
            Ok(Some(now)) => match platform::same_dir(&now, root_dir) {
                Ok(true) => {}
                Ok(false) => self.block("root replaced at its path during observation"),
                Err(e) => self.block(format!("root re-walk identity: {e}")),
            },
            Ok(None) => self.block("root no longer at its path during observation"),
            Err(e) => self.block(format!("root re-walk: {e}")),
        }
        if self.blocked.is_empty() { self.pass("re-check: listing, root-entry identities, held-handle identities and protections, and the root path unchanged across the observation"); }
    }
}

fn u64_field(v: &Value, key: &str) -> Option<u64> { v.get(key)?.as_u64() }
fn str_field<'a>(v: &'a Value, key: &str) -> Option<&'a str> { v.get(key)?.as_str() }
fn version_field(v: &Value) -> Option<String> { let s = str_field(v, "version")?; VERSION_RE.is_match(s).then(|| s.to_owned()) }
fn sha_field(v: &Value) -> Option<String> { let s = str_field(v, "sha256")?; SHA256_RE.is_match(s).then(|| s.to_owned()) }

fn parse_store(v: &Value) -> Option<(Store, String)> {
    if str_field(v, "schema")? != STORE_SCHEMA { return None; }
    let phase = match str_field(v, "phase")? { "initializing" => Phase::Initializing, "prepared" => Phase::Prepared, _ => return None };
    let last_generation = u64_field(v, "last_generation")?;
    let owner = str_field(v, "owner")?.to_owned();
    Some((Store { phase, last_generation }, owner))
}
fn parse_intent(v: &Value) -> Option<Intent> {
    if str_field(v, "schema")? != INTENT_SCHEMA { return None; }
    Some(Intent { generation: u64_field(v, "generation")?, version: version_field(v)?, sha256: sha_field(v)? })
}
#[derive(Debug, Clone)]
struct RawSelection { generation: u64, version: String, sha256: String }
fn parse_selection_raw(v: &Value) -> Option<RawSelection> {
    if str_field(v, "schema")? != SELECTION_SCHEMA { return None; }
    Some(RawSelection { generation: u64_field(v, "generation")?, version: version_field(v)?, sha256: sha_field(v)? })
}

/// Parser for the `FILE_NAMES_INFORMATION` chain returned by `NtQueryDirectoryFile`, kept
/// platform-independent so it is unit-tested on every host. `used` is
/// `IO_STATUS_BLOCK.Information`: only that prefix of the buffer is trusted. Every header,
/// name length (must be even and inside the prefix) and `NextEntryOffset` (must advance past
/// the header, without overflow, and stay inside the prefix) is checked; `"."` and `".."` are
/// skipped; any inconsistency is an error, never a partial listing.
#[cfg(any(windows, test))]
mod dir_names {
    pub const HEADER: usize = 12; // NextEntryOffset u32, FileIndex u32, FileNameLength u32
    fn u32_at(buf: &[u8], at: usize) -> u32 { u32::from_le_bytes([buf[at], buf[at + 1], buf[at + 2], buf[at + 3]]) }
    pub fn parse(buf: &[u8], used: usize) -> Result<Vec<String>, String> {
        if used == 0 { return Err("directory answer carried no bytes".into()); }
        let Some(buf) = buf.get(..used) else { return Err(format!("directory answer claims {used} bytes but the buffer holds {}", buf.len())) };
        let mut names = Vec::new();
        let mut offset = 0usize;
        loop {
            let end = offset.checked_add(HEADER).filter(|e| *e <= buf.len()).ok_or_else(|| format!("entry header at {offset} exceeds the {used}-byte answer"))?;
            let next = u32_at(buf, offset) as usize;
            let name_len = u32_at(buf, offset + 8) as usize;
            if name_len % 2 != 0 { return Err(format!("entry at {offset}: odd FileNameLength {name_len}")); }
            let name_end = end.checked_add(name_len).filter(|e| *e <= buf.len()).ok_or_else(|| format!("entry at {offset}: name of {name_len} bytes exceeds the {used}-byte answer"))?;
            let wide: Vec<u16> = buf[end..name_end].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
            let name = String::from_utf16_lossy(&wide);
            if name != "." && name != ".." { names.push(name); }
            if next == 0 { break; }
            if next < HEADER + name_len { return Err(format!("entry at {offset}: NextEntryOffset {next} does not advance past the entry")); }
            offset = offset.checked_add(next).filter(|o| *o < buf.len()).ok_or_else(|| format!("entry at {offset}: NextEntryOffset {next} leaves the {used}-byte answer"))?;
        }
        Ok(names)
    }
}

// ───────────────────────── platform primitives (read-only) ─────────────────────────
#[cfg(unix)]
mod platform {
    use std::ffi::CString;
    use std::fs::File;
    use std::io;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::fs::MetadataExt;
    use std::path::{Component, Path};

    /// An open directory handle; every child is opened relative to it, never by path.
    pub struct Dir(File);
    impl Dir {
        /// A second handle to the same open directory (dup), used to keep a directory both held
        /// for re-checks and usable for relative opens. Failure is reported, never a panic.
        pub fn clone_handle(&self) -> io::Result<Dir> { Ok(Dir(self.0.try_clone()?)) }
    }
    /// Everything compared for change detection.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Stamp { dev: u64, ino: u64, mode: u32, uid: u32, gid: u32, nlink: u64, size: u64, mtime: i128, ctime: i128 }
    impl Stamp {
        pub fn size(&self) -> u64 { self.size }
        fn of(m: &std::fs::Metadata) -> Self {
            Stamp { dev: m.dev(), ino: m.ino(), mode: m.mode(), uid: m.uid(), gid: m.gid(), nlink: m.nlink(), size: m.size(),
                mtime: m.mtime() as i128 * 1_000_000_000 + m.mtime_nsec() as i128, ctime: m.ctime() as i128 * 1_000_000_000 + m.ctime_nsec() as i128 }
        }
    }

    fn cstr(name: &str) -> io::Result<CString> { CString::new(name).map_err(|_| io::Error::other("name contains NUL")) }
    fn not_found(e: &io::Error) -> bool { e.kind() == io::ErrorKind::NotFound }
    fn euid() -> u32 { unsafe { libc::geteuid() } }
    pub fn owner_text() -> String { format!("uid:{}", euid()) }

    pub fn check_path_shape(root: &Path) -> io::Result<()> {
        if !root.is_absolute() { return Err(io::Error::other("must be absolute")); }
        if root.components().any(|c| matches!(c, Component::ParentDir | Component::CurDir)) { return Err(io::Error::other("must not contain . or ..")); }
        if root.parent().is_none() { return Err(io::Error::other("root has no parent")); }
        Ok(())
    }

    /// `openat(O_NOFOLLOW | O_DIRECTORY)` reports a symlink as ELOOP (Linux) or ENOTDIR
    /// (macOS); the entry type is re-read without following to name the reason.
    fn describe_open_error(parent: &File, name: &CString, at: &Path, e: io::Error) -> io::Error {
        if matches!(e.raw_os_error(), Some(libc::ELOOP) | Some(libc::ENOTDIR)) {
            let mut st: libc::stat = unsafe { std::mem::zeroed() };
            if unsafe { libc::fstatat(parent.as_raw_fd(), name.as_ptr(), &mut st, libc::AT_SYMLINK_NOFOLLOW) } == 0 {
                return if st.st_mode & libc::S_IFMT == libc::S_IFLNK { io::Error::other(format!("{} is a symlink", at.display())) } else { io::Error::other(format!("{} is not a directory", at.display())) };
            }
        }
        io::Error::new(e.kind(), format!("{}: {e}", at.display()))
    }

    /// Open `/`, then each ancestor relative to the previous handle (`openat`, `O_NOFOLLOW`,
    /// `O_DIRECTORY`), checking it on its own handle (owned by root or this user, not
    /// group/world writable unless sticky) and dropping the previous handle. Finally the root
    /// itself, whose handle is the only one returned; `Ok(None)` only when its parent answers
    /// "not found".
    pub fn walk(root: &Path) -> io::Result<Option<Dir>> {
        let fd = unsafe { libc::open(c"/".as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC) };
        if fd < 0 { return Err(io::Error::last_os_error()); }
        let mut cur = unsafe { File::from_raw_fd(fd) };
        let mut at = std::path::PathBuf::from("/");
        let parent = root.parent().ok_or_else(|| io::Error::other("root has no parent"))?;
        for comp in parent.components() {
            let Component::Normal(name) = comp else { continue };
            at.push(name);
            let name = CString::new(name.as_encoded_bytes()).map_err(|_| io::Error::other("path contains NUL"))?;
            let next = crate::internal_fs::child_dir(&cur, &name).map_err(|e| describe_open_error(&cur, &name, &at, e))?;
            let m = next.metadata()?;
            if m.uid() != 0 && m.uid() != euid() { return Err(io::Error::other(format!("{} owned by uid {}", at.display(), m.uid()))); }
            if m.mode() & 0o022 != 0 && m.mode() & 0o1000 == 0 { return Err(io::Error::other(format!("{} is group/world writable without the sticky bit (mode {:o})", at.display(), m.mode() & 0o7777))); }
            cur = next;
        }
        let Some(last) = root.file_name() else { return Err(io::Error::other("root has no name")) };
        at.push(last);
        let name = CString::new(last.as_encoded_bytes()).map_err(|_| io::Error::other("path contains NUL"))?;
        match crate::internal_fs::child_dir(&cur, &name) {
            Ok(f) => Ok(Some(Dir(f))),
            Err(e) if not_found(&e) => Ok(None),
            Err(e) => Err(describe_open_error(&cur, &name, &at, e)),
        }
    }
    pub fn same_dir(a: &Dir, b: &Dir) -> io::Result<bool> {
        let (x, y) = (a.0.metadata()?, b.0.metadata()?);
        Ok(x.dev() == y.dev() && x.ino() == y.ino())
    }
    pub fn open_subdir(dir: &Dir, name: &str) -> io::Result<Option<Dir>> {
        match crate::internal_fs::child_dir(&dir.0, &cstr(name)?) { Ok(f) => Ok(Some(Dir(f))), Err(e) if not_found(&e) => Ok(None), Err(e) => Err(e) }
    }
    pub fn open_regular(dir: &Dir, name: &str) -> io::Result<Option<File>> {
        match crate::internal_fs::regular_at(&dir.0, &cstr(name)?) { Ok(f) => Ok(Some(f)), Err(e) if not_found(&e) => Ok(None), Err(e) => Err(e) }
    }
    pub fn open_executable(dir: &Dir, name: &str) -> io::Result<Option<File>> { open_regular(dir, name) }
    pub fn stamp_file(f: &File) -> io::Result<Stamp> { Ok(Stamp::of(&f.metadata()?)) }
    pub fn stamp_dir(d: &Dir) -> io::Result<Stamp> { stamp_file(&d.0) }
    /// `fstatat(AT_SYMLINK_NOFOLLOW)` of one entry under the held directory; `None` only on
    /// not-found. A symlink entry stamps as a symlink and is refused later when opened.
    pub fn stamp_entry(dir: &Dir, name: &str) -> io::Result<Option<Stamp>> {
        let name = cstr(name)?;
        let mut st: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstatat(dir.0.as_raw_fd(), name.as_ptr(), &mut st, libc::AT_SYMLINK_NOFOLLOW) } < 0 {
            let e = io::Error::last_os_error();
            return if not_found(&e) { Ok(None) } else { Err(e) };
        }
        #[allow(clippy::unnecessary_cast)]
        Ok(Some(Stamp { dev: st.st_dev as u64, ino: st.st_ino as u64, mode: st.st_mode as u32, uid: st.st_uid, gid: st.st_gid, nlink: st.st_nlink as u64, size: st.st_size as u64,
            mtime: st.st_mtime as i128 * 1_000_000_000 + st.st_mtime_nsec as i128, ctime: st.st_ctime as i128 * 1_000_000_000 + st.st_ctime_nsec as i128 }))
    }
    pub fn list_names(dir: &Dir) -> io::Result<Vec<Vec<u8>>> {
        let mut names: Vec<Vec<u8>> = crate::internal_fs::names(&dir.0)?.into_iter().map(|c| c.into_bytes()).collect();
        names.sort();
        Ok(names)
    }

    #[cfg(target_os = "macos")]
    fn has_extended_acl(f: &File) -> io::Result<bool> {
        // acl_get_fd returns NULL with ENOENT when no ACL is set.
        unsafe extern "C" { fn acl_get_fd(fd: i32) -> *mut libc::c_void; fn acl_free(obj: *mut libc::c_void) -> i32; }
        let acl = unsafe { acl_get_fd(f.as_raw_fd()) };
        if acl.is_null() {
            let e = io::Error::last_os_error();
            return if e.raw_os_error() == Some(libc::ENOENT) { Ok(false) } else { Err(e) };
        }
        unsafe { acl_free(acl); }
        Ok(true)
    }
    #[cfg(not(target_os = "macos"))]
    fn has_extended_acl(_f: &File) -> io::Result<bool> {
        Err(io::Error::other("extended ACL check not implemented on this platform; not counted as a pass"))
    }

    /// Owner, exact mode, no extended ACL; for files additionally exactly one link (a directory's
    /// link count reflects its subdirectories and is not a hard-link indicator).
    fn common(f: &File, expect_mode: u32, directory: bool, what: &str) -> io::Result<String> {
        let m = f.metadata()?;
        if m.is_dir() != directory { return Err(io::Error::other(format!("{what}: unexpected file type"))); }
        if m.uid() != euid() { return Err(io::Error::other(format!("{what}: owned by uid {} not {}", m.uid(), euid()))); }
        if !directory && m.nlink() != 1 { return Err(io::Error::other(format!("{what}: link count {} is not 1", m.nlink()))); }
        if m.mode() & 0o7777 != expect_mode { return Err(io::Error::other(format!("{what}: mode {:o} is not {:o}", m.mode() & 0o7777, expect_mode))); }
        if has_extended_acl(f)? { return Err(io::Error::other(format!("{what}: an extended ACL is present"))); }
        Ok(format!("owner uid {}, mode {:o}, {}no extended ACL", euid(), expect_mode, if directory { "" } else { "one link, " }))
    }
    pub fn check_root(dir: &Dir) -> io::Result<String> { common(&dir.0, 0o700, true, "root") }
    pub fn check_dir(dir: &Dir) -> io::Result<String> { common(&dir.0, 0o700, true, "directory") }
    pub fn check_data(f: &File, _s: &Stamp) -> io::Result<String> { common(f, 0o600, false, "data file") }
    pub fn check_exec(f: &File, _s: &Stamp) -> io::Result<String> { common(f, 0o500, false, "executable") }
}

#[cfg(windows)]
mod platform {
    use crate::windows_fs as win;
    use std::ffi::OsStr;
    use std::fs::File;
    use std::io;
    use std::os::windows::io::AsRawHandle;
    use std::path::{Component, Path, PathBuf};
    use windows_sys::Wdk::Storage::FileSystem::{FILE_NAMES_INFORMATION, FileNamesInformation, NtQueryDirectoryFile};
    use windows_sys::Win32::Foundation::STATUS_NO_MORE_FILES;
    use windows_sys::Win32::Storage::FileSystem::FILE_EXECUTE;
    use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;

    /// An open directory handle; every child is opened relative to it with `NtCreateFile`
    /// (`RootDirectory` = this handle), never by path.
    pub struct Dir(File);
    impl Dir {
        pub fn clone_handle(&self) -> io::Result<Dir> { Ok(Dir(self.0.try_clone()?)) }
    }
    /// Everything compared for change detection: volume + 128-bit id, size, link count,
    /// attributes, creation / last-write / change time (`windows_fs::Stamp`).
    pub type Stamp = win::Stamp;
    pub trait StampSize { fn size(&self) -> u64; }
    impl StampSize for Stamp { fn size(&self) -> u64 { self.size } }

    fn not_found(e: &io::Error) -> bool { e.kind() == io::ErrorKind::NotFound }
    pub fn owner_text() -> String {
        win::user().and_then(|u| win::sid_text(win::sid(&u))).map(|s| format!("sid:{s}")).unwrap_or_else(|e| format!("sid:unavailable:{e}"))
    }
    /// `C:\…` / `\\?\C:\…` only, no `.`/`..` (`windows_fs::absolute`).
    pub fn check_path_shape(root: &Path) -> io::Result<()> {
        win::absolute(root).map_err(|_| io::Error::other("must be an absolute drive path without . or .."))?;
        if root.file_name().is_none() { return Err(io::Error::other("root has no name")); }
        Ok(())
    }
    /// Open the volume root by its prefix, then each ancestor relative to the previous handle
    /// (reparse points are opened as themselves and refused), dropping the previous handle as
    /// the walk advances, then the root itself, whose handle is the only one returned.
    pub fn walk(root: &Path) -> io::Result<Option<Dir>> {
        let mut comps = root.components();
        let mut volume = PathBuf::new();
        match comps.next() { Some(p @ Component::Prefix(_)) => volume.push(p), _ => return Err(io::Error::other("no drive prefix")) }
        match comps.next() { Some(Component::RootDir) => volume.push("\\"), _ => return Err(io::Error::other("no root component")) }
        let mut cur = win::open_dir(&volume, win::SHARE_ALL, false).map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", volume.display())))?;
        let mut at = volume;
        let parent_depth = root.components().count() - 3; // prefix, root dir, final name
        for (i, comp) in comps.enumerate() {
            let Component::Normal(name) = comp else { return Err(io::Error::other("unexpected path component")) };
            at.push(name);
            if i < parent_depth {
                cur = win::open_dir_at(&cur, name, win::SHARE_ALL).map_err(|e| describe(&at, e))?;
            } else {
                return match win::open_dir_at(&cur, name, win::SHARE_ALL) {
                    Ok(f) => Ok(Some(Dir(f))),
                    Err(e) if not_found(&e) => Ok(None),
                    Err(e) => Err(describe(&at, e)),
                };
            }
        }
        Err(io::Error::other("root has no name"))
    }
    fn describe(at: &Path, e: io::Error) -> io::Error {
        if e.kind() == io::ErrorKind::Other { io::Error::other(format!("{} refused (reparse point, not a directory, or invalid name): {e}", at.display())) } else { io::Error::new(e.kind(), format!("{}: {e}", at.display())) }
    }
    pub fn same_dir(a: &Dir, b: &Dir) -> io::Result<bool> { Ok(win::id(&a.0)? == win::id(&b.0)?) }
    pub fn open_subdir(dir: &Dir, name: &str) -> io::Result<Option<Dir>> {
        match win::open_dir_at(&dir.0, OsStr::new(name), win::SHARE_ALL) { Ok(f) => Ok(Some(Dir(f))), Err(e) if not_found(&e) => Ok(None), Err(e) => Err(e) }
    }
    fn open_file(dir: &Dir, name: &str, access: u32) -> io::Result<Option<File>> {
        match win::open_entry_at(&dir.0, OsStr::new(name), access, win::SHARE_ALL, Some(false)) { Ok(f) => Ok(Some(f)), Err(e) if not_found(&e) => Ok(None), Err(e) => Err(e) }
    }
    pub fn open_regular(dir: &Dir, name: &str) -> io::Result<Option<File>> { open_file(dir, name, win::READ) }
    /// Opened with FILE_EXECUTE as well: the token's execute right is verified by the open itself.
    pub fn open_executable(dir: &Dir, name: &str) -> io::Result<Option<File>> { open_file(dir, name, win::READ | FILE_EXECUTE) }
    pub fn stamp_file(f: &File) -> io::Result<Stamp> { Stamp::of(f) }
    pub fn stamp_dir(d: &Dir) -> io::Result<Stamp> { Stamp::of(&d.0) }
    /// One entry of either kind opened relative to the held directory (reparse refused); `None`
    /// only on not-found.
    pub fn stamp_entry(dir: &Dir, name: &str) -> io::Result<Option<Stamp>> {
        match win::open_entry_at(&dir.0, OsStr::new(name), win::READ, win::SHARE_ALL, None) { Ok(f) => Ok(Some(Stamp::of(&f)?)), Err(e) if not_found(&e) => Ok(None), Err(e) => Err(e) }
    }
    /// Names enumerated through the held handle (`NtQueryDirectoryFile`), never by path; the
    /// answer is parsed by `dir_names::parse` within `IO_STATUS_BLOCK.Information` bytes.
    pub fn list_names(dir: &Dir) -> io::Result<Vec<Vec<u8>>> {
        const _: () = assert!(std::mem::size_of::<FILE_NAMES_INFORMATION>() >= super::dir_names::HEADER);
        let mut names: Vec<Vec<u8>> = Vec::new();
        let mut buf = vec![0u64; 8192]; // 64 KiB, 8-byte aligned
        let bytes_len = buf.len() * 8;
        let mut restart = true;
        loop {
            let mut block: IO_STATUS_BLOCK = unsafe { std::mem::zeroed() };
            let status = unsafe {
                NtQueryDirectoryFile(dir.0.as_raw_handle(), std::ptr::null_mut(), None, std::ptr::null(), &mut block,
                    buf.as_mut_ptr().cast(), bytes_len as u32, FileNamesInformation, false, std::ptr::null(), restart)
            };
            restart = false;
            if status == STATUS_NO_MORE_FILES { break; }
            if status < 0 { return Err(io::Error::other(format!("NtQueryDirectoryFile NTSTATUS {status:#010x}"))); }
            let bytes: &[u8] = unsafe { std::slice::from_raw_parts(buf.as_ptr().cast::<u8>(), bytes_len) };
            let chunk = super::dir_names::parse(bytes, block.Information).map_err(|e| io::Error::other(format!("NtQueryDirectoryFile answer refused: {e}")))?;
            names.extend(chunk.into_iter().map(String::into_bytes));
        }
        names.sort();
        Ok(names)
    }
    fn one_link(s: &Stamp, what: &str) -> io::Result<()> {
        if s.links != 1 { return Err(io::Error::other(format!("{what}: link count {} is not 1", s.links))); }
        Ok(())
    }
    pub fn check_root(dir: &Dir) -> io::Result<String> { win::private_dir(&dir.0)?; Ok("owner-only protected DACL (user + SYSTEM, inheritable)".into()) }
    pub fn check_dir(dir: &Dir) -> io::Result<String> { win::inherited_only(&dir.0, true)?; Ok("inherited owner-only ACEs only".into()) }
    pub fn check_data(f: &File, s: &Stamp) -> io::Result<String> { win::inherited_only(f, false)?; one_link(&Stamp::of(f)?, "data file")?; let _ = s; Ok("inherited owner-only ACEs only, one link".into()) }
    pub fn check_exec(f: &File, s: &Stamp) -> io::Result<String> { win::inherited_only(f, false)?; one_link(&Stamp::of(f)?, "executable")?; let _ = s; Ok("inherited owner-only ACEs only, one link, opened with FILE_EXECUTE".into()) }
}
#[cfg(windows)]
use platform::StampSize as _;

#[cfg(test)]
mod dir_names_tests {
    use super::dir_names::{parse, HEADER};
    fn entry(next: u32, name: &str, len_override: Option<u32>) -> Vec<u8> {
        let wide: Vec<u8> = name.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let mut out = Vec::new();
        out.extend_from_slice(&next.to_le_bytes());
        out.extend_from_slice(&7u32.to_le_bytes());
        out.extend_from_slice(&len_override.unwrap_or(wide.len() as u32).to_le_bytes());
        out.extend_from_slice(&wide);
        out
    }
    fn chain(names: &[&str]) -> Vec<u8> {
        let mut out = Vec::new();
        for (i, n) in names.iter().enumerate() {
            let mut e = entry(0, n, None);
            while e.len() % 8 != 0 { e.push(0); }
            let total = e.len() as u32;
            if i + 1 < names.len() { e[..4].copy_from_slice(&total.to_le_bytes()); }
            out.extend(e);
        }
        out
    }
    #[test]
    fn normal_chain_skips_dot_entries_and_honours_the_used_prefix() {
        let buf = chain(&[".", "..", "store.json", "versions", "한글"]);
        let mut padded = buf.clone(); padded.extend_from_slice(&[0xAA; 64]);
        assert_eq!(parse(&padded, buf.len()).unwrap(), vec!["store.json", "versions", "한글"]);
        assert_eq!(parse(&chain(&["only"]), HEADER + 8).unwrap(), vec!["only"]);
    }
    #[test]
    fn truncated_answers_are_errors() {
        let buf = chain(&["store.json", "versions"]);
        assert!(parse(&buf, 0).is_err());
        assert!(parse(&buf, buf.len() + 1).unwrap_err().contains("buffer holds"));
        assert!(parse(&buf, HEADER - 1).unwrap_err().contains("header"));
        assert!(parse(&buf, HEADER + 4).unwrap_err().contains("exceeds"));
        let mut cut = buf.clone(); cut.truncate(buf.len() - 6); // past the 4 padding bytes, into the last name
        assert!(parse(&cut, cut.len()).unwrap_err().contains("exceeds"));
    }
    #[test]
    fn bad_lengths_and_offsets_are_errors() {
        let odd = entry(0, "ab", Some(3));
        assert!(parse(&odd, odd.len()).unwrap_err().contains("odd"));
        let huge = entry(0, "ab", Some(u32::MAX - 1)); // even, so the range check (not the parity check) refuses it
        assert!(parse(&huge, huge.len()).unwrap_err().contains("exceeds"));
        let mut back = chain(&["a", "b"]); back[..4].copy_from_slice(&4u32.to_le_bytes());
        assert!(parse(&back, back.len()).unwrap_err().contains("does not advance"));
        let mut overflow = chain(&["a", "b"]); overflow[..4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse(&overflow, overflow.len()).unwrap_err().contains("leaves"));
        let mut past = chain(&["a", "b"]); let len = past.len() as u32; past[..4].copy_from_slice(&len.to_le_bytes());
        assert!(parse(&past, past.len()).unwrap_err().contains("leaves"));
    }
}

#[cfg(all(test, unix))]
#[path = "runtime_observe_tests_unix.rs"]
mod tests_unix;
#[cfg(all(test, windows))]
#[path = "runtime_observe_tests_windows.rs"]
mod tests_windows;
