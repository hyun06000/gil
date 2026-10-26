//! Isolated verification driver behind the `runtime-testbed` feature: runs the crate's own App
//! Server integration test (`runtime_integration`, shipped as a prebuilt test executable) on a
//! user's PC inside **one** folder the caller names, with a Codex CLI that is either the pinned
//! version downloaded from its official npm registry URL into that folder (integrity-checked
//! against the value recorded here — a download check, never a publisher-signature check) or an
//! existing executable the caller names (recorded with its version and marked when it differs
//! from the CI pin). Existing GIL state is not modified. Test files are created/changed and
//! temporary fixtures cleaned inside the folder; nothing is installed, registered or retried.
//! The folder is used once (`work/` must not exist).
//!
//! The outcome is `PASS` / `FAIL` / `BLOCKED` with the raw test output streamed to the console
//! as it happens and saved under `work/evidence/`, so a later turn that has lost the folder
//! still has the report in the conversation. This is a separate App Server test with synthetic
//! Cores: a PASS is not a Codex Desktop daemon observation and not a GIL installation update.
//! Not part of any product binary, manifest or command.
use serde_json::{Value, json};
use sha2::{Digest, Sha256, Sha512};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

/// The Codex CLI version the Windows CI integration ran with (npm `@openai/codex`).
pub const PIN_VERSION: &str = "0.163.0-alpha.11.1";
pub const REGISTRY_BASE: &str = "https://registry.npmjs.org/@openai/codex/-/";
pub const TEST_EXE: &str = if cfg!(windows) { "runtime_integration.exe" } else { "runtime_integration" };
pub const TESTBED_EXE: &str = if cfg!(windows) { "gil-runtime-testbed.exe" } else { "gil-runtime-testbed" };
pub const BUNDLE_MANIFEST: &str = "bundle.json";
pub const LIMITS: [&str; 6] = [
    "separate App Server test over stdio with synthetic Cores in an isolated CODEX_HOME: not an observation of the Codex Desktop daemon and not a GIL installation update",
    "npm tarball integrity equals the value recorded from the registry on 2026-10-24: a download-integrity check, not a verification of the publisher's signature on the CLI",
    "an existing CLI is used as found and only recorded; its behaviour may differ from the pinned version the CI observed",
    "test files are created/changed and temporary fixtures cleaned inside the folder; existing GIL, plugin cache, settings and projects are not modified; downloads and evidence remain",
    "a FAIL or BLOCKED result is final for this run: the driver never retries and asks for no setting, permission or policy change",
    "pid, instance id and fixed arguments come from the synthetic Core's own answers; liveness after the app-server's end is observed, never forced",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pinned { pub npm_platform: &'static str, pub triple: &'static str, pub integrity: &'static str, pub exe_name: &'static str, pub pe_machine: u16 }
impl Pinned {
    pub fn tarball_name(&self) -> String { format!("codex-{PIN_VERSION}-{}.tgz", self.npm_platform) }
    pub fn tarball_url(&self) -> String { format!("{REGISTRY_BASE}{}", self.tarball_name()) }
}
/// The pinned platform package for an OS/CPU, as recorded from the npm registry (version,
/// alias target and `dist.integrity`). Defined for the two Windows CPUs the CI observed only.
pub fn pinned(os: &str, arch: &str) -> Option<Pinned> {
    match (os, arch) {
        ("windows", "x86_64") => Some(Pinned { npm_platform: "win32-x64", triple: "x86_64-pc-windows-msvc", integrity: "sha512-58wBlhzI4n1eLtmvMb1mgwH1T2SaYyCrLD6HDwB0l8WI7wn4Cb17l870POeTSTLSMuDKZZbu60TpTGuM2qYHtA==", exe_name: "codex.exe", pe_machine: 0x8664 }),
        ("windows", "aarch64") => Some(Pinned { npm_platform: "win32-arm64", triple: "aarch64-pc-windows-msvc", integrity: "sha512-U05HBXdAPTP5gHgnkyIaPT8mp5Xuf2Q6c1NVhxUK4GfOgj1TQ65Vlkj1hHNcZwZg7FLznnsCYvTBjIuKaKzPqQ==", exe_name: "codex.exe", pe_machine: 0xAA64 }),
        _ => None,
    }
}

/// `sha512-<base64>` → the 64 digest bytes.
pub fn decode_integrity(s: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;
    let b64 = s.strip_prefix("sha512-").ok_or_else(|| format!("integrity is not sha512-: {s}"))?;
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).map_err(|e| format!("integrity base64: {e}"))?;
    if bytes.len() != 64 { return Err(format!("integrity has {} bytes, not 64", bytes.len())); }
    Ok(bytes)
}
/// SHA-512 and SHA-256 of a file, streamed, with its size.
pub fn digests(path: &Path) -> Result<(Vec<u8>, String, u64), String> {
    let mut f = fs::File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let (mut h512, mut h256, mut n) = (Sha512::new(), Sha256::new(), 0u64);
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let k = std::io::Read::read(&mut f, &mut buf).map_err(|e| format!("read {}: {e}", path.display()))?;
        if k == 0 { break; }
        h512.update(&buf[..k]); h256.update(&buf[..k]); n += k as u64;
    }
    Ok((h512.finalize().to_vec(), format!("{:x}", h256.finalize()), n))
}
pub fn sha256_file(path: &Path) -> Result<(String, u64), String> { digests(path).map(|(_, s, n)| (s, n)) }

