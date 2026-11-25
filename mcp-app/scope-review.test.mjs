import test from 'node:test';
import assert from 'node:assert/strict';
import { makeScopeReview, scopeContinuation } from './scope-review.mjs';
import { setLocale } from '../ui/locale.js';
setLocale('ko');

// Synthetic checks of the scope panel: a fake DOM and a fake App. They show what the panel
// sends for which click; they are not a Host acceptance and no person pressed anything.
class Element {
  constructor(tag) { this.tag = tag; this.children = []; this.listeners = {}; this.textContent = ''; this.hidden = false; this.disabled = false; this.value = ''; this.checked = false; }
  append(...nodes) { this.children.push(...nodes); }
  replaceChildren(...nodes) { this.children = nodes; }
  addEventListener(name, fn) { this.listeners[name] = fn; }
  querySelectorAll(tag) { return this.children.flatMap((n) => [...(n.tag === tag ? [n] : []), ...n.querySelectorAll(tag)]); }
  allText() { return this.textContent + this.children.map((n) => n.allText()).join(' '); }
  click(trusted) { return this.listeners.click?.({ isTrusted: trusted }); }
}

const tracking = (over = {}) => ({
  chain: 'chain:C1', standing: { cycle: 'cycle:C2', state: 'closed_boundary' }, applied_policy: 'policy:P2', applied_by: 'event:M2',
  last_confirmed_world: 'snapshot:A1', confirmed_policy: 'policy:P1', basis_agrees: false,
  waiting: { event: 'event:M2', policy: 'policy:P2', world_before: 'snapshot:A1', failed_attempts: ['external_change'] },
  archive_halt: null, ...over,
});
const halted = (over = {}) => ({
  phase: 'applied', said: 'state', policy_file: 'synced', cleanups: ['continue', 'end_unachieved', 'propose_scope_change'],
  tracking: tracking({ archive_halt: 'event:M2' }), halt: { at: 'event:M2', id: 'a'.repeat(32), state_digest: 'b'.repeat(64) }, ...over,
});

function rig({ value, meta = {}, result, loseReply = false, refuse = false } = {}) {
  const panel = new Element('section'), calls = [], messages = [], doc = { createElement: (tag) => new Element(tag) };
  const state = { value, meta };
  const app = {
    async callServerTool(call) {
      calls.push(call);
      if (call.name === 'gil_scope_review') return { structuredContent: state.value, _meta: state.meta };
      if (refuse) return { isError: true, structuredContent: { said: 'stale' } };
      if (loseReply) throw new Error('reply lost');
      return { structuredContent: result ?? state.value };
    },
    async sendMessage(m) { messages.push(m); return {}; },
  };
  const review = makeScopeReview({ app, scope: 'project:' + 'a'.repeat(64), panel, document: doc });
  const buttons = () => panel.querySelectorAll('button');
  const button = (label) => buttons().find((b) => b.textContent === label);
  const sent = () => calls.filter((c) => c.name !== 'gil_scope_review');
  return { panel, calls, messages, state, review, button, buttons, sent };
}

test('the three values are shown apart and a pending baseline offers only an explicit retry', async () => {
  const r = rig({ value: { phase: 'applied', said: 'state', policy_file: 'synced', tracking: tracking() }, meta: { 'gil/scopeRetryToken': 'retry-token', 'gil/scopeHaltToken': 'halt-token' } });
  await r.review.refresh();
  const text = r.panel.allText();
  assert.ok(text.includes('policy:P2') && text.includes('snapshot:A1') && text.includes('policy:P1'), text);
  assert.equal(r.sent().length, 0, 'reading starts nothing');
  // A synthetic (untrusted) click sends nothing.
  await r.button('지금 기준점 보관 재시도').click(false);
  assert.equal(r.sent().length, 0);
  await r.button('지금 기준점 보관 재시도').click(true);
  assert.deepEqual(r.sent().map((c) => [c.name, c.arguments.event, c.arguments.token]), [['gil_scope_retry', 'event:M2', 'retry-token']]);
  // The capability never reaches the DOM or the message sent to the Agent.
  assert.ok(!r.panel.allText().includes('retry-token') && !JSON.stringify(r.messages).includes('retry-token'));
  assert.equal(r.messages[0].content[0].text, scopeContinuation('baseline archive retry'));
});

