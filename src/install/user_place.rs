//! Linux per-user placement of the Core: `~/.local/share/gil/bin/gil` and its `install.json`.
//!
//! One rule decides every branch: **an entry is ours only when a previous `gil install` recorded
//! it** — a regular file (never a link) whose bytes have a digest the regular `install.json`
//! beside it records. The rule is asked before anything is written, including when the bytes at
//! the fixed path already equal this executable's: equal bytes without our receipt are someone
//! else's file. A standalone receipt can resume only if its pending inode and parent match.
//!
//! ```text
//! parent folders   gil/ and gil/bin must be real folders of this user (created one level at a
//!                  time when absent; a link or another owner's folder refuses). The receipt
//!                  records which of the two THIS installation created; only those are removed.
//! fixed path       absent, no receipt → place · absent, pending receipt → resume; other receipt → refuse ·
//!                  ours, same digest → nothing to do · ours, other digest → update ·
//!                  anything else → refuse, unchanged
//! install.json     absent or a regular file; a link or a folder refuses before any write
//! first install    publish pending receipt, link the recorded inode at the fixed path, settle receipt.
//! update           receipt first, naming the new digest AND the installed one, then the
//!                  executable, then the receipt without the old digest. Whichever step fails or
//!                  is interrupted, the installed file is still recorded as ours, so running
//!                  `gil install` again finishes the update; nobody has to move a file by hand.
//! removing         the folders and the file are checked again right before the first unlink;
//!                  then the executable, its receipt, and the folders this installation made
//!                  (when empty). What was removed and what remains is reported as it happened.
//! ```
//!
//! Nothing here follows a link at the fixed path, parks an old file under another name, or cleans
//! up names it did not create in this call. Compiled on every Unix so the same code is exercised
//! where it is developed; only the Linux user install calls it.
use sha2::{Digest, Sha256};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use super::{INSTALL_RECEIPT, hex};

/// Failure injection for the steps between two renames or unlinks. Unit tests set the stage for
/// their own thread; a debug build also reads `GIL_INSTALL_FAIL=<stage>` so the real `gil install`
/// and `gil uninstall` entries can be driven. Release builds have neither: the function is empty.
fn fail_at(stage: &str) -> std::io::Result<()> {
    #[cfg(test)]
    if tests::STAGE.with(|at| at.borrow().as_deref() == Some(stage)) {
        return Err(std::io::Error::other(format!("injected failure at {stage}")));
    }
    #[cfg(debug_assertions)]
    if std::env::var("GIL_INSTALL_EXIT").is_ok_and(|at| at == stage) {
        std::process::exit(86);
    }
    #[cfg(debug_assertions)]
    if std::env::var("GIL_INSTALL_FAIL").is_ok_and(|at| at == stage) {
        return Err(std::io::Error::other(format!("failpoint GIL_INSTALL_FAIL={stage}")));
    }
    let _ = stage;
    Ok(())
}

fn entry(path: &Path) -> Result<Option<std::fs::Metadata>, String> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(format!("cannot inspect {}: {err}. Nothing was changed", path.display())),
    }
}

fn folders(fixed: &Path) -> Result<(&Path, &Path), String> {
    let dir = fixed.parent().ok_or("fixed path has no parent")?;
    let product = dir.parent().ok_or("fixed path has no product folder")?;
    Ok((product, dir))
}

/// A real folder (not a link) owned by the calling user.
fn own_folder(folder: &Path) -> Result<(), String> {
    let metadata = entry(folder)?.ok_or_else(|| format!("{} does not exist", folder.display()))?;
    if !metadata.is_dir() || metadata.uid() != unsafe { libc::geteuid() } {
        return Err(format!("{} is not a folder owned by this user (a link, a file or another owner's folder). Nothing was changed; move it away yourself, then run gil install again", folder.display()));
    }
    Ok(())
}

/// `…/gil` and `…/gil/bin`: real folders owned by the calling user. The data folder above them
/// (`~/.local/share`) is the user's own layout and is only created when missing. Returns the
/// names (`gil`, `gil/bin`) this call created.
fn own_folders(fixed: &Path) -> Result<Vec<&'static str>, String> {
    let (product, dir) = folders(fixed)?;
    let data = product.parent().ok_or("fixed path has no data folder")?;
    std::fs::create_dir_all(data).map_err(|err| format!("cannot create {}: {err}. Nothing was changed", data.display()))?;
    let mut made = Vec::new();
    for (folder, name) in [(product, "gil"), (dir, "gil/bin")] {
        if entry(folder)?.is_none() {
            std::fs::DirBuilder::new().mode(0o755).create(folder)
                .map_err(|err| format!("cannot create {}: {err}. Nothing was installed", folder.display()))?;
            made.push(name);
        }
        own_folder(folder)?;
    }
    Ok(made)
}