/// The `Machine` field of a PE image (0x8664 x64, 0xAA64 ARM64).
pub fn pe_machine(path: &Path) -> Result<u16, String> {
    let bytes = fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    pe_machine_of(&bytes)
}
pub fn pe_machine_of(bytes: &[u8]) -> Result<u16, String> {
    if bytes.len() < 0x40 || &bytes[..2] != b"MZ" { return Err("not an MZ image".into()); }
    let pe = u32::from_le_bytes(bytes[0x3C..0x40].try_into().unwrap()) as usize;
    if pe + 6 > bytes.len() || &bytes[pe..pe + 4] != b"PE\0\0" { return Err("no PE signature".into()); }
    Ok(u16::from_le_bytes([bytes[pe + 4], bytes[pe + 5]]))
}

/// The one archive member that is the CLI executable of `triple` in an npm platform tarball
/// listing (`package/vendor/<triple>/bin/<exe>`, or the two older layouts); anything else is a
/// layout the CI did not observe and is refused rather than searched.
pub fn select_member(listing: &str, triple: &str, exe_name: &str) -> Result<String, String> {
    let accepted = [format!("package/vendor/{triple}/bin/{exe_name}"), format!("package/vendor/{triple}/codex/{exe_name}"), format!("package/vendor/{triple}/{exe_name}")];
    let found: Vec<&str> = listing.lines().map(|l| l.trim_end_matches('\r')).filter(|l| accepted.iter().any(|a| a == l)).collect();
    match found.as_slice() {
        [one] => Ok((*one).to_owned()),
        [] => Err(format!("no member named {} in the tarball listing ({} entries)", accepted[0], listing.lines().count())),
        many => Err(format!("{} candidate members, expected one: {many:?}", many.len())),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome { Pass, Fail, Blocked }
impl Outcome {
    pub fn word(self) -> &'static str { match self { Outcome::Pass => "PASS", Outcome::Fail => "FAIL", Outcome::Blocked => "BLOCKED" } }
    pub fn exit_code(self) -> u8 { match self { Outcome::Pass => 0, Outcome::Fail => 1, Outcome::Blocked => 2 } }
}
/// PASS only when the test process exited 0 **and** exactly one PASS evidence file exists; a
/// non-zero exit is BLOCKED when the harness itself said so (e.g. bootstrap/CLI), otherwise FAIL.
pub fn decide(exit_ok: bool, stdout: &str, pass_files: usize) -> (Outcome, String) {
    let blocked = stdout.lines().any(|l| l.trim_start().starts_with("OUTCOME: BLOCKED"));
    match (exit_ok, pass_files, blocked) {
        (true, 1, false) => (Outcome::Pass, "test process exited 0 and one PASS evidence file was written".into()),
        (true, n, false) => (Outcome::Fail, format!("test process exited 0 but {n} PASS evidence files were written (expected one)")),
        (_, _, true) => (Outcome::Blocked, "the harness reported BLOCKED (the App Server integration did not run)".into()),
        (false, _, false) => (Outcome::Fail, "test process exited non-zero".into()),
    }
}

/// The CPU this process really runs on (Windows: `IsWow64Process2` native machine), if the OS
/// can say; `None` where the question does not arise.
pub fn native_machine() -> Result<Option<u16>, String> {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        unsafe extern "system" { fn GetCurrentProcess() -> isize; fn IsWow64Process2(h: isize, process: *mut u16, native: *mut u16) -> i32; }
        let (mut process, mut native) = (0u16, 0u16);
        let ok = unsafe { IsWow64Process2(GetCurrentProcess(), &mut process, &mut native) };
        if ok == 0 { return Err(format!("IsWow64Process2 failed: {}", std::io::Error::last_os_error())); }
        Ok(Some(native))
    }
    #[cfg(not(windows))]
    { Ok(None) }
}
pub fn own_machine() -> u16 { if cfg!(target_arch = "x86_64") { 0x8664 } else if cfg!(target_arch = "aarch64") { 0xAA64 } else { 0 } }

#[derive(Debug, Clone)]
pub enum CliSource { Pinned, Existing(PathBuf) }
#[derive(Debug, Clone)]
pub struct Options { pub root: PathBuf, pub cli: CliSource, pub test_exe: Option<PathBuf>, pub testbed: Option<PathBuf> }

struct Blocked(String);
fn blocked<T>(why: impl Into<String>) -> Result<T, Blocked> { Err(Blocked(why.into())) }

fn system_tool(name: &str) -> PathBuf {
    #[cfg(windows)]
    { PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_default()).join("System32").join(format!("{name}.exe")) }
    #[cfg(not(windows))]
    { PathBuf::from("/usr/bin").join(name) }
}
fn step(report: &mut Value, name: &str, detail: impl Into<String>) {
    let detail = detail.into();
    println!("VERIFY {name}: {detail}");
    report["steps"].as_array_mut().unwrap().push(json!({"step": name, "detail": detail}));
}

/// Run the whole verification; the report is complete whatever the outcome.
pub fn run(opts: &Options) -> (Outcome, Value) {
    let started = Instant::now();
    let mut report = json!({"schema": "gil-runtime-verify/1", "outcome": "RUNNING", "reason": null, "started_unix": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
        "root": opts.root.display().to_string(), "steps": [], "bundle": null, "cli": null, "test": null, "observed": [], "evidence": [], "limits": LIMITS});
    let outcome = match drive(opts, &mut report) {
        Ok((o, why)) => { report["reason"] = json!(why); o }
        Err(Blocked(why)) => { report["reason"] = json!(why); Outcome::Blocked }
    };
    report["outcome"] = json!(outcome.word());
    report["elapsed_seconds"] = json!(started.elapsed().as_secs());
    (outcome, report)
}

