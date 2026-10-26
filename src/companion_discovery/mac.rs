use super::*;
use std::io::Write;
use std::os::{
    fd::AsRawFd,
    unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
};

pub(super) fn default_root() -> io::Result<PathBuf> {
    Ok(PathBuf::from(format!("/private/tmp/gil-cs-{}", unsafe {
        libc::geteuid()
    })))
}
fn check_directory(path: &Path) -> io::Result<()> {
    if !path.is_absolute() || path.canonicalize()? != path {
        return Err(invalid("noncanonical coordination directory"));
    }
    let m = std::fs::symlink_metadata(path)?;
    if !m.is_dir() || m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o777 != 0o700 {
        return Err(invalid("coordination directory protection refused"));
    }
    Ok(())
}
fn file(path: &Path, create: bool) -> io::Result<File> {
    let open = || {
        let mut options = std::fs::OpenOptions::new();
        options
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        options
    };
    let result = if create {
        match open().create_new(true).mode(0o600).open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => open().open(path)?,
            Err(e) => return Err(e),
        }
    } else {
        open().open(path)?
    };
    let m = result.metadata()?;
    if !m.is_file()
        || m.uid() != unsafe { libc::geteuid() }
        || m.mode() & 0o777 != 0o600
        || m.nlink() != 1
    {
        return Err(invalid("coordination file protection refused"));
    }
    Ok(result)
}
pub(super) struct Guard {
    root: PathBuf,
    _lock: File,
}
impl Guard {
    pub(super) fn acquire(root: &Path) -> io::Result<Self> {
        match std::fs::DirBuilder::new().mode(0o700).create(root) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
        check_directory(root)?;
        let lock = file(&root.join("launch.lock"), true)?;
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            root: root.into(),
            _lock: lock,
        })
    }
    pub(super) fn existing(&self) -> io::Result<Option<PathBuf>> {
        let file = match file(&self.root.join("current"), false) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let mut generation = String::new();
        file.take(33).read_to_string(&mut generation)?;
        if generation.len() != 32 || !generation.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(invalid("invalid rendezvous generation"));
        }
        Ok(Some(self.root.join(generation).join("s")))
    }
    pub(super) fn next(&self) -> io::Result<PathBuf> {
        check_directory(&self.root)?;
        let generation = crate::companion_ipc::random_nonce()?[..32].to_string();
        let directory = self.root.join(&generation);
        std::fs::DirBuilder::new().mode(0o700).create(&directory)?;
        let endpoint = directory.join("s");
        use std::os::unix::ffi::OsStrExt;
        if endpoint.as_os_str().as_bytes().len() >= 104 {
            return Err(invalid("rendezvous socket path too long"));
        }
        let mut current = file(&self.root.join("current"), true)?;
        // All readers hold launch.lock; a partial/crashed write is Invalid, never guessed/repaired.
        current.write_all(generation.as_bytes())?;
        current.set_len(32)?;
        current.sync_all()?;
        Ok(endpoint)
    }
}
