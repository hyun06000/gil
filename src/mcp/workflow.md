## GIL shared workflow

These rules apply to every GIL connection, with or without a Plugin, on every operating system.
Use only the user's selected absolute `project_root`. Each receipt's text guidance names **one next
call or one wait**; do it, then read the next receipt. Read `structuredContent` too, and follow the
actual recorded position and the connected tool schema. Never repeat a call that already succeeded.

**Project file boundary — at every stage.** Creating, modifying, deleting, renaming or moving
Artifact files inside the selected `project_root` requires a successfully opened Experiment
Verify. This includes importing files made elsewhere, DOCX/PDF production, chart PNGs, rendering,
test output and temporary files written inside the project. Check the actual open Step before
launching a tool that can change these files; if the position is unknown, read `gil_context` once.
Finish all writers before closing Verify. A prepared contract or approved plan is not an open Verify.
Outside Verify, keep project Artifact files unchanged: this applies to every Interview (including
Closing), Define, Hypothesis, Analysis, Outcome, Cycle/Chain Reports, approval waits, boundaries
with no open Step and closed Chains. Follow the current legal next call to reach Verify; never
skip steps, invent approval or backfill an execution that happened earlier.

Work outside the project and web research that leaves project files unchanged are outside this
file boundary; do not require a GIL Step just for them. Existing evidence can be read in any stage.
When bringing an external result into the project, open Verify first, record its actual provenance
and verification, and cite the imported file in `evidence_files`; do not claim it was created there.
GIL's own record/approval writes and explicitly authorized recovery use their existing GIL tools
and contracts; never edit `.gil` by hand. These are agent instructions, not an OS write lock.
The current dirty gate detects changes at GIL boundaries; it does not intercept other tools.

1. **Start and show.** Start a new project once. After successful `gil_start`, follow its returned
   `show_gil_monitor` call before opening the first Interview step. Do not prepare the Monitor again
   if start returned its scope. Never repeat start to open a screen. If start succeeded but Monitor
   preparation failed, report the display failure and continue the recorded Interview via `gil_open`.
   A display failure alone does not block Project work. A successful tool response does not prove
   that a screen is visible. In a new session, read `gil_context` once; its first block
   `[지금 바로 할 호출]` is the next call. Resume there instead of starting again.
   **Keep the in-host MCP App present:** successful Project calls check recent App display reports.
   When a receipt supplies `show_gil_monitor(scope_id)` because the Monitor is absent/hidden/unknown,
   call it once before further work, without asking the user to open a Companion. For an explicit
   display check use `gil_monitor_status(project_root)`; if `should_open` is true, prepare that root
   and show the returned scope. `opening` is a 30-second grace, not proof of display: continue without
   polling in a tight loop. Fresh `visible` reports (15-second lease) distinguish fullscreen/inline;
   preserve an inline choice. Two unanswered show requests stop automatic attempts (`unconfirmed`):
   report the display problem instead of adding cards indefinitely. A fresh visible report resets
   that retry budget. Status covers this Project on this MCP connection. If a Host shares that
   connection across chats, it cannot identify which chat contains the App. App reports do not prove
   OS foreground visibility or that the human has looked.
   `project_root` is always the exact string the Host, `gil_context`/`gil_status` or the last
   receipt gave; copy it verbatim and never retype it (retyped Korean paths change their letters).
