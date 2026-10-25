//! `gil-runtime-testbed` — exists only with the `runtime-testbed` feature (never a product
//! binary). `launcher <store-root> <bundled-core> [evidence-dir]` runs the crate's launch layer with stdio
//! inherited; `core <version> [ignored args...]` serves the synthetic MCP Core; an executable
//! copy carrying an overlay version marker serves the synthetic Core whatever its arguments
//! are (that is how the launcher starts it on Windows: `gil.exe mcp --serve`).
use std::path::PathBuf;
use std::process::ExitCode;
#[path = "../companion_review.rs"]
mod companion_review;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let code = match args.get(1).map(String::as_str) {
        Some("companion-review") if args.len() == 2 => companion_review::run(),
        #[cfg(any(target_os = "macos", windows))]
        Some(gil::companion_ipc::SESSION_ARG) if args.len() == 3 => gil::runtime_testbed::companion_fixture(&PathBuf::from(&args[2])),
        Some("launcher") if args.len() == 4 => gil::runtime_testbed::launcher_main(&PathBuf::from(&args[2]), &PathBuf::from(&args[3])),
        Some("launcher") if args.len() == 5 => gil::runtime_testbed::launcher_with_evidence(&PathBuf::from(&args[2]), &PathBuf::from(&args[3]), &PathBuf::from(&args[4])),
        Some("core") if args.len() >= 3 => gil::runtime_testbed::synthetic_core_main(&args[2], &args[3..]),
        _ => match gil::runtime_testbed::version_from_own_exe() {
            Some(version) => gil::runtime_testbed::synthetic_core_main(&version, &args[1..]),
            None => { eprintln!("usage: gil-runtime-testbed launcher <store-root> <bundled-core> [evidence-dir] | core <version> [args...]  (or an executable copy with an overlay version marker)"); 2 }
        },
    };
    ExitCode::from(code.clamp(0, 255) as u8)
}
