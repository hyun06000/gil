//! Runtime update — **isolated launch mechanism** (Implementation Plan §1 launcher behaviour,
//! §1.1 state table, §3.1 TOCTOU row, acceptance AT-P2/AT-P3).
//!
//! What this layer does:
//! - the target is decided by the pure judge over a held-handle observation: `first_use`,
//!   `initializing`, `prepared` → the **bundled** Core; `selected` → the **selected** Core,
//!   whose generation, version and full digest are taken from that same observation's valid
//!   `selection` record (never re-read separately); `damaged`, `blocked` → no general Core is
//!   run (diagnostic only); no fallback from a failed selected target to the bundled one;
//! - the actual target is re-verified right before the spawn — directory protection,
//!   `complete`, protection, single link, execute right and the full digest, all through a
//!   **pinned handle**;
//! - the executable and its arguments are separate values: the arguments are fixed constants
//!   (`FixedArgs`), never assembled from a shell string and never caller-supplied;
//! - Unix spawns with its own `fork` + `execve` (no `execvp`: libc's `execvp` retries an
//!   ENOEXEC file through `/bin/sh`, which must never happen for a Core); the code after
//!   `fork` is limited to async-signal-safe calls (`stat`, `dup2`, `execve`, `write`,
//!   `_exit`); Windows spawns through `CreateProcess`, which has no shell fallback;
//! - stdio is passed through or piped; the report says what actually happened after the start.
//!   With a finite limit, `finish` bounds **all three** of input delivery, exit confirmation and
//!   output collection by one deadline: the input is written without blocking past it (Unix:
//!   non-blocking pipe + `poll`, checked on every iteration; Windows: a writer thread), on
//!   Unix stdin is closed in every case so the EOF reaches a child that reads, on Windows a
//!   writer thread that is still blocked past the deadline keeps stdin and the report says so
//!   (`Delivery::Timeout { stdin_closed: false }`, no forced cancel); the exit is polled until the deadline; the readers
//!   are given the remaining time. Each outcome is reported separately: `stdin_error`,
//!   `Exit::Code`/`Exit::Signal`/`Exit::NotConfirmed`, and per stream `Capture::Bytes` /
//!   `Capture::Incomplete(bytes so far)` / `Capture::Failed` / `Capture::NotCaptured`. A child
//!   that exited can still leave a descendant holding the output pipe, which shows as a
//!   confirmed exit with an `Incomplete` capture. What stays behind is listed in
//!   `ExitReport::left_behind`: a not-reaped child (the caller ends and reaps it by pid), reader
//!   threads that end when the last pipe writer closes, a detached writer thread (Windows).
//!   Without a limit (`None`, the product's serving case) every step waits as long as needed.
//! - the Unix fork/exec errno channel distinguishes a clean EOF (exec succeeded), a complete
//!   errno answer (exec or the pre-exec checks failed; the child is reaped and its exit
//!   reported), a partial answer and a read error (both failures that keep the pid and say
//!   whether the child could be reaped — the exec outcome is unknown then, so the child is only
//!   polled, never waited for without limit); EINTR is retried.
//!
//! What the replacement defence blocks and what it does not (per platform, as implemented):
//! - Windows: the selected Core is opened with `FILE_EXECUTE` and `FILE_SHARE_READ` only and
//!   held until `CreateProcess` returned, so renaming/deleting that file or writing to it
//!   (same-object content change) fails with a sharing violation; renaming an ancestor fails
//!   while handles are held beneath it (observed on the runners). Not blocked by the pin: a
//!   reparse point planted higher in the path (only the observation walk refuses it).
//! - Unix: the pin is a read-only descriptor used for hashing; the child compares the path's
//!   (device, inode) with the pin's right before `execve` and refuses a different object with
//!   ESTALE. **Not blocked**: a replacement that lands between that comparison and the
//!   `execve` syscall (the window is one syscall wide, not zero); a content change of the same
//!   inode by a writer with write permission after the hash (no sharing locks exist on Unix;
//!   the store's 0500 mode and 0700 directories are the only barrier); an interpreter named by
//!   a script's shebang is resolved by path. The two exec-by-handle routes tried on this Mac
//!   (`/dev/fd/N`: refused by devfs; `/.vol/<dev>/<inode>`: cannot name a replaced inode) did
//!   not work; other routes were not evaluated, so "no exec-by-handle on macOS" is not claimed.
//! - Neither platform verifies the bundled Core's digest (it is the packaged binary; only
//!   regular-file, single-link and execute checks).
//!
//! This is a mechanism check with synthetic candidates. It is **not** a trust decision about a
//! downloaded candidate and not a GIL compatibility check; the trust gate (§4.1) does not
//! exist yet, so nothing here is `pub` outside the crate and no product manifest, MCP tool or
//! installation uses it.
// Crate-private by design: nothing in the product calls this layer until the trust gate exists.
#![cfg_attr(not(test), allow(dead_code))]
use crate::runtime_observe::{CORE_FILE, VERSIONS_DIR, observe_and_judge, observe_and_judge_held, platform, version_dirname};
use crate::runtime_prepare::{Candidate, hash_file, verify_complete_candidate};
use crate::runtime_state::{Record, Run, State, Verdict};
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The only argument vectors a child can be started with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FixedArgs {
    /// The product Core serving MCP on stdio.
    McpServe,
    /// Synthetic test executables (no arguments on Unix; `cmd.exe /d /c more & exit 7` on Windows).
    #[cfg(test)]
    Synthetic,
    /// Windows test child that neither reads stdin nor exits for 30 s (`ping` as a sleeper).
    #[cfg(all(test, windows))]
    SyntheticIdle,
}
impl FixedArgs {
    fn argv(self) -> &'static [&'static str] {
        match self {
            FixedArgs::McpServe => &["mcp", "--serve"],
            #[cfg(test)]
            FixedArgs::Synthetic => SYNTHETIC_ARGS,
            #[cfg(all(test, windows))]
            FixedArgs::SyntheticIdle => &["/d", "/c", "ping -n 31 127.0.0.1 >nul & exit 7"],
        }
    }
}
#[cfg(all(test, unix))]
const SYNTHETIC_ARGS: &[&str] = &[];
#[cfg(all(test, windows))]
const SYNTHETIC_ARGS: &[&str] = &["/d", "/c", "more & exit 7"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TargetKind { Bundled, Selected { generation: u64, version: String, sha256: String } }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LaunchStage { Observe, Judge, Verify, Pin, Spawn }

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LaunchRefusal { pub stage: LaunchStage, pub reason: String, pub verdict: Option<Verdict>, pub spawned: Started, pub child: Option<FailedChild> }

/// A target verified through a handle that stays open until the child has started.
#[derive(Debug)]
pub(crate) struct Target {
    pub kind: TargetKind,
    pub verdict: Verdict,
    /// Pinned handle. Windows: opened with `FILE_EXECUTE` and `FILE_SHARE_READ`, held across
    /// `CreateProcess` (replacement and content change fail with a sharing violation). Unix: the
    /// descriptor the digest was hashed through; its (device, inode) is what the child compares
    /// against the path right before `execve`.
    pinned: File,
    /// The path the image is started from on both platforms (`CreateProcess` / `execve`); the
    /// pin above is what ties it to the verified object, within the limits stated in the
    /// module docs.
    path: PathBuf,
    /// Digest actually hashed through the pinned handle (None for the bundled Core).
    pub verified_sha256: Option<String>,
}

/// Outcome of one capture stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Capture {
    /// EOF reached: every byte the child (and its descendants) wrote.
    Bytes(Vec<u8>),
    /// The deadline passed before EOF; these are the bytes read so far, the reader stays attached.
    Incomplete(Vec<u8>),
    Failed(String),
    NotCaptured,
}
/// How the child ended, as confirmed by the OS — or not confirmed within the wait limit.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(windows, allow(dead_code))]
pub(crate) enum Exit { Code(i32), Signal(i32), NotConfirmed { waited: Duration } }
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExitReport {
    pub pid: u32,
    pub exit: Exit,
    pub stdout: Capture,
    pub stderr: Capture,
    /// The error met while writing the input (stdin was still closed afterwards, so the EOF
    /// reached the child in any case), including "not fully delivered within the limit".
    pub stdin_error: Option<String>,
    /// What this call did not finish and who is responsible for it now.
    pub left_behind: Vec<String>,
}
impl ExitReport {
    pub(crate) fn stdout_bytes(&self) -> &[u8] { match &self.stdout { Capture::Bytes(b) | Capture::Incomplete(b) => b, _ => &[] } }
}
/// What happened to the input written to the child's stdin.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) enum Delivery {
    /// Every byte written and stdin closed (EOF delivered).
    Complete,
    /// The write failed; stdin was closed afterwards (EOF delivered to a child that reads).
    Failed(String),
    /// The deadline passed. `stdin_closed` says whether this side closed stdin (Unix: yes, the
    /// descriptor is ours; Windows: no, the writer thread still holds it and closes it when its
    /// blocking write ends or fails, e.g. when the child exits).
    Timeout { delivered: usize, total: usize, stdin_closed: bool },
    /// The writer thread ended without reporting (Windows); the stdin state is unknown.
    Unknown(String),
}

