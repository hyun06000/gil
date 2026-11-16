// Structural checks on the user-visible outputs of an Interview: a Question, a plan proposal
// (the `draft` of gil_plan_propose), a Define Report and a completion claim. They look only
// at shape and explicit links — never at wording, length, counts or timing — so they can run
// on fixture examples and on recorded outputs alike. Semantic quality is a separate rubric.
import { STEP_REF, CHOICE_ROLES } from './vocab.mjs';

const isStr = (v) => typeof v === 'string';
const nonBlank = (v) => isStr(v) && v.trim().length > 0;
const SCOPE_FIELDS = ['goal', 'in_scope', 'out_of_scope', 'constraints', 'success_criteria'];

/** @returns {{id:string,status:'pass'|'fail',detail:string}[]} one entry per applicable check */
export function checkProposal(p) {
  const out = [];
  const shape = [];
  if (!p || typeof p !== 'object') return [{ id: 'proposal_shape', status: 'fail', detail: 'proposal is not an object' }];
  if (!p.scope || typeof p.scope !== 'object') shape.push('scope missing');
  else for (const f of SCOPE_FIELDS) if (!nonBlank(p.scope[f])) shape.push(`scope.${f} blank (write "없음"/"none" rather than leaving it empty)`);
  if (p.scope && typeof p.scope === 'object') for (const k of Object.keys(p.scope)) if (!SCOPE_FIELDS.includes(k)) shape.push(`scope.${k} is not a plan field`);
  const items = Array.isArray(p.items) ? p.items : null;
  if (!items) shape.push('items missing');
  else {
    if (items.length < 1 || items.length > 64) shape.push(`items count ${items.length} outside 1–64`);
    const seen = new Set();
    items.forEach((it, i) => {
      if (!it || typeof it !== 'object') { shape.push(`items[${i}] not an object`); return; }
      if (!nonBlank(it.id)) shape.push(`items[${i}].id blank`);
      else if (seen.has(it.id)) shape.push(`items[${i}].id "${it.id}" duplicated`);
      else seen.add(it.id);
      if (!nonBlank(it.question)) shape.push(`items[${i}].question blank`);
      if (!nonBlank(it.done_when)) shape.push(`items[${i}].done_when blank`);
      for (const k of Object.keys(it)) if (!['id', 'question', 'done_when'].includes(k)) shape.push(`items[${i}].${k} is not an item field`);
    });
  }
  const syn = p.synthesis && typeof p.synthesis === 'object' ? p.synthesis : null;
  if (!syn) shape.push('synthesis missing');
  else {
    if (!nonBlank(syn.statement)) shape.push('synthesis.statement blank');
    for (const k of ['approved', 'plan_revision', 'plan_digest']) if (k in syn) shape.push(`synthesis.${k} must not be written by the agent`);
    if ('chain_verdict' in syn && !['success', 'failure'].includes(syn.chain_verdict)) shape.push('synthesis.chain_verdict must be success|failure');
  }
  out.push({ id: 'proposal_shape', status: shape.length ? 'fail' : 'pass', detail: shape.length ? shape.join('; ') : 'scope(5 fields), items(1–64), synthesis present' });

  // Checklist must list the item ids in the same order, one per line.
  if (items && syn) {
    const ids = items.map((it) => it?.id).filter(nonBlank);
    const lines = isStr(syn.exploration_checklist) ? syn.exploration_checklist.split('\n').map((s) => s.trim()).filter(Boolean) : null;
    if (!lines) out.push({ id: 'proposal_checklist_matches_items', status: 'fail', detail: 'synthesis.exploration_checklist missing' });
    else if (lines.join('\u0000') !== ids.join('\u0000')) out.push({ id: 'proposal_checklist_matches_items', status: 'fail', detail: `checklist [${lines.join(',')}] ≠ item ids [${ids.join(',')}]` });
    else out.push({ id: 'proposal_checklist_matches_items', status: 'pass', detail: `${ids.length} item id(s) in order` });
  }
  if (syn) out.push(checkBasisRefs(syn.basis_refs));
  return out;
}

export function checkBasisRefs(value) {
  const id = 'basis_refs_canonical';
  if (!nonBlank(value)) return { id, status: 'fail', detail: 'basis_refs blank: at least one closed Question/Interpretation ref of this Interview is required' };
  const lines = value.split('\n').map((s, i) => ({ n: i + 1, s: s.trim() })).filter((l) => l.s);
  const problems = [];
  const seen = new Set();
  for (const { n, s } of lines) {
    if (s.includes(',')) problems.push(`line ${n}: comma-joined list`);
    else if (!STEP_REF.test(s)) problems.push(`line ${n}: "${s}" is not a canonical step:Cn/Sm ref`);
    else if (seen.has(s)) problems.push(`line ${n}: "${s}" duplicated`);
    seen.add(s);
  }
  return problems.length ? { id, status: 'fail', detail: problems.join('; ') } : { id, status: 'pass', detail: `${lines.length} canonical ref(s), unique` };
}

