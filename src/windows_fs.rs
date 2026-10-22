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
    /// The pinned directory `levels` above the last one, if the chain is that deep.
    pub(crate) fn above(&self, levels: usize) -> Option<&File> {
        self.pins.len().checked_sub(levels + 1).and_then(|i| self.pins.get(i))
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
    EqualSid, GetAce, GetSecurityDescriptorControl, GetTokenInformation, INHERITED_ACE, OBJECT_INHERIT_ACE,
    OWNER_SECURITY_INFORMATION, PSID, SE_DACL_PROTECTED, TOKEN_QUERY,
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
/// The SID the OS assigns as owner to objects this process creates without an explicit owner
/// (`TokenOwner`): the user itself, or the Administrators group for elevated administrators.
fn default_owner() -> io::Result<Vec<usize>> {
    use windows_sys::Win32::Security::{TOKEN_OWNER, TokenOwner};
    unsafe {
        let mut handle = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut handle) == 0 {
            return Err(io::Error::last_os_error());
        }
        let token = Token(handle);
        let mut bytes = 0;
        GetTokenInformation(token.0, TokenOwner, null_mut(), 0, &mut bytes);
        if bytes < size_of::<TOKEN_OWNER>() as u32 || bytes > 65536 {
            return Err(refused());
        }
        let mut buf = vec![0usize; (bytes as usize).div_ceil(size_of::<usize>())];
        if GetTokenInformation(token.0, TokenOwner, buf.as_mut_ptr().cast(), bytes, &mut bytes) == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(buf)
    }
}
fn owner_sid(buf: &[usize]) -> PSID {
    use windows_sys::Win32::Security::TOKEN_OWNER;
    unsafe { (*(buf.as_ptr().cast::<TOKEN_OWNER>())).Owner }
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
/// `O:<user>D:PAI(A;OICI;FA;;;<user>)(A;OICI;FA;;;SY)` — protected (no parent inheritance),
/// auto-inherited so entries created below carry the INHERITED_ACE flag (CI run 39128571227
/// showed children without it when only `P` was set), two inheritable full-access ACEs.
fn private_descriptor() -> io::Result<Local> {
    let user = user()?;
    let name = sid_text(sid(&user))?;
    let sddl: Vec<u16> = format!("O:{name}D:PAI(A;OICI;FA;;;{name})(A;OICI;FA;;;SY)\0")
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

/// Apply the owner-only descriptor to the object behind this handle (owner, protected DACL,
/// inheritance to children). Used only on a reservation handle during creation and by in-crate
/// tests; nothing adopts an existing Project's storage on its own.
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
pub(crate) fn set_private_dacl_on(dir: &File) -> io::Result<()> {
    use windows_sys::Win32::Security::{GetSecurityDescriptorDacl, GetSecurityDescriptorOwner, PROTECTED_DACL_SECURITY_INFORMATION};
    use windows_sys::Win32::Security::Authorization::SetSecurityInfo;
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
    Ok(())
}

/// Acceptance for every entry *below* `.gil` (Windows contract §8 item 1a): owned by the current
/// user or by the token's default owner (the Administrators group for an elevated administrator,
/// which is what the OS assigns to entries created by inheritance); a DACL is present (a NULL
/// DACL grants everyone); not protected; exactly two allow ACEs,
/// both inherited (`INHERITED_ACE`; directories also carry OI|CI, files nothing else), both
/// `FILE_ALL_ACCESS`, one for the current user and one for SYSTEM. Deny, audit, object or
/// callback ACEs and any other principal fail. The error names the first failing condition.
pub(crate) fn inherited_only(file: &File, directory: bool) -> io::Result<()> {
    let user = user()?;
    let default_owner = default_owner()?;
    let system = system_sid()?;
    let what = if directory { "directory" } else { "file" };
    let fail = |why: &str| io::Error::other(format!("owner-only storage check failed on a {what}: {why}"));
    let expected_flags = if directory {
        (OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE | INHERITED_ACE) as u8
    } else {
        INHERITED_ACE as u8
    };
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
        if owner.is_null()
            || (EqualSid(owner, sid(&user)) == 0 && EqualSid(owner, owner_sid(&default_owner)) == 0)
        {
            return Err(fail("owner is neither the current user nor this token's default owner"));
        }
        if acl.is_null() {
            return Err(fail("NULL DACL (everyone has access)"));
        }
        let (mut control, mut revision) = (0, 0);
        if GetSecurityDescriptorControl(sd.0, &mut control, &mut revision) == 0 {
            return Err(io::Error::last_os_error());
        }
        if control & SE_DACL_PROTECTED != 0 {
            return Err(fail("DACL is protected instead of inherited from .gil"));
        }
        let count = (*acl.cast::<ACL>()).AceCount;
        if count != 2 {
            return Err(fail(&format!("{count} ACEs instead of the two inherited ones (user, SYSTEM)")));
        }
        let (mut saw_user, mut saw_system) = (false, false);
        for index in 0..2 {
            let mut ace = null_mut();
            if GetAce(acl, index, &mut ace) == 0 || ace.is_null() {
                return Err(fail("unreadable ACE"));
            }
            let header = &*ace.cast::<ACE_HEADER>();
            if header.AceType != 0 {
                return Err(fail(&format!("ACE {index} type {} is not access-allowed", header.AceType)));
            }
            if header.AceFlags != expected_flags {
                return Err(fail(&format!("ACE {index} flags {:#x} are not the inherited flags {expected_flags:#x}", header.AceFlags)));
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

/// Stable code carried in the reopen refusal when `.gil`'s protection is no longer verified; the
/// MCP layer lifts it into the structured response so the Skill can apply one rule to it.
pub(crate) const PROTECTION_LOST_CODE: &str = "storage_protection_lost";

/// Reopening an image Project: `.gil` must still be owner-only and the control and structure
/// entries below it must still carry only the inherited owner-only ACEs. Blobs are checked when
/// they are read, not here (contract §8 item 3: an unused blob widened from outside is not
/// detected until it is read). Nothing is repaired.
pub(crate) fn verify_storage_tree(gil: &Path) -> io::Result<()> {
    let wrap = |e: io::Error| {
        io::Error::other(format!(
            "[{PROTECTION_LOST_CODE}] 이미지 Project 저장소(.gil)의 보호(소유자 전용: 현재 사용자·SYSTEM, 상속 차단)가 더 이상 확인되지 않는다. \
             자동으로 고치지 않으며, 사용자에게 권한·상속·소유권·보안 설정 변경을 요청하지 않는다. 기존 기록은 그대로 보존한다. \
             권한 오류만으로 동기화 폴더라고 단정하지 않고, 이 Project 를 새 Project 로 대체하지 않는다. \
             이 위치가 지원 범위(동기화 폴더 밖의 로컬 고정 NTFS)인지 확인하고, 사용자가 원해서 승인할 때만 별도 작업 폴더를 준비해 거기서 새로 시작한다: {e}"
        ))
    };
    if gil.file_name() != Some(OsStr::new(".gil")) {
        return Err(refused());
    }
    let root = gil.parent().ok_or_else(refused)?.canonicalize()?;
    let mut chain = Chain::open(&root)?;
    chain.push(OsStr::new(".gil"), false)?;
    private_dir(chain.dir()).map_err(wrap)?;
    for name in ["state.yaml", "project.lock"] {
        match open_file(&chain.child(OsStr::new(name))?, READ, SHARE_ALL, OPEN_EXISTING) {
            Ok(file) => inherited_only(&file, false).map_err(|e| wrap(io::Error::other(format!("{name}: {e}"))))?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    for parts in [
        &["artifacts"][..],
        &["artifacts", "blobs"],
        &["artifacts", "blobs", "sha256"],
        &["artifacts", "manifests"],
        &["artifacts", "manifests", "sha256"],
        &["artifacts", "tmp"],
        &["restore"],
    ] {
        let mut below = Chain::open(chain.path())?;
        let mut present = true;
        for part in parts {
            match below.push(OsStr::new(part), false) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    present = false;
                    break;
                }
                Err(e) => return Err(e),
            }
        }
        if present {
            inherited_only(below.dir(), true).map_err(|e| wrap(io::Error::other(format!("{}: {e}", parts.join("/")))))?;
        }
    }
    Ok(())
}

// ── Handle-first reservation (Windows contract §4 W1, design A) ──────────────────────────────

use windows_sys::Wdk::Foundation::OBJECT_ATTRIBUTES;
use windows_sys::Wdk::Storage::FileSystem::{
    FILE_CREATE, FILE_DIRECTORY_FILE, FILE_OPEN, FILE_OPEN_REPARSE_POINT as NT_OPEN_REPARSE_POINT,
    FILE_SYNCHRONOUS_IO_NONALERT, NtCreateFile,
};
use windows_sys::Win32::Foundation::{OBJ_CASE_INSENSITIVE, UNICODE_STRING};
use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;

fn nt_error(status: i32) -> io::Error {
    match status as u32 {
        0xC000_0035 => io::Error::new(io::ErrorKind::AlreadyExists, "object name collision"),
        0xC000_0022 => io::Error::new(io::ErrorKind::PermissionDenied, "access denied"),
        0xC000_0034 | 0xC000_003A => io::Error::new(io::ErrorKind::NotFound, "object not found"),
        0xC000_0043 => io::Error::from_raw_os_error(32),
        other => io::Error::other(format!("NTSTATUS {other:#010x}")),
    }
}

/// One relative component under an already held directory handle: the kernel resolves only
/// `name` inside `parent`, never a path, and the handle to the created or opened object is the
/// result of that same call. Reparse points are opened as themselves and then refused.
fn nt_dir_at(
    parent: &File,
    name: &OsStr,
    access: u32,
    share: u32,
    disposition: u32,
    descriptor: *const c_void,
) -> io::Result<File> {
    plain_name(name)?;
    let mut buffer: Vec<u16> = name.encode_wide().collect();
    let bytes = u16::try_from(buffer.len() * size_of::<u16>()).map_err(|_| refused())?;
    let unicode = UNICODE_STRING { Length: bytes, MaximumLength: bytes, Buffer: buffer.as_mut_ptr() };
    let attributes = OBJECT_ATTRIBUTES {
        Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: parent.as_raw_handle(),
        ObjectName: &unicode,
        Attributes: OBJ_CASE_INSENSITIVE,
        SecurityDescriptor: descriptor.cast(),
        SecurityQualityOfService: std::ptr::null(),
    };
    let mut handle = null_mut();
    let mut block: IO_STATUS_BLOCK = unsafe { zeroed() };
    // SAFETY: all pointers outlive the call; a successful handle is owned exactly once.
    let status = unsafe {
        NtCreateFile(
            &mut handle,
            access | SYNCHRONIZE,
            &attributes,
            &mut block,
            std::ptr::null(),
            FILE_ATTRIBUTE_NORMAL,
            share,
            disposition,
            FILE_DIRECTORY_FILE | NT_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
            std::ptr::null(),
            0,
        )
    };
    if status < 0 {
        return Err(nt_error(status));
    }
    let file = unsafe { File::from_raw_handle(handle) };
    ordinary(&file, true)?;
    Ok(file)
}

/// Exclusive, owner-only creation of `name` under the pinned `parent`, returning the handle of
/// the object this call created. Any existing entry of any kind fails the call before it is
/// touched. The handle carries DELETE so a failed verification can remove this object only.
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
pub(crate) fn create_private_dir_at(parent: &File, name: &OsStr) -> io::Result<File> {
    let sd = private_descriptor()?;
    nt_dir_at(
        parent,
        name,
        READ | READ_CONTROL | WRITE_DAC | WRITE_OWNER | DELETE,
        SHARE_PINNED,
        FILE_CREATE,
        sd.0,
    )
}

/// Open the existing `name` under the pinned `parent` (one relative component, no reparse).
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
pub(crate) fn open_dir_at(parent: &File, name: &OsStr, share: u32) -> io::Result<File> {
    nt_dir_at(parent, name, READ, share, FILE_OPEN, std::ptr::null())
}

/// A second handle to the same object with different access and sharing, obtained without a
/// name: `NtCreateFile` with `RootDirectory` = the original handle and an empty object name
/// reopens that object itself (Win32 `ReOpenFile` refused directory handles on the runner,
/// CI run 39161844632).
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
pub(crate) fn reopen(file: &File, access: u32, share: u32) -> io::Result<File> {
    let unicode = UNICODE_STRING { Length: 0, MaximumLength: 0, Buffer: null_mut() };
    let attributes = OBJECT_ATTRIBUTES {
        Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: file.as_raw_handle(),
        ObjectName: &unicode,
        Attributes: OBJ_CASE_INSENSITIVE,
        SecurityDescriptor: std::ptr::null(),
        SecurityQualityOfService: std::ptr::null(),
    };
    let mut handle = null_mut();
    let mut block: IO_STATUS_BLOCK = unsafe { zeroed() };
    let status = unsafe {
        NtCreateFile(
            &mut handle,
            access | SYNCHRONIZE,
            &attributes,
            &mut block,
            std::ptr::null(),
            FILE_ATTRIBUTE_NORMAL,
            share,
            FILE_OPEN,
            FILE_DIRECTORY_FILE | NT_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
            std::ptr::null(),
            0,
        )
    };
    if status < 0 {
        return Err(nt_error(status));
    }
    let again = unsafe { File::from_raw_handle(handle) };
    if id(&again)? != id(file)? {
        return Err(refused());
    }
    Ok(again)
}


// ── Location support for new image Projects (Windows contract §4 W1 0, §8 1b) ────────────────
//
// Checked **before** anything is created. Detection has known misses and false positives
// (contract §8 1b); an undetectable or unverifiable location is a refusal, never "safe". A plain
// inherited parent ACL is not a reason to refuse: the new `.gil` gets its own protected DACL.

/// WinBase.h `DRIVE_REMOTE`.
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
const NETWORK_DRIVE: u32 = 4;
/// Cloud-files (placeholder / hydration) attributes set by sync providers such as OneDrive
/// Files On-Demand: RECALL_ON_OPEN, PINNED, UNPINNED, RECALL_ON_DATA_ACCESS.
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
const CLOUD_ATTRIBUTES: u32 = 0x4_0000 | 0x8_0000 | 0x10_0000 | 0x40_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
pub(crate) enum LocationKind {
    /// Under a root named by the OneDrive / OneDriveConsumer / OneDriveCommercial variables.
    OneDriveEnv,
    /// Under a sync root the user's registry hive declares (Explorer sync-engine registration or
    /// an OneDrive account's user folder); independent of the process environment.
    SyncRoot,
    /// The folder or an ancestor carries cloud-files attributes (a sync provider is active there).
    CloudFiles,
    /// UNC path or a drive whose type is remote.
    Network,
    /// A drive that is not a local fixed drive (removable, CD, RAM disk, unknown).
    NotFixed,
    /// Local fixed drive whose file system is not NTFS.
    NotNtfs,
    /// A reparse point (junction, symlink, mount point, other provider) in the selected path.
    Reparse,
    /// The checks themselves could not be completed; not treated as supported.
    Unverifiable,
}

/// What a refusal rests on (contract §8 1b): an attribute or type the OS reported for this
/// path, a value the environment declares (which may be stale), or a check that failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
pub(crate) enum Evidence {
    Observed,
    Declared,
    DeclaredByRegistry,
    CheckFailed,
}

#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
impl Evidence {
    pub(crate) fn code(self) -> &'static str {
        match self {
            Self::Observed => "observed",
            Self::Declared => "declared_by_environment",
            Self::DeclaredByRegistry => "declared_by_registry",
            Self::CheckFailed => "check_failed",
        }
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
pub(crate) struct UnsupportedLocation {
    pub kind: LocationKind,
    pub detail: String,
}

#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
impl UnsupportedLocation {
    pub(crate) fn evidence(&self) -> Evidence {
        match self.kind {
            LocationKind::OneDriveEnv => Evidence::Declared,
            LocationKind::SyncRoot => Evidence::DeclaredByRegistry,
            LocationKind::Unverifiable => Evidence::CheckFailed,
            _ => Evidence::Observed,
        }
    }
}

#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
impl LocationKind {
    pub(crate) fn code(self) -> &'static str {
        match self {
            Self::OneDriveEnv => "onedrive",
            Self::SyncRoot => "sync_root",
            Self::CloudFiles => "cloud_files",
            Self::Network => "network",
            Self::NotFixed => "not_fixed_drive",
            Self::NotNtfs => "not_ntfs",
            Self::Reparse => "reparse_point",
            Self::Unverifiable => "unverifiable",
        }
    }
    /// Short notice for a non-developer: support scope, never a claim about the cause of an error.
    pub(crate) fn notice(self) -> &'static str {
        match self {
            Self::OneDriveEnv => "선택한 폴더는 OneDrive 설정이 백업 위치로 선언한 폴더 안에 있습니다. 현재 GIL 검수판은 이 위치에서 기록 보존을 보장하지 못합니다. OneDrive 설정은 그대로 두고, 이 PC의 별도 작업 폴더를 준비할까요?",
            Self::SyncRoot => "선택한 폴더는 이 PC에 등록된 동기화 루트(OneDrive 등 동기화 엔진이 등록한 폴더) 안에 있습니다. 현재 GIL 검수판은 이 위치에서 기록 보존을 보장하지 못합니다. 동기화 설정은 그대로 두고, 이 PC의 별도 작업 폴더를 준비할까요?",
            Self::CloudFiles => "선택한 폴더는 클라우드 동기화(파일 온디맨드) 위치입니다. 현재 GIL 검수판은 이 위치에서 기록 보존을 보장하지 못합니다. 동기화 설정은 그대로 두고, 이 PC의 별도 작업 폴더를 준비할까요?",
            Self::Network => "선택한 폴더는 네트워크 경로입니다. 현재 GIL 검수판은 이 PC의 로컬 고정 NTFS 폴더만 지원합니다. 이 PC의 별도 작업 폴더를 준비할까요?",
            Self::NotFixed => "선택한 폴더는 이동식 드라이브이거나 고정 드라이브가 아닙니다. 현재 GIL 검수판은 이 PC의 로컬 고정 NTFS 폴더만 지원합니다. 별도 작업 폴더를 준비할까요?",
            Self::NotNtfs => "선택한 폴더의 드라이브는 NTFS가 아닙니다. 현재 GIL 검수판은 로컬 고정 NTFS 폴더만 지원합니다. 별도 작업 폴더를 준비할까요?",
            Self::Reparse => "선택한 경로에 링크(junction·symlink)가 포함돼 있습니다. 현재 GIL 검수판은 링크를 거치지 않는 로컬 폴더만 지원합니다. 별도 작업 폴더를 준비할까요?",
            Self::Unverifiable => "선택한 폴더가 지원 범위인지 확인하지 못했습니다. 확인되지 않은 위치에는 기록을 만들지 않습니다. 이 PC의 별도 작업 폴더를 준비할까요?",
        }
    }
}

#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
fn unsupported(kind: LocationKind, detail: impl Into<String>) -> UnsupportedLocation {
    UnsupportedLocation { kind, detail: detail.into() }
}

/// One declared synchronization root and where it was declared.
#[derive(Debug, Clone)]
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
pub(crate) struct SyncRoot {
    pub path: PathBuf,
    pub source: &'static str,
}

/// How one source answered. A failed or partially failed query is never "nothing registered".
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
pub(crate) enum SourceStatus {
    /// Queried normally: `roots` usable roots, `stale` declared roots whose folder does not exist
    /// (recorded, not matched — a documented miss); `key_present` is false when the optional
    /// registration key does not exist at all (nothing registered, not a failure).
    Queried { roots: usize, stale: usize, key_present: bool },
    /// The place to look does not exist (for the registry: the hive of the account this server
    /// runs as is not loaded). Reported and treated as no answer, never as a failure.
    Absent(String),
    /// Access denied, partial enumeration, or a value that could not be interpreted.
    Failed(String),
}

#[derive(Debug, Clone)]
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
pub(crate) struct SourceReport {
    pub source: &'static str,
    pub status: SourceStatus,
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
pub(crate) struct SyncRoots {
    pub roots: Vec<SyncRoot>,
    pub reports: Vec<SourceReport>,
}

pub(crate) const SOURCE_ENV: &str = "environment:OneDrive*";
pub(crate) const SOURCE_SYNC_ENGINES: &str = "registry:SyncEngines\\Providers\\*\\*\\MountPoint";
pub(crate) const SOURCE_ONEDRIVE_ACCOUNTS: &str = "registry:Microsoft\\OneDrive\\Accounts\\*\\UserFolder";

/// Collect declared roots from every source. Declared paths are canonicalized when their folder
/// exists; absent folders count as stale.
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
fn collect(out: &mut SyncRoots, source: &'static str, declared: Result<Declared, String>) {
    match declared {
        Err(why) => out.reports.push(SourceReport { source, status: SourceStatus::Failed(why) }),
        Ok(Declared { values, key_present }) => {
            let (mut roots, mut stale) = (0, 0);
            for value in values {
                match PathBuf::from(&value).canonicalize() {
                    Ok(path) => { out.roots.push(SyncRoot { path, source }); roots += 1; }
                    Err(_) => stale += 1,
                }
            }
            out.reports.push(SourceReport { source, status: SourceStatus::Queried { roots, stale, key_present } });
        }
    }
}

/// What a source handed back before path resolution; `key_present` is false when the optional
/// registration key does not exist (nothing registered).
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
pub(crate) struct Declared {
    pub values: Vec<String>,
    pub key_present: bool,
}

/// `OneDrive` / `OneDriveConsumer` / `OneDriveCommercial` of **this process**. An MCP server may
/// run with a minimal environment, so an empty answer here says nothing about the machine.
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
fn env_declared_roots() -> Result<Declared, String> {
    let values: Vec<String> = ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"]
        .iter()
        .filter_map(|key| std::env::var_os(key))
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string_lossy().into_owned())
        .collect();
    let key_present = !values.is_empty();
    Ok(Declared { values, key_present })
}

/// Every source, with its status. Used by `gil_start` and by the reservation itself.
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
pub(crate) fn sync_roots() -> SyncRoots {
    let mut out = SyncRoots::default();
    collect(&mut out, SOURCE_ENV, env_declared_roots());
    match registry::user_hive() {
        Err(why) => {
            out.reports.push(SourceReport { source: SOURCE_SYNC_ENGINES, status: SourceStatus::Failed(why.clone()) });
            out.reports.push(SourceReport { source: SOURCE_ONEDRIVE_ACCOUNTS, status: SourceStatus::Failed(why) });
        }
        Ok(None) => {
            let what = "the registry hive of the account this server runs as is not loaded".to_string();
            out.reports.push(SourceReport { source: SOURCE_SYNC_ENGINES, status: SourceStatus::Absent(what.clone()) });
            out.reports.push(SourceReport { source: SOURCE_ONEDRIVE_ACCOUNTS, status: SourceStatus::Absent(what) });
        }
        Ok(Some(hive)) => {
            collect(&mut out, SOURCE_SYNC_ENGINES, registry::sync_engine_mount_points(&hive));
            collect(&mut out, SOURCE_ONEDRIVE_ACCOUNTS, registry::onedrive_user_folders(&hive));
        }
    }
    out
}

/// What a passed check actually looked at, for the `location_check` report. It states evidence
/// and limits; it never claims that no synchronization exists.
#[derive(Debug, Clone)]
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
pub(crate) struct LocationReport {
    pub sources: Vec<SourceReport>,
    pub components_checked: usize,
    pub drive: &'static str,
    pub filesystem: String,
}

#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
impl LocationReport {
    /// Documented detection limits (contract §8 1b), repeated with every pass.
    pub(crate) const LIMITS: &'static str = "확인한 소스에서 동기화 루트가 발견되지 않았고 경로 구성 요소에 reparse·클라우드 속성이 없었다는 뜻이다. 다른 동기화 제품, 등록되지 않은 동기화, Files On-Demand 가 꺼진 폴더, 이 프로세스에 전달되지 않은 환경 변수는 보이지 않는다. 동기화가 없음을 보장하지 않는다.";
}

/// Registry sources, read from the hive of **the account this server runs as** (the process
/// token's SID under `HKEY_USERS`), not from whatever `HKEY_CURRENT_USER` maps to. Whether that
/// account is the person in the chat is a separate question this code does not answer. Basis: `Software\SyncEngines\Providers` is the
/// Explorer sync-root registration described in Microsoft's cloud storage provider integration
/// guidance (navigation pane / sync root registration); `Software\Microsoft\OneDrive\Accounts\*\UserFolder`
/// is an OneDrive client convention, not a documented interface. Both are read-only here.
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
mod registry {
    use super::*;
    use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, ERROR_SUCCESS};
    use windows_sys::Win32::System::Registry::{HKEY, HKEY_USERS, KEY_READ, REG_EXPAND_SZ, REG_SZ, RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW};

    pub(super) struct Key(HKEY);
    impl Key {
        #[cfg(test)]
        pub(super) fn raw(&self) -> HKEY { self.0 }
    }
    impl Drop for Key {
        fn drop(&mut self) {
            unsafe { RegCloseKey(self.0); }
        }
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// `Ok(None)` only when the key does not exist; every other failure is an error.
    pub(super) fn open(parent: HKEY, sub: &str) -> Result<Option<Key>, String> {
        let mut out: HKEY = null_mut();
        let status = unsafe { RegOpenKeyExW(parent, wide(sub).as_ptr(), 0, KEY_READ, &mut out) };
        match status {
            ERROR_SUCCESS => Ok(Some(Key(out))),
            ERROR_FILE_NOT_FOUND => Ok(None),
            code => Err(format!("open {sub}: error {code}")),
        }
    }

    pub(super) fn subkeys(key: &Key) -> Result<Vec<String>, String> {
        let mut names = Vec::new();
        let mut index = 0;
        loop {
            let mut name = [0u16; 256];
            let mut len = name.len() as u32;
            let status = unsafe { RegEnumKeyExW(key.0, index, name.as_mut_ptr(), &mut len, null_mut(), null_mut(), null_mut(), null_mut()) };
            match status {
                ERROR_SUCCESS => names.push(String::from_utf16_lossy(&name[..len as usize])),
                ERROR_NO_MORE_ITEMS => return Ok(names),
                code => return Err(format!("enumerate subkeys: error {code} at index {index}")),
            }
            index += 1;
        }
    }

    /// `Ok(None)` only when the value does not exist. A value that is not REG_SZ/REG_EXPAND_SZ,
    /// or whose `%VAR%` references cannot be expanded from this process, is an error.
    pub(super) fn string_value(key: &Key, name: &str) -> Result<Option<String>, String> {
        let name_w = wide(name);
        let mut kind = 0u32;
        let mut bytes = 0u32;
        let status = unsafe { RegQueryValueExW(key.0, name_w.as_ptr(), null_mut(), &mut kind, null_mut(), &mut bytes) };
        match status {
            ERROR_SUCCESS | ERROR_MORE_DATA => {}
            ERROR_FILE_NOT_FOUND => return Ok(None),
            code => return Err(format!("read {name}: error {code}")),
        }
        if kind != REG_SZ && kind != REG_EXPAND_SZ {
            return Err(format!("read {name}: unexpected value type {kind}"));
        }
        let mut data = vec![0u8; bytes as usize + 2];
        let mut got = bytes;
        let status = unsafe { RegQueryValueExW(key.0, name_w.as_ptr(), null_mut(), &mut kind, data.as_mut_ptr(), &mut got) };
        if status != ERROR_SUCCESS {
            return Err(format!("read {name}: error {status}"));
        }
        let units: Vec<u16> = data[..got as usize].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|&u| u != 0).collect();
        let text = String::from_utf16(&units).map_err(|_| format!("read {name}: not valid UTF-16"))?;
        let text = if kind == REG_EXPAND_SZ { expand(&text)? } else { text };
        if text.trim().is_empty() {
            return Err(format!("read {name}: empty value"));
        }
        Ok(Some(text))
    }

    /// `%VAR%` expansion from this process's environment; anything left unexpanded is an error
    /// (a minimal server environment must not silently turn a declared root into nonsense).
    fn expand(text: &str) -> Result<String, String> {
        let mut out = String::new();
        let mut rest = text;
        while let Some(start) = rest.find('%') {
            out.push_str(&rest[..start]);
            let after = &rest[start + 1..];
            let Some(end) = after.find('%') else { return Err(format!("unterminated %VAR% in {text}")); };
            let var = &after[..end];
            match std::env::var_os(var) {
                Some(value) if !value.is_empty() => out.push_str(&value.to_string_lossy()),
                _ => return Err(format!("%{var}% is not set in this process")),
            }
            rest = &after[end + 1..];
        }
        out.push_str(rest);
        Ok(out)
    }

    /// The hive of the account this server runs as: `Ok(None)` when it is not loaded (absent),
    /// `Err` when the token or the open fails (access denied and the like).
    pub(super) fn user_hive() -> Result<Option<Key>, String> {
        #[cfg(test)]
        if super::workspace_hooks::REGISTRY_FAIL.with(Cell::get) {
            return Err("injected hive failure".into());
        }
        let token = super::user().map_err(|e| format!("process token: {e}"))?;
        let sid = super::sid_text(super::sid(&token)).map_err(|e| format!("token SID: {e}"))?;
        open(HKEY_USERS, &sid)
    }

    /// Test-only relocation of both registry sources under a scratch key.
    fn base(sub: &str) -> String {
        #[cfg(test)]
        if let Some(prefix) = super::workspace_hooks::REGISTRY_BASE.with(|c| c.borrow().clone()) {
            return format!("{prefix}\\{sub}");
        }
        format!("Software\\{sub}")
    }

    /// `Software\SyncEngines\Providers\<provider>\<id>\MountPoint` for every registration.
    pub(super) fn sync_engine_mount_points(hive: &Key) -> Result<Declared, String> {
        let Some(providers) = open(hive.0, &base("SyncEngines\\Providers"))? else { return Ok(Declared { values: Vec::new(), key_present: false }); };
        let mut out = Vec::new();
        for provider in subkeys(&providers)? {
            let Some(entries) = open(providers.0, &provider)? else { return Err(format!("provider {provider} vanished during enumeration")); };
            for id in subkeys(&entries)? {
                let Some(entry) = open(entries.0, &id)? else { return Err(format!("registration {provider}\\{id} vanished during enumeration")); };
                if let Some(mount) = string_value(&entry, "MountPoint").map_err(|e| format!("{provider}\\{id}: {e}"))? {
                    out.push(mount);
                }
            }
        }
        Ok(Declared { values: out, key_present: true })
    }

    /// `Software\Microsoft\OneDrive\Accounts\<account>\UserFolder` for every account.
    pub(super) fn onedrive_user_folders(hive: &Key) -> Result<Declared, String> {
        let Some(accounts) = open(hive.0, &base("Microsoft\\OneDrive\\Accounts"))? else { return Ok(Declared { values: Vec::new(), key_present: false }); };
        let mut out = Vec::new();
        for account in subkeys(&accounts)? {
            let Some(entry) = open(accounts.0, &account)? else { return Err(format!("account {account} vanished during enumeration")); };
            if let Some(folder) = string_value(&entry, "UserFolder").map_err(|e| format!("{account}: {e}"))? {
                out.push(folder);
            }
        }
        Ok(Declared { values: out, key_present: true })
    }
}

#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
fn same_component(a: &OsStr, b: &OsStr) -> bool {
    a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
}
/// `path` equals `root` or lies below it, compared component-wise and case-insensitively.
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
pub(crate) fn under(path: &Path, root: &Path) -> bool {
    let (mut p, mut r) = (path.components(), root.components());
    loop {
        match (p.next(), r.next()) {
            (_, None) => return true,
            (None, Some(_)) => return false,
            (Some(a), Some(b)) => {
                if !same_component(a.as_os_str(), b.as_os_str()) {
                    return false;
                }
            }
        }
    }
}

/// Classify a selected folder. `Ok(report)` means every check passed and says what was looked
/// at; any error means refuse. A source that could not be queried refuses as unverifiable.
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
pub(crate) fn classify_location(root: &Path, sync_roots: &SyncRoots) -> Result<LocationReport, UnsupportedLocation> {
    if let Some(failed) = sync_roots.reports.iter().find(|r| matches!(r.status, SourceStatus::Failed(_))) {
        let SourceStatus::Failed(why) = &failed.status else { unreachable!() };
        return Err(unsupported(LocationKind::Unverifiable, format!("sync root source {} could not be queried: {why}", failed.source)));
    }
    if matches!(root.components().next(), Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::UNC(..) | Prefix::VerbatimUNC(..))) {
        return Err(unsupported(LocationKind::Network, "UNC path"));
    }
    let canonical = root
        .canonicalize()
        .map_err(|e| unsupported(LocationKind::Unverifiable, format!("canonicalize: {e}")))?;
    if let Err(e) = absolute(&canonical) {
        return Err(unsupported(LocationKind::Unverifiable, format!("not a drive path: {e}")));
    }
    for sync in &sync_roots.roots {
        if under(&canonical, &sync.path) {
            let kind = if sync.source == SOURCE_ENV { LocationKind::OneDriveEnv } else { LocationKind::SyncRoot };
            return Err(unsupported(kind, format!("under {} ({})", sync.path.display(), sync.source)));
        }
    }
    let drive: PathBuf = canonical.components().take(2).collect();
    let drive_text = wide(&drive).map_err(|e| unsupported(LocationKind::Unverifiable, e.to_string()))?;
    match unsafe { GetDriveTypeW(drive_text.as_ptr()) } {
        LOCAL_FIXED_DRIVE => {}
        NETWORK_DRIVE => return Err(unsupported(LocationKind::Network, "remote drive")),
        other => return Err(unsupported(LocationKind::NotFixed, format!("drive type {other}"))),
    }
    // Walk every component as the entry itself: reparse points and cloud attributes anywhere
    // in the selected path count, and a failed query is unverifiable, not safe.
    let mut at = PathBuf::new();
    let mut last: Option<File> = None;
    let mut components_checked = 0usize;
    for part in canonical.components() {
        at.push(part);
        if matches!(part, Component::Prefix(_)) {
            continue;
        }
        let dir = open_raw(&at, READ, SHARE_ALL, OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS)
            .map_err(|e| unsupported(LocationKind::Unverifiable, format!("{}: {e}", at.display())))?;
        let flags = info(&dir)
            .map_err(|e| unsupported(LocationKind::Unverifiable, format!("{}: {e}", at.display())))?
            .dwFileAttributes;
        if flags & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(unsupported(LocationKind::Reparse, at.display().to_string()));
        }
        if flags & FILE_ATTRIBUTE_DIRECTORY == 0 {
            return Err(unsupported(LocationKind::Unverifiable, format!("{} is not a directory", at.display())));
        }
        if flags & CLOUD_ATTRIBUTES != 0 {
            return Err(unsupported(LocationKind::CloudFiles, format!("{} attributes {flags:#x}", at.display())));
        }
        components_checked += 1;
        last = Some(dir);
    }
    let dir = last.ok_or_else(|| unsupported(LocationKind::Unverifiable, "empty path"))?;
    let mut filesystem = [0u16; 32];
    unsafe {
        if GetVolumeInformationByHandleW(dir.as_raw_handle(), null_mut(), 0, null_mut(), null_mut(), null_mut(), filesystem.as_mut_ptr(), filesystem.len() as u32) == 0 {
            return Err(unsupported(LocationKind::Unverifiable, format!("volume query: {}", io::Error::last_os_error())));
        }
    }
    let filesystem_name = String::from_utf16_lossy(&filesystem).trim_end_matches('\0').to_string();
    if filesystem[..5] != [78, 84, 70, 83, 0] {
        return Err(unsupported(LocationKind::NotNtfs, filesystem_name));
    }
    Ok(LocationReport { sources: sync_roots.reports.clone(), components_checked, drive: "fixed", filesystem: filesystem_name })
}

/// Environment-aware check used by `gil_start` and by the reservation itself.
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
pub(crate) fn unsupported_location(root: &Path) -> Result<LocationReport, UnsupportedLocation> {
    #[cfg(test)]
    if workspace_hooks::UNVERIFIABLE.with(Cell::get) {
        return Err(unsupported(LocationKind::Unverifiable, "injected check failure"));
    }
    classify_location(root, &sync_roots())
}

/// Test-only fault injection for the workspace flow (contract §6 W12-7). Not compiled into
/// product builds.
#[cfg(test)]
pub(crate) mod workspace_hooks {
    use std::cell::{Cell, RefCell};
    use std::path::PathBuf;
    thread_local! {
        /// Replace `%USERPROFILE%` as the allowed range for this thread.
        pub static ALLOWED_ROOT: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
        /// Every location check fails (unverifiable) while set.
        pub static UNVERIFIABLE: Cell<bool> = const { Cell::new(false) };
        /// Fail the workspace folder creation right after the parent was created (once).
        pub static FAIL_AFTER_PARENT: Cell<bool> = const { Cell::new(false) };
        /// Make only the post-creation location check fail (once).
        pub static UNVERIFIABLE_AFTER_CREATE: Cell<bool> = const { Cell::new(false) };
        /// Read both registry sources under this subkey of the user hive instead of `Software`.
        pub static REGISTRY_BASE: RefCell<Option<String>> = const { RefCell::new(None) };
        /// Pretend the user hive cannot be opened.
        pub static REGISTRY_FAIL: Cell<bool> = const { Cell::new(false) };
    }
}
#[cfg(test)]
use std::cell::Cell;

// ── Approved local workspace preparation (Windows contract §4 W1 0) ──────────────────────────

#[derive(Debug, Clone)]
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
pub(crate) struct WorkspacePlan {
    pub parent: PathBuf,
    pub path: PathBuf,
    /// The parent itself does not exist yet and would be created under its (supported) parent.
    pub creates_parent: bool,
}

#[derive(Debug)]
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
pub(crate) enum WorkspaceRefusal {
    InvalidName,
    ParentNotAbsolute,
    /// The parent is not under `%USERPROFILE%` (or the profile is unknown): this is not a tool
    /// for creating folders anywhere.
    OutsideAllowed(PathBuf),
    ParentMissing(PathBuf),
    Unsupported(UnsupportedLocation),
    Exists(PathBuf),
    /// The plan no longer matches what the checks say right before creation.
    PlanChanged,
    Io(io::Error),
}

/// A failed creation: what refused it, and every entry this call made (each with whether it
/// still exists at the time of the report). Entries that existed before are never touched.
#[derive(Debug)]
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
pub(crate) struct WorkspaceFailure {
    pub refusal: WorkspaceRefusal,
    pub created: Vec<(PathBuf, bool)>,
}

/// The allowed range for new workspaces: the user profile, canonicalized.
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
pub(crate) fn allowed_workspace_root() -> io::Result<PathBuf> {
    #[cfg(test)]
    if let Some(root) = workspace_hooks::ALLOWED_ROOT.with(|c| c.borrow().clone()) {
        return root.canonicalize();
    }
    let profile = std::env::var_os("USERPROFILE").filter(|v| !v.is_empty()).ok_or_else(refused)?;
    PathBuf::from(profile).canonicalize()
}

/// `%USERPROFILE%\gil-projects`: the default place for a new local workspace.
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
pub(crate) fn default_workspace_parent() -> io::Result<PathBuf> {
    let profile = std::env::var_os("USERPROFILE").filter(|v| !v.is_empty()).ok_or_else(refused)?;
    Ok(PathBuf::from(profile).join("gil-projects"))
}

#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
fn workspace_name_ok(name: &OsStr) -> bool {
    let text = name.to_string_lossy();
    if plain_name(name).is_err() || text.ends_with(['.', ' ']) || text.chars().any(char::is_control) {
        return false;
    }
    let upper = text.to_ascii_uppercase();
    let stem = upper.split('.').next().unwrap_or("");
    !(matches!(stem, "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")) && stem.as_bytes()[3].is_ascii_digit()))
}

