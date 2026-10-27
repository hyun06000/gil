# Preview.7 publication — 2026-10-28

Scope: opt-in unsigned Codex Plugin for macOS Apple Silicon, for friends and testers.
Windows owner.6 and this Mac build share source commit and agent workflow. Their host installation
and runtime profiles remain different: Mac uses `visual-authoring`, 24 tools, action surface 2,
storage formats 4..5 and migration `requires_review`. Installation does not migrate Project records.

## Published identity

- Version: `0.2.1-preview.7`.
- Source: `64de8e5c1207dda329600ef12a86f39cf7781ee3`, after source
  [PR #53](https://github.com/hyun06000/gil/pull/53) passed all eight checks and merged.
- Candidate workflow: [40568947391](https://github.com/hyun06000/gil/actions/runs/40568947391), passed.
- Marketplace payload: [PR #11](https://github.com/hyun06000/gil-marketplace/pull/11), merged after
  pinned-payload CI passed, as `b81151830a4d723be95705b8f9274b1ff3ed3adb`.
- Fixed tag: `v0.2.1-preview.7` on that marketplace merge commit.
- Prerelease: [v0.2.1-preview.7](https://github.com/hyun06000/gil-marketplace/releases/tag/v0.2.1-preview.7).

## Exact candidate

- Archive: `gil-codex-macos-arm64-0.2.1-preview.7.tar.gz`, 3,218,986 bytes.
- Archive SHA-256: `ecd045e3a043e820bb5ff59a6d8b6d3ca57e68dfa3d8a8426a598f70682dd8ee`.
- Core SHA-256: `badd9a6f208dee96402637c54bc741b4a3e9e75cfbbb650a94a1608213bd1bba`.
- Embedded Monitor UI SHA-256: `8df673ca26fc8f90ca6f9c8f2a256b87b535f8f19fbb3cd3d2f9e93154077b33`.
- Skill SHA-256: `54a18db9c896d77aa737547089129d676804aea246d36452f783b13c7ef1d10a`.
- Shared workflow SHA-256: `fe3cca1210d211ac7f3bdc58198d9044ab0cad6407c71709ec813d0c6177ea22`.
- Source snapshot SHA-256: `2f7edbb49478f6e6eb9f5c6017bc2978a64035e9529d17dbd8da014009845971`.

The public release includes the archive, `checks.json`, `SHA256SUMS` and `update-record.json`.
An unauthenticated HTTPS download matched the exact archive size and hash above.

## Verification and behavior

- Native macOS CI passed packaging, protocol, notices and first-process checks.
- Marketplace integrity/history checks passed (28 tests). Payload files and executable modes match
  the candidate receipt; installed cache and personal settings were not edited manually.
- Official CLI installed the public fixed Git tag into a fresh isolated profile. Plugin list reported
  preview.7; cached Core and Skill hashes matched the reviewed payload.
- Isolated official installs switched preview.6 → preview.7 → preview.6 → preview.7. Both Core
  versions read synthetic format-4 and format-5 Projects without changing their bytes.
- The exact public archive passed the Gitless preparation test, including complete local marketplace
  validation (7/7 tests). The remaining packaging/notices suite passed 95 tests with one archive test
  skipped in that invocation; the separate public-archive run covered that test.
- Native MCP initialization and the generated Plugin Skill carry the same workflow text. It directs
  opening Monitor with Project start, closing Interview after agreement, opening Verify before
  investigation, recording small experiments, durable file evidence, interpretation and learning.
- Actual Luna synthetic tasks passed all ten checks in each of four runtime/entry combinations;
  each left two experiments and two real files. Independent next-action probes passed 61/64.
  Three chat-only Verify probes still selected an incorrect action label despite describing saving
  files first. Model compliance is not claimed to be perfect; Core evidence gates remain enforced.

## Limits and rollback

Apple Developer ID signing and notarization are absent. This is an unsigned friends-and-testers
prerelease, not a stable or Intel Mac release. If macOS blocks it, stop rather than disabling Gatekeeper
or removing quarantine. The previous clean-VM acceptance belongs to preview.6. This release has local
isolated installation checks, not a new clean-VM, native Windows Luna or visible fullscreen acceptance.
The user's post-update Luna trial remains a separate acceptance step.

Rollback target is `v0.2.1-preview.6`, marketplace commit
`2a1ec23576eb35e05210dda1d8b5400a577ec30f`. Plugin rollback never changes Project records.
