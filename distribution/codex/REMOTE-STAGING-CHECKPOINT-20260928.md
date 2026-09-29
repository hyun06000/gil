# Private remote distribution checkpoint — 2026-09-28

Status: **private Git transport and native protocol accepted; not a public release or Host install**.

This is the historical 9/28 staging boundary. The separately approved later Host installation,
removal/reinstallation and human screen result are in the
[9/29 Host checkpoint](HOST-INSTALL-CHECKPOINT-20260929.md); they do not turn this into a public release.

## Source and approvals

- Source PR [#3](https://github.com/hyun06000/gil/pull/3) was reviewed, passed exact-head CI and merged
  through GitHub. It introduced unsigned candidate tooling, not a public distribution endpoint.
- Claude's original `24cd756` investigation was retained in source PR
  [#4](https://github.com/hyun06000/gil/pull/4), reviewed, clarified and merged after CI
  [36412928195](https://github.com/hyun06000/gil/actions/runs/36412928195) passed on `d774a86`.
- The maintainer explicitly approved creating `hyun06000/gil-distribution` **privately** for remote
  acceptance. Current installed Plugin replacement, public visibility and release publication remain separate.
- Both source and distribution repositories were read back as private. Rulesets on the new private
  repository returned the plan-limit 403. PR-only remains an operating rule, not server enforcement.

## Reviewed candidate in Git

Distribution repository: [hyun06000/gil-distribution](https://github.com/hyun06000/gil-distribution).
After GitHub's initial README commit, the payload was proposed and reviewed in
[distribution PR #1](https://github.com/hyun06000/gil-distribution/pull/1).
It was merged after exact-head CI and the fresh-clone checks below; no direct main push was used.

| Evidence | Value |
|---|---|
| Reviewed distribution head | `e7e02a14117183c3a596daa6ec9ca890cccc3c0e` |
| Candidate | `0.2.1-preview.1`, `preview_unsigned` |
| Plugin / marketplace | `gil-companion-prototype` / `gil-preview-macos-arm64`, unchanged |
| Source CI | [36405273140](https://github.com/hyun06000/gil/actions/runs/36405273140) |
| Source commit | `da7fa6f66fefc59d21b0297ed61990ed366ef1bd`, clean, 280 files |
| Original archive SHA-256 | `063ac763eac73e9fcbd36b44fb45d48f1fc89aa8c8aea89d276579006e99f782` |
| Receipt SHA-256 | `9e70d44dfb16cd6b77232e28e34e0bbb8b402f82008690cc5d28fd2af0ee2312` |
| Core SHA-256 | `3572786190ec4a8ad3b703ebe5c8ffd9b170a4e6c7da1ea2776abfc12c520a47` |
| Embedded UI SHA-256 | `0aaf30e3c62d63fdc45919d01ece01c1ea2ac2464bf572e72b4a4d4b87b52688` |

The source commit predates the later documentation-only Claude diagnosis; runtime bytes did not change.
No public version/tag exists. The candidate's archive is not duplicated in Git; its original CI receipt,
per-file hashes/modes, warning and legal notices are retained. A GitHub source ZIP has different bytes
and must not be compared with the original candidate archive checksum.

The distribution repository adds maintainer documentation, original CI evidence and a static verifier
outside the Plugin directory. The verifier pins the receipt itself so rewriting a Skill and its checksum
together fails. CI checks exact files, modes, identity, source and compatibility without running a macOS
executable on its Linux runner. Original third-party whitespace/CRLF is retained, with narrow Git
attributes preventing newline normalization. No license text was reformatted.

## Acceptance performed

- Distribution local tests: **10 passed, 0 failed**; official Plugin validator and marketplace-name helper passed.
- All nine staged payload hashes match the original CI receipt; Core is Git mode `100755`.
- Distribution PR CI: **10 passed**, [36413446692](https://github.com/hyun06000/gil-distribution/actions/runs/36413446692),
  reviewed head `e7e02a1`. Static integrity only.
- Fresh clone from GitHub at that head: exact inventory, receipt, metadata and permissions passed again.
- On the existing Mac, the reviewed source smoke harness launched that fresh clone with system-only
  PATH and isolated temporary state: first process **621 ms**, initialize **7 ms**, **17 tools**, current
  embedded UI hash, system libraries only, no automatic retry. Temporary test state was removed by the harness.
- Source Core/tool/UI code, installed Plugin, user settings, real Projects and Companion were not changed.

One audit command initially hit Node's default 1 MiB output buffer when reading the 8.3 MB staged Core
blob. The audit was rerun with a 20 MiB bound and all hashes matched. This was not a native launch failure.
The old first-launch timeout remains a historical unresolved observation; these passes do not erase it.

## Next approval boundary

The remote Git tree is a valid acceptance source, **not yet an accepted Host installation**. Next, after
approval to replace the current installed development Plugin, use the official marketplace route at a
reviewed immutable distribution commit. Preserve the prior local install source for recovery; do not
rewrite cache or config files by hand, and do not silently register another source under the same
marketplace identity. Test visible fullscreen, restart, actual update/removal/reinstall and Project preservation.

The default installation target remains Codex/macOS Apple Silicon. Claude's folder-route and permission
prompt issues are recorded in [CLAUDE-DESKTOP-ROUTES](../../mcp-app/CLAUDE-DESKTOP-ROUTES.md), not worked around
by adding filesystem privileges, disabling approvals or changing Codex automatic fullscreen.

Public source/history and CI surface audit, a live private security/contact route, enforceable main
protection at public transition, immutable public distribution and publication approval remain open.
Apple Developer ID/notarization are absent, fresh-Mac acceptance is deferred (not passed), and
Windows/Intel/Tauri-specific acceptance is not claimed.

Official packaging reference: [OpenAI marketplace sources and Git refs](https://developers.openai.com/plugins/build/plugins).
Git marketplace registration is separate from universal Plugins Directory publication. The private URL
above is not presented as an installation endpoint for unauthenticated users.
