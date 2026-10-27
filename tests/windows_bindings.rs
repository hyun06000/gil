//! Run the real binding module's tests without compiling unrelated Unix loopback unit tests.
#![cfg(windows)]
#![allow(dead_code)]
mod monitor {
    #[derive(Debug)]
    pub struct Refusal { pub code: &'static str, pub said: &'static str }
    impl Refusal {
        pub fn new(code: &'static str, said: &'static str) -> Self { Self { code, said } }
    }
}
// This harness exercises the binding store in isolation. Its fixtures use the historical inline
// record layout; product integration tests cover the external Windows record locator.
mod project_state {
    use std::path::{Path, PathBuf};
    pub(crate) fn state_for_root(root: &Path) -> Result<PathBuf, String> {
        Ok(root.join(".gil/state.yaml"))
    }
}
#[path = "../src/mcp/bindings.rs"]
mod bindings;
