//! Compatibility entry point; installed Core also provides `gil write-guard`.
fn main() -> std::process::ExitCode {
    gil::write_guard::cli(&std::env::args().skip(1).collect::<Vec<_>>())
}
