# Unsigned preview publication plan — 2026-09-29

Historical plan: the following describes **2026-09-29 preparation**, not the current repository state.
For public HTTPS installation, enforced settings and preview.2 evidence, see the
[2026-09-30 checkpoint](PREVIEW-2-PUBLICATION-20260930.md).

Status at that time: **preparation only; source and active marketplace repositories remain private**. This is not a
public installation URL or announcement. Installed preview.1 remains unchanged. The corrected Skill
requires a new preview.2 candidate, separate CI/provenance and installation acceptance before publication.

## Ready evidence

- [Private Git provenance](REMOTE-STAGING-CHECKPOINT-20260928.md) fixes the original source commit,
  CI, receipt/archive hashes and distribution ref for `0.2.1-preview.1`.
- [Host acceptance](HOST-INSTALL-CHECKPOINT-20260929.md) records official replacement, uninstall,
  reinstall, saved-binding reads, Project preservation and the maintainer's visible Monitor approval.
- [Clean marketplace acceptance](CLEAN-MARKETPLACE-CHECKPOINT-20260929.md) records the independent
  `gil-marketplace` history, merged/pinned ref, official source switch, same payload and visible acceptance.
- Root MIT and original third-party notices remain intact. The candidate includes native Core/UI
  coverage and Rust standard-library notices, not a claim for Companion or every target.

## Privacy decision — resolved for the active route

The old `gil-distribution` payload history includes a personal author/committer email and stays private.
The maintainer approved an independent **`hyun06000/gil-marketplace`** publication history, PR #1 merge and
official installation-source switch. All three reachable marketplace commits passed the no-reply guard;
the installed ref is `e63963db63f0bfaf11be9d7939873e7a31fe05be`. Payload bytes stayed preview.1.
The source repository remains `hyun06000/gil`. No old ancestry was copied, rewritten or force-pushed;
no personal address is repeated here. The original repository is preserved for private audit/recovery,
not queued for public visibility. New candidates must repeat provenance/history checks on their new refs.

## Ordered publication gates

1. Review the new Skill candidate and remaining [open-source readiness gates](../../spec/GIL_Open_Source_Readiness_v0.1.md),
   including the draft conduct policy's private-contact limitation. The active-route metadata decision
   above is complete, not a waiver for new history. Do not invent an email or SLA.
2. Obtain explicit approval for repository visibility, required-PR protection and GitHub private
   vulnerability reporting (PVR). Scope visibility to `gil` and `gil-marketplace`; keep the old development
   repository and `gil-distribution` private. At the approved source
   transition, apply and read back protection/PVR; only then publish an active SECURITY.md and verified
   reporting link. A planned endpoint or inaccessible private-repository setting is not an open inbox.
3. Through a reviewed PR, finalize user-facing installation/recovery instructions for the exact
   distribution identity and immutable ref. Verify unauthenticated HTTPS discovery/download after
   the approved visibility transition. Private SSH success is not a substitute.
4. Verify source/CI/receipt/Core/UI/legal-file correspondence again. Do not rebuild different bytes
   under `0.2.1-preview.1`, relabel the candidate as signed, or flip `publishable:false` by hand.
   The Skill correction therefore uses a new `0.2.1-preview.2` candidate; keep preview.1 intact and
   require a reviewed distribution PR plus official installation/visible acceptance for the new version.
5. Obtain/confirm approval for the exact preview publication surface and publish only those reviewed
   bytes, with the unsigned and fresh-Mac limitations below. A custom Git marketplace is not universal
   Plugins Directory approval. Announce only a working, verified public route.
6. Stop on hash/provenance mismatch, missing protection/PVR, OS blocking, public access failure or
   missing visible Monitor. Preserve evidence and the last working installation; do not silently retry,
   weaken approvals or remove Project records to make the test pass.

## User-facing copy, to activate with the verified route

> GIL is an opt-in preview for Codex on Apple Silicon Macs. It is not Apple Developer ID signed or
> notarized; installation on a fresh Mac has not yet been tested. macOS may block execution. If it does,
> stop and report the problem without disabling security checks. Install the GIL Plugin, select your
> working folder, then ask GIL to start or show your project. Node, Cargo and a separate Companion app
> are not required for this route. The Monitor is read-only; approved Agent actions can change GIL records.

For a new folder: “지금 폴더에서 GIL 프로젝트를 시작해 줘.”
For an existing Project: “기록을 바꾸지 말고 GIL Monitor를 열어 줘.”
Success means a visible horizontal graph/fullscreen Monitor and usable node detail, not merely a tool
success response. Unsupported surfaces should offer an explanation/fallback choice, not launch Companion
without the user's choice. Automatic updates are not promised.

Use the Host's official Plugin uninstall/reinstall controls. Removing the Plugin is not an instruction
to delete the Project's `.gil` records. Preserve the last verified source/version and Project when
recovering; `gil restore` is not a Plugin downgrade command. The real previous-version rollback pair
remains untested. No cache hand-editing, `curl | sh` without verification, quarantine removal or Gatekeeper
disablement is part of the user installation flow.

Supporting plans: [release gates](RELEASE-macos.md),
[security reporting](../../spec/GIL_Security_Reporting_Plan_v0.1.md),
[repository governance](../compliance/REPOSITORY-GOVERNANCE-20260928.md).
