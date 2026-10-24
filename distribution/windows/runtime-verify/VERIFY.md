# GIL runtime-update isolated verification bundle (Windows)

**Not a GIL release, not a product build, not an installer.** This folder holds the crate's
App Server integration test as a prebuilt test executable, the test-only synthetic-Core
testbed and a driver that runs the test inside one folder. It creates and changes test files
inside that folder and cleans up its temporary integration fixtures. Existing GIL installs,
plugin caches, user settings and projects are not modified. No installation, registration,
security-policy change or retry is performed. The harness may terminate only an App Server
it started and owns through a child handle; forced or unconfirmed termination is not PASS.
Downloaded files and evidence remain for inspection.

## What runs

`gil-runtime-verify.exe --root <folder>` (the folder this archive was extracted into):

An existing relative folder such as `--root .` is supported. The driver resolves it to an
absolute path before deriving `work`, temporary directories, evidence and child executable
paths. Changing the integration process's working directory must not reinterpret those paths.

1. Creates `<folder>\work\` (refused if it exists: a folder is used once; make a new one).
2. Verifies this bundle's files against `bundle.json` and checks that the bundle is built for
   this PC's native CPU (an x64 bundle on an ARM64 PC is refused, not emulated).
3. Codex CLI: by default downloads the pinned `@openai/codex@0.163.0-alpha.11.1` Windows
   platform package from its official npm registry URL into `work\cli\`, checks the tarball's
   SHA-512 against the integrity value recorded from the registry (a download check, **not** a
   publisher-signature verification), extracts only `codex.exe`, checks its PE machine and
   `--version`. With `--cli <path to codex.exe>` an existing CLI is used as found and recorded
   (version, path, SHA-256) and marked when it differs from the CI pin; no version is substituted.
4. Runs `runtime_integration.exe --nocapture` with an isolated `CODEX_HOME`, a temporary
   runtime store and temp directory all under `work\`: a connection on synthetic v1 keeps
   answering v1 after v2 is prepared and selected; which actions produce a v2 process (new
   thread, reload, app-server restart) is recorded; app-servers end on stdin EOF; synthetic
   Cores are observed to end on their own (never ended by pid or image name).
5. Prints the raw test output as it happens, then a `SUMMARY` block and the report JSON, and
   saves `work\evidence\verify-report.json`, the test's `run-<pid>-PASS.json` / `-FAIL.json`
   and the stdout/stderr logs. Exit code: 0 PASS, 1 FAIL, 2 BLOCKED.

`PASS` = the test process exited 0 and wrote exactly one PASS evidence file. `FAIL` = the
flow ran and an assertion or termination check did not hold. `BLOCKED` = the test did not
run (folder unusable, bundle mismatch, wrong CPU, CLI not obtained or not matching).

## Limits

- A separate App Server test over stdio with synthetic Cores: not an observation of the
  Codex Desktop daemon and not a successful update of a GIL installation.
- The pinned CLI is obtained from the npm registry at run time and is not part of this
  archive; integrity match ≠ signature. An existing CLI is only recorded.
- Unsigned executables (no Authenticode). `runtime_integration.exe` embeds build-tree paths
  as compile-time defaults; the driver overrides them with run-time environment variables.
- Nothing here wires the product manifest, MCP servers, a trust gate or recovery.

## Agent result collection

Keep the complete execution-tool response, including the session identifier and exit code;
do not forward only its output text. If the tool returns a running session, poll that same
session until completion. Do not launch the driver a second time to recover output. Report
the exit code, full SUMMARY and REPORT-JSON blocks. If the session can no longer be recovered,
read this run's `work/evidence/verify-report.json` and stdout/stderr logs without rerunning;
state explicitly when the driver exit code or process termination remains unconfirmed.
No CIM/WMI lookup or process-name termination is needed to collect the original session.

## Relative-root regression (2026-10-25)

The published `runtime-verify-deb0e6b` driver passed relative `work/tmp` to an integration
process whose working directory was already `work`. The user's `--root .` invocation thus
failed at temporary-directory canonicalization (Windows error 3) before v1/v2 observations.
The old CI self-test used an absolute root and did not exercise that invocation. This is a
driver/bootstrap defect, not evidence of GIL update failure or of OneDrive incompatibility.
Source regression tests cover both root forms and spaces/non-ASCII paths. The Windows
packaged self-test uses `--root .`; a passing result from the same source commit is required
before publication.

Third-party notices for the Rust dependencies linked into these executables:
`THIRD-PARTY-NOTICES.txt`, `THIRD-PARTY-NOTICES.json`, `RUST-STDLIB-NOTICES.html`. GIL: `LICENSE`.