/// What a sane, regular `install.json` records. A link, a folder or an odd shape records nothing.
struct Recorded {
    sha256: String,
    /// Only while an update is between its steps: the digest of the file that was installed.
    previous: Option<String>,
    /// The folders this installation created (`gil`, `gil/bin`). Absent in a receipt written
    /// before this was recorded: then no folder is known to be ours and none is removed.
    made_dirs: Vec<String>,
    pending: bool,
    binary_identity: Option<[u64; 2]>,
    staged_name: Option<String>,
}

fn digest_text(value: Option<&serde_json::Value>) -> Option<String> {
    let digest = value?.as_str()?;
    (digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))).then(|| digest.to_owned())
}

fn recorded(dir: &Path) -> Option<Recorded> {
    let path = dir.join(INSTALL_RECEIPT);
    let metadata = std::fs::symlink_metadata(&path).ok()?;
    if !metadata.is_file() || metadata.len() > 4096 { return None; }
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    if value.get("schema")?.as_u64()? != 1 { return None; }
    let made_dirs = value.get("made_dirs").and_then(|dirs| dirs.as_array())
        .map(|dirs| dirs.iter().filter_map(|d| d.as_str()).filter(|d| matches!(*d, "gil" | "gil/bin")).map(str::to_owned).collect())
        .unwrap_or_default();
    // A pending receipt remains evidence after the executable disappears. A copied or moved
    // receipt does not have the recorded identity in this directory.
    let pending = value.get("pending_identity").is_some_and(|id| {
        std::fs::metadata(dir).is_ok_and(|parent|
            *id == serde_json::json!([metadata.dev(), metadata.ino(), parent.dev(), parent.ino()]))
    });
    if value.get("pending_identity").is_some() && !pending { return None; }
    let binary_identity = if pending { Some(serde_json::from_value(value.get("binary_identity")?.clone()).ok()?) } else { None };
    let staged_name = value.get("staged_name").and_then(|v| v.as_str()).map(str::to_owned);
    Some(Recorded { pending, binary_identity, staged_name, sha256: digest_text(value.get("sha256"))?, previous: digest_text(value.get("previous_sha256")), made_dirs })
}

/// The digest of the file at `fixed` and what the receipt records, when the entry there is ours:
/// a regular file whose bytes are the recorded digest, or the one an interrupted update names.
fn ours(dir: &Path, fixed: &Path) -> Option<(String, Recorded)> {
    if !std::fs::symlink_metadata(fixed).is_ok_and(|metadata| metadata.is_file()) { return None; }
    let record = recorded(dir)?;
    if let Some(expected) = record.binary_identity {
        let meta = std::fs::symlink_metadata(fixed).ok()?;
        if expected != [meta.dev(), meta.ino()] { return None; }
    }
    let installed = hex(&Sha256::digest(std::fs::read(fixed).ok()?));
    (installed == record.sha256 || record.previous.as_deref() == Some(installed.as_str())).then_some((installed, record))
}

