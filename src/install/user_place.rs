//! Linux per-user placement of the Core: `~/.local/share/gil/bin/gil` and its `install.json`.
//!
//! One rule decides every branch: **an entry is ours only when a previous `gil install` recorded
//! it** — a regular file (never a link) whose bytes have the digest in the regular `install.json`
//! beside it. The rule is asked before anything is written, including when the bytes at the fixed
//! path already equal this executable's: equal bytes without our receipt are someone else's file.
//!
//! ```text
//! parent folders   gil/ and gil/bin must be real folders of this user (created one level at a
//!                  time when absent; a link or another owner's folder refuses)
//! fixed path       absent → place · ours, same digest → nothing to do · ours, other digest →
//!                  replace · anything else → refuse, unchanged
//! install.json     absent or a regular file; a link or a folder refuses before any write
//! writing          both files are written to new, exclusively created names in the same folder,
//!                  then renamed over their targets; a failure before the renames removes only
//!                  the two names this call created and leaves the previous installation as it was
//! removing         only a file that is ours, then its receipt, then the two folders when empty
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

fn entry(path: &Path) -> Result<Option<std::fs::Metadata>, String> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(format!("cannot inspect {}: {err}. Nothing was changed", path.display())),
    }
}

/// `…/gil` and `…/gil/bin`: real folders owned by the calling user. The data folder above them
/// (`~/.local/share`) is the user's own layout and is only created when missing.
fn own_folders(dir: &Path) -> Result<(), String> {
    let product = dir.parent().ok_or("fixed path has no product folder")?;
    let data = product.parent().ok_or("fixed path has no data folder")?;
    std::fs::create_dir_all(data).map_err(|err| format!("cannot create {}: {err}. Nothing was changed", data.display()))?;
    let me = unsafe { libc::geteuid() };
    for folder in [product, dir] {
        if entry(folder)?.is_none() {
            std::fs::DirBuilder::new().mode(0o755).create(folder)
                .map_err(|err| format!("cannot create {}: {err}. Nothing was installed", folder.display()))?;
        }
        let metadata = entry(folder)?.ok_or_else(|| format!("{} disappeared. Nothing was installed", folder.display()))?;
        if !metadata.is_dir() || metadata.uid() != me {
            return Err(format!("{} is not a folder owned by this user (a link, a file or another owner's folder). Nothing was changed; move it away yourself, then run gil install again", folder.display()));
        }
    }
    Ok(())
}

/// The digest a sane, regular `install.json` records. A link, a folder or an odd shape records nothing.
fn recorded_digest(dir: &Path) -> Option<String> {
    let path = dir.join(INSTALL_RECEIPT);
    let metadata = std::fs::symlink_metadata(&path).ok()?;
    if !metadata.is_file() || metadata.len() > 4096 { return None; }
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    if value.get("schema")?.as_u64()? != 1 { return None; }
    let digest = value.get("sha256")?.as_str()?;
    (digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))).then(|| digest.to_owned())
}

/// The digest of the file a previous install placed at `fixed`, when the entry there is ours.
fn ours(dir: &Path, fixed: &Path) -> Option<String> {
    if !std::fs::symlink_metadata(fixed).is_ok_and(|metadata| metadata.is_file()) { return None; }
    let recorded = recorded_digest(dir)?;
    (hex(&Sha256::digest(std::fs::read(fixed).ok()?)) == recorded).then_some(recorded)
}

/// A new name in `dir` that this call creates exclusively: never an existing file, never through a link.
fn write_new(dir: &Path, stem: &str, mode: u32, bytes: &[u8]) -> Result<PathBuf, String> {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let path = dir.join(format!(".{stem}.next-{}-{nanos}", std::process::id()));
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

/// Place `bytes` (this executable, digest `digest`) at `fixed`. `receipt` is the text of the new
/// `install.json`. Returns `OK` when our own file with the same digest is already there.
pub(super) fn place(fixed: &Path, bytes: &[u8], digest: &str, receipt: &str) -> Result<&'static str, String> {
    let dir = fixed.parent().ok_or("fixed path has no parent")?;
    own_folders(dir)?;
    if entry(fixed)?.is_some() {
        match ours(dir, fixed) {
            Some(installed) if installed == digest => return Ok("OK"),
            Some(_) => {}
            None => return Err(format!("{} already exists and is not a file placed by gil install (no matching install.json). Nothing was changed; move it away yourself, then run gil install again", fixed.display())),
        }
    }
    let receipt_path = dir.join(INSTALL_RECEIPT);
    if entry(&receipt_path)?.is_some_and(|metadata| !metadata.is_file()) {
        return Err(format!("{} is not a regular file (a link or a folder). Nothing was changed; move it away yourself, then run gil install again", receipt_path.display()));
    }
    let staged = write_new(dir, "gil", 0o755, bytes)?;
    let staged_receipt = match write_new(dir, INSTALL_RECEIPT, 0o644, receipt.as_bytes()) {
        Ok(path) => path,
        Err(why) => { let _ = std::fs::remove_file(&staged); return Err(why); }
    };
    if let Err(err) = std::fs::rename(&staged, fixed) {
        let _ = std::fs::remove_file(&staged);
        let _ = std::fs::remove_file(&staged_receipt);
        return Err(format!("cannot place {}: {err}. The previous installation was not changed", fixed.display()));
    }
    std::fs::rename(&staged_receipt, &receipt_path).map_err(|err| {
        let _ = std::fs::remove_file(&staged_receipt);
        format!("the executable was placed at {} but {INSTALL_RECEIPT} could not be written: {err}. No Host setting was changed. That file is not recorded as installed; move it away yourself, then run gil install again", fixed.display())
    })?;
    Ok("UPDATED")
}

