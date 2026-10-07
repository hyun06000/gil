# Windows preview security checkpoint — 2026-10-07

This is a scoped engineering check, not a guarantee that the application is vulnerability-free.
No individual support approval is required by this checklist. Preserve third-party notices.

## Dependency findings and remediation

`npm audit` initially identified GHSA-6qxp-vccf-f47h in the UI build's MCP client.
The development-only Node bridge additionally reported fast-uri, ip-address and proxy-addr findings.
Updated only the affected dependency families within the existing manifest ranges:

- UI MCP client/core: 2.0.0 → 2.3.1.
- Development bridge SDK: 1.30.0 → 1.32.1.
- fast-uri: 3.1.7 → 3.1.8; ip-address: 10.7.0 → 10.7.3; proxy-addr: 2.0.7 → 2.0.8.

Both npm lockfiles now return zero audit findings. This is date-specific registry evidence,
not proof about unreported vulnerabilities. The Node bridge and node_modules are not shipped
in native Windows packages. No claim is made that a published GIL user was exploited.

The [MCP advisory](https://github.com/advisories/GHSA-6qxp-vccf-f47h) affects OAuth clients over
HTTP; it explicitly excludes stdio clients. GIL Monitor connects through the Host App bridge,
not an OAuth credential provider. The esbuild output-input graph has no contributing SDK OAuth
authentication implementation. A regression test checks this boundary and minimum patched versions.

The UI was rebuilt. Client/core license metadata changes to Apache-2.0; their complete legal text
hash remains unchanged (`0382b005…be58a`), including the transition and residual MIT notice.
Mac, Windows x64 and ARM64 notice policies were updated together; no copyright text was removed.
Previous Windows candidate archive hashes remain historical evidence, not the rebuilt candidate.

## Rust audit

cargo-audit 0.22.2, RustSec database commit `ef6173cbc5c50ec8166f9a5b28f07834144373ee`:
zero entries in the vulnerability list. Workspace warnings remain: six unmaintained packages
(proc-macro-error and five unic packages), and glib 0.18.5 unsoundness (RUSTSEC-2024-0429).
All seven are absent from `cargo tree --locked --offline -p gil --edges normal,build` for each
of Windows x64, Windows ARM64 and macOS ARM64. They are not waived for the optional Companion;
that broader workspace surface requires its own review before distribution.

## Remaining release checks

- Rebuild native candidates from the patched UI; verify exact archive and file hashes.
- Audit the actual allowlisted payload, not merely the source tree, for private data and secrets.
- Verify the public fixed download and official local marketplace installation path without Git,
  Node, Cargo, admin rights, policy changes or manual plugin-cache edits.
- Refuse unexpected archive paths/links and existing destination replacement.
- Actual ordinary-user Windows install and visible Monitor/fullscreen remain untested.

No new Windows release, marketplace publication, user installation or Core/storage change occurred
as part of this dependency remediation.
