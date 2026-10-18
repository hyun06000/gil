//! Windows handle-based storage primitives for the image-report paths (Windows contract §2–§4).
//! Every component is opened with `FILE_FLAG_OPEN_REPARSE_POINT` and refused when it is a
//! reparse point of any kind; ancestors stay pinned while an operation runs. Local fixed NTFS
//! is required only for new Project creation. Same-user races after a check are not all removed.
use std::ffi::{OsStr, c_void};
use std::fs::File;
use std::io;
use std::mem::{size_of, zeroed};
use std::os::windows::{ffi::OsStrExt, io::{AsRawHandle, FromRawHandle}};
use std::path::{Component, Path, PathBuf, Prefix};
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::*;

/// WinBase.h `FILE_RENAME_FLAG_*` (the `WindowsProgramming` feature is not enabled).
const RENAME_REPLACE_IF_EXISTS: u32 = 1;
const RENAME_POSIX_SEMANTICS: u32 = 2;
/// WinBase.h `DRIVE_FIXED`.
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
const LOCAL_FIXED_DRIVE: u32 = 3;

pub(crate) const SHARE_ALL: u32 = FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE;
/// Deny rename/delete of the entry while the handle is held.
pub(crate) const SHARE_PINNED: u32 = FILE_SHARE_READ | FILE_SHARE_WRITE;
pub(crate) const READ: u32 = GENERIC_READ;
pub(crate) const READ_WRITE: u32 = GENERIC_READ | GENERIC_WRITE;
/// Rename and handle-based deletion need `DELETE` on the file handle itself.
pub(crate) const READ_WRITE_DELETE: u32 = GENERIC_READ | GENERIC_WRITE | DELETE;

pub(crate) fn refused() -> io::Error {
    io::Error::other("Windows storage boundary refused")
}

fn wide(path: &Path) -> io::Result<Vec<u16>> {
    let mut text: Vec<u16> = path.as_os_str().encode_wide().collect();
    if text.contains(&0) {
        return Err(refused());
    }
    text.push(0);
    Ok(text)
}

/// A single entry name: no separators, no stream syntax, no `.`/`..`.
fn plain_name(name: &OsStr) -> io::Result<()> {
    if name.is_empty() || name == "." || name == ".." {
        return Err(refused());
    }
    if name
        .encode_wide()
        .any(|c| c == 0 || c == u16::from(b'\\') || c == u16::from(b'/') || c == u16::from(b':'))
    {
        return Err(refused());
    }
    Ok(())
}

fn open_raw(path: &Path, access: u32, share: u32, disposition: u32, flags: u32) -> io::Result<File> {
    let name = wide(path)?;
    // SAFETY: nul-terminated name; no security attributes; the handle is owned exactly once.
    let raw = unsafe {
        CreateFileW(
            name.as_ptr(),
            access,
            share,
            null_mut(),
            disposition,
            flags | FILE_FLAG_OPEN_REPARSE_POINT,
            null_mut(),
        )
    };
    if raw == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_handle(raw) })
}

pub(crate) fn info(file: &File) -> io::Result<BY_HANDLE_FILE_INFORMATION> {
    unsafe {
        let mut info = zeroed();
        if GetFileInformationByHandle(file.as_raw_handle(), &mut info) == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(info)
    }
}

fn basic(file: &File) -> io::Result<FILE_BASIC_INFO> {
    unsafe {
        let mut out: FILE_BASIC_INFO = zeroed();
        if GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileBasicInfo,
            (&mut out as *mut FILE_BASIC_INFO).cast::<c_void>(),
            size_of::<FILE_BASIC_INFO>() as u32,
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(out)
    }
}

/// Volume serial plus the 128-bit file id (NTFS and ReFS), never a pathname.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Id {
    pub volume: u64,
    pub file: u128,
}

