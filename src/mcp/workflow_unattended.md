GIL in an unattended session
- project_root: the absolute path given. One next call per receipt. Unsure → gil_context.
- contract / report: JSON objects, never one text. A refusal shows the object → fix it, resend once.
- Never invent approval, answers, evidence. Never edit .gil, .gilignore.

Start
- gil_start(project_root, start_kind, locale) → tracked_scope: gil_start_prepare(rules = default_rules).
- awaiting_user → the decision arrives recorded → gil_context.

Interview: question → interpretation → synthesis → outcome → Cycle Report (open_child)
- question: ask the user; close with the real answer.
- synthesis: gil_plan_propose. items = {id, question, observation, source}.
  One observation per item; cannot → split. source = quoted request sentence.
  Approved → close with gil_plan_read.close_report unchanged.
- Change the list later → new Interview; copy old items, add new. Files stay.

Experiment: define → hypothesis → verify → analysis → outcome → Cycle Report
- open: gil_open(kind: experiment, contract: gil_plan_read.experiment_link filled in).
- define: smallest problem; success_condition = ONE observation.
- verify: the only place files change. plan_document: plan/approved-plan.md.
  evidence_files: observed results, at least one new or changed.
- analysis: problem_solved, success_condition_met = true | false. Not seen or partly seen → false.
- success → open_child. failure → revisit + target_cycle_ref → gil_revisit → new Experiment.

Files
- Changed outside verify → close refused → gil_restore → redo in the next verify.

End
- All solved → closing Interview: synthesis + chain_verdict → Cycle Report (close_chain) → Chain Report.
