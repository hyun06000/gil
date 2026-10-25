//! Discover only the installer-provisioned companion in the running Core's own cohort.
//! No PATH, Applications/registry search, network, descriptor execution or installer writes.
//! The receipt is an opt-in local installation contract, NOT publisher authentication.
//! Its trust derives from the already installed Core/cohort, and cannot authenticate downloads.
use crate::companion_ipc::{Client, SESSION_ARG};
use crate::companion_session::Release;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

pub const RECEIPT: &str = "companion-install.json";
pub const SCHEMA: &str = "gil-companion-install/1";
#[cfg(windows)]
pub const APP_FILE: &str = "gil-companion.exe";
#[cfg(not(windows))]
pub const APP_FILE: &str = "GIL Companion";
const START_LIMIT: Duration = Duration::from_secs(10);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema: String,
    protocol: u32,
    companion_file: String,
    companion_sha256: String,
}
pub struct Installation {
    executable: PathBuf,
    file: File,
    sha256: String,
}
fn invalid(text: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, text)
}

/// Missing receipt means legacy/no automatic Companion; malformed/inaccessible means refusal.
pub fn discover_for_core(core: &Path) -> io::Result<Option<Installation>> {
    if !core.is_absolute() {
        return Err(invalid("Core path must be absolute"));
    }
    let root = core
        .parent()
        .ok_or_else(|| invalid("Core parent missing"))?;
    let receipt_path = root.join(RECEIPT);
    let mut receipt = match open_regular(&receipt_path, false) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let mut bytes = Vec::new();
    (&mut receipt).take(4097).read_to_end(&mut bytes)?;
    if bytes.len() > 4096 {
        return Err(invalid("installation receipt too large"));
    }
    let decoded: Receipt = serde_json::from_slice(&bytes)?;
    if decoded.schema != SCHEMA
        || decoded.protocol != 1
        || decoded.companion_file != APP_FILE
        || !crate::runtime_observe::valid_sha256(&decoded.companion_sha256)
    {
        return Err(invalid("incompatible installation receipt"));
    }
    let executable = root.join(APP_FILE);
    let file = open_regular(&executable, true)?;
    let mut installed = Installation {
        executable,
        file,
        sha256: decoded.companion_sha256,
    };
    installed.verify()?;
    Ok(Some(installed))
}

impl Installation {
    fn verify(&mut self) -> io::Result<()> {
        self.file.seek(SeekFrom::Start(0))?;
        let before = self.file.metadata()?;
        if before.len() > 512 * 1024 * 1024 {
            return Err(invalid("Companion image exceeds size limit"));
        }
        let mut magic = [0; 4];
        self.file.read_exact(&mut magic)?;
        #[cfg(target_os = "macos")]
        if !matches!(
            magic,
            [0xcf, 0xfa, 0xed, 0xfe]
                | [0xfe, 0xed, 0xfa, 0xcf]
                | [0xca, 0xfe, 0xba, 0xbe]
                | [0xbe, 0xba, 0xfe, 0xca]
        ) {
            return Err(invalid("Companion must be a native Mach-O image"));
        }
        #[cfg(windows)]
        if magic[..2] != *b"MZ" {
            return Err(invalid("Companion must be a native PE image"));
        }
        self.file.seek(SeekFrom::Start(0))?;
        let mut digest = Sha256::new();
        let mut buffer = [0u8; 65536];
        loop {
            let n = self.file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            digest.update(&buffer[..n]);
        }
        if format!("{:x}", digest.finalize()) != self.sha256 {
            return Err(invalid("installed Companion digest mismatch"));
        }
        let current = open_regular(&self.executable, true)?;
        same_file(&self.file, &current)?;
        let after = self.file.metadata()?;
        if before.len() != after.len() || before.modified()? != after.modified()? {
            return Err(invalid("Companion changed while verifying"));
        }
        Ok(())
    }
    /// Existing running endpoint first; otherwise one spawn, with a bounded registration wait.
    /// Lock/rendezvous is private to GIL session IPC, never the runtime update store or Project.
    pub fn connect(&mut self, release: &Release) -> io::Result<Client> {
        self.connect_at(release, &coord::default_root()?)
    }
    // Explicit isolated coordination namespace for tests; no environment override in production.
    #[doc(hidden)]
    pub fn connect_at(&mut self, release: &Release, namespace: &Path) -> io::Result<Client> {
        let deadline = Instant::now() + START_LIMIT;
        let guard = loop {
            match coord::Guard::acquire(namespace) {
                Ok(lock) => break lock,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(50))
                }
                Err(e) => return Err(e),
            }
        };
        if let Some(endpoint) = guard.existing()? {
            match Client::connect(&endpoint, release) {
                Ok(client) => return Ok(client),
                Err(e) if stale(&e) => {}
                Err(e) => return Err(e), // auth, protocol, permission errors are not "not running"
            }
        }
        self.verify()?; // still holding the pinned image and serialized launch gate
        let endpoint = guard.next()?;
        let (reap, receive) = std::sync::mpsc::sync_channel::<Child>(1);
        // Reserve a reaper before spawning; resource exhaustion cannot leave an untracked child.
        std::thread::Builder::new()
            .name("gil-companion-reaper".into())
            .spawn(move || {
                if let Ok(mut child) = receive.recv() {
                    let _ = child.wait();
                }
            })?;
        let mut child = spawn_native(&self.executable, &self.file, &endpoint)?;
        let result = loop {
            match Client::connect(&endpoint, release) {
                Ok(client) => break Ok(client),
                Err(e)
                    if (stale(&e) || e.raw_os_error() == Some(231))
                        && Instant::now() < deadline => {}
                Err(e) => break Err(e),
            }
            match child.try_wait() {
                Ok(Some(status)) => {
                    break Err(io::Error::other(format!(
                        "Companion exited before registration: {status}"
                    )));
                }
                Err(e) => break Err(e),
                Ok(None) => {}
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        // Reap only the exact Child we spawned. Never kill by PID/name, including on timeout.
        let _ = reap.send(child);
        result
    }
}
fn stale(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
    )
}

