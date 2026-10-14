//! Unix entry checks for .gil control files, before parsing or recovery.
use std::ffi::{CStr, CString};
use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::{ffi::OsStrExt, fs::MetadataExt};
use std::path::{Component, Path};

fn refused() -> io::Error {
    io::Error::other("internal Project path boundary refused")
}
fn open(dir: &File, name: &CStr, flags: i32) -> io::Result<File> {
    let fd = unsafe {
        libc::openat(
            dir.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
            0o600,
        )
    };
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}
pub(crate) fn child_dir(dir: &File, name: &CStr) -> io::Result<File> {
    open(dir, name, libc::O_RDONLY | libc::O_DIRECTORY)
}
pub(crate) fn regular_at(dir: &File, name: &CStr) -> io::Result<File> {
    let file = open(dir, name, libc::O_RDONLY)?;
    if !file.metadata()?.is_file() {
        return Err(refused());
    }
    Ok(file)
}
pub(crate) fn unlink_at(dir: &File, name: &CStr) -> io::Result<()> {
    if unsafe { libc::unlinkat(dir.as_raw_fd(), name.as_ptr(), 0) } < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Enumerate the retained directory itself, never re-resolve its filesystem pathname.
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(crate) fn names(dir: &File) -> io::Result<Vec<CString>> {
    use std::os::fd::IntoRawFd;
    struct Stream(*mut libc::DIR);
    impl Drop for Stream {
        fn drop(&mut self) {
            unsafe {
                libc::closedir(self.0);
            }
        }
    }
    let fd = child_dir(dir, c".")?.into_raw_fd();
    let raw = unsafe { libc::fdopendir(fd) };
    if raw.is_null() {
        let error = io::Error::last_os_error();
        unsafe {
            libc::close(fd);
        }
        return Err(error);
    }
    let stream = Stream(raw);
    let mut names = Vec::new();
    loop {
        #[cfg(target_os = "macos")]
        let errno = unsafe { libc::__error() };
        #[cfg(target_os = "linux")]
        let errno = unsafe { libc::__errno_location() };
        unsafe {
            *errno = 0;
        }
        let entry = unsafe { libc::readdir(stream.0) };
        if entry.is_null() {
            if unsafe { *errno } != 0 {
                return Err(io::Error::last_os_error());
            }
            break;
        }
        // d_name is NUL-terminated and owned by the stream until its next readdir.
        let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) };
        if name != c"." && name != c".." {
            names.push(name.to_owned());
        }
    }
    names.sort();
    Ok(names)
}
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub(crate) fn names(_: &File) -> io::Result<Vec<CString>> {
    Err(io::Error::other(
        "safe directory enumeration is not verified on this platform",
    ))
}
pub(crate) fn directory(gil: &Path, create: bool) -> io::Result<File> {
    if gil.file_name() != Some(std::ffi::OsStr::new(".gil")) {
        return Err(refused());
    }
    let dir = project_directory(gil.parent().ok_or_else(refused)?)?;
    let name = c".gil";
    if create {
        if unsafe { libc::mkdirat(dir.as_raw_fd(), name.as_ptr(), 0o700) } < 0 {
            let e = io::Error::last_os_error();
            if e.kind() != io::ErrorKind::AlreadyExists {
                return Err(e);
            }
        } else {
            dir.sync_all()?;
        }
    }
    open(&dir, name, libc::O_RDONLY | libc::O_DIRECTORY)
}
pub(crate) fn project_directory(root: &Path) -> io::Result<File> {
    let project = root.canonicalize()?;
    let mut dir = File::open("/")?;
    for c in project.components() {
        if let Component::Normal(c) = c {
            let name = CString::new(c.as_bytes()).map_err(|_| refused())?;
            dir = open(&dir, &name, libc::O_RDONLY | libc::O_DIRECTORY)?;
        }
    }
    Ok(dir)
}
pub(crate) fn file(path: &Path, create_lock: bool) -> io::Result<(File, File)> {
    let dir = directory(path.parent().ok_or_else(refused)?, false)?;
    let name =
        CString::new(path.file_name().ok_or_else(refused)?.as_bytes()).map_err(|_| refused())?;
    let file = open(
        &dir,
        &name,
        if create_lock {
            libc::O_RDWR | libc::O_CREAT
        } else {
            libc::O_RDONLY
        },
    )?;
    let meta = file.metadata()?;
    if !meta.is_file() || meta.nlink() != 1 {
        return Err(refused());
    }
    Ok((dir, file))
}
pub(crate) fn same(a: &File, b: &File) -> io::Result<bool> {
    let a = a.metadata()?;
    let b = b.metadata()?;
    Ok(a.dev() == b.dev() && a.ino() == b.ino())
}
pub(crate) fn read_state(path: &Path) -> io::Result<String> {
    use std::io::Read;
    let (dir, mut held) = file(path, false)?;
    let mut text = String::new();
    held.read_to_string(&mut text)?;
    let (now_dir, now_file) = file(path, false)?;
    if !same(&dir, &now_dir)? || !same(&held, &now_file)? {
        return Err(refused());
    }
    Ok(text)
}