2. **Agree, then finish Interview.** Interview clarifies and agrees scope; it never changes
   project Artifact files. Synthesis waits for the user's reply to its proposed statement;
   silence, earlier requests and the agent's own judgment are not approval. Do not ask again for
   approval already given to that exact proposal. Outcome judges the approved scope; do not leave it
   open until the whole project is finished. Choose the next call from the **current recorded state**,
   not from the beginning of the sequence:

   | Current state | Next call |
   | --- | --- |
   | New project, Monitor shown, no step open | `gil_open` Question with its action contract |
   | Question is open | Ask the user, end the turn, then `gil_close` with their actual response |
   | Interpretation is open | `gil_close` with what is settled and what is unresolved |
   | Synthesis is open and the user has not answered | Show the statement (approved-plan profile: first `gil_plan_propose`), end the turn and wait; no project file changes |
   | Synthesis is open and the user approved its statement | `gil_close` with `approved: yes` (approved-plan profile: the exact `close_report`) |
   | Synthesis is open and the user declined | `gil_close` with `approved: no`, then `gil_open` Question |
   | Synthesis is closed and Outcome has not been opened | `gil_open` Outcome |
   | Outcome is open and its verdict/next direction are ready | `gil_close` Outcome; do not open it again |
   | Outcome is closed but the Interview Cycle is still open | `gil_close` with the Cycle Report; there is no extra Report step to open |
   | Interview Cycle is closed and the agreed execution work remains | `gil_open` Experiment; never `gil_start` an existing project |

