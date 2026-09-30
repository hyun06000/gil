# Windows native feasibility — not a release

First target: Windows x64 / MSVC. ARM64 and supported end-user OS versions are not yet accepted.
This maintainer CI builds the native Core/MCP executable and runs a dependency-free Node test driver.
Node, npm and Rust are CI tools, not proposed end-user installation requirements.
No executable is published and no marketplace entry or existing Mac installation is changed.

`windows-spike.yml` checks native handshake, MCP initialize/list/resource bytes, project start/status,
duplicate-start refusal and record preservation in a temporary Unicode/space-containing path.
The first spike deliberately refused Monitor preparation. The follow-up implements local NTFS
identity and private binding storage; the smoke now requires a read-only View and live OS watcher.
Protocol success still is not a passing Windows/Codex screen acceptance test.
The legacy loopback Monitor's Unix signal wait remains unsupported on Windows; MCP stdio is separate.

## Remaining gates

- Validate the Windows identity/private settings implementation on the native runner (below).
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

- Local drive paths on NTFS only. UNC, ReFS/FAT and any reparse-point component are refused.
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
