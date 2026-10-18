//! Pinned-handle, no-reparse, no-clobber object publication on Windows (Windows contract §4).
//! Does not make the external working tree or same-user hostile writers immutable.
use super::*;
use crate::windows_fs as win;
use std::ffi::OsStr;
use windows_sys::Win32::Storage::FileSystem::{CREATE_NEW, OPEN_EXISTING};

fn boundary() -> io::Error {
    io::Error::other("object storage boundary refused")
}
fn err(source: io::Error) -> ObjectError {
    ObjectError::Io {
        doing: "안전한 객체 저장을 끝내지",
        source: source.to_string(),
    }
}
/// `<project>/.gil/artifacts` with every level opened as itself and pinned.
fn anchor(store: &ObjectStore, create: bool) -> io::Result<win::Chain> {
    let gil = store.root.parent().ok_or_else(boundary)?;
    if gil.file_name() != Some(OsStr::new(".gil")) {
        return Err(boundary());
    }
    let project = gil.parent().ok_or_else(boundary)?.canonicalize()?;
    let mut chain = win::Chain::open(&project)?;
    chain.push(OsStr::new(".gil"), create)?;
    chain.push(OsStr::new(ARTIFACTS), create)?;
    Ok(chain)
}
/// `.gil` (one level above the pinned `artifacts`) is owner-only: that is what marks an image
/// Project's storage, and only there are the child-entry ACL checks applied (contract §8 1a).
fn owner_only_regime(root: &win::Chain) -> bool {
    root.above(1).is_some_and(|gil| win::private_dir(gil).is_ok())
}
fn stable(store: &ObjectStore, root: &win::Chain) -> io::Result<()> {
    if win::id(anchor(store, false)?.dir())? == win::id(root.dir())? {
        Ok(())
    } else {
        Err(boundary())
    }
}
/// `<kind>/sha256/<shard>` under the pinned anchor, re-walked from the volume root.
fn parent_of(root: &win::Chain, kind: ObjectKind, hex: &str, create: bool) -> io::Result<win::Chain> {
    let mut chain = win::Chain::open(root.path())?;
    if win::id(chain.dir())? != win::id(root.dir())? {
        return Err(boundary());
    }
    chain.push(OsStr::new(kind.dir()), create)?;
    chain.push(OsStr::new("sha256"), create)?;
    chain.push(OsStr::new(&hex[..SHARD]), create)?;
    Ok(chain)
}
fn check_parent(root: &win::Chain, kind: ObjectKind, hex: &str, parent: &win::Chain) -> io::Result<()> {
    if win::id(parent_of(root, kind, hex, false)?.dir())? == win::id(parent.dir())? {
        Ok(())
    } else {
        Err(boundary())
    }
}

pub(super) fn read_object(
    store: &ObjectStore,
    kind: ObjectKind,
    digest: &ContentDigest,
) -> Result<File, ObjectError> {
    let opened = (|| -> io::Result<File> {
        let root = anchor(store, false)?;
        let hex = digest.hex();
        let parent = parent_of(&root, kind, &hex, false)?;
        let at = parent.child(OsStr::new(&hex[SHARD..]))?;
        // The entry itself: a directory, reparse point or link is refused, never followed.
        let file = win::open_file(&at, win::READ, win::SHARE_ALL, OPEN_EXISTING)?;
        if !win::Stamp::of(&file)?.is_single_regular() {
            return Err(boundary());
        }
        // Owner-only regime (image Projects, whose .gil is owner-only): a widened object or shard
        // directory is refused when read. Other storages keep their inherited ACLs untouched.
        if owner_only_regime(&root) {
            win::inherited_only(&file, false)?;
            win::inherited_only(parent.dir(), true)?;
        }
        stable(store, &root)?;
        check_parent(&root, kind, &hex, &parent)?;
        Ok(file)
    })();
    opened.map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            ObjectError::Missing {
                kind,
                address: digest.hex(),
            }
        } else {
            err(e)
        }
    })
}

/// Deletes only the temporary entry this handle was created through, never a replacement.
struct TempGuard {
    file: File,
    armed: bool,
}
impl Drop for TempGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = win::delete(&self.file);
        }
    }
}

