// M6-E design model. A pure, in-memory projection of the GIL state that the proposal touches.
// This is NOT the GIL Core and must not be mistaken for it: no files, no YAML, no locks,
// no Chain container, no Monitor. It exists so the transition table can be checked for
// contradictions before anyone writes Rust. Decision matrices are read from transitions.json.
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
export const TABLE = JSON.parse(readFileSync(join(here, 'transitions.json'), 'utf8'));

export class Refused extends Error {
  constructor(code, detail = '') {
    if (!TABLE.refusals[code]) throw new Error(`unknown refusal code in model: ${code}`);
    super(`${code}: ${TABLE.refusals[code]}${detail ? ` (${detail})` : ''}`);
    this.code = code;
  }
}
const refuse = (code, detail) => { throw new Refused(code, detail); };
const clone = (v) => structuredClone(v);

// ---------- initial states ----------
export function initial(preset = 'approved_plan_ready', opts = {}) {
  const base = {
    existence: 'X1',
    product_safety: opts.product_safety ?? true,
    approved_plan_enabled: opts.approved_plan ?? true,
    journey: { revision: 0, done_wills: [] },
    active_will: null,
    next_will_id: 1,
    snapshots: { A1: { files: { 'README.md': 'r1' } } },
    next_snapshot_id: 2,
    folder: { files: { 'README.md': 'r1' } },
    cycles: {},
    next_cycle_id: 2,
    current_cycle: 1,
    pending_cycle_revisit: null,
    plan: { proposals: [], executions: [], reinterview_requests: [] },
    human: { answered_question: null },
    session: 'alive',
  };
  base.cycles[1] = cycle(1, 'interview', null, 'A1');
  if (preset === 'fresh_interview') return base;
  if (preset === 'approved_plan_ready') {
    // Interview C1 closed with success/open_child and an approved proposal rev 1 [A,B,C].
    const c = base.cycles[1];
    c.steps = {
      1: { id: 1, kind: 'question', status: 'closed', parent: null, report: { question: 'goal?', response: 'ship X' } },
      2: { id: 2, kind: 'interpretation', status: 'closed', parent: 1, report: { interpretation: 'user wants X' } },
      3: { id: 3, kind: 'synthesis', status: 'closed', parent: 2, report: { statement: 'do A,B,C', approved: 'yes' } },
      4: { id: 4, kind: 'outcome', status: 'closed', parent: 3, report: { verdict: 'success', lesson: 'scope agreed', 'next_direction.action': 'close_cycle', 'next_direction.reason': 'approved', synthesis_ref: 'step:C1/S3' } },
    };
    c.current_step = 4; c.next_step_id = 5;
    c.status = 'closed'; c.exit = 'A1';
    c.report = { verdict: 'success', outcome_ref: 'step:C1/S4', handoff_summary: 'A,B,C approved', 'next_direction.action': 'open_child', 'next_direction.reason': 'start A' };
    base.plan.proposals.push({ revision: 1, synthesis_ref: 'step:C1/S3', digest: 'd1', items: opts.items ?? ['A', 'B', 'C'], receipt: { decision: 'approve', source: 'monitor_button' }, activated: true, closed_with_yes: true });
    base.journey = { revision: 4, done_wills: ['W1', 'W2', 'W3', 'W4'] };
    base.next_will_id = 5;
    return base;
  }
  throw new Error(`unknown preset ${preset}`);
}

function cycle(id, kind, parent, entry) {
  return { id, kind, status: 'open', parent, revisit_from: null, entry, exit: null, report: null,
    steps: {}, current_step: null, next_step_id: 1, existence: 'X1' };
}

