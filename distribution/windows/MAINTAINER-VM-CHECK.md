# Windows ARM64: maintainer-owned VM installation check

This is an internal acceptance procedure for the maintainer's own Windows ARM64 VM,
not a public tester release, runtime redistribution clearance, or supported Windows claim.
Do not forward this procedure as the nondeveloper installation channel. The CI artifact
expires after seven days; public distribution still follows RELEASE-READINESS.md.
No new payload, release, catalog publication, license acceptance or security exception is created.

## Exact candidate

- Source: c762634824151ae7a12d52bdcbc6676d9f550dc6.
- [Successful native CI](https://github.com/hyun06000/gil/actions/runs/37542348090).
- Artifact: gil-windows-arm64-REVIEW-c762634824151ae7a12d52bdcbc6676d9f550dc6.
- Artifact ID: 11449352143. Download using the maintainer's GitHub browser login.
  Never give credentials or session cookies to an agent.
- Inner archive: gil-windows-arm64-candidate.tar.gz, 2,519,040 bytes.
- SHA-256: 437f0a8db18ebe7efef660f806fbaf505bb17a540e953e8fe9bea1e8e55abf71.
- Plugin: gil-companion-prototype@gil-preview-windows-arm64.
- Version: 0.2.1-preview.3+windows.arm64.c76263482415.
- Executable: plugins/gil-companion-prototype/core/windows-arm64/gil.exe.
- Executable SHA-256: 6d4b958c98491822e48f6457fa2756f42688d9553c5b30a3b4864b3d0c541ed9.

The downloaded archive and all eight payload file hashes were independently verified on Mac;
PE machine ARM64 and system-only import checks passed. No Windows executable was run on Mac.
The first inspection command exceeded Node's default stdout buffer; rerunning with a bounded
32 MB buffer succeeded. This was an inspection harness limit, not a changed candidate.
CI already executed the extracted binary natively on Windows ARM64. Codex installation/rendering
on an ordinary VM is still untested.

## Installation check

Explain that this unsigned native plugin can read/write the selected Project when its tools
are invoked and store local Monitor bindings. The Monitor is read-only. Obtain explicit consent
before installation. Only the maintainer-owned VM is in scope.

1. In the VM browser, open the CI link and download the exact ARM64 artifact. Extract GitHub's
   outer ZIP into a fresh folder with Windows Explorer. Select that folder in Codex.
2. Inspect installed GIL entries and marketplace sources first. Do not replace an existing
   registration or installation without separate approval. Never use the Mac catalog.
3. Hash the inner tar.gz with Get-FileHash SHA256 against the literal value above **before**
   extracting it. Do not trust a co-downloaded checks.json or SHA256SUMS as the sole trust root.
4. Copy the verified archive into a fresh local staging folder. Keep tar arguments ASCII:
   run tar from that folder with relative filenames and a relative fresh extraction directory.
   Windows tar previously corrupted Unicode command-line paths; do not use a Unicode absolute -C.
5. Verify the executable hash, exact manifest version, ARM64 command and catalog identity.
   Refuse unexpected files or links. Keep all license/notice files.
6. Place the marketplace tree in a new, persistent per-user local directory outside OneDrive,
   projects and temporary directories; verify local fixed NTFS and no reparse-point ancestors.
   Refuse an existing destination; never overwrite. Keep this source directory after installation.
7. Use official management, after checking the installed CLI help:

       codex plugin marketplace add ABSOLUTE_LOCAL_MARKETPLACE_ROOT --json
       codex plugin add gil-companion-prototype@gil-preview-windows-arm64 --json

   The root contains .agents/plugins/marketplace.json and plugins/. Do not supply --ref for a
   local source. Do not directly edit Codex config or installed caches. If the installed CLI
   cannot install, use the app's official Plugin UI; do not invent alternate commands.
8. Verify installed source/version and report installation only. Do not start a Project.
   A new chat may be needed for tools; an app restart is not presumed necessary or sufficient.
   Stop on any OS/Host block. No Unblock-File, execution-policy changes, Defender/SmartScreen
   exceptions, administrator escalation, Git/Node/Cargo installation or Companion launch.

[Official local-marketplace documentation](https://developers.openai.com/plugins/build/plugins#add-a-marketplace-from-the-cli)
supports local roots. The add/install command syntax was also read from Codex CLI 0.160.1;
Windows execution is precisely the acceptance this procedure tests, not a preclaimed success.

After installation, select a separate new empty local NTFS folder and request a calculator
project with the Monitor. Record fullscreen with chat, details, live updates, revisit and
restart separately. Preserve records; do not equate successful tool output with visible UI.

## Remaining external-release gate

The [Microsoft redistribution list](https://learn.microsoft.com/en-us/visualstudio/releases/2026/redistribution)
conditions redistribution on applicable license terms and excludes preview components.
The selected compiler is non-preview, but that alone does not settle the applicable hosted-build
entitlement, SDK/static runtime terms or recipient obligations. Keep publishable:false and
the runtime-review gate pending. Internal VM evidence cannot waive these conditions.