pub(super) enum Removal { Removed, NotInstalled, Kept }

/// Remove the executable only when it is ours, then its receipt, then the two folders the install
/// made when they are empty. A running executable can be removed on Linux; the process keeps its file.
pub(super) fn remove(fixed: &Path) -> Result<Removal, String> {
    let dir = fixed.parent().ok_or("fixed path has no parent")?;
    if entry(fixed)?.is_none() { return Ok(Removal::NotInstalled); }
    if ours(dir, fixed).is_none() { return Ok(Removal::Kept); }
    std::fs::remove_file(fixed).map_err(|err| format!("cannot remove {}: {err}", fixed.display()))?;
    let _ = std::fs::remove_file(dir.join(INSTALL_RECEIPT));
    let _ = std::fs::remove_dir(dir);
    if let Some(product) = dir.parent() { let _ = std::fs::remove_dir(product); }
    Ok(Removal::Removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::tests::scratch;

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

    #[test]
    fn a_first_install_an_update_and_a_repeat_leave_exactly_the_two_recorded_files() {
        let (root, fixed) = home("place-own");
        assert_eq!(put(&fixed, b"core one").unwrap(), "UPDATED");
        let dir = fixed.parent().unwrap();
        assert_eq!(names(dir), ["gil", "install.json"], "no temporary or parked name stays");
        assert_eq!(std::fs::symlink_metadata(&fixed).unwrap().mode() & 0o777, 0o755);
        assert_eq!(std::fs::symlink_metadata(dir.join(INSTALL_RECEIPT)).unwrap().mode() & 0o777, 0o644);
        // The same bytes again: nothing is rewritten.
        let before = std::fs::symlink_metadata(&fixed).unwrap().ino();
        assert_eq!(put(&fixed, b"core one").unwrap(), "OK");
        assert_eq!(std::fs::symlink_metadata(&fixed).unwrap().ino(), before);
        // Other bytes: our own file is replaced, and the receipt follows it.
        assert_eq!(put(&fixed, b"core two").unwrap(), "UPDATED");
        assert_eq!(std::fs::read(&fixed).unwrap(), b"core two");
        assert_eq!(recorded_digest(dir).unwrap(), digest(b"core two"));
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
        assert_eq!(recorded_digest(dir).unwrap(), digest(b"an older core"));
        assert!(matches!(remove(&fixed).unwrap(), Removal::Kept));
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
        assert!(matches!(remove(&fixed).unwrap(), Removal::Kept));
        let _ = std::fs::remove_dir_all(root);
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
        assert!(matches!(remove(&fixed).unwrap(), Removal::Removed));
        assert_eq!(names(dir), [".gil.next-1-1", "gil.exe.old-1", "install.json.next", "notes.txt"], "only the two recorded files went");
        for name in names(dir) { assert_eq!(std::fs::read(dir.join(name)).unwrap(), b"mine"); }
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"keep");
        assert!(matches!(remove(&fixed).unwrap(), Removal::NotInstalled));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn the_removal_takes_the_emptied_folders_and_a_changed_file_is_kept() {
        let (root, fixed) = home("place-remove");
        assert_eq!(put(&fixed, b"core one").unwrap(), "UPDATED");
        let dir = fixed.parent().unwrap().to_owned();
        // The user replaced the file after the install: it is no longer the recorded one.
        std::fs::write(&fixed, b"edited by the user").unwrap();
        assert!(matches!(remove(&fixed).unwrap(), Removal::Kept));
        assert_eq!(std::fs::read(&fixed).unwrap(), b"edited by the user");
        assert!(dir.join(INSTALL_RECEIPT).exists());
        std::fs::write(&fixed, b"core one").unwrap();
        assert!(matches!(remove(&fixed).unwrap(), Removal::Removed));
        assert!(!dir.exists() && !dir.parent().unwrap().exists(), "the two emptied install folders go");
        assert!(root.join("home/.local/share").is_dir(), "the user's data folder stays");
        let _ = std::fs::remove_dir_all(root);
    }
}