fn drive(opts: &Options, report: &mut Value) -> Result<(Outcome, String), Blocked> {
    // 1. one folder, used once
    // Resolve before constructing child paths: the integration process changes cwd to work/.
    let root = fs::canonicalize(&opts.root).map_err(|e| Blocked(format!("root does not exist or cannot be resolved: {}: {e}", opts.root.display())))?;
    report["requested_root"] = json!(opts.root.display().to_string());
    report["root"] = json!(root.display().to_string());
    if !root.is_dir() { return blocked(format!("root folder does not exist or is not a directory: {}", root.display())); }
    let work = root.join("work");
    match fs::create_dir(&work) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return blocked(format!("{} already exists: this folder was used before; make a new folder instead of retrying here", work.display())),
        Err(e) => return blocked(format!("cannot create {}: {e} (no other location is tried)", work.display())),
    }
    report["work_created"] = json!(true); // only then may the report be written inside the folder
    let evidence = work.join("evidence");
    for d in ["evidence", "tmp", "cli"] { fs::create_dir(work.join(d)).map_err(|e| Blocked(format!("cannot create {}: {e}", work.join(d).display())))?; }
    step(report, "folder", format!("created {} (evidence, tmp, cli); nothing outside {} is written", work.display(), root.display()));

    // 2. this bundle: files next to the driver, hashed against its manifest
    let me = std::env::current_exe().and_then(fs::canonicalize).map_err(|e| Blocked(format!("own executable path: {e}")))?;
    let here = me.parent().ok_or_else(|| Blocked("own executable has no parent folder".into()))?.to_path_buf();
    let test_exe = fs::canonicalize(opts.test_exe.clone().unwrap_or_else(|| here.join(TEST_EXE)))
        .map_err(|e| Blocked(format!("bundle manifest executable path (integration): {e}")))?;
    let testbed = fs::canonicalize(opts.testbed.clone().unwrap_or_else(|| here.join(TESTBED_EXE)))
        .map_err(|e| Blocked(format!("bundle manifest executable path (testbed): {e}")))?;
    let manifest_path = here.join(BUNDLE_MANIFEST);
    let manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).map_err(|e| Blocked(format!("bundle manifest {}: {e}", manifest_path.display())))?).map_err(|e| Blocked(format!("bundle manifest is not JSON: {e}")))?;
    let mut files = Vec::new();
    for f in manifest["files"].as_array().ok_or_else(|| Blocked("bundle manifest has no files".into()))? {
        let (rel, want, want_bytes) = (f["path"].as_str().unwrap_or(""), f["sha256"].as_str().unwrap_or(""), f["bytes"].as_u64().unwrap_or(0));
        if rel.is_empty() || rel.contains("..") || rel.contains('/') || rel.contains('\\') { return blocked(format!("bundle manifest names an unexpected path: {rel:?}")); }
        let p = here.join(rel);
        let (got, bytes) = sha256_file(&p).map_err(Blocked)?;
        if got != want || bytes != want_bytes { return blocked(format!("bundle file {rel} differs from the manifest (sha256 {got} / {bytes} bytes vs {want} / {want_bytes}); the bundle is not the one that was built")); }
        files.push(json!({"path": rel, "bytes": bytes, "sha256": got}));
    }
    let listed = |p: &Path| manifest["files"].as_array().unwrap().iter().any(|f| f["path"].as_str() == p.file_name().and_then(|n| n.to_str()));
    for p in [&me, &test_exe, &testbed] { if !listed(p) { return blocked(format!("{} is not listed in the bundle manifest", p.display())); } }
    let own = own_machine();
    if cfg!(windows) {
        for p in [&me, &test_exe, &testbed] {
            let m = pe_machine(p).map_err(Blocked)?;
            if m != own { return blocked(format!("{} is built for machine {m:#06x}, this driver for {own:#06x}", p.display())); }
        }
        if let Some(native) = native_machine().map_err(Blocked)? {
            if native != own { return blocked(format!("this PC's native CPU is machine {native:#06x} but this bundle is built for {own:#06x}: use the bundle for the native CPU (not downloaded or substituted here)")); }
        }
    }
    report["bundle"] = json!({"source": manifest["source"], "arch": manifest["arch"], "folder": here.display().to_string(), "files": files, "machine": format!("{own:#06x}"), "native_machine": native_machine().ok().flatten().map(|m| format!("{m:#06x}"))});
    step(report, "bundle", format!("{} files match {} (source {}); machine {own:#06x}", report["bundle"]["files"].as_array().unwrap().len(), BUNDLE_MANIFEST, manifest["source"]));

    // 3. the Codex CLI
    let cli_path = match &opts.cli {
        CliSource::Existing(p) => {
            let p = fs::canonicalize(p).map_err(|e| Blocked(format!("existing CLI {}: {e}", p.display())))?;
            if !p.is_file() { return blocked(format!("existing CLI is not a file: {}", p.display())); }
            if cfg!(windows) && p.extension().and_then(|e| e.to_str()).map(|e| !e.eq_ignore_ascii_case("exe")).unwrap_or(true) { return blocked(format!("existing CLI must be the codex.exe executable itself, not a wrapper: {}", p.display())); }
            let (sha, bytes) = sha256_file(&p).map_err(Blocked)?;
            let version = cli_version(&p)?;
            let matches = version == format!("codex-cli {PIN_VERSION}");
            report["cli"] = json!({"source": "existing", "path": p.display().to_string(), "bytes": bytes, "sha256": sha, "version_output": version, "ci_pin": format!("codex-cli {PIN_VERSION}"), "matches_ci_pin": matches});
            step(report, "cli", format!("existing {} -> '{version}' ({})", p.display(), if matches { "same version as the CI pin" } else { "DIFFERS from the CI pin; recorded, not substituted" }));
            p
        }
        CliSource::Pinned => {
            let pin = pinned(std::env::consts::OS, std::env::consts::ARCH).ok_or_else(|| Blocked(format!("no pinned CLI source is recorded for {}/{}: pass --cli <codex executable>", std::env::consts::OS, std::env::consts::ARCH)))?;
            let (curl, tar) = (system_tool("curl"), system_tool("tar"));
            for t in [&curl, &tar] { if !t.is_file() { return blocked(format!("system tool missing: {}", t.display())); } }
            let dest = work.join("cli");
            let tgz = dest.join(pin.tarball_name());
            let url = pin.tarball_url();
            step(report, "cli", format!("downloading the pinned platform package from {url} (official npm registry) into {}", tgz.display()));
            let out = download_command(&curl, &dest, &pin).output().map_err(|e| Blocked(format!("run curl: {e}")))?;
            if !out.status.success() { return blocked(format!("download failed ({}): {}; not retried", out.status, String::from_utf8_lossy(&out.stderr).trim())); }
            let (sha512, sha256, bytes) = digests(&tgz).map_err(Blocked)?;
            let want = decode_integrity(pin.integrity).map_err(Blocked)?;
            if sha512 != want { return blocked(format!("tarball integrity differs from the recorded value (got sha512 {}, {bytes} bytes); nothing extracted or run", hex(&sha512))); }
            step(report, "cli", format!("tarball {bytes} bytes, sha512 matches the recorded integrity (download check, not a signature), sha256 {sha256}"));
            let list = archive_command(&tar, &dest, &pin, None).output().map_err(|e| Blocked(format!("run tar -t: {e}")))?;
            if !list.status.success() { return blocked(format!("tar listing failed: {}", String::from_utf8_lossy(&list.stderr).trim())); }
            let member = select_member(&String::from_utf8_lossy(&list.stdout), pin.triple, pin.exe_name).map_err(Blocked)?;
            let ex = archive_command(&tar, &dest, &pin, Some(&member)).output().map_err(|e| Blocked(format!("run tar -x: {e}")))?;
            if !ex.status.success() { return blocked(format!("tar extraction failed: {}", String::from_utf8_lossy(&ex.stderr).trim())); }
            let exe = dest.join(member.replace('/', std::path::MAIN_SEPARATOR_STR));
            if !exe.is_file() { return blocked(format!("extracted member is missing: {}", exe.display())); }
            if cfg!(windows) {
                let m = pe_machine(&exe).map_err(Blocked)?;
                if m != pin.pe_machine { return blocked(format!("extracted CLI is built for machine {m:#06x}, expected {:#06x}", pin.pe_machine)); }
            }
            #[cfg(unix)]
            { use std::os::unix::fs::PermissionsExt; let _ = fs::set_permissions(&exe, fs::Permissions::from_mode(0o500)); }
            let (sha, exe_bytes) = sha256_file(&exe).map_err(Blocked)?;
            let version = cli_version(&exe)?;
            if version != format!("codex-cli {PIN_VERSION}") { return blocked(format!("the pinned CLI reports '{version}', expected 'codex-cli {PIN_VERSION}'")); }
            report["cli"] = json!({"source": "pinned", "npm_package": format!("@openai/codex@{PIN_VERSION} ({})", pin.npm_platform), "url": url, "tarball_bytes": bytes, "tarball_sha256": sha256, "tarball_integrity": pin.integrity, "member": member, "path": exe.display().to_string(), "bytes": exe_bytes, "sha256": sha, "pe_machine": format!("{:#06x}", pin.pe_machine), "version_output": version, "matches_ci_pin": true, "integrity_is_not_signature": true});
            step(report, "cli", format!("{} -> '{version}' ({exe_bytes} bytes, sha256 {sha})", exe.display()));
            exe
        }
    };

    // 4. the integration test, streamed and saved
    let tmp = work.join("tmp");
    let mut cmd = integration_command(&test_exe, &work, &cli_path, &testbed);
    step(report, "test", format!("running {} --nocapture with GIL_RUNTIME_INTEGRATION_REQUIRE=1, evidence in {}, temp in {}", test_exe.display(), evidence.display(), tmp.display()));
    let mut child = cmd.spawn().map_err(|e| Blocked(format!("cannot start the test executable {}: {e}", test_exe.display())))?;
    let (out, err) = (child.stdout.take().unwrap(), child.stderr.take().unwrap());
    let tee = |reader: Box<dyn std::io::Read + Send>, path: PathBuf, to_err: bool| std::thread::spawn(move || {
        let mut errors = Vec::new();
        let mut file = match fs::File::create(&path) { Ok(f) => Some(f), Err(e) => { errors.push(format!("create {}: {e}", path.display())); None } };
        let mut lines = Vec::new();
        for line in BufReader::new(reader).lines() {
            let l = match line { Ok(l) => l, Err(e) => { errors.push(format!("read {}: {e}", path.display())); break; } };
            if to_err { eprintln!("{l}"); } else { println!("{l}"); }
            if let Some(f) = file.as_mut() { if let Err(e) = writeln!(f, "{l}") { errors.push(format!("write {}: {e}", path.display())); file = None; } }
            lines.push(l);
        }
        if let Some(mut f) = file { if let Err(e) = f.flush() { errors.push(format!("flush {}: {e}", path.display())); } }
        (lines, errors)
    });
    let t_out = tee(Box::new(out), evidence.join("runtime_integration.stdout.log"), false);
    let t_err = tee(Box::new(err), evidence.join("runtime_integration.stderr.log"), true);
    let status = child.wait().map_err(|e| Blocked(format!("waiting for the test executable: {e}")))?;
    let (stdout_lines, mut capture_errors) = t_out.join().unwrap_or_else(|_| (vec![], vec!["stdout reader panicked".into()]));
    let (stderr_lines, errors) = t_err.join().unwrap_or_else(|_| (vec![], vec!["stderr reader panicked".into()]));
    capture_errors.extend(errors);
    let (outcome, why) = collect_test_report(report, &evidence, status.success(), status.code(), &stdout_lines, &stderr_lines, capture_errors);
    report["test"]["executable"] = json!(test_exe.display().to_string());
    step(report, "test", format!("exit {:?}; {why}", status.code()));
    Ok((outcome, why))
}

