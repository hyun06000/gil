#[cfg(windows)]
use super::windows::{
    Listener as UnixListener, Stream as UnixStream, connect, nonce, parent, peer,
};
use super::*;
use crate::companion_session::{ConnectionId, Controller, HEARTBEAT_INTERVAL, LEASE_TIMEOUT};
use serde::{Deserialize, Serialize};
#[cfg(target_os = "macos")]
use std::fs;
use std::io::{Read, Write};
#[cfg(target_os = "macos")]
use std::os::fd::{AsRawFd, FromRawFd};
#[cfg(target_os = "macos")]
use std::os::unix::ffi::OsStrExt;
#[cfg(target_os = "macos")]
use std::os::unix::{
    fs::{FileTypeExt, MetadataExt},
    net::{UnixListener, UnixStream},
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const LIMIT: usize = 1024;
const MAX_PEERS: usize = 128;
const IO_LIMIT: Duration = Duration::from_secs(2);
const POLL: Duration = Duration::from_millis(100);

#[derive(Serialize, Deserialize)]
#[serde(tag = "op", deny_unknown_fields)]
enum Frame {
    Challenge {
        nonce: String,
    },
    Register {
        nonce: String,
        version: String,
        sha256: String,
    },
    Ack {
        sequence: u64,
    },
    Heartbeat {
        sequence: u64,
    },
    Response {
        sequence: u64,
    },
}
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn encode(frame: &Frame) -> io::Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec(frame)?;
    bytes.push(b'\n');
    if bytes.len() > LIMIT {
        return Err(invalid("IPC frame exceeds limit"));
    }
    Ok(bytes)
}
#[cfg(target_os = "macos")]
pub(super) fn parent(path: &Path) -> io::Result<()> {
    if !path.is_absolute() {
        return Err(invalid("IPC endpoint must be absolute"));
    }
    let dir = path.parent().ok_or_else(|| invalid("IPC parent missing"))?;
    // Require a canonical private parent; no creation/chmod/ACL repair here.
    if dir.canonicalize()? != dir {
        return Err(invalid("IPC parent must be canonical"));
    }
    let m = fs::symlink_metadata(dir)?;
    if !m.is_dir() || m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o777 != 0o700 {
        return Err(invalid(
            "IPC parent must be an existing owner-only directory",
        ));
    }
    Ok(())
}
#[cfg(target_os = "macos")]
fn peer(stream: &UnixStream) -> io::Result<()> {
    let (mut uid, mut gid) = (0, 0);
    if unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) } != 0 {
        return Err(io::Error::last_os_error());
    }
    if uid != unsafe { libc::geteuid() } {
        return Err(invalid("IPC peer user mismatch"));
    }
    Ok(())
}
#[cfg(target_os = "macos")]
pub(super) fn nonce() -> io::Result<String> {
    let mut bytes = [0u8; 32];
    fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(target_os = "macos")]
fn connect(path: &Path) -> io::Result<UnixStream> {
    // std's blocking connect has no deadline; use a nonblocking socket even for the handshake.
    let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    let bytes = path.as_os_str().as_bytes();
    if bytes.contains(&0) || bytes.len() >= address.sun_path.len() {
        return Err(invalid("invalid or oversized IPC path"));
    }
    address.sun_family = libc::AF_UNIX as _;
    address.sun_len = std::mem::size_of::<libc::sockaddr_un>() as _;
    for (target, byte) in address.sun_path.iter_mut().zip(bytes) {
        *target = *byte as _;
    }
    let fd = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let stream = unsafe { UnixStream::from_raw_fd(fd) };
    // Avoid leaking native session descriptors into later spawned children.
    if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
        return Err(io::Error::last_os_error());
    }
    stream.set_nonblocking(true)?;
    let rc = unsafe {
        libc::connect(
            fd,
            &address as *const _ as *const libc::sockaddr,
            std::mem::size_of_val(&address) as _,
        )
    };
    if rc != 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::EINPROGRESS) {
            return Err(error);
        }
        let deadline = Instant::now() + IO_LIMIT;
        loop {
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "IPC connect timeout",
                ));
            }
            let mut ready = libc::pollfd {
                fd,
                events: libc::POLLOUT,
                revents: 0,
            };
            let rc = unsafe { libc::poll(&mut ready, 1, 10) };
            if rc > 0 {
                break;
            }
            if rc < 0 && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
                return Err(io::Error::last_os_error());
            }
        }
        if let Some(error) = stream.take_error()? {
            return Err(error);
        }
    }
    Ok(stream)
}

