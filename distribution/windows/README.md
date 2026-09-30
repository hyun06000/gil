# Windows native feasibility — not a release

First target: Windows x64 / MSVC. ARM64 and supported end-user OS versions are not yet accepted.
This maintainer CI builds the native Core/MCP executable and runs a dependency-free Node test driver.
Node, npm and Rust are CI tools, not proposed end-user installation requirements.
No executable is published and no marketplace entry or existing Mac installation is changed.

`windows-spike.yml` checks native handshake, MCP initialize/list/resource bytes, project start/status,
duplicate-start refusal and record preservation in a temporary Unicode/space-containing path.
The Windows Monitor preparation must currently refuse with `resume_unavailable`.
This is an explicit security blocker, not a passing Windows Monitor acceptance test.
The legacy loopback Monitor's Unix signal wait remains unsupported on Windows; MCP stdio is separate.

## Remaining gates

- Stable Windows Project identity (volume/file identity, not pathname/timestamps alone).
- Private settings ACL and reparse-point safety, atomic persistence and restart recovery.
- Port Unix-specific test helpers and run the full relevant Windows regression suite.
- Native plugin packaging, execution trust, remote marketplace installation/update/removal.
- Real Windows/Codex fullscreen horizontal graph, details, watcher, locks and restart acceptance.

Mac cross-target `cargo check` proves compilation only, not Windows linking or execution.
Running `node distribution/windows/spike.mjs <native-binary>` on Mac validates the harness's shared
protocol path but does not exercise the Windows refusal branch. Windows CI results must be recorded
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