test('release is sent only with both explicit inputs and carries the halt and state the screen showed', async () => {
  const r = rig({ value: halted(), meta: { 'gil/scopeReleaseToken': 'release-token' } });
  await r.review.refresh();
  assert.equal(r.button('지금 보관과 진행 중단'), undefined, 'while halted there is no halt, retry or approve button');
  assert.equal(r.button('지금 기준점 보관 재시도')?.disabled ?? true, true);
  const confirm = r.panel.querySelectorAll('input')[0], select = r.panel.querySelectorAll('select')[0];
  await r.button('중단 해제').click(true);
  assert.equal(r.sent().length, 0, 'no confirmation, no cleanup choice');
  confirm.checked = true;
  await r.button('중단 해제').click(true);
  assert.equal(r.sent().length, 0, 'state confirmed but no cleanup choice');
  select.value = 'continue';
  confirm.checked = false;
  await r.button('중단 해제').click(true);
  assert.equal(r.sent().length, 0, 'cleanup chosen but state not confirmed');
  confirm.checked = true;
  await r.button('중단 해제').click(false);
  assert.equal(r.sent().length, 0, 'untrusted click');
  await r.button('중단 해제').click(true);
  assert.deepEqual(r.sent().map((c) => c.arguments), [{ scope_id: 'project:' + 'a'.repeat(64), at: 'event:M2', halt_id: 'a'.repeat(32),
    state_digest: 'b'.repeat(64), cleanup: 'continue', state_confirmed: true, token: 'release-token' }]);
});

test('a halt without a record offers no release, and a received but unapplied halt offers nothing', async () => {
  const missing = rig({ value: halted({ halt: { at: 'event:M2', id: null, record_missing: true } }), meta: { 'gil/scopeHaltToken': 'halt-token' } });
  await missing.review.refresh();
  assert.equal(missing.button('중단 해제'), undefined);
  await missing.button('이 중단의 기록 다시 남기기').click(true);
  assert.deepEqual(missing.sent().map((c) => c.name), ['gil_scope_halt']);

  const pending = rig({ value: { phase: 'applied', said: 'state', policy_file: 'synced', halt_request_pending: true, tracking: tracking() }, meta: {} });
  await pending.review.refresh();
  assert.equal(pending.button('중단 해제'), undefined);
  assert.equal(pending.button('지금 보관과 진행 중단'), undefined);
  assert.equal(pending.button('지금 기준점 보관 재시도').disabled, true, 'no capability, no retry');
});

test('after a refusal or a lost reply the panel assumes nothing and reads the stored state again', async () => {
  const refused = rig({ value: halted(), meta: { 'gil/scopeReleaseToken': 'release-token' }, refuse: true });
  await refused.review.refresh();
  refused.panel.querySelectorAll('input')[0].checked = true;
  refused.panel.querySelectorAll('select')[0].value = 'continue';
  await refused.button('중단 해제').click(true);
  assert.equal(refused.messages.length, 0, 'a refused request is not announced as done');
  assert.equal(refused.calls.at(-1).name, 'gil_scope_review', 'a refusal is followed by a fresh read');

  const lost = rig({ value: halted(), meta: { 'gil/scopeReleaseToken': 'release-token' }, loseReply: true });
  await lost.review.refresh();
  lost.panel.querySelectorAll('input')[0].checked = true;
  lost.panel.querySelectorAll('select')[0].value = 'continue';
  await lost.button('중단 해제').click(true);
  assert.equal(lost.sent().length, 1, 'the spent capability is not resent');
  assert.ok(lost.button('다시 읽기'), 'only a re-read is offered after an unknown result');
  assert.equal(lost.button('중단 해제'), undefined);
});

test('when the server could not re-read the record the panel says the current state is unconfirmed', async () => {
  const r = rig({ value: { phase: 'unconfirmed', said: 'unconfirmed', current_state: { reread: false }, tracking: null, policy_file: 'synced' }, meta: {} });
  await r.review.refresh();
  assert.ok(r.panel.allText().includes('현재 상태를 확인하지 못했다'), r.panel.allText());
  assert.equal(r.buttons().filter((b) => b.textContent !== '다시 읽기').length, 0);
});

