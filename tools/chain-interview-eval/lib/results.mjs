// Consistency checks on an evaluation result file (gil.chain-interview.result/1).
// The checker never upgrades a status: not_run stays not_run, unknown stays unknown, and a
// pass/fail without basis, without an observation, from a non-judge or on an unexecuted run is
// reported as a problem. It recomputes the structural checks on the recorded outputs and
// compares them with what the file declares.
import { checkContent } from './structural.mjs';
import { validatePreparedEvidence } from './evidence.mjs';
import {
  RESULT_SCHEMA, STATUSES, VERDICT_STATUSES, APPROVAL_KINDS, CASE_RESULT_STATUSES, JUDGE_KINDS,
  HUMAN_ONLY_CRITERIA, STRUCTURAL_CHECKS,
} from './vocab.mjs';

const nonBlank = (v) => typeof v === 'string' && v.trim().length > 0;
const arr = (v) => (Array.isArray(v) ? v : null);
const zero = () => ({ pass: 0, fail: 0, unknown: 0, not_run: 0 });

/**
 * @param result parsed result file
 * @param caseSet loadCaseSet() output (cases, rubricIndex, digest)
 */
export function checkResult(result, caseSet) {
  const problems = [];
  const push = (check, where, detail) => problems.push({ check, where, detail });
  const out = { run: {}, case_set_match: null, current_case_set_sha256: caseSet.digest, counts: { cases: { evaluated: 0, unknown: 0, not_run: 0, missing: 0 }, automatic: zero(), semantic: zero() }, problems, cases: [] };

  if (!result || typeof result !== 'object') { push('result_schema', 'file', 'not an object'); return out; }
  if (result.schema !== RESULT_SCHEMA) push('result_schema', 'schema', `must be ${RESULT_SCHEMA}`);
  const run = result.run && typeof result.run === 'object' ? result.run : {};
  out.run = { executed: run.executed === true, model: run.model ?? null, guidance: run.guidance ?? null, case_set: run.case_set ?? null, example: run.example === true, id: run.id ?? null };

  // Run metadata: what a later repetition needs.
  if (!nonBlank(run.id)) push('run_metadata', 'run.id', 'blank');
  if (typeof run.executed !== 'boolean') push('run_metadata', 'run.executed', 'must be true or false');
  if (run.executed === true) {
    if (!run.model || !nonBlank(run.model.name) || !nonBlank(run.model.version)) push('run_metadata', 'run.model', 'an executed run must name model.name and model.version');
    if (!nonBlank(run.recorded_at)) push('run_metadata', 'run.recorded_at', 'an executed run must record when it was recorded');
    if (!run.harness || !nonBlank(run.harness.name)) push('run_metadata', 'run.harness', 'an executed run must name the harness (proxy/manual transcript/...)');
  }
  if (!run.guidance || !nonBlank(run.guidance.workflow_md_sha256) || !nonBlank(run.guidance.repo_commit)) push('run_metadata', 'run.guidance', 'needs workflow_md_sha256 and repo_commit (the guidance version under test)');
  if (!run.case_set || !nonBlank(run.case_set.sha256)) push('run_metadata', 'run.case_set', 'needs sha256 of the case set (check-fixtures prints it)');
  else if (caseSet.digest) out.case_set_match = run.case_set.sha256 === caseSet.digest;

  const judges = new Map();
  for (const [i, j] of (arr(result.judges) || []).entries()) {
    if (!nonBlank(j?.id)) { push('semantic_judge_is_person_or_model', `judges[${i}]`, 'id blank'); continue; }
    if (!JUDGE_KINDS.includes(j.kind)) push('semantic_judge_is_person_or_model', `judges[${i}]`, `kind must be ${JUDGE_KINDS.join('|')} (automatic heuristics are not judges)`);
    if (j.kind === 'model' && (!j.model || !nonBlank(j.model.name) || !nonBlank(j.model.version))) push('semantic_judge_is_person_or_model', `judges[${i}]`, 'a model judge needs model.name and model.version');
    judges.set(j.id, j);
  }

  const caseIndex = new Map(caseSet.cases.map((c) => [c.value?.id, c.value]));
  const rubric = caseSet.rubricIndex;
  const seen = new Set();
  const evidenceSources = new Set();
  for (const [i, rc] of (arr(result.cases) || []).entries()) {
    const where = `cases[${i}]`;
    const cid = rc?.case_id;
    if (!caseIndex.has(cid)) { push('case_coverage', where, `case_id ${cid} is not in the case set`); continue; }
    if (seen.has(cid)) { push('case_coverage', where, `case_id ${cid} appears twice`); continue; }
    seen.add(cid);
    if (Object.hasOwn(rc, 'prepared_evidence')) {
      try {
        validatePreparedEvidence(rc.prepared_evidence, run.id, cid, caseIndex.get(cid)?.input?.prior_state?.approved_plan);
        for (const source of Object.values(rc.prepared_evidence.sources).filter(Boolean)) {
          if (evidenceSources.has(source.id)) throw new Error('duplicate source');
          evidenceSources.add(source.id);
        }
      } catch {
        // Evidence is untrusted input. Never print its text, identifiers, tokens or paths.
        push('prepared_evidence_integrity', `cases[${i}].prepared_evidence`, 'invalid preparation: scope, source, hash, reference or projection mismatch');
      }
    }
    out.cases.push(checkCase(rc, caseIndex.get(cid), { where: `${where} (${cid})`, run, judges, rubric, push, counts: out.counts }));
  }
  for (const [cid] of caseIndex) if (!seen.has(cid)) {
    out.counts.cases.missing += 1;
    out.cases.push({ case_id: cid, status: 'not_in_result', approval: '-', automatic: zero(), semantic: zero() });
    push('case_coverage', cid, 'case set has this case but the result does not mention it; it counts as not_run, not as pass');
  }
  return out;
}