/// Single bounded line; nonblocking calls allow an absolute deadline, not a slow-drip timeout.
fn read_frame(stream: &mut UnixStream, buffer: &mut Vec<u8>) -> io::Result<Option<Frame>> {
    loop {
        let mut one = [0u8];
        match stream.read(&mut one) {
            Ok(0) => return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "IPC EOF")),
            Ok(_) => {
                buffer.push(one[0]);
                if buffer.len() > LIMIT {
                    return Err(invalid("IPC frame exceeds limit"));
                }
                if one[0] == b'\n' {
                    let frame = serde_json::from_slice(buffer)?;
                    buffer.clear();
                    return Ok(Some(frame));
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(None),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
}
fn receive(stream: &mut UnixStream) -> io::Result<Frame> {
    let deadline = Instant::now() + IO_LIMIT;
    let mut buffer = Vec::new();
    loop {
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "IPC response timeout",
            ));
        }
        if let Some(frame) = read_frame(stream, &mut buffer)? {
            return Ok(frame);
        }
        thread::sleep(Duration::from_millis(10));
    }
}
fn send(stream: &mut UnixStream, frame: &Frame) -> io::Result<()> {
    // A partial write/WouldBlock closes the connection; it is never retried as a new frame.
    stream.write_all(&encode(frame)?)
}

struct Peer {
    stream: UnixStream,
    buffer: Vec<u8>,
    nonce: String,
    opened: Instant,
    id: Option<ConnectionId>,
    sequence: u64,
    last_frame: Instant,
}
impl Peer {
    fn accept(mut stream: UnixStream) -> io::Result<Self> {
        peer(&stream)?;
        stream.set_nonblocking(true)?;
        let nonce = nonce()?;
        send(
            &mut stream,
            &Frame::Challenge {
                nonce: nonce.clone(),
            },
        )?;
        Ok(Self {
            stream,
            buffer: Vec::new(),
            nonce,
            opened: Instant::now(),
            id: None,
            sequence: 0,
            last_frame: Instant::now(),
        })
    }
    fn step(&mut self, controller: &mut Controller) -> io::Result<()> {
        let now = Instant::now();
        if self.id.is_none() && now.duration_since(self.opened) >= IO_LIMIT {
            return Err(invalid("registration timeout"));
        }
        if now.duration_since(self.last_frame) >= LEASE_TIMEOUT {
            return Err(invalid("lease expired"));
        }
        let Some(frame) = read_frame(&mut self.stream, &mut self.buffer)? else {
            return Ok(());
        };
        let is_response = matches!(&frame, Frame::Response { .. });
        match (self.id, frame) {
            (
                None,
                Frame::Register {
                    nonce,
                    version,
                    sha256,
                },
            ) if nonce == self.nonce => {
                let release =
                    Release::new(&version, &sha256).map_err(|_| invalid("invalid release"))?;
                self.id = Some(
                    controller
                        .project_connected(release, now)
                        .map_err(|_| invalid("registration refused"))?,
                );
            }
            (Some(id), Frame::Heartbeat { sequence } | Frame::Response { sequence })
                if self.sequence.checked_add(1) == Some(sequence) =>
            {
                if !controller
                    .heartbeat(id, now)
                    .map_err(|_| invalid("clock error"))?
                {
                    return Err(invalid("expired connection"));
                }
                // Response confirmation uses a separate client event below; these frames cannot
                // offer, approve, prepare or publish an update.
                if is_response {
                    controller
                        .response_confirmed(id, now)
                        .map_err(|_| invalid("response refused"))?;
                }
                self.sequence = sequence;
            }
            _ => return Err(invalid("unexpected or stale IPC frame")),
        }
        self.last_frame = now;
        send(
            &mut self.stream,
            &Frame::Ack {
                sequence: self.sequence,
            },
        )
    }
}

pub struct Server {
    controller: Arc<Mutex<Controller>>,
    stop: Arc<AtomicBool>,
    finished: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}
impl Server {
    pub fn bind(path: &Path) -> io::Result<Self> {
        parent(path)?;
        // bind fails on any existing endpoint, including a stale one. No unlink race or takeover.
        #[allow(unused_mut)] // Windows accept replaces its pending pipe instance.
        let mut listener = UnixListener::bind(path)?;
        listener.set_nonblocking(true)?;
        let controller = Arc::new(Mutex::new(Controller::new(Instant::now())));
        let stop = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicBool::new(false));
        let (state, stopping, done) = (controller.clone(), stop.clone(), finished.clone());
        let thread = thread::Builder::new()
            .name("gil-companion-sessions".into())
            .spawn(move || {
                let mut peers: Vec<Peer> = Vec::new();
                while !stopping.load(Ordering::Acquire) {
                    // Bounded accepts and work per tick; idle unauthenticated clients cannot block others.
                    for _ in 0..16 {
                        match listener.accept() {
                            Ok((stream, _)) if peers.len() < MAX_PEERS => {
                                if let Ok(one) = Peer::accept(stream) {
                                    peers.push(one);
                                }
                            }
                            Ok(_) => {}
                            Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                            Err(_) => {
                                stopping.store(true, Ordering::Release);
                                break;
                            }
                        }
                    }
                    let Ok(mut state) = state.lock() else { break };
                    peers.retain_mut(|one| {
                        if one.step(&mut state).is_ok() {
                            return true;
                        }
                        if let Some(id) = one.id {
                            let _ = state.disconnected(id, Instant::now());
                        }
                        false
                    });
                    if state
                        .snapshot(Instant::now())
                        .map_or(true, |s| s.should_exit)
                    {
                        // Serialize the last-disconnect/registration race before announcing shutdown.
                        state.request_exit();
                        break;
                    }
                    drop(state);
                    thread::sleep(POLL);
                }
                drop(peers); // persistent streams close; clients do not restart the app.
                done.store(true, Ordering::Release);
            })?;
        Ok(Self {
            controller,
            stop,
            finished,
            thread: Some(thread),
        })
    }
    pub fn snapshot(&self) -> io::Result<Snapshot> {
        self.controller
            .lock()
            .map_err(|_| invalid("session state poisoned"))?
            .snapshot(Instant::now())
            .map_err(|_| invalid("session clock error"))
    }
    pub fn finished(&self) -> bool {
        self.finished.load(Ordering::Acquire)
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        // Leave the endpoint in its caller-owned private directory. No name-based deletion.
    }
}

