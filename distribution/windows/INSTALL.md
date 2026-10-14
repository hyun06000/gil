# Windows unsigned tester preview — f0ecfbd

This is an opt-in test build, not stable Windows support. Native x64 and ARM64 CI passed.
Ordinary-user Codex installation and visible fullscreen are the acceptance test, not already proved.
No Authenticode signature is supplied. Stop on OS security warnings; never bypass them.

Release: https://github.com/hyun06000/gil/releases/tag/windows-preview-f0ecfbd

## Request for the installing Codex agent

Use a local Windows task with PowerShell and the user's normal Codex installation. You have consent
to install this preview, not to overwrite an existing installation or modify existing projects.
Explain briefly that GIL reads/writes the project the user selects and stores its history in `.gil`.
The native Plugin includes the Monitor; Companion, Node, npm, Cargo, Git and administrator rights
are not needed. This source is a local marketplace downloaded from an official project release,
not a listing in the universal plugin directory.

1. Read-only preflight: check PowerShell, `codex plugin --help`, `codex plugin add --help`,
   `codex plugin marketplace add --help`, and the Windows system `tar.exe`. Use the existing Codex
   CLI; do not install development tools. If tools/network access are denied, request the normal
   task permission or explain the blocker. Never alter security policy.
2. Determine native CPU using `Get-CimInstance Win32_Processor` and its `Architecture` field.
   Exactly one distinct value 9 means x64; 12 means ARM64. Do not use the shell process's bitness.
   Missing, conflicting or other values: stop. Do not ask the person to research CPU details.
3. Inspect existing Plugin and marketplace state through official commands. If any GIL install
   exists, report its source/version and ask before replacing or registering a conflicting source.
   Never edit registry/config/cache files manually or delete an old install directory.
4. Choose the matching archive below. Download via HTTPS only into a fresh, user-owned directory
   outside the project, OneDrive, network shares and reparse-point paths. Check available disk space
   first (allow at least 150 MiB). Use a GUID/new name; never reuse or overwrite an existing directory.
   Retain the prepared marketplace at a persistent user-local path, not a temporary cleanup location.
5. Before extraction, verify `Get-FileHash -Algorithm SHA256` against the exact digest below.
   Mismatch: stop, do not execute or register anything. Inspect tar entries: no absolute paths,
   `..`, drive/ADS colon, backslashes, links or special files. Expected file list is below; only their
   parent directories may occur. Stop on extras or duplicates. Hash validation is mandatory even
   if a downloaded checksum file agrees. Do not accept a hash from an unrelated source.
6. Extract only the validated archive into a newly created empty directory. To preserve non-ASCII
   Windows user paths, set the working directory to the staging directory and pass ASCII relative
   arguments to Windows tar, e.g. `tar.exe -xzf package.tar.gz -C marketplace`. Check its exit code.
   Check the resulting file allowlist, no reparse points, manifest version, native executable path
   and marketplace name before registering anything.
7. Run official management commands, checking each exit code and JSON result:

   ```text
   codex plugin marketplace add <absolute-prepared-marketplace-root> --json
   codex plugin add gil-companion-prototype@gil-preview-windows-<architecture> --json
   ```

   Do not pass `--ref` for a local directory. Keep that directory for future marketplace access.
   If this client's CLI does not support installation, use its official Plugin UI for that registered
   source; do not invent commands or patch cache files. Report incomplete installation accurately.
8. Verify installed source/version and GIL tool availability. New tools may require a new Codex
   conversation; ask for that only when necessary. Never claim the tools attached from a CLI success
   response alone. Restart the app only if a new conversation does not load the installed tools.
9. After tools are available, ask the user to select a new empty local test folder in Codex. Never
   guess a project root or initialize an existing project. Start GIL, request the Monitor, and begin
   an interview for a simple calculator. Ask the user to confirm the graph and chat are actually
   visible together; successful tool data is not proof of fullscreen rendering.

## Fixed packages (never substitute Mac or an emulated architecture)

Base URL: `https://github.com/hyun06000/gil/releases/download/windows-preview-f0ecfbd/`

| Native CPU | File | SHA-256 |
| --- | --- | --- |
| x64 | `gil-windows-x64-candidate.tar.gz` | `3d4e374cf4cf7ec735ca5305124b94cb5f2b1e85529d295da829a6ed1d1221ad` |
| ARM64 | `gil-windows-arm64-candidate.tar.gz` | `5cc6f3db2015062e08397d5d1b839d74c188319fdb0456b782ec2d2680c1c465` |

Version: `0.2.1-preview.3+windows.<architecture>.f0ecfbd1ddff`.
Marketplace: `gil-preview-windows-<architecture>`.
Build source: `f0ecfbd1ddffd883a78fb82aba05c9716db084f6`.
Build evidence: https://github.com/hyun06000/gil/actions/runs/37552996947

Exactly eight regular payload files (tar may prefix paths with `./`):

```text
.agents/plugins/marketplace.json
plugins/gil-companion-prototype/.codex-plugin/plugin.json
plugins/gil-companion-prototype/core/windows-<architecture>/gil.exe
plugins/gil-companion-prototype/skills/gil-companion/SKILL.md
plugins/gil-companion-prototype/LICENSE
plugins/gil-companion-prototype/THIRD-PARTY-NOTICES.txt
plugins/gil-companion-prototype/THIRD-PARTY-NOTICES.json
plugins/gil-companion-prototype/RUST-STDLIB-NOTICES.html
```

## Security boundaries

Do not disable Defender/SmartScreen, change execution policy, clear download protection with
Unblock-File, elevate to administrator, or add exclusions. OS/Host refusal is a test result, not
permission to bypass protection. No automatic Companion fallback. No telemetry or personal logs
are required from the tester. Preserve all third-party notices and project history.

## Report back

Ask only whether installation worked, graph+chat appeared, clicking a node opened its details,
and the graph updated during work. Error text or a redacted screenshot is sufficient.
Restart and uninstall/reinstall preservation tests can follow after first-run success.
