//! The regular Linux acceptance run exercises the shipped CLI in an isolated container.
//! Other platforms and offline developer test runs do not start a container or pull an image.
#![cfg(target_os = "linux")]

#[test]
fn autonomous_benchmark_and_default_monitor_are_separate_in_the_same_container_image() {
    if std::env::var_os("GITHUB_ACTIONS").is_none()
        && std::env::var_os("GIL_TEST_BENCHMARK_CONTAINER").as_deref()
            != Some(std::ffi::OsStr::new("1"))
    {
        return;
    }
    let result = std::process::Command::new("bash")
        .args([
            "tools/benchmark/test-container.sh",
            env!("CARGO_BIN_EXE_gil"),
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .status()
        .expect("run container acceptance");
    assert!(
        result.success(),
        "container benchmark acceptance failed: {result}"
    );
}
