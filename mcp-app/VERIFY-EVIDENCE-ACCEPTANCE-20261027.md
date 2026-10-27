# Durable Verify evidence and learning provenance

## Product rule

Every newly closed Verify must cite durable project files through `evidence_files` in its Report.
Chat text or the Report alone is insufficient. This is enforced by `ProjectSession`, the common
filesystem transaction used by CLI, ordinary MCP, and format-5 typed visual MCP. There is no
configuration to skip the gate.

- List 1–32 unique safe project-relative file paths, one per line.
- Every cited file must exist in the captured Snapshot and have nonempty bytes.
- At least one cited file must be new or changed relative to the current baseline Snapshot.
  Changing an unrelated file does not validate unchanged cited evidence.
- Read-only research and failed experiments still leave observations in a document/log/data file.
  Images are appropriate when useful; `not_needed` or `missing` only describes image attachment.
- Verify execution/result, Analysis interpretation, and Experiment Outcome lesson cannot be blank.
- Core binds Analysis to the actual closed Verify (`evidence.verify_ref`) and Outcome to the actual
  closed Analysis (`learning.analysis_ref`). Conflicting supplied references fail.
- The existing Cycle `outcome_ref`, Step Journey revision, and Done Will preserve the connection
  from captured world to interpretation and learning. No invented Knowledge/Memory entry is added.

A refused gate leaves the logical state, open node, Will and Journey unchanged. The existing capture
mechanism may leave unreferenced objects, as before. New-write checks do not reinterpret or rewrite
historical records. Low-level domain/load/save APIs continue to represent historical states;
production CLI/MCP writes use the transaction boundary. File evidence is checked against the
captured manifest, not by joining untrusted Report paths onto a filesystem root.

## Verification

Deterministic tests cover missing paths, empty files, unchanged evidence, unrelated changes,
directories, duplicate paths, absolute/traversal/internal paths, real documents, image attachments,
image-free typed closes, read-only historical loading, blank learning fields and conflicting links.
Raw CLI and real stdio MCP reject chat-only closes. Reopening preserves the links and Snapshot;
deleting a working evidence file and restoring recovers its captured bytes without rolling back
Journey or the lesson. Existing read-model fixtures explicitly model historical records; live-write
fixtures create real evidence files rather than bypassing the production gate.

Final deterministic validation on macOS arm64:

- `cargo test --locked -p gil`: 1,428 passed, two existing ignored tests.
- Product-feature evidence/MCP/visual attachment tests: 56 passed; Core library: 598 passed, two existing ignored tests.
  The final Report-syntax recovery change was also exercised by all 30 product MCP tests.
- `cargo build --locked -p gil --all-targets`: passed.
- JS/layout/MCP/Plugin/codex-packaging/license tests: 203 passed, five existing skipped tests.
- Windows packaging contract tests: 31 passed (not native Windows execution).
- Skill Creator validation: passed.
- Luna independent next-action decisions: 15/15 direct MCP and 15/15 Plugin skill, including
  chat-only Verify requiring document creation rather than closure. These precede the final typed
  close hint refinement; the full workflow rerun below exercises that refinement.

## Luna workflow check

`interview-workflow-eval.mjs` uses an isolated temporary project, the actual product-feature Core
(`visual-authoring,product-safety`, format 5), and `gpt-6-luna` with low reasoning. It starts from
approved Interview Synthesis (direct MCP) or Outcome (Plugin skill). The model reads two fictional
sources, performs two bounded Experiments and returns a Korean report. A fixture tool writes only
within that temporary project and returns actual relative paths. The proxy observes phase usage;
it does not gate source reads or document writes on the GIL stage.

The evidence extension checks real nonempty files saved during Verify, file paths cited in every
successful Verify Report, and final links, in addition to the previous Interview/Experiment checks.
The two source values imply 20% price change and 50% revenue growth; these are synthetic, not market
research. Evidence documents must describe source, calculation, observation and limits.

The first two attempts passed direct MCP (38 calls, two evidence documents) but halted Plugin mode
at its first Verify. Error logs showed an evaluation-fixture fault: the Host did not forward
`GIL_BINARY` into the proxy's environment; after the concurrent default test build replaced the
repository binary, the later proxy used a non-authoring binary against the format-5 fixture. Its
-32602 refusal explicitly said typed inputs were unsupported. These are failed evaluation runs,
not passes or evidence that Luna ignored a working typed tool. The proxy now receives the exact
binary path as an argument; the runner also snapshots the chosen binary into its temporary work
folder. JSON-RPC refusals are now logged alongside tool-result refusals.

During this investigation, open/refusal receipts, the bundled manual and Plugin skill were made
more explicit about the same-tool `gil_close` typed shape with `expected_owner` and `visuals`,
including an image-free `not_needed` example. No missing declaration is silently invented.
With the binary path fixed, direct MCP completed 37 calls, two Experiments and two evidence documents.
Plugin mode saved its first document but submitted an invalid Report (`evidence_files:` followed by
an unindented bare path). The parser correctly refused it, but typed `invalid_report` used an uncertain
close message, giving no repair example. It now explicitly confirms that nothing ran and shows
single-file and multiline-block syntax; instructions/skill show these examples too. The unchanged
open state is covered by a real stdio regression assertion. The final Plugin rerun passed all ten workflow assertions: 51 calls, three closed Experiments
(price, revenue, synthesized report) and three actual nonempty documents. It closed every Verify,
Analysis, Outcome and Cycle before final delivery, with source reads/document writes only in Verify
and links to the saved documents in the final response. The preceding fixed-binary direct run
passed all ten assertions with 37 calls, two Experiments and two documents; the last refinement
only clarified malformed Report recovery. Both persisted states were inspected for actual
Verify/Analysis links and evidence file bytes. These are individual successful trials, not a
statistical guarantee across runs.

Use `GIL_EVAL_MODES=plugin-skill` or `direct-mcp` to select a targeted live check; the default runs both.
Each retained failed run is reported here rather than discarded as a pass.

## Scope and limits

These checks establish durable bytes, valid provenance, recoverability, and the observed model
behavior in synthetic CLI-driven sessions. They do not certify content truth, depth of learning,
all future model behavior, native Windows execution or Host fullscreen visibility. GIL cannot
intercept an agent's unrelated Host tools or prevent it from emitting chat while a node remains
open; it refuses to record that work as a completed Verify without evidence. Historical records
remain historical and are not retroactively certified. No release or installation is part of this
change; the new rule applies after a built version containing it is installed.