/// Collect every case, not just PASS files. Only the explicitly tagged integration flow can
/// determine the main outcome; deliberate negative regressions remain separately inspectable.
fn collect_test_report(report: &mut Value, dir: &Path, exit_ok: bool, exit: Option<i32>, stdout: &[String], stderr: &[String], mut errors: Vec<String>) -> (Outcome, String) {
    let mut files = Vec::new();
    match fs::read_dir(dir) {
        Err(e) => errors.push(format!("list evidence {}: {e}", dir.display())),
        Ok(entries) => for entry in entries {
            match entry {
                Err(e) => errors.push(format!("list evidence entry: {e}")),
                Ok(entry) => {
                    let p = entry.path();
                    if p.file_name().and_then(|n| n.to_str()).is_some_and(|n| (n.starts_with("run-") || n.starts_with("bootstrap-")) && n.ends_with(".json")) { files.push(p); }
                }
            }
        }
    }
    files.sort();
    let mut records = Vec::new();
    for path in &files {
        match fs::read(path).map_err(|e| e.to_string()).and_then(|b| serde_json::from_slice::<Value>(&b).map_err(|e| e.to_string())) {
            Ok(v) => {
                if !matches!(v["kind"].as_str(), Some("integration" | "harness-regression")) || !matches!(v["outcome"].as_str(), Some("PASS" | "FAIL" | "BLOCKED")) {
                    errors.push(format!("invalid evidence identity/outcome: {}", path.display()));
                }
                records.push(json!({"path": path.display().to_string(), "record": v}));
            }
            Err(e) => errors.push(format!("read evidence {}: {e}", path.display())),
        }
    }
    let main: Vec<_> = records.iter().filter(|v| v["record"]["kind"] == "integration" && v["record"]["case"] == "flow").collect();
    let pass_files: Vec<_> = main.iter().filter(|v| v["record"]["outcome"] == "PASS").map(|v| v["path"].clone()).collect();
    let (mut outcome, mut why) = decide(exit_ok, &stdout.join("\n"), pass_files.len());
    if main.len() != 1 { errors.push(format!("expected one integration flow record, found {}", main.len())); }
    if let [one] = main.as_slice() {
        let v = &one["record"];
        for key in ["observed", "terminations", "verdict_after", "error", "protection_evidence", "protection_evidence_limits", "connection_evidence", "root_at_report", "launcher_evidence"] { report[key] = v[key].clone(); }
        report["main_evidence"] = one["path"].clone();
        match v["outcome"].as_str() {
            Some("BLOCKED") => { outcome = Outcome::Blocked; why = v["error"].as_str().unwrap_or("integration blocked").into(); }
            Some("FAIL") => { outcome = Outcome::Fail; why = v["error"].as_str().unwrap_or("integration failed").into(); }
            _ => {}
        }
    }
    if !errors.is_empty() { outcome = Outcome::Fail; why = format!("evidence collection incomplete: {}", errors.join("; ")); }
    // Keep stderr (including multi-line panics and prepare diagnostics) even when no record
    // could be saved. Raw streams are separate: this does not imply cross-stream ordering.
    report["test"] = json!({"exit":exit, "stdout_lines":stdout.len(), "stderr_lines":stderr.len(), "stdout":stdout, "stderr":stderr, "key_lines":stdout, "pass_evidence_files":pass_files, "evidence_records":records, "evidence_errors":errors});
    report["evidence"] = json!([dir.join("runtime_integration.stdout.log"), dir.join("runtime_integration.stderr.log"), dir.join("verify-report.json")].into_iter().chain(files).map(|p| p.display().to_string()).collect::<Vec<_>>());
    (outcome, why)
}