/// A new name in `dir` that this call creates exclusively: never an existing file, never through a link.
fn write_new(dir: &Path, stem: &str, mode: u32, bytes: &[u8]) -> Result<PathBuf, String> {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let path = dir.join(format!(".{stem}.next-{}-{nanos}-{}", std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    let written = std::fs::OpenOptions::new().write(true).create_new(true).mode(mode).open(&path)
        .and_then(|mut file| {
            file.write_all(bytes)?;
            // The mode asked for above is cut by the umask; the installed modes are stated exactly.
            file.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(mode))?;
            file.sync_all()
        });
    match written {
        Ok(()) => Ok(path),
        Err(err) => {
            // Only when this call created the name (an existing one fails `create_new` and is not ours).
            if err.kind() != std::io::ErrorKind::AlreadyExists { let _ = std::fs::remove_file(&path); }
            Err(format!("cannot write in {}: {err}. The previous installation was not changed", dir.display()))
        }
    }
}

/// The receipt text: the caller's base fields plus what only this placement knows.
fn receipt_with(base: &str, made_dirs: &[String], previous: Option<&str>) -> Result<String, String> {
    let mut value: serde_json::Value = serde_json::from_str(base).map_err(|err| err.to_string())?;
    let fields = value.as_object_mut().ok_or("receipt is not an object")?;
    fields.insert("made_dirs".into(), serde_json::json!(made_dirs));
    if let Some(previous) = previous { fields.insert("previous_sha256".into(), serde_json::json!(previous)); }
    Ok(serde_json::to_string_pretty(&value).map_err(|err| err.to_string())? + "\n")
}

/// Write `text` to a new name and rename it over `install.json`. The staged name is removed on failure.
fn put_receipt(dir: &Path, text: &str, stage: &str) -> Result<(), String> {
    let staged = write_new(dir, INSTALL_RECEIPT, 0o644, text.as_bytes())?;
    fail_at(stage).and_then(|()| std::fs::rename(&staged, dir.join(INSTALL_RECEIPT))).map_err(|err| {
        let _ = std::fs::remove_file(&staged);
        format!("{INSTALL_RECEIPT} could not be written: {err}")
    })
}

/// Publish an intent before the executable appears or disappears. Construct the entire record
/// on a private inode before publishing it. A first install never replaces a pre-existing receipt.
fn pending_receipt(dir: &Path, text: &str, binary: &Path, first: bool, replace: bool) -> Result<(), String> {
    let mut value: serde_json::Value = serde_json::from_str(text).map_err(|err| err.to_string())?;
    let path = write_new(dir, INSTALL_RECEIPT, 0o644, b"")?;
    let mut publish = || -> std::io::Result<()> {
        let parent = std::fs::metadata(dir)?;
        let mut file = std::fs::OpenOptions::new().write(true).open(&path)?;
        let meta = file.metadata()?;
        value["pending_identity"] = serde_json::json!([meta.dev(), meta.ino(), parent.dev(), parent.ino()]);
        let binary_meta = std::fs::symlink_metadata(binary)?;
        value["binary_identity"] = serde_json::json!([binary_meta.dev(), binary_meta.ino()]);
        if first { value["staged_name"] = serde_json::json!(binary.file_name().and_then(|n| n.to_str())); }
        serde_json::to_writer_pretty(&mut file, &value)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        let receipt = dir.join(INSTALL_RECEIPT);
        if replace { std::fs::rename(&path, receipt) } else { std::fs::hard_link(&path, receipt) }
    };
    let result = publish();
    let _ = std::fs::remove_file(&path);
    result.map_err(|err| format!("cannot record the installation in {}: {err}", dir.display()))
}

/// An interrupted first publication may still have its private link. Match its recorded inode
/// before unlinking. A different entry at that name belongs to somebody else.
fn clean_staged(dir: &Path, record: &Recorded) -> Result<(), String> {
    if let (Some(id), Some(name)) = (record.binary_identity, record.staged_name.as_deref()) {
        if name.starts_with(".gil.next-") && !name.contains('/') {
            let path = dir.join(name);
            if entry(&path)?.is_some_and(|m| m.is_file() && [m.dev(), m.ino()] == id) {
                std::fs::remove_file(&path).map_err(|err| format!("cannot remove {}: {err}", path.display()))?;
            }
        }
    }
    Ok(())
}

/// Place `bytes` (this executable, digest `digest`) at `fixed`. `receipt` is the base text of the
/// new `install.json`. Returns `OK` when our own file with the same digest is already there.
pub(super) fn place(fixed: &Path, bytes: &[u8], digest: &str, receipt: &str) -> Result<&'static str, String> {
    let (_, dir) = folders(fixed)?;
    let made: Vec<String> = own_folders(fixed)?.into_iter().map(str::to_owned).collect();
    let receipt_path = dir.join(INSTALL_RECEIPT);
    if entry(&receipt_path)?.is_some_and(|metadata| !metadata.is_file()) {
        return Err(format!("{} is not a regular file (a link or a folder). Nothing was changed; move it away yourself, then run gil install again", receipt_path.display()));
    }
    if entry(fixed)?.is_none() {
        let previous = recorded(dir).filter(|record| record.pending);
        if entry(&receipt_path)?.is_some() && previous.is_none() {
            return Err(format!("{} exists without the executable it would record, so it cannot be shown to be gil install's. Nothing was changed; move it away yourself, then run gil install again", receipt_path.display()));
        }
        let dirs = previous.as_ref().map_or(made.as_slice(), |record| &record.made_dirs);
        let settled = receipt_with(receipt, dirs, None)?;
        if let Some(record) = &previous { clean_staged(dir, record)?; }
        let prepare = || {
            fail_at("first-receipt").map_err(|err| format!("cannot record first installation: {err}. Nothing was installed"))?;
            let staged = write_new(dir, "gil", 0o755, bytes)?;
            if let Err(why) = pending_receipt(dir, &settled, &staged, true, previous.is_some()) {
                let _ = std::fs::remove_file(&staged);
                return Err(why);
            }
            Ok(staged)
        };
        let staged = prepare().inspect_err(|_| {
            // No intent was published. Return only the empty folders created by this call.
            remove_made_folders(dir, &made);
        })?;
        fail_at("first-intent").map_err(|err| err.to_string())?;
        let result = fail_at("first-binary").and_then(|()| std::fs::hard_link(&staged, fixed));
        let _ = std::fs::remove_file(staged);
        result.map_err(|err| format!("cannot place {}: {err}. Run gil install again", fixed.display()))?;
        fail_at("first-published").map_err(|err| err.to_string())?;
        put_receipt(dir, &settled, "settle-receipt")?;
        return Ok("UPDATED");
    }
    let Some((installed, record)) = ours(dir, fixed) else {
        return Err(format!("{} already exists and is not a file placed by gil install (no matching install.json). Nothing was changed; move it away yourself, then run gil install again", fixed.display()));
    };
    clean_staged(dir, &record)?;
    // Folders are ours only as the receipt of the installation that made them says; an update never claims more.
    let settled = receipt_with(receipt, &record.made_dirs, None)?;
    if installed == digest {
        // This very file is installed. A receipt left between the steps of an update is settled.
        if record.pending || record.sha256 != digest || record.previous.is_some() {
            put_receipt(dir, &settled, "settle-receipt").map_err(|why| format!("{why}. The installation was not changed; run gil install again"))?;
        }
        return Ok("OK");
    }
    // Update. Step 1: the receipt names the new digest and keeps the installed one, so the file at
    // the fixed path is recorded as ours before, between and after the two renames.
    let staged = write_new(dir, "gil", 0o755, bytes)?;
    if let Err(why) = put_receipt(dir, &receipt_with(receipt, &record.made_dirs, Some(&installed))?, "update-receipt") {
        let _ = std::fs::remove_file(&staged);
        return Err(format!("{why}. The previous installation was not changed"));
    }
    // Step 2: the executable.
    if let Err(err) = fail_at("update-binary").and_then(|()| std::fs::rename(&staged, fixed)) {
        let _ = std::fs::remove_file(&staged);
        return Err(format!("cannot place {}: {err}. The previous executable is still installed and recorded; run gil install again to finish the update", fixed.display()));
    }
    // Step 3: the receipt without the old digest. If this fails the installation is complete and
    // still recorded (the new digest is the receipt's own); the next install settles it.
    put_receipt(dir, &settled, "settle-receipt")
        .map_err(|why| format!("the executable was updated at {} and is recorded, but {why}. No Host setting was changed; run gil install again to settle the record", fixed.display()))?;
    Ok("UPDATED")
}

