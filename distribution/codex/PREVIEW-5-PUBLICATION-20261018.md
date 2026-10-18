# Preview.5 publication — 2026-10-18

Scope: opt-in unsigned Codex/macOS Apple Silicon preview, same image-report contract as preview.4
(runtime profile `visual-authoring`, action surface 2, storage formats 4..5, 24 tools, migration
`requires_review`). **Status: published 2026-10-18 as an unsigned prerelease after separate owner
approval** — marketplace PR #7 merged as `0f841dae120e45d0ac7e2a90bd383685c8756627`, tag
`v0.2.1-preview.5` on that commit, release
https://github.com/hyun06000/gil-marketplace/releases/tag/v0.2.1-preview.5 with the four staging assets.

## What changes from preview.4
Only the companion Skill and its tests (source PRs #18–#21): `gil_close` schema rejections name the
missing field and state that nothing was closed; after any `ok: false` the agent handles the refusal
before any other action; a Synthesis waits for the human's own approval before the Interview closes;
Git-less installation documents permission requests through the Host. Core and UI bytes are rebuilt
from source but identical in contract; no storage, grammar or tool-contract change; no migration.

## Candidate identity
- Source: `c7db94ea4134167c309369065e88b43bafc482df` (main after PRs #18, #19, #20, #21).
- Source CI on that head: default [39108376445](https://github.com/hyun06000/gil/actions/runs/39108376445),
  visual-authoring [39108378176](https://github.com/hyun06000/gil/actions/runs/39108378176),
  Windows x64/ARM64 [39108380119](https://github.com/hyun06000/gil/actions/runs/39108380119) — all passed.
- CI artifact 12129791287; original CI archive SHA-256
  `3fa6713f3fd51576b95e5c9f60d748aa041b01c8f062b97b0a22b128f4275634`.
- Staged with `unsigned-preview.mjs prepare … 0.2.1-preview.5 … --visual-authoring`: candidate archive
  SHA-256 `64ab183502161b81459d7a3e96fb73f62d624b75b39bb667da44212de46632e4` (3,132,841 bytes).
  Files changed by versioning: `plugins/gil-companion-prototype/.codex-plugin/plugin.json` (version only),
  `PREVIEW.md`, `release.json`. Unchanged from CI: Core, Skill, UI, licences/notices, marketplace catalog.
- Core `efaa940222520a64b1a2202e8ff3f93a6b838c023c4baf9f20967f99dabd1506`,
  Skill `57f39445fcc0a90097b760eacc5b862162069637a4b3fa8b73ded882cc393702` (= main c7db94ea),
  UI `306e137986f918d61e0e5e39c73fbfd4aef6e77d334180e78af91a66bbd1381f`,
  receipt (`release.json`, schema 3) `d676e5392eb6d69523339a06b2c414173c24b778edc34bf399da38f66d005c95`.
- Published: marketplace PR #7 (`pinned-payload` runs 39110623033 and 39111147726 passed) merged as
  `0f841dae120e45d0ac7e2a90bd383685c8756627`; tag `v0.2.1-preview.5` → that commit; prerelease assets and
  GitHub digests: archive `64ab1835…` (3,132,841 bytes), `checks.json` `e8ca39d5…`, `SHA256SUMS` `e22c07ca…`,
  `update-record.json` `6f6bc8de…`. An unauthenticated download of all four matched byte for byte.
  Existing tags `v0.2.1-preview.2/3/4` and the preview.4 release assets are unchanged.
- 2026-10-18, fresh isolated `CODEX_HOME` with no stored Git credentials or Codex sign-in: official CLI
  registered the public tag `v0.2.1-preview.5` (clone commit `0f841dae…`) and installed the Plugin; the seven
  installed files matched staging bytes/modes; static `verify.mjs` passed on the clone; the installed
  executable answered 24 tools with format-4/format-5 synthetic projects and shared image reads.

## Verification done before publication
- CI artifact zip digest and receipt matched; nine payload files matched by hash and mode.
- Desktop server/UI check of the exact c7db94ea executable on the owner's Mac through a temporary
  connection with a synthetic image project: Cycle rectangle selection, image/caption/source display
  beside chat, click-to-enlarge and return, and preservation across an MCP server reconnect. The
  Desktop conversation loaded the installed preview.3 Skill, so the new Skill was **not** verified on
  Desktop by that check.
- Isolated headless agent runs (Codex CLI, fresh profile, synthetic projects) with the merged Skill:
  refusal handling (one corrected call of the same action), Synthesis stop until the user's own
  approval, implementation only inside an open Verify, typed Verify close with a `not_needed`
  declaration. One earlier run without the Synthesis rule skipped the approval and is recorded as a
  partial-success failure that motivated the rule.
- Marketplace branch: `checks/verify.mjs` pinned to preview.5; 28 tests, `verify.mjs` and `history.mjs`
  pass locally. Git-less script pinned to the preview.5 asset URL/size/hash; 7 tests pass against the
  staged archive; the public URL does not exist until the release is published.

## Not verified / limits
Fresh Mac or VM installation of preview.5; expanded (fullscreen) Monitor switch; full Desktop
quit/relaunch restore (an MCP server reconnect is not a Desktop restart); external testers; Developer
ID signing/notarization; Windows or Claude image-report hosts; public anonymous download of a
preview.5 release (none exists yet). `update-policy.mjs` blocks preview.4 ↔ preview.5 as
`explicit_contract_and_migration_review_required` because `migration: requires_review` is declared.

## Pins applied after publication (this follow-up)
The default Git-less path (`prepare-gitless-macos.sh`, `gitless.test.mjs`, `GITLESS-INSTALL.md`) and the
user install guide (`INSTALL.md`) stayed on preview.4 until the release existed and its asset digest was
verified; they are now pinned to:

```text
version        0.2.1-preview.5
url            https://github.com/hyun06000/gil-marketplace/releases/download/v0.2.1-preview.5/gil-codex-macos-arm64-0.2.1-preview.5.tar.gz
expected       64ab183502161b81459d7a3e96fb73f62d624b75b39bb667da44212de46632e4
expected_bytes 3132841
install pin    tag v0.2.1-preview.5 / the marketplace merge commit recorded at publication
rollback       preview.4 (tag v0.2.1-preview.4, archive b2c40ec3…)
```

Those pin values were verified against the published archive (7 Git-less tests including the success
path, and a real download through the script). A clean-Mac VM check of the preview.5 Git-less path is
still pending.

Rollback target after publication: preview.4 (`v0.2.1-preview.4`). Installing never migrates a project;
migration keeps its separate per-project consent. Never remove `.gil`, change OS security or hand-edit caches.
