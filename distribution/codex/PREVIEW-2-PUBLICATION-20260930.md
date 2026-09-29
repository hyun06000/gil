# Preview.2 public-route checkpoint — 2026-09-30

Scope: **Codex / macOS Apple Silicon / 0.2.1-preview.2 / opt-in unsigned preview**.
Public HTTPS installation and repository settings are verified. This is not a stable, Apple-notarized,
fresh-Mac, Windows, Intel or Claude work-mode acceptance, or a universal Plugins Directory listing.

## Public entry points

- Source: [hyun06000/gil](https://github.com/hyun06000/gil).
- Installable Git marketplace: [hyun06000/gil-marketplace](https://github.com/hyun06000/gil-marketplace).
- Exact installed ref: `076bb49719ffd94225c2c0adf479607e42fb09e2`.
- User guide: [INSTALL.md](INSTALL.md). Core, native MCP and Monitor are bundled; Companion is optional.
- Versioned publication: [v0.2.1-preview.2](https://github.com/hyun06000/gil-marketplace/releases/tag/v0.2.1-preview.2).
  Check actual publication and downloadable-asset status on that page, not by inferring it from this document.
- Private vulnerability reports: [source form](https://github.com/hyun06000/gil/security/advisories/new).

The owner approved continuing through deployment after accepting the preview.2 fullscreen and details.
Only source `gil` and active `gil-marketplace` became public. The original development repository and
`gil-distribution` were rechecked as private. Their history was neither deleted nor rewritten.

## Exact provenance (unchanged by publication documentation)

| Item | Value |
|---|---|
| Source built by CI | `77fde0132b5af139e470d3a149b4a132d01fafd2` |
| Source PR / merge | [#7](https://github.com/hyun06000/gil/pull/7) / `6c59b45bbe82b4abf21707c75c8c5015a6452612` (identical tree) |
| Candidate CI | [36566086605](https://github.com/hyun06000/gil/actions/runs/36566086605) |
| Marketplace PR / merge | [#2](https://github.com/hyun06000/gil-marketplace/pull/2) / `076bb49719ffd94225c2c0adf479607e42fb09e2` |
| Original tar.gz SHA-256 | `b1b969185e69e9c4e0549d7a37188f05e4ae60c1c9f8edc8d84b36f7b67fc6a8` |
| Receipt SHA-256 | `ed43d0d6d758d23bc1b628107fdf927892edcf324fc9ee2787ed21eb869b2f7e` |
| Core SHA-256 | `3572786190ec4a8ad3b703ebe5c8ffd9b170a4e6c7da1ea2776abfc12c520a47` |
| Embedded UI SHA-256 | `0aaf30e3c62d63fdc45919d01ece01c1ea2ac2464bf572e72b4a4d4b87b52688` |

Core, UI and notices are unchanged from preview.1. Preview.2's Skill clarifies fullscreen-by-default,
optional Companion and the Codex-only channel. Version labels and evidence change accordingly.
The receipt remains `publishable:false`: it records build-time authority, not subsequent publication
approval. Do not rewrite it or rebuild different bytes under this version. The original CI archive
is not interchangeable with GitHub's generated Source ZIP.

## Installation, recovery and preservation

- Official Codex Plugin CLI: preview.2 → preview.1 → preview.2 at reviewed remote commits.
  Both versions matched seven installed files, modes, 17 tools and embedded UI hash. Fresh native
  processes restored the approved scope without another prepare call and read node details.
- The existing explicitly approved Project and three saved-binding entries had identical fingerprints
  before/after. No user write tool, Companion launch, cache edit or project restore was used.
- An isolated synthetic Project passed preview.1 → preview.2 → preview.1 → preview.2 with the same
  timeline/detail and unchanged Project bytes. This does not substitute for Host installation.
- Anonymous HTTPS clone used no credentials, global/system Git config or credential helper.
  At the pinned ref, 19 integrity/history tests passed; 21 files and nine payload files matched;
  all five reachable commit identities passed the marketplace history guard.
- Official Codex registration then switched from SSH to public HTTPS at the **same ref**.
  The actual installed preview.2 again passed seven-file/native 17-tool/UI/detail/saved-scope checks.
  Project and bindings were unchanged.
- Human acceptance: the owner confirmed preview.2 fullscreen graph and node details before this pass.
  HTTPS checks the same payload; it is not a new visual observation or a complete Host restart.

The official installer can prune old caches. Rollback relies on immutable refs and receipts, not an
old cache still existing. Preview.1 ref: `e63963db63f0bfaf11be9d7939873e7a31fe05be`.
Rollback is not `gil restore` and never removes `.gil`. Earlier basic Host restart UX acceptance is
separate; perfect restoration of every presentation detail is not promised.

## Public-surface review

Before visibility changed, inspected source at `6c59b45` (19 reachable commits, 345 blobs), marketplace
at `076bb49` (five commits, 31 blobs), GitHub PR/issues/comments, nine source CI logs, four marketplace CI
logs and 11 retained source artifacts. Eight artifact IDs matched earlier audits; three newer archives
were downloaded and their nested contents reviewed. Targeted checks found no actual developer paths,
user records or secrets. Two generic home-path matches were synthetic negative fixtures. Gitleaks 8.30.1
found no secrets in fetched Git histories or collected GitHub text surfaces.

This is a bounded publication audit, not a complete security or malware assessment. Source attribution,
including the previously reviewed vendor co-author exception, remains intact; marketplace history has
the stricter GitHub no-reply-only guard. Private raw audit logs and acceptance Project data are not shipped.
New PR commits and CI outputs are still reviewed before merge; these counts describe the pre-change baseline.

## Enforced settings

Both repositories were changed and read back through GitHub's API:

- Main requires a PR, including for administrators; zero external approvals permits one-maintainer operation.
- Required checks: source `macos-arm64-preview`, marketplace `pinned-payload`; branch must be current.
- Conversation resolution required; force push and branch deletion disabled.
- Private Vulnerability Reporting enabled; owner administrative access verified.

See [SECURITY](../../SECURITY.md), [SUPPORT](../../SUPPORT.md) and [conduct policy](../../CODE_OF_CONDUCT.md).
No fake vulnerability was submitted, no notification email receipt was claimed, and there is no invented
private conduct inbox, personal email, response deadline or bounty. Enabled settings are not proof of
an actual report or notification being received.

## Limits that stay open

Apple signing/notarization and fresh-Mac acceptance; Windows/Intel; Claude work-mode UI; universal directory
submission; independent private conduct reporting; notification delivery; this version's complete Host
restart and manual inline/vertical persistence. Tauri-only verification remains deferred. Companion is
preserved, not bundled or required. Never bypass OS security.
