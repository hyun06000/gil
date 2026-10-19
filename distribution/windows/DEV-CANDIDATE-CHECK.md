# Windows image-authoring development candidate — isolated PC check (direct file transfer)

This is a maintainer-owned review candidate, **not** the public Windows tester preview and not a
release. It carries the `visual-authoring` runtime profile (24 tools, new image Projects stored as
format 5). Nothing here is published; the archive reaches the test PC only by direct file transfer
from the maintainer, who records the exact hashes in the candidate record for that head.

Identity (always read from `checks.json` and the maintainer's record, never assumed):

- Version: `0.2.1-dev.visual-authoring+windows.<arch>.<head12>` — the `dev.visual-authoring`
  prerelease tag is what distinguishes it from `0.2.1-preview.N` public builds.
- Marketplace: `gil-dev-windows-visual-<arch>` (never `gil-preview-windows-<arch>`).
- Archive: `gil-windows-<arch>-visual-authoring-candidate.tar.gz`; `checks.json` says
  `runtime_profile: visual-authoring`, `mcp_tools: 24`, `storage_format: 5`, `publishable: false`,
  the source head, the toolchain receipt and every payload file's SHA-256.
- x64 and ARM64 are separate archives; never substitute one for the other or run under emulation.

## Request for the checking Codex agent (short form)

You have consent to register and install this development candidate in a **standard (non-admin)
user session** and to create GIL records only in a **new, empty synthetic folder** that the user
selects. You do not have consent to replace an existing GIL installation, to change security
settings, or to touch any existing project.

1. Read-only preflight: `codex plugin list --json`, `codex plugin marketplace list --json`. If any
   GIL plugin or GIL marketplace is already registered, **stop and report it; wait for the
   maintainer's approval** before registering anything. Never remove or overwrite it yourself.
2. Native CPU: `Get-CimInstance Win32_Processor` → `Architecture` 9 = x64, 12 = ARM64. Anything
   else, or more than one distinct value: stop.
3. The archive arrives by direct transfer into a fresh user-owned folder on the local fixed NTFS
   drive (not OneDrive, network or a reparse-point path). Before extracting, verify
   `Get-FileHash -Algorithm SHA256` against the hash the maintainer gave you for that CPU; then
   compare `checks.json` (`archive_sha256`, `source`, `version`, `runtime_profile`). Mismatch: stop.
4. Extract with Windows tar from that folder using ASCII relative arguments
   (`tar.exe -xzf <archive> -C marketplace`). Check: exactly the eight payload files, no links, the
   manifest version above, `core/windows-<arch>/gil.exe`, marketplace name `gil-dev-windows-visual-<arch>`.
   Verify `gil.exe`'s SHA-256 against `checks.json`.
5. Register and install with the official commands, checking exit codes and JSON:

   ```text
   codex plugin marketplace add <absolute-marketplace-root> --json
   codex plugin add gil-companion-prototype@gil-dev-windows-visual-<arch> --json
   ```

   No `--ref` for a local root. Do not edit Codex config or caches by hand. If the CLI cannot
   install, use the app's official Plugin UI for that source; do not invent commands.
6. In a **new conversation**, confirm the connected `gil_start` description mentions image-report
   creation (format 5, Windows local fixed NTFS) and that 24 GIL tools are listed, including
   `gil_report_visuals` and the four `gil_migration_*` tools. Then ask the user to select a new empty
   synthetic folder, start GIL there, open the Monitor, and run a short interview → experiment →
   verify flow that attaches one generated PNG. Reuse that image in a later report. Fully restart
   Codex and confirm the report and image are still shown.
7. Also confirm the refusals say what the tools do: `gil_migration_check` on any folder must
   answer `unsupported_platform` on Windows (migration of existing format-4 records is not
   supported there), and `gil_start` on a network/UNC/junction path must refuse without creating
   anything. Do not work around either refusal.

Security boundaries: no Defender/SmartScreen changes, no `Unblock-File`, no execution-policy
change, no administrator elevation, no exclusions. An OS or Host block is a test result; stop
and report it. Record separately: install, tool inventory, start, image attachment, reuse,
Monitor display with chat, restart preservation, each refusal, and any error text (redacted).

The `.gil` of the new Project is created owner-only (current user + SYSTEM). If the chosen folder
is shared with other accounts, that is the intended protection; it is not a sign of failure.
