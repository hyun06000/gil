//! Reading and replacing a Host settings file the way a careful person would: read it whole,
//! keep a backup beside it, write the new content to a temporary file, check that nobody
//! changed the original in the meantime, then rename. A GIL-only lock serializes GIL writers;
//! other editors are expected to be closed.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[cfg(test)]
mod lock_probe;

const KEEP_BACKUPS: usize = 5;

/// What `load` read: the exact bytes, whether the file existed, and their digest.
#[derive(Debug)]
pub(crate) struct Loaded {
    pub bytes: Vec<u8>,
    pub existed: bool,
    pub digest: Option<[u8; 32]>,
}

fn digest_of(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// Read a settings file. A missing file is an empty, non-existing one; a symlink, a non-file
/// or an oversized file is refused so that nothing unexpected is ever rewritten.
pub(crate) fn load(path: &Path, max_bytes: u64, what: &str) -> Result<Loaded, String> {
    let parent = path.parent().ok_or_else(|| format!("{what} path has no folder"))?;
    if !parent.is_dir() {
        return Err(format!("{what} folder does not exist: {}", parent.display()));
    }
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Loaded { bytes: Vec::new(), existed: false, digest: None });
        }
        Err(err) => return Err(format!("cannot read {what}: {err}")),
    };
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(format!("{what} is not a regular file; left untouched"));
    }
    if metadata.len() > max_bytes {
        return Err(format!("{what} is larger than expected; left untouched"));
    }
    let bytes = std::fs::read(path).map_err(|err| format!("cannot read {what}: {err}"))?;
    let digest = Some(digest_of(&bytes));
    Ok(Loaded { bytes, existed: true, digest })
}

/// Text of a settings file as UTF-8, tolerating a BOM.
pub(crate) fn text_of<'a>(bytes: &'a [u8], what: &str) -> Result<&'a str, String> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    std::str::from_utf8(bytes).map_err(|_| format!("{what} is not UTF-8 text; left untouched"))
}

/// Replace `path` with `text`, keeping a backup of the bytes read by `loaded`.
pub(crate) fn store(path: &Path, loaded: &Loaded, text: &str, what: &str) -> Result<(), String> {
    let dir = path.parent().ok_or_else(|| format!("{what} path has no folder"))?;
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or_else(|| format!("{what} path has no file name"))?;

    // Keep the lock file in place: unlinking a held lock would let another writer lock a new inode.
    let lock_path = dir.join(format!("{name}.gil-lock"));
    let lock = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false)
        .open(&lock_path)
        .map_err(|err| format!("cannot open the {what} lock: {err}"))?;
    let attempt = lock.try_lock();
    // Test builds only: note the error as returned before it is replaced by the message below.
    #[cfg(test)]
    lock_probe::observe(&lock_path, &attempt);
    attempt.map_err(|_| format!("another GIL registration of {what} is in progress; retry when it has finished"))?;
    ensure_unchanged(path, loaded, what)?;

    let id = format!("{}-{}", now_unix(), std::process::id());
    if loaded.existed {
        write_new_synced(&dir.join(format!("{name}.bak-{id}")), &loaded.bytes, what)?;
    }
    let temporary = dir.join(format!("{name}.gil-next-{id}"));
    write_new_synced(&temporary, text.as_bytes(), what)?;
    if let Err(err) = ensure_unchanged(path, loaded, what) {
        let _ = std::fs::remove_file(&temporary);
        return Err(err);
    }
    if let Err(err) = std::fs::rename(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(format!("cannot replace {what}: {err}"));
    }
    prune_backups(dir, &name);
    Ok(())
}

fn ensure_unchanged(path: &Path, loaded: &Loaded, what: &str) -> Result<(), String> {
    let now = load(path, u64::MAX, what)?;
    if now.existed != loaded.existed || now.digest != loaded.digest {
        return Err(format!("{what} changed after it was read; nothing was written. Finish editing it, then retry"));
    }
    Ok(())
}

fn write_new_synced(path: &Path, bytes: &[u8], what: &str) -> Result<(), String> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|err| format!("cannot create a {what} backup/temporary file: {err}"))?;
    if let Err(err) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(format!("cannot write a {what} backup/temporary file: {err}"));
    }
    Ok(())
}

fn prune_backups(dir: &Path, name: &str) {
    let prefix = format!("{name}.bak-");
    let mut backups: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.file_name().is_some_and(|file| {
            let file = file.to_string_lossy();
            file.strip_prefix(&prefix).is_some_and(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit() || b == b'-'))
        }))
        .collect();
    backups.sort();
    while backups.len() > KEEP_BACKUPS {
        let _ = std::fs::remove_file(backups.remove(0));
    }
}