// ---------- derived views ----------
const digestOf = (files) => Object.keys(files).sort().map(k => `${k}=${files[k]}`).join(';');
function internSnapshot(state, files) {
  for (const [id, snap] of Object.entries(state.snapshots)) if (digestOf(snap.files) === digestOf(files)) return id;
  const id = `A${state.next_snapshot_id++}`;
  state.snapshots[id] = { files: clone(files) };
  return id;
}
export function openCycle(state) { const c = state.cycles[state.current_cycle]; return c.status === 'open' ? c : null; }
export function openStep(state) {
  const c = openCycle(state); if (!c || c.current_step == null) return null;
  const s = c.steps[c.current_step]; return s.status === 'open' ? s : null;
}
function lineage(c, stepId) { const out = []; for (let id = stepId; id != null; id = c.steps[id].parent) out.push(c.steps[id]); return out; }
function nearestVerifySnapshot(c, fromStep) {
  if (fromStep == null) return null;
  for (const s of lineage(c, fromStep)) if (s.kind === 'verify' && s.status === 'closed') return s.snapshot_ref;
  return null;
}
export function worldSnapshot(state) {
  const c = state.cycles[state.current_cycle];
  if (c.status === 'closed') return c.exit;
  return nearestVerifySnapshot(c, c.current_step) ?? c.entry;
}
export function isClean(state) { return digestOf(state.folder.files) === digestOf(state.snapshots[worldSnapshot(state)].files); }
function cycleLineage(state, id) { const out = []; for (let x = id; x != null; x = state.cycles[x].parent) out.push(state.cycles[x]); return out; }
const isBranchPoint = (c) => c.status === 'closed' && c.report?.['next_direction.action'] === 'open_child';
const conclusionOf = (report) => report?.conclusion ?? TABLE.outcome.default_conclusion;
function lastOutcome(c) { const ids = Object.keys(c.steps).map(Number).sort((a, b) => b - a); for (const id of ids) if (c.steps[id].kind === 'outcome' && c.steps[id].status === 'closed') return c.steps[id]; return null; }
function activeProposal(state) { return [...state.plan.proposals].reverse().find(p => p.activated && p.receipt?.decision === 'approve') ?? null; }
// The Cycle Report is the truth source of an attempt's outcome; the ledger only binds.
export function executionConclusion(state, e) {
  const c = state.cycles[e.cycle];
  return c.status === 'closed' ? conclusionOf(c.report) : 'open';
}

// ---------- apply ----------
export function apply(input, event, args = {}) {
  const state = clone(input);
  const fn = HANDLERS[event];
  if (!fn) throw new Error(`unknown event ${event}`);
  const transition = TABLE.transitions.find(t => t.event === event)?.id;
  fn(state, args);
  return { state, transition };
}

const requireFields = (report, names) => { for (const n of names) if (!report?.[n] || !String(report[n]).trim()) refuse('ReportIncomplete', n); };