/// What a removal actually did.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Removal {
    pub(super) found: bool,
    /// The entry at the fixed path is not the recorded file: nothing was touched.
    pub(super) kept: bool,
    pub(super) removed: Vec<String>,
    pub(super) remaining: Vec<String>,
}

impl Removal {
    pub(super) fn complete(&self) -> bool { self.found && !self.kept && self.remaining.is_empty() }
}

/// Remove the executable only when it is ours, then its receipt, then the folders this
/// installation created when they are empty. A running executable can be removed on Linux; the
/// process keeps its file. Ownership is asked again right before the first unlink.
pub(super) fn remove(fixed: &Path) -> Result<Removal, String> {
    let (product, dir) = folders(fixed)?;
    let receipt = dir.join(INSTALL_RECEIPT);
    let binary_present = entry(fixed)?.is_some();
    if !binary_present && entry(&receipt)?.is_none() { return Ok(Removal::default()); }
    own_folder(product)?;
    own_folder(dir)?;
    let record = if binary_present {
        ours(dir, fixed).map(|(_, record)| record)
    } else {
        recorded(dir).filter(|record| record.pending)
    };
    let Some(record) = record else {
        return Ok(Removal { found: true, kept: true, ..Removal::default() });
    };
    let mut done = Removal { found: true, ..Removal::default() };
    clean_staged(dir, &record)?;
    if binary_present {
        let text = std::fs::read_to_string(&receipt).map_err(|err| err.to_string())?;
        pending_receipt(dir, &text, fixed, false, true)?;
        if let Err(err) = fail_at("remove-binary").and_then(|()| std::fs::remove_file(fixed)) {
            done.remaining.push(format!("{} ({err})", fixed.display()));
            done.remaining.push(receipt.display().to_string());
            return Ok(done);
        }
        done.removed.push(fixed.display().to_string());
    }
    match fail_at("remove-receipt").and_then(|()| std::fs::remove_file(&receipt)) {
        Ok(()) => done.removed.push(receipt.display().to_string()),
        Err(err) => {
            done.remaining.push(format!("{} ({err})", receipt.display()));
            return Ok(done);
        }
    }
    // Folders: only the ones the receipt says this installation made, innermost first, when empty.
    // A folder that existed before the installation, or one whose origin was never recorded, stays.
    done.removed.extend(remove_made_folders(dir, &record.made_dirs));
    Ok(done)
}

