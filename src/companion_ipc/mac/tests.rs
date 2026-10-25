use super::*;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = PathBuf::from("/private/tmp").join(format!(
            "gil-session-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
            nonce().unwrap()[..8].to_string()
        ));
        fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        Self(dir)
    }
    fn socket(&self) -> PathBuf {
        self.0.join("session.sock")
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn release() -> Release {
    Release::new("test.1", &"a".repeat(64)).unwrap()
}
fn until(mut check: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !check() {
        assert!(Instant::now() < deadline, "timed out");
        thread::sleep(Duration::from_millis(20));
    }
}
fn raw(path: &Path) -> UnixStream {
    let stream = UnixStream::connect(path).unwrap();
    stream.set_nonblocking(true).unwrap();
    stream
}

#[test]
fn persistent_connections_share_one_server_and_last_eof_exits() {
    let dir = Scratch::new();
    let server = Server::bind(&dir.socket()).unwrap();
    assert_eq!(server.snapshot().unwrap().active_connections, 0);
    let a = Client::connect(&dir.socket(), &release()).unwrap();
    let b = Client::connect(&dir.socket(), &release()).unwrap();
    assert_eq!(server.snapshot().unwrap().active_connections, 2);
    assert!(a.response());
    drop(a);
    until(|| server.snapshot().unwrap().active_connections == 1);
    assert!(!server.finished());
    drop(b);
    until(|| server.finished());
    assert!(server.snapshot().unwrap().exit_requested);
    assert!(Client::connect(&dir.socket(), &release()).is_err());
    // Caller-owned endpoint remains; no unlink or takeover.
    assert!(
        fs::symlink_metadata(dir.socket())
            .unwrap()
            .file_type()
            .is_socket()
    );
}

#[test]
fn malformed_oversized_wrong_challenge_and_replayed_frames_never_register() {
    let dir = Scratch::new();
    let server = Server::bind(&dir.socket()).unwrap();
    for bad in [
        b"not json\n".to_vec(),
        vec![b'x'; LIMIT + 1],
        encode(&Frame::Register {
            nonce: "wrong".into(),
            version: "1".into(),
            sha256: "a".repeat(64),
        })
        .unwrap(),
        b"{\"op\":\"approve\"}\n".to_vec(),
    ] {
        let mut stream = raw(&dir.socket());
        assert!(matches!(
            receive(&mut stream).unwrap(),
            Frame::Challenge { .. }
        ));
        stream.write_all(&bad).unwrap();
        assert!(receive(&mut stream).is_err());
        assert_eq!(server.snapshot().unwrap().active_connections, 0);
    }
    let mut stream = raw(&dir.socket());
    let Frame::Challenge { nonce } = receive(&mut stream).unwrap() else {
        panic!()
    };
    send(
        &mut stream,
        &Frame::Register {
            nonce,
            version: "1".into(),
            sha256: "a".repeat(64),
        },
    )
    .unwrap();
    assert!(matches!(
        receive(&mut stream).unwrap(),
        Frame::Ack { sequence: 0 }
    ));
    send(&mut stream, &Frame::Heartbeat { sequence: 1 }).unwrap();
    assert!(matches!(
        receive(&mut stream).unwrap(),
        Frame::Ack { sequence: 1 }
    ));
    send(&mut stream, &Frame::Heartbeat { sequence: 1 }).unwrap();
    assert!(receive(&mut stream).is_err());
    until(|| server.finished());
}

#[test]
fn idle_handshake_cannot_block_another_client_and_times_out() {
    let dir = Scratch::new();
    let server = Server::bind(&dir.socket()).unwrap();
    let mut idle = raw(&dir.socket());
    let Frame::Challenge { nonce: first } = receive(&mut idle).unwrap() else {
        panic!()
    };
    let mut other = raw(&dir.socket());
    let Frame::Challenge { nonce: second } = receive(&mut other).unwrap() else {
        panic!()
    };
    assert_ne!(first, second);
    let client = Client::connect(&dir.socket(), &release()).unwrap();
    thread::sleep(IO_LIMIT + Duration::from_millis(300));
    assert!(receive(&mut idle).is_err());
    assert!(client.alive());
    assert_eq!(server.snapshot().unwrap().active_connections, 1);
}

#[test]
fn endpoint_validation_never_repairs_permissions_or_replaces_existing_files() {
    let dir = Scratch::new();
    fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(Server::bind(&dir.socket()).is_err());
    assert_eq!(fs::metadata(&dir.0).unwrap().mode() & 0o777, 0o755);
    fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(dir.socket(), b"preserve").unwrap();
    assert!(Server::bind(&dir.socket()).is_err());
    assert!(Client::connect(&dir.socket(), &release()).is_err());
    assert_eq!(fs::read(dir.socket()).unwrap(), b"preserve");
    assert!(Server::bind(Path::new("relative.sock")).is_err());
}

#[test]
fn explicit_shutdown_disconnects_without_background_reconnect() {
    let dir = Scratch::new();
    let server = Server::bind(&dir.socket()).unwrap();
    let client = Client::connect(&dir.socket(), &release()).unwrap();
    drop(server);
    assert!(client.response());
    until(|| !client.alive());
    assert!(!Client::connect(&dir.socket(), &release()).is_ok());
}

#[test]
fn lease_expiry_cannot_be_revived_on_the_old_stream() {
    // Exercise the real stream handler with an expired policy entry, without a minute sleep.
    let (a, b) = UnixStream::pair().unwrap();
    b.set_nonblocking(true).unwrap();
    let mut b = b;
    let mut one = Peer::accept(a).unwrap();
    let Frame::Challenge { nonce } = receive(&mut b).unwrap() else {
        panic!()
    };
    let old = Instant::now() - LEASE_TIMEOUT - Duration::from_secs(1);
    let mut controller = Controller::new(old);
    one.id = Some(controller.project_connected(release(), old).unwrap());
    send(&mut b, &Frame::Heartbeat { sequence: 1 }).unwrap();
    assert!(one.step(&mut controller).is_err());
    assert_eq!(
        controller
            .snapshot(Instant::now())
            .unwrap()
            .active_connections,
        0
    );
    assert_eq!(nonce.len(), 64);
}
