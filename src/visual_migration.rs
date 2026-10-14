//! Bounded development migration, not a general-purpose backup service.
//! See spec/GIL_Visual_Report_Migration_v0.1.md. Never calls recovery or rollback.
use crate::{ProjectSession, RuleSet, STATE_PATH, internal_fs as safe};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    ffi::CString,
    fs::File,
    io::{self, Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::MetadataExt,
    },
    path::Path,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const TEMP: &str = "state.yaml.visual-migration";
const MAX_BYTES: usize = 256 * 1024 * 1024;
const MAX_ENTRIES: usize = 20_000;
const MAX_DEPTH: usize = 64;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    provenance: Vec<u8>,
    directory: bool,
    mode: u32,
    size: u64,
    sha256: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Tree {
    mode: u32,
    provenance: Vec<u8>,
    entries: BTreeMap<String, Entry>,
    bytes: BTreeMap<String, Vec<u8>>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    root_mode: u32,
    root_provenance: Vec<u8>,
    version: u32,
    backup: String,
    backup_device: u64,
    backup_inode: u64,
    target_format: u32,
    root: String,
    device: u64,
    inode: u64,
    old: String,
    new: String,
    inventory: BTreeMap<String, Entry>,
}

// This gate is intentionally narrower than the historical Unix rehearsal API.
// Unsupported metadata is refused, never silently omitted from a claimed backup.
#[cfg(target_os = "macos")]
fn metadata_gate(f: &File) -> Result<()> {
    use std::ffi::c_void;
    use std::os::darwin::fs::MetadataExt as _;
    unsafe extern "C" {
        fn acl_get_fd_np(fd: libc::c_int, kind: libc::c_int) -> *mut c_void;
        fn acl_get_entry(acl: *mut c_void, id: libc::c_int, entry: *mut *mut c_void)
        -> libc::c_int;
        fn acl_free(object: *mut c_void) -> libc::c_int;
    }
    let m = f.metadata()?;
    if m.uid() != unsafe { libc::geteuid() }
        || m.gid() != unsafe { libc::getegid() }
        || m.st_flags() != 0
        || m.mode() & 0o7000 != 0
        || (m.is_dir() && m.mode() & 0o700 != 0o700)
        || (!m.is_dir() && m.mode() & 0o400 == 0)
    {
        return refuse("unsupported ownership, flags or restoration permissions");
    }
    provenance(f)?;
    let acl = unsafe { acl_get_fd_np(f.as_raw_fd(), 0x100) }; // ACL_TYPE_EXTENDED
    if acl.is_null() {
        let error = io::Error::last_os_error();
        // macOS reports ENOENT for a descriptor with no extended ACL.
        return if error.raw_os_error() == Some(libc::ENOENT) {
            Ok(())
        } else {
            Err(error.into())
        };
    }
    let mut entry = std::ptr::null_mut();
    let found = unsafe { acl_get_entry(acl, 0, &mut entry) }; // ACL_FIRST_ENTRY
    let error = io::Error::last_os_error().raw_os_error();
    unsafe {
        acl_free(acl);
    }
    // macOS returns -1 with EINVAL when an empty ACL has no first entry.
    if found == 0 {
        return refuse("ACL preservation is unsupported");
    }
    if found != -1 || error != Some(libc::EINVAL) {
        return refuse("ACL inspection failed");
    }
    Ok(())
}
#[cfg(not(target_os = "macos"))]
fn metadata_gate(_: &File) -> Result<()> {
    Ok(())
}

fn provenance(f: &File) -> Result<Vec<u8>> {
    #[cfg(target_os = "macos")]
    {
        let mut names = [0u8; 1024];
        let size =
            unsafe { libc::flistxattr(f.as_raw_fd(), names.as_mut_ptr().cast(), names.len(), 0) };
        if size < 0 {
            return refuse("extended attributes unavailable");
        }
        if size == 0 {
            return Ok(Vec::new());
        }
        if &names[..size as usize] != b"com.apple.provenance\0" {
            return refuse("unsupported extended attributes; none omitted");
        }
        let mut bytes = vec![0; 4096];
        let size = unsafe {
            libc::fgetxattr(
                f.as_raw_fd(),
                c"com.apple.provenance".as_ptr(),
                bytes.as_mut_ptr().cast(),
                bytes.len(),
                0,
                0,
            )
        };
        if size < 0 {
            return refuse("provenance cannot be read");
        }
        bytes.truncate(size as usize);
        // Distinguish absent from a present empty attribute.
        bytes.insert(0, 1);
        Ok(bytes)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = f;
        Ok(Vec::new())
    }
}

/// Preflight for the opt-in Agent tools. No recovery, cleanup or implicit root search.
pub fn precheck(root: &Path) -> Result<serde_json::Value> {
    if !cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        return refuse("Agent migration is not accepted on this platform");
    }
    let held = exact_root(root)?;
    local(&held)?;
    let s = audited(root)?;
    s.project().no_pending_revisit()?;
    let before = tree(&held, false)?;
    no_temporary(&before)?;
    candidate(&s, before.bytes.get(STATE_PATH).ok_or("missing state")?)?;
    let bytes: u64 = before.entries.values().map(|e| e.size).sum();
    let c = s.project().current_cycle();
    Ok(
        serde_json::json!({"ok":true,"storage_format":4,"target_format":5,
        "cycle":c.id().to_ref().to_string(),"cycle_open":!c.is_closed(),
        "open_step":c.step_now_open().map(|n|c.step_ref(n).to_string()),
        "entries":before.entries.len(),"bytes":bytes,"backup_bytes_estimate":bytes.saturating_mul(3),
        "space_reserved":false,"recovery_required":false,"development_only":true}),
    )
}
fn local(f: &File) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let mut stat = std::mem::MaybeUninit::<libc::statfs>::uninit();
        if unsafe { libc::fstatfs(f.as_raw_fd(), stat.as_mut_ptr()) } < 0 {
            return Err(io::Error::last_os_error().into());
        }
        if unsafe { stat.assume_init() }.f_flags & libc::MNT_LOCAL as u32 == 0 {
            return refuse("only local filesystems accepted");
        }
    }
    let _ = f;
    Ok(())
}