enum Event {
    Response,
    Stop,
}
pub struct Client {
    events: mpsc::SyncSender<Event>,
    alive: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}
impl Client {
    pub fn connect(path: &Path, release: &Release) -> io::Result<Self> {
        parent(path)?;
        #[cfg(target_os = "macos")]
        {
            let metadata = fs::symlink_metadata(path)?;
            if !metadata.file_type().is_socket() || metadata.uid() != unsafe { libc::geteuid() } {
                return Err(invalid("invalid IPC endpoint"));
            }
        }
        let mut stream = connect(path)?;
        peer(&stream)?;
        stream.set_nonblocking(true)?;
        let Frame::Challenge { nonce } = receive(&mut stream)? else {
            return Err(invalid("missing challenge"));
        };
        if nonce.len() != 64 || !nonce.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(invalid("invalid challenge"));
        }
        let identity = serde_json::to_value(release)?;
        send(
            &mut stream,
            &Frame::Register {
                nonce,
                version: identity["version"].as_str().unwrap().into(),
                sha256: identity["sha256"].as_str().unwrap().into(),
            },
        )?;
        if !matches!(receive(&mut stream)?, Frame::Ack { sequence: 0 }) {
            return Err(invalid("registration not acknowledged"));
        }
        let (events, rx) = mpsc::sync_channel(1);
        let alive = Arc::new(AtomicBool::new(true));
        let running = alive.clone();
        let thread = thread::Builder::new().name("gil-companion-lease".into()).spawn(move || {
            let mut sequence = 0u64;
            loop {
                let response = match rx.recv_timeout(HEARTBEAT_INTERVAL) {
                    Ok(Event::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Ok(Event::Response) => true,
                    Err(mpsc::RecvTimeoutError::Timeout) => false,
                };
                let Some(next) = sequence.checked_add(1) else { break };
                sequence = next;
                let frame = if response { Frame::Response { sequence } } else { Frame::Heartbeat { sequence } };
                if send(&mut stream, &frame).is_err() || !matches!(receive(&mut stream), Ok(Frame::Ack { sequence: n }) if n == sequence) { break; }
            }
            running.store(false, Ordering::Release);
        })?;
        Ok(Self {
            events,
            alive,
            thread: Some(thread),
        })
    }
    pub fn alive(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }
    pub fn response(&self) -> bool {
        self.events.try_send(Event::Response).is_ok()
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        // At most one queued response precedes Stop. Each exchange has a two-second deadline;
        // drop never waits for the fifteen-second heartbeat timer.
        let _ = self.events.send(Event::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(all(test, target_os = "macos"))]
#[path = "mac/tests.rs"]
mod tests;

#[cfg(all(test, windows))]
mod windows_protocol_tests {
    use super::*;
    fn path() -> std::path::PathBuf {
        format!(r"\\.\pipe\gil-companion-v1-protocol-{}", nonce().unwrap()).into()
    }
    #[test]
    fn bad_registration_and_oversized_frame_never_create_a_lease() {
        let path = path();
        let server = Server::bind(&path).unwrap();
        for bytes in [
            encode(&Frame::Register {
                nonce: "wrong".into(),
                version: "1".into(),
                sha256: "a".repeat(64),
            })
            .unwrap(),
            vec![b'x'; LIMIT + 1],
        ] {
            let mut stream = connect(&path).unwrap();
            assert!(matches!(
                receive(&mut stream).unwrap(),
                Frame::Challenge { .. }
            ));
            stream.write_all(&bytes).unwrap();
            assert!(receive(&mut stream).is_err());
            assert_eq!(server.snapshot().unwrap().active_connections, 0);
        }
    }
    #[test]
    fn idle_handshake_does_not_block_an_authenticated_client() {
        let path = path();
        let server = Server::bind(&path).unwrap();
        let mut idle = connect(&path).unwrap();
        assert!(matches!(
            receive(&mut idle).unwrap(),
            Frame::Challenge { .. }
        ));
        let client =
            Client::connect(&path, &Release::new("fixture", &"a".repeat(64)).unwrap()).unwrap();
        assert_eq!(server.snapshot().unwrap().active_connections, 1);
        drop(idle);
        drop(client);
        let end = Instant::now() + Duration::from_secs(5);
        while !server.finished() {
            assert!(Instant::now() < end);
            thread::sleep(POLL);
        }
    }
}
