## GIL shared workflow

These rules apply to every GIL connection, with or without a Plugin, on every operating system.
Use only the user's selected absolute `project_root`. Read the tool's text guidance as well as
`structuredContent`; follow the actual recorded position and the connected tool schema.

1. **Start and show.** Start a new project once. After successful `gil_start`, follow its returned
   `show_gil_monitor` call before opening the first Interview step. Do not prepare the Monitor again
   if start returned its scope. Never repeat start to open a screen. If start succeeded but Monitor
   preparation failed, report the display failure and continue the recorded Interview via `gil_open`.
   A display failure alone does not block Project work. A successful tool response does not prove
   that a screen is visible. In a new session, read `gil_context` once and resume instead of starting again.
2. **Agree, then finish Interview.** Interview clarifies and agrees scope. Synthesis waits for the
   user's reply to its proposed statement; silence, earlier requests and the agent's own judgment
   are not approval. After explicit assent: close Synthesis → open/close Interview Outcome →
   separately close the Interview Cycle with its Report → open a bounded Experiment. Wait for
   each success. Do not ask again for approval already given to that proposal. Outcome judges the
   approved scope; do not leave it open until the whole project is finished. Resume from the actual
   recorded position without repeating already closed steps.
3. **One checkable question at a time.** Open a small action contract, perform its `next_action`,
   then `gil_close` when `done_when` is met, before further external work or final delivery.
   A new request does not extend an already completed contract. Substantive research, source
   checking, calculations, deliverable writing (even a report requested in chat), implementation
   and tests belong in Experiment Verify, never an open Interview. Split independent uncertainties
   into successive Experiments with observable success conditions. Multiple related searches,
   edits and tests within the same unfinished Verify are valid.
   **Open before acting:** after closing Hypothesis, the next action is `gil_open` for Verify.
   Wait for its success before reading sources, searching, calculating or writing files. A closed
   Hypothesis or a prepared Verify contract does not mean Verify is open. Do not gather evidence
   first and then open Verify to record work already performed. This also applies between cycles:
   finish the old cycle and reach a successfully opened Verify before the next investigation.
4. **Leave evidence in the world.** Every Verify must save its actual observations/results as
   project files: documents, logs, datasets, code or visual artifacts. Chat alone cannot complete
   Verify. Even read-only investigation or a failed test needs a saved observation document.
   Include `evidence_files` in its Report: 1–32 unique safe project-relative file paths. Every
   listed file must be nonempty in the Snapshot, and at least one must be new or changed from the
   baseline. One file: `evidence_files: evidence/result.md`. Multiple files use this Report syntax:

   ```text
   evidence_files: |
     evidence/one.md
     evidence/two.md
   ```

   Record sources, inputs/method, observed results and limitations; never invent results or save
   meaningless markers to satisfy the gate. Use visual evidence when it helps assess the work.
   Image declarations `not_needed`/`missing` do not waive ordinary file evidence. If evidence is
   refused, save truthful evidence while Verify remains open, then submit the corrected close.
5. **Learn from that evidence and close.** After Verify, write a nonblank `interpretation` in
   Analysis and a nonblank `lesson` in Experiment Outcome. Core links `evidence.verify_ref` and
   `learning.analysis_ref` to the actual current lineage; do not supply different references.
   Finish Analysis, Outcome and the Cycle Report before the next independent problem or final
   delivery. Deliver links to the saved evidence/deliverable alongside the chat summary.
6. **Use the actual close contract.** Close both Steps and Cycles with `gil_close`; the legacy
   `gil_cycle` cannot accept a Report. Format-4 steps (including Verify) and format-5 non-Verify
   reports without stored images use `{project_root, report}`. Every format-5 Verify uses
   `{project_root, report, expected_owner, visuals}`, even without images. Supply both
   `expected_owner: {kind: "step", ref: "<actual open step ref>"}` and `visuals`; with no images,
   use `visuals: {attachments: [], uses: [], declaration: {state: "not_needed", reason: "<actual reason>"}}`.
   Format-5 non-Verify reports citing stored images also use both fields, per the connected schema.
   Keep `evidence_files` in the Report in every Verify. Available capabilities and project format,
   not the OS name, determine the input shape. Never migrate an existing project merely to follow
   these instructions.
7. **Recover and keep the user informed.** A rejected Project action must be resolved before
   further Project work. Never treat a refused close as closed, backfill skipped steps or fabricate
   history. A confirmed input rejection means the call was not executed: check current state and
   submit the corrected input. An uncertain/lost close response requires inspecting recorded state,
   not blindly repeating the close. Briefly tell the user the current step and immediate action.
   Monitor shows recorded progress, not private reasoning or proof that every external action used GIL.