fn now_unix() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_backs_up_replaces_and_refuses_a_concurrent_change() {
        let dir = super::super::tests::scratch("file");
        let path = dir.join("settings.json");
        std::fs::write(&path, b"{\"a\":1}").unwrap();
        let loaded = load(&path, 1024, "settings").unwrap();
        store(&path, &loaded, "{\"a\":2}\n", "settings").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"a\":2}\n");
        let backups: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with("settings.json.bak-")).collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(std::fs::read(backups[0].path()).unwrap(), b"{\"a\":1}");

        // A stale `loaded` must not overwrite a newer file.
        std::fs::write(&path, b"{\"a\":3}").unwrap();
        let err = store(&path, &loaded, "{\"a\":4}\n", "settings").unwrap_err();
        assert!(err.contains("changed after it was read"), "{err}");
        assert_eq!(std::fs::read(&path).unwrap(), b"{\"a\":3}");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Control for the test-only lock probe, not a reproduction of any past intermittent failure:
    /// a second open of the same lock file holds the lock, so `store`'s `try_lock` must fail.
    /// Both opens are in this thread, so no timing or synchronization is involved.
    #[test]
    fn lock_probe_records_the_returned_error_and_store_keeps_its_message() {
        let dir = super::super::tests::scratch("lock-probe");
        let path = dir.join("settings.json");
        std::fs::write(&path, b"{\"a\":1}").unwrap();
        let loaded = load(&path, 1024, "settings").unwrap();
        let lock_path = dir.join("settings.json.gil-lock");
        let holder = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false)
            .open(&lock_path).unwrap();
        holder.try_lock().unwrap();

        let err = store(&path, &loaded, "{\"a\":2}\n", "settings").unwrap_err();
        assert_eq!(err, "another GIL registration of settings is in progress; retry when it has finished");
        assert_eq!(std::fs::read(&path).unwrap(), b"{\"a\":1}");
        let names: Vec<String> = std::fs::read_dir(&dir).unwrap().filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert!(names.iter().all(|n| n == "settings.json" || n == "settings.json.gil-lock"), "{names:?}");

        let lock = lock_probe::relative_to_temp(&lock_path);
        assert!(lock.starts_with("$TMPDIR/gil-install-lock-probe-"), "{lock}");
        let seen: Vec<_> = lock_probe::failures().into_iter().filter(|r| r.lock == lock).collect();
        assert_eq!(seen.len(), 1, "{seen:?}");
        let record = &seen[0];
        assert_eq!(record.variant, "WouldBlock", "{record:?}");
        // Observed with Rust 1.97.1: this variant's Debug is the quoted word, so `variant` is kept apart.
        assert_eq!(record.debug, "\"WouldBlock\"");
        assert_eq!(record.kind, None);
        assert_eq!(record.raw_os_error, None);
        assert_eq!(record.pid, std::process::id());
        assert_eq!(record.thread_attempt, 1);
        assert_eq!(record.since_previous_on_thread_us, None);
        let line = lock_probe::render(record);
        assert!(line.contains("raw_os_error=not provided (TryLockError::WouldBlock carries no io::Error)"), "{line}");
        assert!(line.contains("RUST_TEST_THREADS="), "{line}");
        assert!(!line.contains(&std::env::temp_dir().display().to_string()), "{line}");
        drop(holder);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// The probe's formatting of the other variant, from a constructed value (not an OS result).
    #[test]
    fn lock_probe_keeps_the_inner_io_error_of_the_error_variant() {
        let error = std::fs::TryLockError::Error(std::io::Error::from_raw_os_error(5));
        let record = lock_probe::describe(&error);
        assert_eq!(record.variant, "Error");
        assert_eq!(record.raw_os_error, Some(5));
        assert!(record.kind.is_some());
        // Observed with Rust 1.97.1: this variant's Debug is the inner io::Error's, without a wrapper.
        assert!(record.debug.starts_with("Os { code: 5"), "{}", record.debug);
        let line = lock_probe::render(&record);
        assert!(line.contains("raw_os_error=5"), "{line}");
    }

    #[test]
    fn load_refuses_symlinks_and_oversized_files() {
        let dir = super::super::tests::scratch("load");
        let path = dir.join("big.json");
        std::fs::write(&path, vec![b' '; 10]).unwrap();
        assert!(load(&path, 5, "big").unwrap_err().contains("larger"));
        #[cfg(unix)]
        {
            let link = dir.join("link.json");
            std::os::unix::fs::symlink(&path, &link).unwrap();
            assert!(load(&link, 1024, "link").unwrap_err().contains("not a regular file"));
        }
        assert!(!load(&dir.join("missing.json"), 1024, "missing").unwrap().existed);
        assert!(load(&dir.join("nope").join("x.json"), 1024, "x").unwrap_err().contains("does not exist"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn text_of_strips_a_bom() {
        assert_eq!(text_of(b"\xEF\xBB\xBF{}", "x").unwrap(), "{}");
        assert!(text_of(&[0xff, 0xfe], "x").is_err());
    }
}