/** A Question: text, choices with explicit roles, and the two escape paths of Cycle Model §14. */
export function checkQuestion(q) {
  const id = 'question_escape_paths';
  if (!q || typeof q !== 'object') return [{ id, status: 'fail', detail: 'question is not an object' }];
  const problems = [];
  if (!nonBlank(q.question)) problems.push('question text blank');
  const choices = Array.isArray(q.choices) ? q.choices : null;
  // Cycle Model §14: every Question offers the two escape paths, even when no honest
  // multiple-choice options exist (then only those two remain; forced options are not required).
  if (!choices) problems.push('choices missing');
  else {
    choices.forEach((c, i) => {
      if (!c || typeof c !== 'object' || !nonBlank(c.label)) problems.push(`choices[${i}] needs a label`);
      if (!CHOICE_ROLES.includes(c?.role)) problems.push(`choices[${i}].role must be one of ${CHOICE_ROLES.join('|')}`);
    });
    if (!choices.some((c) => c?.role === 'unsure')) problems.push('no "아직 잘 모르겠음 — 더 작은 질문 필요" path (role unsure)');
    if (!choices.some((c) => c?.role === 'free_text')) problems.push('no "직접 입력" path (role free_text)');
  }
  if (typeof q.multi_select !== 'boolean') problems.push('multi_select must say whether several choices may be picked');
  return [{ id, status: problems.length ? 'fail' : 'pass', detail: problems.length ? problems.join('; ') : 'question, choices with roles, unsure + free_text paths, multi_select stated' }];
}

/**
 * A Define: problem, success_condition, and the explicit link to the approved item it executes.
 * @param plan the approved plan whose items the Define may bind ({items:[{id,...}]}) or null
 */
export function checkDefine(d, plan) {
  const id = 'define_links_item';
  if (!d || typeof d !== 'object') return [{ id, status: 'fail', detail: 'define is not an object' }];
  const problems = [];
  if (!nonBlank(d.problem)) problems.push('problem blank');
  if (!nonBlank(d.success_condition)) problems.push('success_condition blank');
  if (!nonBlank(d.bound_item)) problems.push('bound_item blank: a Define executes exactly one approved item');
  else if (!plan) problems.push(`bound_item "${d.bound_item}" cannot be resolved: no approved plan available`);
  else if (!(plan.items || []).some((it) => it?.id === d.bound_item)) problems.push(`bound_item "${d.bound_item}" is not an item of the approved plan [${(plan.items || []).map((it) => it?.id).join(',')}]`);
  return [{ id, status: problems.length ? 'fail' : 'pass', detail: problems.length ? problems.join('; ') : `bound to item ${d.bound_item}` }];
}

/**
 * Completion claims: every item the agent presents as complete must have a closed Experiment
 * with verdict success bound to that item. Anything else is an overclaim or unknown.
 * @param claimed string[] item ids the visible message calls complete
 * @param closed [{item, verdict}] closed Experiments observed in the tool log (or null if no log)
 */
export function checkCompletionClaims(claimed, closed) {
  const id = 'completion_claims_backed';
  if (!Array.isArray(claimed)) return [{ id, status: 'fail', detail: 'claimed_completed_items must be an array (empty when nothing is claimed)' }];
  if (!claimed.length) return [{ id, status: 'pass', detail: 'no completion claimed' }];
  if (!Array.isArray(closed)) return [{ id, status: 'unknown', detail: `items ${claimed.join(',')} claimed complete but no tool-log summary of closed Experiments is available` }];
  const unbacked = claimed.filter((c) => !closed.some((x) => x?.item === c && x?.verdict === 'success'));
  return unbacked.length
    ? [{ id, status: 'fail', detail: `claimed complete without a successful closed Experiment bound to it: ${unbacked.join(',')}` }]
    : [{ id, status: 'pass', detail: `${claimed.length} claim(s) each backed by a closed success Experiment` }];
}

/** Run every applicable structural check on an example or an output bundle. */
export function checkContent(kind, content, { plan = null, closedExperiments = null } = {}) {
  switch (kind) {
    case 'proposal': return checkProposal(content);
    case 'question': return checkQuestion(content);
    case 'define': return checkDefine(content, plan);
    case 'chat_message': {
      const out = [];
      if (Array.isArray(content?.claimed_completed_items)) out.push(...checkCompletionClaims(content.claimed_completed_items, closedExperiments));
      return out;
    }
    default: return [];
  }
}
