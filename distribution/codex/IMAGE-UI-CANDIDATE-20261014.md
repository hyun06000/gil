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
