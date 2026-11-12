//! Reading and replacing a Host settings file the way a careful person would: read it whole,
//! keep a backup beside it, write the new content to a temporary file, check that nobody
//! changed the original in the meantime, then rename. A GIL-only lock serializes GIL writers;
//! other editors are expected to be closed.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

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
    let lock = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false)
        .open(dir.join(format!("{name}.gil-lock")))
        .map_err(|err| format!("cannot open the {what} lock: {err}"))?;
    lock.try_lock().map_err(|_| format!("another GIL registration of {what} is in progress; retry when it has finished"))?;
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
