//! Fixture tests for the isolated launch mechanism. Stores come from the preparation and
//! selection layers; the "Cores" are synthetic executables made here (Unix: shell scripts that
//! record a start trace, echo stdin and exit with a fixed code; Windows: copies of cmd.exe).
//! Nothing here downloads, trusts or installs anything.
use super::*;
use crate::runtime_prepare::{self as prep, Request};
use crate::runtime_select::{self as sel, SelectRequest};
use sha2::{Digest, Sha256};
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

const V1: &str = "0.2.1-dev.a";
const V2: &str = "0.2.2-dev.b";
fn sha(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

struct Fixture { base: PathBuf, root: PathBuf, trace: PathBuf, bundled: PathBuf }
impl Fixture {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let tmp = std::env::temp_dir();
        #[cfg(unix)]
        let tmp = fs::canonicalize(tmp).unwrap();
        let base = tmp.join(format!("gil-launch-{label}-{}-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir_all(&base).unwrap();
        #[cfg(unix)]
        { use std::os::unix::fs::PermissionsExt; fs::set_permissions(&base, fs::Permissions::from_mode(0o700)).unwrap(); }
        let trace = base.join("trace");
        let bundled = base.join("bundled-core");
        let fx = Self { root: base.join("runtime-v1"), base, trace, bundled };
        fs::write(&fx.bundled, synthetic_core(&fx.trace, "bundled", 3)).unwrap();
        make_executable(&fx.bundled);
        fx
    }
    fn core_bytes(&self, mark: &str, exit: i32) -> Vec<u8> { synthetic_core(&self.trace, mark, exit) }
    fn source(&self, name: &str, bytes: &[u8]) -> PathBuf { let p = self.base.join(name); fs::write(&p, bytes).unwrap(); p }
    fn prepare(&self, version: &str, bytes: &[u8]) -> String {
        let src = self.source(&format!("src-{version}"), bytes);
        let r = prep::prepare(&self.root, &Request { version, expected_sha256: &sha(bytes), source: &src }).unwrap();
        assert!(matches!(r.outcome, prep::Outcome::Prepared | prep::Outcome::AlreadyPrepared));
        sha(bytes)
    }
    fn select(&self, version: &str, digest: &str, expected: u64) -> u64 { sel::select(&self.root, &SelectRequest { version, sha256: digest, expected_generation: expected }).unwrap().generation }
    fn trace(&self) -> String { fs::read_to_string(&self.trace).unwrap_or_default() }
    fn core_path(&self, version: &str, digest: &str) -> PathBuf { self.root.join(VERSIONS_DIR).join(version_dirname(version, digest)).join(CORE_FILE) }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        prep::hooks::reset(); hooks::reset();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fn relax(p: &Path) { if let Ok(rd) = fs::read_dir(p) { for e in rd.flatten() { let q = e.path(); if fs::symlink_metadata(&q).map(|m| m.file_type().is_symlink()).unwrap_or(true) { continue; } let _ = fs::set_permissions(&q, fs::Permissions::from_mode(0o700)); if q.is_dir() { relax(&q); } } } }
            relax(&self.base);
        }
        let _ = fs::remove_dir_all(&self.base);
    }
}
fn out(r: &ExitReport) -> String { String::from_utf8_lossy(r.stdout_bytes()).into_owned() }

