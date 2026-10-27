# Guided GIL progress — development verification

This source change addresses missed Monitor opening, work continuing under one completed node,
and difficulty seeing recorded progress. It is not an installer release or live Host acceptance.

Follow-up: owner.4 field testing found work continuing inside an approved Interview. The original
18/18 decision result below did not cover that multi-call failure. See the later
[Interview handoff verification](INTERVIEW-HANDOFF-ACCEPTANCE-20261027.md) for the fix, expanded tests,
observed failures and limits; this historical result is not end-to-end acceptance.

## Behavior

- A successful MCP `gil_start` prepares the Monitor for the same explicit root and puts the exact
  `show_gil_monitor` call first in text content. The original structured receipt stays intact.
  A preparation failure does not undo creation, become a failed start, or justify repeating start.
- Direct MCP and Plugin installations receive common server instructions and local tool hints.
  Open → actual work → close follows the recorded `done_when`; a completed contract is closed before
  a different objective. An unfinished Verify can contain multiple edits and tests. User approval
  is still required at Synthesis. No retrospective/fabricated steps are generated.
- The shared Monitor presents the recorded action, completion condition, current node, closed-step
  count, and the time this screen observed a record change. Polls and file dirtiness do not advance
  that time. First observation is labeled separately. Connection failures retain the last record.
  These observations are local to the screen, not persisted agent activity or private reasoning.

## Checks on macOS arm64

- `cargo test --locked -p gil`: 1,422 passed, no failures (two ignored). The first sandbox run could
  not open local sockets; rerunning with local IPC permitted passed. Existing start tests were
  updated for automatic preparation and separate guidance text; structured CLI/MCP equality remains.
- `cargo build --locked -p gil --all-targets`: passed, existing launcher dead-code warnings remain.
- `cargo test --locked -p gil --features visual-authoring,product-safety --test mcp`: 28 passed.
- JS/layout/Plugin/macOS-and-Windows packaging/license tests: 234 passed, five existing skips for
  unavailable bundled binaries or published archives, no failures.
- `mcp-app/browser-check.mjs` in real Chrome with a mock Host: passed graph/detail/fullscreen,
  progress contract rendering, connection failure/recovery, and narrow viewport checks. Shared
  native/fixture UI selftest: 72/72. This is not evidence of real Codex fullscreen visibility.
  The test server now serves `.mjs` as JavaScript; the fixture rejects visible unsupported native
  management buttons while permitting their hidden shared markup.
- Skill Creator `quick_validate.py`: passed for the Plugin skill.
- `cargo test --locked -p gil-companion`: 102 passed, no failures. This builds/tests Companion
  separately but does not run or install its native window.

## Lightweight model decision check

Run manually with an authenticated Codex CLI and a built `target/debug/gil`:

```sh
node mcp-app/guidance-eval.mjs
```

`GIL_BINARY`, `GIL_CODEX_CLI`, and `GIL_EVAL_MODEL` are optional overrides. This is opt-in, not part
of ordinary CI, and consumes model usage. It uses temporary synthetic projects, reads actual MCP
initialize/start/open and Monitor-preparation-failure responses, and never uses real project data.
The remaining cases are synthetic state summaries. JSON results and logs stay in a printed temp
directory. Expected choices are scored by the script and are not supplied to the model.

Model: `gpt-6-luna`, low reasoning. Two independent CLI invocations, nine cases each: direct MCP
instructions alone and instructions plus the Plugin skill. Final result: 9/9 and 9/9. Cases cover
start → show, waiting for a reply, closing after a reply, incomplete/complete Verify, Synthesis
without approval, a refused close, resuming an existing project, and successful start with failed
Monitor preparation. Neither mode claimed confirmed screen visibility from protocol evidence.

Earlier trials exposed extending a completed Verify for a new request and treating display failure
as a Project blocker. Guidance now distinguishes both. A trial also labeled report preparation as
`perform_current_action` while its prose said to close; the evaluation now explicitly distinguishes
external work from preparing/submitting a close report. The failure case was strengthened to use an
actual response. These are iterations on both guidance and the test, not a controlled before/after
benchmark or a statistical reliability estimate.

## Limits and next acceptance

The model selects its next action from supplied scenarios; it does not execute a complete live
project in this evaluation. MCP cannot observe or enforce unrelated editor/shell calls or thinking.
Do not claim that lightweight models can no longer skip GIL. On the next separately authorized
build, test actual Windows and Mac installations with a new lightweight-model conversation: start,
visible Monitor, Interview, Verify, close, new objective, and reconnection. Installed versions and
marketplace tags are not changed by these source tests.
