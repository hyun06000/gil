//! Local NTFS only. Never shell out, weaken an existing ACL, or follow reparse points.
//! Parent handles deny rename/delete for the entire settings operation. Files are
//! born with an explicit protected current-user DACL, before any bytes are written.
use std::{ffi::c_void, fs::{self, File, OpenOptions}, io::{self, Read, Write},
    mem::{size_of, zeroed}, os::windows::{ffi::OsStrExt, fs::OpenOptionsExt, io::{AsRawHandle, FromRawHandle}},
    path::{Component, Path, PathBuf, Prefix}, ptr::null_mut};
use windows_sys::Win32::{Foundation::*, Security::{*, Authorization::*},
    Storage::FileSystem::*, System::Threading::*};

fn refused() -> io::Error { io::Error::new(io::ErrorKind::PermissionDenied, "unsafe Windows binding storage") }
// WinBase.h DRIVE_FIXED (avoid importing the unrelated WindowsProgramming API surface).
const LOCAL_FIXED_DRIVE: u32 = 3;
fn wide(path: &Path) -> io::Result<Vec<u16>> {
    let mut text: Vec<u16> = path.as_os_str().encode_wide().collect();
    if text.contains(&0) { return Err(refused()); }
    text.push(0); Ok(text)
}
struct Local(*mut c_void);
impl Drop for Local { fn drop(&mut self) { unsafe { LocalFree(self.0); } } }
struct Token(HANDLE);
impl Drop for Token { fn drop(&mut self) { unsafe { CloseHandle(self.0); } } }

