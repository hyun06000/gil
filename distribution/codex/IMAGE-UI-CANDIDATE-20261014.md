# Image report UI — release preparation checkpoint

Date: 2026-10-14. Scope: macOS Apple Silicon, Codex, `visual-authoring`.
Status: isolated development candidate verified; **not published or approved for publication**.
The installed public Plugin and user projects have not been replaced or migrated.

## Candidate and UI

The release-mode native executable embeds the UI accepted in the preceding Codex session:
click an image to enlarge and click again to return; click a Cycle rectangle to select its
summary and report. Numbered image selectors, explicit zoom buttons and the Cycle dropdown
are removed. Report ownership and cross-report image reuse are unchanged.

- UI SHA-256: `306e137986f918d61e0e5e39c73fbfd4aef6e77d334180e78af91a66bbd1381f`.
- Development archive SHA-256: `6be3f51432ba0e3c0ff280fc27747fcce448180e6f01b15781732045e609a2b1`.
- Source base: `a2e4f073ad0437860b6e42f430f9e5253d6f2463`, with uncommitted changes.
- Build-time source snapshot: `7440f32d32cfb3d5e19c8b0df6897c4cb6d239a745def172a312960779b12173`.
- Contract: 24 tools, action surface 2, storage 4..5, protocol 1, explicit migration review.

These hashes identify local evidence, not publisher authentication. This document is recorded
after the build and is not part of that build-time source snapshot. The archive remains
`development_unsigned`, `publishable: false`. Do not edit receipts to bypass clean-source gates.

## Essential checks

- Archive extraction round-trip, payload bytes/modes, executable permissions, manifest entry,
  native descriptor/tools/UI, system-library-only runtime and 93 dependency notices passed.
- Official local Plugin installation into a new isolated profile passed. Every installed
  payload file matched the candidate bytes and permissions; no production profile was used.
- The exact installed release executable served the synthetic Verify C2/S3 and Cycle C2
  reports and shared PNG. Two separate server lifetimes returned the same persistent report
  references, descriptions, provenance, image digest, scope and UI digest.
- 26 protected configuration/installed-Skill/synthetic-record fingerprints remained unchanged.
  The temporary MCP configuration baseline includes the explicitly approved prior reconnection.
- JavaScript UI/Plugin/packaging/notices: 191 tests, 187 passed, 4 existing skips, no failures.
- Working-tree bounded scan: 128 changed/untracked file entries, no matches for the checked
  credential patterns, personal development paths, raw NULs or excluded generated/user material.
  This is not a complete secret scan or security certification. No secret values were printed.
- Rust full sequential regression: 1,338 passed, 0 failed, 3 existing ignored child-process
  harness tests across 35 suite summaries. `cargo build --locked -p gil --all-targets
  --features visual-authoring` also passed. These results do not erase the parallel failure below.

### Failures retained in the evidence

The sandboxed Rust run could not bind loopback ports (50 failures). With loopback available,
the parallel library run passed 504 tests but failed `looking_does_not_make_more_hints`
(observation count 2 versus 1). Its settling window is 250 ms and reconciliation interval is
3 seconds; delayed startup events or scheduling are possible explanations, not a proven cause.
The full suite passed sequentially, without changing production behavior or weakening
the assertion. That pass does not erase the parallel failure; the clean CI run must
resolve or reproduce this timing risk before publication.

The first isolated runtime harness created its private Monitor directory with default permissions.
The server correctly refused it with `settings_unreadable`. Creating that synthetic directory
as 0700 fixed the harness; no server security rule or existing directory permission was changed.

## What these checks do not prove

The preceding actual Codex UI acceptance used an isolated development executable with identical
embedded HTML. It verified image click enlargement/return and Cycle selection, not a new clean
CI package's Desktop installation. The local release candidate checks above are installation
and protocol evidence, not a substitute for that final Host check.

Windows image-authoring builds, Claude distribution acceptance, signed/notarized distribution,
public download and a new-version update/rollback pair are not accepted by this macOS run.
Earlier external tester successes concern older payloads and are not transferred to this build.

## Shortest remaining release path and approvals

1. **Source approval:** review the cumulative changes, commit on the topic branch, push a PR and
   run CI for that exact commit with `visual-authoring`. No direct main push or force push.
   The parallel watcher failure above remains a CI gate, not an ignored failure.
2. **Channel/version/support decision:** the current publisher accepts only unsigned
   `X.Y.Z-preview.N`, not stable versions. Choose another explicitly labeled preview, or approve
   a separately reviewed unsigned stable-channel policy and tooling change. Do not rename a
   development archive or claim Apple signing. Agree on macOS-only versus additional platforms;
   additional image-authoring platforms need their own build/install evidence.
3. **Exact CI artifact acceptance:** verify source/receipt/hash/notices, install in isolation,
   inspect the real Codex UI and server restart, and exercise the previous/new Plugin pair.
   Preserve format-5 records; old Plugin downgrade is not a storage-format downgrade.
   This does not require replacing the maintainer's existing Plugin or migrating user projects.