pub(super) fn put(
    store: &ObjectStore,
    kind: ObjectKind,
    mut reader: impl Read,
) -> Result<ContentDigest, ObjectError> {
    let root = anchor(store, true).map_err(err)?;
    let mut tmp = win::Chain::open(root.path()).map_err(err)?;
    if win::id(tmp.dir()).map_err(err)? != win::id(root.dir()).map_err(err)? {
        return Err(err(boundary()));
    }
    tmp.push(OsStr::new(TMP), true).map_err(err)?;
    let ticket = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
    let temp_name = format!("{}-{ticket}", std::process::id());
    let temp_path = tmp.child(OsStr::new(&temp_name)).map_err(err)?;
    let mut file =
        win::open_file(&temp_path, win::READ_WRITE_DELETE, win::SHARE_ALL, CREATE_NEW).map_err(err)?;
    let mut cleanup = TempGuard {
        file: file.try_clone().map_err(err)?,
        armed: true,
    };
    let mut hasher = Sha256::new();
    let mut chunk = vec![0; CHUNK];
    loop {
        let n = reader.read(&mut chunk).map_err(err)?;
        if n == 0 {
            break;
        }
        hasher.update(&chunk[..n]);
        file.write_all(&chunk[..n]).map_err(err)?;
    }
    file.flush().and_then(|()| file.sync_all()).map_err(err)?;
    let digest = ContentDigest::from_raw(DigestAlgorithm::Sha256, hasher.finalize().into());
    stable(store, &root).map_err(err)?;
    let hex = digest.hex();
    let parent = parent_of(&root, kind, &hex, true).map_err(err)?;
    let target = parent.child(OsStr::new(&hex[SHARD..])).map_err(err)?;
    let named = win::open_file(&temp_path, win::READ, win::SHARE_ALL, OPEN_EXISTING).map_err(err)?;
    let (held, current) = (
        win::Stamp::of(&file).map_err(err)?,
        win::Stamp::of(&named).map_err(err)?,
    );
    if held.id != current.id || !held.is_single_regular() || !current.is_single_regular() {
        return Err(err(boundary()));
    }
    drop(named);
    // No-clobber: an existing entry of any kind at the address makes the link fail.
    let linked = match win::hard_link(&temp_path, &target) {
        Ok(()) => true,
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => false,
        Err(e) if win::links_unsupported(&e) => {
            return Err(ObjectError::PublishUnsupported {
                kind,
                source: e.to_string(),
            });
        }
        Err(e) => return Err(err(e)),
    };
    // Remove only our temporary entry, through the handle we created it with.
    win::delete(&file).map_err(err)?;
    cleanup.armed = false;
    drop(file);
    win::flush_dir(tmp.path(), tmp.dir())
        .and_then(|()| win::flush_dir(parent.path(), parent.dir()))
        .map_err(err)?;
    #[cfg(test)]
    if linked {
        meddle::after_link(&store.object_path(kind, &digest));
    }
    #[cfg(not(test))]
    let _ = linked;
    let mut final_file = win::open_file(&target, win::READ, win::SHARE_ALL, OPEN_EXISTING).map_err(err)?;
    // Under the owner-only regime the published object must carry only the inherited ACEs.
    if owner_only_regime(&root) {
        win::inherited_only(&final_file, false).map_err(err)?;
    }
    // Concurrent publishers may temporarily retain a second link; don't reject that valid race.
    let mut hash = Sha256::new();
    loop {
        let n = final_file.read(&mut chunk).map_err(err)?;
        if n == 0 {
            break;
        }
        hash.update(&chunk[..n]);
    }
    let found = ContentDigest::from_raw(DigestAlgorithm::Sha256, hash.finalize().into());
    if found != digest {
        return Err(ObjectError::Corrupt {
            kind,
            address: hex,
            found: found.hex(),
        });
    }
    stable(store, &root).map_err(err)?;
    check_parent(&root, kind, &hex, &parent).map_err(err)?;
    Ok(digest)
}
