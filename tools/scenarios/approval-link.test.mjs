// Wave 2 D (review R5): approval_before_execution and its neighbours link an execution to the
// valid approval of ITS OWN Chain, or say unknown. Every fixture here is synthetic scorer input
// built from imitation Core receipts; none is an observation of the product authorizing anything.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseEvents, replay } from './lib/trace.mjs';
import { scoreProcedure, approvalLink } from './lib/score.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const fixtures = join(here, 'fixtures', 'approval');

import { builder } from './fixtures/approval/builder.mjs';

const judge = (events) => Object.fromEntries(scoreProcedure(replay(parseEvents(events.map((e) => JSON.stringify(e)).join('\n')))).map((x) => [x.id, x]));

test('positive: two Chains, each with its own approval, both executions pass', () => {
  const b = builder(); b.start();
  const p1 = b.interview(1);
  b.experiment(2, { proposal: { ...p1, receipt: { decision: 'approve', source: 'monitor_button' } } });
  b.interview(3, { extra: { chain_verdict: 'success' } }); b.closeChain(1, 3);
  b.openChain(2, 4);
  const q1 = b.interview(4, { digest: 'f'.repeat(64) });
  b.experiment(5, { proposal: { ...q1, receipt: { decision: 'approve', source: 'monitor_button' } }, item: 'X' });
  const j = judge(b.events);
  assert.equal(j.approval_before_execution.status, 'pass', j.approval_before_execution.detail);
  assert.match(j.approval_before_execution.detail, /2 Experiment open/);
  assert.equal(j.no_reused_approval.status, 'pass', j.no_reused_approval.detail);
  assert.equal(j.closing_interview_before_chain_close.status, 'pass', j.closing_interview_before_chain_close.detail);
});