const HANDLERS = {
  open_step(state, { kind, contract }) {
    if (state.pending_cycle_revisit != null) refuse('RevisitPending');
    const c = openCycle(state) ?? refuse('NoOpenCycle');
    if (openStep(state)) refuse('StepStillOpen');
    if (state.active_will) refuse('WillAlreadyActive');
    for (const k of ['objective', 'next_action', 'done_when']) if (!contract?.[k]?.trim()) refuse('ContractIncomplete', k);
    const parentKind = c.current_step == null ? 'cycle_entry' : c.steps[c.current_step].kind;
    if (!TABLE.grammar[c.kind][parentKind]?.includes(kind)) refuse('GrammarRefused', `${parentKind}→${kind}`);
    const id = c.next_step_id++;
    c.steps[id] = { id, kind, status: 'open', parent: c.current_step, report: null, existence: state.existence };
    c.current_step = id;
    state.active_will = { id: `W${state.next_will_id++}`, target: { cycle: c.id, step: id }, ...contract };
  },
  revise_will(state, { objective, next_action, done_when, target }) {
    const w = state.active_will ?? refuse('NoActiveWill');
    if (target && (target.cycle !== w.target.cycle || target.step !== w.target.step)) refuse('WillTargetLocked');
    const next = { ...w, ...(objective != null && { objective }), ...(next_action != null && { next_action }), ...(done_when != null && { done_when }) };
    for (const k of ['objective', 'next_action', 'done_when']) if (!next[k]?.trim()) refuse('ContractIncomplete', k);
    state.active_will = next;
  },
  edit_files(state, { files }) { state.folder.files = clone(files); },
  close_verify(state, { report, observation_refused }) {
    const s = openStep(state) ?? refuse('NothingOpen');
    if (s.kind !== 'verify') refuse('NotVerify');
    requireFields(report, ['execution', 'result']);
    if (observation_refused) refuse('ObservationRefused');
    const paths = String(report?.evidence_files ?? '').split('\n').map(x => x.trim()).filter(Boolean);
    if (paths.length === 0) refuse('EvidenceMissing');
    const found = state.folder.files, before = state.snapshots[worldSnapshot(state)].files;
    let fresh = false;
    for (const p of paths) {
      if (!(p in found)) refuse('EvidenceNotInWorld', p);
      if (found[p] === '') refuse('EvidenceEmptyFile', p);
      fresh ||= before[p] !== found[p];
    }
    if (!fresh) refuse('EvidenceStale');
    const snap = internSnapshot(state, found);
    finishAction(state, s, report, snap);
  },
  close_step(state, { report }) {
    const s = openStep(state) ?? refuse('NothingOpen');
    if (s.kind === 'verify') refuse('IsVerify');
    if (!isClean(state)) refuse('DirtyWorld');
    requireFields(report, TABLE.step_close_requires[s.kind]);
    const c = openCycle(state);
    if (s.kind === 'question') {
      if (state.human.answered_question !== s.id) refuse('ResponseNotFromHuman');
    }
    if (s.kind === 'synthesis') {
      if (!['yes', 'no'].includes(report.approved)) refuse('ReportIncomplete', 'approved');
      if (state.approved_plan_enabled) {
        const p = state.plan.proposals.find(p => p.synthesis_ref === `step:C${c.id}/S${s.id}` && !p.superseded);
        if (!p?.receipt) refuse('ApprovalNotFromHuman');
        if (p.digest !== report.digest) refuse('ApprovalForOtherProposal');
        const want = p.receipt.decision === 'approve' ? 'yes' : 'no';
        if (report.approved !== want) refuse('ApprovalNotFromHuman', 'approved differs from receipt');
      } else if (!state.human.synthesis_decision || state.human.synthesis_decision.step !== s.id) {
        refuse('ApprovalNotFromHuman');
      } else if (state.human.synthesis_decision.approved !== report.approved) refuse('ApprovalNotFromHuman');
    }
    if (s.kind === 'outcome') {
      const allowed = TABLE.outcome.allowed_actions[c.kind][report.verdict] ?? refuse('VerdictValueRefused', report.verdict);
      const conclusion = conclusionOf(report);
      if (!TABLE.outcome.conclusion_values.includes(conclusion)) refuse('ConclusionRefused', conclusion);
      if (conclusion === 'not_reached' && report.verdict !== 'failure') refuse('ConclusionRefused', 'not_reached requires failure');
      if (conclusion === 'not_reached') for (const f of TABLE.outcome.not_reached_requires) if (!report[f]?.trim()) refuse('BlockerMissing', f);
      if (!allowed.includes(report['next_direction.action'])) refuse('ActionNotAllowed', report['next_direction.action']);
      if (c.kind === 'interview') {
        const ref = report.synthesis_ref; const m = /^step:C(\d+)\/S(\d+)$/.exec(ref ?? '');
        const target = m && Number(m[1]) === c.id ? c.steps[Number(m[2])] : null;
        if (!target || target.kind !== 'synthesis' || target.status !== 'closed' || target.report.approved !== 'yes' || !lineage(c, s.id).includes(target)) refuse('SynthesisRefRefused');
      }
    }
    finishAction(state, s, report, null);
    if (s.kind === 'synthesis' && state.approved_plan_enabled) {
      const p = state.plan.proposals.find(p => p.synthesis_ref === `step:C${c.id}/S${s.id}` && !p.superseded);
      if (p && report.approved === 'yes') p.closed_with_yes = true;
    }
  },
  close_cycle(state, { report }) {
    const c = openCycle(state) ?? refuse('CycleNotOpen');
    const open = openStep(state); if (open) refuse('InnerStepStillOpen');
    if (state.active_will) refuse('WillStillActive');
    if (!isClean(state)) refuse('DirtyWorld');
    requireFields(report, ['verdict', 'outcome_ref', 'handoff_summary', 'next_direction.action', 'next_direction.reason']);
    const outcome = lastOutcome(c) ?? refuse('ReportIncomplete', 'no closed outcome');
    if (report.outcome_ref !== `step:C${c.id}/S${outcome.id}`) refuse('ReportIncomplete', 'outcome_ref');
    if (report.verdict !== outcome.report.verdict) refuse('VerdictMismatch');
    const conclusion = conclusionOf(report);
    if (conclusion !== conclusionOf(outcome.report)) refuse('ConclusionMismatch');
    const key = `${report.verdict}/${conclusion}`;
    const allowed = TABLE.cycle_report.allowed_actions[c.kind][key] ?? refuse('ActionNotAllowed', key);
    const action = report['next_direction.action'];
    if (!allowed.includes(action)) refuse('ActionNotAllowed', `${key}→${action}`);
    const target = report['next_direction.target_cycle_ref'];
    if (action === 'revisit') {
      if (!target) refuse('TargetRequiredOrForbidden');
      const m = /^cycle:C(\d+)$/.exec(target); const id = m ? Number(m[1]) : NaN;
      const t = state.cycles[id];
      if (!t || id === c.id || !cycleLineage(state, c.parent).includes(t) || !isBranchPoint(t)) refuse('RevisitTargetRefused', target);
    } else if (target) refuse('TargetRequiredOrForbidden');
    c.exit = nearestVerifySnapshot(c, outcome.id) ?? c.entry;
    c.report = clone(report); c.status = 'closed';
    if (c.kind === 'interview' && state.approved_plan_enabled) {
      const p = state.plan.proposals.find(p => p.synthesis_ref === outcome.report.synthesis_ref && !p.superseded);
      if (p && p.closed_with_yes && p.receipt?.decision === 'approve') {
        p.activated = true;
        // A newly approved revision answers every stored re-interview request (model bookkeeping).
        for (const r of state.plan.reinterview_requests) r.resolved_by_revision ??= p.revision;
      }
    }
  },
  revisit_cycle(state) {
    const c = state.cycles[state.current_cycle];
    if (c.status !== 'closed') refuse('CycleStillOpen');
    if (state.pending_cycle_revisit != null) refuse('RevisitPending');
    if (c.report['next_direction.action'] !== 'revisit') refuse('NoRevisitDeclared');
    if (!isClean(state)) refuse('DirtyWorld');
    if (state.product_safety) refuse('ProductSafetyBlocked');
    const id = Number(/^cycle:C(\d+)$/.exec(c.report['next_direction.target_cycle_ref'])[1]);
    const t = state.cycles[id]; if (!t.exit) refuse('TargetHasNoExitWorld');
    state.current_cycle = id; state.pending_cycle_revisit = c.id;
    state.folder.files = clone(state.snapshots[t.exit].files);
  },
  open_cycle(state, { kind }) {
    if (openCycle(state)) refuse('AnotherCycleOpen');
    const here_ = state.cycles[state.current_cycle];
    let from = null;
    if (state.pending_cycle_revisit != null) {
      from = state.cycles[state.pending_cycle_revisit];
      const declared = from.report['next_direction.target_cycle_ref'];
      if (declared !== `cycle:C${here_.id}`) refuse('NotWhereItSaid');
    }
    if (!isBranchPoint(here_)) refuse('TargetCannotBranch');
    let binding = null;
    if (kind === 'experiment' && state.approved_plan_enabled) {
      const active = activeProposal(state) ?? refuse('NoPendingItem', 'no active approved revision');
      if (state.plan.reinterview_requests.some(r => r.resolved_by_revision == null)) refuse('ReinterviewRequested');
      // M6-E: an item counts as executed only when some attempt reached a conclusion (judged)
      // or is still open. Attempts that ended not_reached leave the item pending.
      const judged = new Set(state.plan.executions.filter(e => executionConclusion(state, e) !== 'not_reached').map(e => e.item));
      const item = active.items.find(i => !judged.has(i)) ?? refuse('NoPendingItem');
      const priorAttempts = state.plan.executions.filter(e => e.item === item && e.revision === active.revision);
      const attempt = priorAttempts.length + 1;
      if (attempt > TABLE.approved_plan.max_not_reached_attempts_per_revision) refuse('RetryBudgetExhausted', `${item} attempt ${attempt}`);
      const last = [...state.plan.executions].reverse().find(e => e.item === item);
      binding = { item, revision: active.revision, attempt, retry_of: last ? last.cycle : null };
    }
    const id = state.next_cycle_id++;
    const born = cycle(id, kind, here_.id, here_.exit);
    born.revisit_from = from ? from.id : null;
    state.cycles[id] = born; state.current_cycle = id; state.pending_cycle_revisit = null;
    if (binding) state.plan.executions.push({ cycle: id, ...binding });
  },
  session_crash(state) { state.session = 'dead'; },
  session_resume(state, { existence = 'X1' } = {}) {
    const s = openStep(state) ?? openCycle(state);
    if (s && s.existence !== existence) refuse('ExistenceMismatch');
    state.session = 'alive';
  },
  switch_existence(state, { to }) {
    if (openStep(state) || openCycle(state)) refuse('OpenNodeBlocksSwitch');
    state.existence = to;
  },
  propose_plan(state, { digest, items }) {
    const s = openStep(state); const c = openCycle(state);
    if (!s || s.kind !== 'synthesis') refuse('NoOpenSynthesis');
    const ref = `step:C${c.id}/S${s.id}`;
    const same = state.plan.proposals.find(p => p.synthesis_ref === ref && p.digest === digest && !p.superseded);
    if (same) return;
    for (const p of state.plan.proposals) if (p.synthesis_ref === ref && !p.superseded) p.superseded = true;
    state.plan.proposals.push({ revision: state.plan.proposals.length + 1, synthesis_ref: ref, digest, items: clone(items), receipt: null, activated: false });
  },
  human_decide(state, { digest, decision }) {
    const p = [...state.plan.proposals].reverse().find(p => !p.superseded && !p.activated) ?? refuse('NoProposal');
    if (p.digest !== digest) refuse('DigestMismatch');
    if (p.receipt) refuse('AlreadyDecided');
    p.receipt = { decision, source: 'monitor_button' };
  },
  human_answer(state, { approved } = {}) {
    const s = openStep(state);
    if (s?.kind === 'synthesis' && !state.approved_plan_enabled) { state.human.synthesis_decision = { step: s.id, approved }; return; }
    if (!s || s.kind !== 'question') refuse('NoOpenQuestion');
    state.human.answered_question = s.id;
  },
  request_interview(state, { reason }) {
    const c = openCycle(state); if (!c || c.kind !== 'experiment') refuse('NotInExperiment');
    if (!reason?.trim()) refuse('ReasonEmpty');
    if (!state.plan.reinterview_requests.some(r => r.cycle === c.id && r.reason === reason)) state.plan.reinterview_requests.push({ cycle: c.id, reason });
  },
  restore_world(state) {
    if (state.product_safety) refuse('ProductSafetyBlocked');
    state.folder.files = clone(state.snapshots[worldSnapshot(state)].files);
  },
};

