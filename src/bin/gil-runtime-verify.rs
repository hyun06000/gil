//! `gil-runtime-verify` — exists only with the `runtime-testbed` feature (never a product
//! binary). Runs the crate's App Server integration test from a prebuilt bundle inside one
//! folder: `--root <folder>` (required, used once), `--cli-source pinned` (default: the pinned
//! Codex CLI downloaded from its official npm registry URL into the folder and integrity-checked)
//! or `--cli <codex executable>` (an existing CLI, recorded as found). Exit 0 PASS, 1 FAIL,
//! 2 BLOCKED; the raw test output and a summary are printed in the same run.
use gil::runtime_verify::{self as verify, CliSource, Options};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage: gil-runtime-verify --root <folder> [--cli <codex executable> | --cli-source pinned] [--test-exe <path>] [--testbed <path>]";

fn parse(args: &[String]) -> Result<Options, String> {
    let (mut root, mut cli, mut test_exe, mut testbed, mut source) = (None, None, None, None, None);
    let mut i = 0;
    while i < args.len() {
        let value = |i: usize| args.get(i + 1).cloned().ok_or_else(|| format!("{} needs a value", args[i]));
        match args[i].as_str() {
            "--root" => { root = Some(PathBuf::from(value(i)?)); i += 2; }
            "--cli" => { cli = Some(PathBuf::from(value(i)?)); i += 2; }
            "--cli-source" => { source = Some(value(i)?); i += 2; }
            "--test-exe" => { test_exe = Some(PathBuf::from(value(i)?)); i += 2; }
            "--testbed" => { testbed = Some(PathBuf::from(value(i)?)); i += 2; }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let root = root.ok_or("--root is required")?;
    let cli = match (cli, source.as_deref()) {
        (Some(p), None) | (Some(p), Some("existing")) => CliSource::Existing(p),
        (None, None) | (None, Some("pinned")) => CliSource::Pinned,
        (None, Some("existing")) => return Err("--cli-source existing needs --cli <codex executable>".into()),
        (Some(_), Some("pinned")) => return Err("--cli and --cli-source pinned contradict each other".into()),
        (_, Some(other)) => return Err(format!("unknown --cli-source {other}")),
    };
    Ok(Options { root, cli, test_exe, testbed })
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") { println!("{USAGE}"); return ExitCode::from(0); }
    let opts = match parse(&args) { Ok(o) => o, Err(e) => { eprintln!("{e}\n{USAGE}"); return ExitCode::from(2); } };
    let (outcome, report) = verify::run(&opts);
    ExitCode::from(verify::finish(&opts.root, outcome, report))
}
