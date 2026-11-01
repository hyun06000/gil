# M4-R local recovery validation — 2026-11-02

Contract: [GIL_Revisit_Recovery_v0.1.md](GIL_Revisit_Recovery_v0.1.md).
Base: `514adb8bb94930c6672bff3bebd3664600484434`.
Branch: `fix/product-revisit-recovery-20261102`.
Environment: macOS / Apple Silicon. Isolated synthetic projects only; no user project,
installed plugin, Companion installation or published release was changed.

## Results

| Check | Result | Scope |
| --- | --- | --- |
| Default Core full test suite, sequential | 1,518 passed, 2 ignored | All 47 suite summaries including doc tests |
| Product library + selected integration suites, sequential | 710 passed, 1 ignored | Library; approved-plan wire; context; context profiles; guidance waves 1/2 |
| Final guidance/recovery integration rerun | 32 passed | After final interrupted-recovery guidance edits |
| Late-file restore regression | 1 passed | File created after consent must not enter the deletion plan; no restore area created |
| Default and product `--all-targets` builds | Passed | Core package only; not Tauri/Companion |
| Windows MSVC cross-check | Passed | Compilation check only; no Windows runtime test; recovery remains refused there |
| UI and plugin JS suites | 150 passed, 4 skipped | Synthetic DOM/events, common workflow and bundle checks |
| Packaging and license-notice checks | 98 passed, 1 skipped | Only changed source/bundle input hashes updated; license texts unchanged |
| `git diff --check` | Passed | No whitespace errors |

The full suite ran before the final product-only guidance adjustment. The final integration
rerun covers that adjustment. The late-file regression was added and run afterwards; both
all-target builds include it. These counts overlap and must not be added into one coverage claim.

## Recovery evidence

`tests/support_wave1/revisit_cases.rs` drives actual product-profile `gil mcp --serve` processes
and real temporary files. Its six cases cover:

1. Already-closed failure → read-only review → forged/stale consent refusal → explicit synthetic
   App approval → preservation and restore → two retries → success → next approved item.
   Prior failure nodes, Will/Existence, Snapshot objects and approval history are compared.
2. Changes to `.env`, `.git/HEAD`, dependency output and `.gitignore`-excluded files refuse
   without altering project state or working files.
3. Failure after logical state save, then actual child-process abort after first restore apply,
   then restart and completion from the durable approval.
4. Server restart invalidates the old private token; corrupt evidence objects refuse before writes.
5. A symlink at the preservation area cannot redirect writes outside it.
6. Reinterview after failure obtains a new approved revision; retry keeps its old failure lineage
   and binds to the new revision, while old execution bindings remain unchanged.

The restore unit regression separately introduces a new file between the approved observation
and actual restore-plan construction, and verifies that neither old nor new files change.
UI controller tests cover explicit trusted-event gating, safe path text, token privacy,
duplicate suppression, uncertain replies, stale consent and stopped controllers.

## Reproduce

Run from the checkout, with locked dependencies installed as in CONTRIBUTING.md:

```sh
cargo test --locked -p gil -- --test-threads=1
cargo test --locked -p gil --features approved-plan,visual-authoring,product-safety --lib --test approved_plan_wire_wave1 --test context --test context_profiles --test guidance_wave1 --test guidance_wave2 -- --test-threads=1
cargo test --locked -p gil --lib approval_tests
cargo build --locked -p gil --all-targets
cargo build --locked -p gil --all-targets --features approved-plan,visual-authoring,product-safety
cargo check --locked -p gil --features approved-plan,visual-authoring,product-safety --target x86_64-pc-windows-msvc
node --test ui/layout.test.mjs mcp-app/*.test.mjs plugins/gil-companion-prototype/*.test.mjs
node --test distribution/codex/*.test.mjs distribution/compliance/notices.test.mjs
git diff --check
```

Initial sandbox runs failed at local socket creation; tests requiring sockets were rerun with
that permission. An existing file-watcher timing test failed under parallel execution, passed
alone, then passed in sequential full runs. Initial packaging failures were stale input hashes;
package/toolchain/license inputs were compared before updating the two changed hashes.
Existing launcher dead-code and other compiler warnings remain; these builds are not warning-free.

## Still unverified / deliberately out of scope

- A real human click in an actual fullscreen Monitor, visual layout and Luna behavior.
  Synthetic App calls and DOM events are not evidence of actual user consent or screen visibility.
- The user's blocked project on Desktop; it was not opened or modified.
- Release installation and update; this change is local, not deployed.
- General `restore`, Step-level revisit, Windows destination safety, schema migration and recovery
  when protected files differ. Those paths remain refused.
- Claude's concurrent host-support integration. Its client, installer and host-selection code was
  not edited. Shared MCP registration, App bootstrap, workflow and generated HTML require a joint
  check when the branches are combined; regenerate the HTML from the combined sources.
