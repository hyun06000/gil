// Bench plan: which scales to generate, within which limits. Pure functions, no I/O.
//
// Every axis grows one dimension of a real Core record while the others stay at the base point.
// Limits are deliberately small: the bench measures growth shape at small scale, it is not a
// load generator. A plan that exceeds a cap or the call budget is refused before any Core runs.

export const AXES = {
  chains: { label: '이전 Chain 수', cap: 6, defaults: [0, 1, 2], min: 0 },
  cycles: { label: '현재 Chain의 닫힌 Experiment Cycle 수', cap: 8, defaults: [0, 1, 2], min: 0 },
  failures: { label: '같은 계보의 누적 실패 Cycle 수', cap: 8, defaults: [0, 1, 2], min: 0 },
  steps: { label: '현재 Cycle의 닫힌 Step 수', cap: 4, defaults: [0, 2, 4], min: 0 },
  report_lines: { label: '현재 Cycle Report 칸당 줄 수', cap: 32, defaults: [1, 4, 16], min: 1 },
  plan_items: { label: '승인 계획의 미실행 항목 수(현재 항목 포함)', cap: 16, defaults: [1, 2, 4], min: 1 },
};
export const AXIS_NAMES = Object.keys(AXES);

/** The point every axis starts from: an open Experiment, Define+Hypothesis closed, Verify open. */
export const BASE = Object.freeze({ chains: 0, cycles: 0, failures: 0, steps: 2, report_lines: 1, plan_items: 1 });

export const REPEAT = { default: 2, cap: 3 };
export const CALL_BUDGET = { default: 3000, cap: 10000 };
export const READ_TOOLS = ['gil_context', 'gil_status', 'gil_plan_read', 'gil_story', 'gil_help'];
export const DEFAULT_READS = ['gil_context', 'gil_status', 'gil_plan_read'];
export const DEFAULT_FEATURES = 'approved-plan,visual-authoring';

/** Generation calls per building block. Kept next to the generator's walk (lib/scenario.mjs). */
export const CALLS = {
  start: 1,
  opening_interview: 13, // Q(2) I(2) open synthesis, propose, review, decide, read, close synthesis, outcome(2), cycle close
  experiment_success: 13, // open, plan_read, define(2) hypothesis(2) verify(2) analysis(2) outcome(2), cycle close
  experiment_failure: 14, // the same walk + gil_revisit
  closing_interview: 14, // open interview + the opening walk
  chain_handoff: 2, // close chain, open next chain
  current_experiment_open: 3, // open experiment, plan_read, open the next Step
  per_closed_step: 2,
};

export function estimateCalls(scale, reads) {
  return CALLS.start
    + scale.chains * (CALLS.opening_interview + CALLS.experiment_success + CALLS.closing_interview + CALLS.chain_handoff)
    + CALLS.opening_interview
    + scale.cycles * CALLS.experiment_success
    + scale.failures * CALLS.experiment_failure
    + CALLS.current_experiment_open + scale.steps * CALLS.per_closed_step
    + reads.length;
}

export function scaleKey(scale) {
  return AXIS_NAMES.map((a) => `${a}=${scale[a]}`).join(',');
}

function parseLevelList(axis, text) {
  const spec = AXES[axis];
  if (!spec) throw new Error(`unknown axis "${axis}" (axes: ${AXIS_NAMES.join(', ')})`);
  const parts = String(text).split(',').map((s) => s.trim()).filter(Boolean);
  if (!parts.length) throw new Error(`--levels ${axis}= needs at least one level`);
  const levels = parts.map((p) => {
    if (!/^\d+$/.test(p)) throw new Error(`--levels ${axis}: "${p}" is not a non-negative integer`);
    const n = Number(p);
    if (n < spec.min) throw new Error(`--levels ${axis}: ${n} is below the minimum ${spec.min}`);
    if (n > spec.cap) throw new Error(`--levels ${axis}: ${n} exceeds the safety cap ${spec.cap}`);
    return n;
  });
  return [...new Set(levels)].sort((a, b) => a - b);
}

/**
 * Expand the chosen axes into cases. Each case changes one axis from BASE; identical scales
 * (the base point appears on every axis) are generated once and shared.
 */
export function buildPlan({ axes = AXIS_NAMES, levels = {}, repeat = REPEAT.default, reads = DEFAULT_READS, maxCalls = CALL_BUDGET.default } = {}) {
  for (const a of axes) if (!AXES[a]) throw new Error(`unknown axis "${a}" (axes: ${AXIS_NAMES.join(', ')})`);
  for (const a of Object.keys(levels)) if (!axes.includes(a)) throw new Error(`--levels ${a}: axis is not selected by --axes`);
  if (!Number.isInteger(repeat) || repeat < 1 || repeat > REPEAT.cap) throw new Error(`--repeat takes 1..${REPEAT.cap}`);
  if (!Number.isInteger(maxCalls) || maxCalls < 1 || maxCalls > CALL_BUDGET.cap) throw new Error(`--max-calls takes 1..${CALL_BUDGET.cap}`);
  if (!reads.length) throw new Error('--reads needs at least one read tool');
  for (const r of reads) if (!READ_TOOLS.includes(r)) throw new Error(`--reads: ${r} is not an allowed read-only tool (${READ_TOOLS.join(', ')})`);

  const cases = new Map();
  const axisRows = [];
  for (const axis of axes) {
    const chosen = levels[axis] ?? AXES[axis].defaults;
    const rows = [];
    for (const level of chosen) {
      const scale = { ...BASE, [axis]: level };
      const key = scaleKey(scale);
      if (!cases.has(key)) cases.set(key, { id: `case-${String(cases.size + 1).padStart(2, '0')}`, scale, key, axes: [], estimated_calls: estimateCalls(scale, reads) });
      cases.get(key).axes.push({ axis, level });
      rows.push({ level, case_id: cases.get(key).id });
    }
    axisRows.push({ axis, label: AXES[axis].label, base: BASE[axis], levels: rows });
  }
  const list = [...cases.values()];
  const estimated = list.reduce((n, c) => n + c.estimated_calls, 0) * repeat;
  if (estimated > maxCalls) {
    throw new Error(`plan needs about ${estimated} Core calls (${list.length} cases × ${repeat} repeats), above --max-calls ${maxCalls}; lower the levels or repeats`);
  }
  return { base: { ...BASE }, axes: axisRows, cases: list, repeat, reads: [...reads], max_calls: maxCalls, estimated_calls: estimated };
}

export { parseLevelList };
