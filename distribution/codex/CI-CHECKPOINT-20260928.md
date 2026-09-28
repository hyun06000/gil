# First independent gil preview CI

2026-09-28 · private repository · development evidence, not a release

## Source and run

- Repository: `hyun06000/gil`, **private** throughout this check.
- First/root source commit: `fbc410062dd79f3e0bcd7c27d86b8d1075b66f52`, no parent or imported refs.
- [GitHub Actions run 36363411078](https://github.com/hyun06000/gil/actions/runs/36363411078):
  manual `workflow_dispatch`, `macos-15` arm64, job **success in 1m56s**.
- Clean source snapshot: 263 files, SHA-256
  `e66f05cfceb8323542979c325d0a11f00daee33ded1d538794f8dd2f12767d6b`.

This is new evidence from this repository, not the previous development repository's CI.
The later documentation-only commit records these results; the run tested the source commit above.

## What passed

| Check | Result |
|---|---|
| Packaging and notice regression tests | **35 passed, 0 failed, 0 skipped** |
| Locked third-party coverage | **84 packages**: Core 79 + bundled UI 5, plus Rust standard-library notices |
| Embedded UI and native release build | Success; Rust compiler warnings 0 |
| Build-time source stability | Clean snapshot unchanged before/after build and packaging |
| Archive roundtrip on runner | File bytes, SHA-256 and executable modes preserved |
| Runner first challenge / MCP initialize | **12 ms / 9 ms**, each below 30 s; no automatic retry |
| Runner MCP | **17 tools**, correct resource URI/MIME and exact embedded UI hash |
| Downloaded archive | Checksum agrees with `SHA256SUMS` and `checks.json` |
| Archive inspection before extraction | 10 allowlisted files; no symlinks, hard links, traversal, devices or extra metadata |
| Downloaded source correspondence | HEAD, source snapshot, manifests, catalog, Skill and MIT LICENSE match clean local source |
| Original notices | All three notice files byte-identical to the locally assembled pinned originals |
| Local first challenge / MCP initialize | **559 ms / 7 ms**, no automatic retry |
| Local native smoke | **17 tools**, same UI hash, system dylibs only, child PATH `/usr/bin:/bin` |

The local smoke used an isolated temporary state directory and did not open a user Project or Companion.
`0755` is preserved on the native executable; other packaged files are `0644`. Tar ownership is normalized
to root/wheel, with no developer owner name, ACL, xattr or resource fork. The source/binary path-remapping
check rejects developer home paths. This is not a bit-for-bit reproducible-build or publisher-signature claim.

## Artifact identity

- Actions artifact ID: `10946640902` (7-day development retention; reported expiry `2026-10-05T00:47:53Z`).
- Tar: `gil-codex-macos-arm64-preview.tar.gz`
- Tar SHA-256: `538ade6cc45e5e9cd886dddf22696df6c15a010d9f1c92269dca311f7c106ecc`
- Native binary SHA-256: `3572786190ec4a8ad3b703ebe5c8ffd9b170a4e6c7da1ea2776abfc12c520a47`
- Embedded UI SHA-256: `0aaf30e3c62d63fdc45919d01ece01c1ea2ac2464bf572e72b4a4d4b87b52688`
- Channel: **`development_unsigned`**, **`publishable: false`**.

The downloadable artifact is temporary CI evidence, not a stable installation URL. Expiry does not change
the recorded source/hash evidence. No release, tag, marketplace registration or installed Plugin was changed.

## Warning and remaining gates

GitHub emitted one workflow annotation: the pinned checkout and upload-artifact Actions target Node 20
and were forced to run on Node 24. The run succeeded, but maintained Node-24-compatible Action revisions
still needed a separate review and pinned-SHA update at this first checkpoint. The follow-up below records
that correction without erasing the original warning. Compiler-warning-free does not mean annotation-free.

This workflow does **not** rerun the full Core suite or the browser UI suite. The local migration checks
(Core 1,175, UI 70, Node 104 passed / 4 skipped) remain separate in [SOURCE-MIGRATION](../../SOURCE-MIGRATION.md).
Native protocol success is not proof of visible fullscreen or marketplace installation. No new-machine
or quarantine acceptance, Developer ID signing, notarization, Windows or Tauri-specific verification was
performed. The earlier unexplained first-launch timeout remains a new-machine acceptance item.

The repository remains private. Public conversion, release and marketplace publication require separate
approval and the remaining [open-source gates](../../spec/GIL_Open_Source_Readiness_v0.1.md).

## Follow-up: Node 24 Actions on PR 1

The first PR, [#1](https://github.com/hyun06000/gil/pull/1), pins official checkout and upload-artifact
v7.0.1 by full commit SHA. Both immutable action manifests specify `node24`. The workflow remains
manual-only, read-only, credential-nonpersisting, with seven-day artifacts; explicit `archive: true`
keeps the tar, checksum and checks together. No direct main push or release publication was used.

- Tested clean PR head: `19fe57baa7487102fb63c452ab35ac6d800f94ef`.
- [Run 36369496342](https://github.com/hyun06000/gil/actions/runs/36369496342): **success, 1m52s**.
- Packaging/notice tests: **35 passed, 0 failed, 0 skipped**. Rust compiler warnings **0**;
  GitHub workflow annotations **0**, including no Node 20 compatibility warning.
- Clean source: 272 files, snapshot SHA-256
  `7c91ea9843d8160447fcd1f80159d2158529b2b4ac6a886ec16b758442cbffd4`.
- Artifact ID `10948817561`, reported expiry `2026-10-05T02:23:15Z`.
- Tar SHA-256 `dac498d4b6817286729c660535001129f83de2ab7167d7f44adf3552a1ed731a`.
- Downloaded archive: exact ten-file allowlist, safe paths/types, native mode `0755`, others `0644`;
  source identity, manifests, catalog, Skill, MIT and all three original notice files matched.
- Native binary and UI hashes are unchanged from the first CI above. Coverage remains 84 packages plus
  Rust standard-library notices. Runner native smoke: **17 tools**, first challenge **9 ms**, initialize
  **8 ms**. Downloaded local smoke: **17 tools**, **585 ms / 7 ms**, isolated state and system-only PATH.
- Channel remains **development_unsigned**, **publishable: false**. No plugin installation, real Host
  UI, new-machine/quarantine, signing, notarization, Windows or Tauri acceptance was performed.

This documentation follow-up is later than the tested head. Final PR-head CI must also succeed before
merging; do not treat the earlier head's run as a check of later commits. Main's server-side PR protection
is separately blocked by the private-repository plan requirement described in the
[governance checkpoint](../compliance/REPOSITORY-GOVERNANCE-20260928.md).
