//! Bounded polling of nonblocking byte-mode named pipes (not overlapped/asynchronous I/O).
//! Remote clients are rejected. The DACL applies only to newly created kernel objects.
use crate::windows_fs as win;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::{
    fs::File,
    io::{self, Read, Write},
    mem::size_of,
    path::{Path, PathBuf},
    ptr::null_mut,
};
use windows_sys::Win32::{
    Foundation::*, Security::Authorization::*, Security::Cryptography::*, Security::*,
    Storage::FileSystem::*, System::Pipes::*, System::Threading::*,
};

fn refused(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message)
}
fn waiting() -> io::Error {
    io::Error::from(io::ErrorKind::WouldBlock)
}
pub(crate) struct Owned(pub HANDLE);
unsafe impl Send for Owned {} // unique owner; no thread-affine operations on these handles
impl Drop for Owned {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

pub(crate) fn user_name() -> io::Result<String> {
    let u = win::user()?;
    win::sid_text(win::sid(&u))
}
pub(crate) fn user_key() -> io::Result<String> {
    use sha2::{Digest, Sha256};
    Ok(format!("{:x}", Sha256::digest(user_name()?.as_bytes())))
}
pub(crate) fn endpoint() -> io::Result<PathBuf> {
    Ok(PathBuf::from(format!(
        r"\\.\pipe\gil-companion-v1-{}",
        user_key()?
    )))
}
pub(crate) fn parent(path: &Path) -> io::Result<()> {
    let text = path.to_str().ok_or_else(|| refused("invalid pipe name"))?;
    let suffix = text
        .strip_prefix(r"\\.\pipe\gil-companion-v1-")
        .ok_or_else(|| refused("not a local GIL pipe"))?;
    if suffix.is_empty()
        || suffix.len() > 128
        || !suffix
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(refused("invalid pipe suffix"));
    }
    Ok(())
}
fn wide(path: &Path) -> io::Result<Vec<u16>> {
    parent(path)?;
    Ok(path
        .to_str()
        .unwrap()
        .encode_utf16()
        .chain(Some(0))
        .collect())
}
pub(crate) struct Descriptor(*mut std::ffi::c_void);
impl Descriptor {
    pub(crate) fn new() -> io::Result<Self> {
        let user = user_name()?;
        let text: Vec<u16> = format!("O:{user}D:P(A;;GA;;;{user})(A;;GA;;;SY)\0")
            .encode_utf16()
            .collect();
        let mut sd = null_mut();
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                text.as_ptr(),
                1,
                &mut sd,
                null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(sd))
    }
    pub(crate) fn attributes(&self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.0,
            bInheritHandle: 0,
        }
    }
}
impl Drop for Descriptor {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0);
        }
    }
}