// A rig whose approve/retry answer is held until the test releases it, so a halt can be sent meanwhile.
function slowRig({ value, meta, haltResult }) {
  const panel = new Element('section'), calls = [], doc = { createElement: (tag) => new Element(tag) };
  let release; const held = new Promise((r) => { release = r; });
  const app = {
    async callServerTool(call) {
      calls.push(call);
      if (call.name === 'gil_scope_review') return { structuredContent: value, _meta: meta };
      if (call.name === 'gil_scope_halt') return { structuredContent: haltResult };
      await held;
      return { structuredContent: value };
    },
    async sendMessage() { return {}; },
  };
  const review = makeScopeReview({ app, scope: 'project:' + 'a'.repeat(64), panel, document: doc });
  const button = (label) => panel.querySelectorAll('button').find((b) => b.textContent === label);
  return { panel, calls, review, button, release, sent: (name) => calls.filter((c) => c.name === name) };
}

const proposal = { revision: 3, digest: 'd'.repeat(64), engine: 'gil-gilignore/1', rules: 'data/\n', review_interview: 'cycle:C1',
  policy_before: 'policy:P1', world_before: 'snapshot:A1', confirmed_policy: 'policy:P1', newly_tracked: [], newly_excluded: ['data/raw.csv'], effects: [], response: null };

test('while an approval is running the halt can still be sent, with its own capability and its own answer', async () => {
  const r = slowRig({
    value: { phase: 'awaiting_approval', said: 'waiting', policy_file: 'synced', proposal, tracking: tracking({ waiting: null, basis_agrees: true }) },
    meta: { 'gil/scopeApprovalToken': 'approve-token', 'gil/scopeHaltToken': 'halt-token' },
    haltResult: { said: 'received', halt_request: { received: true, applied: false } },
  });
  await r.review.refresh();
  const running = r.button('이 범위 변경 승인').click(true);
  await Promise.resolve();
  assert.equal(r.sent('gil_scope_decide').length, 1);
  // The approval is in flight: its buttons are disabled, a second click sends nothing, the halt button still works.
  assert.equal(r.button('이 범위 변경 승인').disabled, true);
  await r.button('이 범위 변경 승인').click(true);
  await r.button('변경 요청').click(true);
  assert.equal(r.sent('gil_scope_decide').length, 1, 'no duplicate approval or second decision');
  const halt = r.button('지금 보관과 진행 중단');
  assert.equal(halt.disabled, false, 'the halt stays available during the work');
  await halt.click(false);
  assert.equal(r.sent('gil_scope_halt').length, 0, 'untrusted click');
  await halt.click(true);
  assert.deepEqual(r.sent('gil_scope_halt').map((c) => c.arguments), [{ scope_id: 'project:' + 'a'.repeat(64), token: 'halt-token' }]);
  assert.equal(r.sent('gil_scope_decide')[0].arguments.token, 'approve-token', 'each request carried its own capability');
  // The halt answer is shown as received, not as the approval's result, and no re-read interrupts the running work.
  assert.ok(r.panel.allText().includes('중단 요청: 받았다'), r.panel.allText());
  assert.equal(r.sent('gil_scope_review').length, 1, 'no re-read while the approval is still running');
  await halt.click(true);
  assert.equal(r.sent('gil_scope_halt').length, 1, 'the spent halt capability is not resent');
  r.release();
  await running;
  assert.equal(r.sent('gil_scope_review').length, 2, 'after the approval answers the stored state is read again');
});

test('while an explicit retry is running the halt can still be sent and the retry is not repeated', async () => {
  const r = slowRig({
    value: { phase: 'applied', said: 'state', policy_file: 'synced', tracking: tracking() },
    meta: { 'gil/scopeRetryToken': 'retry-token', 'gil/scopeHaltToken': 'halt-token' },
    haltResult: { said: 'halted', halt_request: { received: true, applied: true }, tracking: tracking({ archive_halt: 'event:M2' }) },
  });
  await r.review.refresh();
  const running = r.button('지금 기준점 보관 재시도').click(true);
  await Promise.resolve();
  await r.button('지금 기준점 보관 재시도').click(true);
  assert.equal(r.sent('gil_scope_retry').length, 1, 'no duplicate retry');
  await r.button('지금 보관과 진행 중단').click(true);
  assert.equal(r.sent('gil_scope_halt').length, 1);
  assert.ok(r.panel.allText().includes('중단 요청: 받았고 적용됐다'), r.panel.allText());
  r.release();
  await running;
});
