# Preview.5 publication — prepared 2026-10-18 (NOT published)

Scope: opt-in unsigned Codex/macOS Apple Silicon preview, same image-report contract as preview.4
(runtime profile `visual-authoring`, action surface 2, storage formats 4..5, 24 tools, migration
`requires_review`). **Status: local staging and isolated verification complete; no tag, release,
marketplace merge or installation change has been made.** Publication needs separate approval.

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
- Intended destinations (not executed): marketplace branch `release/preview5-20261018` → PR → `pinned-payload`
  → merge → tag `v0.2.1-preview.5` → prerelease with the four staging assets. Existing tags and the
  preview.4 release assets stay untouched.

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

## After publication only: changes to apply in a follow-up PR (not in this branch)
The default Git-less path (`prepare-gitless-macos.sh`, `gitless.test.mjs`, `GITLESS-INSTALL.md`) and
the user install guide (`INSTALL.md`) stay on the published preview.4 until the preview.5 release and
its assets exist. Once `v0.2.1-preview.5` is published and its asset digest is verified, pin them to:

```text
version        0.2.1-preview.5
url            https://github.com/hyun06000/gil-marketplace/releases/download/v0.2.1-preview.5/gil-codex-macos-arm64-0.2.1-preview.5.tar.gz
expected       64ab183502161b81459d7a3e96fb73f62d624b75b39bb667da44212de46632e4
expected_bytes 3132841
install pin    tag v0.2.1-preview.5 / the marketplace merge commit recorded at publication
rollback       preview.4 (tag v0.2.1-preview.4, archive b2c40ec3…)
```

Those pin values were verified locally against the staged archive (7 Git-less tests, including the
success path) but are intentionally not merged before the release exists, so the default path keeps
downloading and installing preview.4 until then.

Rollback target after publication: preview.4 (`v0.2.1-preview.4`). Installing never migrates a project;
migration keeps its separate per-project consent. Never remove `.gil`, change OS security or hand-edit caches.
