# Preview.3 publication — 2026-09-30

Scope: opt-in unsigned Codex/macOS Apple Silicon preview. Not stable, Apple-notarized,
Windows/Intel, Claude work-mode or universal directory acceptance.

- Source: `fc3a59c4a3ada98666f96256e5f8f53aeaf46b83`, merged source PRs #9 and #10.
- Candidate CI: [36662221990](https://github.com/hyun06000/gil/actions/runs/36662221990), passed.
- Marketplace PR: [#4](https://github.com/hyun06000/gil-marketplace/pull/4), required CI passed.
- Marketplace merge/install pin: `8445dbdb30c9cfe16a77245adcc4da56c9655b46`.
- Release: [v0.2.1-preview.3](https://github.com/hyun06000/gil-marketplace/releases/tag/v0.2.1-preview.3).
- Archive SHA-256: `f484c09919c3c58e722ef12a466158bcb478e56bada2e216b7cbbcdfc3102d1c`.
- Receipt SHA-256: `81248b61e4a39777c84877754c07eff849049d719bee95d883e845ba38b37188`.

Original CI payload was copied without rebuilding. Core, embedded UI and licenses are unchanged
from preview.2. Skill adds one-request start → prepare → Monitor → first Interview, bounded retries,
single-open-Cycle protection and honest visual-success reporting. No schema or storage migration.
The receipt remains publishable:false; publication authorization is separate from build-time gates.

## Evidence and limits

- Local MCP tests: 17 passed with live OS watcher branch.
- JS/packaging: 154 passed, 4 existing skips. These are not Host rendering tests.
- Marketplace integrity/history: 19 tests passed; exact nine-file payload and receipt verified.
- Official CLI installed the CI candidate locally; installed Core and Skill bytes matched.
- Owner reported success after the requested one-sentence start/fullscreen/first-node check.
  This is existing-Mac human acceptance, not an external machine result.
- The owner subsequently authorized publication and restoring the remote install source.
- Public prerelease is non-draft with all four original CI assets uploaded; GitHub's archive digest
  matches the original SHA-256 above. Official CLI registration now uses the fixed public Git commit.
  Installed preview.3 is enabled and all seven installed files/modes match the original receipt.
  This is remote installation/integrity evidence, not a second human visual check or restart test.
- One external Apple Silicon Mac tester is available; no test results have been received.
- Duplicate-start human acceptance, full restart and preview.3 rollback pair remain pending.
  T1.1 stays in progress; prior preview.2 checks are not silently inherited.

Recovery target: preview.2 at `076bb49719ffd94225c2c0adf479607e42fb09e2`, through official
Plugin management with approval. Never remove .gil, change OS security or hand-edit caches.
Use [INSTALL.md](INSTALL.md) for the fixed version. No Companion or developer tools are required.

## External tester checklist

Use an empty disposable folder, not sensitive or important work. Record macOS and Codex versions
and Apple Silicon model without publishing personal paths or conversations.

1. Install using the public fixed-ref guide without development tools. Record any OS block; stop
   rather than bypassing security.
2. New conversation, selected folder: ask to start GIL once. Observe graph, fullscreen, chat and
   first Interview node; distinguish tool success from visible UI.
3. Do a small task and click node details; observe updates.
4. Repeat the start request in the same project: no reinitialization or duplicate Cycle.
5. Restart Codex and request the existing Monitor without initializing again.
6. Report success/failure per item. Unperformed items remain unverified.
