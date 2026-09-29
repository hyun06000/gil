# Unsigned preview publication plan — 2026-09-29

Status: **preparation only; both repositories remain private**. This is not an active installation URL
or announcement. Candidate bytes/version are unchanged; runtime, UI, Project storage and installed Plugin
are not modified by this documentation work.

## Ready evidence

- [Private Git provenance](REMOTE-STAGING-CHECKPOINT-20260928.md) fixes the original source commit,
  CI, receipt/archive hashes and distribution ref for `0.2.1-preview.1`.
- [Host acceptance](HOST-INSTALL-CHECKPOINT-20260929.md) records official replacement, uninstall,
  reinstall, saved-binding reads, Project preservation and the maintainer's visible Monitor approval.
- Root MIT and original third-party notices remain intact. The candidate includes native Core/UI
  coverage and Rust standard-library notices, not a claim for Companion or every target.

## Privacy decision before public visibility

The distribution payload commit's author and committer metadata contain a personal email address.
The current source repository uses GitHub no-reply/service identities. Payload files are a separate
surface: a clean content scan does not make Git author metadata private.
The address itself is deliberately not repeated here. No history was rewritten, no ref was forced,
and no repository was deleted or made public.

The maintainer must either explicitly accept publishing that metadata or authorize a separately
reviewed cleanup plan. Prefer retaining the present private audit history and preparing a clean
publication history with a no-reply identity if disclosure is unwanted. That may change Git refs and
repository/install routing, so it requires renewed provenance and official installation verification;
it must not silently break an already pinned installation. A new ordinary commit does not remove an
email from its ancestors, and a force-push alone does not guarantee removal from retained PR refs.

## Ordered publication gates

1. Resolve the metadata decision and review the [open-source readiness gates](../../spec/GIL_Open_Source_Readiness_v0.1.md),
   including the draft conduct policy's private-contact limitation. Do not invent an email or SLA.
2. Obtain explicit approval for repository visibility, required-PR protection and GitHub private
   vulnerability reporting (PVR). Keep the old development repository private. At the approved source
   transition, apply and read back protection/PVR; only then publish an active SECURITY.md and verified
   reporting link. A planned endpoint or inaccessible private-repository setting is not an open inbox.
3. Through a reviewed PR, finalize user-facing installation/recovery instructions for the exact
   distribution identity and immutable ref. Verify unauthenticated HTTPS discovery/download after
   the approved visibility transition. Private SSH success is not a substitute.
4. Verify source/CI/receipt/Core/UI/legal-file correspondence again. Do not rebuild different bytes
   under `0.2.1-preview.1`, relabel the candidate as signed, or flip `publishable:false` by hand.
   If packaging/content must change, create and review a new candidate version.
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
