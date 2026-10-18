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

/// Rename the open file to `to` inside the pinned directory `dir` (POSIX semantics). The Win32
/// wrapper rejects a `RootDirectory`-relative name (ERROR_INVALID_PARAMETER, CI run 39125038859),
/// so the destination is the full path under `dir_path`, like `std::fs::rename`; the pinned
/// ancestors cannot be renamed or replaced while this runs, and `dir` is re-checked by identity.
/// Without `replace` an existing entry of any kind fails; with it the entry is replaced, never
/// followed. The file handle needs `DELETE` access.
pub(crate) fn rename(file: &File, dir: &File, dir_path: &Path, to: &OsStr, replace: bool) -> io::Result<()> {
    plain_name(to)?;
    let named = open_dir(dir_path, SHARE_ALL, false)?;
    if id(&named)? != id(dir)? {
        return Err(refused());
    }
    let mut name: Vec<u16> = dir_path.join(to).as_os_str().encode_wide().collect();
    if name.contains(&0) {
        return Err(refused());
    }
    name.push(0);
    let name_bytes = name.len() * size_of::<u16>();
    let total = size_of::<FILE_RENAME_INFO>() + name_bytes;
    let mut buffer = vec![0u64; total.div_ceil(size_of::<u64>())];
    unsafe {
        let record = buffer.as_mut_ptr().cast::<FILE_RENAME_INFO>();
        (*record).Anonymous.Flags =
            RENAME_POSIX_SEMANTICS | if replace { RENAME_REPLACE_IF_EXISTS } else { 0 };
        (*record).RootDirectory = null_mut();
        // Length excludes the terminating NUL that the buffer still carries.
        (*record).FileNameLength = (name_bytes - size_of::<u16>()) as u32;
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

// ── Owner-only Project storage (Windows contract §4 W1/W4, §8) ───────────────────────────
//
// `.gil` of an image Project is born with an explicit protected DACL: the current user and
// SYSTEM have full access with container+object inheritance, nothing else, no inherited ACEs.
// That is the Windows equivalent of the Unix `0o700` reservation: other local accounts cannot
// read or change the records, whatever the surrounding folder inherits. Existing Projects are
// never repaired; a weakened DACL is refused when a format-5 Project is opened.

use windows_sys::Win32::Security::{
    ACCESS_ALLOWED_ACE, ACE_HEADER, ACL, CONTAINER_INHERIT_ACE, DACL_SECURITY_INFORMATION,
    EqualSid, GetAce, GetSecurityDescriptorControl, GetTokenInformation, OBJECT_INHERIT_ACE,
    OWNER_SECURITY_INFORMATION, PSID, SE_DACL_PROTECTED, SECURITY_ATTRIBUTES, TOKEN_QUERY,
    TOKEN_USER, TokenUser,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
    ConvertStringSidToSidW, GetSecurityInfo, SE_FILE_OBJECT,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows_sys::Win32::Foundation::{CloseHandle, LocalFree};

struct Local(*mut c_void);
impl Drop for Local {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0);
        }
    }
}
struct Token(windows_sys::Win32::Foundation::HANDLE);
impl Drop for Token {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

// usize storage supplies TOKEN_USER alignment. Its embedded SID points into this buffer.
fn user() -> io::Result<Vec<usize>> {
    unsafe {
        let mut handle = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut handle) == 0 {
            return Err(io::Error::last_os_error());
        }
        let token = Token(handle);
        let mut bytes = 0;
        GetTokenInformation(token.0, TokenUser, null_mut(), 0, &mut bytes);
        if bytes < size_of::<TOKEN_USER>() as u32 || bytes > 65536 {
            return Err(refused());
        }
        let mut buf = vec![0usize; (bytes as usize).div_ceil(size_of::<usize>())];
        if GetTokenInformation(token.0, TokenUser, buf.as_mut_ptr().cast(), bytes, &mut bytes) == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(buf)
    }
}
fn sid(buf: &[usize]) -> PSID {
    unsafe { (*(buf.as_ptr().cast::<TOKEN_USER>())).User.Sid }
}
fn sid_text(sid: PSID) -> io::Result<String> {
    unsafe {
        let mut text = null_mut();
        if ConvertSidToStringSidW(sid, &mut text) == 0 {
            return Err(io::Error::last_os_error());
        }
        let _text = Local(text.cast());
        let mut len = 0;
        while *text.add(len) != 0 {
            len += 1;
        }
        String::from_utf16(std::slice::from_raw_parts(text, len)).map_err(|_| refused())
    }
}
fn system_sid() -> io::Result<Local> {
    let text: Vec<u16> = "S-1-5-18\0".encode_utf16().collect();
    unsafe {
        let mut sid = null_mut();
        if ConvertStringSidToSidW(text.as_ptr(), &mut sid) == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Local(sid))
    }
}
/// `O:<user>D:P(A;OICI;FA;;;<user>)(A;OICI;FA;;;SY)`
fn private_descriptor() -> io::Result<Local> {
    let user = user()?;
    let name = sid_text(sid(&user))?;
    let sddl: Vec<u16> = format!("O:{name}D:P(A;OICI;FA;;;{name})(A;OICI;FA;;;SY)\0")
        .encode_utf16()
        .collect();
    unsafe {
        let mut sd = null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.as_ptr(), 1, &mut sd, null_mut()) == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Local(sd))
    }
}

