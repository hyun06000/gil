//! Descriptor-relative, no-follow, no-clobber object publication on Unix.
//! Does not make the external working tree or same-user hostile writers immutable.
use super::*;
use std::ffi::{CStr, CString};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::{ffi::OsStrExt, fs::MetadataExt};

fn boundary() -> io::Error {
    io::Error::other("object storage boundary refused")
}
fn err(source: io::Error) -> ObjectError {
    ObjectError::Io {
        doing: "안전한 객체 저장을 끝내지",
        source: source.to_string(),
    }
}
fn name(value: &str) -> CString {
    CString::new(value).expect("internal name has no NUL")
}
fn open_at(dir: &File, name: &CStr, flags: i32) -> io::Result<File> {
    let fd = unsafe {
        libc::openat(
            dir.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK,
            0o600,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}
fn child(dir: &File, part: &CStr, create: bool) -> io::Result<File> {
    if create {
        let rc = unsafe { libc::mkdirat(dir.as_raw_fd(), part.as_ptr(), 0o700) };
        if rc < 0 {
            let e = io::Error::last_os_error();
            if e.kind() != io::ErrorKind::AlreadyExists {
                return Err(e);
            }
        } else {
            dir.sync_all()?;
        }
    }
    open_at(dir, part, libc::O_RDONLY | libc::O_DIRECTORY)
}
fn anchor(store: &ObjectStore, create: bool) -> io::Result<File> {
    let gil = store.root.parent().ok_or_else(boundary)?;
    if gil.file_name() != Some(std::ffi::OsStr::new(".gil")) {
        return Err(boundary());
    }
    let project = gil.parent().ok_or_else(boundary)?.canonicalize()?;
    let mut dir = File::open("/")?;
    for c in project.components() {
        if let std::path::Component::Normal(c) = c {
            let c = CString::new(c.as_bytes()).map_err(|_| boundary())?;
            dir = child(&dir, &c, false)?;
        }
    }
    dir = child(&dir, &name(".gil"), create)?;
    child(&dir, &name(ARTIFACTS), create)
}
fn same(a: &File, b: &File) -> io::Result<bool> {
    let a = a.metadata()?;
    let b = b.metadata()?;
    Ok(a.dev() == b.dev() && a.ino() == b.ino())
}
fn stable(store: &ObjectStore, root: &File) -> io::Result<()> {
    if same(root, &anchor(store, false)?)? {
        Ok(())
    } else {
        Err(boundary())
    }
}
fn check_parent(root: &File, kind: ObjectKind, hex: &str, parent: &File) -> io::Result<()> {
    let dir = child(root, &name(kind.dir()), false)?;
    let dir = child(&dir, &name("sha256"), false)?;
    let dir = child(&dir, &name(&hex[..SHARD]), false)?;
    if same(&dir, parent)? {
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
        let dir = child(&root, &name(kind.dir()), false)?;
        let dir = child(&dir, &name("sha256"), false)?;
        let dir = child(&dir, &name(&hex[..SHARD]), false)?;
        let file = open_at(&dir, &name(&hex[SHARD..]), libc::O_RDONLY)?;
        if !file.metadata()?.is_file() {
            return Err(boundary());
        }
        stable(store, &root)?;
        check_parent(&root, kind, &hex, &dir)?;
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
struct TempGuard<'a> {
    dir: &'a File,
    name: &'a CStr,
    file: File,
}
impl Drop for TempGuard<'_> {
    fn drop(&mut self) {
        if let Ok(named) = open_at(self.dir, self.name, libc::O_RDONLY) {
            if same(&self.file, &named).unwrap_or(false) {
                unsafe {
                    libc::unlinkat(self.dir.as_raw_fd(), self.name.as_ptr(), 0);
                }
            }
        }
    }
}

pub(super) fn put(
    store: &ObjectStore,
    kind: ObjectKind,
    mut reader: impl Read,
) -> Result<ContentDigest, ObjectError> {
    let root = anchor(store, true).map_err(err)?;
    let tmp = child(&root, &name(TMP), true).map_err(err)?;
    let ticket = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
    let temp = name(&format!("{}-{ticket}", std::process::id()));
    let mut file =
        open_at(&tmp, &temp, libc::O_RDWR | libc::O_CREAT | libc::O_EXCL).map_err(err)?;
    let _cleanup = TempGuard {
        dir: &tmp,
        name: &temp,
        file: file.try_clone().map_err(err)?,
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
    let mut parent = child(&root, &name(kind.dir()), true).map_err(err)?;
    parent = child(&parent, &name("sha256"), true).map_err(err)?;
    parent = child(&parent, &name(&hex[..SHARD]), true).map_err(err)?;
    let target = name(&hex[SHARD..]);
    check_parent(&root, kind, &hex, &parent).map_err(err)?;
    let named = open_at(&tmp, &temp, libc::O_RDONLY).map_err(err)?;
    if !same(&file, &named).map_err(err)? || file.metadata().map_err(err)?.nlink() != 1 {
        return Err(err(boundary()));
    }
    let rc = unsafe {
        libc::linkat(
            tmp.as_raw_fd(),
            temp.as_ptr(),
            parent.as_raw_fd(),
            target.as_ptr(),
            0,
        )
    };
    let linked = rc == 0;
    if !linked {
        let e = io::Error::last_os_error();
        if e.kind() != io::ErrorKind::AlreadyExists {
            return Err(if unsupported(&e) {
                ObjectError::PublishUnsupported {
                    kind,
                    source: e.to_string(),
                }
            } else {
                err(e)
            });
        }
    }
    // Remove only our still-identical temporary entry. unlinkat never follows a symlink.
    let current = open_at(&tmp, &temp, libc::O_RDONLY).map_err(err)?;
    if !same(&file, &current).map_err(err)? {
        return Err(err(boundary()));
    }
    if unsafe { libc::unlinkat(tmp.as_raw_fd(), temp.as_ptr(), 0) } < 0 {
        return Err(err(io::Error::last_os_error()));
    }
    tmp.sync_all()
        .and_then(|()| parent.sync_all())
        .map_err(err)?;
    #[cfg(test)]
    if linked {
        meddle::after_link(&store.object_path(kind, &digest));
    }
    let mut final_file = open_at(&parent, &target, libc::O_RDONLY).map_err(err)?;
    let meta = final_file.metadata().map_err(err)?;
    // Concurrent publishers may temporarily retain a second link; don't reject that valid race.
    if !meta.is_file() {
        return Err(err(boundary()));
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    struct Reader<F>(F);
    impl<F: FnMut(&mut [u8]) -> io::Result<usize>> Read for Reader<F> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            (self.0)(buf)
        }
    }
    fn setup() -> (PathBuf, ObjectStore) {
        let ticket = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "gil-secure-objects-{}-{ticket}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        let store = ObjectStore::at(&root.join(".gil"));
        store.put_blob(&b"seed"[..]).unwrap();
        (root, store)
    }
    #[test]
    fn linked_internal_directories_never_receive_writes() {
        for relative in [
            ".gil",
            ".gil/artifacts",
            ".gil/artifacts/tmp",
            ".gil/artifacts/blobs",
            ".gil/artifacts/blobs/sha256",
        ] {
            let (root, store) = setup();
            let path = root.join(relative);
            let moved = root.join("held");
            fs::rename(&path, &moved).unwrap();
            symlink(&moved, &path).unwrap();
            let before: Vec<_> = fs::read_dir(&moved)
                .unwrap()
                .map(|v| v.unwrap().file_name())
                .collect();
            assert!(store.put_blob(&b"new bytes"[..]).is_err(), "{relative}");
            let after: Vec<_> = fs::read_dir(&moved)
                .unwrap()
                .map(|v| v.unwrap().file_name())
                .collect();
            assert_eq!(before, after);
            if !relative.ends_with("/tmp") {
                let mut output = Vec::new();
                assert!(
                    store
                        .read_blob(&codec::address(b"seed"), &mut output)
                        .is_err()
                );
                assert!(output.is_empty());
            }
            fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn linked_final_object_is_not_followed_or_overwritten() {
        let (root, store) = setup();
        let bytes = b"seed";
        let digest = codec::address(bytes);
        let at = store.object_path(ObjectKind::Blob, &digest);
        let victim = root.join("victim");
        fs::write(&victim, bytes).unwrap();
        fs::remove_file(&at).unwrap();
        symlink(&victim, &at).unwrap();
        assert!(store.put_blob(&bytes[..]).is_err());
        let mut output = Vec::new();
        assert!(store.read_blob(&digest, &mut output).is_err());
        assert!(output.is_empty());
        assert_eq!(fs::read(&victim).unwrap(), bytes);
        assert!(fs::symlink_metadata(&at).unwrap().file_type().is_symlink());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn directory_swap_during_source_read_cannot_redirect_publication() {
        let (root, store) = setup();
        let moved = root.join("held");
        let mut first = true;
        let reader = Reader(|buffer: &mut [u8]| {
            if !first {
                return Ok(0);
            }
            first = false;
            fs::rename(root.join(".gil/artifacts"), &moved)?;
            symlink(&moved, root.join(".gil/artifacts"))?;
            buffer[0] = 1;
            Ok(1)
        });
        assert!(store.put_blob(reader).is_err());
        assert!(
            !store
                .object_path(ObjectKind::Blob, &codec::address(&[1]))
                .exists()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn replacing_temp_name_never_writes_or_deletes_the_replacement() {
        let (root, store) = setup();
        let victim = root.join("keep");
        fs::write(&victim, b"untouched").unwrap();
        let mut first = true;
        let mut replaced = None;
        let reader = Reader(|buffer: &mut [u8]| {
            if !first {
                return Ok(0);
            }
            first = false;
            let temp = fs::read_dir(store.tmp())?.next().unwrap()?.path();
            fs::remove_file(&temp)?;
            symlink(&victim, &temp)?;
            replaced = Some(temp);
            buffer[0] = 42;
            Ok(1)
        });
        assert!(store.put_blob(reader).is_err());
        assert_eq!(fs::read(victim).unwrap(), b"untouched");
        assert!(
            fs::symlink_metadata(replaced.unwrap())
                .unwrap()
                .file_type()
                .is_symlink()
        );
        fs::remove_dir_all(root).unwrap();
    }
}
