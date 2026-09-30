# Windows native feasibility — not a release

First target: Windows x64 / MSVC, local fixed NTFS drives. ARM64 and supported end-user OS versions are not yet accepted.
This maintainer CI builds the native Core/MCP executable and runs a dependency-free Node test driver.
Node, npm and Rust are CI tools, not proposed end-user installation requirements.
No release is published and no active marketplace entry or existing Mac installation is changed.
An opt-in workflow dispatch can retain an unsigned review artifact for seven days.

`windows-spike.yml` checks native handshake, MCP initialize/list/resource bytes, project start/status,
duplicate-start refusal and record preservation in a temporary Unicode/space-containing path.
The first spike deliberately refused Monitor preparation. The follow-up implements local NTFS
identity and private binding storage; the smoke now requires a read-only View and live OS watcher.
Protocol success still is not a passing Windows/Codex screen acceptance test.
The legacy loopback Monitor's Unix signal wait remains unsupported on Windows; MCP stdio is separate.

## Remaining gates

- Validate on an ordinary tester account and Codex (hosted native runner passed; below).
- Port Unix-specific test helpers and run the full relevant Windows regression suite.
- Native plugin packaging, execution trust, remote marketplace installation/update/removal.
- Real Windows/Codex fullscreen horizontal graph, details, watcher, locks and restart acceptance.

Mac cross-target `cargo check` proves compilation only, not Windows linking or execution.
Running `node distribution/windows/spike.mjs <native-binary>` on Mac validates the harness's shared
  protocol path but does not exercise Windows ACLs. Windows CI results must be recorded
separately. Do not disable OS security or relax binding privacy checks to make the spike pass.

## Local checkpoint — 2026-09-30

- macOS → `x86_64-pc-windows-msvc` `cargo check --locked -p gil --bin gil`: passed, no warnings.
- macOS native build and the shared smoke driver: passed (17 tools, embedded UI hash,
  start/status and duplicate-start byte preservation).