/// Decide exactly what would be created, creating nothing. Shown to the user before approval.
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
pub(crate) fn plan_workspace(parent: &Path, name: &OsStr) -> Result<WorkspacePlan, WorkspaceRefusal> {
    if !workspace_name_ok(name) {
        return Err(WorkspaceRefusal::InvalidName);
    }
    if !parent.is_absolute() {
        return Err(WorkspaceRefusal::ParentNotAbsolute);
    }
    if parent.components().any(|c| matches!(c, Component::ParentDir | Component::CurDir)) {
        return Err(WorkspaceRefusal::ParentNotAbsolute);
    }
    let allowed = allowed_workspace_root().map_err(|_| WorkspaceRefusal::OutsideAllowed(parent.to_path_buf()))?;
    let creates_parent = match std::fs::symlink_metadata(parent) {
        Ok(meta) if meta.is_dir() && !meta.is_symlink() => false,
        Ok(_) => return Err(WorkspaceRefusal::Unsupported(unsupported(LocationKind::Reparse, parent.display().to_string()))),
        Err(e) if e.kind() == io::ErrorKind::NotFound => true,
        Err(e) => return Err(WorkspaceRefusal::Io(e)),
    };
    let checked = if creates_parent {
        let above = parent.parent().ok_or(WorkspaceRefusal::ParentNotAbsolute)?;
        match std::fs::symlink_metadata(above) {
            Ok(meta) if meta.is_dir() && !meta.is_symlink() => above.to_path_buf(),
            Ok(_) => return Err(WorkspaceRefusal::Unsupported(unsupported(LocationKind::Reparse, above.display().to_string()))),
            Err(_) => return Err(WorkspaceRefusal::ParentMissing(above.to_path_buf())),
        }
    } else {
        parent.to_path_buf()
    };
    // Allowed range: the existing part, canonicalized (the location check below refuses any
    // reparse point in it), must lie under the user profile.
    let canonical = checked.canonicalize().map_err(WorkspaceRefusal::Io)?;
    if !under(&canonical, &allowed) {
        return Err(WorkspaceRefusal::OutsideAllowed(parent.to_path_buf()));
    }
    unsupported_location(&checked).map_err(WorkspaceRefusal::Unsupported)?;
    let path = parent.join(name);
    match std::fs::symlink_metadata(&path) {
        Ok(_) => return Err(WorkspaceRefusal::Exists(path)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(WorkspaceRefusal::Io(e)),
    }
    Ok(WorkspacePlan { parent: parent.to_path_buf(), path, creates_parent })
}

/// Create exactly the planned folder(s), each exclusively (an entry appearing in between is a
/// refusal, never reused), with the ordinary inherited ACL of a user folder. The Project's own
/// `.gil` gets its protected DACL later, at `gil_start`. The plan is re-derived right before
/// creation (location, ancestors, conflicts); any difference refuses without creating.
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
pub(crate) fn create_workspace(plan: &WorkspacePlan) -> Result<Vec<PathBuf>, WorkspaceFailure> {
    let mut created: Vec<PathBuf> = Vec::new();
    let outcome = (|| -> Result<(), WorkspaceRefusal> {
        let name = plan.path.file_name().ok_or(WorkspaceRefusal::InvalidName)?;
        let now = plan_workspace(&plan.parent, name)?;
        if now.path != plan.path || now.parent != plan.parent || now.creates_parent != plan.creates_parent {
            return Err(WorkspaceRefusal::PlanChanged);
        }
        let mut make = |at: &Path| -> Result<(), WorkspaceRefusal> {
            let text = wide(at).map_err(WorkspaceRefusal::Io)?;
            if unsafe { CreateDirectoryW(text.as_ptr(), null_mut()) } == 0 {
                let e = io::Error::last_os_error();
                return Err(if e.kind() == io::ErrorKind::AlreadyExists { WorkspaceRefusal::Exists(at.to_path_buf()) } else { WorkspaceRefusal::Io(e) });
            }
            created.push(at.to_path_buf());
            Ok(())
        };
        if plan.creates_parent {
            make(&plan.parent)?;
        }
        #[cfg(test)]
        if workspace_hooks::FAIL_AFTER_PARENT.with(Cell::take) {
            return Err(WorkspaceRefusal::Io(io::Error::new(io::ErrorKind::PermissionDenied, "injected after parent")));
        }
        make(&plan.path)?;
        #[cfg(test)]
        if workspace_hooks::UNVERIFIABLE_AFTER_CREATE.with(Cell::take) {
            return Err(WorkspaceRefusal::Unsupported(unsupported(LocationKind::Unverifiable, "injected after creation")));
        }
        // The result must itself pass the location check (no reparse, local fixed NTFS, no sync root).
        unsupported_location(&plan.path).map(|_| ()).map_err(WorkspaceRefusal::Unsupported)
    })();
    match outcome {
        Ok(()) => Ok(created),
        Err(refusal) => Err(WorkspaceFailure {
            refusal,
            created: created.into_iter().map(|p| { let remains = std::fs::symlink_metadata(&p).is_ok(); (p, remains) }).collect(),
        }),
    }
}

#[cfg(test)]
mod sync_root_tests {
    //! Windows contract §4 W1 0 / §8 1b: sync roots from the user hive, with found / none /
    //! failed kept apart. Registry fixtures live under a scratch key of the current user's hive
    //! and are removed afterwards; nothing outside it is touched.
    use super::*;
    use std::fs;
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{HKEY, HKEY_CURRENT_USER, KEY_ALL_ACCESS, REG_DWORD, REG_EXPAND_SZ, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey, RegCreateKeyExW, RegDeleteKeyW, RegDeleteTreeW, RegSetValueExW};

    fn w(s: &str) -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() }
    struct Scratch { base: String, dir: PathBuf }
    impl Scratch {
        fn new(label: &str) -> Self {
            static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let base = format!("Software\\GilTest\\sync-{label}-{}-{n}", std::process::id());
            let dir = std::env::temp_dir().join(format!("gil-sync-{label}-{}-{n}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            workspace_hooks::REGISTRY_BASE.with(|c| *c.borrow_mut() = Some(base.clone()));
            Self { base, dir }
        }
        fn set(&self, sub: &str, name: &str, kind: u32, data: &[u8]) {
            unsafe {
                let mut key: HKEY = null_mut();
                let path = w(&format!("{}\\{sub}", self.base));
                assert_eq!(RegCreateKeyExW(HKEY_CURRENT_USER, path.as_ptr(), 0, null_mut(), REG_OPTION_NON_VOLATILE, KEY_ALL_ACCESS, null_mut(), &mut key, null_mut()), ERROR_SUCCESS);
                assert_eq!(RegSetValueExW(key, w(name).as_ptr(), 0, kind, data.as_ptr(), data.len() as u32), ERROR_SUCCESS);
                RegCloseKey(key);
            }
        }
        fn set_sz(&self, sub: &str, name: &str, value: &str, kind: u32) {
            let units: Vec<u8> = w(value).iter().flat_map(|u| u.to_le_bytes()).collect();
            self.set(sub, name, kind, &units);
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            workspace_hooks::REGISTRY_BASE.with(|c| *c.borrow_mut() = None);
            workspace_hooks::REGISTRY_FAIL.with(|c| c.set(false));
            unsafe {
                // Only this run's key; the shared `Software\\GilTest` parent goes only when empty.
                RegDeleteTreeW(HKEY_CURRENT_USER, w(&self.base).as_ptr());
                RegDeleteKeyW(HKEY_CURRENT_USER, w("Software\\GilTest").as_ptr());
            }
            let _ = fs::remove_dir_all(&self.dir);
        }
    }
    fn status(roots: &SyncRoots, source: &str) -> SourceStatus {
        roots.reports.iter().find(|r| r.source == source).map(|r| r.status.clone()).expect(source)
    }

    #[test]
    fn registered_sync_engine_root_refuses_below_it_and_only_below_it() {
        let s = Scratch::new("engine");
        let root = s.dir.join("내 동기화 폴더 (회사)");
        let sibling = s.dir.join("내 동기화 폴더 (회사)-backup");
        fs::create_dir_all(root.join("바탕 화면").join("test3")).unwrap();
        fs::create_dir_all(&sibling).unwrap();
        s.set_sz("SyncEngines\\Providers\\GilTestProvider\\{id-1}", "MountPoint", &format!("{}\\", root.display()), REG_SZ);
        let roots = sync_roots();
        assert_eq!(status(&roots, SOURCE_SYNC_ENGINES), SourceStatus::Queried { roots: 1, stale: 0, key_present: true });
        assert_eq!(status(&roots, SOURCE_ONEDRIVE_ACCOUNTS), SourceStatus::Queried { roots: 0, stale: 0, key_present: false });
        let refused = classify_location(&root.join("바탕 화면").join("test3"), &roots).unwrap_err();
        assert_eq!(refused.kind, LocationKind::SyncRoot);
        assert_eq!(refused.evidence(), Evidence::DeclaredByRegistry);
        assert!(refused.detail.contains(SOURCE_SYNC_ENGINES), "{}", refused.detail);
        assert_eq!(classify_location(&root, &roots).unwrap_err().kind, LocationKind::SyncRoot, "the root itself counts");
        let passed = classify_location(&sibling, &roots).expect("a sibling with a longer name is outside the root");
        assert_eq!(passed.sources.len(), 3);
        assert!(passed.components_checked >= 3);
    }

    #[test]
    fn onedrive_accounts_with_a_stale_and_an_expandable_entry_are_reported_apart() {
        let s = Scratch::new("accounts");
        let live = s.dir.join("OneDrive - 개인");
        fs::create_dir_all(live.join("문서")).unwrap();
        s.set_sz("Microsoft\\OneDrive\\Accounts\\Personal", "UserFolder", &live.display().to_string(), REG_SZ);
        s.set_sz("Microsoft\\OneDrive\\Accounts\\Business1", "UserFolder", &s.dir.join("gone").display().to_string(), REG_SZ);
        // An expandable value resolved through this process's environment (%TEMP% is set).
        let rel = live.strip_prefix(std::env::temp_dir()).unwrap().display().to_string();
        s.set_sz("SyncEngines\\Providers\\P\\{id}", "MountPoint", &format!("%TEMP%\\{rel}"), REG_EXPAND_SZ);
        let roots = sync_roots();
        assert_eq!(status(&roots, SOURCE_ONEDRIVE_ACCOUNTS), SourceStatus::Queried { roots: 1, stale: 1, key_present: true });
        assert_eq!(status(&roots, SOURCE_SYNC_ENGINES), SourceStatus::Queried { roots: 1, stale: 0, key_present: true });
        assert_eq!(classify_location(&live.join("문서"), &roots).unwrap_err().kind, LocationKind::SyncRoot);
        assert!(classify_location(&s.dir, &roots).is_ok(), "the parent of a root is not under it");
    }

    #[test]
    fn nested_roots_and_a_root_declared_twice_still_refuse_once() {
        let s = Scratch::new("nested");
        let outer = s.dir.join("outer sync");
        let inner = outer.join("inner sync");
        fs::create_dir_all(inner.join("p")).unwrap();
        s.set_sz("SyncEngines\\Providers\\A\\{1}", "MountPoint", &outer.display().to_string(), REG_SZ);
        s.set_sz("SyncEngines\\Providers\\B\\{2}", "MountPoint", &inner.display().to_string(), REG_SZ);
        s.set_sz("Microsoft\\OneDrive\\Accounts\\X", "UserFolder", &outer.display().to_string().to_uppercase(), REG_SZ);
        let roots = sync_roots();
        assert_eq!(roots.roots.len(), 3);
        assert_eq!(classify_location(&inner.join("p"), &roots).unwrap_err().kind, LocationKind::SyncRoot);
    }

    #[test]
    fn nothing_registered_is_queried_normally_not_a_failure() {
        let s = Scratch::new("none");
        let roots = sync_roots();
        assert_eq!(status(&roots, SOURCE_SYNC_ENGINES), SourceStatus::Queried { roots: 0, stale: 0, key_present: false }, "an absent optional key is not a failure");
        assert_eq!(status(&roots, SOURCE_ONEDRIVE_ACCOUNTS), SourceStatus::Queried { roots: 0, stale: 0, key_present: false });
        let report = classify_location(&s.dir, &roots).expect("no signal passes");
        assert!(LocationReport::LIMITS.contains("보장하지 않는다"));
        assert_eq!(report.drive, "fixed");
    }

    #[test]
    fn a_hive_that_cannot_be_opened_refuses_as_unverifiable() {
        let s = Scratch::new("hive-fail");
        workspace_hooks::REGISTRY_FAIL.with(|c| c.set(true));
        let roots = sync_roots();
        assert!(matches!(status(&roots, SOURCE_SYNC_ENGINES), SourceStatus::Failed(_)));
        assert!(matches!(status(&roots, SOURCE_ONEDRIVE_ACCOUNTS), SourceStatus::Failed(_)));
        let refused = classify_location(&s.dir, &roots).unwrap_err();
        assert_eq!(refused.kind, LocationKind::Unverifiable);
        assert_eq!(refused.evidence(), Evidence::CheckFailed);
        assert!(refused.detail.contains("could not be queried"), "{}", refused.detail);
    }

    #[test]
    fn a_value_that_cannot_be_interpreted_is_a_failure_not_an_empty_answer() {
        let s = Scratch::new("badvalue");
        s.set("SyncEngines\\Providers\\P\\{1}", "MountPoint", REG_DWORD, &1u32.to_le_bytes());
        let roots = sync_roots();
        assert!(matches!(status(&roots, SOURCE_SYNC_ENGINES), SourceStatus::Failed(ref why) if why.contains("unexpected value type")));
        assert_eq!(classify_location(&s.dir, &roots).unwrap_err().kind, LocationKind::Unverifiable);
        let s2 = Scratch::new("unexpandable");
        s2.set_sz("Microsoft\\OneDrive\\Accounts\\P", "UserFolder", "%GIL_TEST_VARIABLE_THAT_IS_NOT_SET%\\OneDrive", REG_EXPAND_SZ);
        let roots = sync_roots();
        assert!(matches!(status(&roots, SOURCE_ONEDRIVE_ACCOUNTS), SourceStatus::Failed(ref why) if why.contains("not set in this process")));
        assert_eq!(classify_location(&s2.dir, &roots).unwrap_err().kind, LocationKind::Unverifiable);
    }

    #[test]
    fn the_hive_read_is_the_process_tokens_user_hive() {
        let s = Scratch::new("hive");
        s.set_sz("Probe", "Marker", "written-through-HKCU", REG_SZ);
        let hive = registry::user_hive().expect("no failure").expect("the hive of the account the tests run as is loaded");
        let probe = registry::open(hive_key(&hive), &format!("{}\\Probe", s.base)).unwrap().expect("visible through HKEY_USERS\\<token SID>");
        assert_eq!(registry::string_value(&probe, "Marker").unwrap().as_deref(), Some("written-through-HKCU"));
    }
    fn hive_key(k: &registry::Key) -> windows_sys::Win32::System::Registry::HKEY { k.raw() }
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
