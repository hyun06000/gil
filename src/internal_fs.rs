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
/// Open a caller-named directory itself (`O_DIRECTORY | O_NOFOLLOW`): a symlink in the last
/// component is refused, ancestors are resolved by the OS. A trailing `/` would make the kernel
/// resolve the last link and defeat `O_NOFOLLOW`, so the path is rebuilt from its components.
/// Shared by the policy engine's tree walk and the root `.gilignore` observation.
pub(crate) fn dir_nofollow(path: &Path) -> io::Result<File> {
    let path: std::path::PathBuf = path.components().collect();
    let path = CString::new(path.as_os_str().as_bytes()).map_err(|_| io::Error::other("경로에 NUL 이 있다"))?;
    // SAFETY: NUL-terminated path; a successful fd is owned exactly once.
    let fd = unsafe { libc::open(path.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}
/// Linux has no local-mount flag. The folder must sit on one of the filesystems named here — an
/// allow-list, read from the held descriptor, of local filesystems whose no-replace rename, in-folder
/// rename, directory identity and `fsync` GIL relies on. Every other type (network, FUSE, FAT-family,
/// unknown) is refused; nothing falls back to a weaker path. The list is a support scope, not a
/// durability claim. Shared by the image-report creation and the approved recovery.
#[cfg(target_os = "linux")]
pub(crate) fn listed_local_filesystem(dir: &File) -> io::Result<()> {
    let mut filesystem = std::mem::MaybeUninit::<libc::statfs>::uninit();
    // SAFETY: a valid descriptor and a buffer of the type the call fills.
    if unsafe { libc::fstatfs(dir.as_raw_fd(), filesystem.as_mut_ptr()) } < 0 {
        return Err(io::Error::last_os_error());
    }
    #[allow(clippy::unnecessary_cast)]
    let kind = unsafe { filesystem.assume_init() }.f_type as i64;
    if !listed_filesystem_type(kind) {
        return Err(io::Error::new(io::ErrorKind::Unsupported, "검수 대상 로컬 파일 시스템(ext4·XFS·Btrfs·tmpfs·overlayfs)이 아니다"));
    }
    Ok(())
}
/// statfs(2) `f_type` values: ext2/3/4, XFS, Btrfs, tmpfs, overlayfs.
#[cfg(any(target_os = "linux", test))]
pub(crate) fn listed_filesystem_type(kind: i64) -> bool {
    const LISTED: [i64; 5] = [0xEF53, 0x5846_5342, 0x9123_683E, 0x0102_1994, 0x794C_7630];
    LISTED.contains(&kind)
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
/// Exclusive directory creation relative to a held directory (`mkdirat`); the new directory is
/// opened without following links and its mode set exactly (umask-independent).
pub(crate) fn create_dir_at(dir: &File, name: &CStr, mode: u32) -> io::Result<File> {
    if unsafe { libc::mkdirat(dir.as_raw_fd(), name.as_ptr(), mode as libc::mode_t) } < 0 {
        return Err(io::Error::last_os_error());
    }
    let made = open(dir, name, libc::O_RDONLY | libc::O_DIRECTORY)?;
    chmod(&made, mode)?;
    Ok(made)
}
/// Exclusive regular-file creation relative to a held directory (`O_CREAT | O_EXCL`, read-write),
/// mode set exactly afterwards.
pub(crate) fn create_file_at(dir: &File, name: &CStr, mode: u32) -> io::Result<File> {
    let file = open(dir, name, libc::O_RDWR | libc::O_CREAT | libc::O_EXCL)?;
    chmod(&file, mode)?;
    Ok(file)
}
pub(crate) fn chmod(file: &File, mode: u32) -> io::Result<()> {
    if unsafe { libc::fchmod(file.as_raw_fd(), mode as libc::mode_t) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
/// `renameat` inside one held directory (replaces an existing `to`, never follows links).
pub(crate) fn rename_at(dir: &File, from: &CStr, to: &CStr) -> io::Result<()> {
    if unsafe { libc::renameat(dir.as_raw_fd(), from.as_ptr(), dir.as_raw_fd(), to.as_ptr()) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
/// `fsync` of a file or directory descriptor.
pub(crate) fn fsync(file: &File) -> io::Result<()> {
    if unsafe { libc::fsync(file.as_raw_fd()) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    /// The allow-list names exactly five local filesystems; a network, FUSE or FAT-family type
    /// (and anything unknown) is not on it.
    #[test]
    fn only_the_reviewed_local_filesystem_types_are_listed() {
        for (name, kind) in [("ext2/3/4", 0xEF53_i64), ("XFS", 0x5846_5342), ("Btrfs", 0x9123_683E), ("tmpfs", 0x0102_1994), ("overlayfs", 0x794C_7630)] {
            assert!(super::listed_filesystem_type(kind), "{name}");
        }
        for (name, kind) in [("NFS", 0x6969_i64), ("CIFS", 0xFF53_4D42), ("SMB2", 0xFE53_4D42), ("FUSE", 0x6573_5546), ("vfat", 0x4d44), ("exFAT", 0x2011_BAB0), ("9p", 0x0102_1997), ("unknown", 0), ("negative", -1)] {
            assert!(!super::listed_filesystem_type(kind), "{name}");
        }
    }
}
