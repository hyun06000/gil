## GIL shared workflow

### Scope and guidance on Codex and Claude Code

- Ending a chat turn is not GIL project completion. Report the recorded state truthfully; never fabricate approval, evidence or a completed Chain.
- The user may pause, change topic, request a chat report, or wait for Monitor approval at any stage. No special pause phrase, exact final question, or `gil_await_user` call is required to end a turn. Do not create a Question, Interview or Experiment merely to make ending a turn possible.
- When continuing a requested GIL workflow with an unknown goal, open the first Question with the objective “ask the user for the goal”, then ask and wait for the actual answer.
- `gil_await_user(project_root, question)` is an optional read-only check for an open Question or pending Synthesis proposal. It does not authorize turn completion or register a wait. Recovery/revisit approval and cycle boundaries do not require it. If it refuses, read the current context; do not manufacture a supported Step just to call it.
- GIL installs no global, project-local, Plugin or Skill hooks. Do not create hooks, enroll folders in a Host guard registry, wrap ordinary shell commands, or request elevated permissions on GIL's behalf. Use the Host's normal execution and approval flow.
- These instructions apply only when the user is working on an explicitly selected GIL project. Selecting an existing GIL project and requesting changes to its deliverables selects that workflow again, even in a new session and without saying "GIL". An enabled Plugin, another project's records, a cwd alone, or old registry entries do not make an unrelated task a GIL task. For unrelated chats, development, installation or file-free research, do not call GIL tools or demand GIL Steps just to proceed.
- **Language.** Answer the user in the user's language. GIL's receipts, guidance and the Monitor speak the Project's remembered locale, which comes from two optional fields: `gil_start(locale)` (BCP 47, for example `en-US`, `ko-KR`, `zh-TW`, `ja-JP`) and the Monitor App's own Host locale sent with its presence. Supported: `en`, `ko`, `zh-Hans`, `zh-Hant`, `ja`; anything else falls back to English, and a Project that was never told a language speaks the Core default. When the Project's locale is unknown and the user's language is clear, pass it as `locale` on `gil_start`; an existing Project keeps the language of its receipts. The block headings quoted in these instructions appear in that language.
- Project file discipline is guidance, not an enforced file lock. The Core still validates its own transitions, approvals, reports, evidence and dirty boundaries; it does not intercept Bash, Write or Edit. Never claim that prompts prevent all out-of-Verify writes or protect a project from other sessions.
- File-free web research and unrelated outside work remain allowed. They do not close a requested GIL workflow. Honor user cancellation without changing project records; use `gil_context` to resume the recorded position when requested.
- `show_gil_monitor` is a display request. Read `gil_monitor_status` before claiming visible/fullscreen. If `display_mode=inline`, say the Host kept the preview inline and guide the Host widget's “show in another panel”/expand control. Its `automatic_fullscreen` says why, as the App itself reported it: `refused` means the App received an explicit response requiring a fresh press inside its frame (suggest clicking the Monitor card or the Expand Monitor button without promising success); `error` means a timeout, connection failure or unclassified error, not evidence of a Host policy (check the connection or suggest a manual retry); `declined` means the Host applied inline to the automatic request; `unavailable` means the surface does not advertise fullscreen; `manual` means the user chose. Never claim that more retries guarantee fullscreen, and never call `show_gil_monitor` again to force it.

These rules apply to every GIL connection, with or without a Plugin, on every operating system.
Use only the user's selected absolute `project_root`. Each receipt's text guidance names **one next
call or one wait** for continuing GIL; when continuing, do it, then read the next receipt. Read `structuredContent` too, and follow the
actual recorded position and the connected tool schema. Never repeat a call that already succeeded.

**New-session project discovery — before producing files.** For a request to create or change
artifacts in the user's selected task folder, read its project instructions and check for GIL
record markers (`.gil` at that root, or a GIL project continuity instruction). This is a bounded
read-only check of the selected task folder, not a scan of HOME, registries or other projects.
A plain folder with no GIL evidence stays an ordinary project; never call `gil_start` merely to
check membership. Do not enroll a folder because a GIL plugin is enabled. File-free research and
unrelated questions need no discovery or GIL Steps. If the selected root is unclear, resolve it
with the user before file work; do not guess an ancestor or another session's project.

For an existing GIL project, call `gil_context(project_root)` once **before any file-producing
tool**, then follow the actual receipt and current plan/contract. "Add charts to this report"
is project continuation, not permission to skip to image generation. A new request outside an
old open Verify's unfinished contract requires the legal plan/Cycle/Step transition first.
A new session alone does not require a new Cycle, and a closed Chain or a refusal is not permission
to work untracked. Missing/corrupt/inaccessible records or a disconnected MCP require an explanation
and resolution before project writes, not silent fallback, `gil_start`, manual `.gil` edits or
fabricated history. Never copy records or bind a different checkout/worktree to an old root by guess.

New starts prepare local Codex/Claude continuity instructions before the initial Snapshot.
Existing projects gain them at the next successful Verify open; context/status remain read-only.
Preserve existing instruction bytes and obey a `pending/review_required` receipt; do not overwrite
custom instructions or alter old Snapshots to hide a missing guide. If a Verify is already open,
any guide adoption must stay within that write boundary and preserve the original instructions.
Guide installation alone is not evidence that the requested report, chart or code work is complete.
No global/project/Plugin/Skill hooks or global instruction edits are part of this flow.

**Quick rules — read these first; the numbered rules below only add detail.**

1. A GIL project is not finished in chat. When continuing that project, follow the last receipt's next call **or wait**.
   Do not close a step before its completion condition is met. If no receipt exists yet, read
   `gil_context` once (existing project) or call `gil_start` once (new project).
2. In an explicitly headless session, follow the recorded next action or wait. On desktop after `gil_start`: call the `show_gil_monitor(scope_id)` the receipt names, then
   `gil_open(kind: question)` with `objective`, `next_action`, `done_when` in its `contract` string.
3. A Question is answered by the user, not by you. Ask and wait. Close it only with the user's actual reply;
   never paste the original request as the answer. Synthesis needs explicit approval;
   in the approved-plan profile, register its proposal with `gil_plan_propose` and wait for the Monitor button.
4. Project file changes and deliverable production belong inside an open Experiment **Verify**;
   save its results as project files named in `evidence_files`. File-free web research, reading
   existing evidence at any stage, and unrelated outside work remain allowed. Those activities
   and a chat summary do not complete the requested GIL workflow.
