# Clean marketplace route — 2026-09-29

Status: **private route accepted; public access and publication not performed**.
This supersedes the active-route/privacy-decision portions of the earlier
[remote staging](REMOTE-STAGING-CHECKPOINT-20260928.md) and
[Host install](HOST-INSTALL-CHECKPOINT-20260929.md) checkpoints. Their historical results remain valid.

## Repository and immutable provenance

The maintainer chose a new independent publication history instead of disclosing the old distribution
commit's personal author/committer email. `hyun06000/gil-distribution` remains private as preserved
audit/recovery history. No history rewrite, force push, ref deletion or copying of that ancestry occurred.
Source remains `hyun06000/gil`; the active built-dist repository is **`hyun06000/gil-marketplace`**.
Both active repositories remain private. This is not a public installation announcement.

- [Marketplace PR #1](https://github.com/hyun06000/gil-marketplace/pull/1): reviewed head
  `98d736a9d4580434d8e704523163120837e1a964`.
- Approved merge and installed immutable ref: `e63963db63f0bfaf11be9d7939873e7a31fe05be`.
- Reviewed head and merge tree: `191a24ab76f190c4a18ed9800804dc9b1db9fdcb`.
- [PR CI](https://github.com/hyun06000/gil-marketplace/actions/runs/36526365573) and
  [merge CI](https://github.com/hyun06000/gil-marketplace/actions/runs/36563126941): passed;
  9 history-policy + 10 integrity tests, raw reachable-history guard and pinned payload verification.
- All three reachable marketplace commits pass its no-reply metadata guard. No annotated tag exists.

The move preserved **`0.2.1-preview.1`** byte-for-byte, not a rebuild or new version:

| Evidence | SHA / identity |
|---|---|
| Original source | `da7fa6f66fefc59d21b0297ed61990ed366ef1bd` |
| Original source CI | [36405273140](https://github.com/hyun06000/gil/actions/runs/36405273140) |
| Archive SHA-256 | `063ac763eac73e9fcbd36b44fb45d48f1fc89aa8c8aea89d276579006e99f782` |
| Receipt SHA-256 | `9e70d44dfb16cd6b77232e28e34e0bbb8b402f82008690cc5d28fd2af0ee2312` |
| Native Core SHA-256 | `3572786190ec4a8ad3b703ebe5c8ffd9b170a4e6c7da1ea2776abfc12c520a47` |
| Embedded UI SHA-256 | `0aaf30e3c62d63fdc45919d01ece01c1ea2ac2464bf572e72b4a4d4b87b52688` |

## Installation and visible acceptance

After explicit approval, the official Host CLI changed the existing marketplace registration to the
new Git repository pinned to the merge above and installed/enabled the same Plugin identity/version.
The actual checkout origin, HEAD and tree, all 9 receipt payload files and all 7 installed Plugin files
matched. Configuration outside the target marketplace section stayed byte-identical. No cache hand
edit, Companion launch or unrelated Plugin change occurred.

A fresh installed native process, with child PATH limited to system tools, answered in 618 ms and
provided 17 tools and the pinned UI. It restored a previously saved scope without prepare and read its
View/detail. No Node/Cargo runtime dependency or Project write was used. A separate Host prepare/show
call returned data; afterward the user confirmed the horizontal fullscreen graph and node detail were
visible. The fresh-process check and human screen confirmation are separate evidence: this conversation
may retain an older MCP connection. A complete Host restart on this exact route is not claimed.

The existing acceptance Project (183 entries) and Monitor bindings (3 entries) retained their
tree/content/mode fingerprints before and after the switch. No Project path or private fingerprint
inventory is published here. The previous source remains available for an approved recovery; an actual
previous-version rollback pair has not been exercised.

## Final review baseline and remaining gates

At source `21b82634421464887f6dce1c4a39ba5aee93910f`, the 9/29 read-only review passed 73 packaging/compliance
tests and 19 marketplace tests. It checked source (17 commits / 335 blobs), marketplace (3 commits /
22 blobs), fetched ref coverage, 10 successful CI runs, 7 PR/issue records, 2 comments and 9 source
artifacts. Targeted privacy scans and Gitleaks reported no secret detections. This bounded review is
not a security guarantee or a scan of unobservable refs/external clones.

Source raw author/committer identities use GitHub no-reply/service addresses. One preserved Claude
co-author attribution is a vendor no-reply address: applying the stricter marketplace-only metadata
guard to source rejects it. That classified exception is not called a full source guard pass; the
attribution and guard were not changed. No personal email is copied into this checkpoint.

The review found that preview.1's shipped Skill incorrectly required Companion and overstated Cowork
as a fallback. The source correction makes fullscreen Plugin-only the accepted default and Companion
an explicitly chosen option. **It does not alter the installed preview.1.** A new candidate
`0.2.1-preview.2` must receive its own clean-source/CI/receipt evidence, distribution PR and installation
acceptance; this checkpoint is not evidence that those future steps passed.

Private main protection remained unenforced (protection/rulesets API 403); PVR intake was unverified
(404); no releases/tags existed. Public visibility, required-PR enforcement, PVR/SECURITY activation,
conduct-reporting policy, unauthenticated HTTPS installation and exact-version publication remain
separate gates/approvals. See the [publication plan](PUBLICATION-PLAN-20260929.md).

The supported first-preview scope remains Codex/macOS arm64, Developer ID unsigned and unnotarized.
Fresh-Mac validation is deferred, not passed. Windows, Intel, Claude work-Plugin rendering and new
Companion distribution are not accepted by this result; no OS security bypass or automatic update is
authorized.
