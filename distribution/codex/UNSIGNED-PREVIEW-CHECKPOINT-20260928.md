# Unsigned preview candidate — 2026-09-28

Status: local `0.2.1-preview.1` candidate prepared; **not installed, tagged or published**.
The maintainer approved proceeding without Apple Developer ID signing/notarization and deferring
fresh-Mac testing. Neither absence is reported as a passed check. See [channel policy](RELEASE-macos.md).

## Input and evidence boundary

The runtime comes from the reviewed clean `gil` CI artifact, not a fresh build of this working branch:

- Source: `fcfdf63b5c8e3965f01b0d50f0612d1e0e86cf5c`, 275 source files, dirty false.
- Source snapshot SHA-256: `2cb3c5f58b4e11c65670cdca6decc28cd565d0b093c26f313ded7e710155019b`.
- CI run: `36386001031`; original development archive SHA-256:
  `5a6bff63173817770ad50b28cc8afc52629abc9fa822eac6aab069777ed2fe05`.
- Prior official local marketplace installation kept all seven installed payload files' bytes and modes.
  The maintainer subsequently confirmed that the Monitor displayed correctly on the existing Mac.
  This is human evidence for that installed development version, not this new prerelease's Host acceptance.

This branch adds packaging/policy tooling and documentation only. It does not change Core, MCP tool
semantics, shared UI, installed Plugin files or user Project records. Its own clean PR-head CI remains
to be run; the old CI result is not counted as coverage of the new tooling.

## Candidate results

| Check | Observation |
|---|---|
| Version / channel | `0.2.1-preview.1` / `preview_unsigned`; no public tag |
| Existing identity | `gil-companion-prototype` in `gil-preview-macos-arm64`; unchanged |
| Apple trust | `codesign` reports ad-hoc, linker-signed; no Developer ID/notarization supplied |
| Input preservation | Original development tree unchanged |
| Allowed changes | Output manifest version, PREVIEW warning, release receipt only |
| Payload | Core, catalog, Skill and all legal notices retain exact bytes/modes |
| Archive roundtrip | Exact inventory/mode match after unpacking |
| Fresh challenge | 501 ms; no retry |
| MCP initialize | 5 ms; 30-second deadline |
| Native tools | 17, unique names |
| UI resource | Embedded HTML hash matches the prior CI UI; no new visual acceptance claimed |
| Runtime PATH | System tools only; no Node/npm/Cargo/Homebrew requirement |
| Official Plugin validator | Passed |
| Publication / fresh Mac | Separate approval required / deferred, not passed |

Hashes:

```text
candidate archive  420aaa939e39ae34061d588981f59e1a05e7d9bc313e88527bdd501ef3ec4a56
Core               3572786190ec4a8ad3b703ebe5c8ffd9b170a4e6c7da1ea2776abfc12c520a47
embedded UI        0aaf30e3c62d63fdc45919d01ece01c1ea2ac2464bf572e72b4a4d4b87b52688
```

The Core descriptor remains version `0.1.0`, protocol/action surface `1`, storage format `4`.
The reviewed Monitor/Detail wire contracts remain `1`; packaging uses migration `none`.
Plugin release version and Core version are different namespaces. No domain format was bumped.

The first local staging trial used `0.2.0-preview.1`. Review caught that this sorts **below** the installed
development base `0.2.0+codex.…`. That trial is retained locally, not selected for release. The final choice is
`0.2.1-preview.1`, and a regression now rejects candidate versions that do not sort above their input base.
No installed version was changed during either trial.

Packaging/policy/legal regression suite: **73 passed, 0 failed, 0 skipped**. `git diff --check` is clean.
Core/UI source did not change, so the full Rust/UI suites were not rerun in this packaging-only turn.

The candidate directory contains the marketplace tree, archive, SHA256SUMS, checks.json and
update-record.json. Generated payloads and local paths stay outside Git. A checksum is neither publisher
authentication nor an Apple malware check. These records are review evidence, not signed attestations.

## Remaining before users receive it

- Review this branch and run CI on the exact PR head. Do not push directly to main.
- Complete source/history, CI logs/artifacts, public contact/security and governance checks before public visibility.
- Choose and verify the immutable remote distribution location and installation instructions. No public URL exists yet.
- Test the new prerelease metadata through the official Host install path; retain the prior version for recovery.
- Validate actual update/removal/reinstall pairs and Project preservation, not just metadata compatibility.
- Publish only reviewed bytes under an approved immutable version; no automatic unsigned updates.

New Mac, Windows/Intel, Claude work-Plugin rendering and renewed Tauri acceptance remain explicitly outside
this checkpoint. The previously observed first-launch timeout is not declared fixed by this successful run.