5. A refusal closes nothing. For a repairable input error, fix the field the refusal names and resend
   the same call once; the refusal already shows the current input skeleton, so read a help topic only
   when the rule itself is unfamiliar. An approved-plan refusal (`Approved plan:` / `승인 계획:`) is a
   state, not an input error: read `gil_plan_read` and follow it (rule 8). Otherwise follow the stated
   recovery or wait. Never repeat `gil_start`, skip a step, or invent approval.

**Project file boundary — at every stage.** Creating, modifying, deleting, renaming or moving
Artifact files inside the selected `project_root` requires a successfully opened Experiment
Verify. This includes importing files made elsewhere, DOCX/PDF production, chart PNGs, rendering,
test output and temporary files written inside the project. Check the actual open Step before
launching a tool that can change these files; if the position is unknown, read `gil_context` once.
PNG attachments must be static: each dimension 1–8192 pixels, at most 16,777,216 total pixels,
and at most 16,777,216 bytes (16 MiB) per file. A close accepts at most 8 new attachments and
33,554,432 bytes (32 MiB) total. PNG decoding/integrity and resource checks still apply
(decoder memory 32 MiB, decoded output 128 MiB, at most 4096 chunks). Split or resize long
charts inside the open Verify and check readability. Analysis and the Experiment Cycle Report
must cite the Verify's attached assets using the confirmed stored source metadata.

Finish all writers before closing Verify. A prepared contract or approved plan is not an open Verify.
Outside Verify, keep project Artifact files unchanged: this applies to every Interview (including
Closing), Define, Hypothesis, Analysis, Outcome, Cycle/Chain Reports, approval waits, boundaries
with no open Step and closed Chains. Follow the current legal next call to reach Verify; never
skip steps, invent approval or backfill an execution that happened earlier.

**Project deliverables belong in the project by default.** Save the requested report, code,
charts, intermediate renders and production scripts under `project_root` during an open Verify,
so Snapshot and `evidence_files` can track them. A Host scratchpad/default output directory is not
an alternative delivery location merely because Interview, approval or Verify is unfinished.
Before invoking a file-producing tool, choose its output path inside this project explicitly.
If external creation is genuinely necessary (for example, a tool cannot write to the project),
first tell the user the concrete reason and destination and explicitly say: "Files created there
are not tracked by GIL; they are not included in this project's Snapshot or evidence." Do not
claim those external files as GIL-verified delivery. If they are needed as project evidence, import
and verify them inside a later open Verify; an external exception never completes the Interview,
approval, Experiment or Closing on its own. Do not create an external file just to evade a gate.

Work outside the project and web research that leaves project files unchanged are outside this
file boundary; do not require a GIL Step just for them. Unrelated external work remains allowed.
Existing evidence can be read in any stage.
When bringing an external result into the project, open Verify first, record its actual provenance
and verification, and cite the imported file in `evidence_files`; do not claim it was created there.
GIL's own record/approval writes and explicitly authorized recovery use their existing GIL tools
and contracts; never edit `.gil` by hand. These are agent instructions, not an OS write lock.
The current dirty gate detects changes at GIL boundaries; it does not intercept other tools.

**Before closing, reconcile the world with what you know.** Inspect the available GIL world status
and changed paths, then compare the actual differences with the work you can explain from your
current session context. A path list alone does not explain its contents or author. A change you
cannot explain is an unknown-origin change, including your own forgotten edit; do not assume the
user made it. Pause closing, summarize the differences and their effects, and ask the user about
origin and intent. Never silently accept, discard or restore such changes.
Within Verify, the agreed general workflow requires explicit Monitor acceptance or restoration
before closing. That general Monitor approval path is not yet connected in the current product:
report the limitation and keep closing paused; do not invent a button/tool, substitute a chat reply
for Monitor approval, or reuse failure-recovery approval for this purpose.
Outside Verify, ask whether the change is needed, preserve needed contents and intent before any
restoration, and follow only the recovery path supported by the current product. Confirm the world
matches its baseline before closing the same node; perform the needed change in the next Cycle's
Verify. Do not jump to Verify with unresolved changes or use approval to grant writes to another
Step. Do not invent a preservation store or overwrite existing files; if safe preservation or recovery
is unavailable, explain the limitation and pause.
For `.gilignore`, use the existing explicit tracking-policy approval contract when available; a disk
edit is a proposal, not activation. Policy history belongs to the forward-moving Journey even though
the file is visible in the filesystem. Never roll it back with the world or add duplicate general-file
approval. Its new policy engine is not yet connected to product capture/restore; do not claim it is.
World records and the agent's recorded actions and learning must correspond in one version.
This does not require author tracking, write hooks or a new Snapshot for every node.

