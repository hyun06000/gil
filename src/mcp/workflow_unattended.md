## GIL in an unattended session

GIL records how a problem gets solved: an Interview agrees what to solve, then small Experiments each answer one question. This session has no screen and no Monitor. What the user decides (the start, the plan) arrives recorded; nobody approves anything else. Use only the absolute `project_root` you were given. Every receipt names one next call: do it, then read the next receipt. Never repeat a call that succeeded. If you are unsure where you are, call `gil_context`. Never invent an approval, an answer or evidence, and never edit `.gil` or `.gilignore`.

### Bodies

`gil_open` takes `contract` and `gil_close` takes `report` as JSON objects of fields, never as one text. A contract is `{"objective": "…", "next_action": "…", "done_when": "…"}`. A refusal shows the object to fill in: correct the field it names and send the call once more.

### Start

`gil_start(project_root, start_kind, locale)`. For `tracked_scope`, propose the exact `.gilignore` text with `gil_start_prepare(rules)`, starting from the `default_rules` the receipt gives. When a receipt lists `awaiting_user`, the decision arrives in that same result or the next one; then read `gil_context` and go on.

### Interview

Steps: question → interpretation → (another question only if something that changes the work is still unknown) → synthesis → outcome → the Cycle Report.

Look before you ask: before the first Question, read what the request points at and the files of the project it touches, until you can say how the relevant parts work today. An Interview may read anything; it changes no file. Do not ask what the request or the project already answers.

- Question: ask the user only what would change what gets built, and close with the user's actual answer.
- Interpretation: say what the answer settled. In the Interview that draws the first plan, lay the work out from the top:
  - `major_problems`: the big results the request asks for, one per line as `M1: <result> — judged by <what shows it is done>`, in the request's own words.
  - `anti_goals`: what would make this a failure even though it runs.
  - `edge_cases`: inputs and conditions the request implies but does not spell out (empty, boundary, malformed, repeated, failing halfway), one per line. A request's silence on an input is not permission to break on it.
  - `hardest`: the part most likely to fail, and why.
  - `not_observable_here`: what cannot be observed here yet, and the means you will build to observe it (a test input, a reference you write, a measurement), or none.
- Synthesis: propose the plan as a tree with `gil_plan_propose({project_root, draft: {scope, items, synthesis}})`. Take the major problems one by one and split each into branches, and a branch again, until ONE observation decides each leaf. `items` are the leaves: `{id, part_of, question, observation, command, source}`. `part_of` is the leaf's path, major problem first (`M2 > timestamps > rollover`); every major problem needs at least one leaf. `observation` is the ONE observation that decides the question, `command` a command that can actually be run here to make it (not a placeholder), `source` the sentence of the request it comes from. Each anti-goal and each edge case belongs to some leaf. If nothing here can observe a leaf yet, the leaf that builds the means to observe it comes first; it is not a reason to leave the problem out. Once approved, `gil_plan_read.close_report` is the Report to close the Synthesis with, unchanged.
- Outcome, then the Cycle Report with `next_direction.action: open_child`.

The list of problems changes only in an Interview. To change it later, open a new Interview at a Cycle boundary (`gil_open(kind: interview)`) and propose again, copying the existing items unchanged from `gil_plan_read.active_plan.items` and adding new ones. A new Interview keeps the files as they are.

### Experiment

One Experiment answers one small question. Open it with `gil_open(kind: experiment, contract: …)`, the contract being `gil_plan_read.experiment_link` with its fields filled in. Steps: define → hypothesis → verify → analysis → outcome → the Cycle Report.

- Define: the smallest problem you can decide now. `success_condition` is the one observation that decides it: one command and what it shows, or one thing to look at.
- Hypothesis: the method you expect to bring that about, and in `change` what you will change in which files. An Experiment brings the state about; it does not only check whether it already holds.
- Verify: the only step in which project files may be created or changed. Make the change the hypothesis names, then run the command and observe; if the state does not hold yet, fix it inside this Verify and run again. Do what the hypothesis says and no more. GIL saved the approved plan at `plan/approved-plan.md`: put that path in `plan_document`. Save what you observed in files and list them in `evidence_files`; at least one must be new or changed. `command` is the command you actually ran and `observed` the line of its output that decides the question.
- Analysis: `problem_solved` and `success_condition_met` are booleans. `true` only if the observation the Define fixed was actually seen. If it was not seen, could not be checked or was checked only in part, `false` is the right answer. `unverified` names what the condition asked for and was not seen (`none` if nothing); anything there means `false`.
- Outcome: `verdict` success or failure, and the lesson. On a failure, `changed` says what this Experiment changed in the files: going back returns the files, and only this record reaches the next Hypothesis.
- The Cycle Report. After a success, `next_direction.action: open_child` and open the next small Experiment. After a failure, `next_direction.action: revisit` with `next_direction.target_cycle_ref`, then call `gil_revisit` once: it puts the files back as that Cycle left them, with no approval. Then open a new Experiment with a smaller problem or another method. Its Define and Hypothesis receipts list the failed Experiments left behind, with what each tried, changed and learned: build on them and do not repeat a method that failed.

A success of one Experiment does not finish a requirement; several small Experiments may serve one.

### The files

Only a Verify may change project files. If a close is refused because files changed outside a Verify, call `gil_restore`: it returns the tracked files to the recorded world, with no approval. Whatever you want to keep, make again in the next Verify.

### Ending

When every problem of the plan has an Experiment that closed as success, open a closing Interview (a Chain cannot be proposed closed as success before that; what could not be solved closes it as failure): its Synthesis proposal keeps the items and adds `chain_verdict` (success or failure). After approval close the Synthesis, the Outcome and the Cycle Report with `next_direction.action: close_chain`, then the Chain Report. Read what earlier steps recorded with `gil_report_read` and `gil_evidence_read`; `gil_help` opens the manual topic a refusal names.