pub(crate) fn id(file: &File) -> io::Result<Id> {
    unsafe {
        let mut out: FILE_ID_INFO = zeroed();
        if GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileIdInfo,
            (&mut out as *mut FILE_ID_INFO).cast::<c_void>(),
            size_of::<FILE_ID_INFO>() as u32,
        ) != 0
        {
            return Ok(Id { volume: out.VolumeSerialNumber, file: u128::from_le_bytes(out.FileId.Identifier) });
        }
    }
    // Volumes without 128-bit ids answer with the 64-bit index instead of a guess.
    let meta = info(file)?;
    let index = (u64::from(meta.nFileIndexHigh) << 32) | u64::from(meta.nFileIndexLow);
    if index == 0 || index == u64::MAX {
        return Err(refused());
    }
    Ok(Id { volume: u64::from(meta.dwVolumeSerialNumber), file: u128::from(index) })
}

/// Everything that must stay equal for a retained handle and its current name (contract §4 W2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Stamp {
    pub id: Id,
    pub size: u64,
    pub links: u32,
    pub attributes: u32,
    pub created: i64,
    pub written: i64,
    pub changed: i64,
}

impl Stamp {
    pub(crate) fn of(file: &File) -> io::Result<Self> {
        let meta = info(file)?;
        let times = basic(file)?;
        Ok(Self {
            id: id(file)?,
            size: (u64::from(meta.nFileSizeHigh) << 32) | u64::from(meta.nFileSizeLow),
            links: meta.nNumberOfLinks,
            attributes: meta.dwFileAttributes,
            created: times.CreationTime,
            written: times.LastWriteTime,
            changed: times.ChangeTime,
        })
    }
    /// An ordinary file with exactly one name: not a directory, reparse point or hard link.
    pub(crate) fn is_single_regular(&self) -> bool {
        self.attributes & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DIRECTORY) == 0 && self.links == 1
    }
}

fn ordinary(file: &File, directory: bool) -> io::Result<()> {
    let flags = info(file)?.dwFileAttributes;
    if flags & FILE_ATTRIBUTE_REPARSE_POINT != 0 || (flags & FILE_ATTRIBUTE_DIRECTORY != 0) != directory {
        return Err(refused());
    }
    Ok(())
}

/// Only `C:\…` / `\\?\C:\…` with no `.`/`..` components. UNC and device paths are refused.
pub(crate) fn absolute(path: &Path) -> io::Result<()> {
    let mut parts = path.components();
    match parts.next() {
        Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)) => {}
        _ => return Err(refused()),
    }
    if parts.next() != Some(Component::RootDir) {
        return Err(refused());
    }
    if parts.any(|c| !matches!(c, Component::Normal(_))) {
        return Err(refused());
    }
    Ok(())
}

/// Open a directory entry itself (no reparse traversal), optionally creating it first.
/// `SHARE_PINNED` denies rename/delete of that directory while the handle is held.
pub(crate) fn open_dir(path: &Path, share: u32, create: bool) -> io::Result<File> {
    if create {
        let name = wide(path)?;
        // Inherited ACL, like the existing Windows format-4 bootstrap (contract §8 item 1).
        if unsafe { CreateDirectoryW(name.as_ptr(), null_mut()) } == 0 {
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::AlreadyExists {
                return Err(error);
            }
        }
    }
    let dir = open_raw(path, READ, share, OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS)?;
    ordinary(&dir, true)?;
    Ok(dir)
}