4. **Publication approval:** after those gates, approve the precise version, immutable tag,
   artifact hashes, release/marketplace destinations and PR merge. Then publish and verify an
   unauthenticated public download/install path. Public announcement wording is a separate action.

Project migration remains two explicit per-project approvals: backup/recovery verification,
then application of the verified plan. Release/installation consent never replaces them.
No commit, push, remote CI, public tag, release, marketplace update or user migration occurred here.

## PR preparation follow-up

The maintainer subsequently authorized commit, PR push and CI, but not publication or installation
replacement. The 129-file checkpoint matched byte-for-byte before this follow-up note.
The unchanged parallel visual-authoring library suite passed 505 tests (2 existing ignored
child-process harnesses) on recheck. The prior 2-versus-1 observation failure is intermittent;
its root cause is not established and it has not been hidden by changing the assertion.

The reviewed base commit is the head of existing Windows PR #15, not current main. Submit this
increment as a stacked PR against that branch so its 27 prerequisite commits are not presented
as new image-report changes. Do not merge or rewrite the prerequisite branches. Dispatch default
and visual-authoring macOS workflows explicitly for the new head: the pull-request trigger is
main-only, and default-only CI does not validate the image-writing contract.

## Clean CI and real Host acceptance follow-up

The preceding development-only limits are historical. Source `e51e7018907e1324af973b86fee1de7ff0161e67`
is now committed in [PR #16](https://github.com/hyun06000/gil/pull/16). Both exact-head runs passed:
[default](https://github.com/hyun06000/gil/actions/runs/38520372682) and
[visual-authoring](https://github.com/hyun06000/gil/actions/runs/38520375664).
The unchanged parallel watcher test passed on CI; its earlier intermittent failure remains recorded
above and its root cause is not claimed fixed. PR #16 is stacked, so its empty PR check rollup
must not be described as an automatically triggered main-branch check pass.

Downloaded visual artifact 11873283111 has inner archive SHA-256
`034807aa266076b2afd58fdb729fd68f09302fa705ee2e46aa5daaab5439e287`.
Its exact installed executable, not a local rebuild, served the real Codex Monitor. Cycle rectangle
selection, shared image/caption/provenance and 1200×700 enlargement/preview return were observed.
The user explicitly confirmed image and chat together. CSP remained unchanged; the existing
verified-byte data PNG fallback displayed when Blob PNG was blocked.

Isolated official Plugin replacement old→CI→old→CI returned 17→24→17→24 tools. Synthetic
format-4/5 trees, shared-image references and reports were preserved across replacement and
process restart. Production Plugin bytes were unchanged. Full Desktop quit/relaunch is not
claimed from a server reconnect. No real project was migrated.

### Next preview decision and publication boundary

The maintainer chose **another preview before stable release**, not a new unsigned stable policy.
`0.2.1-preview.4` is a local staging candidate (no existing marketplace tag with that name at
the check). Scope remains macOS Apple Silicon / Codex / visual-authoring; no new Windows or
Claude image-authoring acceptance is inferred. Developer ID/notarization remain absent.
Clean CI Core/Skill/UI bytes are preserved; only approved packaging metadata and notices are
versioned by the existing preview staging tool. `publishable: false` remains intact.

Before requesting publication approval:

1. Verify preview.4 staging inventory, native smoke and explicit preview.3↔preview.4 installation
   pair in isolation. Format-5 refusal by preview.3 is expected, not data loss or permission to
   downgrade storage. Migration still needs separate backup and apply consent.
2. Review the stacked source chain: PR #12 → #13 → #15 → #16; PR #14 is a separate main-based
   Gitless installer fix. Do not silently merge prerequisites or claim main contains this code.
   Confirm required checks against the intended integration head after approved merges.
3. Present the fixed preview version, source head, hashes, release/marketplace destinations,
   support limits and rollback warning for publication and merge approval. Do not overwrite tags.
4. After approval only, publish the verified bytes and validate anonymous download and official
   installation. Release creation, marketplace update, announcement and stable support are not
   completed by this local acceptance.

Existing user installation and project records remain out of scope. These documentation updates
do not imply another commit, push, merge or public release.

### Preview.4 local staging and version-pair result

The preview staging tool passed inventory verification, archive round-trip and 24-tool native
smoke. Candidate archive SHA-256:
`33e12319cb19224fe69df12ed4f857d8a36b006ed4f006f6e6eb542ae671dcd3`.
The original CI Core and Skill file hashes/modes are unchanged; embedded UI remains the digest
above. A new isolated official Plugin profile passed preview.3 → preview.4 → preview.3 →
preview.4 with tool counts 17 → 24 → 17 → 24. Synthetic format-4/5 file hashes and modes,
Verify/Cycle image references and reports, production Plugin inventory and current user config
were unchanged. This is an explicit approved replacement test, **not automatic update permission**.
The compatibility change intentionally requires review. No storage downgrade or migration occurred.
Desktop UI was observed with the same CI payload before the packaging version changed; no claim
is made that a published preview.4 has already been downloaded or installed by a user.