- Windows x64 native runner: **passed** on source commit `3fd7e57`;
  [CI run 36679663944](https://github.com/hyun06000/gil/actions/runs/36679663944).
  Build completed without warnings. The executable answered its native challenge, exposed 17 MCP
  tools, served hash-matching embedded UI, started/read a temporary Project, refused duplicate start
  without changing state bytes, and refused Monitor preparation with `resume_unavailable` as expected.
- Existing macOS CI also passed:
  [run 36679663787](https://github.com/hyun06000/gil/actions/runs/36679663787).
- Windows installation and visible fullscreen: **not run**. The full Windows Rust suite is not part
  of this smoke gate; the outstanding platform security work remains unchanged.

## Monitor implementation candidate

- Local fixed drive paths on NTFS only. Mapped network/removable drives, UNC, ReFS/FAT and any
  reparse-point component are refused. Unsupported root drives are checked before creating settings.
- Stable `.gil` directory volume serial + 64-bit NTFS file index + creation time; existing binding
  schema and Unix identity remain unchanged. `state.yaml` replacement does not change identity.
- Directory handles pin ancestors against rename/delete during storage operations. Read records
  through handles that deny concurrent writes/delete; validate owner and protected DACL on the handle.
- New settings directories and records have exactly one effective full-access ACE for the current
  process user SID, with inheritance disabled. Existing ACLs are never silently repaired.
- Flush new file bytes, close the private temporary file, then same-directory `MoveFileExW` with
  `MOVEFILE_WRITE_THROUGH` and without replacement/cross-volume copy. Corrupt/existing records survive.
  Power-loss durability on every storage controller is not established by these tests.
- Native tests cover persistence, Project replacement, concurrency, corrupt/future records, scope
  traversal, Unicode paths, public ACL refusal and junction refusal. The normal MCP integration suite
  additionally checks process restart and stale scopes against the real executable.
- `windows-sys 0.61.2` was already locked, and is now a direct Windows-only dependency. Mac dependency
  inventory is unchanged (only Cargo input digests in the Mac notice policy are refreshed). Windows
  dependency notices and package trust checks remain a packaging gate before any distribution.

References: [NTFS file identity](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/ns-fileapi-by_handle_file_information),
[file security](https://learn.microsoft.com/en-us/windows/win32/fileio/file-security-and-access-rights),
[MoveFileExW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw).

## Native Monitor checkpoint — 2026-09-30

Source `55d5ebf` passed [Windows CI 36684755556](https://github.com/hyun06000/gil/actions/runs/36684755556):
11 binding/security tests + 17 MCP integration tests, no failures or skips, native build without warnings.
The smoke received a complete read-only Monitor View with `watching: true`. This includes process
restart/restoration, stale-scope refusal, first Step hint, protected ACL, junction rejection and
rename-guard tests; it does not include real Codex rendering. Mac CI
[36684755606](https://github.com/hyun06000/gil/actions/runs/36684755606) also passed.

First failures and fixes are retained in PR #12, not erased by a green rerun:

- A Unix-only slash expectation in a receipt test was changed to the native relative path.
- An attribute-only directory handle did not enforce the intended sharing guard. It now requests
  read access; the real rename refusal assertion passed unchanged.
- The ACL mutation test needed READ_CONTROL as well as WRITE_DAC; junction fixture creation now uses
  PowerShell with paths passed through environment values, never source interpolation. Neither test skips.
- Mac's initial sandbox MCP run missed a watcher hint. The unrestricted MCP rerun and full
  `cargo test --locked -p gil` / `cargo build --locked -p gil --all-targets` passed.

Still open: Windows dependency-notice inventory, installable artifact/trust checks, ordinary-user
installation, real Codex fullscreen and tester acceptance. No Windows package has been released.

## Unsigned packaging candidate (not tester installation approval)

Dispatch `windows-spike.yml` on the reviewed branch with `prepare_candidate=true`.
`candidate.mjs` checks a separate pinned Windows notice policy (89 packages), builds with a static
CRT and remapped build paths, inspects x64 PE imports, assembles a native-only plugin and an isolated
`gil-preview-windows-x64` catalog, and verifies every file after archive extraction. The extracted
binary must pass the native smoke with no Node/Cargo on its child PATH. The workflow only retains
the archive, SHA256SUMS and evidence; it never pushes a catalog, installs, or creates a release.

The source-level runtime/build dependency graph and original legal texts are recorded separately
under `compliance/`. Shared notice validation defaults remain macOS-only; selecting Windows is explicit.
The stdlib notice is the complete compiler distribution notice already reviewed for Rust 1.97.1.
Static MSVC runtime redistribution review is a separate pending gate, not certified by Cargo coverage.
Authenticode, downloaded-file security prompts, ordinary-user Codex install and visible fullscreen
also remain unverified. Never advise users to disable Defender/SmartScreen or change execution policy.

Packaging failures retained: run 36689441583 lacked prefetched workspace metadata dependencies;
the offline gate remained and CI now fetches the locked target first. Run 36689772910 then built
the release but stopped at the DLL allowlist: `bcryptprimitives.dll` was missing from that list.
It is the Windows [ProcessPrng system library](https://learn.microsoft.com/en-us/windows/win32/seccng/processprng),
now explicitly tested. No arbitrary DLL directory was permitted.
Run 36690272236 passed release/import checks but Windows tar converted the Unicode `-C` path
to question marks. Archive transport now uses stdin/stdout bytes and a Unicode process cwd,
retaining the same Korean/space-containing extraction and executable path acceptance gate.
Microsoft documents [static CRT deployment and its update responsibility](https://learn.microsoft.com/en-us/cpp/windows/deployment-in-visual-cpp?view=msvc-170)
and separate [redistribution terms](https://learn.microsoft.com/en-us/cpp/windows/redistributing-visual-cpp-files?view=msvc-170).
This candidate does not treat static linking as automatic license approval or an OS trust bypass.

Do not send a nondeveloper tester these build commands or a CI login requirement. After release gates
and a stable installation source are ready, give them one Codex prompt and a simple graph/detail/restart
check. Windows ARM64/Snapdragon is not this candidate's target.

## Downloaded candidate checkpoint — 2026-09-30

Source `7d7aec67b6f5feca03cebfe5d3e9de923bf5c7d3`,
[Windows candidate run 36690752815](https://github.com/hyun06000/gil/actions/runs/36690752815): passed.
Native binding/MCP tests 28, packaging tests 7; relocated release binary smoke passed with 17 tools,
matching embedded UI bytes, read-only Monitor View and active watcher. Same-source Mac PR CI
[36690756906](https://github.com/hyun06000/gil/actions/runs/36690756906) and Windows PR CI
[36690756776](https://github.com/hyun06000/gil/actions/runs/36690756776) passed.
Local Mac packaging/compliance regression: 82 passed, no skips.

Archive: `gil-windows-x64-candidate.tar.gz`, 2,693,120 bytes.
SHA-256: `1071266e719093cb1d6c23c9641e32972e9f0296db5851b78df0330d384b4d05`.
Downloaded independently; archive checksum, all 8 extracted file hashes, PE imports and the
official plugin-creator validator passed. No executable was run on the Mac during this validation.
Imports: advapi32, api-ms-win-core-synch-l1-2-0, bcryptprimitives, kernel32, ntdll, ws2_32 (all DLLs).
No external VCRUNTIME/Node dependency was present in the PE import table. This is not a blanket
claim about every dynamically loaded dependency or every clean Windows installation.

The receipt remains `publishable: false`: no Authenticode signature, actual Windows/Codex install
and fullscreen not yet tested, no marketplace publication, MSVC runtime redistribution review pending.
CI retention is seven days; a CI artifact is not the stable nondeveloper installation channel.
