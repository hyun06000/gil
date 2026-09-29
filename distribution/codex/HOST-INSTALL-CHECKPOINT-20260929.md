# Private remote Host installation acceptance — 2026-09-29

Status: **existing-Mac remote Plugin installation and visible Monitor accepted; not published**.
This follows the [private Git transport checkpoint](REMOTE-STAGING-CHECKPOINT-20260928.md).
The maintainer approved replacing the development installation and testing removal/reinstallation.
No public visibility change, tag, release, automatic update or Companion installation was authorized here.

## Fixed candidate and installation

| Item | Observed value |
|---|---|
| Distribution ref | `e7e02a14117183c3a596daa6ec9ca890cccc3c0e` |
| Version transition | `0.2.0+codex.20260927174145` → `0.2.1-preview.1` |
| Plugin / marketplace | `gil-companion-prototype` / `gil-preview-macos-arm64` |
| Channel | `preview_unsigned`; Apple Developer ID and notarization absent |
| Core SHA-256 | `3572786190ec4a8ad3b703ebe5c8ffd9b170a4e6c7da1ea2776abfc12c520a47` |
| Embedded UI SHA-256 | `0aaf30e3c62d63fdc45919d01ece01c1ea2ac2464bf572e72b4a4d4b87b52688` |

The official marketplace CLI removed only the old local registration, then registered the private Git
source at the reviewed ref. Both the persisted ref and actual marketplace checkout HEAD were read back.
The pinned 19-file tree verifier passed. Official Plugin installation then enabled the preview version.
The previous local source was preserved for recovery; installation cache/config files were not hand-edited.

All seven installed Plugin files matched the receipt in bytes, size and mode. A fresh native process
returned **17 tools**, the expected embedded HTML, and the existing explicitly selected Project's View
and current detail using its saved binding, without another prepare call. Its PATH contained only system
directories. Node ran the maintainer test harness, not the installed MCP server.

The official app uninstall operation removed this exact installation; the installed entry and version
cache directory were absent. Official reinstall restored the same version and seven exact files.
A fresh installed process again read the saved-scope View/detail. The separate personal development
installation remained disabled; no second active GIL installation was silently enabled.

Before replacement, after replacement, while uninstalled and after reinstall, content/mode/tree
fingerprints of the acceptance Project, Monitor binding store and preserved rollback source were
identical. No Project write tool or Companion launcher was used. Private paths, fingerprints and report
contents are retained only in local acceptance evidence, not copied into this public-facing document.

## Human screen evidence and its limits

After the request to inspect the new installation's horizontal fullscreen Monitor and node detail,
the maintainer reported that it worked correctly. This is human visible-UI acceptance, not inferred
from an MCP response. The request also mentioned quitting/reopening Codex, but the reply did not
explicitly distinguish that check: a **complete Host restart of this exact preview is not separately
reconfirmed**. Prior development-version Host restart acceptance remains valid only for that checkpoint.
Fresh-server saved-binding restoration above is independently verified and is not a full-app restart.

## Failed checks retained

The initial installed-process harness passed file/process/tool/resource checks but could not read the
saved Project. A diagnostic rerun returned `unreadable`; a no-write handle probe identified sandbox
`EPERM` when opening the existing Project lock read/write. The read-only session needs this handle
for locking. The same reviewed harness under approved execution permissions restored View/detail;
Project/settings permissions were not changed and fingerprints remained identical.
This is a harness permission boundary, not a silently retried native launch timeout or proof of Host UI.
The older first-launch timeout remains a historical unresolved observation.

## Still open

- Actual preview → previous-version rollback; preserved source and commands are not an executed pair.
- Unauthenticated public marketplace installation. Private authenticated Git/SSH acceptance is different.
- Fresh-Mac installation/quarantine: explicitly deferred, **not passed**. Windows/Intel, Claude's
  folder-attached work route, Tauri redistribution and Apple-signed stable distribution are not claimed.
- Public source/distribution audit closeout, privacy choices, security reporting/main protection at
  public transition, and publication approval. See the [publication plan](PUBLICATION-PLAN-20260929.md).

Official installation reference: [OpenAI Plugin packaging and marketplace sources](https://developers.openai.com/plugins/build/plugins).
