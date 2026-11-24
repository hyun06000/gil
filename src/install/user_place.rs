//! Linux per-user placement of the Core: `~/.local/share/gil/bin/gil` and its `install.json`.
//!
//! One rule decides every branch: **an entry is ours only when a previous `gil install` recorded
//! it** — a regular file (never a link) whose bytes have a digest the regular `install.json`
//! beside it records. The rule is asked before anything is written, including when the bytes at
//! the fixed path already equal this executable's: equal bytes without our receipt are someone
//! else's file. A receipt with no executable beside it proves nothing either and is never
//! overwritten.
//!
//! ```text
//! parent folders   gil/ and gil/bin must be real folders of this user (created one level at a
//!                  time when absent; a link or another owner's folder refuses). The receipt
//!                  records which of the two THIS installation created; only those are removed.
//! fixed path       absent, no receipt → place · absent, receipt present → refuse, unchanged ·
//!                  ours, same digest → nothing to do · ours, other digest → update ·
//!                  anything else → refuse, unchanged
//! install.json     absent or a regular file; a link or a folder refuses before any write
//! first install    executable, then receipt. If the receipt cannot be placed, the executable
//!                  this call just placed is taken back: the folder is as it was.
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
    Some(Recorded { sha256: digest_text(value.get("sha256"))?, previous: digest_text(value.get("previous_sha256")), made_dirs })
}

/// The digest of the file at `fixed` and what the receipt records, when the entry there is ours:
/// a regular file whose bytes are the recorded digest, or the one an interrupted update names.
fn ours(dir: &Path, fixed: &Path) -> Option<(String, Recorded)> {
    if !std::fs::symlink_metadata(fixed).is_ok_and(|metadata| metadata.is_file()) { return None; }
    let record = recorded(dir)?;
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
        // A receipt with no executable beside it records nothing we can check: it may be the user's
        // own file, and it is not overwritten.
        if entry(&receipt_path)?.is_some() {
            return Err(format!("{} exists without the executable it would record, so it cannot be shown to be gil install's. Nothing was changed; move it away yourself, then run gil install again", receipt_path.display()));
        }
        return first(fixed, dir, bytes, &receipt_with(receipt, &made, None)?);
    }
    let Some((installed, record)) = ours(dir, fixed) else {
        return Err(format!("{} already exists and is not a file placed by gil install (no matching install.json). Nothing was changed; move it away yourself, then run gil install again", fixed.display()));
    };
    // Folders are ours only as the receipt of the installation that made them says; an update never claims more.
    let settled = receipt_with(receipt, &record.made_dirs, None)?;
    if installed == digest {
        // This very file is installed. A receipt left between the steps of an update is settled.
        if record.sha256 != digest || record.previous.is_some() {
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

/// First installation into a folder that holds neither file: the executable, then its receipt.
fn first(fixed: &Path, dir: &Path, bytes: &[u8], receipt: &str) -> Result<&'static str, String> {
    let staged = write_new(dir, "gil", 0o755, bytes)?;
    if let Err(err) = fail_at("first-binary").and_then(|()| std::fs::rename(&staged, fixed)) {
        let _ = std::fs::remove_file(&staged);
        return Err(format!("cannot place {}: {err}. Nothing was installed", fixed.display()));
    }
    if let Err(why) = put_receipt(dir, receipt, "first-receipt") {
        // Take back the file this call placed a moment ago, and only that file: same bytes, regular.
        let ours_still = std::fs::symlink_metadata(fixed).is_ok_and(|metadata| metadata.is_file())
            && std::fs::read(fixed).is_ok_and(|now| now == bytes);
        let taken_back = ours_still && fail_at("first-rollback").and_then(|()| std::fs::remove_file(fixed)).is_ok();
        return Err(if taken_back {
            format!("{why}. The executable placed by this run was removed again; nothing is installed and no Host setting was changed. Run gil install again")
        } else {
            format!("{why}, and the executable placed by this run at {} could not be removed again. It is not recorded as installed and no Host setting was changed; remove that file yourself, then run gil install again", fixed.display())
        });
    }
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
    if entry(fixed)?.is_none() {
        return Ok(Removal::default());
    }
    let Some((_, record)) = ours(dir, fixed) else { return Ok(Removal { found: true, kept: true, ..Removal::default() }); };
    // Right before removing: the two folders are still this user's real folders, and the file is
    // still the recorded one. A swap since the check above stops here with nothing removed.
    own_folder(product)?;
    own_folder(dir)?;
    if ours(dir, fixed).is_none() {
        return Ok(Removal { found: true, kept: true, ..Removal::default() });
    }
    let mut done = Removal { found: true, ..Removal::default() };
    let receipt = dir.join(INSTALL_RECEIPT);
    if let Err(err) = fail_at("remove-binary").and_then(|()| std::fs::remove_file(fixed)) {
        done.remaining.push(format!("{} ({err})", fixed.display()));
        done.remaining.push(receipt.display().to_string());
        return Ok(done);
    }
    done.removed.push(fixed.display().to_string());
    match fail_at("remove-receipt").and_then(|()| std::fs::remove_file(&receipt)) {
        Ok(()) => done.removed.push(receipt.display().to_string()),
        Err(err) => {
            done.remaining.push(format!("{} ({err})", receipt.display()));
            return Ok(done);
        }
    }
    // Folders: only the ones the receipt says this installation made, innermost first, when empty.
    // A folder that existed before the installation, or one whose origin was never recorded, stays.
    for (folder, name) in [(dir, "gil/bin"), (product, "gil")] {
        if record.made_dirs.iter().any(|made| made == name) && std::fs::remove_dir(folder).is_ok() {
            done.removed.push(folder.display().to_string());
        }
    }
    Ok(done)
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
            // Removal has nothing recorded to remove and leaves that file alone.
            assert_eq!(remove(&fixed).unwrap(), Removal::default());
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
        // What is left is a receipt without its executable: not overwritten by a later install (①).
        assert!(put(&fixed, b"core one").is_err());
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

    /// A first installation whose receipt cannot be placed takes its own executable back: the
    /// folder holds nothing of it, and the same command works afterwards.
    #[test]
    fn a_first_install_that_cannot_record_itself_takes_its_executable_back() {
        let (root, fixed) = home("first-rollback");
        let failed = failing("first-receipt", || put(&fixed, b"core one")).unwrap_err();
        assert!(failed.contains("removed again") && !failed.contains("yourself"), "{failed}");
        let dir = fixed.parent().unwrap();
        assert!(names(dir).is_empty(), "something of the failed installation stayed: {:?}", names(dir));
        assert_eq!(put(&fixed, b"core one").unwrap(), "UPDATED");
        let _ = std::fs::remove_dir_all(root);
        // The executable itself cannot be placed: nothing is installed and nothing staged stays.
        let (root, fixed) = home("first-binary");
        assert!(failing("first-binary", || put(&fixed, b"core one")).unwrap_err().contains("Nothing was installed"));
        assert!(names(fixed.parent().unwrap()).is_empty());
        let _ = std::fs::remove_dir_all(root);
    }
}