/// Exclusive directory reservation: an entry of any kind under that name is an error.
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
pub(crate) fn create_dir_exclusive(path: &Path) -> io::Result<()> {
    let name = wide(path)?;
    if unsafe { CreateDirectoryW(name.as_ptr(), null_mut()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Open or create a file entry itself; the result is never a directory or reparse point.
pub(crate) fn open_file(path: &Path, access: u32, share: u32, disposition: u32) -> io::Result<File> {
    let file = open_raw(path, access, share, disposition, FILE_ATTRIBUTE_NORMAL)?;
    ordinary(&file, false)?;
    Ok(file)
}

/// Ancestor-pinned directory walk from the volume root. Each level is opened without following
/// reparse points and stays open, so a later path resolution cannot be redirected above it.
pub(crate) struct Chain {
    path: PathBuf,
    pins: Vec<File>,
}

impl Chain {
    pub(crate) fn open(root: &Path) -> io::Result<Self> {
        absolute(root)?;
        let mut at = PathBuf::new();
        let mut pins = Vec::new();
        for part in root.components() {
            at.push(part);
            if matches!(part, Component::Prefix(_)) {
                continue;
            }
            pins.push(open_dir(&at, SHARE_PINNED, false)?);
        }
        if pins.is_empty() {
            return Err(refused());
        }
        Ok(Self { path: at, pins })
    }
    pub(crate) fn push(&mut self, name: &OsStr, create: bool) -> io::Result<()> {
        plain_name(name)?;
        let at = self.path.join(name);
        let dir = open_dir(&at, SHARE_PINNED, create)?;
        self.path = at;
        self.pins.push(dir);
        Ok(())
    }
    pub(crate) fn dir(&self) -> &File {
        self.pins.last().expect("a chain always holds its last directory")
    }
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
    /// A validated child entry path under the pinned directory.
    pub(crate) fn child(&self, name: &OsStr) -> io::Result<PathBuf> {
        plain_name(name)?;
        Ok(self.path.join(name))
    }
    /// Re-open the pinned directory by name and confirm it is still the same entry.
    pub(crate) fn still_named(&self) -> io::Result<()> {
        let named = open_dir(&self.path, SHARE_ALL, false)?;
        if id(&named)? != id(self.dir())? {
            return Err(refused());
        }
        Ok(())
    }
}

pub(crate) fn flush(file: &File) -> io::Result<()> {
    if unsafe { FlushFileBuffers(file.as_raw_handle()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Flush directory metadata through a write handle to the same pinned entry.
/// Power-loss durability on every storage controller is not established by this call.
pub(crate) fn flush_dir(path: &Path, pinned: &File) -> io::Result<()> {
    let dir = open_raw(path, READ_WRITE, SHARE_ALL, OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS)?;
    ordinary(&dir, true)?;
    if id(&dir)? != id(pinned)? {
        return Err(refused());
    }
    flush(&dir)
}

/// Rename the open file to `to` inside `dir` (NT handle-relative, POSIX semantics).
/// Without `replace` an existing entry of any kind fails; with it the entry is replaced,
/// never followed. The file handle needs `DELETE` access.
pub(crate) fn rename(file: &File, dir: &File, to: &OsStr, replace: bool) -> io::Result<()> {
    plain_name(to)?;
    let name: Vec<u16> = to.encode_wide().collect();
    let name_bytes = name.len() * size_of::<u16>();
    let total = size_of::<FILE_RENAME_INFO>() + name_bytes;
    let mut buffer = vec![0u64; total.div_ceil(size_of::<u64>())];
    unsafe {
        let record = buffer.as_mut_ptr().cast::<FILE_RENAME_INFO>();
        (*record).Anonymous.Flags =
            RENAME_POSIX_SEMANTICS | if replace { RENAME_REPLACE_IF_EXISTS } else { 0 };
        (*record).RootDirectory = dir.as_raw_handle();
        (*record).FileNameLength = name_bytes as u32;
        std::ptr::copy_nonoverlapping(
            name.as_ptr(),
            std::ptr::addr_of_mut!((*record).FileName).cast::<u16>(),
            name.len(),
        );
        if SetFileInformationByHandle(
            file.as_raw_handle(),
            FileRenameInfoEx,
            buffer.as_ptr().cast::<c_void>(),
            total as u32,
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

/// Delete the entry this handle was opened through. Nothing is resolved by name again, so a
/// replacement planted under the old name is never deleted. Needs `DELETE` access.
pub(crate) fn delete(file: &File) -> io::Result<()> {
    let record = FILE_DISPOSITION_INFO_EX {
        Flags: FILE_DISPOSITION_FLAG_DELETE | FILE_DISPOSITION_FLAG_POSIX_SEMANTICS,
    };
    if unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            FileDispositionInfoEx,
            (&record as *const FILE_DISPOSITION_INFO_EX).cast::<c_void>(),
            size_of::<FILE_DISPOSITION_INFO_EX>() as u32,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// No-clobber publication: fails when `new` already exists as anything.
pub(crate) fn hard_link(existing: &Path, new: &Path) -> io::Result<()> {
    let (existing, new) = (wide(existing)?, wide(new)?);
    if unsafe { CreateHardLinkW(new.as_ptr(), existing.as_ptr(), null_mut()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Hard links are unavailable on this volume (FAT/exFAT answer with these codes).
pub(crate) fn links_unsupported(error: &io::Error) -> bool {
    matches!(error.kind(), io::ErrorKind::Unsupported | io::ErrorKind::PermissionDenied)
        || matches!(error.raw_os_error(), Some(1) | Some(50))
}

/// New image Projects are created only on a local fixed NTFS volume (Distribution §6.2b).
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
pub(crate) fn local_fixed_ntfs(path: &Path, dir: &File) -> io::Result<()> {
    // Drive root from the validated prefix (`C:\` / `\\?\C:\`); folder mount points are reparse
    // points and were already refused by the chain.
    absolute(path)?;
    let root: PathBuf = path.components().take(2).collect();
    let root = wide(&root)?;
    if unsafe { GetDriveTypeW(root.as_ptr()) } != LOCAL_FIXED_DRIVE {
        return Err(io::Error::new(io::ErrorKind::Unsupported, "not a local fixed drive"));
    }
    let mut filesystem = [0u16; 32];
    unsafe {
        if GetVolumeInformationByHandleW(
            dir.as_raw_handle(),
            null_mut(),
            0,
            null_mut(),
            null_mut(),
            null_mut(),
            filesystem.as_mut_ptr(),
            filesystem.len() as u32,
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    if filesystem[..5] != [78, 84, 70, 83, 0] {
        return Err(io::Error::new(io::ErrorKind::Unsupported, "not an NTFS volume"));
    }
    Ok(())
}

/// `<project>/.gil/<name>` opened as the entry itself under a pinned `.gil`: a regular file with
/// one link. The returned directory handle shares deletion like the Unix descriptor; identity is
/// re-checked by the caller instead (contract §4 W4).
pub(crate) fn control_file(path: &Path, create: bool) -> io::Result<(File, File)> {
    let gil = path.parent().ok_or_else(refused)?;
    if gil.file_name() != Some(OsStr::new(".gil")) {
        return Err(refused());
    }
    let root = gil.parent().ok_or_else(refused)?.canonicalize()?;
    let mut chain = Chain::open(&root)?;
    chain.push(OsStr::new(".gil"), false)?;
    let name = path.file_name().ok_or_else(refused)?;
    let at = chain.child(name)?;
    let file = if create {
        open_file(&at, READ_WRITE, SHARE_ALL, OPEN_ALWAYS)?
    } else {
        open_file(&at, READ, SHARE_ALL, OPEN_EXISTING)?
    };
    if !Stamp::of(&file)?.is_single_regular() {
        return Err(refused());
    }
    let directory = open_dir(chain.path(), SHARE_ALL, false)?;
    if id(&directory)? != id(chain.dir())? {
        return Err(refused());
    }
    Ok((directory, file))
}

pub(crate) fn same(a: &File, b: &File) -> io::Result<bool> {
    Ok(id(a)? == id(b)?)
}

pub(crate) fn read_state(path: &Path) -> io::Result<String> {
    use std::io::Read;
    let (dir, mut held) = control_file(path, false)?;
    let mut text = String::new();
    held.read_to_string(&mut text)?;
    let (now_dir, now_file) = control_file(path, false)?;
    if !same(&dir, &now_dir)? || !same(&held, &now_file)? {
        return Err(refused());
    }
    Ok(text)
}
