//! Unix restore destinations: retain directory handles rather than follow mutable pathnames.
use super::*;
use crate::artifact::EntryPath;
use crate::internal_fs as guarded;
use std::ffi::CString;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::PermissionsExt;

fn refused() -> io::Error {
    io::Error::other("rollback destination boundary refused")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::fs::symlink;

    #[test]
    fn publishing_refuses_a_parent_swapped_during_the_copy() {
        let root = std::env::temp_dir().join(format!(
            "gil-forward-swap-{}-{}",
            std::process::id(),
            NEXT_TICKET.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("nested")).unwrap();
        fs::create_dir(root.join("outside")).unwrap();
        fs::write(root.join("nested/file"), b"before").unwrap();
        fs::write(root.join("outside/file"), b"sentinel").unwrap();
        let world = World::open(&root).unwrap();
        let path = EntryPath::parse("nested/file").unwrap();
        let target = digest_of(&b"after"[..]).unwrap();
        let result = world.publish(&path, Some(0o640), &target, &mut vec![], |sink| {
            fs::rename(root.join("nested"), root.join("held"))?;
            symlink(root.join("outside"), root.join("nested"))?;
            sink.write_all(b"after")
        });
        assert!(result.is_err());
        assert_eq!(fs::read(root.join("outside/file")).unwrap(), b"sentinel");
        assert_eq!(fs::read(root.join("held/file")).unwrap(), b"before");
        assert!(world.digest(&path).is_err());
        assert!(world.remove(&path).is_err());
        assert_eq!(fs::read(root.join("outside/file")).unwrap(), b"sentinel");
    }

    #[test]
    fn publication_records_only_directories_it_created() {
        let root = std::env::temp_dir().join(format!(
            "gil-forward-mkdir-{}-{}",
            std::process::id(),
            NEXT_TICKET.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("existing")).unwrap();
        let world = World::open(&root).unwrap();
        let path = EntryPath::parse("existing/new/deep/file").unwrap();
        let target = digest_of(&b"payload"[..]).unwrap();
        let mut made = vec![];
        world
            .publish(&path, None, &target, &mut made, |sink| {
                sink.write_all(b"payload")
            })
            .unwrap();
        assert_eq!(
            made,
            vec![root.join("existing/new"), root.join("existing/new/deep")]
        );
        assert_eq!(world.digest(&path).unwrap(), Some(target));
    }
}
pub(super) struct World {
    root: PathBuf,
    reported_root: PathBuf,
    held: File,
}
impl World {
    pub fn open(root: &Path) -> io::Result<Self> {
        let reported_root = root.to_path_buf();
        let root = root.canonicalize()?;
        Ok(Self {
            held: guarded::project_directory(&root)?,
            root,
            reported_root,
        })
    }
    fn parent(&self, path: &EntryPath, create: bool) -> io::Result<(File, CString)> {
        self.parent_tracking(path, create, &mut Vec::new())
    }
    fn parent_tracking(
        &self,
        path: &EntryPath,
        create: bool,
        made: &mut Vec<PathBuf>,
    ) -> io::Result<(File, CString)> {
        if !guarded::same(&self.held, &guarded::project_directory(&self.root)?)? {
            return Err(refused());
        }
        let mut parts: Vec<_> = path.as_str().split('/').collect();
        let leaf = CString::new(parts.pop().ok_or_else(refused)?)?;
        let mut dir = guarded::child_dir(&self.held, c".")?;
        // Rollback's made-directory list uses the caller's root spelling.
        // Keep aliases such as /var -> /private/var consistent with that list.
        let mut at = self.reported_root.clone();
        for part in parts {
            at.push(part);
            let name = CString::new(part)?;
            if create {
                if unsafe { libc::mkdirat(dir.as_raw_fd(), name.as_ptr(), 0o777) } < 0 {
                    let e = io::Error::last_os_error();
                    if e.kind() != io::ErrorKind::AlreadyExists {
                        return Err(e);
                    }
                } else {
                    made.push(at.clone());
                    dir.sync_all()?;
                }
            }
            dir = guarded::child_dir(&dir, &name)?;
        }
        Ok((dir, leaf))
    }
    fn still_named(&self, path: &EntryPath, dir: &File) -> io::Result<()> {
        if guarded::same(dir, &self.parent(path, false)?.0)? {
            Ok(())
        } else {
            Err(refused())
        }
    }
    pub fn check(&self, path: &EntryPath) -> io::Result<()> {
        let result = self
            .parent(path, false)
            .and_then(|(dir, leaf)| guarded::regular_at(&dir, &leaf));
        match result {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }
    pub fn digest(&self, path: &EntryPath) -> io::Result<Option<ContentDigest>> {
        let result: io::Result<ContentDigest> = (|| {
            let (dir, leaf) = self.parent(path, false)?;
            let file = guarded::regular_at(&dir, &leaf)?;
            let digest = digest_of(file)?;
            self.still_named(path, &dir)?;
            Ok(digest)
        })();
        match result {
            Ok(digest) => Ok(Some(digest)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }
    pub fn remove(&self, path: &EntryPath) -> io::Result<()> {
        let (dir, leaf) = match self.parent(path, false) {
            Ok(pair) => pair,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e),
        };
        match guarded::regular_at(&dir, &leaf) {
            Ok(_) => (),
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e),
        }
        self.still_named(path, &dir)?;
        guarded::unlink_at(&dir, &leaf)?;
        dir.sync_all()?;
        self.still_named(path, &dir)
    }
    pub fn restore(
        &self,
        source: &mut File,
        path: &EntryPath,
        mode: u32,
        expected: &ContentDigest,
    ) -> io::Result<()> {
        self.publish(path, Some(mode), expected, &mut Vec::new(), |sink| {
            io::copy(source, sink).map(|_| ())
        })
    }
    pub fn publish(
        &self,
        path: &EntryPath,
        mode: Option<u32>,
        expected: &ContentDigest,
        made: &mut Vec<PathBuf>,
        write: impl FnOnce(&mut File) -> io::Result<()>,
    ) -> io::Result<()> {
        self.check(path)?;
        let (dir, leaf) = self.parent_tracking(path, true, made)?;
        let temp = CString::new(format!(
            "{}.gil-restore-{}-{}",
            path.as_str().rsplit('/').next().unwrap(),
            std::process::id(),
            NEXT_TICKET.fetch_add(1, Ordering::Relaxed)
        ))?;
        let fd = unsafe {
            libc::openat(
                dir.as_raw_fd(),
                temp.as_ptr(),
                libc::O_RDWR | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                if mode.is_some() { 0o600 } else { 0o666 },
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut sink = unsafe { File::from_raw_fd(fd) };
        write(&mut sink)?;
        verify_backup(&mut sink, expected)?;
        let named = guarded::regular_at(&dir, &temp)?;
        if !guarded::same(&sink, &named)? {
            return Err(refused());
        }
        // Validate while readable, then preserve even a write-only original mode.
        // Set permissions on the held file, never on a re-resolved temporary pathname.
        if let Some(mode) = mode {
            sink.set_permissions(fs::Permissions::from_mode(mode))?;
        }
        sink.sync_all()?;
        self.still_named(path, &dir)?;
        if unsafe {
            libc::renameat(
                dir.as_raw_fd(),
                temp.as_ptr(),
                dir.as_raw_fd(),
                leaf.as_ptr(),
            )
        } < 0
        {
            return Err(io::Error::last_os_error());
        }
        dir.sync_all()?;
        self.still_named(path, &dir)
    }
    pub fn remove_empty_dir(&self, path: &EntryPath) -> io::Result<()> {
        let (dir, leaf) = self.parent(path, false)?;
        guarded::child_dir(&dir, &leaf)?;
        self.still_named(path, &dir)?;
        if unsafe { libc::unlinkat(dir.as_raw_fd(), leaf.as_ptr(), libc::AT_REMOVEDIR) } < 0 {
            return Err(io::Error::last_os_error());
        }
        dir.sync_all()
    }
}