function finishAction(state, step, report, snapshot) {
  const w = state.active_will ?? refuse('NoActiveWill');
  if (w.target.step !== step.id) refuse('NothingOpen', 'will targets elsewhere');
  step.report = clone(report); step.status = 'closed';
  if (snapshot) step.snapshot_ref = snapshot;
  state.journey.done_wills.push(w.id); state.journey.revision += 1; state.active_will = null;
  step.journey_ref = `J${state.journey.revision}`;
  if (step.kind === 'synthesis' && report.approved === 'yes') step.approved_yes = true;
}

// ---------- invariants ----------
export function checkInvariants(state) {
  const problems = [];
  const openCycles = Object.values(state.cycles).filter(c => c.status === 'open');
  if (openCycles.length > 1) problems.push('more than one open cycle');
  if (openCycles.length === 1 && openCycles[0].id !== state.current_cycle) problems.push('open cycle is not current');
  if (state.active_will && !openStep(state)) problems.push('active will without open step');
  const s = openStep(state);
  if (s && (!state.active_will || state.active_will.target.step !== s.id)) problems.push('open step without will targeting it');
  if (state.journey.done_wills.length !== state.journey.revision) problems.push('journey revision != done will count');
  for (const c of Object.values(state.cycles)) {
    if (c.status === 'open' && c.exit) problems.push(`open cycle ${c.id} has exit`);
    if (c.status === 'closed' && (!c.exit || !c.report)) problems.push(`closed cycle ${c.id} lacks exit/report`);
    if (!(c.entry in state.snapshots)) problems.push(`cycle ${c.id} entry snapshot missing`);
    if (c.exit && !(c.exit in state.snapshots)) problems.push(`cycle ${c.id} exit snapshot missing`);
    if (c.revisit_from != null && state.cycles[c.revisit_from].status !== 'closed') problems.push(`cycle ${c.id} born from open cycle`);
    if (c.parent != null && c.id !== state.current_cycle && c.status === 'open') problems.push('non-current open cycle');
    if (c.parent === c.revisit_from && c.revisit_from != null) problems.push(`cycle ${c.id} parent is the abandoned failure`);
    const openSteps = Object.values(c.steps).filter(x => x.status === 'open');
    if (openSteps.length > 1) problems.push(`cycle ${c.id} has two open steps`);
    if (c.status === 'closed' && openSteps.length) problems.push(`closed cycle ${c.id} has open step`);
    for (const st of Object.values(c.steps)) {
      if (st.status === 'closed' && !st.report) problems.push(`closed step without report C${c.id}/S${st.id}`);
      if (st.kind === 'verify' && st.status === 'closed' && !(st.snapshot_ref in state.snapshots)) problems.push(`verify without snapshot C${c.id}/S${st.id}`);
      if (st.kind === 'outcome' && st.status === 'closed' && conclusionOf(st.report) === 'not_reached' && st.report.verdict !== 'failure') problems.push('not_reached with non-failure verdict');
    }
  }
  if (state.pending_cycle_revisit != null && (openStep(state) || openCycle(state))) problems.push('pending revisit with an open node');
  for (const e of state.plan.executions) {
    if (e.retry_of != null && executionConclusion(state, { cycle: e.retry_of }) !== 'not_reached') problems.push('retry of a judged or open cycle');
    if (e.attempt > TABLE.approved_plan.max_not_reached_attempts_per_revision) problems.push('attempt over budget');
    const same = state.plan.executions.filter(x => x.item === e.item && x.revision === e.revision && x.cycle !== e.cycle);
    if (same.some(x => executionConclusion(state, x) === 'judged') && executionConclusion(state, e) === 'open') problems.push('item reopened after a judged attempt');
  }
  return problems;
}

// Frozen view of every closed record, for immutability checks between events.
export function closedRecords(state) {
  const out = {};
  for (const c of Object.values(state.cycles)) {
    if (c.status === 'closed') out[`cycle:${c.id}`] = { report: c.report, exit: c.exit, entry: c.entry, parent: c.parent, revisit_from: c.revisit_from };
    for (const s of Object.values(c.steps)) if (s.status === 'closed') out[`step:C${c.id}/S${s.id}`] = s;
  }
  for (const w of state.journey.done_wills) out[`will:${w}`] = w;
  for (const [id, snap] of Object.entries(state.snapshots)) out[`snapshot:${id}`] = snap;
  return out;
}