function checkCase(rc, c, { where, run, judges, rubric, push, counts }) {
  const row = { case_id: rc.case_id, status: rc.status, approval: rc.approval?.kind ?? '(none)', automatic: zero(), semantic: zero() };
  if (!CASE_RESULT_STATUSES.includes(rc.status)) { push('result_schema', where, `status must be ${CASE_RESULT_STATUSES.join('|')}`); return row; }
  counts.cases[rc.status] += 1;
  const auto = arr(rc.automatic) || [];
  const sem = arr(rc.semantic) || [];
  const tally = (list, bucket) => { for (const e of list) if (STATUSES.includes(e?.status)) { bucket[e.status] += 1; counts[bucket === row.automatic ? 'automatic' : 'semantic'][e.status] += 1; } };
  tally(auto, row.automatic); tally(sem, row.semantic);
  for (const [i, e] of auto.entries()) if (!STATUSES.includes(e?.status)) push('result_schema', `${where}.automatic[${i}]`, `status must be ${STATUSES.join('|')}`);
  for (const [i, e] of sem.entries()) if (!STATUSES.includes(e?.status)) push('result_schema', `${where}.semantic[${i}]`, `status must be ${STATUSES.join('|')}`);

  const verdicts = [...auto.map((e, i) => ({ e, at: `automatic[${i}]` })), ...sem.map((e, i) => ({ e, at: `semantic[${i}]` }))].filter((x) => VERDICT_STATUSES.includes(x.e?.status));

  // Approval provenance.
  const ap = rc.approval && typeof rc.approval === 'object' ? rc.approval : null;
  if (!ap || !APPROVAL_KINDS.includes(ap.kind)) push('synthetic_approval_not_human', `${where}.approval`, `kind must be ${APPROVAL_KINDS.join('|')}`);
  else {
    if (ap.kind === 'human_click' && !nonBlank(ap.evidence)) push('synthetic_approval_not_human', `${where}.approval`, 'human_click needs evidence (who observed the button press, where it is recorded)');
    if (ap.kind === 'synthetic') for (const [i, e] of sem.entries()) if (HUMAN_ONLY_CRITERIA.includes(e?.criterion) && e?.status === 'pass') push('synthetic_approval_not_human', `${where}.semantic[${i}]`, `${e.criterion} cannot pass on a synthetic approval`);
    if (ap.kind === 'none' || ap.kind === 'unknown') for (const [i, e] of sem.entries()) if (HUMAN_ONLY_CRITERIA.includes(e?.criterion) && e?.status === 'pass') push('synthetic_approval_not_human', `${where}.semantic[${i}]`, `${e.criterion} passes only with approval.kind human_click and evidence`);
  }

  // not_run / unexecuted runs carry no verdicts.
  if (run.executed !== true && rc.status !== 'not_run') push('not_run_preserved', where, `run.executed is not true, so every case must be not_run (found ${rc.status})`);
  if (rc.status === 'not_run') {
    for (const v of verdicts) push('not_run_preserved', `${where}.${v.at}`, `a not_run case carries ${v.e.status} on ${v.e.check || v.e.criterion}`);
    if (rc.outputs && Object.values(rc.outputs).some((x) => x != null && !(Array.isArray(x) && x.length === 0))) push('not_run_preserved', `${where}.outputs`, 'a not_run case has recorded outputs; either it ran (status evaluated/unknown) or the outputs do not belong here');
    return row;
  }
  // unknown: observation insufficient; no verdicts, each unknown has a reason.
  if (rc.status === 'unknown') {
    if (!nonBlank(rc.reason)) push('unknown_preserved', where, 'status unknown needs a reason (what observation is missing)');
    for (const v of verdicts) push('unknown_preserved', `${where}.${v.at}`, `an unknown case carries ${v.e.status} on ${v.e.check || v.e.criterion}`);
    for (const [i, e] of sem.entries()) if (e?.status === 'unknown' && !nonBlank(e.rationale)) push('unknown_preserved', `${where}.semantic[${i}]`, 'unknown needs a rationale');
    return row;
  }
  // evaluated: needs an observation.
  const obs = rc.observation && typeof rc.observation === 'object' ? rc.observation : null;
  const hasMessages = Array.isArray(obs?.visible_messages) && obs.visible_messages.some((m) => nonBlank(m?.text));
  if (!obs || (!hasMessages && !nonBlank(obs.tool_log_ref) && !nonBlank(obs.transcript_ref))) push('verdict_has_basis', `${where}.observation`, 'an evaluated case needs visible_messages, a tool_log_ref or a transcript_ref');

  // Every verdict has a basis; semantic verdicts come from a listed judge.
  for (const [i, e] of auto.entries()) {
    if (VERDICT_STATUSES.includes(e?.status) && !nonBlank(e.detail)) push('verdict_has_basis', `${where}.automatic[${i}]`, `${e.check}: ${e.status} without detail`);
    if (e?.status === 'unknown' && !nonBlank(e.detail)) push('unknown_preserved', `${where}.automatic[${i}]`, `${e.check}: unknown without detail`);
  }
  for (const [i, e] of sem.entries()) {
    const at = `${where}.semantic[${i}]`;
    if (!rubric.semantic.has(e?.criterion)) push('criteria_coverage', at, `criterion ${e?.criterion} is not in the rubric`);
    if (e?.status === 'unknown' && !nonBlank(e.rationale)) push('unknown_preserved', at, `${e.criterion}: unknown without rationale`);
    if (!VERDICT_STATUSES.includes(e?.status)) continue;
    const quotes = (arr(e.basis) || []).filter((b) => nonBlank(b?.quote));
    if (!quotes.length) push('verdict_has_basis', at, `${e.criterion}: ${e.status} without a quoted basis from the visible output`);
    if (!nonBlank(e.rationale)) push('verdict_has_basis', at, `${e.criterion}: ${e.status} without rationale`);
    const j = e.judge && typeof e.judge === 'object' ? e.judge : null;
    if (!j || !nonBlank(j.id)) push('semantic_judge_is_person_or_model', at, `${e.criterion}: verdict without judge.id`);
    else if (!judges.has(j.id)) push('semantic_judge_is_person_or_model', at, `${e.criterion}: judge ${j.id} is not in the judges list`);
    else if (!JUDGE_KINDS.includes(judges.get(j.id).kind)) push('semantic_judge_is_person_or_model', at, `${e.criterion}: judge ${j.id} is not human|model`);
  }

  // Criteria the case asked for must each have an entry.
  const asked = arr(c?.judgement?.semantic) || [];
  const present = new Set(sem.map((e) => e?.criterion));
  for (const s of asked) if (!present.has(s)) { push('criteria_coverage', where, `criterion ${s} requested by the case has no entry; it counts as not_run`); row.semantic.not_run += 1; counts.semantic.not_run += 1; }

  // Recompute structural checks on the recorded outputs and compare.
  const o = rc.outputs && typeof rc.outputs === 'object' ? rc.outputs : {};
  const plan = c?.input?.prior_state?.approved_plan?.items ? c.input.prior_state.approved_plan : (o.proposal?.items ? o.proposal : null);
  const closed = Array.isArray(obs?.tool_log_summary?.closed_experiments) ? obs.tool_log_summary.closed_experiments : null;
  const recomputed = [];
  for (const q of arr(o.questions) || []) recomputed.push(...checkContent('question', q));
  if (o.proposal) recomputed.push(...checkContent('proposal', o.proposal));
  if (o.define) recomputed.push(...checkContent('define', o.define, { plan }));
  // An empty claim list has nothing to record; a nonempty one must be checked against the log.
  if (Array.isArray(o.claimed_completed_items) && o.claimed_completed_items.length) recomputed.push(...checkContent('chat_message', { claimed_completed_items: o.claimed_completed_items }, { closedExperiments: closed }));
  const declared = new Map(auto.filter((e) => STRUCTURAL_CHECKS.includes(e?.check)).map((e) => [e.check, e]));
  const byId = new Map();
  for (const r of recomputed) {
    // Several questions → the worst status wins for the shared id.
    const prev = byId.get(r.id);
    if (!prev || (prev.status === 'pass' && r.status !== 'pass')) byId.set(r.id, r);
  }
  for (const [id, r] of byId) {
    const d = declared.get(id);
    if (!d) { push('structural_recomputed', where, `${id}: outputs allow this check (recomputed ${r.status}) but the result does not record it`); continue; }
    if (d.status !== r.status) push('structural_recomputed', where, `${id}: result says ${d.status}, recomputation says ${r.status} (${r.detail})`);
  }
  for (const [id, d] of declared) if (!byId.has(id) && VERDICT_STATUSES.includes(d.status)) push('structural_recomputed', where, `${id}: result records ${d.status} but outputs hold nothing this check applies to`);
  return row;
}