/// Whether a Core process was started by a failed spawn attempt: `No` (nothing exec'd), or
/// `Unknown` (the Unix errno channel gave no usable answer, so the exec outcome is unknown).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(windows, allow(dead_code))]
pub(crate) enum Started { No, Unknown }
/// The forked child of a failed Unix spawn: its pid and, when it could be reaped within a short
/// poll, how it ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FailedChild { pub pid: u32, pub exit: Option<Exit> }

#[derive(Debug, Clone)]
#[cfg_attr(test, allow(dead_code))]
pub(crate) enum IoMode { Inherit, Piped }

#[cfg(not(test))]
mod hooks_placeholder { #[derive(Clone, Copy)] pub enum Never {} }
#[cfg(test)]
pub(crate) mod hooks {
    use std::cell::Cell;
    use std::path::Path;
    thread_local! {
        /// Runs right after the observation that decides the target, before verification.
        pub static AFTER_OBSERVE: Cell<Option<fn(&Path)>> = const { Cell::new(None) };
        /// Makes the stdout reader fail instead of returning bytes.
        pub static FAIL_READER: Cell<bool> = const { Cell::new(false) };
        /// Unix only: what the parent sees on the fork/exec errno channel instead of the real pipe.
        pub static ERRNO_CHANNEL: Cell<Option<ErrnoInject>> = const { Cell::new(None) };
        /// Windows only: the completion channel of the last stdin writer thread left behind by
        /// a timeout, so a test can confirm the thread ends once the child is gone.
        pub static LAST_WRITER: std::cell::RefCell<Option<std::sync::mpsc::Receiver<Result<(), String>>>> = const { std::cell::RefCell::new(None) };
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[cfg_attr(windows, allow(dead_code))]
    pub enum ErrnoInject { Partial, ReadError, EintrThenReal }
    pub fn reset() { AFTER_OBSERVE.with(|c| c.set(None)); FAIL_READER.with(|c| c.set(false)); ERRNO_CHANNEL.with(|c| c.set(None)); LAST_WRITER.with(|w| *w.borrow_mut() = None); }
}

/// Decide the target from a held-handle observation and verify it through a pinned handle.
/// No child is started here; a refusal means nothing was spawned (`spawned: false`).
pub(crate) fn prepare_target(root: &Path, bundled: &Path) -> Result<Target, LaunchRefusal> {
    let refuse = |stage, reason: String, verdict: Option<Verdict>| LaunchRefusal { stage, reason, verdict, spawned: Started::No, child: None };
    // 1. Observation through a held root handle (absent root = first use is observed by path).
    let root_dir = match platform::walk(root) {
        Ok(Some(d)) => Some(d),
        Ok(None) => None,
        Err(e) => return Err(refuse(LaunchStage::Observe, format!("root walk: {e}"), None)),
    };
    let (observed, verdict) = match &root_dir { Some(d) => observe_and_judge_held(root, d), None => observe_and_judge(root) };
    #[cfg(test)]
    if let Some(f) = hooks::AFTER_OBSERVE.with(|c| c.get()) { f(root); }
    // 2. Judge → target kind; the selected identity comes from the same observation's record.
    let kind = match (&verdict.run, &verdict.state, &observed.observation.selection) {
        (Run::Bundled, State::FirstUse | State::Initializing | State::Prepared, _) => TargetKind::Bundled,
        (Run::Selected, State::Selected { generation, version, .. }, Record::Valid(sel)) if sel.generation == *generation && &sel.version == version => {
            TargetKind::Selected { generation: sel.generation, version: sel.version.clone(), sha256: sel.sha256.clone() }
        }
        (Run::Selected, state, sel) => return Err(refuse(LaunchStage::Judge, format!("selected verdict {state:?} without a matching valid selection record in the same observation ({sel:?})"), Some(verdict.clone()))),
        (Run::None, state, _) => return Err(refuse(LaunchStage::Judge, format!("{state:?}: diagnostic only, no general Core is run (no fallback)"), Some(verdict.clone()))),
        (run, state, _) => return Err(refuse(LaunchStage::Judge, format!("unexpected verdict {run:?} for {state:?}"), Some(verdict.clone()))),
    };
    // 3. Verify and pin the actual executable.
    match &kind {
        TargetKind::Bundled => {
            let pinned = lfs::pin_executable_path(bundled).map_err(|e| refuse(LaunchStage::Pin, format!("bundled Core {}: {e}", bundled.display()), Some(verdict.clone())))?;
            Ok(Target { kind, verdict, pinned, path: bundled.to_path_buf(), verified_sha256: None })
        }
        TargetKind::Selected { version, sha256, .. } => {
            let root_dir = root_dir.ok_or_else(|| refuse(LaunchStage::Verify, "selected state without a root handle".into(), Some(verdict.clone())))?;
            let versions = match platform::open_subdir(&root_dir, VERSIONS_DIR) { Ok(Some(d)) => d, Ok(None) => return Err(refuse(LaunchStage::Verify, "versions/ absent".into(), Some(verdict.clone()))), Err(e) => return Err(refuse(LaunchStage::Verify, format!("versions/: {e}"), Some(verdict.clone()))) };
            platform::check_dir(&versions).map_err(|e| refuse(LaunchStage::Verify, format!("versions/ protection: {e}"), Some(verdict.clone())))?;
            let dirname = version_dirname(version, sha256);
            match verify_complete_candidate(&versions, &dirname, sha256) {
                Ok(()) => {}
                Err(Candidate::Absent) => return Err(refuse(LaunchStage::Verify, format!("{VERSIONS_DIR}/{dirname} absent"), Some(verdict.clone()))),
                Err(Candidate::Incomplete) => return Err(refuse(LaunchStage::Verify, format!("{VERSIONS_DIR}/{dirname} incomplete"), Some(verdict.clone()))),
                Err(Candidate::DigestDiffers) => return Err(refuse(LaunchStage::Verify, format!("{VERSIONS_DIR}/{dirname} digest differs from the selection"), Some(verdict.clone()))),
                Err(Candidate::Refused(why)) => return Err(refuse(LaunchStage::Verify, format!("{VERSIONS_DIR}/{dirname} {why}"), Some(verdict.clone()))),
            }
            let vdir = match platform::open_subdir(&versions, &dirname) { Ok(Some(d)) => d, _ => return Err(refuse(LaunchStage::Verify, format!("{VERSIONS_DIR}/{dirname} vanished"), Some(verdict.clone()))) };
            let pinned = lfs::pin_executable_at(&vdir, CORE_FILE).map_err(|e| refuse(LaunchStage::Pin, format!("{VERSIONS_DIR}/{dirname}/{CORE_FILE} pin: {e}"), Some(verdict.clone())))?;
            let stamp = platform::stamp_file(&pinned).map_err(|e| refuse(LaunchStage::Pin, e.to_string(), Some(verdict.clone())))?;
            platform::check_exec(&pinned, &stamp).map_err(|e| refuse(LaunchStage::Pin, format!("{VERSIONS_DIR}/{dirname}/{CORE_FILE} protection through the pin: {e}"), Some(verdict.clone())))?;
            let digest = hash_file(&pinned).map_err(|e| refuse(LaunchStage::Pin, format!("hash through the pin: {e}"), Some(verdict.clone())))?;
            if &digest != sha256 { return Err(refuse(LaunchStage::Pin, "digest through the pinned handle differs from the selection".into(), Some(verdict.clone()))); }
            let path = root.join(VERSIONS_DIR).join(&dirname).join(CORE_FILE);
            Ok(Target { kind, verdict, pinned, path, verified_sha256: Some(digest) })
        }
    }
}

/// A started child. The pinned handle was released after the start. When piped, stdin stays
/// open until `finish` delivers the input and the EOF.
#[derive(Debug)]
pub(crate) struct Child { inner: lfs::Spawned, pub kind: TargetKind, stdout: Option<Reader>, stderr: Option<Reader> }

/// Start the verified target with its fixed arguments. A spawn failure is a refusal (nothing
/// else is tried).
pub(crate) fn spawn(target: Target, args: FixedArgs, io_mode: IoMode) -> Result<Child, LaunchRefusal> {
    let piped = matches!(io_mode, IoMode::Piped);
    let mut inner = lfs::spawn(&target, args.argv(), piped).map_err(|e| {
        let reason = if e.replaced { "spawn refused: the target at its path was no longer the verified object right before exec (replaced between verification and start); no fallback to another Core".to_owned() } else { format!("spawn failed: {} (no shell fallback, no fallback to another Core)", e.message) };
        LaunchRefusal { stage: LaunchStage::Spawn, reason, verdict: Some(target.verdict.clone()), spawned: e.started, child: e.child }
    })?;
    drop(target.pinned); // the image is loaded; the pin may go now
    #[cfg(test)]
    let fail_reader = hooks::FAIL_READER.with(|c| c.get());
    #[cfg(not(test))]
    let fail_reader = false;
    let stdout = inner.take_stdout().map(|s| Reader::start("stdout", s, fail_reader));
    let stderr = inner.take_stderr().map(|s| Reader::start("stderr", s, false));
    Ok(Child { inner, kind: target.kind, stdout, stderr })
}

/// A capture thread whose progress is visible from outside, so a deadline can take a snapshot.
#[derive(Debug)]
struct Reader { name: &'static str, buf: std::sync::Arc<std::sync::Mutex<Vec<u8>>>, done: std::sync::mpsc::Receiver<io::Result<()>> }
impl Reader {
    fn start(name: &'static str, mut src: Box<dyn Read + Send>, fail: bool) -> Reader {
        let buf = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let (tx, done) = std::sync::mpsc::channel();
        let shared = buf.clone();
        std::thread::spawn(move || {
            let result = (|| -> io::Result<()> {
                if fail { return Err(io::Error::other("injected reader failure")); }
                let mut chunk = [0u8; 8192];
                loop {
                    let n = src.read(&mut chunk)?;
                    if n == 0 { return Ok(()); }
                    shared.lock().map_err(|_| io::Error::other("capture buffer poisoned"))?.extend_from_slice(&chunk[..n]);
                }
            })();
            let _ = tx.send(result);
        });
        Reader { name, buf, done }
    }
    fn snapshot(&self) -> Vec<u8> { self.buf.lock().map(|b| b.clone()).unwrap_or_default() }
    /// Wait for EOF until the deadline (None = no deadline).
    fn collect(self, deadline: Option<std::time::Instant>, left_behind: &mut Vec<String>) -> Capture {
        let result = match deadline {
            None => self.done.recv().map_err(|_| std::sync::mpsc::RecvTimeoutError::Disconnected),
            Some(d) => self.done.recv_timeout(d.saturating_duration_since(std::time::Instant::now())),
        };
        match result {
            Ok(Ok(())) => Capture::Bytes(self.snapshot()),
            Ok(Err(e)) => Capture::Failed(e.to_string()),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => { left_behind.push(format!("{} reader still attached: the pipe is held open (by the child or a descendant); the thread ends when the last writer closes it", self.name)); Capture::Incomplete(self.snapshot()) }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Capture::Failed("reader thread ended without a result (panic)".into()),
        }
    }
}

impl Child {
    pub(crate) fn id(&self) -> u32 { self.inner.pid() }
    /// Deliver the input and close stdin (EOF), without a deadline. A write error is returned
    /// but stdin is closed regardless, so a child that reads always sees the EOF.
    pub(crate) fn send_and_close(&mut self, input: &[u8]) -> io::Result<()> {
        match self.inner.write_stdin_and_close(input, None) { Delivery::Complete => Ok(()), Delivery::Failed(e) | Delivery::Unknown(e) => Err(io::Error::other(e)), Delivery::Timeout { .. } => Err(io::Error::other("timeout without a deadline")) }
    }
    /// Exit confirmation and output collection, bounded by one deadline when `limit` is given.
    /// Order: exit first (a child never waits for our readers: they run concurrently), then the
    /// readers get whatever time is left (possibly none: then a snapshot is taken).
    pub(crate) fn finish(self, stdin_error: Option<String>, limit: Option<Duration>) -> ExitReport {
        let deadline = limit.map(|l| std::time::Instant::now() + l);
        self.finish_by(stdin_error, deadline, Vec::new())
    }
    fn finish_by(mut self, stdin_error: Option<String>, deadline: Option<std::time::Instant>, mut left_behind: Vec<String>) -> ExitReport {
        let pid = self.inner.pid();
        let exit = self.inner.wait_until(deadline);
        if matches!(exit, Exit::NotConfirmed { .. }) { left_behind.push(format!("child pid {pid} not confirmed exited and not reaped: the caller ends and reaps it")); }
        let stdout = self.stdout.take().map(|r| r.collect(deadline, &mut left_behind)).unwrap_or(Capture::NotCaptured);
        let stderr = self.stderr.take().map(|r| r.collect(deadline, &mut left_behind)).unwrap_or(Capture::NotCaptured);
        ExitReport { pid, exit, stdout, stderr, stdin_error, left_behind }
    }
    /// Input, EOF, exit and readers in one step, all under one deadline when `limit` is given.
    pub(crate) fn send_eof_and_finish(mut self, input: &[u8], limit: Option<Duration>) -> ExitReport {
        let deadline = limit.map(|l| std::time::Instant::now() + l);
        let mut left_behind = Vec::new();
        let stdin_error = match self.inner.write_stdin_and_close(input, deadline) {
            Delivery::Complete => None,
            Delivery::Failed(e) => Some(format!("input write failed: {e} (stdin closed afterwards; EOF reaches a child that reads)")),
            Delivery::Timeout { delivered, total, stdin_closed: true } => Some(format!("input not fully delivered within the limit: {delivered} of {total} bytes; stdin closed, EOF reaches a child that reads")),
            Delivery::Timeout { delivered, total, stdin_closed: false } => {
                left_behind.push(format!("stdin writer thread still holds stdin with {} of {total} bytes pending; it closes stdin (EOF) only when its write ends or fails, for example when the child exits", total - delivered));
                Some(format!("input not fully delivered within the limit: {delivered} of {total} bytes confirmed; stdin NOT closed yet, EOF not delivered"))
            }
            Delivery::Unknown(why) => { left_behind.push(format!("stdin state unknown: {why}")); Some(format!("input delivery unknown: {why}")) }
        };
        self.finish_by(stdin_error, deadline, left_behind)
    }
}

/// Convenience for one connection: decide, verify, spawn, pass the input and the EOF, wait
/// (without limit, like a serving Core), return the report.
pub(crate) fn launch(root: &Path, bundled: &Path, args: FixedArgs, input: Option<&[u8]>) -> Result<(TargetKind, ExitReport), LaunchRefusal> {
    let target = prepare_target(root, bundled)?;
    let child = spawn(target, args, if input.is_some() { IoMode::Piped } else { IoMode::Inherit })?;
    let kind = child.kind.clone();
    Ok((kind, child.send_eof_and_finish(input.unwrap_or(&[]), None)))
}

#[cfg(unix)]
mod lfs {
    use super::{Exit, FailedChild, Started, Target};
    use crate::runtime_observe::platform::Dir;
    use std::ffi::CString;
    use std::fs::File;
    use std::io::{self, Read};
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
    use std::path::Path;
    use std::time::{Duration, Instant};

