import test from 'node:test';
import assert from 'node:assert/strict';
import { initial, apply, active, item, draftFrom, pending, retryPending, currentItem,
  restart, summary, immutableRecords } from './model.mjs';

function step(s, event) {
  const before = structuredClone(s), frozen = immutableRecords(s);
  const next = apply(s, event);
  assert.deepEqual(s, before, 'input is immutable even on successful transitions');
  const after = immutableRecords(next);
  for (const key of Object.keys(frozen)) assert.deepEqual(after[key].slice(0, frozen[key].length), frozen[key], key);
  assert.deepEqual(restart(JSON.stringify(next)), next, 'each boundary roundtrips through independent replay');
  return next;
}
function refused(s, event, code) {
  const before = structuredClone(s);
  assert.throws(() => apply(s, event), error => error.message === code);
  assert.deepEqual(s, before, 'refusal is atomic');
}
const relation = (source = 'A', targets = ['A1', 'A2']) => ({ source, targets, reason: 'Preserve partial evidence; approve remaining scope and B assumptions' });
function split(s, targets = ['A1', 'A2']) {
  const d = draftFrom(s, targets, [relation('A', targets)]);
  d.items = [item('A'), ...targets.map(item), item('B')];
  return d;
}
function propose(s, draft) { return step(s, { type: 'propose', base: s.active, draft }); }
function decide(s, decision = 'approve') {
  const p = s.proposals.at(-1);
  return step(s, { type: 'decide', revision: p.revision, digest: p.digest, decision });
}
function activate(s) { return step(s, { type: 'activate', revision: s.proposals.at(-1).revision }); }
function approved(s, draft) {
  return activate(decide(propose(step(s, { type: 'interview' }), draft)));
}
function execute(s, id, verdict = 'success', reuse = []) {
  s = step(s, { type: 'open', revision: s.active, item: id });
  return step(s, { type: 'finish', verdict, fullCoverage: verdict === 'success', freshObservation: `New observation of ${id}`, reuse });
}
function failed() { return execute(initial(), 'A', 'failure'); }

test('A: split unexecuted A; approval receipt alone never permits execution', () => {
  let s = initial();
  const d = split(s);
  s = propose(step(s, { type: 'interview' }), d);
  refused(s, { type: 'open', revision: 2, item: 'A1' }, 'execution-boundary');
  s = decide(s);
  assert.deepEqual(pending(s), ['A', 'B']);
  refused(s, { type: 'open', revision: 2, item: 'A1' }, 'execution-boundary');
  s = activate(s);
  assert.deepEqual(pending(s), ['A1', 'A2', 'B']);
  refused(s, { type: 'open', revision: 2, item: 'A' }, 'first-pending');
  s = execute(execute(s, 'A1'), 'A2');
  assert.deepEqual(summary(s)[0], { id: 'A', verdict: null, replacedBy: ['A1', 'A2'], unfinished: false });
});

test('B: partial failure is preserved; no M6-E transition is silently assumed', () => {
  let s = failed();
  const original = structuredClone(s.cycles[0]);
  assert.equal(currentItem(s), 'A', 'closed current_item is still the historical binding');
  refused(s, { type: 'interview' }, 'legal-interview-boundary');
  s = step(s, { type: 'recovered' });
  s = approved(s, split(s));
  assert.deepEqual(retryPending(s), []);
  assert.deepEqual(s.cycles[0], original);
  s = execute(s, 'A1', 'success', ['cycle:1']);
  assert.deepEqual(pending(s), ['A2', 'B']);
  assert.equal(summary(s)[0].verdict, 'failure');
  assert.deepEqual(s.cycles[0], original);
});

test('partial scope or old evidence alone cannot establish a new success', () => {
  let s = step(failed(), { type: 'recovered' });
  s = approved(s, split(s));
  s = step(s, { type: 'open', revision: s.active, item: 'A1' });
  refused(s, { type: 'finish', verdict: 'success', fullCoverage: false, freshObservation: 'Only half' }, 'partial-is-not-success');
  refused(s, { type: 'finish', verdict: 'success', fullCoverage: true, reuse: ['cycle:1'] }, 'fresh-observation-required');
  refused(s, { type: 'finish', verdict: 'success', fullCoverage: true, freshObservation: 'Check', reuse: ['cycle:999'] }, 'evidence-reference');
});

