// Runs the M6-E design fixtures against the pure model and checks the transition table for
// contradictions. Standard library only: node --test design-tests/m6e/m6e.test.mjs
// This is a design check, not a GIL Core test. Passing here proves the table is self-consistent,
// not that any Rust implementation exists or behaves this way.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { TABLE, Refused, apply, initial, checkInvariants, closedRecords } from './model.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const fixturesDir = join(here, 'fixtures');
const shared = JSON.parse(readFileSync(join(fixturesDir, '_shared.json'), 'utf8'));
const files = readdirSync(fixturesDir).filter(f => f.endsWith('.json') && !f.startsWith('_')).sort();

const scenarios = [];
for (const f of files) {
  const doc = JSON.parse(readFileSync(join(fixturesDir, f), 'utf8'));
  for (const s of doc.scenarios ?? [doc]) scenarios.push({ file: f, ...s });
}

function expand(steps) {
  const out = [];
  for (const step of steps) {
    if (step.macro) {
      const body = shared.macros[step.macro];
      assert.ok(body, `unknown macro ${step.macro}`);
      body.forEach((s, i) => out.push(i === body.length - 1 ? { ...structuredClone(s), assert: step.assert } : structuredClone(s)));
    } else out.push(step);
  }
  return out;
}

function pick(state, path) {
  const parts = Array.isArray(path) ? path : path.split('.');
  let v = state;
  for (const p of parts) { if (v == null) return undefined; v = v[p]; }
  return v;
}

const used = { transitions: new Set(), refusals: new Set() };

function run(scenario) {
  let state = initial(scenario.preset, scenario.options ?? {});
  assert.deepEqual(checkInvariants(state), [], 'initial state must satisfy invariants');
  const steps = expand(scenario.steps);
  steps.forEach((step, i) => {
    const label = `${scenario.name} / step ${i + 1} (${step.event})`;
    const before = structuredClone(state);
    const frozen = closedRecords(before);
    let result, error;
    try { result = apply(state, step.event, step.args ?? {}); } catch (e) { error = e; }
    if (step.expect?.refused) {
      assert.ok(error instanceof Refused, `${label}: expected refusal ${step.expect.refused}, got ${error ? error.message : 'ok'}`);
      assert.equal(error.code, step.expect.refused, `${label}: wrong refusal`);
      used.refusals.add(error.code);
      // A refusal is all-or-nothing: the caller's state is untouched because apply works on a copy.
      assert.deepEqual(state, before, `${label}: refusal must leave state unchanged`);
      return;
    }
    if (error) throw new Error(`${label}: unexpected ${error.message}`);
    used.transitions.add(result.transition);
    state = result.state;
    const problems = checkInvariants(state);
    assert.deepEqual(problems, [], `${label}: invariants broken: ${problems.join('; ')}`);
    // Closed records never change: every record closed before the event is identical after it.
    const after = closedRecords(state);
    for (const [k, v] of Object.entries(frozen)) assert.deepEqual(after[k], v, `${label}: closed record ${k} changed`);
    for (const [path, want] of Object.entries(step.assert ?? {})) {
      if (path.startsWith('$path')) {
        const [p, w] = want;
        assert.deepEqual(pick(state, p) ?? null, w, `${label}: ${JSON.stringify(p)}`);
      } else assert.deepEqual(pick(state, path) ?? null, want, `${label}: ${path}`);
    }
  });
  return state;
}

test('transition table is well formed', () => {
  const ids = TABLE.transitions.map(t => t.id);
  assert.equal(new Set(ids).size, ids.length, 'transition ids unique');
  const events = TABLE.transitions.map(t => t.event);
  assert.equal(new Set(events).size, events.length, 'one row per event (deterministic dispatch)');
  for (const t of TABLE.transitions) for (const code of t.preconditions) assert.ok(TABLE.refusals[code], `${t.id} names unknown refusal ${code}`);
  for (const kind of ['interview', 'experiment']) {
    const g = TABLE.grammar[kind];
    for (const [from, tos] of Object.entries(g)) for (const to of tos) assert.ok(g[to] !== undefined, `${kind}.${from}→${to} names unknown kind`);
    assert.deepEqual(g.outcome, [], `${kind}: outcome has no child; the cycle exit follows`);
  }
  const actions = new Set(['open_child', 'revisit', 'close_chain']);
  for (const [kind, table] of Object.entries(TABLE.cycle_report.allowed_actions)) for (const [key, list] of Object.entries(table)) {
    const [verdict, conclusion] = key.split('/');
    assert.ok(['success', 'failure'].includes(verdict) && TABLE.outcome.conclusion_values.includes(conclusion), `${kind} ${key}`);
    for (const a of list) assert.ok(actions.has(a), `${kind} ${key} → ${a}`);
    if (conclusion === 'not_reached') assert.equal(verdict, 'failure', 'not_reached is only ever a failure row');
  }
  for (const k of Object.keys(TABLE.approved_plan.unreachable_in_model)) assert.ok(TABLE.refusals[k], `unreachable list names unknown code ${k}`);
});

test('the only extension to the existing Cycle Report rule is the failure/not_reached row', () => {
  const base = { experiment: { 'success/judged': ['open_child'], 'failure/judged': ['revisit'] }, interview: { 'success/judged': ['open_child', 'close_chain'] } };
  const { 'failure/not_reached': extension, ...rest } = TABLE.cycle_report.allowed_actions.experiment;
  assert.deepEqual(rest, base.experiment);
  assert.deepEqual(TABLE.cycle_report.allowed_actions.interview, base.interview);
  assert.deepEqual(extension, ['revisit', 'open_child']);
  assert.deepEqual(TABLE.outcome.allowed_actions, { experiment: { success: ['close_cycle'], failure: ['revisit', 'close_cycle'] }, interview: { success: ['close_cycle'] } }, 'step-level Outcome rule is unchanged');
});

for (const scenario of scenarios) {
  test(`${scenario.file}: ${scenario.name}`, () => { run(scenario); });
}

test('model is deterministic: the same event on the same state yields the same state', () => {
  const s0 = initial('approved_plan_ready');
  const a = apply(s0, 'open_cycle', { kind: 'experiment' }).state;
  const b = apply(s0, 'open_cycle', { kind: 'experiment' }).state;
  assert.deepEqual(a, b);
  assert.deepEqual(s0, initial('approved_plan_ready'), 'apply never mutates its input');
});

test('every transition row and every reachable refusal is exercised by a fixture', () => {
  for (const scenario of scenarios) run(scenario); // ensure coverage sets are complete regardless of test order
  const missingT = TABLE.transitions.map(t => t.id).filter(id => !used.transitions.has(id));
  assert.deepEqual(missingT, [], 'transitions without a fixture');
  const unreachable = new Set(Object.keys(TABLE.approved_plan.unreachable_in_model));
  const missingR = Object.keys(TABLE.refusals).filter(c => !used.refusals.has(c) && !unreachable.has(c));
  assert.deepEqual(missingR, [], 'refusal codes without a fixture');
  for (const c of unreachable) assert.ok(!used.refusals.has(c), `${c} is declared unreachable but a fixture reached it`);
});