pub(super) fn nonce() -> io::Result<String> {
    let mut bytes = [0u8; 32];
    if unsafe {
        BCryptGenRandom(
            null_mut(),
            bytes.as_mut_ptr(),
            bytes.len() as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    } < 0
    {
        return Err(io::Error::other("OS entropy unavailable"));
    }
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

pub(super) struct Stream {
    file: File,
    server_side: bool,
}
impl Stream {
    pub(super) fn set_nonblocking(&self, _: bool) -> io::Result<()> {
        let mode = PIPE_NOWAIT | PIPE_READMODE_BYTE;
        if unsafe {
            SetNamedPipeHandleState(self.file.as_raw_handle(), &mode, null_mut(), null_mut())
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}
impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let mut n = 0;
        if unsafe {
            ReadFile(
                self.file.as_raw_handle(),
                buf.as_mut_ptr(),
                buf.len().min(u32::MAX as usize) as u32,
                &mut n,
                null_mut(),
            )
        } != 0
        {
            return if n == 0 {
                Err(waiting())
            } else {
                Ok(n as usize)
            };
        }
        let e = io::Error::last_os_error();
        match e.raw_os_error().map(|v| v as u32) {
            Some(ERROR_BROKEN_PIPE | ERROR_PIPE_NOT_CONNECTED) => Ok(0),
            Some(ERROR_NO_DATA) => {
                // NO_DATA alone is ambiguous; distinguish disconnected pipe from an empty one.
                if unsafe {
                    PeekNamedPipe(
                        self.file.as_raw_handle(),
                        null_mut(),
                        0,
                        null_mut(),
                        null_mut(),
                        null_mut(),
                    )
                } == 0
                {
                    let error = io::Error::last_os_error();
                    if matches!(
                        error.raw_os_error().map(|v| v as u32),
                        Some(ERROR_BROKEN_PIPE | ERROR_PIPE_NOT_CONNECTED)
                    ) {
                        return Ok(0);
                    }
                    return Err(error);
                }
                Err(waiting())
            }
            _ => Err(e),
        }
    }
}
impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let mut n = 0;
        if unsafe {
            WriteFile(
                self.file.as_raw_handle(),
                buf.as_ptr(),
                buf.len().min(u32::MAX as usize) as u32,
                &mut n,
                null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        if n == 0 {
            return Err(waiting());
        }
        Ok(n as usize)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    } // never FlushFileBuffers: it waits for peer reads
}

pub(super) fn peer(stream: &Stream) -> io::Result<()> {
    let mut pid = 0;
    let ok = unsafe {
        if stream.server_side {
            GetNamedPipeClientProcessId(stream.file.as_raw_handle(), &mut pid)
        } else {
            GetNamedPipeServerProcessId(stream.file.as_raw_handle(), &mut pid)
        }
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return Err(io::Error::last_os_error());
    }
    let process = Owned(process);
    let mut token = null_mut();
    if unsafe { OpenProcessToken(process.0, TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let token = Owned(token);
    let mut size = 0;
    unsafe {
        GetTokenInformation(token.0, TokenUser, null_mut(), 0, &mut size);
    }
    if size < size_of::<TOKEN_USER>() as u32 || size > 65536 {
        return Err(refused("invalid peer token"));
    }
    let mut buffer = vec![0usize; (size as usize).div_ceil(size_of::<usize>())];
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            size,
            &mut size,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let current = win::user()?;
    if unsafe { EqualSid(win::sid(&buffer), win::sid(&current)) } == 0 {
        return Err(refused("IPC peer user mismatch"));
    }
    // Re-query while the process handle is held. PID names are never used to kill processes.
    let mut again = 0;
    let ok = unsafe {
        if stream.server_side {
            GetNamedPipeClientProcessId(stream.file.as_raw_handle(), &mut again)
        } else {
            GetNamedPipeServerProcessId(stream.file.as_raw_handle(), &mut again)
        }
    };
    if ok == 0 || again != pid {
        return Err(refused("IPC peer changed"));
    }
    Ok(())
}

pub(super) struct Listener {
    name: Vec<u16>,
    pending: Stream,
}
fn instance(name: &[u16], first: bool) -> io::Result<Stream> {
    let descriptor = Descriptor::new()?;
    let attributes = descriptor.attributes();
    let handle = unsafe {
        CreateNamedPipeW(
            name.as_ptr(),
            PIPE_ACCESS_DUPLEX
                | if first {
                    FILE_FLAG_FIRST_PIPE_INSTANCE
                } else {
                    0
                },
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_NOWAIT | PIPE_REJECT_REMOTE_CLIENTS,
            255,
            4096,
            4096,
            0,
            &attributes,
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    Ok(Stream {
        file: unsafe { File::from_raw_handle(handle) },
        server_side: true,
    })
}
impl Listener {
    pub(super) fn bind(path: &Path) -> io::Result<Self> {
        let name = wide(path)?;
        let pending = instance(&name, true)?;
        Ok(Self { name, pending })
    }
    pub(super) fn set_nonblocking(&self, _: bool) -> io::Result<()> {
        Ok(())
    }
    pub(super) fn accept(&mut self) -> io::Result<(Stream, ())> {
        let connected = unsafe { ConnectNamedPipe(self.pending.file.as_raw_handle(), null_mut()) };
        // In NOWAIT mode TRUE means "available", not "a client connected". Only
        // ERROR_PIPE_CONNECTED proves a connection (ConnectNamedPipe official contract).
        if connected != 0 {
            return Err(waiting());
        }
        if connected == 0 {
            let e = io::Error::last_os_error();
            match e.raw_os_error().map(|v| v as u32) {
                Some(ERROR_PIPE_LISTENING) => return Err(waiting()),
                Some(ERROR_PIPE_CONNECTED) => {}
                Some(ERROR_NO_DATA) => {
                    unsafe {
                        DisconnectNamedPipe(self.pending.file.as_raw_handle());
                    }
                    return Err(waiting());
                }
                _ => return Err(e),
            }
        }
        let next = instance(&self.name, false)?;
        Ok((std::mem::replace(&mut self.pending, next), ()))
    }
}
pub(super) fn connect(path: &Path) -> io::Result<Stream> {
    let name = wide(path)?;
    // Local name only; no WaitNamedPipe(INFINITE), remote redirector, or impersonation privilege.
    let raw = unsafe {
        CreateFileW(
            name.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            0,
            null_mut(),
            OPEN_EXISTING,
            SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
            null_mut(),
        )
    };
    if raw == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let stream = Stream {
        file: unsafe { File::from_raw_handle(raw) },
        server_side: false,
    };
    stream.set_nonblocking(true)?;
    Ok(stream)
}

#[cfg(test)]
mod tests;