test('C: retry keeps ID and approval; replacement gets fresh IDs and new approval', () => {
  let retry = step(failed(), { type: 'recovered' });
  assert.deepEqual(retryPending(retry), ['A']);
  retry = execute(retry, 'A');
  assert.deepEqual(retry.cycles.map(c => [c.item, c.revision, c.verdict]), [['A', 1, 'failure'], ['A', 1, 'success']]);
  let replacement = step(failed(), { type: 'recovered' });
  replacement = approved(replacement, split(replacement, ['X']));
  replacement = execute(replacement, 'X');
  assert.deepEqual(replacement.cycles.map(c => [c.item, c.revision]), [['A', 1], ['X', 2]]);
});

test('D: successful A is immutable; extra work is appended and separately approved', () => {
  let s = execute(initial(), 'A');
  const interview = step(s, { type: 'interview' });
  refused(interview, { type: 'propose', base: 1, draft: split(s) }, 'successful-source');
  s = approved(s, draftFrom(s, ['D']));
  assert.deepEqual(pending(s), ['B', 'D']);
  assert.equal(summary(s)[0].verdict, 'success');
});

test('E: rejection preserves active plan, bindings, pending and evidence', () => {
  let s = step(failed(), { type: 'recovered' });
  const old = { active: active(s), cycles: s.cycles, pending: pending(s), retry: retryPending(s) };
  s = decide(propose(step(s, { type: 'interview' }), split(s)), 'request_changes');
  assert.deepEqual({ active: active(s), cycles: s.cycles, pending: pending(s), retry: retryPending(s) }, old);
  refused(s, { type: 'activate', revision: 2 }, 'approval-required');
  refused(s, { type: 'open', revision: 1, item: 'A' }, 'execution-boundary');
});

test('E: editing before or after receipt requires a fresh exact revision decision', () => {
  let s = propose(step(initial(), { type: 'interview' }), split(initial()));
  const unapproved = structuredClone(s.proposals.at(-1));
  const edit = structuredClone(unapproved.draft);
  edit.replacements[0].reason += '; preserve B input';
  s = propose(s, edit);
  refused(s, { type: 'decide', revision: unapproved.revision, digest: unapproved.digest, decision: 'approve' }, 'stale-decision');
  s = decide(s);
  const accepted = structuredClone(s.proposals.at(-1));
  edit.scope = 'Changed scope requires review';
  s = propose(s, edit);
  assert.deepEqual(s.proposals[accepted.revision - 1], accepted);
  refused(s, { type: 'activate', revision: s.proposals.at(-1).revision }, 'approval-required');
  refused(s, { type: 'decide', revision: accepted.revision, digest: accepted.digest, decision: 'approve' }, 'stale-decision');
});

test('F: disconnect after receipt and after activation; duplicates do not apply twice', () => {
  let s = propose(step(initial(), { type: 'interview' }), split(initial()));
  const count = s.proposals.length;
  s = propose(s, s.proposals.at(-1).draft);
  assert.equal(s.proposals.length, count);
  s = restart(JSON.stringify(decide(s)));
  s = decide(s);
  assert.equal(s.active, 1);
  s = restart(JSON.stringify(activate(s)));
  refused(s, { type: 'activate', revision: 2 }, 'open-proposal-required');
  refused(s, { type: 'open', revision: 1, item: 'A1' }, 'stale-execution');
  s = step(s, { type: 'open', revision: 2, item: 'A1' });
  refused(s, { type: 'open', revision: 2, item: 'A1' }, 'execution-boundary');
  assert.equal(s.cycles.length, 1);
});

test('G: B stays behind all successor leaves, including after later reordering', () => {
  let s = approved(initial(), split(initial()));
  refused(s, { type: 'open', revision: 2, item: 'B' }, 'first-pending');
  s = execute(s, 'A1');
  const d = structuredClone(active(s).draft);
  d.items = [item('A'), item('A1'), item('B'), item('A2')];
  const interview = step(s, { type: 'interview' });
  refused(interview, { type: 'propose', base: 2, draft: d }, 'downstream-order');
  s = execute(s, 'A2');
  s = execute(s, 'B');
  assert.deepEqual(pending(s), []);
  assert.equal(summary(s)[0].verdict, null, 'empty queue never fabricates original success');
});

test('recursive split preserves ancestors and downstream barriers without merging attempts', () => {
  let s = approved(initial(), split(initial()));
  const d = draftFrom(s, ['A11', 'A12'], [relation('A1', ['A11', 'A12'])]);
  d.items = ['A', 'A1', 'A11', 'A12', 'A2', 'B'].map(item);
  s = approved(s, d);
  assert.deepEqual(pending(s), ['A11', 'A12', 'A2', 'B']);
  assert.deepEqual(summary(s)[0].replacedBy, ['A1', 'A2']);
});

