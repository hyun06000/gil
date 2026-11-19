//! Process-level separation of the MSIX guide and the ordinary CLI. No real user profile.
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

fn run(binary: &str, args: &[&str]) -> Output {
    let mut child = Command::new(binary).args(args)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    // Keep stdin OPEN: redirected guide/CLI calls must not wait even without EOF.
    let input = child.stdin.take().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if child.try_wait().unwrap().is_some() { break; }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let _ = child.wait();
            panic!("{binary} {args:?} waited for redirected input");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(input);
    child.wait_with_output().unwrap()
}

#[test]
fn redirected_start_guide_prints_both_languages_and_exits_without_input() {
    let output = run(env!("CARGO_BIN_EXE_gil-start"), &[]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("English - Connect and use GIL"));
    assert!(text.contains("한국어 - GIL 연결과 사용"));
    assert!(text.contains("gil install --no-wait"));
    assert!(text.contains("gil doctor"));
    assert!(!text.contains("[0] Exit"), "pipes do not present an interactive menu");
}

#[test]
fn guide_refuses_cli_and_mcp_arguments_instead_of_swallowing_them() {
    for args in [&["mcp", "--serve"][..], &["--version"], &["install"]] {
        let output = run(env!("CARGO_BIN_EXE_gil-start"), args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn ordinary_cli_still_exits_with_help_and_never_shows_the_guide() {
    let binary = env!("CARGO_BIN_EXE_gil");
    let bare = run(binary, &[]);
    let help = run(binary, &["--help"]);
    assert!(bare.status.success() && help.status.success());
    assert!(bare.stderr.is_empty() && help.stderr.is_empty());
    assert_eq!(bare.stdout, help.stdout);
    assert!(!String::from_utf8(help.stdout).unwrap().contains("[0] Exit"));
    let version = run(binary, &["--version"]);
    assert!(version.status.success());
    assert_eq!(String::from_utf8(version.stdout).unwrap(), format!("gil {}\n", gil::CORE_VERSION));
}