/// Exclusive reservation of an owner-only directory: an entry of any kind under that name is
/// an error, and the DACL exists before any record is written into it.
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
pub(crate) fn create_dir_exclusive_private(path: &Path) -> io::Result<()> {
    let sd = private_descriptor()?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: sd.0,
        bInheritHandle: 0,
    };
    let name = wide(path)?;
    if unsafe { CreateDirectoryW(name.as_ptr(), &attributes) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Open a directory entry with `DELETE` access so a just-created, still empty reservation can be
/// removed through the handle when its DACL does not verify. Never follows a reparse point.
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
pub(crate) fn open_dir_deletable(path: &Path) -> io::Result<File> {
    let dir = open_raw(path, READ | DELETE, SHARE_ALL, OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS)?;
    ordinary(&dir, true)?;
    Ok(dir)
}

/// The directory behind this handle is owner-only: owned by the current user, protected DACL,
/// exactly two inheritable full-access allow ACEs (current user, SYSTEM), nothing else.
/// This reads the effective security descriptor, not a path or a string we wrote ourselves.
/// The error names the first condition that failed, so a user can see what to change by hand.
pub(crate) fn private_dir(file: &File) -> io::Result<()> {
    let user = user()?;
    let system = system_sid()?;
    let fail = |why: &str| io::Error::other(format!("owner-only storage check failed: {why}"));
    unsafe {
        let (mut owner, mut acl, mut sd) = (null_mut(), null_mut(), null_mut());
        let error = GetSecurityInfo(
            file.as_raw_handle(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            null_mut(),
            &mut acl,
            null_mut(),
            &mut sd,
        );
        if error != 0 {
            return Err(io::Error::from_raw_os_error(error as i32));
        }
        let sd = Local(sd);
        if owner.is_null() || EqualSid(owner, sid(&user)) == 0 {
            return Err(fail("owner is not the current user"));
        }
        if acl.is_null() {
            return Err(fail("no DACL (everyone has access)"));
        }
        let (mut control, mut revision) = (0, 0);
        if GetSecurityDescriptorControl(sd.0, &mut control, &mut revision) == 0 {
            return Err(io::Error::last_os_error());
        }
        if control & SE_DACL_PROTECTED == 0 {
            return Err(fail("DACL still inherits from the parent folder"));
        }
        let count = (*acl.cast::<ACL>()).AceCount;
        if count != 2 {
            return Err(fail(&format!("{count} ACEs instead of the two expected (user, SYSTEM)")));
        }
        let (mut saw_user, mut saw_system) = (false, false);
        for index in 0..2 {
            let mut ace = null_mut();
            if GetAce(acl, index, &mut ace) == 0 || ace.is_null() {
                return Err(fail("unreadable ACE"));
            }
            let header = &*ace.cast::<ACE_HEADER>();
            if header.AceType != 0 {
                return Err(fail(&format!("ACE {index} is not an access-allowed ACE")));
            }
            if header.AceFlags != (OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE) as u8 {
                return Err(fail(&format!("ACE {index} flags {:#x} are not exactly OI|CI", header.AceFlags)));
            }
            if (header.AceSize as usize) < size_of::<ACCESS_ALLOWED_ACE>() {
                return Err(fail("truncated ACE"));
            }
            let allow = &*ace.cast::<ACCESS_ALLOWED_ACE>();
            if allow.Mask != FILE_ALL_ACCESS {
                return Err(fail(&format!("ACE {index} mask {:#x} is not full access", allow.Mask)));
            }
            let allowed: PSID = std::ptr::addr_of!(allow.SidStart).cast_mut().cast();
            if EqualSid(allowed, sid(&user)) != 0 {
                saw_user = true;
            } else if EqualSid(allowed, system.0) != 0 {
                saw_system = true;
            } else {
                return Err(fail(&format!("ACE {index} grants another principal ({})", sid_text(allowed).unwrap_or_default())));
            }
        }
        if !(saw_user && saw_system) {
            return Err(fail("user or SYSTEM ACE missing"));
        }
    }
    Ok(())
}

/// Explicitly give an existing directory the same owner-only descriptor new reservations are
/// born with (owner, protected DACL, inheritance to children). Nothing in GIL calls this on its
/// own; it exists for an explicit, caller-driven adoption and for test fixtures.
pub(crate) fn set_private_dacl(path: &Path) -> io::Result<()> {
    use windows_sys::Win32::Security::{GetSecurityDescriptorDacl, GetSecurityDescriptorOwner, PROTECTED_DACL_SECURITY_INFORMATION};
    use windows_sys::Win32::Security::Authorization::SetSecurityInfo;
    use windows_sys::Win32::Storage::FileSystem::{READ_CONTROL, WRITE_DAC, WRITE_OWNER};
    let dir = open_raw(path, READ | READ_CONTROL | WRITE_DAC | WRITE_OWNER, SHARE_ALL, OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS)?;
    ordinary(&dir, true)?;
    let sd = private_descriptor()?;
    unsafe {
        let (mut present, mut defaulted, mut dacl, mut owner) = (0, 0, null_mut(), null_mut());
        if GetSecurityDescriptorDacl(sd.0, &mut present, &mut dacl, &mut defaulted) == 0
            || GetSecurityDescriptorOwner(sd.0, &mut owner, &mut defaulted) == 0
            || present == 0
        {
            return Err(refused());
        }
        let error = SetSecurityInfo(
            dir.as_raw_handle(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            owner,
            null_mut(),
            dacl,
            null_mut(),
        );
        if error != 0 {
            return Err(io::Error::from_raw_os_error(error as i32));
        }
    }
    private_dir(&dir)
}

/// Reopening an image Project: its `.gil` must still be owner-only. Nothing is repaired.
pub(crate) fn require_private_gil(gil: &Path) -> io::Result<()> {
    let dir = open_dir(gil, SHARE_ALL, false)?;
    private_dir(&dir).map_err(|_| {
        io::Error::other(
            "이미지 Project 저장소(.gil)의 접근 권한이 소유자 전용(현재 사용자·SYSTEM, 상속 차단)이 아니다. \
             자동으로 고치지 않는다. 기록을 보존한 채 권한을 확인한다",
        )
    })
}

#[cfg(test)]
mod tests {
    //! Windows contract §6 W7-*: the full-path rename stays bound to the pinned directory.
    use super::*;
    use std::fs;
    use windows_sys::Win32::Storage::FileSystem::CREATE_NEW;

    fn scratch(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("gil-winfs-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root.canonicalize().unwrap()
    }
    fn junction(link: &Path, target: &Path) {
        let shell = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let made = std::process::Command::new(shell)
            .args(["-NoProfile", "-NonInteractive", "-Command",
                "$ErrorActionPreference='Stop'; New-Item -ItemType Junction -Path $env:GIL_TEST_LINK -Target $env:GIL_TEST_TARGET | Out-Null"])
            .env("GIL_TEST_LINK", link)
            .env("GIL_TEST_TARGET", target)
            .output()
            .unwrap();
        assert!(made.status.success(), "junction fixture creation failed");
    }

    #[test]
    fn pinned_chain_blocks_rename_and_delete_of_every_component_until_released() {
        let root = scratch("pin");
        let deep = root.join("a").join("b").join("c");
        fs::create_dir_all(&deep).unwrap();
        let chain = Chain::open(&deep).unwrap();
        for path in [root.join("a"), root.join("a/b"), deep.clone()] {
            let moved = path.with_extension("moved");
            assert!(fs::rename(&path, &moved).is_err(), "{path:?} renamed while pinned");
            assert!(!moved.exists());
        }
        assert!(fs::remove_dir(&deep).is_err(), "pinned leaf removed");
        // A sibling the chain does not hold is not affected: the pin is per entry.
        fs::create_dir(root.join("free")).unwrap();
        fs::rename(root.join("free"), root.join("free.moved")).unwrap();
        drop(chain);
        fs::rename(&deep, deep.with_extension("moved")).unwrap();
    }

    #[test]
    fn rename_refuses_a_directory_replaced_behind_an_unpinned_handle() {
        let root = scratch("swap");
        let dir_path = root.join("gil");
        fs::create_dir(&dir_path).unwrap();
        let held = root.join("held");
        fs::create_dir(&held).unwrap();
        let file_path = held.join("state.writing");
        let file = open_file(&file_path, READ_WRITE_DELETE, SHARE_ALL, CREATE_NEW).unwrap();
        // A handle that shares deletion (like the lock's directory handle) does not pin.
        let unpinned = open_dir(&dir_path, SHARE_ALL, false).unwrap();
        fs::rename(&dir_path, root.join("gil.old")).unwrap();
        fs::create_dir(&dir_path).unwrap();
        // The replacement at the same path is a different entry: refused before any rename.
        assert!(rename(&file, &unpinned, &dir_path, OsStr::new("state"), false).is_err());
        assert!(!dir_path.join("state").exists());
        assert!(!root.join("gil.old").join("state").exists());
        assert!(file_path.exists());
    }

    #[test]
    fn exclusive_rename_fails_on_any_existing_entry_and_replace_swaps_only_the_entry() {
        let root = scratch("exclusive");
        let dir_path = root.join("gil");
        fs::create_dir(&dir_path).unwrap();
        let chain = Chain::open(&dir_path).unwrap();
        let victim = root.join("victim.txt");
        fs::write(&victim, b"keep").unwrap();
        let target_dir = root.join("target-dir");
        fs::create_dir(&target_dir).unwrap();
        for (kind, make) in [
            ("file", Box::new(|at: &Path| fs::write(at, b"existing").unwrap()) as Box<dyn Fn(&Path)>),
            ("directory", Box::new(|at: &Path| fs::create_dir(at).unwrap())),
            ("junction", Box::new(|at: &Path| junction(at, &target_dir))),
        ] {
            let target = dir_path.join("state");
            make(&target);
            let temp = dir_path.join("state.writing");
            let file = open_file(&temp, READ_WRITE_DELETE, SHARE_ALL, CREATE_NEW).unwrap();
            assert!(rename(&file, chain.dir(), chain.path(), OsStr::new("state"), false).is_err(), "{kind}");
            assert!(temp.exists(), "{kind}: temporary vanished");
            let meta = fs::symlink_metadata(&target).unwrap();
            match kind {
                "file" => assert_eq!(fs::read(&target).unwrap(), b"existing"),
                "directory" => assert!(meta.is_dir()),
                _ => assert!(meta.is_symlink()),
            }
            delete(&file).unwrap();
            drop(file);
            if meta.is_symlink() { fs::remove_dir(&target).unwrap() } else if meta.is_dir() { fs::remove_dir(&target).unwrap() } else { fs::remove_file(&target).unwrap() }
        }
        // Replacing: a file symlink at the destination is swapped as an entry; its target is kept.
        let target = dir_path.join("state");
        match std::os::windows::fs::symlink_file(&victim, &target) {
            Ok(()) => {
                let temp = dir_path.join("state.writing");
                let file = open_file(&temp, READ_WRITE_DELETE, SHARE_ALL, CREATE_NEW).unwrap();
                use std::io::Write;
                (&file).write_all(b"published").unwrap();
                rename(&file, chain.dir(), chain.path(), OsStr::new("state"), true).unwrap();
                assert_eq!(fs::read(&victim).unwrap(), b"keep");
                assert!(!fs::symlink_metadata(&target).unwrap().is_symlink());
                assert_eq!(fs::read(&target).unwrap(), b"published");
                assert!(!temp.exists());
            }
            Err(e) => panic!("file symlink fixture needs the symlink privilege: {e}"),
        }
    }

    #[test]
    fn control_file_refuses_a_hard_linked_or_linked_record() {
        let root = scratch("control");
        fs::create_dir(root.join(".gil")).unwrap();
        let state = root.join(".gil/state.yaml");
        fs::write(&state, b"format: 5\n").unwrap();
        assert!(control_file(&state, false).is_ok());
        fs::hard_link(&state, root.join("twin")).unwrap();
        assert!(control_file(&state, false).is_err(), "two links accepted");
        fs::remove_file(root.join("twin")).unwrap();
        assert!(control_file(&state, false).is_ok());
        let outside = root.join("outside.yaml");
        fs::write(&outside, b"format: 5\n").unwrap();
        fs::remove_file(&state).unwrap();
        std::os::windows::fs::symlink_file(&outside, &state).expect("symlink privilege");
        assert!(control_file(&state, false).is_err(), "symlink followed");
        assert!(read_state(&state).is_err());
    }
}
