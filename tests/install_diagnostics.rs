//! CLI output compatibility in a disposable profile; no real Host settings are used.
#![cfg(any(windows, target_os = "macos"))]
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

struct Profile(PathBuf);
impl Drop for Profile {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}

fn run(binary: &Path, args: &[&str], root: &Path) -> Output {
    let mut child = Command::new(binary).args(args).current_dir(root)
        .env("HOME", root.join("home")).env("USERPROFILE", root.join("home"))
        .env("LOCALAPPDATA", root.join("Local")).env("APPDATA", root.join("Roaming"))
        .env("CODEX_HOME", root.join("absent-codex"))
        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().unwrap();
    let started = Instant::now();
    loop {
        if child.try_wait().unwrap().is_some() { break; }
        if started.elapsed() > Duration::from_secs(15) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("installation diagnostic command timed out: {args:?}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    child.wait_with_output().unwrap()
}

#[test]
fn verbose_doctor_preserves_stdout_and_keeps_missing_hosts_distinct_from_connection() {
    let mut random = [0_u8; 8];
    getrandom::fill(&mut random).unwrap();
    let root = Profile(std::env::temp_dir().join(format!("gil-doctor-cli-{:016x}", u64::from_ne_bytes(random))));
    std::fs::create_dir_all(&root.0).unwrap();
    let installed = run(Path::new(env!("CARGO_BIN_EXE_gil")), &["install", "--no-wait"], &root.0);
    assert!(installed.status.success(), "{}", String::from_utf8_lossy(&installed.stderr));
    assert!(String::from_utf8_lossy(&installed.stdout).contains("PENDING is not completed registration"));
    let fixed = if cfg!(windows) { root.0.join("Local/Programs/gil/gil.exe") }
        else { root.0.join("home/Library/Application Support/GIL/bin/gil") };
    let plain = run(&fixed, &["doctor"], &root.0);
    let verbose = run(&fixed, &["doctor", "--verbose"], &root.0);
    assert!(plain.status.success() && verbose.status.success());
    assert_eq!(plain.stdout, verbose.stdout);
    assert!(plain.stderr.is_empty());
    let text = String::from_utf8(plain.stdout).unwrap();
    assert_eq!(text.lines().count(), 3);
    assert!(text.starts_with("binary: OK "));
    assert!(text.ends_with("claude-desktop: NOT_INSTALLED\ncodex: NOT_INSTALLED\n"));
    let detail = String::from_utf8(verbose.stderr).unwrap();
    assert!(detail.contains("does not verify MCP connection or Monitor display"));
    assert!(detail.contains("not created; historical results"));
    assert!(!root.0.join(".gil").exists());
    assert!(!fixed.parent().unwrap().join("install-log.txt").exists(), "diagnosis must not create a helper log");
    assert!(!run(&fixed, &["doctor", "--unknown"], &root.0).status.success());
}
