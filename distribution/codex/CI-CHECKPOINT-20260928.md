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
still need a separate review and pinned-SHA update. Compiler-warning-free does not mean annotation-free.

This workflow does **not** rerun the full Core suite or the browser UI suite. The local migration checks
(Core 1,175, UI 70, Node 104 passed / 4 skipped) remain separate in [SOURCE-MIGRATION](../../SOURCE-MIGRATION.md).
Native protocol success is not proof of visible fullscreen or marketplace installation. No new-machine
or quarantine acceptance, Developer ID signing, notarization, Windows or Tauri-specific verification was
performed. The earlier unexplained first-launch timeout remains a new-machine acceptance item.

The repository remains private. Public conversion, release and marketplace publication require separate
approval and the remaining [open-source gates](../../spec/GIL_Open_Source_Readiness_v0.1.md).
