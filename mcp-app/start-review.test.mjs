import test from 'node:test';
import assert from 'node:assert/strict';
import { makeStartReview, startedContinuation, heldContinuation } from './start-review.mjs';
import { setLocale, TABLES } from '../ui/locale.js';
setLocale('ko');

class Element {
  constructor(tag) { this.tag = tag; this.children = []; this.listeners = {}; this.textContent = ''; this.hidden = false; this.disabled = false; }
  append(...nodes) { this.children.push(...nodes); }
  replaceChildren(...nodes) { this.children = nodes; }
  addEventListener(name, fn) { this.listeners[name] = fn; }
  querySelectorAll(tag) { return this.children.flatMap((n) => [...(n.tag === tag ? [n] : []), ...n.querySelectorAll(tag)]); }
  allText() { return this.textContent + this.children.map((n) => n.allText()).join(' '); }
  click(trusted) { return this.listeners.click?.({ isTrusted: trusted }); }
}

const proposal = (over = {}) => ({
  revision: 2, digest: 'd'.repeat(64), engine: 'gil-gilignore/1', rules: 'node_modules/\n.env\n', rules_digest: 'r',
  tracked_count: 2, tracked: ['README.md', 'src/<img onerror=x>.py'], tracked_truncated: false,
  excluded: [{ path: 'node_modules', kind: 'directory', reason: 'line 1: node_modules/', interior: 'not_explored' }], excluded_count: 1,
  blockers: [], notices: [], recommendations: [{ key: 'gil:secrets', category: 'secrets', sources: ['gil_catalog'], rules: ['.env'], default_selected: true, in_rules: true, reason: 'secret', impacts: [] }],
  policy_file: { disk: 'absent', effect: 'create' }, impact: ['archive', 'restore'], system_protections: [{ code: 'git_metadata', shown: '.git/' }],
  combination: null, response: null, approvable: true, ...over,
});

function rig({ value, token = 'private-token', applyToken = null, decideResult, loseReply = false, refuse = false } = {}) {
  const panel = new Element('section'), calls = [], messages = [], doc = { createElement: (tag) => new Element(tag) };
  const state = { value: value ?? { phase: 'awaiting_approval', said: 'waiting', proposal: proposal() }, token };
  const app = {
    async callServerTool(call) {
      calls.push(call);
      if (call.name === 'gil_start_review') return { structuredContent: state.value, _meta: state.token ? { 'gil/startApprovalToken': state.token } : applyToken ? { 'gil/startApplyToken': applyToken } : undefined };
      if (refuse) return { isError: true, structuredContent: { said: 'stale' } };
      if (loseReply) throw new Error('reply lost');
      return { structuredContent: decideResult };
    },
    async sendMessage(m) { messages.push(m); return {}; },
  };
  const review = makeStartReview({ app, scope: 'project:' + 'a'.repeat(64), panel, document: doc });
  return { review, panel, calls, messages, state };
}

const buttonsOf = (panel) => panel.querySelectorAll('button');

test('reading and synthetic clicks never approve; the token never reaches the DOM', async () => {
  const r = rig({ decideResult: { phase: 'started', said: 'started', started: { policy: 'policy:P1' } } });
  await r.review.refresh();
  assert.ok(!r.panel.allText().includes('private-token'));
  assert.ok(r.panel.allText().includes('node_modules/\n.env\n'), 'exact rules shown');
  assert.ok(r.panel.allText().includes('<img onerror=x>'), 'paths are text, never markup');
  const [approve] = buttonsOf(r.panel);
  await approve.click(false);
  assert.equal(r.calls.filter((c) => c.name === 'gil_start_decide').length, 0);
  await approve.click(true);
  const decide = r.calls.filter((c) => c.name === 'gil_start_decide');
  assert.equal(decide.length, 1);
  assert.deepEqual(decide[0].arguments, { scope_id: 'project:' + 'a'.repeat(64), revision: 2, digest: 'd'.repeat(64), token: 'private-token', decision: 'approve' });
  assert.equal(r.messages.length, 1);
  assert.match(r.messages[0].content[0].text, /show_gil_monitor/);
  assert.ok(!JSON.stringify(r.messages).includes('private-token'));
});

