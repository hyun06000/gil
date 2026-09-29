# Publication-surface review — 2026-09-29

Result: **content checks completed; distribution Git identity disclosure remains a publication hold**.
Both repositories are still private. This is a bounded privacy/secret/legal-file review, not a security
certification, penetration test or authorization to publish.

## Reviewed baseline

| Surface | Bound |
|---|---|
| Source | `1719e0546ca9556fc15009627aeb924e2a71540f`, 282 tracked files |
| Source history | 15 reachable commits including merges, 327 unique historical blobs |
| Distribution | reviewed `e7e02a14117183c3a596daa6ec9ca890cccc3c0e`, merged at `c03d22728200c657a45ff0ab521fe26bdd81037d` |
| Distribution history | 3 reachable commits including merge, 20 unique blobs, 19 current files |
| GitHub CI | 7 source run logs, 1 distribution run log; all 8 available source artifact downloads |
| Collaboration content | 5 source PRs, 1 distribution PR and 2 issue comments; no review comments at audit time |

The source starts from the independent root recorded in SOURCE-MIGRATION, not the old development
history. Current baseline refs, rather than a moving branch name alone, define this review. Later commits,
comments, workflows and artifacts need their own delta review before publication.

## Checks and results

- Gitleaks **8.30.1**, default rules and `--ignore-gitleaks-allow`, scanned both histories and downloaded
  GitHub surfaces (archive depth 3): no secret detections. No custom config environment was set.
  Its source count is 9 non-merge commits and distribution count 2; these do not contradict the
  full reachable-commit counts above. A detector finding no match is not proof of no possible secret.
- Separately read every historical blob, including binary bytes, for known private developer/workspace/
  temporary paths, acceptance-Project identifiers and private-key headers: no matches. Expanded bytes
  of all 8 CI tar archives were checked too. No unexpected tracked `.gil`, settings/dependency cache,
  private-key container or conversation dump was found by the scoped filename rules.
- Generic home-path matches in source are two synthetic rejection-test inputs. They are not a real
  development path. GitHub runner paths in logs are build infrastructure, not a user's workstation.
- UI fixture reports were reviewed as synthetic examples/placeholders, not copied user conversations.
  The three tracked images are generated `gil` wordmarks; their PNG chunks contain only IHDR/IDAT/IEND,
  not embedded text/EXIF. The application icon and its generator were inspected.
- Current relative Markdown file targets resolve. Historical private audit records and raw GitHub log
  downloads remain outside these repositories. Their contents are not copied into public reports.
- Root MIT and upstream legal text were preserved. Offline notice coverage check passed for **84
  packages plus Rust standard-library notices**; packaging/legal regressions **73 passed, 0 failed**.
  This does not certify other targets or substitute for legal advice.
- Current source Git identities are GitHub user no-reply/service addresses. **Distribution payload
  commit author and committer metadata contain a personal email.** The value is intentionally omitted.
  It is not in the installed payload, but would be visible through Git history if made public.

The initial GitHub log-download command was blocked while trying to create a CLI cache outside the
approved workspace. Logs were then downloaded directly through the read-only API into private audit
outputs; no broad cache permission or GitHub setting change was used to work around it.

## Required decision and remaining gates

Do not mark publication ready until the maintainer approves the metadata disclosure or a separate
cleanup plan. No ordinary follow-up commit can erase that metadata from ancestors. No history rewrite,
force push, repository deletion or private-to-public conversion occurred in this review.

The [publication plan](../codex/PUBLICATION-PLAN-20260929.md) keeps public access, required-PR protection,
live PVR/SECURITY, conduct-policy contact review and exact-byte publication as explicit gates.
Private remote installation and visible fullscreen acceptance are recorded separately in the
[Host checkpoint](../codex/HOST-INSTALL-CHECKPOINT-20260929.md). Fresh-Mac testing remains deferred.