// Windows tools may use narrow argv for filesystem paths. Keep all path-bearing arguments
// ASCII and pass the absolute Unicode destination through CreateProcessW's cwd instead.
fn download_command(curl: &Path, dest: &Path, pin: &Pinned) -> Command {
    let mut cmd = Command::new(curl);
    cmd.current_dir(dest).args(["--fail", "--silent", "--show-error", "--location", "--output"])
        .arg(pin.tarball_name()).arg(pin.tarball_url());
    cmd
}
fn archive_command(tar: &Path, dest: &Path, pin: &Pinned, member: Option<&str>) -> Command {
    let mut cmd = Command::new(tar);
    cmd.current_dir(dest).arg(if member.is_some() { "-xzf" } else { "-tzf" }).arg(pin.tarball_name());
    if let Some(member) = member { cmd.arg(member); }
    cmd
}

fn integration_command(test_exe: &Path, work: &Path, cli: &Path, testbed: &Path) -> Command {
    let mut cmd = Command::new(test_exe);
    let tmp = work.join("tmp");
    cmd.arg("--nocapture").current_dir(work).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .env("GIL_CODEX_CLI", cli).env("GIL_RUNTIME_INTEGRATION_REQUIRE", "1")
        .env("GIL_RUNTIME_INTEGRATION_EVIDENCE_DIR", work.join("evidence")).env("GIL_RUNTIME_TESTBED", testbed)
        .env("TEMP", &tmp).env("TMP", &tmp).env("TMPDIR", &tmp).env_remove("GIL_RUNTIME_INTEGRATION_INJECT_FAIL");
    cmd
}

