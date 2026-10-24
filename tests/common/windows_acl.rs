//! Test-only fixture: give a fixture-created `.gil` the owner-only descriptor that the product
//! establishes at creation (Windows contract §8). This lives in test code on purpose; the
//! product has no API that changes an existing Project's ACL.
use std::ffi::c_void;
use std::fs::{File, OpenOptions};
use std::io;
use std::mem::size_of;
use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
use std::path::Path;
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{CloseHandle, LocalFree};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SE_FILE_OBJECT,
    SetSecurityInfo,
};
use windows_sys::Win32::Security::{
    DACL_SECURITY_INFORMATION, GetSecurityDescriptorDacl, GetSecurityDescriptorOwner,
    GetTokenInformation, OWNER_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION,
    TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    GetFileInformationByHandle, READ_CONTROL, WRITE_DAC, WRITE_OWNER,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

struct Local(*mut c_void);
impl Drop for Local {
    fn drop(&mut self) {
        unsafe { LocalFree(self.0); }
    }
}

fn current_user_sid() -> io::Result<String> {
    unsafe {
        let mut token = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut bytes = 0;
        GetTokenInformation(token, TokenUser, null_mut(), 0, &mut bytes);
        let mut buf = vec![0usize; (bytes as usize).div_ceil(size_of::<usize>()).max(1)];
        let ok = GetTokenInformation(token, TokenUser, buf.as_mut_ptr().cast(), bytes, &mut bytes);
        CloseHandle(token);
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        let sid = (*(buf.as_ptr().cast::<TOKEN_USER>())).User.Sid;
        let mut text = null_mut();
        if ConvertSidToStringSidW(sid, &mut text) == 0 {
            return Err(io::Error::last_os_error());
        }
        let _text = Local(text.cast());
        let mut len = 0;
        while *text.add(len) != 0 { len += 1; }
        String::from_utf16(std::slice::from_raw_parts(text, len)).map_err(|_| io::Error::other("sid"))
    }
}

/// `O:<user>D:PAI(A;OICI;FA;;;<user>)(A;OICI;FA;;;SY)` applied through a handle opened without
/// following reparse points; refuses anything that is not a plain directory.
pub fn apply_owner_only(dir: &Path) -> io::Result<()> {
    let handle: File = OpenOptions::new()
        .access_mode(READ_CONTROL | WRITE_DAC | WRITE_OWNER)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(dir)?;
    unsafe {
        let mut info = std::mem::zeroed();
        if GetFileInformationByHandle(handle.as_raw_handle(), &mut info) == 0 {
            return Err(io::Error::last_os_error());
        }
        if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 || info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
            return Err(io::Error::other("fixture target is not a plain directory"));
        }
        let user = current_user_sid()?;
        let sddl: Vec<u16> = format!("O:{user}D:PAI(A;OICI;FA;;;{user})(A;OICI;FA;;;SY)\0").encode_utf16().collect();
        let mut sd = null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.as_ptr(), 1, &mut sd, null_mut()) == 0 {
            return Err(io::Error::last_os_error());
        }
        let sd = Local(sd);
        let (mut present, mut defaulted, mut dacl, mut owner) = (0, 0, null_mut(), null_mut());
        if GetSecurityDescriptorDacl(sd.0, &mut present, &mut dacl, &mut defaulted) == 0
            || GetSecurityDescriptorOwner(sd.0, &mut owner, &mut defaulted) == 0
            || present == 0
        {
            return Err(io::Error::other("descriptor"));
        }
        let error = SetSecurityInfo(
            handle.as_raw_handle(),
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
