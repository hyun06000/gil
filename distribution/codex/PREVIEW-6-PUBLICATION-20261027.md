# Preview.6 publication — 2026-10-27

Scope: opt-in unsigned Codex preview for macOS Apple Silicon. Runtime profile `visual-authoring`,
24 MCP tools, action surface 2, storage formats 4..5 and migration `requires_review`.

## Published identity

- Version: `0.2.1-preview.6`.
- Source: `db267bfdf8467b96a2a6f98dd1745148362edbb6` (`hyun06000/gil` main after PR #46).
- Source candidate workflow: [40478269851](https://github.com/hyun06000/gil/actions/runs/40478269851), passed.
- Marketplace payload PR: [#9](https://github.com/hyun06000/gil-marketplace/pull/9), merged as
  `2a1ec23576eb35e05210dda1d8b5400a577ec30f` after the pinned-payload check passed.
- Immutable tag: `v0.2.1-preview.6` on that merge commit.
- Prerelease: [v0.2.1-preview.6](https://github.com/hyun06000/gil-marketplace/releases/tag/v0.2.1-preview.6).
- Installation-document follow-up: marketplace PR
  [#10](https://github.com/hyun06000/gil-marketplace/pull/10), merged after the same check passed.

## Exact candidate

- Archive: `gil-codex-macos-arm64-0.2.1-preview.6.tar.gz`, 3,189,503 bytes.
- Archive SHA-256: `477ad55fb6757fc58c4264bd265c7dd8e196c4a5ecf648ac6e1a181a8338d2b5`.
- Core SHA-256: `1822b5ad6a52b877384053ac9bea9d05975a33f101c5cda84dff1290d984c04e`.
- Embedded Monitor UI SHA-256: `9abd2a27ae636ae69d9b0554a0b9a347f9721e0f62fabb9464bd533be34e044f`.
- Skill SHA-256: `1a39507c02a9d883bc14c2662b0738325c59f7273fa03404984941ec7c0f1bb4`.
- Source snapshot SHA-256: `591b46ccf524d73c5d22ac23bc8ab291ca57c9f08de81e41899d280329079275`.

The release contains the candidate archive plus `checks.json`, `SHA256SUMS` and
`update-record.json`. GitHub reported matching asset digests. A separate public HTTPS download of the
archive matched the size and SHA-256 above.

## Verification

- Source CI passed Core, shared UI, packaging, notice coverage, first-process, protocol, archive
  roundtrip and unsigned-candidate checks on a macOS 15 Apple Silicon runner.
- The candidate reported macOS/aarch64, 24 tools, protocol 1, action surface 2 and storage formats 4..5.
- Marketplace static integrity and raw Git history privacy tests passed before both merges.
- A fresh isolated `CODEX_HOME` registered the public fixed tag through official Plugin management,
  installed `gil-companion-prototype@gil-preview-macos-arm64`, and reported version
  `0.2.1-preview.6`. The checkout resolved to the tagged merge commit; installed Core and Skill hashes
  matched the reviewed payload.

## Changes and compatibility

This version refreshes the bundled native Core, Skill and embedded fullscreen Monitor UI from the
current source line. The reviewed visual-authoring contract remains the same as preview.5. Installing
the Plugin does not migrate an existing Project. Any format migration still requires separate backup,
recovery verification and per-project consent.

## Trust and remaining limits

Apple Developer ID signing and notarization are not provided. This is a friends-and-testers prerelease,
not a stable release or universal Plugins Directory listing. If macOS blocks execution, stop; do not
disable Gatekeeper or remove quarantine.

The exact public tag installation was verified on the maintainer's existing Mac. A clean Mac/VM,
external tester, visible fullscreen interaction and full application restart were not repeated for
preview.6. Preview.5 remains the rollback target at tag `v0.2.1-preview.5`. Plugin rollback never
changes Project records; preview.3 cannot read format-5 Projects.