fn refuse<T>(why: &str) -> Result<T> {
    Err(io::Error::other(why).into())
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn name(s: &str) -> Result<CString> {
    Ok(CString::new(s)?)
}
fn rules() -> Result<RuleSet> {
    Ok(RuleSet::builtin()?)
}
fn exact_root(root: &Path) -> Result<File> {
    if !root.is_absolute() || root.canonicalize()? != root || root.parent().is_none() {
        return refuse("use an explicit canonical Project directory, not a root or alias");
    }
    if std::env::var_os("HOME")
        .and_then(|home| Path::new(&home).canonicalize().ok())
        .is_some_and(|home| home == root)
    {
        return refuse("the entire home directory is not a migration target");
    }
    let dir = safe::project_directory(root)?;
    // A read-only session must not create a missing lock during this procedure.
    let gil = safe::child_dir(&dir, c".gil")?;
    let lock = safe::regular_at(&gil, c"project.lock")?;
    if lock.metadata()?.nlink() != 1 {
        return refuse("unsafe Project lock");
    }
    Ok(dir)
}
fn bounded_read(mut f: File, remaining: usize) -> Result<Vec<u8>> {
    if f.metadata()?.nlink() != 1 {
        return refuse("hardlinked file refused; no entries silently skipped");
    }
    if f.metadata()?.len() > remaining as u64 {
        return refuse("development migration byte limit exceeded");
    }
    let mut bytes = Vec::new();
    (&mut f)
        .take(remaining as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > remaining {
        return refuse("development migration byte limit exceeded");
    }
    Ok(bytes)
}
fn tree(dir: &File, skip_temp: bool) -> Result<Tree> {
    fn walk(
        dir: &File,
        prefix: &str,
        depth: usize,
        skip: bool,
        total: &mut usize,
        out: &mut Tree,
    ) -> Result<()> {
        metadata_gate(dir)?;
        if depth > MAX_DEPTH {
            return refuse("development migration depth limit exceeded");
        }
        for n in safe::names(dir)? {
            let leaf = n.to_str()?;
            let rel = if prefix.is_empty() {
                leaf.to_owned()
            } else {
                format!("{prefix}/{leaf}")
            };
            if skip && rel == format!(".gil/{TEMP}") {
                continue;
            }
            if out.entries.len() >= MAX_ENTRIES {
                return refuse("development migration entry limit exceeded");
            }
            if let Ok(child) = safe::child_dir(dir, &n) {
                let m = child.metadata()?;
                out.entries.insert(
                    rel.clone(),
                    Entry {
                        provenance: provenance(&child)?,
                        directory: true,
                        mode: m.mode() & 0o7777,
                        size: 0,
                        sha256: String::new(),
                    },
                );
                walk(&child, &rel, depth + 1, skip, total, out)?;
                if !safe::same(&child, &safe::child_dir(dir, &n)?)? {
                    return refuse("directory changed while reading");
                }
            } else {
                let f = safe::regular_at(dir, &n)?;
                metadata_gate(&f)?;
                let held = f.try_clone()?;
                let m = f.metadata()?;
                let bytes = bounded_read(f, MAX_BYTES - *total)?;
                *total += bytes.len();
                if !safe::same(&held, &safe::regular_at(dir, &n)?)? {
                    return refuse("file changed while reading");
                }
                out.entries.insert(
                    rel.clone(),
                    Entry {
                        provenance: provenance(&held)?,
                        directory: false,
                        mode: m.mode() & 0o7777,
                        size: bytes.len() as u64,
                        sha256: digest(&bytes),
                    },
                );
                out.bytes.insert(rel, bytes);
            }
        }
        Ok(())
    }
    let mut out = Tree {
        mode: dir.metadata()?.mode() & 0o7777,
        provenance: provenance(dir)?,
        entries: BTreeMap::new(),
        bytes: BTreeMap::new(),
    };
    walk(dir, "", 0, skip_temp, &mut 0, &mut out)?;
    Ok(out)
}
fn mkdir(parent: &File, n: &CString) -> Result<File> {
    if unsafe { libc::mkdirat(parent.as_raw_fd(), n.as_ptr(), 0o700) } < 0 {
        return Err(io::Error::last_os_error().into());
    }
    let dir = safe::child_dir(parent, n)?;
    parent.sync_all()?;
    Ok(dir)
}
fn create(parent: &File, n: &CString) -> Result<File> {
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            n.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
    };
    if fd < 0 {
        Err(io::Error::last_os_error().into())
    } else {
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}
fn write_new(
    parent: &File,
    n: &str,
    bytes: &[u8],
    hook: &mut impl FnMut(&str) -> io::Result<()>,
) -> Result<File> {
    let mut f = create(parent, &name(n)?)?;
    let split = bytes.len() / 2;
    f.write_all(&bytes[..split])?;
    hook("partial_write")?;
    f.write_all(&bytes[split..])?;
    hook("file_sync")?;
    f.sync_all()?;
    Ok(f)
}
fn copy_tree(
    source: &Tree,
    dest: &File,
    hook: &mut impl FnMut(&str) -> io::Result<()>,
) -> Result<()> {
    let mut dirs = BTreeMap::new();
    dirs.insert(String::new(), dest.try_clone()?);
    for (rel, entry) in &source.entries {
        let (parent, leaf) = rel.rsplit_once('/').unwrap_or(("", rel));
        let parent = dirs.get(parent).ok_or("invalid tree parent")?;
        if entry.directory {
            let dir = mkdir(parent, &name(leaf)?)?;
            dirs.insert(rel.clone(), dir);
        } else {
            write_new(parent, leaf, &source.bytes[rel], hook)?;
        }
    }
    for dir in dirs.values().rev() {
        hook("directory_sync")?;
        dir.sync_all()?;
    }
    Ok(())
}
fn same_copy(source: &Tree, copied: &Tree) -> bool {
    copied.mode == 0o700
        && source.provenance == copied.provenance
        && source.bytes == copied.bytes
        && source.entries.len() == copied.entries.len()
        && source.entries.iter().all(|(n, e)| {
            copied.entries.get(n).is_some_and(|c| {
                c.directory == e.directory
                    && c.provenance == e.provenance
                    && c.size == e.size
                    && c.sha256 == e.sha256
                    && c.mode == if c.directory { 0o700 } else { 0o600 }
            })
        })
}
fn restore_modes(source: &Tree, dest: &File) -> Result<()> {
    // Only held no-follow descriptors beneath the newly created private copy.
    fn walk(source: &Tree, dest: &File, prefix: &str) -> Result<()> {
        for n in safe::names(dest)? {
            let leaf = n.to_str()?;
            let rel = if prefix.is_empty() {
                leaf.into()
            } else {
                format!("{prefix}/{leaf}")
            };
            let f = if source.entries[&rel].directory {
                let f = safe::child_dir(dest, &n)?;
                walk(source, &f, &rel)?;
                f
            } else {
                safe::regular_at(dest, &n)?
            };
            if unsafe { libc::fchmod(f.as_raw_fd(), source.entries[&rel].mode as libc::mode_t) } < 0
            {
                return Err(io::Error::last_os_error().into());
            }
            f.sync_all()?;
        }
        Ok(())
    }
    walk(source, dest, "")?;
    if unsafe { libc::fchmod(dest.as_raw_fd(), source.mode as libc::mode_t) } < 0 {
        return Err(io::Error::last_os_error().into());
    }
    dest.sync_all()?;
    Ok(())
}
fn sync_tree(dir: &File) -> Result<()> {
    for n in safe::names(dir)? {
        if let Ok(child) = safe::child_dir(dir, &n) {
            sync_tree(&child)?;
        } else {
            let f = safe::regular_at(dir, &n)?;
            if f.metadata()?.nlink() != 1 {
                return refuse("linked backup");
            }
            f.sync_all()?;
        }
    }
    dir.sync_all()?;
    Ok(())
}
fn audited(root: &Path) -> Result<crate::ReadOnlySession> {
    let s = ProjectSession::open_read_only(rules()?, root.join(STATE_PATH))?;
    s.verify_snapshot_objects()?;
    Ok(s)
}
fn candidate(s: &crate::ReadOnlySession, old: &[u8]) -> Result<Vec<u8>> {
    s.project().no_pending_revisit()?;
    if s.project()
        .current_cycle()
        .steps()
        .pending_revisit()
        .is_some()
    {
        return refuse("pending Step revisit is not accepted for migration");
    }
    // Older readable formats are not migration inputs.
    let mut before: serde_norway::Value = serde_norway::from_slice(old)?;
    if before.get("format").and_then(|v| v.as_u64()) != Some(4) {
        return refuse("only format 4 can be prepared/applied");
    }
    let bytes = crate::store::encode(&s.project().prepare_visual_upgrade()?)?;
    let mut after: serde_norway::Value = serde_norway::from_slice(&bytes)?;
    for value in [&mut before, &mut after] {
        let map = value.as_mapping_mut().ok_or("invalid state mapping")?;
        map.remove(serde_norway::Value::String("format".into()));
        map.remove(serde_norway::Value::String("visuals".into()));
    }
    if before != after {
        return refuse("migration changed existing state semantics");
    }
    Ok(bytes)
}
fn identity(root: &Path, held: &File, gil: &File, lock: &File) -> Result<()> {
    let now = exact_root(root)?;
    let now_gil = safe::child_dir(&now, c".gil")?;
    if !safe::same(held, &now)?
        || !safe::same(gil, &now_gil)?
        || !safe::same(lock, &safe::regular_at(&now_gil, c"project.lock")?)?
    {
        return refuse("Project or lock identity changed; no publication");
    }
    Ok(())
}
fn no_temporary(t: &Tree) -> Result<()> {
    if t.entries.contains_key(&format!(".gil/{TEMP}"))
        || t.entries.contains_key(".gil/state.yaml.writing")
    {
        return refuse("existing temporary state retained; inspect before migration");
    }
    Ok(())
}

/// Prepare a fresh private backup, converted clone and independently restored copy.
/// Returns the plan SHA-256 to bind a later, explicit `apply` invocation.
pub fn prepare(root: &Path, backup: &Path) -> Result<String> {
    prepare_with(root, backup, &mut |_| Ok(()))
}
fn prepare_with(
    root: &Path,
    backup: &Path,
    hook: &mut impl FnMut(&str) -> io::Result<()>,
) -> Result<String> {
    let held = exact_root(root)?;
    local(&held)?;
    let gil = safe::child_dir(&held, c".gil")?;
    let lock = safe::regular_at(&gil, c"project.lock")?;
    let s = audited(root)?;
    let original = tree(&held, false)?;
    no_temporary(&original)?;
    let old = original.bytes.get(STATE_PATH).ok_or("missing state")?;
    let bytes = candidate(&s, old)?;
    if !backup.is_absolute() {
        return refuse("backup must be absolute");
    }
    let parent = backup
        .parent()
        .ok_or("backup parent missing")?
        .canonicalize()?;
    let leaf = backup
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("backup name missing")?;
    let canonical = parent.join(leaf);
    if canonical != backup || backup.starts_with(root) || root.starts_with(backup) {
        return refuse("backup must be separate and canonical");
    }
    let parent_dir = safe::project_directory(&parent)?;
    local(&parent_dir)?;
    let run = mkdir(&parent_dir, &name(leaf)?)?;
    for label in ["original", "candidate", "restored"] {
        // Restoration is read FROM the completed backup, not from source memory.
        let mut expected = if label == "restored" {
            tree(&safe::child_dir(&run, c"original")?, false)?
        } else {
            original.clone()
        };
        if label == "candidate" {
            expected.bytes.insert(STATE_PATH.into(), bytes.clone());
            let e = expected
                .entries
                .get_mut(STATE_PATH)
                .ok_or("missing state")?;
            e.size = bytes.len() as u64;
            e.sha256 = digest(&bytes);
        }
        let dest = mkdir(&run, &name(label)?)?;
        copy_tree(&expected, &dest, hook)?;
        if !same_copy(&expected, &tree(&dest, false)?) {
            return refuse("backup/clone byte verification failed");
        }
        if label == "restored" {
            restore_modes(&original, &dest)?;
            if tree(&dest, false)? != original {
                return refuse("restored permissions differ");
            }
        }
        let _verified = audited(&backup.join(label))?;
    }
    s.verify_snapshot_objects()?;
    identity(root, &held, &gil, &lock)?;
    if tree(&held, false)? != original {
        return refuse("source changed during preparation");
    }
    let m = held.metadata()?;
    let plan = Plan {
        root_mode: original.mode,
        root_provenance: original.provenance,
        version: 2,
        backup: backup.to_str().ok_or("non UTF-8 backup")?.into(),
        backup_device: run.metadata()?.dev(),
        backup_inode: run.metadata()?.ino(),
        target_format: 5,
        root: root.to_str().ok_or("non UTF-8 root")?.into(),
        device: m.dev(),
        inode: m.ino(),
        old: digest(old),
        new: digest(&bytes),
        inventory: original.entries,
    };
    let encoded = serde_json::to_vec_pretty(&plan)?;
    hook("before_plan")?;
    write_new(&run, "plan.json", &encoded, hook)?;
    run.sync_all()?;
    parent_dir.sync_all()?;
    Ok(digest(&encoded))
}
fn plan(backup: &File, expected: &str) -> Result<Plan> {
    metadata_gate(backup)?;
    let bytes = bounded_read(safe::regular_at(backup, c"plan.json")?, 8 * 1024 * 1024)?;
    if digest(&bytes) != expected {
        return refuse("plan digest differs from explicit approval");
    }
    let p: Plan = serde_json::from_slice(&bytes)?;
    if p.version != 2 || p.target_format != 5 {
        return refuse("unsupported migration plan");
    }
    let m = backup.metadata()?;
    if m.dev() != p.backup_device || m.ino() != p.backup_inode || m.mode() & 0o777 != 0o700 {
        return refuse("backup identity or privacy differs");
    }
    Ok(p)
}

/// Explicit publication. On any error preserve backup and temporary files; use status,
/// never retry or restore automatically. `expected` is the digest returned by prepare.
pub fn apply(root: &Path, backup: &Path, expected: &str) -> Result<()> {
    apply_with(root, backup, expected, &mut |_| Ok(()))
}
fn apply_with(
    root: &Path,
    backup: &Path,
    expected: &str,
    hook: &mut impl FnMut(&str) -> io::Result<()>,
) -> Result<()> {
    let held = exact_root(root)?;
    let gil = safe::child_dir(&held, c".gil")?;
    let lock = safe::regular_at(&gil, c"project.lock")?;
    let s = audited(root)?;
    let run = safe::project_directory(backup)?;
    let p = plan(&run, expected)?;
    if backup.to_str() != Some(p.backup.as_str()) {
        return refuse("backup path differs");
    }
    let m = held.metadata()?;
    if p.root != root.to_str().ok_or("non UTF-8 root")? || p.device != m.dev() || p.inode != m.ino()
    {
        return refuse("plan belongs to another Project identity");
    }
    let original = tree(&held, false)?;
    no_temporary(&original)?;
    if original.entries != p.inventory {
        return refuse("source changed since preparation; preserve new records");
    }
    if original.provenance != p.root_provenance || original.mode != p.root_mode {
        return refuse("root metadata changed");
    }
    let old = original.bytes.get(STATE_PATH).ok_or("missing state")?;
    let bytes = candidate(&s, old)?;
    if digest(old) != p.old || digest(&bytes) != p.new {
        return refuse("state digest mismatch");
    }
    for label in ["original", "restored", "candidate"] {
        let copy = safe::child_dir(&run, &name(label)?)?;
        let mut wanted = original.clone();
        if label == "candidate" {
            wanted.bytes.insert(STATE_PATH.into(), bytes.clone());
            let e = wanted.entries.get_mut(STATE_PATH).ok_or("missing state")?;
            e.size = bytes.len() as u64;
            e.sha256 = digest(&bytes);
        }
        let copied = tree(&copy, false)?;
        if !(if label == "restored" {
            wanted == copied
        } else {
            same_copy(&wanted, &copied)
        }) {
            return refuse("backup/rehearsal copy changed; no publication");
        }
        let _verified = audited(&backup.join(label))?;
        sync_tree(&copy)?;
    }
    run.sync_all()?;
    safe::project_directory(backup.parent().ok_or("backup parent missing")?)?.sync_all()?;
    hook("before_temp")?;
    let temp = write_new(&gil, TEMP, &bytes, hook)?;
    if unsafe {
        libc::fchmod(
            temp.as_raw_fd(),
            original.entries[STATE_PATH].mode as libc::mode_t,
        )
    } < 0
    {
        return Err(io::Error::last_os_error().into());
    }
    temp.sync_all()?;
    if provenance(&temp)? != original.entries[STATE_PATH].provenance {
        return refuse("replacement provenance differs; preserve temporary state");
    }
    let read = safe::regular_at(&gil, &name(TEMP)?)?;
    if !safe::same(&temp, &read)? || bounded_read(read, MAX_BYTES)? != bytes {
        return refuse("temporary state verification failed");
    }
    hook("before_rename")?;
    if !safe::same(&run, &safe::project_directory(backup)?)? {
        return refuse("backup moved before publication");
    }
    plan(&run, expected)?;
    identity(root, &held, &gil, &lock)?;
    s.verify_snapshot_objects()?;
    if tree(&held, true)? != original {
        return refuse("source changed before publication");
    }
    let final_temp = safe::regular_at(&gil, &name(TEMP)?)?;
    if !safe::same(&temp, &final_temp)? || bounded_read(final_temp, MAX_BYTES)? != bytes {
        return refuse("temporary state replaced or modified");
    }
    if unsafe {
        libc::renameat(
            gil.as_raw_fd(),
            name(TEMP)?.as_ptr(),
            gil.as_raw_fd(),
            c"state.yaml".as_ptr(),
        )
    } < 0
    {
        return Err(io::Error::last_os_error().into());
    }
    hook("after_rename")?;
    gil.sync_all()?;
    hook("after_directory_sync")?;
    identity(root, &held, &gil, &lock)?;
    write_new(
        &run,
        "applied.json",
        &serde_json::to_vec(
            &serde_json::json!({"plan":expected,"state":p.new,"directory_synced":true}),
        )?,
        hook,
    )?;
    run.sync_all()?;
    Ok(())
}

/// Read-only state classification, not a promise of physical power-loss durability.
/// Never changes state, cleans temporary files, repairs or rolls back.
pub fn status(root: &Path, backup: &Path, expected: &str) -> Result<&'static str> {
    Ok(inspect(root, backup, expected)?.0)
}

/// State and complete preservation are read under the same held project lock.
pub fn inspect(root: &Path, backup: &Path, expected: &str) -> Result<(&'static str, bool)> {
    let held = exact_root(root)?;
    let _s = audited(root)?;
    let p = plan(&safe::project_directory(backup)?, expected)?;
    if backup.to_str() != Some(p.backup.as_str()) {
        return refuse("backup path differs");
    }
    let m = held.metadata()?;
    if p.root != root.to_str().ok_or("non UTF-8 root")? || m.dev() != p.device || m.ino() != p.inode
    {
        return refuse("Project identity differs from plan");
    }
    let gil = safe::child_dir(&held, c".gil")?;
    let lock = safe::regular_at(&gil, c"project.lock")?;
    let actual = tree(&held, false)?;
    let state = actual.entries.get(STATE_PATH).ok_or("missing state")?;
    let hash = &state.sha256;
    let mut wanted = p.inventory;
    let e = wanted.get_mut(STATE_PATH).ok_or("plan missing state")?;
    e.size = state.size;
    e.sha256 = hash.clone();
    let preserved = (hash == &p.old || hash == &p.new)
        && actual.entries == wanted
        && actual.provenance == p.root_provenance
        && actual.mode == p.root_mode;
    identity(root, &held, &gil, &lock)?;
    Ok((
        if hash == &p.old {
            "not_applied"
        } else if hash == &p.new {
            "applied_state_present"
        } else {
            "changed_or_unknown_preserve_all"
        },
        preserved,
    ))
}

/// Full tree preservation, separately from state classification. Later work may differ legitimately.
pub fn preservation(root: &Path, backup: &Path, expected: &str) -> Result<bool> {
    Ok(inspect(root, backup, expected)?.1)
}

#[cfg(test)]
#[path = "../tests/common/mod.rs"]
mod common;
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::{
            fs::{DirBuilderExt, symlink},
            process::ExitStatusExt,
        },
        path::PathBuf,
        process::Command,
    };
    fn fixture(closed: bool) -> (PathBuf, PathBuf) {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let base = loop {
            let p = std::env::temp_dir().join(format!(
                "gil-inplace-synthetic-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            match fs::DirBuilder::new().mode(0o700).create(&p) {
                Ok(()) => break p.canonicalize().unwrap(),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("fixture: {e}"),
            }
        };
        let root = base.join("source");
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        fs::create_dir(root.join("empty directory")).unwrap();
        fs::write(root.join("work.txt"), "원래 세계 🧭\n").unwrap();
        let mut s = ProjectSession::start(rules().unwrap(), root.join(STATE_PATH)).unwrap();
        let mut p = common::bootstrap_from(s.project().clone());
        common::up_to_verify(&mut p);
        if closed {
            let report = common::full_report(
                p.current_cycle().rules(),
                crate::CycleKind::Experiment,
                crate::NodeKind::Verify,
            )
            .with("execution", "  공백과 한글 🧭\n두 번째 줄\n")
            .with("result", "보존할 결과\n");
            let world = p.world_manifest(common::snapshot(1)).unwrap().clone();
            p.close_verify_step(report, world).unwrap();
        }
        *s.project_mut() = p;
        s.commit().unwrap();
        drop(s);
        fs::write(root.join(".gil/unreferenced-data"), "미참조도 보존").unwrap();
        (root, base.join("backup"))
    }
    fn snap(p: &Path) -> Tree {
        tree(&safe::project_directory(p).unwrap(), false).unwrap()
    }
    fn old_unchanged(root: &Path, before: &Tree) {
        assert_eq!(
            tree(&safe::project_directory(root).unwrap(), true).unwrap(),
            *before
        );
    }
    #[test]
    fn apply_preserves_reports_objects_working_files_and_backup_for_open_and_closed_verify() {
        for closed in [false, true] {
            let (root, backup) = fixture(closed);
            let before = snap(&root);
            let sha = prepare(&root, &backup).unwrap();
            assert_eq!(snap(&root), before);
            let backup_before = snap(&backup.join("original"));
            assert!(same_copy(&before, &backup_before));
            assert_eq!(status(&root, &backup, &sha).unwrap(), "not_applied");
            apply(&root, &backup, &sha).unwrap();
            assert_eq!(
                status(&root, &backup, &sha).unwrap(),
                "applied_state_present"
            );
            let after = snap(&root);
            for (n, bytes) in &before.bytes {
                if n != STATE_PATH {
                    assert_eq!(&after.bytes[n], bytes);
                }
            }
            assert_eq!(snap(&backup.join("original")), backup_before);
            assert!(audited(&root).unwrap().project().visual_state().is_some());
            assert!(apply(&root, &backup, &sha).is_err());
            assert_eq!(snap(&root), after);
            // Subsequent valid state bytes must never trigger a downgrade or rewrite.
            let mut later = fs::read(root.join(STATE_PATH)).unwrap();
            later.extend_from_slice(b"\n# later revision evidence\n");
            fs::write(root.join(STATE_PATH), later).unwrap();
            let later = snap(&root);
            assert_eq!(
                status(&root, &backup, &sha).unwrap(),
                "changed_or_unknown_preserve_all"
            );
            assert!(apply(&root, &backup, &sha).is_err());
            assert_eq!(snap(&root), later);
        }
    }
    #[test]
    fn copied_or_moved_backup_identity_cannot_authorize_apply() {
        let (root, backup) = fixture(false);
        let before = snap(&root);
        let sha = prepare(&root, &backup).unwrap();
        let moved = backup.with_file_name("moved-backup");
        fs::rename(&backup, &moved).unwrap();
        assert!(apply(&root, &moved, &sha).is_err());
        fs::create_dir(&backup).unwrap();
        let copy = snap(&moved);
        copy_tree(
            &copy,
            &safe::project_directory(&backup).unwrap(),
            &mut |_| Ok(()),
        )
        .unwrap();
        assert!(apply(&root, &backup, &sha).is_err());
        assert_eq!(snap(&root), before);
    }
    #[test]
    fn oversized_git_file_is_not_silently_excluded_from_backup() {
        let (root, backup) = fixture(false);
        let state = fs::read(root.join(STATE_PATH)).unwrap();
        fs::create_dir(root.join(".git")).unwrap();
        let large = File::create(root.join(".git/sparse-pack")).unwrap();
        large.set_len(MAX_BYTES as u64 + 1).unwrap();
        assert!(prepare(&root, &backup).is_err());
        assert!(!backup.exists());
        assert_eq!(large.metadata().unwrap().len(), MAX_BYTES as u64 + 1);
        assert_eq!(fs::read(root.join(STATE_PATH)).unwrap(), state);
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn unsupported_xattrs_and_acl_are_retained_and_refused_before_backup() {
        for acl in [false, true] {
            let (root, backup) = fixture(false);
            let state = fs::read(root.join(STATE_PATH)).unwrap();
            let file = root.join("work.txt");
            if acl {
                let done = Command::new("/bin/chmod")
                    .args(["+a", "everyone allow read"])
                    .arg(&file)
                    .output()
                    .unwrap();
                assert!(
                    done.status.success(),
                    "{}",
                    String::from_utf8_lossy(&done.stderr)
                );
            } else {
                let f = File::open(&file).unwrap();
                assert_eq!(
                    unsafe {
                        libc::fsetxattr(
                            f.as_raw_fd(),
                            c"org.gil.synthetic".as_ptr(),
                            b"keep".as_ptr().cast(),
                            4,
                            0,
                            0,
                        )
                    },
                    0
                );
            }
            assert!(precheck(&root).is_err());
            assert!(prepare(&root, &backup).is_err());
            assert!(!backup.exists());
            assert_eq!(fs::read(root.join(STATE_PATH)).unwrap(), state);
            // Do not strip metadata to make the migration pass.
            assert!(metadata_gate(&File::open(&file).unwrap()).is_err());
        }
    }
    #[test]
    fn preparation_enospc_never_modifies_source_or_publishes_a_plan() {
        for stage in [
            "partial_write",
            "file_sync",
            "directory_sync",
            "before_plan",
        ] {
            let (root, backup) = fixture(false);
            let before = snap(&root);
            let result = prepare_with(&root, &backup, &mut |at| {
                if at == stage {
                    Err(io::Error::from_raw_os_error(libc::ENOSPC))
                } else {
                    Ok(())
                }
            });
            assert!(result.is_err(), "{stage}");
            assert_eq!(snap(&root), before);
            assert!(!backup.join("plan.json").exists());
            // Existing partial backups are not overwritten on a fresh attempt.
            let partial = snap(&backup);
            assert!(prepare(&root, &backup).is_err());
            assert_eq!(snap(&backup), partial);
        }
    }
    #[test]
    fn new_report_and_image_after_migration_cannot_be_replaced_by_old_plan() {
        use crate::visual_report::{
            VisualDeclaration, VisualUse, image::VisualAttachment, registry::SourceKind,
        };
        let (root, backup) = fixture(false);
        let sha = prepare(&root, &backup).unwrap();
        let original = snap(&backup.join("original"));
        apply(&root, &backup, &sha).unwrap();
        let mut bytes = Vec::new();
        png::Encoder::new(&mut bytes, 1, 1)
            .write_header()
            .unwrap()
            .write_image_data(&[127])
            .unwrap();
        fs::write(root.join("new-screen.png"), &bytes).unwrap();
        let mut s = ProjectSession::open(rules().unwrap(), root.join(STATE_PATH)).unwrap();
        let report = common::full_report(
            s.project().current_cycle().rules(),
            crate::CycleKind::Experiment,
            crate::NodeKind::Verify,
        )
        .with("execution", "이관 이후 새 보고서 — 절대로 버리지 않는다");
        s.close_verify_with_images_and_commit(
            report,
            vec![VisualAttachment {
                path: "new-screen.png".into(),
                source_kind: SourceKind::Generated,
                source_description: "합성 회색 픽셀".into(),
            }],
            |ids| VisualDeclaration::Attached {
                uses: ids
                    .iter()
                    .map(|id| VisualUse {
                        asset: *id,
                        caption: "새 이미지".into(),
                        alt: "회색 픽셀".into(),
                        purpose: "이관 후 기록 보존 검수".into(),
                    })
                    .collect(),
            },
        )
        .unwrap();
        drop(s);
        let after = snap(&root);
        assert_eq!(
            status(&root, &backup, &sha).unwrap(),
            "changed_or_unknown_preserve_all"
        );
        assert!(apply(&root, &backup, &sha).is_err());
        assert_eq!(snap(&root), after);
        assert_eq!(snap(&backup.join("original")), original);
        assert!(
            after.bytes[STATE_PATH]
                .windows("이관 이후 새 보고서".len())
                .any(|w| w == "이관 이후 새 보고서".as_bytes())
        );
        audited(&root).unwrap();
    }
    #[test]
    fn apply_enospc_before_and_after_publication_has_unambiguous_state_and_intact_backup() {
        for stage in [
            "before_temp",
            "partial_write",
            "file_sync",
            "before_rename",
            "after_rename",
            "after_directory_sync",
            "receipt_partial",
        ] {
            let (root, backup) = fixture(true);
            let before = snap(&root);
            let sha = prepare(&root, &backup).unwrap();
            let saved = snap(&backup.join("original"));
            let mut partials = 0;
            let result = apply_with(&root, &backup, &sha, &mut |at| {
                if at == "partial_write" {
                    partials += 1;
                }
                if at == stage
                    || (stage == "receipt_partial" && at == "partial_write" && partials == 2)
                {
                    Err(io::Error::from_raw_os_error(libc::ENOSPC))
                } else {
                    Ok(())
                }
            });
            assert!(result.is_err(), "{stage}");
            let published = matches!(
                stage,
                "after_rename" | "after_directory_sync" | "receipt_partial"
            );
            assert_eq!(
                status(&root, &backup, &sha).unwrap(),
                if published {
                    "applied_state_present"
                } else {
                    "not_applied"
                },
                "{stage}"
            );
            if !published {
                old_unchanged(&root, &before);
            }
            assert_eq!(snap(&backup.join("original")), saved);
            let failed = snap(&root);
            assert!(apply(&root, &backup, &sha).is_err() || stage == "before_temp");
            if stage != "before_temp" {
                assert_eq!(snap(&root), failed);
            }
        }
    }
    #[test]
    fn changed_source_bad_approval_or_damaged_backup_refuses_without_rewriting() {
        for what in ["source", "backup", "approval", "candidate", "restored"] {
            let (root, backup) = fixture(false);
            let sha = prepare(&root, &backup).unwrap();
            match what {
                "source" => fs::write(root.join("work.txt"), "new user work").unwrap(),
                "backup" => {
                    fs::remove_file(backup.join("original/.gil/unreferenced-data")).unwrap();
                }
                "candidate" => fs::write(backup.join("candidate/work.txt"), "tampered").unwrap(),
                "restored" => fs::write(backup.join("restored/work.txt"), "tampered").unwrap(),
                _ => (),
            }
            let before = snap(&root);
            assert!(
                apply(
                    &root,
                    &backup,
                    if what == "approval" { "wrong" } else { &sha }
                )
                .is_err(),
                "{what}"
            );
            assert_eq!(snap(&root), before);
        }
    }
    #[test]
    fn busy_unsafe_paths_old_temps_and_backup_inside_project_are_refused() {
        let (root, backup) = fixture(false);
        let before = snap(&root);
        let held = audited(&root).unwrap();
        assert!(prepare(&root, &backup).is_err());
        drop(held);
        assert_eq!(snap(&root), before);
        assert!(prepare(&root, &root.join("backup")).is_err());
        assert_eq!(snap(&root), before);
        for temp in [TEMP, "state.yaml.writing"] {
            let (root, backup) = fixture(false);
            fs::write(root.join(".gil").join(temp), "do not remove").unwrap();
            let before = snap(&root);
            assert!(prepare(&root, &backup).is_err());
            assert_eq!(snap(&root), before);
        }
        let (root, backup) = fixture(false);
        let external = root.parent().unwrap().join("external");
        fs::write(&external, "private").unwrap();
        symlink(&external, root.join("link")).unwrap();
        assert!(prepare(&root, &backup).is_err());
        assert_eq!(fs::read(&external).unwrap(), b"private");
        assert!(!backup.exists());
        let (root, backup) = fixture(false);
        fs::hard_link(root.join("work.txt"), root.join("hardlink")).unwrap();
        assert!(prepare(&root, &backup).is_err());
        assert!(!backup.exists());
    }
    #[test]
    fn changes_at_last_gate_never_publish_candidate_or_remove_new_evidence() {
        for change in ["source", "temp", "lock"] {
            let (root, backup) = fixture(false);
            let sha = prepare(&root, &backup).unwrap();
            let old = fs::read(root.join(STATE_PATH)).unwrap();
            assert!(
                apply_with(&root, &backup, &sha, &mut |stage| {
                    if stage == "before_rename" {
                        match change {
                            "source" => fs::write(root.join("work.txt"), "concurrent edit")?,
                            "temp" => fs::write(root.join(".gil").join(TEMP), "modified in place")?,
                            "lock" => {
                                fs::rename(
                                    root.join(".gil/project.lock"),
                                    root.join(".gil/project.lock.retained"),
                                )?;
                                fs::write(root.join(".gil/project.lock"), "new lock")?;
                            }
                            _ => unreachable!(),
                        }
                    }
                    Ok(())
                })
                .is_err()
            );
            assert_eq!(fs::read(root.join(STATE_PATH)).unwrap(), old);
            assert_eq!(
                fs::read(backup.join("original").join(STATE_PATH)).unwrap(),
                old
            );
        }
    }
    #[test]
    #[ignore = "child process used only by interrupted_process_preserves_old_or_complete_new_state"]
    fn crash_child() {
        let root =
            PathBuf::from(std::env::var("GIL_SYNTHETIC_MIGRATION_ROOT").expect("test child only"));
        assert!(
            root.parent()
                .unwrap()
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("gil-inplace-synthetic-")
        );
        let backup = root.parent().unwrap().join("backup");
        let sha = std::env::var("GIL_SYNTHETIC_PLAN").unwrap();
        let stage = std::env::var("GIL_SYNTHETIC_CRASH").unwrap();
        apply_with(&root, &backup, &sha, &mut |at| {
            if at == stage {
                // Test-only real termination: no unwinding or Drop/receipt path.
                unsafe {
                    libc::kill(libc::getpid(), libc::SIGKILL);
                }
                unreachable!("SIGKILL returned");
            }
            Ok(())
        })
        .unwrap();
        panic!("crash checkpoint not reached");
    }
    #[test]
    fn interrupted_process_preserves_old_or_complete_new_state() {
        for stage in [
            "partial_write",
            "before_rename",
            "after_rename",
            "after_directory_sync",
        ] {
            let (root, backup) = fixture(true);
            let before = snap(&root);
            let sha = prepare(&root, &backup).unwrap();
            let saved = snap(&backup.join("original"));
            let result = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "visual_migration::tests::crash_child",
                    "--ignored",
                    "--nocapture",
                ])
                .env("GIL_SYNTHETIC_MIGRATION_ROOT", &root)
                .env("GIL_SYNTHETIC_PLAN", &sha)
                .env("GIL_SYNTHETIC_CRASH", stage)
                .output()
                .unwrap();
            assert_eq!(
                result.status.signal(),
                Some(libc::SIGKILL),
                "{}",
                String::from_utf8_lossy(&result.stdout)
            );
            let published = stage.starts_with("after_");
            assert_eq!(
                status(&root, &backup, &sha).unwrap(),
                if published {
                    "applied_state_present"
                } else {
                    "not_applied"
                }
            );
            if !published {
                old_unchanged(&root, &before);
            }
            assert_eq!(snap(&backup.join("original")), saved);
            assert!(!backup.join("applied.json").exists());
        }
    }
}
