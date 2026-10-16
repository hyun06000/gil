# Preview.4 publication — 2026-10-14

Scope: opt-in unsigned Codex/macOS Apple Silicon preview with **image reports** (runtime profile
`visual-authoring`). Not stable, not Apple-signed/notarized, not Windows/Intel, not Claude work-mode,
not a universal Plugins Directory listing.

- Source: `58659b4615531c02154cad582fc8ce900fe1d70c` (main after PRs #12 → #13 → #15 → #14 → #16 → #17).
- Source CI on that head: visual-authoring [38541886780](https://github.com/hyun06000/gil/actions/runs/38541886780),
  default [38541880620](https://github.com/hyun06000/gil/actions/runs/38541880620),
  Windows x64/ARM64 [38541890753](https://github.com/hyun06000/gil/actions/runs/38541890753) — all passed.
  The Windows run is a regression check only, not Windows image-report Host acceptance.
- CI artifact 11882648504; original CI archive SHA-256
  `330587c5ef0cd7ba2aabdbf756e5caf24ab5599673ac37cb02143a1661cd999f`.
- Candidate staged with `unsigned-preview.mjs prepare … 0.2.1-preview.4 … --visual-authoring` from the
  extracted CI tree: Core, Skill, UI and notices bytes preserved; only manifest version and `PREVIEW.md`
  versioned. Candidate archive SHA-256 `b2c40ec3933a32ac3a74f56910f89555bb6b6428fd6d47959b3913513e47cdb4`.
- Core `e3c1f749189c9c1b3dee7897dbf8f9cd44ac472fd2b082dd956c19087933ea43`,
  UI `306e137986f918d61e0e5e39c73fbfd4aef6e77d334180e78af91a66bbd1381f`,
  receipt (`release.json`, schema 3) `2825343531aad40727bdb923e752f2ad59fa6ab2d797ba4dd8dcada379e7324d`.
- Contract: protocol 1, action surface 2, storage formats 4..5, 24 tools, `migration: requires_review`.
  `update-policy.mjs` blocks preview.3 ↔ preview.4 as `explicit_contract_and_migration_review_required`;
  this is the intended gate, not an automatic update permission.
- Marketplace PR: [#5](https://github.com/hyun06000/gil-marketplace/pull/5), head
  `0f547871ca8618e38ab4ff39b4d214d4ed354c12`, `pinned-payload` run 38579827925 passed; merged as
  `e3ce1be57f1a2b6829b89258e703a6cabc1b77eb` (merge tree identical to the reviewed head);
  read-only verify run 38580637981 on the merge commit passed.
  `checks/verify.mjs` now pins preview.4 (receipt schema 3, visual-authoring, 24 tools, storage 4..5).
- Release: [v0.2.1-preview.4](https://github.com/hyun06000/gil-marketplace/releases/tag/v0.2.1-preview.4),
  tag → `e3ce1be57f1a2b6829b89258e703a6cabc1b77eb`, prerelease, four assets copied unchanged from staging:
  archive (above), `checks.json` `f0d83948c497d440bb60cd588d5685469325931d8cec3cdfd474186c5ae12216`,
  `SHA256SUMS` `5263aa518071b02191cc4ba4f0ac82ead85913d871fe0f4660b71a23e3645482`,
  `update-record.json` `083fbd486139ffa63965b87ba796d55212fbee3783437ace0d87ecdf2418ccda`.
  GitHub asset digests matched; an unauthenticated download matched byte for byte.
  Existing tags `v0.2.1-preview.2`/`v0.2.1-preview.3` and their assets are unchanged.

Each step (branch push, merge, tag/release, this documentation) was separately approved by the owner.
The receipt keeps `publishable: false` as build-time authority; owner approval is recorded by the PR and
release, not by rewriting the receipt.

## Evidence and limits

- Download/receipt check: the nine CI payload files matched the receipt by hash and mode; Skill bytes
  equal the source head.
- Isolated official Codex CLI replacement pair in a separate `CODEX_HOME`: preview.3 → preview.4 →
  preview.3 → preview.4 returned 17 → 24 → 17 → 24 tools; installed preview.4 bytes/modes equal staging.
  Synthetic format-4 and format-5 projects, Verify/Cycle shared-image references, captions and provenance
  were unchanged across replacement and MCP server restart; preview.3 refused format 5 as expected.
  `gil_monitor_report_image` bytes decoded to the project PNG hash.
- Packaging/notices tests: 93 passed. Rust suites were not rerun (no source change since the final
  main CI above).
- Owner's existing Mac, 2026-10-14, exact candidate executable (verified by process path and hash)
  serving a synthetic image project through a temporary Codex connection: Cycle rectangle selection
  opened the cycle report; the report image, caption, purpose and source were displayed; click enlarged
  to 1200×700 and click returned; the Verify step report showed the same image with its own caption;
  Codex chat stayed usable beside the Monitor side panel. Background accessibility clicks did not reach
  the Monitor web view; real mouse clicks did.
- 2026-10-15, fresh isolated `CODEX_HOME` with no stored Git credentials or Codex sign-in: official CLI
  registered the public tag `v0.2.1-preview.4` and installed the Plugin; the seven installed files matched
  staging bytes/modes, the cached marketplace commit was `e3ce1be5…`, static `verify.mjs` passed, and the
  installed executable answered 24 tools with format-4/format-5 synthetic projects and shared image reads.
- Production preview.3 installation (seven files) and real user projects were not modified or used at
  any step; protected Skill/record fingerprints stayed unchanged. **One approved configuration change
  did occur:** the owner's temporary Codex MCP connection used for Desktop acceptance was repointed from
  the earlier development executable to the isolated preview.4 install (command path and Monitor state
  directory only), after a 0600 backup of the previous configuration. No other setting, the production
  Plugin entry or any project record was changed.
- The Git-less (no developer tools) installation path documented in `GITLESS-INSTALL.md` remains
  preview.3-only; preview.4 was not verified through it.

**Not verified for preview.4:** fresh Mac / VM installation (the 2026-10-07 VM check was preview.3),
full Desktop quit/relaunch restore, the expanded (fullscreen) Monitor switch with this executable,
external tester results, Developer ID signing/notarization, Windows or Claude image-report hosts.
The installation success above does not close any of these.

Rollback target: preview.3 at tag `v0.2.1-preview.3` / `8445dbdb30c9cfe16a77245adcc4da56c9655b46`
through official Plugin management with approval. preview.3 cannot read format-5 projects; that refusal
is expected and does not lose or downgrade records. Installing preview.4 does not migrate any project;
migration needs per-project backup/recovery verification consent and then apply consent.
Never remove `.gil`, change OS security or hand-edit caches.