fn open_regular(path: &Path, executable: bool) -> io::Result<File> {
    // Reject links in the cohort path. This is not a general hostile-same-user race defense.
    let mut walked = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::ParentDir | std::path::Component::CurDir => {
                return Err(invalid("noncanonical installation path"));
            }
            _ => {}
        }
        walked.push(part.as_os_str());
        if matches!(part, std::path::Component::Prefix(_)) {
            continue;
        }
        let metadata = std::fs::symlink_metadata(&walked)?;
        if metadata.file_type().is_symlink() {
            return Err(invalid("installation link refused"));
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err(invalid("installation reparse point refused"));
            }
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)?;
        let m = file.metadata()?;
        if !m.is_file()
            || m.nlink() != 1
            || m.mode() & 0o022 != 0
            || (m.uid() != 0 && m.uid() != unsafe { libc::geteuid() })
            || executable && m.mode() & 0o111 == 0
        {
            return Err(invalid("installation file protection refused"));
        }
        Ok(file)
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Storage::FileSystem::*;
        let access = crate::windows_fs::READ | if executable { FILE_EXECUTE } else { 0 };
        let file = crate::windows_fs::open_raw(path, access, FILE_SHARE_READ, OPEN_EXISTING, 0)?;
        let info = crate::windows_fs::info(&file)?;
        if info.dwFileAttributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT) != 0
            || info.nNumberOfLinks != 1
        {
            return Err(invalid("installation file type refused"));
        }
        Ok(file)
    }
}
fn same_file(a: &File, b: &File) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let (a, b) = (a.metadata()?, b.metadata()?);
        if (a.dev(), a.ino()) != (b.dev(), b.ino()) {
            return Err(invalid("installation file replaced"));
        }
    }
    #[cfg(windows)]
    if crate::windows_fs::id(a)? != crate::windows_fs::id(b)? {
        return Err(invalid("installation file replaced"));
    }
    Ok(())
}

fn spawn_native(executable: &Path, file: &File, endpoint: &Path) -> io::Result<Child> {
    let mut command = Command::new(executable);
    command
        .arg(SESSION_ARG)
        .arg(endpoint)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::os::unix::{ffi::OsStrExt, fs::MetadataExt, process::CommandExt};
        let c = |s: &std::ffi::OsStr| {
            CString::new(s.as_bytes()).map_err(|_| invalid("NUL in launch argument"))
        };
        let exe = c(executable.as_os_str())?;
        let arg = CString::new(SESSION_ARG).unwrap();
        let endpoint = c(endpoint.as_os_str())?;
        let environment: Vec<CString> = std::env::vars_os()
            .map(|(k, v)| {
                let mut pair = k;
                pair.push("=");
                pair.push(v);
                c(&pair)
            })
            .collect::<io::Result<_>>()?;
        struct Pointers(Vec<*const libc::c_char>);
        unsafe impl Send for Pointers {} // immutable pointers; CStrings captured by the same closure
        unsafe impl Sync for Pointers {}
        impl Pointers {
            fn ptr(&self) -> *const *const libc::c_char {
                self.0.as_ptr()
            }
        }
        let environment_ptr = Pointers(
            environment
                .iter()
                .map(|s| s.as_ptr())
                .chain(Some(std::ptr::null()))
                .collect(),
        );
        let held = file.metadata()?;
        let identity = (held.dev(), held.ino());
        unsafe {
            command.pre_exec(move || {
                let _keep_alive = &environment;
                let mut info: libc::stat = std::mem::zeroed();
                if libc::stat(exe.as_ptr(), &mut info) != 0 {
                    return Err(io::Error::last_os_error());
                }
                if (info.st_dev as u64, info.st_ino as u64) != identity {
                    return Err(io::Error::from_raw_os_error(libc::ESTALE));
                }
                let argv = [
                    exe.as_ptr(),
                    arg.as_ptr(),
                    endpoint.as_ptr(),
                    std::ptr::null(),
                ];
                libc::execve(exe.as_ptr(), argv.as_ptr(), environment_ptr.ptr());
                // No execvp shell fallback, PATH lookup, or allocations after fork.
                Err(io::Error::last_os_error())
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _pin = file;
        command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
    }
    command.spawn()
}

#[cfg(target_os = "macos")]
#[path = "companion_discovery/mac.rs"]
mod coord;
#[cfg(windows)]
#[path = "companion_discovery/windows.rs"]
mod coord;
