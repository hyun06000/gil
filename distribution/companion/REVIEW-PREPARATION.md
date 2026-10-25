# Windows Core + Companion review preparation — 2026-10-26

Local implementation, not a built/published Windows installer. No existing GIL installation or
Windows PC was accessed. No commit, push or CI dispatch is part of this preparation turn.

## Inputs and outputs

- `companion-review.yml`: scoped PR/manual Windows x64/ARM64 build, separate Tauri review identifier,
  actual Core with runtime-testbed feature, existing feature-gated testbed driver.
- `package-review.mjs`: CPU/hash/build identity validation, adjacent receipt, portable cohort,
  license inventory, checksums. Never merges into an existing output directory.
- `INSTALL-review.md`: double-click plain START-REVIEW.cmd, approve one new Project, observe tray,
  close this MCP connection, observe tray disappearance. No PowerShell scripting by user.
- `companion_review.rs`: direct actual Core MCP client, evidence, bounded automatic waits, no kill.
  Human prompts deliberately wait for input; no model calls or background update checks.
- Review app settings are under the new run directory. Its separate app ID prevents installed
  Companion single-instance routing. Explicit review mode is required before legacy handshake setup.
- Core's review namespace override is feature-gated; normal products do not read it.

## Notice gate (missing originals resolved, not waived)

Local x64 dependency scan completed but notice assembly refused because these locked packages omit
license text from their crate archives and initially had no pinned upstream fallback:

- alloc-stdlib 0.2.4
- selectors 0.36.1
- tauri-plugin 2.6.3
- unic-char-property, unic-char-range, unic-common, unic-ucd-ident, unic-ucd-version: 0.9.0
- webview2-com 0.38.2, webview2-com-macros 0.8.1, webview2-com-sys 0.38.2

This meant missing distributable original notices in this assembly, **not** absence of a license grant.
Exact-revision root license/copyright originals were retrieved into `legal/` with source URLs, crate
VCS identities and full digests; selectors retains its original source header plus the official MPL
2.0 text. No SPDX-label reconstruction or package exclusion was used. The retrieval script is
maintainer-only; packaging and user execution do not fetch legal texts. Both x64 and ARM64 offline
inventory assemblies subsequently succeeded. Any future missing original still emits LEGAL-GAPS.json
and stops packaging. Core-only notices are retained separately and do not stand in for Tauri notices.
This is technical provenance/completeness evidence, not automatic legal clearance.

## Remaining acceptance

Windows native compilation of the new review adapter/driver, final
artifact hashes, real tray/GUI and process-handle exit confirmation are not yet executed. Mac
synthetic Companion/MCP tests do not substitute for them. No release URL or hash is invented.
The user will approve publication only after receiving actual preparation/CI results.

## Local validation

- Actual Core MCP + synthetic adjacent Companion in isolated namespace: 4/4 (includes new review path).
- Driver RPC parsing/evidence: 2/2. No GUI executed by these tests.
- Companion unit suite: 96/96 in permitted isolated socket/watcher environment. First sandbox run
  was 92/96 (socket EPERM and three watcher deadlines); no safety checks were weakened for the rerun.
- Core driver and discovery test x64 Windows cross-check succeeded; native Windows is not executed.
- Alternate Tauri review configuration check succeeded on Mac without launching the GUI.
- Packaging: 5/5, including full offline assembly with fake PE fixtures (never executable user artifacts),
  exact hashes, notices, wrong CPU/identity rejection and existing-output preservation.
- Existing packaging/compliance/Windows package regression: 105 passed, 1 skipped.
- Workflow YAML parsing and whitespace checks passed. Existing Mac actions.rs unused_mut remains.
- Packaging fixture first encountered the Mac temp path's symlink alias; the fixture now resolves
  its own temp parent before creation. The production no-symlink check was not weakened.

The review path deliberately does not register with Codex, exercise Desktop restricted tokens,
perform an update, replace a Plugin, or provide a production installer. Those remain separate gates.

## Follow-up CI preparation

The next approved step includes commit/push and Windows CI artifact verification, not publication.
The new workflow runs on relevant PR changes as well as manual dispatch; checkout, build receipt and
artifact name all bind to the exact PR head rather than a synthetic merge SHA. Package fixture
assembly uses the runner's CPU so ARM64 does not require unfetched x64 dependencies in offline mode.
