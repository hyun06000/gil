# Windows tester channels — release gates

2026-09-30: maintainer approved a limited unsigned tester channel and publication after redistribution
and installation-route checks. This is not stable support or successful ordinary-user acceptance.
The existing Mac marketplace/default branch and installed plugin must remain unchanged.

2026-10-07: native Windows ARM64 VM validation now takes priority; physical Intel/AMD x64
acceptance is deferred, not removed. Both architectures use the same functional/security gates.
ARM64 review packaging is not itself publication approval or successful VM installation.

## Runtime review

The previous candidate was built on GitHub's `windows-2025-vs2026` image,
version `20260925.250.1` (run 36690752815). Its receipt did not identify the selected MSVC libraries.
Do not infer the selected compiler from the image's inventory alone or apply VS 2022 terms by habit.

`toolchain.ps1` now selects a complete non-prerelease MSVC installation, initializes its native architecture build
environment and explicitly passes that link.exe to Cargo. The receipt records VS/MSVC/SDK versions
and SHA-256 for link.exe, libcmt.lib, libvcruntime.lib and libucrt.lib, without machine paths.
This identifies selected inputs; hashes are not a redistribution license or a link map.

First provenance run 36692696766 passed native tests but failed before release build: nested cmd
quoting broke the spaced Visual Studio path. Use Microsoft's Launch-VsDevShell.ps1 directly;
do not remove the explicit toolchain selection gate to get a green run.

Official sources reviewed (2026-09-30):

- [Deployment](https://learn.microsoft.com/en-us/cpp/windows/deployment-in-visual-cpp?view=msvc-170):
  static CRT is a deployment method; runtime security fixes require rebuilding/redeploying the app.
- [VS licensing guidance](https://www.microsoft.com/licensing/guidance/Visual-Studio): identified
  distributable .lib code is linked into the application, not shipped as standalone libraries.
- [VS 2026 distributable list](https://learn.microsoft.com/en-us/visualstudio/releases/2026/redistribution):
  rights depend on the applicable licensed product; preview components are excluded.
- [VS 2026 development license](https://visualstudio.microsoft.com/license-terms/vs2026-ga-pro-enterprise/):
  distributable-code conditions and restrictions apply separately from GIL's MIT source license.
  The runtime end-user license alone is not the developer's redistribution grant.

Pending: review the actual selected toolchain's applicable terms (including Windows SDK/UCRT),
required recipient terms/notices and hosted build entitlement. Preserve Microsoft rights; do not
label every byte of the linked executable MIT. Cargo notice coverage does not settle these items.
This is engineering release review, not a legal certification.

## Publication and acceptance

- Keep the candidate receipt `publishable: false` until the preceding review is resolved and a
  separate publication record identifies the approved bytes, version, immutable ref and channel.
- Use an isolated Windows catalog/ref, not the Mac default catalog. Verify anonymous HTTPS access
  and the official Codex installation route; CI's seven-day artifact is not the tester channel.
- No Node, Cargo, administrator rights, Companion or security-policy changes for testers.
- Verify native ARM64 on Win-test first; Intel/AMD x64 remains a separate later acceptance. Use a new local fixed NTFS folder,
  outside OneDrive/junction/network paths. Do not use valuable existing projects for first acceptance.
- Tester checks: install, start + Monitor, fullscreen with chat, node details, live updates, restart,
  disable/re-enable and uninstall/reinstall preserving Project records. Separate each observation.
- If OS/Host blocks installation, stop and record the message. Do not suggest Defender/SmartScreen
  exclusions or execution-policy changes. Do not claim success from tool responses alone.

No tester-ready installation prompt is issued until the fixed public source and hash are verified.

## Provenance checkpoint

Source `0c2e8c31d32b59a18d6883a646beaf57a3663aa2` passed
[Windows candidate CI 36693181563](https://github.com/hyun06000/gil/actions/runs/36693181563).
The same-source Mac and Windows PR checks passed. Local packaging regression: 75 Mac/common
checks plus 7 Windows checks, all passed. No Core source changed in this provenance follow-up.

The downloaded archive passed SHA-256 verification:
`4466ec503561be93fe4167492f9a8952f9de7652a1c9facce700dab9a505cefc`.
Selected VS Enterprise `18.10.12210.168`, MSVC `14.51.36231`, SDK `10.0.26100.0`,
runner `win25-vs2026` / `20260922.246.2`, non-prerelease. The immediately preceding passing
run used a different VS/runner patch; this is why each artifact must carry its own provenance.
The receipt includes the four input file hashes, archive inventory and native smoke evidence.

Runtime terms/recipient notices remain unresolved; the receipt correctly remains `publishable: false`.
No marketplace entry, release or user installation was changed. The unsigned policy approval is
recorded, but it is not a substitute for this review. [Tester checks](TESTER-CHECK.md) are a draft,
not an installation invitation. No further unsigned-policy approval is needed to continue the review.