fn cli_version(exe: &Path) -> Result<String, Blocked> {
    let out = Command::new(exe).arg("--version").stdin(Stdio::null()).output().map_err(|e| Blocked(format!("run {} --version: {e}", exe.display())))?;
    if !out.status.success() { return blocked(format!("{} --version exited {}: {}", exe.display(), out.status, String::from_utf8_lossy(&out.stderr).trim())); }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}
fn hex(b: &[u8]) -> String { b.iter().map(|x| format!("{x:02x}")).collect() }

/// Save the report inside the folder (best effort, reported) and print the summary block that
/// must reach the conversation even if the folder disappears later.
pub fn finish(root: &Path, outcome: Outcome, mut report: Value) -> u8 {
    let resolved = report["root"].as_str().map(PathBuf::from).unwrap_or_else(|| root.to_path_buf());
    let path = resolved.join("work").join("evidence").join("verify-report.json");
    if report["work_created"] != json!(true) {
        // A folder this run did not create (used before, absent) keeps whatever it holds.
        report["report_saved"] = json!(null);
    } else {
        match serde_json::to_vec_pretty(&report).map_err(|e| e.to_string()).and_then(|b| fs::write(&path, b).map_err(|e| e.to_string())) {
            Ok(()) => report["report_saved"] = json!(path.display().to_string()),
            Err(e) => { eprintln!("REPORT NOT WRITTEN: {e}"); report["report_saved"] = json!(null); }
        }
    }
    println!("\n===== GIL RUNTIME VERIFY SUMMARY =====");
    println!("OUTCOME: {} — {}", outcome.word(), report["reason"].as_str().unwrap_or(""));
    println!("root: {}", report["root"].as_str().unwrap_or(""));
    if let Some(b) = report["bundle"].as_object() { println!("bundle: source {} arch {} ({} files verified against {})", b["source"], b["arch"], b["files"].as_array().map(|a| a.len()).unwrap_or(0), BUNDLE_MANIFEST); }
    if let Some(c) = report["cli"].as_object() { println!("cli: {} {} version '{}' matches_ci_pin={} sha256 {}", c["source"], c["path"], c["version_output"].as_str().unwrap_or(""), c["matches_ci_pin"], c["sha256"].as_str().unwrap_or("")); }
    if let Some(t) = report["test"].as_object() { println!("test: exit {} ; PASS evidence files {}", t["exit"], t["pass_evidence_files"].as_array().map(|a| a.len()).unwrap_or(0)); }
    if let Some(error) = report["error"].as_str() { println!("integration error: {error}"); }
    for o in report["observed"].as_array().into_iter().flatten() { println!("observed: {}", o.as_str().unwrap_or("")); }
    for t in report["terminations"].as_array().into_iter().flatten() { println!("termination: {} pid {} {} — {}", t["kind"].as_str().unwrap_or(""), t["pid"], t["how"].as_str().unwrap_or(""), t["detail"].as_str().unwrap_or("")); }
    println!("limits:"); for l in LIMITS { println!("  - {l}"); }
    println!("report: {}", report["report_saved"]);
    println!("===== END SUMMARY (exit {}) =====", outcome.exit_code());
    println!("REPORT-JSON-BEGIN\n{}\nREPORT-JSON-END", serde_json::to_string(&report).unwrap_or_default());
    outcome.exit_code()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_flow_keeps_error_observations_stderr_and_separates_negative_regressions() {
        let dir = std::env::temp_dir().join(format!("gil-evidence-report-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&dir).unwrap();
        let main = json!({"kind":"integration", "case":"flow", "outcome":"FAIL", "error":"prepare v2: DACL still inherits", "observed":["v1 connected"], "terminations":[{"how":"Unconfirmed"}], "verdict_after":"Blocked", "protection_evidence":[{"stage":"core_created","protection":{"state":"error","error":"read denied"}}], "protection_evidence_limits":"sequential, no actor attribution", "connection_evidence":[{"stage":"after reload","root":{"state":"error","error":"denied"}}], "root_at_report":{"state":"opened","identity":{"id":42}}, "launcher_evidence":{"records":[{"events":[{"stage":"refused","data":{"reason":"protection failed"}}]}],"errors":[]}});
        fs::write(dir.join("run-main-FAIL.json"), main.to_string()).unwrap();
        fs::write(dir.join("run-negative-FAIL.json"), json!({"kind":"harness-regression", "case":"stranger", "outcome":"FAIL", "error":"deliberate negative test"}).to_string()).unwrap();
        let mut report = json!({});
        let stderr = vec!["thread panicked".into(), "prepare v2: DACL still inherits".into()];
        let (outcome, why) = collect_test_report(&mut report, &dir, false, Some(101), &[], &stderr, vec![]);
        assert_eq!(outcome, Outcome::Fail); assert_eq!(why, "prepare v2: DACL still inherits");
        assert_eq!(report["observed"], main["observed"]);
        assert_eq!(report["error"], main["error"]);
        assert_eq!(report["terminations"], main["terminations"]);
        assert_eq!(report["protection_evidence"], main["protection_evidence"]);
        assert_eq!(report["protection_evidence_limits"], main["protection_evidence_limits"]);
        for key in ["connection_evidence", "root_at_report", "launcher_evidence"] { assert_eq!(report[key], main[key]); }
        assert_eq!(report["test"]["stderr"], json!(stderr));
        assert_eq!(report["test"]["evidence_records"].as_array().unwrap().len(), 2);
        assert_eq!(report["evidence"].as_array().unwrap().len(), 5);
        // A successful flow plus the expected negative regression is a PASS, not a failure.
        let pass = json!({"kind":"integration", "case":"flow", "outcome":"PASS", "observed":["v2 connected"]});
        fs::write(dir.join("run-main-FAIL.json"), pass.to_string()).unwrap(); // outcome comes from JSON, not filename
        assert_eq!(collect_test_report(&mut report, &dir, true, Some(0), &[], &[], vec![]).0, Outcome::Pass);
        // Capture errors and malformed evidence must not silently become a PASS.
        assert_eq!(collect_test_report(&mut report, &dir, true, Some(0), &[], &[], vec!["stderr write failed".into()]).0, Outcome::Fail);
        fs::write(dir.join("run-corrupt-FAIL.json"), b"{broken").unwrap();
        assert_eq!(collect_test_report(&mut report, &dir, true, Some(0), &[], &stderr, vec![]).0, Outcome::Fail);
        assert!(!report["test"]["evidence_errors"].as_array().unwrap().is_empty());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn missing_duplicate_and_blocked_flow_records_are_explicit() {
        let dir = std::env::temp_dir().join(format!("gil-evidence-missing-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&dir).unwrap();
        let mut report = json!({});
        let stderr = vec!["original failure before save".into()];
        assert_eq!(collect_test_report(&mut report, &dir, true, Some(0), &[], &stderr, vec![]).0, Outcome::Fail);
        assert_eq!(report["test"]["stderr"], json!(stderr));
        let blocked = json!({"kind":"integration", "case":"flow", "outcome":"BLOCKED", "error":"CLI missing"});
        fs::write(dir.join("bootstrap-a-BLOCKED.json"), blocked.to_string()).unwrap();
        assert_eq!(collect_test_report(&mut report, &dir, false, Some(101), &[], &[], vec![]), (Outcome::Blocked, "CLI missing".into()));
        fs::write(dir.join("run-b-BLOCKED.json"), blocked.to_string()).unwrap();
        assert_eq!(collect_test_report(&mut report, &dir, false, Some(101), &[], &[], vec![]).0, Outcome::Fail);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn external_tools_receive_ascii_names_and_a_separate_unicode_working_directory() {
        let dest = std::env::temp_dir().join("한글 path").join("cli");
        for arch in ["x86_64", "aarch64"] {
            let pin = pinned("windows", arch).unwrap();
            let member = format!("package/vendor/{}/bin/codex.exe", pin.triple);
            for cmd in [download_command(Path::new("curl"), &dest, &pin),
                archive_command(Path::new("tar"), &dest, &pin, None),
                archive_command(Path::new("tar"), &dest, &pin, Some(&member))] {
                assert_eq!(cmd.get_current_dir(), Some(dest.as_path()));
                let args: Vec<_> = cmd.get_args().map(|a| a.to_str().unwrap()).collect();
                assert!(args.iter().all(|a| a.is_ascii()), "{args:?}");
                assert!(!args.contains(&"-C"));
                assert!(args.contains(&pin.tarball_name().as_str()));
            }
        }
    }
    #[test]
    fn child_paths_survive_a_working_directory_change() {
        const ROLE: &str = "GIL_VERIFY_PATH_TEST_ROLE";
        let name = "runtime_verify::tests::child_paths_survive_a_working_directory_change";
        match std::env::var(ROLE).as_deref() {
            Ok("driver") => {
                let requested = std::env::var_os("GIL_VERIFY_PATH_TEST_ROOT").unwrap();
                let (outcome, report) = run(&Options { root: requested.into(), cli: CliSource::Pinned, test_exe: None, testbed: None });
                // No bundle is installed by this unit test; preparation stops before any CLI download.
                assert_eq!(outcome, Outcome::Blocked);
                let root = PathBuf::from(report["root"].as_str().unwrap());
                assert!(root.is_absolute());
                let me = fs::canonicalize(std::env::current_exe().unwrap()).unwrap();
                let output = integration_command(&me, &root.join("work"), &me, &me)
                    .args(["--exact", name]).env(ROLE, "child").output().unwrap();
                assert!(output.status.success(), "{} {}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
            }
            Ok("child") => {
                for key in ["TEMP", "TMP", "TMPDIR", "GIL_RUNTIME_INTEGRATION_EVIDENCE_DIR", "GIL_CODEX_CLI", "GIL_RUNTIME_TESTBED"] {
                    let path = PathBuf::from(std::env::var_os(key).unwrap());
                    assert!(path.is_absolute() && path.exists(), "{key}: {}", path.display());
                }
                assert_eq!(fs::canonicalize(std::env::temp_dir()).unwrap(), fs::canonicalize(std::env::current_dir().unwrap().join("tmp")).unwrap());
            }
            _ => {
                let base = std::env::temp_dir().join(format!("gil-path 한글 space-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
                for mode in ["relative", "absolute"] {
                    let dir = base.join(mode); fs::create_dir_all(&dir).unwrap();
                    let output = Command::new(std::env::current_exe().unwrap()).args(["--exact", name, "--nocapture"])
                        .current_dir(&dir).env(ROLE, "driver")
                        .env("GIL_VERIFY_PATH_TEST_ROOT", if mode == "relative" { Path::new(".") } else { &dir }).output().unwrap();
                    assert!(output.status.success(), "{} {}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
                    assert!(!dir.join("work/work").exists());
                }
                fs::remove_dir_all(base).unwrap();
            }
        }
    }
    #[test]
    fn integrity_strings_decode_to_64_bytes_and_match_sha512() {
        let want = decode_integrity(pinned("windows", "x86_64").unwrap().integrity).unwrap();
        assert_eq!(want.len(), 64);
        assert!(decode_integrity("sha256-abc").is_err());
        assert!(decode_integrity("sha512-not base64!").is_err());
        let dir = std::env::temp_dir().join(format!("gil-verify-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&dir).unwrap();
        let f = dir.join("blob"); fs::write(&f, b"hello").unwrap();
        let (s512, s256, n) = digests(&f).unwrap();
        assert_eq!(n, 5);
        assert_eq!(s256, "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
        assert_eq!(hex(&s512), "9b71d224bd62f3785d96d46ad3ea3d73319bfbc2890caadae2dff72519673ca72323c3d99ba5c11d7c7acc6e14b8c5da0c4663475c2e5c3adef46f73bcdec043");
        let _ = fs::remove_dir_all(&dir);
    }
    #[test]
    fn pe_machine_is_read_from_the_header_and_garbage_is_refused() {
        let mut img = vec![0u8; 0x80]; img[0] = b'M'; img[1] = b'Z'; img[0x3C] = 0x40; img[0x40..0x44].copy_from_slice(b"PE\0\0"); img[0x44] = 0x64; img[0x45] = 0x86;
        assert_eq!(pe_machine_of(&img).unwrap(), 0x8664);
        img[0x44] = 0x64; img[0x45] = 0xAA; assert_eq!(pe_machine_of(&img).unwrap(), 0xAA64);
        assert!(pe_machine_of(b"not an image").is_err());
        let mut bad = img.clone(); bad[0x41] = b'X'; assert!(pe_machine_of(&bad).is_err());
    }
    #[test]
    fn exactly_one_known_member_layout_is_accepted() {
        let listing = "package/package.json\npackage/vendor/x86_64-pc-windows-msvc/bin/codex.exe\npackage/vendor/x86_64-pc-windows-msvc/bin/other.exe\n";
        assert_eq!(select_member(listing, "x86_64-pc-windows-msvc", "codex.exe").unwrap(), "package/vendor/x86_64-pc-windows-msvc/bin/codex.exe");
        assert!(select_member(listing, "aarch64-pc-windows-msvc", "codex.exe").is_err(), "other triple");
        let two = format!("{listing}package/vendor/x86_64-pc-windows-msvc/codex.exe\n");
        assert!(select_member(&two, "x86_64-pc-windows-msvc", "codex.exe").unwrap_err().contains("2 candidate"));
        assert!(select_member("package/vendor/x86_64-pc-windows-msvc/bin/codex.exe/evil\n", "x86_64-pc-windows-msvc", "codex.exe").is_err());
    }
    #[test]
    fn the_decision_never_passes_without_one_pass_file_and_exit_zero() {
        assert_eq!(decide(true, "OUTCOME: PASS\n", 1).0, Outcome::Pass);
        assert_eq!(decide(true, "OUTCOME: PASS\n", 0).0, Outcome::Fail);
        assert_eq!(decide(true, "", 2).0, Outcome::Fail);
        assert_eq!(decide(false, "OUTCOME: FAIL\n", 0).0, Outcome::Fail);
        assert_eq!(decide(false, "OUTCOME: BLOCKED\n", 0).0, Outcome::Blocked);
        assert_eq!(decide(false, "x\n  OUTCOME: BLOCKED (no cli)\n", 1).0, Outcome::Blocked);
        assert_eq!(Outcome::Pass.exit_code(), 0); assert_eq!(Outcome::Fail.exit_code(), 1); assert_eq!(Outcome::Blocked.exit_code(), 2);
    }
    #[test]
    fn pinned_sources_exist_for_the_two_windows_cpus_only() {
        let x = pinned("windows", "x86_64").unwrap(); let a = pinned("windows", "aarch64").unwrap();
        assert_eq!(x.tarball_url(), "https://registry.npmjs.org/@openai/codex/-/codex-0.163.0-alpha.11.1-win32-x64.tgz");
        assert_eq!(a.tarball_url(), "https://registry.npmjs.org/@openai/codex/-/codex-0.163.0-alpha.11.1-win32-arm64.tgz");
        assert!(pinned("macos", "aarch64").is_none()); assert!(pinned("linux", "x86_64").is_none());
    }
    #[test]
    fn a_used_folder_and_a_missing_manifest_are_blocked_without_touching_anything_else() {
        let dir = std::env::temp_dir().join(format!("gil-verify-run-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(dir.join("work")).unwrap();
        fs::create_dir_all(dir.join("work").join("evidence")).unwrap();
        let earlier = dir.join("work").join("evidence").join("verify-report.json"); fs::write(&earlier, b"{\"earlier\":true}").unwrap();
        let (o, r) = run(&Options { root: dir.clone(), cli: CliSource::Pinned, test_exe: None, testbed: None });
        assert_eq!(o, Outcome::Blocked); assert!(r["reason"].as_str().unwrap().contains("used before"), "{r}");
        assert_eq!(finish(&dir, o, r), 2);
        assert_eq!(fs::read(&earlier).unwrap(), b"{\"earlier\":true}", "an earlier run's report is never overwritten");
        let fresh = dir.join("fresh"); fs::create_dir_all(&fresh).unwrap();
        let (o, r) = run(&Options { root: fresh.clone(), cli: CliSource::Pinned, test_exe: None, testbed: None });
        assert_eq!(o, Outcome::Blocked); assert!(r["reason"].as_str().unwrap().contains("bundle manifest"), "{r}");
        assert!(fresh.join("work").join("evidence").is_dir() && !fresh.join("work").join("cli").join("x").exists());
        let (o, r) = run(&Options { root: dir.join("absent"), cli: CliSource::Pinned, test_exe: None, testbed: None });
        assert_eq!(o, Outcome::Blocked); assert!(r["reason"].as_str().unwrap().contains("does not exist"), "{r}");
        let _ = fs::remove_dir_all(&dir);
    }
}