fn remove_made_folders(dir: &Path, made_dirs: &[String]) -> Vec<String> {
    let mut removed = Vec::new();
    for (folder, name) in [(Some(dir), "gil/bin"), (dir.parent(), "gil")] {
        if let Some(folder) = folder {
            if made_dirs.iter().any(|made| made == name) && std::fs::remove_dir(folder).is_ok() {
                removed.push(folder.display().to_string());
            }
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::tests::scratch;

    thread_local! { pub(super) static STAGE: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) }; }
    /// Runs `f` with one injected failure; the stage is cleared afterwards whatever happens.
    fn failing<T>(stage: &str, f: impl FnOnce() -> T) -> T {
        STAGE.with(|at| *at.borrow_mut() = Some(stage.to_owned()));
        let out = f();
        STAGE.with(|at| *at.borrow_mut() = None);
        out
    }

    fn receipt(digest: &str) -> String { format!("{{\"schema\":1,\"version\":\"0.0.0\",\"sha256\":\"{digest}\",\"installed_at\":1}}\n") }
    fn digest(bytes: &[u8]) -> String { hex(&Sha256::digest(bytes)) }
    fn put(fixed: &Path, bytes: &[u8]) -> Result<&'static str, String> { place(fixed, bytes, &digest(bytes), &receipt(&digest(bytes))) }
    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }
    fn home(tag: &str) -> (PathBuf, PathBuf) {
        let root = scratch(tag);
        let fixed = root.join("home/.local/share/gil/bin/gil");
        (root, fixed)
    }
    fn receipt_value(dir: &Path) -> serde_json::Value { serde_json::from_slice(&std::fs::read(dir.join(INSTALL_RECEIPT)).unwrap()).unwrap() }

    #[test]
    fn a_first_install_an_update_and_a_repeat_leave_exactly_the_two_recorded_files() {
        let (root, fixed) = home("place-own");
        assert_eq!(put(&fixed, b"core one").unwrap(), "UPDATED");
        let dir = fixed.parent().unwrap();
        assert_eq!(names(dir), ["gil", "install.json"], "no temporary or parked name stays");
        assert_eq!(std::fs::symlink_metadata(&fixed).unwrap().mode() & 0o777, 0o755);
        assert_eq!(std::fs::symlink_metadata(dir.join(INSTALL_RECEIPT)).unwrap().mode() & 0o777, 0o644);
        assert_eq!(receipt_value(dir)["made_dirs"], serde_json::json!(["gil", "gil/bin"]), "the folders this installation made are recorded");
        // The same bytes again: nothing is rewritten.
        let before = (std::fs::symlink_metadata(&fixed).unwrap().ino(), std::fs::read(dir.join(INSTALL_RECEIPT)).unwrap());
        assert_eq!(put(&fixed, b"core one").unwrap(), "OK");
        assert_eq!((std::fs::symlink_metadata(&fixed).unwrap().ino(), std::fs::read(dir.join(INSTALL_RECEIPT)).unwrap()), before);
        // Other bytes: our own file is replaced, and the receipt follows it without an old digest.
        assert_eq!(put(&fixed, b"core two").unwrap(), "UPDATED");
        assert_eq!(std::fs::read(&fixed).unwrap(), b"core two");
        let record = recorded(dir).unwrap();
        assert_eq!((record.sha256, record.previous, record.made_dirs), (digest(b"core two"), None, vec!["gil".to_owned(), "gil/bin".to_owned()]));
        assert_eq!(names(dir), ["gil", "install.json"]);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn equal_bytes_without_our_receipt_are_not_taken_over() {
        let (root, fixed) = home("place-equal");
        let dir = fixed.parent().unwrap();
        std::fs::create_dir_all(dir).unwrap();
        // Someone else's copy of the very same bytes, with no receipt: the digest branch must not adopt it.
        std::fs::write(&fixed, b"core one").unwrap();
        let refused = put(&fixed, b"core one").unwrap_err();
        assert!(refused.contains("Nothing was changed"), "{refused}");
        assert_eq!(names(dir), ["gil"], "no receipt was written beside a file that is not ours");
        // A receipt for other bytes does not make it ours either.
        std::fs::write(dir.join(INSTALL_RECEIPT), receipt(&digest(b"an older core"))).unwrap();
        assert!(put(&fixed, b"core one").is_err());
        assert_eq!(recorded(dir).unwrap().sha256, digest(b"an older core"));
        let kept = remove(&fixed).unwrap();
        assert!(kept.kept && kept.removed.is_empty());
        assert_eq!(std::fs::read(&fixed).unwrap(), b"core one");
        // A link to equal bytes is never ours and never written through, with or without a receipt.
        std::fs::remove_file(&fixed).unwrap();
        let elsewhere = root.join("elsewhere");
        std::fs::write(&elsewhere, b"core one").unwrap();
        std::os::unix::fs::symlink(&elsewhere, &fixed).unwrap();
        std::fs::write(dir.join(INSTALL_RECEIPT), receipt(&digest(b"core one"))).unwrap();
        assert!(put(&fixed, b"core one").is_err());
        assert!(put(&fixed, b"core two").is_err());
        assert_eq!(std::fs::read(&elsewhere).unwrap(), b"core one", "the install wrote through a link");
        assert!(std::fs::symlink_metadata(&fixed).unwrap().is_symlink());
        assert!(remove(&fixed).unwrap().kept);
        let _ = std::fs::remove_dir_all(root);
    }

    /// ① A regular `install.json` with no executable beside it is not shown to be ours: it is not
    /// overwritten, whatever it holds, and nothing is placed.
    #[test]
    fn a_receipt_without_its_executable_is_never_overwritten() {
        for (tag, text) in [("lone-user-file", b"my own notes, not a receipt\n".to_vec()), ("lone-receipt-shape", receipt(&digest(b"core one")).into_bytes()),
            ("lone-same-digest", receipt(&digest(b"core two")).into_bytes())] {
            let (root, fixed) = home(tag);
            let dir = fixed.parent().unwrap();
            std::fs::create_dir_all(dir).unwrap();
            std::fs::write(dir.join(INSTALL_RECEIPT), &text).unwrap();
            let refused = put(&fixed, b"core two").unwrap_err();
            assert!(refused.contains("Nothing was changed"), "{tag}: {refused}");
            assert_eq!(std::fs::read(dir.join(INSTALL_RECEIPT)).unwrap(), text, "{tag}: the file was rewritten");
            assert_eq!(names(dir), ["install.json"], "{tag}: something was placed or staged");
            // Removal reports the collision and leaves the standalone file alone.
            assert!(remove(&fixed).unwrap().kept);
            assert_eq!(std::fs::read(dir.join(INSTALL_RECEIPT)).unwrap(), text);
            let _ = std::fs::remove_dir_all(root);
        }
    }

    #[test]
    fn a_linked_or_foreign_parent_folder_and_an_odd_receipt_refuse_before_any_write() {
        // `gil/` is a link to another place: nothing is created behind it.
        let (root, fixed) = home("place-parent-link");
        let share = root.join("home/.local/share");
        std::fs::create_dir_all(&share).unwrap();
        let other = root.join("other-place");
        std::fs::create_dir(&other).unwrap();
        std::os::unix::fs::symlink(&other, share.join("gil")).unwrap();
        assert!(put(&fixed, b"core").unwrap_err().contains("Nothing was changed"));
        assert!(names(&other).is_empty(), "something was written behind a linked folder");
        let _ = std::fs::remove_dir_all(root);

        // `gil/bin` is a file.
        let (root, fixed) = home("place-parent-file");
        std::fs::create_dir_all(root.join("home/.local/share/gil")).unwrap();
        std::fs::write(root.join("home/.local/share/gil/bin"), b"not a folder").unwrap();
        assert!(put(&fixed, b"core").is_err());
        assert_eq!(std::fs::read(root.join("home/.local/share/gil/bin")).unwrap(), b"not a folder");
        let _ = std::fs::remove_dir_all(root);

        // `install.json` is a link or a folder: refused before the executable is staged.
        for kind in ["link", "folder"] {
            let (root, fixed) = home(&format!("place-receipt-{kind}"));
            let dir = fixed.parent().unwrap();
            std::fs::create_dir_all(dir).unwrap();
            let target = root.join("receipt-target");
            if kind == "link" {
                std::fs::write(&target, b"keep").unwrap();
                std::os::unix::fs::symlink(&target, dir.join(INSTALL_RECEIPT)).unwrap();
            } else {
                std::fs::create_dir(dir.join(INSTALL_RECEIPT)).unwrap();
            }
            assert!(put(&fixed, b"core").unwrap_err().contains("Nothing was changed"));
            assert_eq!(names(dir), ["install.json"], "{kind}: the executable was staged or placed");
            if kind == "link" { assert_eq!(std::fs::read(&target).unwrap(), b"keep"); }
            let _ = std::fs::remove_dir_all(root);
        }
    }

    #[test]
    fn names_the_install_did_not_create_survive_an_update_and_the_removal() {
        let (root, fixed) = home("place-keep");
        assert_eq!(put(&fixed, b"core one").unwrap(), "UPDATED");
        let dir = fixed.parent().unwrap();
        // A user's own files beside the executable, including names an older installer would have swept.
        for name in ["notes.txt", "gil.exe.old-1", "install.json.next", ".gil.next-1-1"] { std::fs::write(dir.join(name), b"mine").unwrap(); }
        assert_eq!(put(&fixed, b"core two").unwrap(), "UPDATED");
        assert_eq!(names(dir), [".gil.next-1-1", "gil", "gil.exe.old-1", "install.json", "install.json.next", "notes.txt"]);
        let sentinel = root.join("home/.local/share/other-app.conf");
        std::fs::write(&sentinel, b"keep").unwrap();
        let removal = remove(&fixed).unwrap();
        assert!(removal.complete(), "{removal:?}");
        assert_eq!(removal.removed.len(), 2, "the folder is not empty, so only the two recorded files went: {removal:?}");
        assert_eq!(names(dir), [".gil.next-1-1", "gil.exe.old-1", "install.json.next", "notes.txt"], "only the two recorded files went");
        for name in names(dir) { assert_eq!(std::fs::read(dir.join(name)).unwrap(), b"mine"); }
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"keep");
        assert_eq!(remove(&fixed).unwrap(), Removal::default());
        let _ = std::fs::remove_dir_all(root);
    }

    /// ② Folders that were there before the installation stay after the removal, also when they
    /// are empty and also when the receipt (an older one) does not say who made them.
    #[test]
    fn folders_that_existed_before_the_installation_are_never_removed() {
        for (tag, existing, expect_made) in [("dirs-both", vec!["gil", "gil/bin"], vec![]), ("dirs-product", vec!["gil"], vec!["gil/bin"]), ("dirs-none", vec![], vec!["gil", "gil/bin"])] {
            let (root, fixed) = home(tag);
            let share = root.join("home/.local/share");
            std::fs::create_dir_all(&share).unwrap();
            for folder in &existing { std::fs::create_dir(share.join(folder)).unwrap(); }
            assert_eq!(put(&fixed, b"core one").unwrap(), "UPDATED");
            let dir = fixed.parent().unwrap();
            assert_eq!(receipt_value(dir)["made_dirs"], serde_json::json!(expect_made), "{tag}");
            // An update does not claim folders the first installation did not make.
            assert_eq!(put(&fixed, b"core two").unwrap(), "UPDATED");
            assert_eq!(receipt_value(dir)["made_dirs"], serde_json::json!(expect_made), "{tag}");
            let removal = remove(&fixed).unwrap();
            assert!(removal.complete(), "{removal:?}");
            for folder in ["gil", "gil/bin"] {
                assert_eq!(share.join(folder).exists(), existing.contains(&folder), "{tag}: {folder}");
            }
            assert!(share.is_dir(), "the user's data folder stays");
            let _ = std::fs::remove_dir_all(root);
        }
        // A receipt that never recorded who made the folders (written before that was recorded).
        let (root, fixed) = home("dirs-unrecorded");
        let dir = fixed.parent().unwrap().to_owned();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&fixed, b"core one").unwrap();
        std::fs::write(dir.join(INSTALL_RECEIPT), receipt(&digest(b"core one"))).unwrap();
        assert_eq!(put(&fixed, b"core two").unwrap(), "UPDATED");
        assert_eq!(receipt_value(&dir)["made_dirs"], serde_json::json!([]));
        assert!(remove(&fixed).unwrap().complete());
        assert!(dir.is_dir() && names(&dir).is_empty(), "the emptied folder whose origin is unknown stays");
        let _ = std::fs::remove_dir_all(root);
    }

    /// ③ Ownership is asked again right before the first unlink, and a removal that only half
    /// happened says exactly what went and what is still there.
    #[test]
    fn a_removal_rechecks_ownership_and_reports_a_partial_result_as_partial() {
        // A file changed by the user after the install is kept.
        let (root, fixed) = home("remove-changed");
        assert_eq!(put(&fixed, b"core one").unwrap(), "UPDATED");
        let dir = fixed.parent().unwrap().to_owned();
        std::fs::write(&fixed, b"edited by the user").unwrap();
        let kept = remove(&fixed).unwrap();
        assert!(kept.kept && !kept.complete() && kept.removed.is_empty());
        assert_eq!(std::fs::read(&fixed).unwrap(), b"edited by the user");
        assert!(dir.join(INSTALL_RECEIPT).exists());
        // The folder became a link to another place: nothing is removed through it.
        std::fs::write(&fixed, b"core one").unwrap();
        let moved = root.join("moved-bin");
        std::fs::rename(&dir, &moved).unwrap();
        std::os::unix::fs::symlink(&moved, &dir).unwrap();
        assert!(remove(&fixed).is_err(), "a linked folder was accepted at removal");
        assert_eq!(names(&moved), ["gil", "install.json"]);
        std::fs::remove_file(&dir).unwrap();
        std::fs::rename(&moved, &dir).unwrap();

        // The executable goes, the receipt cannot: partial, with both facts.
        let partial = failing("remove-receipt", || remove(&fixed)).unwrap();
        assert!(!partial.complete() && !partial.kept, "{partial:?}");
        assert_eq!(partial.removed, [fixed.display().to_string()]);
        assert!(partial.remaining.len() == 1 && partial.remaining[0].starts_with(&dir.join(INSTALL_RECEIPT).display().to_string()), "{partial:?}");
        assert!(!fixed.exists() && dir.join(INSTALL_RECEIPT).exists() && dir.is_dir(), "the folders stay while something of the installation remains");
        // This partial removal, unlike a foreign standalone receipt, can be finished.
        assert!(remove(&fixed).unwrap().complete());
        assert!(!dir.exists());
        let _ = std::fs::remove_dir_all(root);

        // The executable itself cannot be removed: nothing went, and both are reported as remaining.
        let (root, fixed) = home("remove-nothing");
        assert_eq!(put(&fixed, b"core one").unwrap(), "UPDATED");
        let nothing = failing("remove-binary", || remove(&fixed)).unwrap();
        assert!(nothing.removed.is_empty() && nothing.remaining.len() == 2 && !nothing.complete(), "{nothing:?}");
        assert!(ours(fixed.parent().unwrap(), &fixed).is_some(), "the installation is intact");
        assert!(remove(&fixed).unwrap().complete(), "and a later removal completes");
        assert!(!fixed.parent().unwrap().parent().unwrap().exists(), "the two folders this installation made are gone");
        let _ = std::fs::remove_dir_all(root);
    }

    /// ④ An update that stops at any step leaves the installed file recorded as ours, and the same
    /// command run again completes it. No file has to be moved by hand.
    #[test]
    fn an_update_interrupted_at_any_step_is_finished_by_running_the_install_again() {
        for stage in ["update-receipt", "update-binary", "settle-receipt"] {
            let (root, fixed) = home(&format!("update-{stage}"));
            assert_eq!(put(&fixed, b"core one").unwrap(), "UPDATED");
            let dir = fixed.parent().unwrap();
            let failed = failing(stage, || put(&fixed, b"core two")).unwrap_err();
            assert!(!failed.contains("move it away"), "{stage}: the user was told to move a file: {failed}");
            assert_eq!(names(dir), ["gil", "install.json"], "{stage}: a staged name was left");
            let installed = std::fs::read(&fixed).unwrap();
            assert_eq!(installed, if stage == "settle-receipt" { b"core two".to_vec() } else { b"core one".to_vec() }, "{stage}");
            let (seen, record) = ours(dir, &fixed).unwrap_or_else(|| panic!("{stage}: the installed file is no longer recorded as ours"));
            assert_eq!(seen, digest(&installed));
            match stage {
                // Nothing was renamed: the previous installation exactly as it was.
                "update-receipt" => assert_eq!((record.sha256, record.previous), (digest(b"core one"), None)),
                // The receipt is ahead of the executable and still names the installed digest.
                "update-binary" => assert_eq!((record.sha256, record.previous), (digest(b"core two"), Some(digest(b"core one")))),
                // The executable is updated and recorded; only the old digest was not yet dropped.
                _ => assert_eq!((record.sha256, record.previous), (digest(b"core two"), Some(digest(b"core one")))),
            }
            // The interrupted state is removable as ours, and equally finishable.
            assert_eq!(put(&fixed, b"core two").unwrap(), if stage == "settle-receipt" { "OK" } else { "UPDATED" }, "{stage}");
            assert_eq!(std::fs::read(&fixed).unwrap(), b"core two");
            let record = recorded(dir).unwrap();
            assert_eq!((record.sha256, record.previous), (digest(b"core two"), None), "{stage}: the record is settled");
            assert_eq!(names(dir), ["gil", "install.json"]);
            let _ = std::fs::remove_dir_all(root);
        }
        // The old executable run again while the receipt is ahead: it is still the installed one,
        // and its own install settles the record back to it.
        let (root, fixed) = home("update-old-again");
        assert_eq!(put(&fixed, b"core one").unwrap(), "UPDATED");
        failing("update-binary", || put(&fixed, b"core two")).unwrap_err();
        assert_eq!(put(&fixed, b"core one").unwrap(), "OK");
        let record = recorded(fixed.parent().unwrap()).unwrap();
        assert_eq!((record.sha256, record.previous), (digest(b"core one"), None));
        let _ = std::fs::remove_dir_all(root);
    }

    /// A first installation can retry both before and after publishing its intent.
    #[test]
    fn a_first_install_can_retry_a_receipt_or_binary_failure() {
        let (root, fixed) = home("first-rollback");
        let failed = failing("first-receipt", || put(&fixed, b"core one")).unwrap_err();
        assert!(failed.contains("Nothing was installed") && !failed.contains("yourself"), "{failed}");
        let dir = fixed.parent().unwrap();
        assert!(!dir.exists(), "new folders remained after the pre-publication failure");
        assert_eq!(put(&fixed, b"core one").unwrap(), "UPDATED");
        let _ = std::fs::remove_dir_all(root);
        // The executable itself cannot be placed: nothing is installed and nothing staged stays.
        let (root, fixed) = home("first-binary");
        assert!(failing("first-binary", || put(&fixed, b"core one")).is_err());
        assert!(recorded(fixed.parent().unwrap()).unwrap().pending);
        let dir = fixed.parent().unwrap();
        let original = dir.join(INSTALL_RECEIPT);
        let saved = dir.join("saved-receipt");
        std::fs::rename(&original, &saved).unwrap();
        std::fs::copy(&saved, &original).unwrap();
        assert!(put(&fixed, b"core one").is_err(), "a byte-identical copy cannot resume");
        assert!(remove(&fixed).unwrap().kept);
        std::fs::rename(&saved, &original).unwrap();
        assert_eq!(put(&fixed, b"core one").unwrap(), "UPDATED");
        let _ = std::fs::remove_dir_all(root);
    }
}