test('negative: an earlier Chain approval does not authorize the next Chain (review R5)', () => {
  const b = builder(); b.start();
  const p1 = b.interview(1);
  b.experiment(2, { proposal: { ...p1, receipt: { decision: 'approve', source: 'monitor_button' } } });
  b.interview(3, { extra: { chain_verdict: 'success' } }); b.closeChain(1, 3);
  b.openChain(2, 4);
  b.experiment(5, { active: 1, item: 'X' }); // no proposal, no decision in Chain #2
  const j = judge(b.events);
  assert.equal(j.approval_before_execution.status, 'fail');
  assert.match(j.approval_before_execution.detail, /cycle:C5.*only an earlier Chain's approval/);
});

test('negative: approval given before an edit (superseded revision) is not an approval of the execution', () => {
  const b = builder(); b.start();
  b.interview(1, { editAfterDecision: true, closeRevision: 1 });
  b.experiment(2);
  const j = judge(b.events);
  assert.equal(j.approval_before_execution.status, 'fail');
  assert.match(j.approval_before_execution.detail, /revision 1 was superseded by revision 2/);
  assert.equal(j.no_reused_approval.status, 'fail');
});

test('negative: request_changes is a receipt but not consent', () => {
  const b = builder(); b.start();
  b.interview(1, { decision: 'request_changes' });
  b.raw('gil_open', { project_root: '/p', kind: 'experiment' }, { structuredContent: { ok: true, said: '열었다: cycle:C2 · experiment\n' }, isError: false });
  const j = judge(b.events);
  assert.equal(j.approval_before_execution.status, 'fail');
  assert.match(j.approval_before_execution.detail, /request_changes on revision 1/);
});

test('negative: a decide whose reply was lost leaves no receipt', () => {
  const b = builder(); b.start();
  b.interview(1, { answered: false });
  b.experiment(2);
  const j = judge(b.events);
  assert.equal(j.approval_before_execution.status, 'fail');
  assert.match(j.approval_before_execution.detail, /got no response/);
});

test('positive: a lost decide reply is recovered when a later read shows the receipt; duplicate decides are harmless', () => {
  const b = builder(); b.start();
  const p = b.interview(1, { answered: false });
  const receipt = { decision: 'approve', source: 'monitor_button' };
  // the App retries and the model re-reads before closing: the receipt is observed through gil_plan_read
  b.events.splice(b.events.length - 4, 0,
    { seq: 0, actor: 'app', request: { name: 'gil_plan_decide', arguments: { scope_id: 'project:aa', revision: 1, digest: p.digest, token: 'x'.repeat(64), decision: 'approve' } }, result: { structuredContent: { supported: true, proposal: { ...p, receipt }, pending: [], reinterview_requests: [], executed_cycles: [] }, isError: false } },
    { seq: 0, actor: 'model', request: { name: 'gil_plan_read', arguments: { project_root: '/p' } }, result: { structuredContent: { supported: true, proposal: { ...p, receipt }, pending: [], reinterview_requests: [], executed_cycles: [] }, isError: false } });
  b.events.forEach((e, i) => { e.seq = i + 1; });
  b.experiment(2, { proposal: { ...p, receipt } });
  const j = judge(b.events);
  assert.equal(j.approval_before_execution.status, 'pass', j.approval_before_execution.detail);
});

test('unknown: a log that begins after the approval cannot prove it', () => {
  const b = builder();
  b.read({ revision: 1, synthesis_ref: 'step:C1/S3', digest: 'd'.repeat(64), receipt: { decision: 'approve', source: 'monitor_button' } }, { active_revision: 1 });
  const j = judge(b.events.slice(0, 0));
  assert.equal(j.approval_before_execution.status, 'not_run');
  const c = builder();
  c.raw('gil_plan_read', { project_root: '/p' }, { structuredContent: { supported: true, proposal: null, active_revision: 1, pending: [{ id: 'A' }], reinterview_requests: [], executed_cycles: [] }, isError: false });
  c.experiment(2);
  const k = judge(c.events);
  assert.equal(k.approval_before_execution.status, 'unknown');
  assert.match(k.approval_before_execution.detail, /log begins after the approval/);
});

test('unknown: approval observed but no read inside the Experiment confirms the active revision', () => {
  const b = builder(); b.start();
  b.interview(1);
  b.experiment(2, { read: false });
  const j = judge(b.events);
  assert.equal(j.approval_before_execution.status, 'unknown');
  assert.match(j.approval_before_execution.detail, /no gil_plan_read inside the Experiment/);
});

test('negative: the read inside the Experiment shows another active revision or no bound item', () => {
  const b = builder(); b.start();
  b.interview(1);
  b.experiment(2, { active: 7 });
  assert.match(judge(b.events).approval_before_execution.detail, /active_revision 7, not an approved revision/);
  const c = builder(); c.start();
  c.interview(1);
  c.experiment(2, { item: null });
  assert.match(judge(c.events).approval_before_execution.detail, /no bound current_item/);
});

test('negative: a Chain closed on the previous Chain\'s closing Synthesis', () => {
  const b = builder(); b.start();
  b.interview(1, { extra: { chain_verdict: 'success' } }); b.closeChain(1, 1);
  b.openChain(2, 2);
  b.interview(2, { digest: 'f'.repeat(64) });
  b.closeChain(2, 1);
  const j = judge(b.events);
  assert.equal(j.closing_interview_before_chain_close.status, 'fail');
  assert.match(j.closing_interview_before_chain_close.detail, /chain:C2: closed without an approved closing Synthesis .* in its own Chain/);
});

test('approvalLink is exported for other harnesses and never reads outside the facts', () => {
  const b = builder(); b.start(); b.interview(1); b.experiment(2);
  const facts = replay(parseEvents(b.events.map((e) => JSON.stringify(e)).join('\n')));
  assert.equal(approvalLink(facts, facts.experimentOpens[0]).status, 'pass');
});

test('committed JSONL fixtures score as their file names say', () => {
  const files = readdirSync(fixtures).filter((f) => f.endsWith('.jsonl')).sort();
  assert.ok(files.length >= 4, 'approval fixtures present');
  for (const f of files) {
    const want = f.replace(/^\d+-/, '').replace(/-.*$/, ''); // NN-<status>-<name>.jsonl
    const facts = replay(parseEvents(readFileSync(join(fixtures, f), 'utf8')));
    const got = scoreProcedure(facts).find((x) => x.id === 'approval_before_execution');
    assert.equal(got.status, want, `${f}: ${got.detail}`);
  }
});

