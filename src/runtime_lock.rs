//! Runtime update — the **selection lock** (`selection.lock` in the store root): an OS
//! exclusive lock (Unix `flock(LOCK_EX)`, Windows `LockFileEx(EXCLUSIVE)`) held by the
//! selection transition and by the prepared-record update of the preparation layer, so that
//! `store.json` is never replaced by two writers at once and a history update cannot be
//! overwritten by a late `initializing → prepared` record. The lock file is opened or created
//! relative to the held root handle (no path re-resolution, no links), must satisfy the data
//! file protection, and is **never deleted** by any layer. An existing lock file is only
//! opened and verified (type, owner, mode / DACL, single link) — never chmod'ed or otherwise
//! modified; only a lock file this call creates gets its mode set. The lock is released when
//! the handle is dropped (and by the OS if the process dies).
use crate::runtime_observe::platform;
use std::fs::File;
use std::io;

pub const LOCK_FILE: &str = "selection.lock";

/// Held while in scope; releasing is dropping.
#[derive(Debug)]
pub struct SelectionLock { _file: File }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wait { NonBlocking, Blocking }

/// Acquire the lock. `NonBlocking` fails with `WouldBlock` when another holder exists.
pub fn acquire(root: &platform::Dir, wait: Wait) -> io::Result<SelectionLock> {
    let file = lfs::open_lock(root)?;
    let stamp = platform::stamp_file(&file)?;
    platform::check_data(&file, &stamp).map_err(|e| io::Error::other(format!("{LOCK_FILE} protection: {e}")))?;
    lfs::lock(&file, wait)?;
    Ok(SelectionLock { _file: file })
}

#[cfg(unix)]
mod lfs {
    use super::Wait;
    use crate::runtime_observe::platform::Dir;
    use std::fs::File;
    use std::io;
    use std::os::fd::{AsRawFd, FromRawFd};

    /// Exclusive creation (mode set by this call) or, when it already exists, a plain open of
    /// the existing file without following links and without changing anything about it.
    pub fn open_lock(root: &Dir) -> io::Result<File> {
        let fd = unsafe { libc::openat(root.as_file().as_raw_fd(), c"selection.lock".as_ptr(), libc::O_RDWR | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC, 0o600) };
        if fd >= 0 {
            let file = unsafe { File::from_raw_fd(fd) };
            crate::internal_fs::chmod(&file, 0o600)?;
            return Ok(file);
        }
        let e = io::Error::last_os_error();
        if e.kind() != io::ErrorKind::AlreadyExists { return Err(e); }
        let fd = unsafe { libc::openat(root.as_file().as_raw_fd(), c"selection.lock".as_ptr(), libc::O_RDWR | libc::O_NOFOLLOW | libc::O_CLOEXEC) };
        if fd < 0 { return Err(io::Error::last_os_error()); }
        let file = unsafe { File::from_raw_fd(fd) };
        if !file.metadata()?.is_file() { return Err(io::Error::other("selection.lock is not a regular file")); }
        Ok(file)
    }
    pub fn lock(file: &File, wait: Wait) -> io::Result<()> {
        let op = libc::LOCK_EX | if wait == Wait::NonBlocking { libc::LOCK_NB } else { 0 };
        if unsafe { libc::flock(file.as_raw_fd(), op) } < 0 { return Err(io::Error::last_os_error()); }
        Ok(())
    }
}

#[cfg(windows)]
mod lfs {
    use super::Wait;
    use crate::runtime_observe::platform::Dir;
    use crate::windows_fs as win;
    use std::ffi::OsStr;
    use std::fs::File;
    use std::io;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::GENERIC_WRITE;
    use windows_sys::Win32::Storage::FileSystem::{LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY, LockFileEx};
    use windows_sys::Win32::System::IO::OVERLAPPED;

    /// Exclusive creation (DACL inherited) or a plain open of the existing file; nothing about an
    /// existing file is changed.
    pub fn open_lock(root: &Dir) -> io::Result<File> {
        match win::create_file_at(root.as_file(), OsStr::new("selection.lock"), win::READ | GENERIC_WRITE, win::SHARE_ALL) {
            Ok(f) => Ok(f),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => win::open_entry_at(root.as_file(), OsStr::new("selection.lock"), win::READ | GENERIC_WRITE, win::SHARE_ALL, Some(false)),
            Err(e) => Err(e),
        }
    }
    pub fn lock(file: &File, wait: Wait) -> io::Result<()> {
        let flags = LOCKFILE_EXCLUSIVE_LOCK | if wait == Wait::NonBlocking { LOCKFILE_FAIL_IMMEDIATELY } else { 0 };
        let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
        if unsafe { LockFileEx(file.as_raw_handle(), flags, 0, 1, 0, &mut overlapped) } == 0 {
            let e = io::Error::last_os_error();
            // ERROR_LOCK_VIOLATION (33) / ERROR_IO_PENDING (997) both mean "held elsewhere" here.
            return Err(if matches!(e.raw_os_error(), Some(33) | Some(997)) { io::Error::new(io::ErrorKind::WouldBlock, "selection.lock is held by another process or thread") } else { e });
        }
        Ok(())
    }
}