    /// The selected Core: opened relative to the held version directory, no links.
    pub fn pin_executable_at(vdir: &Dir, name: &str) -> io::Result<File> {
        match crate::runtime_observe::platform::open_executable(vdir, name)? { Some(f) => Ok(f), None => Err(io::Error::new(io::ErrorKind::NotFound, format!("{name} absent"))) }
    }
    /// The bundled Core: a regular file with one link and the owner execute bit, opened without
    /// following links (its digest is not known to this layer).
    pub fn pin_executable_path(path: &Path) -> io::Result<File> {
        let f = std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC).open(path)?;
        let m = f.metadata()?;
        if !m.is_file() || m.nlink() != 1 { return Err(io::Error::other("not a regular file with one link")); }
        if m.permissions().mode() & 0o100 == 0 { return Err(io::Error::other("owner execute bit missing")); }
        Ok(f)
    }
    /// A spawn failure with everything the parent still knows about the forked child.
    #[derive(Debug)]
    pub struct SpawnError { pub message: String, pub replaced: bool, pub started: Started, pub child: Option<FailedChild> }
    impl From<io::Error> for SpawnError { fn from(e: io::Error) -> Self { SpawnError { message: e.to_string(), replaced: false, started: Started::No, child: None } } }

    #[derive(Debug)]
    pub struct Spawned { pid: libc::pid_t, stdin: Option<File>, stdout: Option<File>, stderr: Option<File>, reaped: Option<Exit> }
    impl Spawned {
        pub fn pid(&self) -> u32 { self.pid as u32 }
        pub fn take_stdout(&mut self) -> Option<Box<dyn Read + Send>> { self.stdout.take().map(|f| Box::new(f) as Box<dyn Read + Send>) }
        pub fn take_stderr(&mut self) -> Option<Box<dyn Read + Send>> { self.stderr.take().map(|f| Box::new(f) as Box<dyn Read + Send>) }
        /// Write the input (non-blocking pipe + `poll`, bounded by the deadline when given) and
        /// close stdin in every case (the descriptor is ours). The deadline is checked on every
        /// iteration, so steady partial progress is bounded too, not only a full pipe.
        pub fn write_stdin_and_close(&mut self, input: &[u8], deadline: Option<Instant>) -> super::Delivery {
            use super::Delivery;
            let Some(stdin) = self.stdin.take() else { return Delivery::Complete };
            let fd = stdin.as_raw_fd();
            let result = (|| -> Delivery {
                let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
                if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 { return Delivery::Failed(io::Error::last_os_error().to_string()); }
                let mut off = 0usize;
                while off < input.len() {
                    let left_ms = match deadline {
                        None => -1,
                        Some(d) => { let left = d.saturating_duration_since(Instant::now()); if left.is_zero() { return Delivery::Timeout { delivered: off, total: input.len(), stdin_closed: true }; } left.as_millis().clamp(1, i32::MAX as u128) as i32 }
                    };
                    let n = unsafe { libc::write(fd, input[off..].as_ptr().cast(), input.len() - off) };
                    if n >= 0 { off += n as usize; continue; }
                    let e = io::Error::last_os_error();
                    match e.raw_os_error() {
                        Some(libc::EINTR) => continue,
                        Some(libc::EAGAIN) => {
                            let mut pfd = libc::pollfd { fd, events: libc::POLLOUT, revents: 0 };
                            let r = unsafe { libc::poll(&mut pfd, 1, left_ms) };
                            if r < 0 { let e = io::Error::last_os_error(); if e.raw_os_error() == Some(libc::EINTR) { continue; } return Delivery::Failed(e.to_string()); }
                            // r == 0: the deadline passed while waiting; the loop top reports it
                        }
                        _ => return Delivery::Failed(e.to_string()),
                    }
                }
                Delivery::Complete
            })();
            drop(stdin); // EOF, whatever happened above
            result
        }
        /// `waitpid`: blocking without a deadline, otherwise polled with `WNOHANG` until it.
        pub fn wait_until(&mut self, deadline: Option<Instant>) -> Exit {
            if let Some(e) = &self.reaped { return e.clone(); }
            let started = Instant::now();
            loop {
                match waitpid_once(self.pid, deadline.is_some()) {
                    Ok(Some(exit)) => { self.reaped = Some(exit.clone()); return exit; }
                    Ok(None) => {}
                    Err(_) => return Exit::NotConfirmed { waited: started.elapsed() },
                }
                if let Some(d) = deadline { if Instant::now() >= d { return Exit::NotConfirmed { waited: started.elapsed() }; } }
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
    /// One `waitpid` (EINTR retried): `Ok(Some)` reaped, `Ok(None)` still running (WNOHANG).
    fn waitpid_once(pid: libc::pid_t, nohang: bool) -> io::Result<Option<Exit>> {
        loop {
            let mut status = 0;
            let r = unsafe { libc::waitpid(pid, &mut status, if nohang { libc::WNOHANG } else { 0 }) };
            if r == pid {
                return Ok(Some(if libc::WIFEXITED(status) { Exit::Code(libc::WEXITSTATUS(status)) } else if libc::WIFSIGNALED(status) { Exit::Signal(libc::WTERMSIG(status)) } else { Exit::NotConfirmed { waited: Duration::ZERO } }));
            }
            if r == 0 { return Ok(None); }
            let e = io::Error::last_os_error();
            if e.raw_os_error() == Some(libc::EINTR) { continue; }
            return Err(e);
        }
    }
    /// Poll for the failed child's exit for a short while (its exec outcome may be unknown, so
    /// no unbounded wait).
    fn reap_briefly(pid: libc::pid_t) -> Option<Exit> {
        let deadline = Instant::now() + Duration::from_millis(500);
        loop {
            match waitpid_once(pid, true) { Ok(Some(e)) => return Some(e), Ok(None) => {}, Err(_) => return None }
            if Instant::now() >= deadline { return None; }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    /// What the parent read from the errno channel.
    #[derive(Debug, PartialEq, Eq)]
    enum Channel { Eof, Errno(i32), Partial(usize), ReadError(String) }
    fn read_errno_channel(err_r: &mut File) -> Channel {
        #[cfg(test)]
        let inject = super::hooks::ERRNO_CHANNEL.with(|c| c.get());
        #[cfg(not(test))]
        let inject: Option<super::hooks_placeholder::Never> = None;
        let mut buf = [0u8; 4];
        let mut got = 0usize;
        #[allow(unused_mut)]
        let mut eintr_injected = false;
        loop {
            #[cfg(test)]
            match inject {
                Some(super::hooks::ErrnoInject::Partial) => return Channel::Partial(2),
                Some(super::hooks::ErrnoInject::ReadError) => return Channel::ReadError("injected read error".into()),
                Some(super::hooks::ErrnoInject::EintrThenReal) if !eintr_injected => { eintr_injected = true; continue; }
                _ => {}
            }
            let _ = &inject; let _ = eintr_injected;
            match err_r.read(&mut buf[got..]) {
                Ok(0) => return if got == 0 { Channel::Eof } else { Channel::Partial(got) },
                Ok(n) => { got += n; if got == 4 { return Channel::Errno(i32::from_ne_bytes(buf)); } }
                Err(e) if e.raw_os_error() == Some(libc::EINTR) => continue,
                Err(e) => return Channel::ReadError(e.to_string()),
            }
        }
    }

    fn pipe() -> io::Result<(File, File)> {
        let mut fds = [0i32; 2];
        if unsafe { libc::pipe(fds.as_mut_ptr()) } < 0 { return Err(io::Error::last_os_error()); }
        for fd in fds { if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 { return Err(io::Error::last_os_error()); } }
        Ok(unsafe { (File::from_raw_fd(fds[0]), File::from_raw_fd(fds[1])) })
    }
    unsafe extern "C" { static environ: *const *const libc::c_char; }

    /// `fork` + `execve` with no path search and no shell fallback. Child side (async-signal-safe
    /// only): `stat` of the path compared with the pinned identity, `dup2` of the pipe ends,
    /// `execve`; on any failure the errno is written to a CLOEXEC pipe and the child `_exit`s.
    pub fn spawn(target: &Target, args: &[&str], piped: bool) -> Result<Spawned, SpawnError> {
        let pinned = target.pinned.metadata().map(|m| (m.dev(), m.ino()))?;
        let path = CString::new(target.path.as_os_str().as_encoded_bytes()).map_err(|_| io::Error::other("path contains NUL"))?;
        let mut argv: Vec<CString> = vec![path.clone()];
        for a in args { argv.push(CString::new(*a).map_err(|_| io::Error::other("argument contains NUL"))?); }
        let mut argv_ptrs: Vec<*const libc::c_char> = argv.iter().map(|a| a.as_ptr()).collect();
        argv_ptrs.push(std::ptr::null());
        let (err_r, err_w) = pipe()?;
        let (stdin_pair, stdout_pair, stderr_pair) = if piped { (Some(pipe()?), Some(pipe()?), Some(pipe()?)) } else { (None, None, None) };
        let child_fds = (stdin_pair.as_ref().map(|p| p.0.as_raw_fd()), stdout_pair.as_ref().map(|p| p.1.as_raw_fd()), stderr_pair.as_ref().map(|p| p.1.as_raw_fd()));
        let err_fd = err_w.as_raw_fd();
        let path_ptr: *const libc::c_char = path.as_ptr();
        // SAFETY: after fork, the child only calls async-signal-safe functions and touches
        // memory that was fully built before the fork.
        let pid = unsafe { libc::fork() };
        if pid < 0 { return Err(SpawnError::from(io::Error::last_os_error())); }
        if pid == 0 {
            unsafe {
                let mut st: libc::stat = std::mem::zeroed();
                if libc::stat(path_ptr, &mut st) < 0 { fail(err_fd, *libc::__error()); }
                if (st.st_dev as u64, st.st_ino as u64) != pinned { fail(err_fd, libc::ESTALE); }
                if let Some(fd) = child_fds.0 { if libc::dup2(fd, 0) < 0 { fail(err_fd, *libc::__error()); } }
                if let Some(fd) = child_fds.1 { if libc::dup2(fd, 1) < 0 { fail(err_fd, *libc::__error()); } }
                if let Some(fd) = child_fds.2 { if libc::dup2(fd, 2) < 0 { fail(err_fd, *libc::__error()); } }
                libc::execve(path_ptr, argv_ptrs.as_ptr(), environ);
                fail(err_fd, *libc::__error());
            }
        }
        drop(err_w);
        let (stdin, stdout, stderr) = match (stdin_pair, stdout_pair, stderr_pair) {
            (Some((_r, w)), Some((r2, _w2)), Some((r3, _w3))) => (Some(w), Some(r2), Some(r3)),
            _ => (None, None, None),
        };
        let mut err_r = err_r;
        let answer = read_errno_channel(&mut err_r);
        // clean EOF: the channel closed on exec, the Core is running
        if matches!(answer, Channel::Eof) { return Ok(Spawned { pid, stdin, stdout, stderr, reaped: None }); }
        // every failure path: close our pipe ends first, so a child that did exec and reads
        // stdin sees the EOF and can exit before the brief reap poll
        drop(stdin); drop(stdout); drop(stderr);
        match answer {
            Channel::Eof => unreachable!(),
            // complete answer: the child failed before or at exec and exits 127; reap it
            Channel::Errno(errno) => {
                let exit = reap_briefly(pid);
                let e = io::Error::from_raw_os_error(errno);
                Err(SpawnError { message: format!("{e}; child pid {pid} {}", match &exit { Some(x) => format!("reaped ({x:?})"), None => "not reaped within 500 ms".into() }), replaced: errno == libc::ESTALE, started: Started::No, child: Some(FailedChild { pid: pid as u32, exit }) })
            }
            // partial answer or read error: the exec outcome is unknown; never treated as success
            Channel::Partial(n) => {
                let exit = reap_briefly(pid);
                Err(SpawnError { message: format!("errno channel answered {n} of 4 bytes (exec outcome unknown); child pid {pid} {}", describe(&exit)), replaced: false, started: Started::Unknown, child: Some(FailedChild { pid: pid as u32, exit }) })
            }
            Channel::ReadError(why) => {
                let exit = reap_briefly(pid);
                Err(SpawnError { message: format!("errno channel read failed: {why} (exec outcome unknown); child pid {pid} {}", describe(&exit)), replaced: false, started: Started::Unknown, child: Some(FailedChild { pid: pid as u32, exit }) })
            }
        }
    }
    fn describe(exit: &Option<Exit>) -> String { match exit { Some(x) => format!("reaped ({x:?})"), None => "still running or not reapable within 500 ms: the caller ends and reaps it".into() } }
    /// Child side only: report errno through the pipe and exit without running destructors.
    unsafe fn fail(err_fd: i32, errno: i32) -> ! {
        let bytes = errno.to_ne_bytes();
        unsafe { libc::write(err_fd, bytes.as_ptr().cast(), 4) };
        unsafe { libc::_exit(127) }
    }
}

#[cfg(windows)]
mod lfs {
    use super::{Exit, FailedChild, Started, Target};
    use crate::runtime_observe::platform::Dir;
    use crate::windows_fs as win;
    use std::ffi::OsStr;
    use std::fs::File;
    use std::io::{self, Read, Write};
    use std::path::Path;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    use windows_sys::Win32::Storage::FileSystem::{FILE_EXECUTE, FILE_SHARE_READ, OPEN_EXISTING};

    /// Opened for read+execute with **read sharing only**: a rename over it, a delete or a write
    /// to it fails with a sharing violation while this handle lives (AT-P2).
    pub fn pin_executable_at(vdir: &Dir, name: &str) -> io::Result<File> {
        win::open_entry_at(vdir.as_file(), OsStr::new(name), win::READ | FILE_EXECUTE, FILE_SHARE_READ, Some(false))
    }
    pub fn pin_executable_path(path: &Path) -> io::Result<File> {
        let f = win::open_file(path, win::READ | FILE_EXECUTE, FILE_SHARE_READ, OPEN_EXISTING)?;
        if !win::Stamp::of(&f)?.is_single_regular() { return Err(io::Error::other("not a regular file with one link")); }
        Ok(f)
    }
    #[derive(Debug)]
    pub struct SpawnError { pub message: String, pub replaced: bool, pub started: Started, pub child: Option<FailedChild> }
    impl From<io::Error> for SpawnError { fn from(e: io::Error) -> Self { SpawnError { message: e.to_string(), replaced: false, started: Started::No, child: None } } }

    #[derive(Debug)]
    pub struct Spawned { inner: std::process::Child }
    impl Spawned {
        pub fn pid(&self) -> u32 { self.inner.id() }
        pub fn take_stdout(&mut self) -> Option<Box<dyn Read + Send>> { self.inner.stdout.take().map(|s| Box::new(s) as Box<dyn Read + Send>) }
        pub fn take_stderr(&mut self) -> Option<Box<dyn Read + Send>> { self.inner.stderr.take().map(|s| Box::new(s) as Box<dyn Read + Send>) }
        /// Anonymous pipes have no non-blocking mode: a writer thread delivers the input and
        /// closes stdin when its write ends or fails. Past the deadline the thread keeps stdin
        /// (no forced cancel): that is reported as `Timeout { stdin_closed: false }`, never as
        /// "closed" or "EOF delivered". A thread that ends without reporting is `Unknown`.
        pub fn write_stdin_and_close(&mut self, input: &[u8], deadline: Option<Instant>) -> super::Delivery {
            use super::Delivery;
            let Some(mut stdin) = self.inner.stdin.take() else { return Delivery::Complete };
            let data = input.to_vec();
            let total = data.len();
            let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
            std::thread::spawn(move || { let r = stdin.write_all(&data); drop(stdin); let _ = tx.send(r.map_err(|e| e.to_string())); });
            let result = match deadline { None => rx.recv().map_err(|_| std::sync::mpsc::RecvTimeoutError::Disconnected), Some(d) => rx.recv_timeout(d.saturating_duration_since(Instant::now())) };
            match result {
                Ok(Ok(())) => Delivery::Complete,
                Ok(Err(e)) => Delivery::Failed(e),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    #[cfg(test)]
                    super::hooks::LAST_WRITER.with(|w| *w.borrow_mut() = Some(rx));
                    // delivered bytes are not observable from outside a blocking write_all
                    Delivery::Timeout { delivered: 0, total, stdin_closed: false }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Delivery::Unknown("writer thread ended without a result".into()),
            }
        }
        pub fn wait_until(&mut self, deadline: Option<Instant>) -> Exit {
            let started = Instant::now();
            match deadline {
                None => match self.inner.wait() { Ok(s) => s.code().map(Exit::Code).unwrap_or(Exit::NotConfirmed { waited: started.elapsed() }), Err(_) => Exit::NotConfirmed { waited: started.elapsed() } },
                Some(d) => loop {
                    match self.inner.try_wait() {
                        Ok(Some(s)) => return s.code().map(Exit::Code).unwrap_or(Exit::NotConfirmed { waited: started.elapsed() }),
                        Ok(None) if Instant::now() >= d => return Exit::NotConfirmed { waited: started.elapsed() },
                        Ok(None) => std::thread::sleep(Duration::from_millis(5)),
                        Err(_) => return Exit::NotConfirmed { waited: started.elapsed() },
                    }
                },
            }
        }
    }
    /// `CreateProcess` by path (no shell, no path search for an absolute path); the pinned
    /// handle held by the caller is what keeps that path's file from being replaced meanwhile.
    pub fn spawn(target: &Target, args: &[&str], piped: bool) -> Result<Spawned, SpawnError> {
        let mut c = Command::new(&target.path);
        c.args(args);
        if piped { c.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()); } else { c.stdin(Stdio::inherit()).stdout(Stdio::inherit()).stderr(Stdio::inherit()); }
        Ok(Spawned { inner: c.spawn()? })
    }
}

#[cfg(test)]
#[path = "runtime_launch_tests.rs"]
mod tests;
