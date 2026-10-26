use super::*;
use crate::companion_ipc::{Client, Server};
use crate::companion_session::Release;
use std::time::{Duration, Instant};
fn path() -> PathBuf {
    PathBuf::from(format!(
        r"\\.\pipe\gil-companion-v1-test-{}",
        nonce().unwrap()
    ))
}
fn until(mut predicate: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(6);
    while !predicate() {
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(20));
    }
}
#[test]
fn two_authenticated_connections_share_server_and_last_close_exits() {
    let path = path();
    let server = Server::bind(&path).unwrap();
    assert!(
        Server::bind(&path).is_err(),
        "never take over an occupied name"
    );
    let release = Release::new("fixture", &"a".repeat(64)).unwrap();
    let a = Client::connect(&path, &release).unwrap();
    let b = Client::connect(&path, &release).unwrap();
    assert_eq!(server.snapshot().unwrap().active_connections, 2);
    assert!(a.response());
    drop(a);
    until(|| server.snapshot().unwrap().active_connections == 1);
    assert!(!server.finished());
    drop(b);
    until(|| server.finished());
}
#[test]
fn byte_stream_distinguishes_empty_from_eof_and_verifies_both_peers() {
    let path = path();
    let mut listener = Listener::bind(&path).unwrap();
    assert!(matches!(listener.accept(), Err(e) if e.kind()==io::ErrorKind::WouldBlock));
    let mut client = connect(&path).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut accepted = loop {
        assert!(Instant::now() < deadline, "pipe accept timed out");
        match listener.accept() {
            Ok((s, _)) => break s,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10))
            }
            Err(e) => panic!("{e}"),
        }
    };
    peer(&client).unwrap();
    peer(&accepted).unwrap();
    let mut byte = [0];
    assert!(matches!(accepted.read(&mut byte), Err(e) if e.kind()==io::ErrorKind::WouldBlock));
    client.write_all(b"x").unwrap();
    until(|| accepted.read(&mut byte).is_ok_and(|n| n == 1));
    assert_eq!(byte, *b"x");
    drop(client);
    until(|| accepted.read(&mut byte).is_ok_and(|n| n == 0));
}
#[test]
fn remote_or_arbitrary_pipe_names_are_refused_before_opening() {
    for name in [
        r"\\remote\pipe\gil-companion-v1-x",
        r"\\.\pipe\other",
        r"\\.\pipe\gil-companion-v1-x\y",
    ] {
        assert!(parent(Path::new(name)).is_err());
        assert!(connect(Path::new(name)).is_err());
    }
}
