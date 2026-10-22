# Windows image-authoring development candidate — isolated PC check (direct file transfer)

This is a maintainer-owned review candidate, **not** the public Windows tester preview and not a
release. It carries the `visual-authoring` runtime profile (25 tools on Windows, new image Projects stored as
format 5). Nothing here is published; the archive reaches the test PC only by direct file transfer
from the maintainer, who records the exact hashes in the candidate record for that head.

Identity (always read from `checks.json` and the maintainer's record, never assumed):

- Version: `0.2.1-dev.visual-authoring+windows.<arch>.<head12>` — the `dev.visual-authoring`
  prerelease tag is what distinguishes it from `0.2.1-preview.N` public builds.
- Marketplace: `gil-dev-windows-visual-<arch>` (never `gil-preview-windows-<arch>`).
- Archive: `gil-windows-<arch>-visual-authoring-candidate.tar.gz`; `checks.json` says
  `runtime_profile: visual-authoring`, `mcp_tools: 25`, `storage_format: 5`, `publishable: false`,
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
2. Native CPU, from read-only sources that need no administrator rights, no setting change and
   no working CIM client. Run this block as it is (PowerShell 5.1 or 7, any shell host) and use
   only its last line:

   <!-- gil-cpu-detect:begin -->
   ```powershell
   $ErrorActionPreference = 'Continue'; $src = [ordered]@{}; $notes = @()
   try { $a = @(Get-CimInstance -ClassName Win32_Processor -ErrorAction Stop | ForEach-Object { [int]$_.Architecture } | Sort-Object -Unique)
         if ($a.Count -eq 1 -and $a[0] -eq 9) { $src.cim = 'x64' } elseif ($a.Count -eq 1 -and $a[0] -eq 12) { $src.cim = 'arm64' } else { $src.cim = $null; $notes += 'cim: unsupported or mixed value' } }
   catch { $src.cim = $null; $notes += 'cim unavailable (refused or missing)' }
   try { $e = (Get-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Environment' -Name PROCESSOR_ARCHITECTURE -ErrorAction Stop).PROCESSOR_ARCHITECTURE
         if ($e -eq 'AMD64') { $src.sysenv = 'x64' } elseif ($e -eq 'ARM64') { $src.sysenv = 'arm64' } else { $src.sysenv = $null; $notes += 'sysenv: unsupported value' } }
   catch { $src.sysenv = $null; $notes += 'sysenv unavailable' }
   try { $i = (Get-ItemProperty -Path 'HKLM:\HARDWARE\DESCRIPTION\System\CentralProcessor\0' -Name Identifier -ErrorAction Stop).Identifier
         if ($i -match '^ARM') { $src.cpuid = 'arm64' } elseif ($i -match '^(Intel64|AMD64|EM64T)') { $src.cpuid = 'x64' } else { $src.cpuid = $null; $notes += 'cpuid: unsupported value' } }
   catch { $src.cpuid = $null; $notes += 'cpuid unavailable' }
   $found = @($src.Values | Where-Object { $_ } | Sort-Object -Unique)
   'sources: ' + (($src.GetEnumerator() | ForEach-Object { "$($_.Key)=$(if ($_.Value) { $_.Value } else { 'n/a' })" }) -join ' ') + " | process: PROCESSOR_ARCHITECTURE=$($env:PROCESSOR_ARCHITECTURE) PROCESSOR_ARCHITEW6432=$($env:PROCESSOR_ARCHITEW6432) host=$($PSVersionTable.PSVersion)" + $(if ($notes) { ' | ' + ($notes -join '; ') } else { '' })
   if ($found.Count -eq 1) { "NATIVE CPU: $($found[0])" } elseif ($found.Count -gt 1) { "CONTRADICTION: $($found -join ',') -> stop and ask" } else { 'UNDETERMINED -> stop and ask' }
   ```
   <!-- gil-cpu-detect:end -->

   `NATIVE CPU: x64` or `NATIVE CPU: arm64` decides, even when the `process:` part says
   `AMD64` or `x86` (that is only this shell, which may run under emulation on an ARM64 PC).
   `cim unavailable` alone is not a reason to stop: the registry sources answer without CIM.
   On `CONTRADICTION` or `UNDETERMINED`, do not guess and do not run anything as administrator;
   ask the person exactly one thing — whether Windows Settings › System › About › "System type"
   says "ARM-based processor" — and continue only with their answer.
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
   creation (format 5, Windows local fixed NTFS) and that 25 GIL tools are listed, including
   `gil_report_visuals`, `gil_prepare_workspace` and the four `gil_migration_*` tools. Then ask the user to select a new empty
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