3. **One checkable question at a time.** An Experiment handles one independently checkable
   question. Open a small action contract (`objective`, `next_action`, `done_when`), perform its
   `next_action`, then `gil_close` when `done_when` is met, before further project file changes or
   final project delivery. A new request does not extend an already completed contract. Deliverable
   writing, implementation, calculations and tests that change project files belong in Experiment
   Verify, never an open Interview. Web research without project file changes needs no Verify.

   | Current state | Next call |
   | --- | --- |
   | Experiment Cycle just opened | `gil_open` Define: one question and its success criterion; no project file changes yet |
   | Define is closed | `gil_open` Hypothesis, then `gil_close` it with expectation, rationale and guardrail |
   | Hypothesis is closed | `gil_open` Verify. **Wait for its success** before changing project files |
   | Verify is open | Do the contracted work, save the observation as project files, `gil_close` with `evidence_files` |
   | Verify is closed | `gil_open` Analysis, `gil_close` with a nonblank `interpretation` of the saved evidence |
   | Analysis is closed | `gil_open` Outcome, `gil_close` with `verdict`, `lesson`, `next_direction` |
   | Outcome is closed but the Experiment Cycle is still open | `gil_close` with the Cycle Report |
   | Cycle closed `success` | `gil_open` the next Experiment (next independent question) or Interview. When no work remains and the project has a Chain, the Closing Interview and Chain Report follow; a chat summary alone never completes the project |
   | Cycle closed `failure` · `revisit` | When the Core receipt requires Monitor approval, open Monitor and wait for the user's “보존 후 되돌아가기” button after reviewing file changes; do not call App-only tools or invent consent. After confirmed recovery, `gil_open` a new Cycle. Otherwise follow the connected Core receipt; never bypass a product refusal |

   A closed Hypothesis or a prepared Verify contract does not mean Verify is open. Do not change
   project files first and then open Verify to record work already performed. This also applies between
   cycles: finish the old cycle and reach a successfully opened Verify before the next file change.
   Multiple related searches, edits and tests within the same unfinished Verify are valid.
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
   meaningless markers to satisfy the gate. Image declarations `not_needed`/`missing` do not
   waive ordinary file evidence. If evidence is refused, save truthful evidence while Verify
   remains open, then submit the corrected close.
   Accepted: `evidence_files: evidence/load-test.md` where that file holds the measured numbers.
   Refused: an empty file, a file unchanged since the baseline, or the chat summary alone.

   **Make the result visible — without being asked.** A Verify Report is read later by people
   who were not there; a picture of the real result is the fastest handoff. Before closing a
   Verify, decide which visual evidence fits the work and produce it as a static PNG inside the
   project, using whatever the Host offers (Python matplotlib/pandas, Node canvas, Graphviz,
   Mermaid CLI, a headless Chromium/Playwright or the ordinary browser's screenshot):

   | What the Verify observed | Visual evidence to attach |
   | --- | --- |
   | Numbers, measurements, datasets | A plot drawn by code from the actual data (bars, trend, distribution) |
   | Structure, flow, dependencies | A diagram generated from the real structure (graph, sequence, module map) |
   | A web page, UI, rendered document | A screenshot of the real page in a headless or ordinary browser |
   | Files or settings before/after | A rendered comparison table or diff image, with the raw files in `evidence_files` |
   | Build/test runs | A results table image; keep the log file as ordinary evidence |

   Visual evidence shows something that exists in the world. An AI-generated or decorative
   picture is never visual evidence; `source_kind` is `captured` (real screen/page),
   `generated` (drawn by code from data) or `provided` (supplied by the user), truthfully.
   Save the PNG in the project, list it in `evidence_files` too, and attach it with
   `visuals.attachments`/`uses` and `declaration: {state: "attached"}`. Choose `not_needed`
   only when nothing visual exists to show (byte equality, an exit code, one number) and say
   what replaces the image: `reason: "두 파일의 sha256 일치만 비교했고 화면이 없다"`. Refused:
   one-word or boilerplate reasons (`없음`, `불필요`, `not needed`), and `not_needed` while an
   image file is listed in `evidence_files`. Use `missing` with `reason` and `follow_up` when the
   picture could not be produced; never relabel missing as unnecessary.
   After Verify is closed and before the Cycle Report, create no project files: integrated reports
   and deliverables are saved inside Verify, or in the next Experiment's Verify.
5. **Learn from that evidence and close.** After Verify, write a nonblank `interpretation` in
   Analysis and a nonblank `lesson` in Experiment Outcome. Core links `evidence.verify_ref` and
   `learning.analysis_ref` to the actual current lineage; do not supply different references.
   **Cite the pictures.** When the Verify attached visuals, the Analysis and the Experiment Cycle
   Report must cite at least one of them (Core refuses otherwise, listing the citable assets):
   send `expected_owner` and `visuals` together, `attachments: []`, no `declaration`, and
   `uses: [{source: {kind: "stored", owner: {kind: "step", ref: "<verify ref>"}, asset_id, sha256},
   caption, alt, purpose}]` with the values from the Verify receipt or `gil_report_visuals`.
   Outcome may cite them too. Finish Analysis, Outcome and the Cycle Report before the next
   independent problem or final delivery. Deliver links to the saved evidence/deliverable
   alongside the chat summary, and name the attached visuals.
6. **Use the actual close contract.** Close both Steps and Cycles with `gil_close`; the legacy
   `gil_cycle` cannot accept a Report. Format-4 steps (including Verify) and format-5 non-Verify
   reports without stored images use `{project_root, report}`. Every format-5 Verify uses
   `{project_root, report, expected_owner, visuals}`, even without images. Supply both
   `expected_owner: {kind: "step", ref: "<actual open step ref>"}` and `visuals`; with no images,
   use `visuals: {attachments: [], uses: [], declaration: {state: "not_needed", reason: "<what replaces the image>"}}`.
   Format-5 non-Verify reports citing stored images also use both fields, per the connected schema.
   Keep `evidence_files` in the Report in every Verify. Available capabilities and project format,
   not the OS name, determine the input shape. Never migrate an existing project merely to follow
   these instructions. The rules behind every visual refusal are one topic: `gil_help` with
   `topic: report/visuals`.
7. **Approved plans (only where recorded).** A project uses the approved-plan profile only when
   its `gil_context` says `승인 계획 프로필` or `gil_plan_read` answers `supported: true`, and the
   connected `tools/list` offers `gil_plan_propose`/`gil_plan_read`/`gil_plan_request_interview`.
   Otherwise (legacy projects, default builds, tools absent) never require `gil_plan_*`; use the
   ordinary Synthesis. Do not infer this from the OS or the install kind.
   - In an open Interview Synthesis, call `gil_plan_propose` with `scope` (`goal`, `in_scope`,
     `out_of_scope`, `constraints`, `success_criteria`; write `없음` rather than leaving one empty),
     ordered `items` (`id`, `question`, `done_when`) and `synthesis` (the Synthesis Report fields
     such as `statement` and `basis_refs`). `exploration_checklist` lists the item IDs in the same
     order, one per line. Never write `approved`, `plan_revision` or `plan_digest`: Core adds them.
   - Then **end the turn and wait for the Monitor button.** Chat agreement is not approval. While
     waiting, make no project file changes or re-proposal. Check approval only with `gil_plan_read`.
   - `receipt.decision: approve` → `gil_close` Synthesis with `close_report` **unchanged**, then
     `gil_open`/`gil_close` Outcome, then `gil_close` the Interview Cycle Report, then `gil_open`
     Experiment: the first pending item binds automatically. `request_changes` → close with that
     `close_report` (`approved: no`) and ask what to change in a new Question. Editing an open
     proposal makes a new revision; an earlier approval is never reused.
   - In that Experiment's Verify, save `gil_plan_read.document` **byte for byte** (UTF-8) as a
     project file and name it in `plan_document`. The plan file is not execution evidence: save the
     separate observation of the real work and name it in `evidence_files`. Accepted:
     `plan_document: plan/A.md` + `evidence_files: evidence/A-result.md`. Refused: the plan file
     alone, or a plan file whose bytes differ from the export.
   - A scope change during an Experiment: `gil_plan_request_interview(reason)` preserves the
     request and closes nothing. Finish the current Step and Cycle truthfully: the next call is
     still the one the latest receipt names (the open Step's `gil_close`, then Hypothesis, Verify,
     Analysis, Outcome, and the Cycle Report only when that receipt offers it), never an early Cycle
     close. The request stays preserved meanwhile; there is no cancel or interrupt call. Then
     `gil_open` Interview and approve a new revision before any further Experiment. Never fabricate
     evidence to leave an unfinished Step.
   - When every approved item has executed, the project is not finished by a chat summary:
     `gil_open` Interview as the Closing Interview, propose the final `chain_verdict` for approval,
     then `gil_close` the Chain Report.
8. **Recover and keep the user informed.** A rejected Project action must be resolved before
   further Project work. Never treat a refused close as closed, backfill skipped steps or fabricate
   history. A confirmed input rejection means the call was not executed: check current state and
   submit one corrected input. Briefly tell the user the current step and immediate action. Monitor
   shows recorded progress, not private reasoning or proof that every external action used GIL.

   **Resume after a Monitor decision:** when the user says “continue”, “clicked” or “done”, or
   the Monitor sends a continuation message after an approval/recovery wait, first call
   `gil_context` once for the same selected `project_root`. The earlier wait may now be stale.
   Read the current recorded next call before asking for another click. Chat text itself is not
   approval: if Core still requires approval, keep waiting; if recovery completed, follow Core's
   next call without repeating restore/revisit or reopening a Cycle already opened. Never require
   the user to repeat a special phrase such as “눌렀어”. Do not poll while waiting for user input.

   | Situation | Recovery |
   | --- | --- |
   | `gil_start` refused because a project exists | Read `gil_context` once and continue there; never start again |
   | Close response lost or uncertain | Read `gil_status`/`gil_context`; close again only if the step is still open |
   | Plan decision or save result uncertain | `gil_plan_read`; never assume approval or re-propose blindly |
   | Approved earlier, proposal edited since | The old approval is gone; wait for the new revision's button |
   | Asked to re-interview during an Experiment | The request is preserved and closes nothing; keep following each receipt's next call and close the Cycle only when its receipt offers the Cycle Report, then `gil_open` Interview |
   | `gil_revisit` refused | If Monitor approval is required, show Monitor and wait for its button. If protected files differ or the platform is unsupported, preserve files and report the limitation. Never edit records, fake success or loop restore. A durably approved interrupted recovery can be resumed by `gil_revisit` when the receipt explicitly says so |
   | Refused `evidence_files` | Save truthful evidence while Verify is open, then resubmit the close |
   | Monitor cannot be shown | Report it and continue the recorded step; the record is unaffected |

   Do not loop a tool to confirm a fact that a receipt already states, and never promise that a
   screen, button or save "always works": report what was observed.
