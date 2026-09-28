//! CLI I/O boundary. Command semantics and receipts live in the library.
use std::process::ExitCode;
fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match gil::command::cli(&args) {
        Ok(text) => { print!("{text}"); ExitCode::SUCCESS }
        Err(message) => { eprintln!("{message}"); ExitCode::FAILURE }
    }
}
