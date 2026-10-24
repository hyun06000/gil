//! Format-5 state publication. Does not harden Artifact capture or Session opening.
//! Unix: descriptor-relative no-follow writes. Windows: pinned-handle writes (contract §4).
use std::path::Path;

pub(crate) fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    write_inner(path, bytes, false, &mut |_| Ok(()))
}

/// First publication must never replace a concurrently created state entry.
#[cfg(all(
    feature = "visual-authoring",
    any(all(target_os = "macos", target_arch = "aarch64"), windows)
))]
pub(crate) fn write_new(
    path: &Path,
    bytes: &[u8],
    checkpoint: &mut impl FnMut(&str) -> std::io::Result<()>,
) -> std::io::Result<()> {
    write_inner(path, bytes, true, checkpoint)
}

#[cfg(unix)]
fn write_inner(
    path: &Path,
    bytes: &[u8],
    exclusive: bool,
    checkpoint: &mut impl FnMut(&str) -> std::io::Result<()>,
) -> std::io::Result<()> {
    use std::ffi::CString;
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom, Write};
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::{ffi::OsStrExt, fs::MetadataExt};
    use std::path::Component;
    let invalid = || std::io::Error::other("visual state storage boundary refused");
    let parent = path.parent().ok_or_else(invalid)?;
    // The selected Project root may use an OS alias (/tmp on macOS). Never canonicalize .gil.
    if parent.file_name() != Some(std::ffi::OsStr::new(".gil")) {
        return Err(invalid());
    }
    let root = parent.parent().ok_or_else(invalid)?.canonicalize()?;
    let name = path.file_name().ok_or_else(invalid)?;
    let target = CString::new(name.as_bytes()).map_err(|_| invalid())?;
    let temp = CString::new([name.as_bytes(), b".writing"].concat()).map_err(|_| invalid())?;
    let mut dir = File::open("/")?;
    for component in root.components().chain(Path::new(".gil").components()) {
        let Component::Normal(part) = component else {
            continue;
        };
        let part = CString::new(part.as_bytes()).map_err(|_| invalid())?;
        // No creation through a possibly linked internal directory; it must already exist.
        let fd = unsafe {
            libc::openat(
                dir.as_raw_fd(),
                part.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        dir = unsafe { File::from_raw_fd(fd) };
    }
    // O_EXCL prevents truncating a pre-existing file, symlink or hardlink.
    let fd = unsafe {
        libc::openat(
            dir.as_raw_fd(),
            temp.as_ptr(),
            libc::O_RDWR | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let mut file = unsafe { File::from_raw_fd(fd) };
    let parent_still_named = || -> std::io::Result<()> {
        let held = dir.metadata()?;
        let named = std::fs::symlink_metadata(root.join(".gil"))?;
        if !named.is_dir() || held.dev() != named.dev() || held.ino() != named.ino() {
            return Err(invalid());
        }
        Ok(())
    };
    let result = (|| {
        if exclusive {
            let split = bytes.len() / 2;
            file.write_all(&bytes[..split])?;
            checkpoint("partial_write")?;
            file.write_all(&bytes[split..])?;
        } else {
            file.write_all(bytes)?;
        }
        file.flush()?;
        file.sync_all()?;
        checkpoint("file_synced")?;
        file.seek(SeekFrom::Start(0))?;
        let mut observed = Vec::new();
        (&mut file)
            .take(bytes.len() as u64 + 1)
            .read_to_end(&mut observed)?;
        if observed != bytes {
            return Err(invalid());
        }
        let named_fd = unsafe {
            libc::openat(
                dir.as_raw_fd(),
                temp.as_ptr(),
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
            )
        };
        if named_fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let named = unsafe { File::from_raw_fd(named_fd) };
        let a = file.metadata()?;
        let b = named.metadata()?;
        if !b.is_file()
            || a.nlink() != 1
            || b.nlink() != 1
            || a.dev() != b.dev()
            || a.ino() != b.ino()
        {
            return Err(invalid());
        }
        parent_still_named()?;
        checkpoint("before_publish")?;
        // Both names are relative to the same retained directory; destination symlinks
        // are replaced as entries, never followed. A moved parent cannot redirect this write.
        let published = if exclusive {
            #[cfg(target_os = "macos")]
            {
                unsafe {
                    libc::renameatx_np(
                        dir.as_raw_fd(),
                        temp.as_ptr(),
                        dir.as_raw_fd(),
                        target.as_ptr(),
                        libc::RENAME_EXCL,
                    )
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "exclusive visual creation is not verified on this platform",
                ));
            }
        } else {
            unsafe {
                libc::renameat(
                    dir.as_raw_fd(),
                    temp.as_ptr(),
                    dir.as_raw_fd(),
                    target.as_ptr(),
                )
            }
        };
        if published < 0 {
            return Err(std::io::Error::last_os_error());
        }
        checkpoint("after_publish")?;
        dir.sync_all()?;
        checkpoint("directory_synced")?;
        parent_still_named()
    })();
    // Keep a failed temporary file for inspection; never delete a possibly replaced entry.
    result
}

/// Windows: same checkpoints and preservation rules as Unix. `.gil` is pinned (no rename or
/// delete while held), the temporary is created exclusively, verified by re-read and by
/// identity, then renamed relative to the pinned directory handle.
#[cfg(windows)]
fn write_inner(
    path: &Path,
    bytes: &[u8],
    exclusive: bool,
    checkpoint: &mut impl FnMut(&str) -> std::io::Result<()>,
) -> std::io::Result<()> {
    use crate::windows_fs as win;
    use std::ffi::{OsStr, OsString};
    use std::io::{Read, Seek, SeekFrom, Write};
    use windows_sys::Win32::Storage::FileSystem::{CREATE_NEW, OPEN_EXISTING};
    let invalid = || std::io::Error::other("visual state storage boundary refused");
    let parent = path.parent().ok_or_else(invalid)?;
    if parent.file_name() != Some(OsStr::new(".gil")) {
        return Err(invalid());
    }
    let root = parent.parent().ok_or_else(invalid)?.canonicalize()?;
    let name = path.file_name().ok_or_else(invalid)?;
    let mut temp = OsString::from(name);
    temp.push(".writing");
    // No creation through a possibly linked internal directory; it must already exist.
    let mut chain = win::Chain::open(&root)?;
    chain.push(OsStr::new(".gil"), false)?;
    let temp_path = chain.child(&temp)?;
    // CREATE_NEW never truncates a pre-existing file, symlink, hard link or directory.
    let mut file = win::open_file(&temp_path, win::READ_WRITE_DELETE, win::SHARE_ALL, CREATE_NEW)?;
    let result = (|| {
        if exclusive {
            let split = bytes.len() / 2;
            file.write_all(&bytes[..split])?;
            checkpoint("partial_write")?;
            file.write_all(&bytes[split..])?;
        } else {
            file.write_all(bytes)?;
        }
        file.flush()?;
        file.sync_all()?;
        checkpoint("file_synced")?;
        file.seek(SeekFrom::Start(0))?;
        let mut observed = Vec::new();
        (&mut file)
            .take(bytes.len() as u64 + 1)
            .read_to_end(&mut observed)?;
        if observed != bytes {
            return Err(invalid());
        }
        let named = win::open_file(&temp_path, win::READ, win::SHARE_ALL, OPEN_EXISTING)?;
        let (a, b) = (win::Stamp::of(&file)?, win::Stamp::of(&named)?);
        if a.id != b.id || !a.is_single_regular() || !b.is_single_regular() {
            return Err(invalid());
        }
        chain.still_named()?;
        checkpoint("before_publish")?;
        // Relative to the pinned directory handle: destination entries are replaced (ordinary
        // save) or refused (exclusive creation), never followed.
        win::rename(&file, chain.dir(), chain.path(), name, !exclusive)?;
        checkpoint("after_publish")?;
        win::flush_dir(chain.path(), chain.dir())?;
        checkpoint("directory_synced")?;
        chain.still_named()
    })();
    // Keep a failed temporary file for inspection; never delete a possibly replaced entry.
    result
}
