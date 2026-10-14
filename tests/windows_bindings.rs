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
#[path = "../src/mcp/bindings.rs"]
mod bindings;