test('G: changed B assumptions require a separate replacement, preserving original B', () => {
  let s = initial();
  const d = split(s);
  d.items.push({ ...item('B1'), question: 'Use the newly verified A1/A2 outputs' });
  d.replacements.push(relation('B', ['B1']));
  s = approved(s, d);
  assert.deepEqual(pending(s), ['A1', 'A2', 'B1']);
  assert.deepEqual(active(s).draft.items.find(i => i.id === 'B'), item('B'));
  refused(s, { type: 'open', revision: 2, item: 'B1' }, 'first-pending');
  s = execute(execute(s, 'A1'), 'A2');
  s = execute(s, 'B1');
  assert.deepEqual(summary(s).find(i => i.id === 'B'), { id: 'B', verdict: null, replacedBy: ['B1'], unfinished: false });
});

const invalid = [
  ['missing original', d => { d.items = d.items.filter(i => i.id !== 'A'); }, 'old-definition'],
  ['redefined original', d => { d.items[0].done_when = 'Only a subset'; }, 'old-definition'],
  ['duplicate source', d => d.replacements.push(relation()), 'duplicate-replacement'],
  ['missing source', d => { d.replacements[0].source = 'missing'; }, 'relation-reference'],
  ['foreign Chain source', d => { d.replacements[0].source = 'chain:C9/A'; }, 'relation-reference'],
  ['missing target', d => { d.replacements[0].targets = ['missing']; }, 'relation-reference'],
  ['empty replacement cancellation', d => { d.replacements[0].targets = []; }, 'relation-reference'],
  ['shared target', d => { d.replacements[0].targets = ['A1', 'A1']; }, 'shared-target'],
  ['self cycle', d => { d.replacements[0].targets = ['A']; }, 'relation-cycle'],
  ['indirect cycle', d => d.replacements.push(relation('A1', ['A'])), 'relation-cycle'],
  ['reuse existing B', d => { d.replacements[0].targets = ['B']; }, 'target-not-fresh'],
  ['new source in same revision', d => d.replacements.push(relation('A2', ['B'])), 'source-not-live'],
  ['B overtakes children', d => { d.items = ['A', 'B', 'A1', 'A2'].map(item); }, 'replacement-order'],
];
for (const [label, corrupt, code] of invalid) test(`reject atomically: ${label}`, () => {
  const s = step(initial(), { type: 'interview' }), d = split(s);
  corrupt(d);
  refused(s, { type: 'propose', base: 1, draft: d }, code);
});

test('reject stale base, receipt flip and modification of an activated relation', () => {
  let s = propose(step(initial(), { type: 'interview' }), split(initial()));
  refused(s, { type: 'propose', base: 0, draft: split(initial()) }, 'stale-base');
  s = decide(s);
  const p = s.proposals.at(-1);
  refused(s, { type: 'decide', revision: p.revision, digest: p.digest, decision: 'request_changes' }, 'decision-final');
  s = step(activate(s), { type: 'interview' });
  const d = structuredClone(active(s).draft);
  d.replacements[0].reason = 'Erase old rationale';
  refused(s, { type: 'propose', base: s.active, draft: d }, 'old-relation');
});

test('restore rejects forged history, active pointer, evidence, bindings and cyclic relations', () => {
  let s = step(failed(), { type: 'recovered' });
  s = execute(approved(s, split(s)), 'A1', 'success', ['cycle:1']);
  for (const corrupt of [
    x => { x.active = 1; },
    x => { x.proposals[0].draft.items[0].question = 'Rewritten'; },
    x => { x.cycles[0].evidence = []; },
    x => { x.cycles[0].revision = 2; },
    x => { x.proposals[1].draft.replacements[0].targets = ['A']; },
  ]) {
    const changed = structuredClone(s); corrupt(changed);
    assert.throws(() => restart(JSON.stringify(changed)), /invalid-restored-state/);
  }
});

test('bounded alternative histories preserve all old records across success/failure and decisions', () => {
  for (const prior of ['unexecuted', 'failure']) for (const decision of ['approve', 'request_changes']) for (const result of ['success', 'failure']) {
    let s = prior === 'failure' ? step(failed(), { type: 'recovered' }) : initial();
    s = decide(propose(step(s, { type: 'interview' }), split(s)), decision);
    if (decision === 'request_changes') assert.deepEqual(pending(s), ['A', 'B']);
    else {
      s = execute(activate(s), 'A1', result);
      assert.deepEqual(pending(s), result === 'failure' ? ['A1', 'A2', 'B'] : ['A2', 'B']);
      assert.notEqual(summary(s)[0].verdict, 'success');
    }
  }
});