#[cfg(unix)]
fn synthetic_core(trace: &Path, mark: &str, exit: i32) -> Vec<u8> {
    format!("#!/bin/sh\nprintf 'started {mark}\\n' >> '{}'\nprintf 'child {mark} start\\n'\ncat\nprintf 'child {mark} end\\n'\nexit {exit}\n", trace.display()).into_bytes()
}
#[cfg(unix)]
fn make_executable(p: &Path) { use std::os::unix::fs::PermissionsExt; fs::set_permissions(p, fs::Permissions::from_mode(0o500)).unwrap(); }
#[cfg(windows)]
fn synthetic_core(_trace: &Path, mark: &str, _exit: i32) -> Vec<u8> {
    let mut bytes = fs::read(PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32").join("cmd.exe")).unwrap();
    bytes.extend_from_slice(format!("\n--gil-synthetic-{mark}--").as_bytes());
    bytes
}
#[cfg(windows)]
fn make_executable(_p: &Path) {}

#[test]
fn first_use_initializing_and_prepared_run_the_bundled_core() {
    let fx = Fixture::new("bundled");
    let (kind, r) = launch(&fx.root, &fx.bundled, FixedArgs::Synthetic, Some(b"hello\n")).unwrap();
    assert_eq!(kind, TargetKind::Bundled);
    #[cfg(unix)]
    { assert_eq!(out(&r), "child bundled start\nhello\nchild bundled end\n"); assert_eq!(r.exit, Exit::Code(3)); assert_eq!(fx.trace(), "started bundled\n"); }
    #[cfg(windows)]
    { assert!(out(&r).contains("hello")); assert_eq!(r.exit, Exit::Code(7)); }
    assert!(r.stdin_error.is_none() && matches!(r.stderr, Capture::Bytes(ref b) if b.is_empty()), "{r:?}");
    let a = fx.core_bytes("v1", 7);
    let src = fx.source("src-init", &a);
    prep::hooks::FAIL_AFTER.with(|h| h.set(Some(prep::Point::StoreWritten)));
    prep::prepare(&fx.root, &Request { version: V1, expected_sha256: &sha(&a), source: &src }).unwrap_err();
    prep::hooks::reset();
    assert_eq!(crate::runtime_observe::observe_and_judge(&fx.root).1.state, State::Initializing);
    let (kind, _) = launch(&fx.root, &fx.bundled, FixedArgs::Synthetic, Some(b"")).unwrap();
    assert_eq!(kind, TargetKind::Bundled);
    fx.prepare(V1, &a);
    assert_eq!(crate::runtime_observe::observe_and_judge(&fx.root).1.state, State::Prepared);
    let (kind, _) = launch(&fx.root, &fx.bundled, FixedArgs::Synthetic, Some(b"")).unwrap();
    assert_eq!(kind, TargetKind::Bundled);
    #[cfg(unix)]
    assert_eq!(fx.trace(), "started bundled\nstarted bundled\nstarted bundled\n", "only the bundled Core ever started");
}
#[test]
fn selected_runs_the_verified_selection_with_stdio_eof_and_exit_status() {
    let fx = Fixture::new("selected");
    let a = fx.core_bytes("v1", 7);
    let d = fx.prepare(V1, &a);
    assert_eq!(fx.select(V1, &d, 0), 1);
    let target = prepare_target(&fx.root, &fx.bundled).unwrap();
    assert_eq!(target.kind, TargetKind::Selected { generation: 1, version: V1.into(), sha256: d.clone() });
    assert_eq!(target.verified_sha256.as_deref(), Some(d.as_str()), "the digest is re-hashed through the pinned handle");
    assert_eq!(target.verdict.state, State::Selected { generation: 1, version: V1.into(), history_lagging: false });
    let (kind, r) = launch(&fx.root, &fx.bundled, FixedArgs::Synthetic, Some(b"line one\nline two\n")).unwrap();
    assert!(matches!(kind, TargetKind::Selected { generation: 1, .. }));
    assert!(r.pid > 0);
    #[cfg(unix)]
    {
        assert_eq!(out(&r), "child v1 start\nline one\nline two\nchild v1 end\n", "stdin reached the child and EOF ended it");
        assert_eq!(r.exit, Exit::Code(7)); assert_eq!(fx.trace(), "started v1\n");
    }
    #[cfg(windows)]
    { assert!(out(&r).contains("line one") && out(&r).contains("line two")); assert_eq!(r.exit, Exit::Code(7)); println!("OBSERVED exit confirmed on Windows: {:?} for pid {}", r.exit, r.pid); }
    assert!(r.stdin_error.is_none());
}
#[test]
fn the_selected_identity_comes_from_one_observation_even_if_the_selection_changes_right_after() {
    let fx = Fixture::new("one-observation");
    let a = fx.core_bytes("v1", 7); let b = fx.core_bytes("v2", 0);
    let da = fx.prepare(V1, &a); let db = fx.prepare(V2, &b);
    fx.select(V1, &da, 0);
    thread_local! { static DB: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) }; }
    DB.with(|d| *d.borrow_mut() = db.clone());
    fn select_v2(root: &Path) {
        let db = DB.with(|d| d.borrow().clone());
        assert_eq!(sel::select(root, &SelectRequest { version: V2, sha256: &db, expected_generation: 1 }).unwrap().generation, 2);
    }
    hooks::AFTER_OBSERVE.with(|h| h.set(Some(select_v2)));
    let target = prepare_target(&fx.root, &fx.bundled).unwrap();
    hooks::reset();
    // every field of the target is the observed v1 record; nothing is mixed with the new v2 record
    assert_eq!(target.kind, TargetKind::Selected { generation: 1, version: V1.into(), sha256: da.clone() });
    assert_eq!(target.verified_sha256.as_deref(), Some(da.as_str()));
    let r = spawn(target, FixedArgs::Synthetic, IoMode::Piped).unwrap().send_eof_and_finish(b"x\n", Some(Duration::from_secs(30)));
    assert_eq!(r.exit, Exit::Code(7), "{r:?}");
    #[cfg(unix)]
    assert_eq!(out(&r), "child v1 start\nx\nchild v1 end\n");
    // a fresh decision follows the new selection, again from one observation
    let target = prepare_target(&fx.root, &fx.bundled).unwrap();
    assert_eq!(target.kind, TargetKind::Selected { generation: 2, version: V2.into(), sha256: db });
}
#[test]
fn damaged_blocked_and_unverifiable_targets_run_nothing_and_never_fall_back() {
    let fx = Fixture::new("refuse");
    let a = fx.core_bytes("v1", 7);
    let d = fx.prepare(V1, &a);
    fx.select(V1, &d, 0);
    fs::rename(fx.root.join("selection.json"), fx.base.join("selection-moved")).unwrap();
    let e = prepare_target(&fx.root, &fx.bundled).unwrap_err();
    assert_eq!(e.stage, LaunchStage::Judge); assert!(matches!(e.verdict.as_ref().unwrap().state, State::Damaged { .. }), "{e:?}");
    assert!(e.reason.contains("no fallback") && e.spawned == Started::No);
    fs::rename(fx.base.join("selection-moved"), fx.root.join("selection.json")).unwrap();
    let core = fx.core_path(V1, &d);
    break_protection(&core);
    let e = prepare_target(&fx.root, &fx.bundled).unwrap_err();
    assert_eq!(e.stage, LaunchStage::Judge); assert!(matches!(e.verdict.as_ref().unwrap().state, State::Blocked { .. }), "{e:?}");
    restore_protection(&core);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&core, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(&core, fx.core_bytes("tampered", 0)).unwrap();
        fs::set_permissions(&core, fs::Permissions::from_mode(0o500)).unwrap();
        let e = prepare_target(&fx.root, &fx.bundled).unwrap_err();
        assert!(matches!(e.stage, LaunchStage::Judge | LaunchStage::Verify), "{e:?}");
        fs::set_permissions(&core, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(&core, &a).unwrap();
        fs::set_permissions(&core, fs::Permissions::from_mode(0o500)).unwrap();
    }
    assert_eq!(fx.trace(), "", "no child was started by any refused launch (and the bundled Core was not used as a fallback)");
}
#[cfg(unix)]
#[test]
fn images_the_kernel_refuses_are_refused_without_any_shell_and_without_fallback() {
    // Each "Core" below would run under /bin/sh if execvp's ENOEXEC fallback were in play; the
    // shell command inside would then write a trace. With execve there is no shell at all.
    let fx = Fixture::new("noexec");
    let trace = fx.trace.clone();
    let shell_payload = |label: &str| format!("printf 'shell ran {label}\\n' >> '{}'\n", trace.display());
    let images: Vec<(&str, Vec<u8>)> = vec![
        ("no magic, shell commands", shell_payload("plain").into_bytes()),
        ("Mach-O magic then garbage", { let mut v = vec![0xcf, 0xfa, 0xed, 0xfe]; v.extend_from_slice(shell_payload("macho").as_bytes()); v.extend_from_slice(&[0u8; 64]); v }),
        ("ELF magic then garbage", { let mut v = b"\x7fELF".to_vec(); v.extend_from_slice(shell_payload("elf").as_bytes()); v }),
        ("shebang to a missing interpreter", format!("#!/nonexistent/interpreter\n{}", shell_payload("shebang")).into_bytes()),
        ("empty file", Vec::new()),
    ];
    for (i, (what, bytes)) in images.iter().enumerate() {
        let version = format!("9.{i}.0");
        let d = fx.prepare(&version, bytes);
        fx.select(&version, &d, i as u64);
        let e = launch(&fx.root, &fx.bundled, FixedArgs::Synthetic, Some(b"")).unwrap_err();
        assert_eq!(e.stage, LaunchStage::Spawn, "{what}: {e:?}");
        assert!(e.reason.contains("no shell fallback") && e.spawned == Started::No, "{what}: {e:?}");
        // a complete errno answer: the forked child was reaped and its exit (127) reported
        let child = e.child.as_ref().expect("the forked child is reported");
        assert!(child.pid > 0 && child.exit == Some(Exit::Code(127)), "{what}: {e:?}");
        println!("OBSERVED {what}: {}", e.reason);
    }
    assert_eq!(fx.trace(), "", "no shell ran any of the refused images and the bundled Core never started");
}
#[cfg(unix)]
#[test]
fn a_replacement_after_pinning_is_refused_in_the_child_before_exec() {
    let fx = Fixture::new("pin");
    let a = fx.core_bytes("v1", 7);
    let d = fx.prepare(V1, &a);
    fx.select(V1, &d, 0);
    let target = prepare_target(&fx.root, &fx.bundled).unwrap();
    let planted = fx.base.join("planted");
    fs::write(&planted, fx.core_bytes("planted", 9)).unwrap();
    make_executable(&planted);
    fs::rename(&planted, fx.core_path(V1, &d)).unwrap();
    let e = spawn(target, FixedArgs::Synthetic, IoMode::Piped).unwrap_err();
    assert_eq!(e.stage, LaunchStage::Spawn); assert!(e.reason.contains("replaced") && e.spawned == Started::No, "{e:?}");
    assert_eq!(fx.trace(), "", "neither the verified nor the planted file started");
    println!("OBSERVED pin branch: replacement refused in the child before exec (ESTALE)");
    let e = prepare_target(&fx.root, &fx.bundled).unwrap_err();
    assert!(matches!(e.stage, LaunchStage::Judge | LaunchStage::Verify), "{e:?}");
    let restore = fx.base.join("restore"); fs::write(&restore, &a).unwrap(); make_executable(&restore);
    fs::rename(&restore, fx.core_path(V1, &d)).unwrap();
    let (_, r) = launch(&fx.root, &fx.bundled, FixedArgs::Synthetic, Some(b"x\n")).unwrap();
    assert_eq!(out(&r), "child v1 start\nx\nchild v1 end\n"); assert_eq!(r.exit, Exit::Code(7));
}
#[cfg(windows)]
#[test]
fn a_pinned_core_cannot_be_replaced_while_the_pin_is_held() {
    let fx = Fixture::new("pin");
    let a = fx.core_bytes("v1", 7);
    let d = fx.prepare(V1, &a);
    fx.select(V1, &d, 0);
    let target = prepare_target(&fx.root, &fx.bundled).unwrap();
    let planted = fx.base.join("planted.exe");
    fs::write(&planted, fx.core_bytes("planted", 9)).unwrap();
    let e = fs::rename(&planted, fx.core_path(V1, &d)).unwrap_err();
    println!("OBSERVED rename over a pinned Core refused: {e}");
    let r = spawn(target, FixedArgs::Synthetic, IoMode::Piped).unwrap().send_eof_and_finish(b"x\n", Some(Duration::from_secs(30)));
    assert_eq!(r.exit, Exit::Code(7));
}
#[cfg(windows)]
#[test]
fn w_a_blocked_stdin_writer_is_reported_honestly_and_ends_once_the_child_is_gone() {
    let fx = Fixture::new("w-writer");
    let a = fx.core_bytes("v1", 7);
    let d = fx.prepare(V1, &a);
    fx.select(V1, &d, 0);
    let child = spawn(prepare_target(&fx.root, &fx.bundled).unwrap(), FixedArgs::SyntheticIdle, IoMode::Piped).unwrap();
    let pid = child.id();
    let big = vec![b'z'; 8 * 1024 * 1024];
    let started = std::time::Instant::now();
    let r = child.send_eof_and_finish(&big, Some(Duration::from_secs(3)));
    let elapsed = started.elapsed();
    assert!(elapsed < Duration::from_secs(20), "bounded: {elapsed:?}");
    assert_eq!(r.pid, pid);
    let msg = r.stdin_error.clone().expect("timeout must be reported");
    assert!(msg.contains("NOT closed") && msg.contains("EOF not delivered"), "{r:?}");
    assert!(r.left_behind.iter().any(|l| l.contains("writer thread still holds stdin")), "{r:?}");
    assert!(matches!(r.exit, Exit::NotConfirmed { .. }), "{r:?}");
    assert!(matches!(r.stdout, Capture::Incomplete(_)), "{r:?}");
    println!("OBSERVED Windows stdin timeout: {msg}; exit {:?}; left behind {:?}", r.exit, r.left_behind);
    // the test ends the child tree it created; the writer's blocking write then fails and the
    // thread closes stdin and reports
    let kill = std::process::Command::new(PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/taskkill.exe")).args(["/PID", &pid.to_string(), "/T", "/F"]).output().unwrap();
    println!("OBSERVED taskkill /T /F for pid {pid}: {}", String::from_utf8_lossy(&kill.stdout).trim());
    assert!(kill.status.success(), "{}", String::from_utf8_lossy(&kill.stderr));
    let writer = hooks::LAST_WRITER.with(|w| w.borrow_mut().take()).expect("the writer channel of the timed-out write is kept for the test");
    let outcome = writer.recv_timeout(Duration::from_secs(20)).expect("the writer thread ends once the child is gone");
    println!("OBSERVED writer thread ended after the child's end: {outcome:?}");
    assert!(outcome.is_err(), "the pending write must fail once the reader side is gone: {outcome:?}");
}
#[test]
fn an_existing_v1_connection_keeps_running_while_v2_is_selected_and_new_launches_use_v2() {
    let fx = Fixture::new("flow");
    let a = fx.core_bytes("v1", 7); let b = fx.core_bytes("v2", 0);
    let da = fx.prepare(V1, &a); let db = fx.prepare(V2, &b);
    assert_eq!(fx.select(V1, &da, 0), 1);
    let mut a_child = spawn(prepare_target(&fx.root, &fx.bundled).unwrap(), FixedArgs::Synthetic, IoMode::Piped).unwrap();
    assert!(matches!(a_child.kind, TargetKind::Selected { generation: 1, .. }));
    assert_eq!(fx.select(V2, &db, 1), 2);
    let mut b_child = spawn(prepare_target(&fx.root, &fx.bundled).unwrap(), FixedArgs::Synthetic, IoMode::Piped).unwrap();
    assert!(matches!(&b_child.kind, TargetKind::Selected { generation: 2, version, .. } if version == V2));
    assert_ne!(a_child.id(), b_child.id());
    a_child.send_and_close(b"to v1\n").unwrap();
    b_child.send_and_close(b"to v2\n").unwrap();
    let ra = a_child.finish(None, Some(Duration::from_secs(30))); let rb = b_child.finish(None, Some(Duration::from_secs(30)));
    #[cfg(unix)]
    {
        assert_eq!(out(&ra), "child v1 start\nto v1\nchild v1 end\n"); assert_eq!(ra.exit, Exit::Code(7));
        assert_eq!(out(&rb), "child v2 start\nto v2\nchild v2 end\n"); assert_eq!(rb.exit, Exit::Code(0));
        assert_eq!(fx.trace(), "started v1\nstarted v2\n");
    }
    #[cfg(windows)]
    { assert!(out(&ra).contains("to v1")); assert!(out(&rb).contains("to v2")); }
}
#[cfg(unix)]
#[test]
fn errors_after_the_start_are_reported_with_the_pid_and_the_exit_confirmation() {
    // (a) the child exits without reading: the input write fails, stdin is still closed, the
    //     exit is confirmed and reported with the pid
    let fx = Fixture::new("post-start");
    let early = format!("#!/bin/sh\nprintf 'started early\\n' >> '{}'\nexit 5\n", fx.trace.display()).into_bytes();
    let d = fx.prepare(V1, &early);
    fx.select(V1, &d, 0);
    let child = spawn(prepare_target(&fx.root, &fx.bundled).unwrap(), FixedArgs::Synthetic, IoMode::Piped).unwrap();
    let pid = child.id();
    let big = vec![b'x'; 8 * 1024 * 1024];
    let r = child.send_eof_and_finish(&big, Some(Duration::from_secs(30)));
    assert_eq!(r.pid, pid);
    assert!(r.stdin_error.is_some(), "writing 8 MiB to a child that exited must fail: {r:?}");
    assert_eq!(r.exit, Exit::Code(5), "exit confirmed despite the stdin error");
    assert!(r.left_behind.is_empty(), "{r:?}");
    assert_eq!(fx.trace(), "started early\n");
    // (b) a reader failure is reported as such, never as empty output
    let fx2 = Fixture::new("post-start-reader");
    let a = fx2.core_bytes("v1", 7);
    let d2 = fx2.prepare(V1, &a);
    fx2.select(V1, &d2, 0);
    hooks::FAIL_READER.with(|c| c.set(true));
    let r = spawn(prepare_target(&fx2.root, &fx2.bundled).unwrap(), FixedArgs::Synthetic, IoMode::Piped).unwrap().send_eof_and_finish(b"x\n", Some(Duration::from_secs(30)));
    hooks::reset();
    assert!(matches!(r.stdout, Capture::Failed(ref m) if m.contains("injected")), "{r:?}");
    assert!(matches!(r.stderr, Capture::Bytes(_)));
    assert_eq!(r.exit, Exit::Code(7));
    // (c) a child that does not exit within the limit: NotConfirmed with the pid, never a code
    let fx3 = Fixture::new("post-start-hang");
    let hang = format!("#!/bin/sh\nprintf 'started hang\\n' >> '{}'\nsleep 30\n", fx3.trace.display()).into_bytes();
    let d3 = fx3.prepare(V1, &hang);
    fx3.select(V1, &d3, 0);
    let child = spawn(prepare_target(&fx3.root, &fx3.bundled).unwrap(), FixedArgs::Synthetic, IoMode::Inherit).unwrap();
    let pid = child.id();
    let r = child.finish(None, Some(Duration::from_millis(400)));
    assert_eq!(r.pid, pid);
    assert!(matches!(r.exit, Exit::NotConfirmed { waited } if waited >= Duration::from_millis(400)), "{r:?}");
    assert_eq!(r.stdout, Capture::NotCaptured);
    assert!(r.left_behind.iter().any(|l| l.contains(&format!("pid {pid}")) && l.contains("reaps")), "{r:?}");
    // the test ends the child it left running and confirms that
    unsafe { libc::kill(pid as i32, libc::SIGKILL) };
    let mut status = 0;
    assert_eq!(unsafe { libc::waitpid(pid as i32, &mut status, 0) }, pid as i32);
    assert!(libc::WIFSIGNALED(status));
}
#[cfg(unix)]
#[test]
fn a_finite_limit_bounds_input_exit_and_output_in_piped_mode() {
    // (a) the child neither reads stdin nor closes its output: every step is bounded, the report
    //     says what was not delivered, not confirmed and not complete, and names the leftovers
    let fx = Fixture::new("bounded");
    // `exec sleep`: the shell becomes the sleeper, so killing the child's pid ends everything
    let idle = format!("#!/bin/sh\nprintf 'started idle\\n' >> '{}'\nprintf 'idle up\\n'\nexec sleep 30\n", fx.trace.display()).into_bytes();
    let d = fx.prepare(V1, &idle);
    fx.select(V1, &d, 0);
    let child = spawn(prepare_target(&fx.root, &fx.bundled).unwrap(), FixedArgs::Synthetic, IoMode::Piped).unwrap();
    let pid = child.id();
    let big = vec![b'x'; 8 * 1024 * 1024];
    let started = std::time::Instant::now();
    let r = child.send_eof_and_finish(&big, Some(Duration::from_millis(700)));
    let elapsed = started.elapsed();
    assert!(elapsed < Duration::from_secs(5), "the limit must bound the whole finish: {elapsed:?}");
    assert_eq!(r.pid, pid);
    assert!(r.stdin_error.as_deref().is_some_and(|e| e.contains("not fully delivered") && e.contains("stdin closed")), "{r:?}");
    assert!(matches!(r.exit, Exit::NotConfirmed { .. }), "{r:?}");
    assert!(matches!(&r.stdout, Capture::Incomplete(b) if b == b"idle up\n" || b.is_empty()), "{r:?}");
    assert!(matches!(r.stderr, Capture::Incomplete(_)), "{r:?}");
    assert!(r.left_behind.iter().any(|l| l.contains(&format!("pid {pid}"))) && r.left_behind.iter().any(|l| l.contains("stdout reader")), "{r:?}");
    assert!(!r.left_behind.iter().any(|l| l.contains("stdin")), "on Unix stdin is ours and closed, nothing about it is left behind: {r:?}");
    println!("OBSERVED bounded finish (a): {:?} in {elapsed:?}; left behind: {:?}", r.exit, r.left_behind);
    // the test ends and reaps what it left running
    unsafe { libc::kill(pid as i32, libc::SIGKILL) };
    let mut status = 0;
    assert_eq!(unsafe { libc::waitpid(pid as i32, &mut status, 0) }, pid as i32);
    assert!(libc::WIFSIGNALED(status));
    // (b) the child exits but a descendant keeps the output pipe: the exit is confirmed in time,
    //     the capture is reported incomplete, the reader is listed as left behind
    let fx2 = Fixture::new("descendant");
    let pidfile = fx2.base.join("desc.pid");
    let forker = format!("#!/bin/sh\nprintf 'child start\\n'\nsleep 30 &\necho $! > '{}'\nexit 0\n", pidfile.display()).into_bytes();
    let d2 = fx2.prepare(V1, &forker);
    fx2.select(V1, &d2, 0);
    let child = spawn(prepare_target(&fx2.root, &fx2.bundled).unwrap(), FixedArgs::Synthetic, IoMode::Piped).unwrap();
    let pid = child.id();
    let started = std::time::Instant::now();
    // The limit is generous because child start-up (dyld) takes seconds on this Mac when the
    // whole suite runs in parallel; what matters is that the 30 s pipe hold by the descendant
    // does not delay the report past the limit.
    let r = child.send_eof_and_finish(b"", Some(Duration::from_secs(12)));
    let elapsed = started.elapsed();
    assert!(elapsed < Duration::from_secs(15), "{elapsed:?}");
    assert_eq!(r.pid, pid); assert_eq!(r.exit, Exit::Code(0), "the direct child's exit is confirmed: {r:?}");
    assert_eq!(r.stdout, Capture::Incomplete(b"child start\n".to_vec()), "{r:?}");
    assert!(r.left_behind.iter().any(|l| l.contains("stdout reader")) && !r.left_behind.iter().any(|l| l.contains(&format!("pid {pid}"))), "{r:?}");
    println!("OBSERVED bounded finish (b): exit {:?}, stdout {:?}, left behind: {:?}", r.exit, r.stdout, r.left_behind);
    // the test ends the descendant it created (not our child: it cannot be reaped here)
    let desc: i32 = fs::read_to_string(&pidfile).unwrap().trim().parse().unwrap();
    assert_eq!(unsafe { libc::kill(desc, libc::SIGKILL) }, 0, "descendant pid {desc} was alive");
    let gone = (0..200).any(|_| { std::thread::sleep(Duration::from_millis(10)); (unsafe { libc::kill(desc, 0) }) < 0 });
    assert!(gone, "descendant pid {desc} still signalable after SIGKILL");
}
#[cfg(unix)]
#[test]
fn a_steadily_progressing_input_is_bounded_by_the_deadline_too() {
    // the child drains 1 KiB every 10 ms forever: the write loop keeps making small progress
    // (no long EAGAIN wait), so only a per-iteration deadline check can bound it
    let fx = Fixture::new("steady");
    let slow = format!("#!/bin/sh\nprintf 'started slow\\n' >> '{}'\nwhile dd bs=1024 count=1 >/dev/null 2>&1; do sleep 0.01; done\n", fx.trace.display()).into_bytes();
    let d = fx.prepare(V1, &slow);
    fx.select(V1, &d, 0);
    let child = spawn(prepare_target(&fx.root, &fx.bundled).unwrap(), FixedArgs::Synthetic, IoMode::Piped).unwrap();
    let pid = child.id();
    let big = vec![b'y'; 64 * 1024 * 1024];
    let started = std::time::Instant::now();
    let r = child.send_eof_and_finish(&big, Some(Duration::from_millis(800)));
    let elapsed = started.elapsed();
    assert!(elapsed < Duration::from_secs(4), "bounded despite steady progress: {elapsed:?}");
    let msg = r.stdin_error.clone().expect("partial delivery must be reported");
    assert!(msg.contains("not fully delivered") && msg.contains("stdin closed"), "{r:?}");
    let delivered: usize = msg.split("limit: ").nth(1).and_then(|s| s.split(' ').next()).and_then(|n| n.parse().ok()).expect("delivered count in the message");
    assert!(delivered < big.len(), "{msg}");
    println!("OBSERVED steady input bounded: {delivered} of {} bytes in {elapsed:?}; exit {:?}", big.len(), r.exit);
    unsafe { libc::kill(pid as i32, libc::SIGKILL) };
    let mut status = 0;
    assert_eq!(unsafe { libc::waitpid(pid as i32, &mut status, 0) }, pid as i32);
}
#[cfg(unix)]
#[test]
fn the_fork_exec_errno_channel_distinguishes_eof_complete_partial_and_read_errors() {
    let fx = Fixture::new("errno-channel");
    let a = fx.core_bytes("v1", 7);
    let d = fx.prepare(V1, &a);
    fx.select(V1, &d, 0);
    // The layer closes its pipe ends before its brief (500 ms) reap poll, so the script (which
    // exits on stdin EOF) is normally reaped by the layer with its exit code. Under the parallel
    // suite a child can still be in dyld start-up after 500 ms on this Mac; then the layer
    // reports it as not reaped and the pid, and the caller (here the test) must reap it.
    let reap_if_needed = |child: &FailedChild| {
        match &child.exit {
            Some(Exit::Code(7)) => "reaped by the layer (exit 7)".to_owned(),
            Some(other) => panic!("unexpected exit for the injected child: {other:?}"),
            None => {
                let mut status = 0;
                let reaped = unsafe { libc::waitpid(child.pid as i32, &mut status, 0) };
                assert_eq!(reaped, child.pid as i32, "a not-reaped child must still be reapable by its pid");
                format!("not reaped within the layer's poll; reaped by the test (exit status {status:#x})")
            }
        }
    };
    // partial answer: never success; pid kept; exec outcome unknown
    hooks::ERRNO_CHANNEL.with(|c| c.set(Some(hooks::ErrnoInject::Partial)));
    let e = spawn(prepare_target(&fx.root, &fx.bundled).unwrap(), FixedArgs::Synthetic, IoMode::Piped).unwrap_err();
    hooks::reset();
    assert_eq!(e.stage, LaunchStage::Spawn); assert!(e.reason.contains("2 of 4 bytes") && e.reason.contains("unknown"), "{e:?}");
    assert_eq!(e.spawned, Started::Unknown);
    let child = e.child.clone().expect("pid kept"); assert!(child.pid > 0);
    println!("OBSERVED partial errno answer: pid {} {}", child.pid, reap_if_needed(&child));
    // read error: same, with the error named
    hooks::ERRNO_CHANNEL.with(|c| c.set(Some(hooks::ErrnoInject::ReadError)));
    let e = spawn(prepare_target(&fx.root, &fx.bundled).unwrap(), FixedArgs::Synthetic, IoMode::Piped).unwrap_err();
    hooks::reset();
    assert!(e.reason.contains("read failed: injected read error") && e.reason.contains("unknown"), "{e:?}");
    assert_eq!(e.spawned, Started::Unknown);
    let child = e.child.clone().expect("pid kept");
    println!("OBSERVED errno channel read error: pid {} {}", child.pid, reap_if_needed(&child));
    // EINTR then the real answer: retried, the launch succeeds normally
    hooks::ERRNO_CHANNEL.with(|c| c.set(Some(hooks::ErrnoInject::EintrThenReal)));
    let r = spawn(prepare_target(&fx.root, &fx.bundled).unwrap(), FixedArgs::Synthetic, IoMode::Piped).unwrap().send_eof_and_finish(b"x\n", Some(Duration::from_secs(30)));
    hooks::reset();
    assert_eq!(r.exit, Exit::Code(7)); assert_eq!(out(&r), "child v1 start\nx\nchild v1 end\n");
    // the clean EOF case is every successful launch above; the complete-answer case is the
    // refused-images test (exit 127 reaped and reported)
    assert_eq!(fx.trace().matches("started v1").count(), 3, "the three forked children with a valid image all ran (the injected ones after their stdin EOF): {}", fx.trace());
}
#[test]
fn arguments_are_fixed_constants_and_nothing_is_public_or_shell_assembled() {
    assert_eq!(FixedArgs::McpServe.argv(), ["mcp", "--serve"]);
    const SRC: &str = include_str!("runtime_launch.rs");
    assert!(!SRC.contains("\npub fn ") && !SRC.contains("\npub struct ") && !SRC.contains("\npub enum "), "no public entry point before the trust gate");
    // calls, not the doc comments that explain why they are avoided
    for forbidden in ["libc::execvp(", "libc::execlp(", "libc::execv(", "posix_spawnp(", "posix_spawn(", "Command::new(\"sh\"", "Command::new(\"/bin/sh\"", "\"-c\"", "Command::new(format!", "args_from", "extra_args"] {
        assert!(!SRC.contains(forbidden), "{forbidden} must not appear in runtime_launch.rs");
    }
    #[cfg(unix)]
    assert!(!SRC.contains("std::process::Command") || SRC.contains("#[cfg(windows)]"), "Unix spawns with its own fork/execve");
}

#[cfg(unix)]
fn break_protection(p: &Path) { use std::os::unix::fs::PermissionsExt; fs::set_permissions(p, fs::Permissions::from_mode(0o755)).unwrap(); }
#[cfg(unix)]
fn restore_protection(p: &Path) { use std::os::unix::fs::PermissionsExt; fs::set_permissions(p, fs::Permissions::from_mode(0o500)).unwrap(); }
#[cfg(windows)]
fn icacls(args: &[&str], target: &Path) {
    let icacls = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/icacls.exe");
    let out = std::process::Command::new(icacls).arg(target).args(args).output().unwrap();
    assert!(out.status.success(), "icacls {args:?}: {}", String::from_utf8_lossy(&out.stdout));
}
#[cfg(windows)]
fn break_protection(p: &Path) { icacls(&["/grant", "*S-1-1-0:(RX)"], p); }
#[cfg(windows)]
fn restore_protection(p: &Path) { icacls(&["/reset"], p); }