test('no capability, a blocked proposal or a recorded response shows no usable approve button', async () => {
  for (const value of [
    { phase: 'awaiting_approval', said: 'w', proposal: proposal() },
    { phase: 'blocked', said: 'b', proposal: proposal({ blockers: ['nested .gilignore'], approvable: false }) },
  ]) {
    const r = rig({ value, token: value.phase === 'blocked' ? 'private-token' : null });
    await r.review.refresh();
    const [approve] = buttonsOf(r.panel);
    assert.equal(approve.disabled, true);
    await approve.click(true);
    assert.equal(r.calls.filter((c) => c.name === 'gil_start_decide').length, 0);
  }
  const done = rig({ value: { phase: 'approved', said: 'a', proposal: proposal({ response: { decision: 'approve', source: 'monitor_button' } }) }, token: null });
  await done.review.refresh();
  assert.equal(buttonsOf(done.panel).filter((b) => b.textContent === TABLES.ko['start.approve']).length, 0);
});

test('a lost reply is not resent; the person rereads the stored state', async () => {
  const r = rig({ loseReply: true });
  await r.review.refresh();
  await buttonsOf(r.panel)[0].click(true);
  assert.equal(r.calls.filter((c) => c.name === 'gil_start_decide').length, 1);
  assert.ok(r.panel.allText().includes(TABLES.ko['start.result_unknown']));
  const reread = buttonsOf(r.panel).find((b) => b.textContent === TABLES.ko['start.reread']);
  r.state.token = null;
  await reread.click(true);
  assert.equal(r.calls.filter((c) => c.name === 'gil_start_decide').length, 1, 'never re-sent');
  assert.equal(r.calls.filter((c) => c.name === 'gil_start_review').length, 2);
});

test('a held application is shown with its reason and the agent is told to read the status', async () => {
  const r = rig({ decideResult: { phase: 'approved', said: 'approved', application_held: { code: 'combine_required', said: 'combine' } } });
  await r.review.refresh();
  await buttonsOf(r.panel)[0].click(true);
  assert.ok(r.panel.allText().includes('combine_required'));
  assert.match(r.messages[0].content[0].text, /gil_start_status/);
  assert.match(heldContinuation('combine_required'), /combine_with_revision/);
  assert.match(startedContinuation('project:x'), /"scope_id":"project:x"/);
});

test('a combined proposal shows both differences and the rule-order effects', async () => {
  const r = rig({ value: { phase: 'awaiting_approval', said: 'w', proposal: proposal({ combination: {
    approved_revision: 1, approved_rules: 'node_modules/\n', disk_rules: '*.log\n',
    diff_vs_approved: ['  node_modules/', '+ *.log'], diff_vs_disk: ['+ node_modules/', '  *.log'],
    rule_order_effects: [{ path: 'a.log', decided: 'excluded by line 2: *.log' }] } }) } });
  await r.review.refresh();
  const all = r.panel.allText();
  assert.ok(all.includes('+ *.log') && all.includes('+ node_modules/') && all.includes('excluded by line 2: *.log'));
});

test('every language table carries the start panel strings', () => {
  for (const [lang, table] of Object.entries(TABLES)) {
    for (const key of Object.keys(TABLES.en).filter((k) => k.startsWith('start.') || k === 'html.start_review_aria')) {
      assert.ok(typeof table[key] === 'string' && table[key], `${lang} ${key}`);
    }
  }
});

test('an approved but held start offers only a trusted retry of that exact proposal with its own capability', async () => {
  const value = { phase: 'approved', said: 'a', proposal: proposal({ response: { decision: 'approve', source: 'monitor_button' } }) };
  const r = rig({ value, token: null, applyToken: 'apply-token', decideResult: { phase: 'started', said: 's', started: { policy: 'policy:P1' } } });
  await r.review.refresh();
  const retry = buttonsOf(r.panel).find((b) => b.textContent === TABLES.ko['start.apply_retry']);
  assert.ok(retry && !retry.disabled);
  assert.ok(!r.panel.allText().includes('apply-token'));
  await retry.click(false);
  assert.equal(r.calls.filter((c) => c.name === 'gil_start_apply').length, 0);
  await retry.click(true);
  const sent = r.calls.filter((c) => c.name === 'gil_start_apply');
  assert.deepEqual(sent.map((c) => c.arguments), [{ scope_id: 'project:' + 'a'.repeat(64), revision: 2, digest: 'd'.repeat(64), token: 'apply-token' }]);
  assert.equal(r.calls.filter((c) => c.name === 'gil_start_decide').length, 0, 'a retry is not a new approval');
});