// usize storage supplies TOKEN_USER alignment. Its embedded SID points into this buffer.
fn user() -> io::Result<Vec<usize>> {
    unsafe {
        let mut handle = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut handle) == 0 { return Err(io::Error::last_os_error()); }
        let token = Token(handle);
        let mut bytes = 0;
        GetTokenInformation(token.0, TokenUser, null_mut(), 0, &mut bytes);
        if bytes < size_of::<TOKEN_USER>() as u32 || bytes > 65536 { return Err(refused()); }
        let mut buf = vec![0usize; (bytes as usize).div_ceil(size_of::<usize>())];
        if GetTokenInformation(token.0, TokenUser, buf.as_mut_ptr().cast(), bytes, &mut bytes) == 0 { return Err(io::Error::last_os_error()); }
        Ok(buf)
    }
}
fn sid(buf: &[usize]) -> PSID { unsafe { (*(buf.as_ptr().cast::<TOKEN_USER>())).User.Sid } }
fn descriptor() -> io::Result<Local> {
    let user = user()?;
    unsafe {
        let mut text = null_mut();
        if ConvertSidToStringSidW(sid(&user), &mut text) == 0 { return Err(io::Error::last_os_error()); }
        let _text = Local(text.cast());
        let mut len = 0;
        while *text.add(len) != 0 { len += 1; }
        let name = String::from_utf16(std::slice::from_raw_parts(text, len)).map_err(|_| refused())?;
        let sddl: Vec<u16> = format!("O:{name}D:P(A;;FA;;;{name})\0").encode_utf16().collect();
        let mut sd = null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.as_ptr(), 1, &mut sd, null_mut()) == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Local(sd))
    }
}
fn attributes(sd: &Local) -> SECURITY_ATTRIBUTES {
    SECURITY_ATTRIBUTES { nLength: size_of::<SECURITY_ATTRIBUTES>() as u32, lpSecurityDescriptor: sd.0, bInheritHandle: 0 }
}
fn info(file: &File) -> io::Result<BY_HANDLE_FILE_INFORMATION> {
    unsafe {
        let mut info = zeroed();
        if GetFileInformationByHandle(file.as_raw_handle(), &mut info) == 0 { return Err(io::Error::last_os_error()); }
        Ok(info)
    }
}
fn ordinary(file: &File, directory: bool) -> io::Result<()> {
    let flags = info(file)?.dwFileAttributes;
    if flags & FILE_ATTRIBUTE_REPARSE_POINT != 0 || (flags & FILE_ATTRIBUTE_DIRECTORY != 0) != directory { return Err(refused()); }
    Ok(())
}
fn ntfs(file: &File) -> io::Result<()> {
    let mut name = [0u16; 32];
    unsafe {
        if GetVolumeInformationByHandleW(file.as_raw_handle(), null_mut(), 0, null_mut(), null_mut(), null_mut(), name.as_mut_ptr(), name.len() as u32) == 0 {
            return Err(io::Error::last_os_error());
        }
    }
    if &name[..5] != [78, 84, 70, 83, 0] { return Err(refused()); }
    Ok(())
}
fn private(file: &File) -> io::Result<()> {
    let user = user()?;
    unsafe {
        let (mut owner, mut acl, mut sd) = (null_mut(), null_mut(), null_mut());
        let error = GetSecurityInfo(file.as_raw_handle(), SE_FILE_OBJECT, OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner, null_mut(), &mut acl, null_mut(), &mut sd);
        if error != 0 { return Err(io::Error::from_raw_os_error(error as i32)); }
        let sd = Local(sd);
        let (mut control, mut revision) = (0, 0);
        if owner.is_null() || EqualSid(owner, sid(&user)) == 0 || acl.is_null() || (*acl).AceCount != 1 ||
            GetSecurityDescriptorControl(sd.0, &mut control, &mut revision) == 0 || control & SE_DACL_PROTECTED == 0 {
            return Err(refused());
        }
        let mut ace = null_mut();
        if GetAce(acl, 0, &mut ace) == 0 || ace.is_null() { return Err(refused()); }
        let header = &*ace.cast::<ACE_HEADER>();
        // Only a simple, effective allow ACE for the owner. No inherited/object/callback ACEs.
        if header.AceType != 0 || header.AceFlags != 0 || (header.AceSize as usize) < size_of::<ACCESS_ALLOWED_ACE>() { return Err(refused()); }
        let allow = &*ace.cast::<ACCESS_ALLOWED_ACE>();
        let allowed_sid = std::ptr::addr_of!(allow.SidStart).cast_mut().cast();
        if allow.Mask != FILE_ALL_ACCESS || EqualSid(allowed_sid, sid(&user)) == 0 { return Err(refused()); }
    }
    Ok(())
}
fn directory(path: &Path) -> io::Result<File> {
    let file = OpenOptions::new().access_mode(FILE_READ_ATTRIBUTES | READ_CONTROL)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT).open(path)?;
    ordinary(&file, true)?; Ok(file)
}
fn chain(path: &Path, create: bool) -> io::Result<Vec<File>> {
    if !path.is_absolute() || !matches!(path.components().next(), Some(Component::Prefix(p))
        if matches!(p.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))) { return Err(refused()); }
    let mut at = PathBuf::new();
    let mut pins = Vec::new();
    for part in path.components() {
        match part {
            Component::Prefix(_) => { at.push(part); continue; },
            Component::ParentDir | Component::CurDir => return Err(refused()),
            _ => at.push(part),
        }
        if matches!(part, Component::RootDir) {
            let root = wide(&at)?;
            // Reject mapped network/removable drives and unsupported filesystems BEFORE mkdir.
            if unsafe { GetDriveTypeW(root.as_ptr()) } != LOCAL_FIXED_DRIVE { return Err(refused()); }
            ntfs(&directory(&at)?)?;
        }
        let file = match directory(&at) {
            Ok(file) => file,
            Err(e) if create && e.kind() == io::ErrorKind::NotFound => {
                let sd = descriptor()?;
                let name = wide(&at)?;
                if unsafe { CreateDirectoryW(name.as_ptr(), &attributes(&sd)) } == 0 {
                    let error = io::Error::last_os_error();
                    if error.kind() != io::ErrorKind::AlreadyExists { return Err(error); }
                }
                directory(&at)?
            },
            Err(e) => return Err(e),
        };
        pins.push(file);
    }
    ntfs(pins.last().ok_or_else(refused)?)?;
    Ok(pins)
}
pub(super) fn identity(path: &Path) -> io::Result<(u64, u64, u128)> {
    let pins = chain(path, false)?;
    let meta = info(pins.last().ok_or_else(refused)?)?;
    let index = (u64::from(meta.nFileIndexHigh) << 32) | u64::from(meta.nFileIndexLow);
    if index == 0 || index == u64::MAX { return Err(refused()); }
    // NTFS file index is 64 bits. ReFS (128-bit IDs), UNC and reparse paths are refused.
    let created = (u64::from(meta.ftCreationTime.dwHighDateTime) << 32) | u64::from(meta.ftCreationTime.dwLowDateTime);
    let unix_ticks = created.checked_sub(116_444_736_000_000_000).ok_or_else(refused)?;
    Ok((u64::from(meta.dwVolumeSerialNumber), index, u128::from(unix_ticks) * 100))
}
pub(super) struct PrivateDir { path: PathBuf, _pins: Vec<File> }
impl PrivateDir {
    pub fn open(path: &Path, create: bool) -> io::Result<Self> {
        let pins = chain(path, create)?;
        private(pins.last().ok_or_else(refused)?)?;
        Ok(Self { path: path.to_owned(), _pins: pins })
    }
    fn child(&self, path: &Path) -> io::Result<()> {
        if path.parent() != Some(self.path.as_path()) { return Err(refused()); } Ok(())
    }
    pub fn read(&self, path: &Path, max: u64) -> io::Result<Vec<u8>> {
        self.child(path)?;
        let file = OpenOptions::new().read(true).share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT).open(path)?;
        ordinary(&file, false)?; private(&file)?;
        if file.metadata()?.len() > max { return Err(refused()); }
        let mut bytes = Vec::new(); file.take(max + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > max { return Err(refused()); } Ok(bytes)
    }
    pub fn publish(&self, path: &Path, bytes: &[u8]) -> io::Result<bool> {
        self.child(path)?;
        let tmp = self.path.join(format!(".binding-{}-{}.tmp", std::process::id(),
            super::SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
        let sd = descriptor()?;
        let name = wide(&tmp)?;
        // SAFETY: nul-terminated path and descriptor remain live through CreateFileW.
        let raw = unsafe { CreateFileW(name.as_ptr(), GENERIC_WRITE, 0, &attributes(&sd),
            CREATE_NEW, FILE_ATTRIBUTE_NORMAL, null_mut()) };
        if raw == INVALID_HANDLE_VALUE { return Err(io::Error::last_os_error()); }
        // SAFETY: newly created owned handle, transferred exactly once to File.
        let mut file = unsafe { File::from_raw_handle(raw) };
        let written = file.write_all(bytes).and_then(|_| file.sync_all());
        drop(file);
        let result = written.and_then(|()| {
            let (from, to) = (wide(&tmp)?, wide(path)?);
            // Same-directory move, no replacement, no cross-volume copy, flushed bytes first.
            if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), MOVEFILE_WRITE_THROUGH) } != 0 { return Ok(true); }
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::AlreadyExists { Ok(false) } else { Err(error) }
        });
        let _ = fs::remove_file(&tmp); // Only the unique file we created; never the destination.
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("gil-win-private-{}-{}", std::process::id(),
                super::super::SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
            fs::create_dir(&path).unwrap(); Self(path)
        }
    }
    impl Drop for Scratch { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
    #[test]
    fn protected_owner_acl_and_unicode_paths_roundtrip_without_inherited_access() {
        let s = Scratch::new();
        let dir = PrivateDir::open(&s.0.join("한글 설정/private"), true).unwrap();
        let record = dir.path.join("entry.json");
        assert!(dir.publish(&record, b"private").unwrap());
        assert_eq!(dir.read(&record, 100).unwrap(), b"private");
        assert!(!dir.publish(&record, b"replacement").unwrap());
        assert_eq!(dir.read(&record, 100).unwrap(), b"private");
        assert!(dir.read(&record, 2).is_err());
        assert!(dir.read(&s.0.join("outside"), 100).is_err());
        assert!(fs::rename(&dir.path, s.0.join("moved")).is_err(), "directory is pinned while in use");
    }
    #[test]
    fn public_directory_is_refused_without_repairing_its_acl() {
        let s = Scratch::new();
        let path = s.0.join("public"); fs::create_dir(&path).unwrap();
        assert!(PrivateDir::open(&path, true).is_err());
        assert_eq!(fs::read_dir(path).unwrap().count(), 0);
    }
    #[test]
    fn broadened_file_acl_is_refused_and_bytes_are_preserved() {
        let s = Scratch::new(); let dir = PrivateDir::open(&s.0.join("private"), true).unwrap();
        let path = dir.path.join("record.json"); dir.publish(&path, b"original").unwrap();
        let file = OpenOptions::new().access_mode(WRITE_DAC).open(&path).unwrap();
        let sddl: Vec<u16> = "D:P(A;;FA;;;WD)\0".encode_utf16().collect();
        unsafe {
            let mut raw = null_mut();
            assert_ne!(ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.as_ptr(), 1, &mut raw, null_mut()), 0);
            let sd = Local(raw);
            let (mut present, mut defaulted, mut acl) = (0, 0, null_mut());
            assert_ne!(GetSecurityDescriptorDacl(sd.0, &mut present, &mut acl, &mut defaulted), 0);
            assert_eq!(SetSecurityInfo(file.as_raw_handle(), SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION, null_mut(), null_mut(), acl, null_mut()), 0);
        }
        drop(file);
        assert!(dir.read(&path, 100).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original");
    }
    #[test]
    fn junctions_are_refused_for_settings_and_project_identity() {
        let s = Scratch::new(); let target = s.0.join("target"); fs::create_dir(&target).unwrap();
        let alias = s.0.join("alias");
        let command = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/cmd.exe");
        let made = std::process::Command::new(command).args(["/d", "/c", "mklink", "/J"])
            .arg(&alias).arg(&target).output().unwrap();
        assert!(made.status.success(), "junction fixture creation must succeed, not silently skip");
        assert!(PrivateDir::open(&alias, false).is_err());
        assert!(identity(&alias).is_err());
        assert!(PrivateDir::open(&alias.join("child"), true).is_err());
        assert!(!target.join("child").exists());
        fs::remove_dir(&alias).unwrap(); // Junction itself only, never target contents.
    }
}