1. **Start and show (desktop display mode).** Start a new project once. A successful `gil_start` receipt carries the exact
   next call, `show_gil_monitor({"scope_id": …})`, in its opening guidance and its final text block:
   `structuredContent.next_call` carries the same name and arguments for structured-only Hosts;
   that single call is the next call on every Host (Codex, Claude Desktop's Claude Code tab, Plugin or
   direct registration alike), before the first `gil_open`. Copy its argument verbatim and do not
   skip it because no card is visible yet or because the body also names `gil open` for later. Do not
   prepare the Monitor again if start returned its scope. Never repeat start to open a screen.
   If the Host rejects/loses the start response (including output-schema validation), creation may
   already be committed: read `gil_context` once, then follow its `structuredContent.next_call`.
   If the project exists but no scope was delivered, call `gil_monitor_prepare(project_root)` once
   and show that returned scope; a missing scope in the lost response is not a reason to omit Monitor.
   If
   start succeeded but Monitor preparation failed, report the display failure and continue the
   recorded Interview via `gil_open`. A display failure alone does not block Project work. The App
   requests fullscreen by itself (at connect, again when the Monitor renders, bounded retries, then
   the user's first click); never call `show_gil_monitor` again to force fullscreen. A successful
   tool response does not prove that a screen is visible. In a new session, read `gil_context` once
   and find the next-call heading in the reply's language (`[Call now]` in English, `[지금 바로 할 호출]`
   in Korean); the action under that heading is the next call, even when a resume summary such as
   `[Resume from saved records]` comes before it. Resume there instead of starting again.
   **Start kind (only when the connected `gil_start` schema has a `start_kind` input with both
   `tracked_scope` and `image_reports`).** The two are separate kinds of new project and a project is
   never converted from one to the other. Before starting in a folder with no `.gil`, tell the user
   both options with their limits as the tool description states them, and pass the user's own choice
   as `start_kind`; never choose for the user. `tracked_scope` begins the start preparation below;
   `image_reports` creates the project at once and its receipt names the `show_gil_monitor` call. If
   `gil_start` answers `code: start_kind_required`, nothing was created: ask the user and call it once
   more with their choice. Any other `start_kind` refusal changed nothing: report its words, read
   `gil_context` or `gil_start_status` as it says, and do not try the other kind, edit `.gil` or
   delete records to make the start pass. An existing project or preparation keeps its own kind.
   **Start preparation (only when `gil_start` answers `phase: preparing`; development tracking-policy
   builds).** No Chain, Cycle or Snapshot exists yet and no project file is archived. Explain, grouped by
   folder, what will be tracked and excluded and the archive/restore impact; keep general and
   secret-candidate recommendations preselected and project-specific ones unselected unless the user
   chooses them (`.gitignore` is reference only). Call `gil_start_prepare` with the exact `.gilignore`
   text, then `show_gil_monitor(scope_id)`, and wait for the user's button in the Monitor. Chat consent,
   editing `.gilignore` or your own input is never approval; without a Monitor the start stays waiting.
   Do not create project files while preparing. A nested `.gilignore` holds the start: explain it and
   tidy it only with the user's consent. After a lost reply, restart or reconnect read `gil_start_status`
   once; if it says `approved` but not started, ask the user to press the Monitor's retry button — no model
   tool applies a start. A detected condition change (`scope_changed`, `condition_changed`, `policy_file_changed`) needs a new
   proposal and approval; an unstable observation or a storage failure keeps the approval for a Monitor retry. On `combine_required` tell the user once that you will combine the approved rules and
   the file now on disk (do not ask whether you may), write the combined text keeping the engine's
   last-matching-rule order and send it with `combine_with_revision`; it needs its own Monitor approval.
   Cancel only on the user's explicit request (`gil_start_cancel`); a closed app is not a cancel, and a
   canceled preparation restarts only when the user explicitly asks. In such a project `gil_restore`
   and revisits are refused until latest-policy restore exists; never work around them.
   **Scope change (development tracking-policy builds; a started format-10 project).** Changing what is
   tracked is reviewed in an Interview: discuss the reason, what becomes newly tracked or excluded and
   the archive/restore impact, and write no project files. After that Interview is closed, call
   `gil_scope_propose` with the exact new `.gilignore` text, then `show_gil_monitor(scope_id)`, and wait
   for the user's decision in the Monitor. Approval applies the new policy the moment it is saved; the
   policy-file sync and the baseline archive are separate results. Never describe a failed archive as
   an unapplied policy, or a saved approval as an archived world. `gil_scope_status` reports three
   distinct values: the applied policy, the last confirmed world with the policy that confirmed it, and
   whether an event waits for its baseline archive. While it waits (baseline archive needed — next
   Experiment on hold) an Experiment is refused and an ordinary Interview may be opened, carried through
   and closed on its own decision. Do not approve a Chain-closing Synthesis (`chain_verdict` with
   `approved: yes`) before the baseline is confirmed; Core refuses it. The archive retry, the archive
   halt and its release are the user's Monitor buttons: no model tool performs them, and you never retry
   after a reconnect, a restart or by repeating a request. While a halt marker is present every open and
   close is refused: read the status, tell the user and wait. A release only removes the marker; wait
   for the user's explicit next instruction. Unconfirmed changes on continuously tracked paths block an
   approval even when the proposal would exclude those paths. On `held_three_way` or `base_undecided`
   show the user the exact difference and do not work around it. After a lost reply read
   `gil_scope_status` once instead of repeating an approval.
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
2. **Agree, then finish Interview.** An unknown goal is the reason to open Question, not a
   prerequisite to opening it. Use a clarification contract, for example `objective: clarify the
   user's goal`, `next_action: ask the user for the desired result and completion criteria`, and
   `done_when: the user has answered or identified what remains uncertain`. Open Question before
   asking, then wait. The optional `gil_await_user` check is not needed to yield. Later record the user's actual reply with `gil_close`; never invent the goal.
   Permitted outside work or file-free web research does not close that Question or replace an
   explicitly requested GIL project's Interview, approval, and eventual Closing. Resume the recorded
   next call after the user answers; do not treat a chat-only answer as completed project delivery.
   Interview clarifies and agrees scope; it never changes
   project Artifact files. Synthesis waits for the user's reply to its proposed statement;
   silence, earlier requests and the agent's own judgment are not approval. Do not ask again for
   approval already given to that exact proposal. Outcome judges the approved scope; do not leave it
   open until the whole project is finished.

   **Deepen the interview before proposing.** A vague wish ("make it convenient", "analyse it
   well", "automate everything") is not a completion criterion. In Question, turn it into the real
   situation, the inputs, an example of the wanted result and a counter-example of the unwanted one,
   what is in and out of scope, the constraints, the priority and a completion condition the user
   can judge. Reuse answers already given; ask first the one open point that changes the next
   decision, one at a time; a request that is already concrete gets no extra question, and a fixed
   number of questions proves nothing. If the user does not know how, offer concrete options and
   their differences instead of making them design the procedure. In Interpretation, separate what
   the user's answers confirmed, your own interpretation or recommendation, and what is still
   unknown; if an open point still matters for the next item or its approval, open another Question
   (Interpretation → Question is legal) instead of hiding it in the proposal. Detail:
   `gil_help(topic: "cycle/interview/clarify")`.

   **Propose short, in small items.** The Synthesis statement (and, in the approved-plan profile,
   `draft.scope`/`draft.items`/`draft.synthesis`) leads with the goal, this scope, the key
   constraints and the decisions to confirm, as short statements the user can accept or correct at
   a glance; the ordered items follow. Each `question` is one problem that can be performed and
   checked, and each `done_when` is that problem's observable completion; say the needed inputs, the
   prerequisite, the first action and the check beside it, briefly. The first item must be
   startable now with the material, permissions and scope at hand; when a prerequisite is
   unconfirmed, a small item that checks it comes first, and later items that need an earlier
   result say so instead of assuming it. Do not list feature names or bundle several independent
   goals in one item, and do not hide several separately-refusable decisions in one statement. Do
   not repeat the interview transcript or the same background; link each statement to the actual
   Question/Interpretation refs in `basis_refs`. Brevity drops no constraint, risk, uncertainty or
   approval scope, and the existing scope/items/synthesis shape is unchanged. Under the exploration
   contract (rule 3) the items are the requirements themselves: each `question` is a question to
   answer and each `done_when` the criteria the user will judge it by. Agree on the data scope,
   the quality of evidence and how limits will be reported; do not fix the answer or enumerate the
   experiments and their order in advance. Choose the next call
   from the **current recorded state**, not from the beginning of the sequence:

   | Current state | Next call |
   | --- | --- |
   | New project, Monitor shown, no step open | `gil_open` Question with its action contract |
   | Question is open | Ask and wait, then `gil_close` after their actual response |
   | Interpretation is open | `gil_close` with what is settled and what is unresolved |
   | Synthesis is open and the user has not answered | Show the statement (approved-plan profile: first `gil_plan_propose`), wait for explicit approval; no project file changes |
   | Synthesis is open and the user approved its statement | `gil_close` with `approved: yes` (approved-plan profile: the exact `close_report`) |
   | Synthesis is open and the user declined | `gil_close` with `approved: no`, then `gil_open` Question |
   | Synthesis is closed and Outcome has not been opened | `gil_open` Outcome |
   | Outcome is open and its verdict/next direction are ready | `gil_close` Outcome; do not open it again |
   | Outcome is closed but the Interview Cycle is still open | `gil_close` with the Cycle Report; there is no extra Report step to open |
   | Interview Cycle is closed and the agreed execution work remains | `gil_open` Experiment; never `gil_start` an existing project (exploration contract: the Experiment takes its requirement link in `contract`, rule 3) |

3. **One checkable question at a time.** An Experiment handles one independently checkable
   question. Open a small action contract (`objective`, `next_action`, `done_when`), perform its
   `next_action`, then `gil_close` when `done_when` is met, before further project file changes or
   final project delivery. A new request does not extend an already completed contract. Deliverable
   writing, implementation, calculations and tests that change project files belong in Experiment
   Verify, never an open Interview. Web research without project file changes needs no Verify.

   **Define is a yes/no proposition, not an activity.** State one bounded question that can be
   answered yes or no from evidence. “Inspect dates” and “check whether dates work” are tasks,
   not the problem. Ask “Can every date in the specified files be converted without information
   loss under the agreed rules?” Fix the criterion before observing the result. Completing an
   inspection or writing a report does not make the proposition true. Insufficient evidence is
   not a supported no. Exploratory analysis does not assume a desired answer in advance.

   **Two execution contracts — the record says which.** A Chain's contract is recorded when the
   Chain opens and never changes. Read it from `gil_plan_read`: `contract: exploration` means the
   exploration contract below; a response without that field means the item-binding rules that
   follow it. An existing Chain keeps its contract, and a connected Core that does not report
   `contract: exploration` does not provide it: opening a new Chain there does not enable it. Do
   not bypass item bindings or repeatedly revise a plan to simulate the other contract; report the
   limitation and follow the legal receipt.

   **Exploration contract (only when `gil_plan_read` reports `contract: exploration`).** The
   approved `items` are stable requirements — the question to answer and its judgment criteria —
   not an execution queue. Nothing is bound automatically and no order is imposed:
   - Open each Experiment with its requirement link in `contract`:
     `gil_open(kind: "experiment", contract: "plan_revision: …\nplan_digest: …\nrequirement_id: …\nrelevance: …\nreadiness: …\nuncertainty: …")`.
     Copy `gil_plan_read.experiment_link`, keep one `requirement_id`, and write the three
     statements yourself from what has been observed: why this question serves that requirement,
     which inputs and earlier results are actually at hand, and which uncertainty it reduces. The
     Core checks the reference (this Chain, the active approved revision, an approved requirement);
     it does not verify what the statements mean. An unapproved requirement, a superseded revision
     or another Chain's approval is refused — correct the link from `gil_plan_read`, never invent
     approval.
   - Define is one yes/no proposition that contributes to the linked requirement and can be judged
     now, not a copy of the requirement. Several different small Experiments may serve one
     requirement, chosen from observation; refuting evidence is kept and may reopen a candidate.
   - One Experiment's `success` does not complete a requirement. `gil_plan_read.requirements` lists
     the linked Experiments and their recorded verdicts; whether a requirement is answered is
     decided only in the Closing Interview the user approves. Report an answer that could not be
     reached as not reached, never as a supported no.
   - A failed Experiment still requires the explicit revisit, even with no file change. After the
     restore, open a different small Define linked to the same (or another approved) requirement;
     the failed Define need not be repeated.
   - Ground the next Experiment, and the answer, evidence and limits reviewed per requirement in
     the Closing Interview, in the original records — each linked Experiment's Define, Hypothesis,
     Verify observation, Analysis and Outcome lesson. Do not keep a separate running “current
     answer” or provisional summary per requirement as a premise for later Experiments: a guess
     that is not re-checked against its evidence hardens into one. When `tools/list` offers
     `gil_report_read` and `gil_evidence_read`, read only what the next decision needs:
     `gil_plan_read.requirements[].experiments[].cycle_ref` → `gil_report_read(owner: {kind:
     "cycle", ref})` for Step references and the Cycle Report → `gil_report_read(owner: {kind:
     "step", ref})` for one stored Report → `gil_evidence_read(owner: <that Verify>, path)` for a
     file it declared, read from that Verify's Snapshot, never from the working folder, where a
     later Verify or a revisit may have changed the file of that name. `owner: {kind: "chain",
     ref}` lists an earlier Chain's Cycles. Failed and revisited Experiments read the same way.
     These reads return stored text: existing in the record is not being true or still valid,
     disagreeing records are both shown, and nothing is summarized or judged. An answer with
     `truncated: true` left part out to stay small and names the input that reads it (`more`,
     a field's `read`, `next_offset`); left out never means absent. Without those tools the
     same Reports are in `gil_story`.
   - Do not re-propose the plan merely to split, re-order or change the method of experiments.
     When the requirement, scope, permission or judgment criteria themselves must change, do not
     disguise that as a Define: agree on it in an Interview (`gil_plan_request_interview` preserves
     the request during an Experiment).

   **Define the smallest problem performable now.** From the interview and the agreed item for
   this Experiment (approved-plan profile: the bound `current_item`), Define infers the one smallest
   problem that can be performed and judged now; its `problem` and `success_condition` point at the
   same problem and completion scope as that item, never at the Chain goal or the whole proposal.
   (That same-scope rule is item binding. Under the exploration contract they point at one
   proposition contributing to the linked requirement, as above.)
   Name the inputs already at hand, the first action, the observable result and where this
   Experiment stops; the next problem stays a separate candidate. "Smallest" is a meaningful
   experiment that can be observed, judged and stopped on its own — not a short sentence or one
   command: do not bundle independent goals (data check, cause analysis, chart and proposal) into
   one Experiment, and do not split a mere preparation that yields no checkable result of its own
   into a separate one. When the inputs or a prerequisite are not confirmed, confirming them is
   this problem. Under item binding, executing only part of a large approved item does not complete
   it; splitting or replacing an already approved item is not supported there, so say that plainly (rule 7:
   `gil_plan_request_interview` preserves the request), keep the record, and change no approved
   scope. Detail: `gil_help(topic: "step/define/smallest")`.

   | Current state | Next call |
   | --- | --- |
   | Experiment Cycle just opened | `gil_open` Define: one question and its success criterion — the smallest problem performable now (item binding: the same problem as the agreed item; exploration contract: one proposition contributing to the linked requirement); no project file changes yet |
   | Define is closed | `gil_open` Hypothesis, then `gil_close` it with expectation, rationale and guardrail |
   | Hypothesis is closed | `gil_open` Verify. **Wait for its success** before changing project files |
   | Verify is open | Do the contracted work, save the observation as project files, `gil_close` with `evidence_files` |
   | Verify is closed | `gil_open` Analysis, `gil_close` with a nonblank `interpretation` of the saved evidence |
   | Analysis is closed | `gil_open` Outcome, `gil_close` with `verdict`, `lesson`, `next_direction` |
   | The open Experiment Step's contracted action cannot be carried out (schema offers `unachieved_will`, format 8/9 only) | Write what was done, what blocked it and what stays unverified in the Report (Verify: save it and list it in `evidence_files`), then `gil_close` with `unachieved_will: <active Will>`. Continue with the remaining Steps; Outcome is `failure` · `close_cycle` |
   | Outcome is closed but the Experiment Cycle is still open | `gil_close` with the Cycle Report |
   | Cycle closed `success` | `gil_open` the next Experiment (next independent question; exploration contract: linked to the same or another approved requirement — the success completed no requirement) or Interview. When no work remains and the project has a Chain, the Closing Interview and Chain Report follow; a chat summary alone never completes the project, and that Interview's first message shows the result before asking to close |
   | Cycle closed `failure` · `revisit` | When the Core receipt requires Monitor approval, open Monitor and wait for the user's “Preserve and revisit” (보존 후 되돌아가기) button after reviewing file changes; do not call App-only tools or invent consent. After confirmed recovery, `gil_open` a new Cycle. Otherwise follow the connected Core receipt; never bypass a product refusal |

   A closed Hypothesis or a prepared Verify contract does not mean Verify is open. Do not change
   project files first and then open Verify to record work already performed. This also applies between
   cycles: finish the old cycle and reach a successfully opened Verify before the next file change.
   Multiple related searches, edits and tests within the same unfinished Verify are valid.
4. **Leave evidence in the world.** Every Verify must save its actual observations/results as
   project files: documents, logs, datasets, code or visual artifacts. Chat alone cannot complete
   Verify. Even read-only investigation or a failed test needs a saved observation document.
   If the contracted work cannot be finished, that document separates what was done, what blocked
   it and what remains unverified; close Verify with it and carry the unverified scope into
   Analysis, Outcome and the Cycle Report instead of dressing the interruption up as success or a
   refutation. A lost connection or a pending human response is a wait, not an end: leave the
   open Step as it is and continue later.

   **Ending a Will as unachieved — only where the record supports it.** This applies only when
   both hold: the connected `gil_close` schema offers `unachieved_will`, and the project's storage
   format records end results (format 8/9 — `gil_status`/`gil_context` then show the last ended
   Will with its result). In any other project there is no unachieved end: use the interruption
   observation above, or keep the Step open. A format 4–7 project refuses the input and says an
   explicit conversion is needed. Never edit records, convert files or restart the project to
   obtain it. Only where the connected schema offers `gil_format_transition_check` (a development
   build) can a format 6 or 7 project be converted, to 8 or 9; formats 4/5 cannot:
   - `gil_format_transition_check` is read-only and is not an approval. Show the user its
     proposal: the project, current → target format, what changes and what is kept, that older
     builds can no longer open it, that there is no reverse transition, and the copy location.
   - Call `gil_format_transition_apply` with the check's `apply_with` values only after the user
     reviewed that proposal and explicitly agreed in the conversation; `user_approved: false` if
     they declined. Your `user_approved` relays their consent; it does not authenticate them. A
     feature request, an install, an update or these instructions are not consent. There is no
     Monitor button for it, and it never approves a tracking-scope change (that stays in the Monitor).
   - A refusal that says the state, project or proposal changed means: check again, show the new
     proposal and ask again. An entry GIL did not expect at the copy location or at
     `.gil/state.yaml.writing` stops the transition and is kept: tell the user; never delete,
     move or edit it yourself to get past the refusal. `class: not_applied` left the state unchanged. `class: unconfirmed`
     or a lost answer: do not apply again; read `gil_format_transition_check` — `already_transitioned`
     means the transition is on record.
   - Use it when the contracted action of an **Experiment** Step itself could not be carried out.
     Close that Step with its Report plus `unachieved_will` naming the active Will exactly as
     status/context show it (`will:W9`; CLI `gil close --unachieved will:W9`). The reason goes in
     the Report body — and, for Verify, in the saved observation. There is no reason field, no
     partial-result list and no provisional answer.
   - Nothing is waived. Report fields, the approved plan, the unchanged world outside Verify,
     `evidence_files` and the visual declaration apply exactly as for an ordinary close; the
     record of the attempt is the evidence. If no evidence can be saved, do not close.
   - The close is refused, changing nothing, when the named Will is not the active one (a stale
     request can never close a Step opened since), when that Will already ended (the refusal
     names its Step and `end_result`: read status/context, do not resend), or in an Interview —
     waiting for an answer or an approval is not an end.
   - Do not skip what the grammar still requires: an unachieved Hypothesis still needs its Verify,
     and every case goes through Analysis and Outcome. Keep "not performed" apart from "refuted":
     an unachieved end is not an answer to the question, and an achieved Will can still end in a
     refuted question.
   - That Experiment now closes only as a failure: Outcome `verdict: failure` with
     `next_direction.action: close_cycle`, then the Cycle Report `failure` · `revisit`. A new
     Hypothesis in the same Experiment, an in-Experiment revisit and `success` are refused. Try
     again in the next Experiment after the recovery. An Experiment without an unachieved end
     keeps its ordinary repetition (next Hypothesis, sibling revisit).
   - The failure verdict is not a recovery approval and closes no Chain: the existing Monitor
     approval, target checks and product recovery limits apply unchanged.
   Include `evidence_files` in its Report: 1–32 unique safe project-relative file paths. Every
   listed file must be nonempty in the Snapshot, and at least one must be new or changed from the
   baseline. One file: `evidence_files: evidence/result.md`. Multiple files use this Report syntax:

   ```text
   evidence_files: |
     evidence/one.md
     evidence/two.md
   ```

   Record sources, inputs/method, observed results and limitations; never invent results or save
   meaningless markers to satisfy the gate. The Verify Report's `result` holds observations only
   (numbers, output, screens, file changes); what they mean for the hypothesis is the Analysis
   `interpretation`, and success or failure is the Outcome `verdict`. Image declarations `not_needed`/`missing` do not
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
   `visuals.attachments`/`uses` and `declaration: {state: "attached"}`. For each new PNG, use
   `attachments: [{key: "plot", path: "report/plot.png", source_kind: "generated", source_description: "actual method"}]`
   and `uses: [{source: {kind: "new", key: "plot"}, caption: "result", alt: "observed content", purpose: "evidence"}]`.
   Arrays and declaration are siblings inside visuals. Never put uses/key/reason inside an attached
   declaration or use `stored` with an attachment key. Read `gil_help(topic: "report/visuals")`
   for complete two-image JSON; CLI alias is `gil help visuals`. Choose `not_needed`
   only when nothing visual exists to show (byte equality, an exit code, one number) and say
   what replaces the image: `reason: "only the sha256 equality of two files was compared; there is no screen"`. Refused:
   one-word or boilerplate reasons (`none`, `없음`, `not needed`), and `not_needed` while an
   image file is listed in `evidence_files`. Use `missing` with `reason` and `follow_up` when the
   picture could not be produced; never relabel missing as unnecessary.
   After Verify is closed and before the Cycle Report, create no project files: integrated reports
   and deliverables are saved inside Verify, or in the next Experiment's Verify.
5. **Learn from that evidence and close.** After Verify, write a nonblank `interpretation` in
   Analysis and a nonblank `lesson` in Experiment Outcome. The interpretation does not repeat the
   Verify observation: it says what the observation means for the hypothesis and what remains
   uncertain, so observation and interpretation stay in their own Reports. Core links `evidence.verify_ref` and
   `learning.analysis_ref` to the actual current lineage; do not supply different references.
   **Cite the pictures.** When the Verify attached visuals, the Analysis and the Experiment Cycle
   Report must cite at least one of them (Core refuses otherwise, listing the citable assets):
   send `expected_owner` and `visuals` together, `attachments: []`, no `declaration`, and
   `uses: [{source: {kind: "stored", owner: {kind: "step", ref: "<verify ref>"}, asset_id, sha256},
   caption, alt, purpose}]` with the values from the Verify receipt or `gil_report_visuals`.
   Outcome may cite them too. Finish Analysis, Outcome and the Cycle Report before the next
   independent problem or final delivery. Deliver links to the saved evidence/deliverable
   alongside the chat summary, and name the attached visuals.

   **Write every Report short.** Lead with what this Step newly established (observation,
   judgment or decision) and the few facts that support it; point at saved evidence files or
   earlier records instead of pasting logs, tables or project background. Do not repeat the same
   content across fields, and do not restate a Step Report inside the Cycle Report or a Cycle
   Report inside the Chain Report: a Cycle Report carries the verdict, the lesson and the next
   direction; a Chain Report carries the approved verdict and the handoff. Keep failure causes,
   uncertainty, unverified scope and warnings for the next work — brevity never drops them. Do
   not shorten with undefined abbreviations or internal code names, and do not fill a field with
   "done", "pass" or a bare link. There is no length cap and closed Reports are never rewritten.

   **Show the result before asking to close.** When the work is done and the project moves to
   its Closing Interview (or any Interview that follows executed Experiments), the first message
   presents the result itself, in this order: what was made and what it is for → the actual
   deliverable and how to open it (a link to the saved file, the key numbers or table, the real
   screen or saved capture, the changed code and its test result) → the verification result and
   the remaining limits → the question whether to close. Do not open with the closing question
   alone, do not substitute a file list or "done" for the result, and do not ask "shall I show
   it?" or add a separate "did you see it?" step. Present partial results, no result and
   anything that cannot be shown truthfully. Read and present the recorded evidence; creating
   new files, captures or re-runs belongs to an earlier Verify, never to the Closing Interview.
6. **Use the actual close contract.** Close both Steps and Cycles with `gil_close`; the legacy
   `gil_cycle` cannot accept a Report. Outcome Step's `close_cycle` means submit the Cycle Report
   next; do not copy it into that report. Cycle `open_child` records a direction, it neither opens
   nor executes another Experiment. Follow the actual Cycle receipt for the allowed next kind.
   Approved Closing Interview uses `close_chain`; ordinary execution uses its own Cycle contract.
   After the Chain Report the Chain is closed: open nothing else inside it, and report the verdict,
   the deliverables and the remaining questions to the user. Only a new topic starts the next Chain,
   with one explicit `gil_open(kind: chain)`; there, read `gil_context` once, then open its Question.
   The new Chain is approved in its own Opening Interview and reuses no earlier approval, plan or
   re-interview request. Never `gil_start` again.
   A project's storage format number says whether it holds the visual registry: an even format
   has none, an odd one is a visual-format project (which formats exist is the connected
   build's own; a number never implies a feature that build lacks). Every step of an even-format
   project, Verify included, and visual-format non-Verify reports without stored images use
   `{project_root, report}`. Every visual-format Verify uses
   `{project_root, report, expected_owner, visuals}`, even without images. Supply both
   `expected_owner: {kind: "step", ref: "<actual open step ref>"}` and `visuals`; with no images,
   use `visuals: {attachments: [], uses: [], declaration: {state: "not_needed", reason: "<what replaces the image>"}}`.
   Visual-format non-Verify reports citing stored images also use both fields, per the connected schema.
   Keep `evidence_files` in the Report in every Verify. Available capabilities and project format,
   not the OS name, determine the input shape. Never migrate an existing project merely to follow
   these instructions. The rules behind every visual refusal are one topic: `gil_help` with
   `topic: report/visuals`.
7. **Approved plans (only where recorded).** A project uses the approved-plan profile only when
   its `gil_context` names the approved-plan profile (`Approved-plan profile`, `승인 계획 프로필`) or `gil_plan_read` answers `supported: true`, and the
   connected `tools/list` offers `gil_plan_propose`/`gil_plan_read`/`gil_plan_request_interview`.
   Otherwise (legacy projects, default builds, tools absent) never require `gil_plan_*`; use the
   ordinary Synthesis. Do not infer this from the OS or the install kind.
   - In an open Interview Synthesis, call `gil_plan_propose` with exactly two top-level fields,
     `project_root` and `draft`. The three draft fields live **inside `draft`**. Extra fields at
     the top level, in `draft`, in `scope` or in an item are refused; `synthesis` is a map of
     string-valued Report fields, including `chain_verdict` when proposing Chain closure:

     ```json
     {"project_root": "<absolute path>",
      "draft": {
        "scope": {"goal": "…", "in_scope": "…", "out_of_scope": "…", "constraints": "…", "success_criteria": "…"},
        "items": [{"id": "A", "question": "…", "done_when": "…"}],
        "synthesis": {"statement": "…", "basis_refs": "step:C1/S1\nstep:C1/S2", "exploration_checklist": "A"}}}
     ```

     `scope` has those five string fields (write `none`, or its equivalent in the user's language, rather than leaving one empty); `items`
     is the ordered checklist (1–64) of `id`, `question`, `done_when`; `synthesis` holds the
     Synthesis Report fields as strings, where `exploration_checklist` lists the item IDs in the same
     order, one per line. Replace the example's basis refs with the actual closed Question and
     Interpretation refs in the current Synthesis lineage. From the first proposal, each item is one
     performable, checkable problem with an observable `done_when` (rule 2); Core does not judge
     problem size, so write it right rather than expecting a refusal. `basis_refs` is a nonempty newline-separated
     string of unique canonical `step:Cn/Sm` references, one per line, in this same Interview only.
     Do not add commas, prose, Cycle references, other-Cycle steps, open steps or abandoned branches.
     Previous-Cycle/external background belongs in `statement` prose and cannot replace these current
     Interview references. Never silently move or correct references; correct the draft explicitly.
     A refusal lists every invalid line with its original 1-based line number and field path.
     Opening and later Interviews use
     the same wrapper. A Closing proposal keeps the existing items and checklist (including
     executed items) and adds `draft.synthesis.chain_verdict` as `"success"` or `"failure"`.
     To extend an existing plan, first read `gil_plan_read.active_plan`. It returns the exact
     executable approved revision's `scope` and ALL `items`, including executed items, even
     in an Interview where `document` is null. Copy every existing item verbatim, then insert
     new IDs and update `exploration_checklist` in that order. Never reconstruct item text
     from memory or send only new items. Scope and order may change with a new approval;
     existing `question`/`done_when` may not. `active_plan: null` means no executable approved
     revision yet; a pending proposal is separate. `document` keeps its Experiment binding meaning.
     Never write `approved`, `plan_revision` or `plan_digest`: Core adds them.
   - **End the turn and wait for the Monitor button.** Chat agreement is not approval. While
     waiting, make no project file changes or re-proposal. Check approval only with `gil_plan_read`.
   - `receipt.decision: approve` → `gil_close` Synthesis with `close_report` **unchanged**, then
     `gil_open`/`gil_close` Outcome, then `gil_close` the Interview Cycle Report. For execution,
     then `gil_open` Experiment: under item binding the first pending item binds automatically;
     under the exploration contract send the requirement link (rule 3). For Closing, use
     `next_direction.action: close_chain` in the Interview Cycle Report, then submit the Chain
     Report with the approved `chain_verdict` as its `verdict`; do not open another Experiment.
     The Closing Interview's own Outcome and Cycle verdicts are `success` because the approval
     was obtained; that is not the Chain verdict, so a Chain approved as `failure` is reported
     as `failure`.
     `request_changes` → close with that
     `close_report` (`approved: no`) and ask what to change in a new Question. A change request
     is never an approval: nothing of the plan executes and no Chain closes. Editing an open
     proposal makes a new revision; an earlier approval is never reused.
   - One Experiment binds exactly one `current_item`, the first pending item, when it opens.
     `executed_cycles` records that binding, not success and not a semantic audit of every task.
     (That is item binding. Under the exploration contract `current_item` is the requirement the
     Experiment was linked to and `pending` lists requirements with no linked Experiment yet — rule 3.)
     Doing I2–I4 while bound to I1 does not mark those items executed. Do not backfill bindings or
     repeat completed work just to change the ledger; disclose the mismatch and agree on the
     remaining verification scope. Work that is one problem is one item before approval; independent goals are separate items.
   - `document` is available immediately after the Experiment opens and binds its revision, before
     Define/Verify. Before binding (including Interview) it is null; reading or a refused close
     never creates it. It stays tied to that Cycle's revision, not a newer proposal. If an open,
     bound Experiment returns null, preserve the response and check the current connection/state;
     never invent a summary. `close_report` belongs to its Synthesis and is not the current Step's
     report after Synthesis has closed; follow the current-state `next` instead.
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
     evidence to leave an unfinished Step. If the item cannot be finished, close truthfully with an
     interruption observation or, where the record supports it, an unachieved end (rule 4). After
     a `failure` · `revisit` close, show the preserved
     partial results before the Monitor recovery removes them from the folder (they stay in the
     Snapshot and the preserved location). Keeping the partial results in place and continuing is
     not a recorded transition yet, and cancelling, splitting or replacing an approved item is not
     either: the next plan keeps the interrupted item verbatim and adds or reorders items. A large
     approved item of which only a part was executed is not complete; disclose that gap instead of
     redefining the item or counting the part as the whole.
   - When every approved item has executed, the project is not finished by a chat summary:
     `gil_open` Interview as the Closing Interview, propose the final `chain_verdict` for approval,
     then `gil_close` the Chain Report. Its first message shows the result before asking (rule 5).
     The closing proposal states, for each approved question, the answer reached with its
     evidence references, the counter-evidence and the limits, read from the original Reports; it
     separates what is still unanswered, why this exploration stops, and candidate follow-up
     experiments or alternative material. The Chain Report's `handoff_summary` is written once
     from those same records and names the candidates as candidates for the next Chain's
     Interview, never as an approved plan. Unanswered questions stay stated as unanswered; they
     are not turned into success.
   **Reflection and recovery.** Use Analysis evidence and Outcome's lesson to trace relevant
   earlier Steps/Cycles and distinguish observations, assumptions and interpretations. Explain
   where a faulty premise entered, how it propagated, and why the proposed recovery target is
   suitable; the error location is not automatically the recovery target. Cite relevant records
   instead of copying the entire history. Use the existing report fields, not a new Reflection kind.
   Every failed Experiment requires explicit revisit, even when its world has no file changes.
   Restore the whole tracked world (working code and generated outputs); do not retain selected
   failed files because “most worked”. Preserve evidence and learning in the records. Honor the
   current target and approval gates; cross-Chain investigation is not cross-Chain restore support.
   For successful continuation, explain the next question and prerequisites. Before proposing
   Chain closure, explain why to stop, show results and remaining questions, and obtain the user's
   exact closing approval. Waiting is not failure or consent. The next Chain's Interview uses prior
   answers, evidence, uncertainty and alternative data/experiments, but needs its own approval.

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
   the user to repeat a special phrase such as “I pressed it”. Do not poll while waiting for user input.
   When that message reports a plan decision, `gil_plan_read` is the record to read: its receipt,
   the exact proposal and `next` outrank the message text. A recorded change request is not an
   approval, and an approved closing proposal leads to the Chain Report, never to an Experiment.
   If the record and the message disagree, or the decision is unclear, follow the record and do
   not guess.

   **Brief new-session handoff:** use the saved-record resume summary and the current next-call
   line to state the position, last recorded result, response/decision target and one permitted
   next action or wait. Link to the existing reports/`gil_story`, and to `gil_plan_read` only for
   an approved-plan project. An open Question has no recorded answer; this does not prove the
   question was delivered or that an actual answer already in the chat must be requested again.
   A saved Monitor approval is distinct from an open Synthesis or Interview: continue their
   remaining steps without asking for the same approval again. A binding means the Experiment
   opened, not that its work finished. A re-interview request does not close the current Cycle.
   After the Closing Interview, submit the Chain Report; after Chain closure, wait for a new
   topic and an explicit next Chain start. Follow `gil_plan_read.next`, not a surviving binding
   or an old `close_report`, to choose the current action.

   Context alone does not observe current file changes. Use `gil_status` for that existing
   observation path and retain an observation/read failure as unknown, never “unchanged”.
   Stored decisions do not prove Host message delivery, a visible screen or external work
   completion. Re-read an uncertain approval result; do not infer approval, failure or closure
   from silence, elapsed time or disconnection. Plan changes follow the exact revision/digest
   contract; never invalidate a saved approval just because a file changed.

   | Situation | Recovery |
   | --- | --- |
   | `gil_start` refused because a project exists | Read `gil_context` once and continue there; never start again |
   | Close response lost or uncertain | Read `gil_status`/`gil_context`; close again only if the step is still open. For an `unachieved_will` close, format 8/9 status and context show the last ended Will and its result; a resend whose Will already ended is refused with that Step and result and changes nothing |
   | `unachieved_will` refused | Nothing was closed. Not the active Will: read `gil_status` for the active one. Already ended: the end is on record, do not resend. Interview: keep waiting. Format 4–7: the project cannot record it — keep the Step open or use the interruption observation; a format 6/7 project is converted only through the reviewed, user-approved transition where the schema offers it (rule 4) |
   | `gil_close` input/Report rejected | The refusal names the missing or invalid field and shows the current input skeleton: put the Report STRING in `report`, not `contract` (the latter is for `gil_open`), fix only those fields and resend once; no manual lookup is needed for a field the refusal already shows. Read `gil_help({project_root, topic: "close"})` only when the refusal shows no skeleton or the position may have changed; do not run a PATH `gil` binary that may be a different build. Preserve typed `expected_owner`/`visuals` when required. |
   | `gil_close`/`gil_open` refused with an approved-plan reason (`Approved plan:` / `승인 계획:` in the reason) | Not an input error: do not fix and resend the same call. Read `gil_plan_read` once and follow the next action its reply names. For an actually open Synthesis: no proposal → `gil_plan_propose`; waiting → end the turn and wait for the Monitor button; approved → `gil_close` with `close_report` verbatim; changes requested → `close_report` (`approved: no`), then a Question. Never write `approved` yourself |
   | Plan decision or save result uncertain | `gil_plan_read`; never assume approval or re-propose blindly. Three states differ: approval still needed (wait), approval recorded (`receipt.decision`), save result unknown (read again; do not re-propose) |
   | Approved earlier, proposal edited since | Read the current revision/digest and receipt with gil_plan_read. Do not infer that the old approval is gone from history: it remains bound to its original revision/digest and does not approve the edited revision. Wait only if the current revision has no decision |
   | Asked to re-interview during an Experiment | The request is preserved and closes nothing; keep following each receipt's next call and close the Cycle only when its receipt offers the Cycle Report, then `gil_open` Interview |
   | `gil_revisit` refused | If Monitor approval is required, show Monitor and wait for its button. If protected files differ or the platform is unsupported, preserve files and report the limitation. Never edit records, fake success or loop restore. A durably approved interrupted recovery can be resumed by `gil_revisit` when the receipt explicitly says so |
   | Refused `evidence_files` | Save truthful evidence while Verify is open, then resubmit the close |
   | Monitor cannot be shown | Report it and continue the recorded step; the record is unaffected |

   Do not loop a tool to confirm a fact that a receipt already states, and never promise that a
   screen, button or save "always works": report what was observed.
