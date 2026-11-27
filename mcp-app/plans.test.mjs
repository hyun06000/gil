import test from 'node:test';
import assert from 'node:assert/strict';
import { makePlanReview, planSummary, planItems, currentCycle, verdictSentence, bannerLine, PLAN_STATE, continuation, decisionKind } from './plans.mjs';
import { makePlanPolling } from './plan-poll.mjs';
// These assertions read the Korean table; the language-specific cases live in locale.test.mjs.
import { setLocale } from '../ui/locale.js';
setLocale('ko');

// Deterministic controller fixture. Synthetic trusted flags test event routing, not physical user identity.
class Element {
  constructor(tag, doc) {
    this.tag = tag; this.doc = doc; this.children = []; this.listeners = {}; this.textContent = '';
    this.dataset = {}; this.hidden = false; this.disabled = false; this.className = ''; this.tabIndex = 0;
    this.scrolled = 0;
  }
  // Like a browser: disabling the focused control drops focus to the document.
  get disabled() { return this._disabled ?? false; }
  set disabled(v) { this._disabled = v; if (v && this.doc.activeElement === this) this.doc.activeElement = null; }
  append(...nodes) { this.children.push(...nodes); }
  replaceChildren(...nodes) { this.children = nodes; }
  addEventListener(name, fn) { this.listeners[name] = fn; }
  querySelectorAll(tag) { return this.children.flatMap(n => [...(n.tag === tag ? [n] : []), ...n.querySelectorAll(tag)]); }
  allText() { return this.textContent + this.children.map(n => n.allText()).join(' '); }
  contains(node) { return node === this || this.children.some(n => n.contains(node)); }
  focus() { this.doc.activeElement = this; }
  scrollIntoView() { this.scrolled += 1; }
  click(trusted) { return this.listeners.click?.({ isTrusted: trusted }); }
}
const DIGEST = 'a'.repeat(64);
function proposal(extra = {}) {
  return { revision: 1, base_revision: null, digest: DIGEST, synthesis_ref: 'step:C1/S3', receipt: null,
    draft: { scope: { goal: 'Safe <img onerror=attack>', in_scope: 'in', out_of_scope: 'out', constraints: 'none', success_criteria: 'proof' },
      items: [{ id: 'A', question: 'Question A', done_when: 'File A' }, { id: 'B', question: 'Question B', done_when: 'File B' }],
      synthesis: { statement: 'Proposed scope', basis_refs: 'step:C1/S1\nstep:C1/S2', exploration_checklist: 'A\nB' } }, ...extra };
}
function rig({ withBanner = true, value: initial } = {}) {
  const doc = { activeElement: null };
  doc.createElement = tag => new Element(tag, doc);
  const panel = new Element('section', doc), banner = withBanner ? new Element('section', doc) : null, calls = [], messages = [];
  let change = 0;
  const value = initial ?? { supported: true, active_revision: null, pending: [], executed_cycles: [], reinterview_requests: [], current_item: null, proposal: proposal() };
  const state = { value, token: 'private-token', error: false, loseReply: false, dropBeforeSave: false, refuseMessage: false, tokenMissing: false };
  const app = {
    async sendMessage(m) { if (state.refuseMessage) throw new Error('no chat'); messages.push(m); return {}; },
    async callServerTool(request) {
      calls.push(structuredClone(request));
      if (state.error) throw new Error('offline');
      if (request.name === 'gil_plan_decide') {
        assert.equal(request.arguments.token, state.token);
        assert.equal(request.arguments.digest, state.value.proposal.digest);
        if (state.dropBeforeSave) throw new Error('timeout before save');
        state.value = structuredClone(state.value);
        state.value.proposal.receipt = { decision: request.arguments.decision, source: 'monitor_button' };
        if (state.loseReply) throw new Error('reply lost');
        if (state.badAck) return { structuredContent: { supported: true } };
      }
      const token = state.value.proposal && !state.value.proposal.receipt && !state.tokenMissing ? state.token : undefined;
      return { structuredContent: structuredClone(state.value), _meta: { 'gil/planApprovalToken': token } };
    },
  };
  const review = makePlanReview({ app, scope: 'project:one', panel, banner, document: doc, changed: () => change++ });
  const decisions = () => panel.querySelectorAll('button').filter(b => b.dataset.decision);
  const actions = (name) => panel.querySelectorAll('button').filter(b => b.dataset.action === name);
  const decides = () => calls.filter(c => c.name === 'gil_plan_decide').length;
  return { doc, panel, banner, calls, messages, state, review, decisions, actions, decides, changes: () => change };
}

test('rendering and synthetic clicks cannot approve; displayed version and private capability accompany explicit input', async () => {
  const r = rig(); await r.review.refresh();
  assert.equal(r.calls.length, 1);
  assert.equal(r.messages.length, 0);
  assert.equal(r.calls[0].name, 'gil_plan_review');
  assert.match(r.panel.allText(), /Safe <img onerror=attack>/);
  assert.match(r.panel.allText(), /File A/);
  assert.match(r.panel.allText(), /step:C1\/S2/);
  assert.ok(!r.panel.allText().includes('private-token'));
  assert.ok(!r.banner.allText().includes('private-token'));
  const buttons = r.decisions();
  assert.deepEqual(buttons.map(b => b.dataset.decision), ['approve', 'request_changes']);
  await buttons[0].click(false); assert.equal(r.calls.length, 1);
  await buttons[0].click(true);
  assert.equal(r.calls[1].name, 'gil_plan_decide');
  assert.deepEqual(r.calls[1].arguments, { scope_id: 'project:one', revision: 1, digest: DIGEST, token: 'private-token', decision: 'approve' });
  assert.equal(r.messages.length, 1);
  assert.ok(!JSON.stringify(r.messages).includes('private-token'));
  assert.equal(r.changes(), 1); assert.equal(r.decisions().length, 0);
  assert.equal(r.panel.dataset.state, 'approved');
  assert.match(r.panel.allText(), /채팅에 계속 요청을 전달했습니다/);
  await buttons[0].click(true); assert.equal(r.calls.length, 2);
});

test('proposal changes replace buttons; polling preserves focus and cannot reuse the previous proposal', async () => {
  const r = rig(); await r.review.refresh();
  const first = r.decisions()[0];
  first.focus();
  await r.review.refresh(); assert.equal(r.decisions()[0], first, 'an unchanged value does not rebuild the panel');
  assert.equal(r.doc.activeElement, first);
  r.state.value.proposal.revision = 2; r.state.value.proposal.digest = 'b'.repeat(64); r.state.token = 'new-token';
  await r.review.refresh();
  assert.notEqual(r.decisions()[0], first);
  assert.equal(r.doc.activeElement, r.decisions()[0], 'focus moves to the same decision on the new proposal');
  assert.equal(r.panel.scrolled, 0, 'polling never scrolls');
  assert.match(r.panel.allText(), /보고 있던 제안이 새 판으로 바뀌었습니다/);
  await first.click(true); assert.ok(r.calls.every(c => c.name === 'gil_plan_review'));
  await r.decisions()[1].click(true);
  assert.equal(r.calls.at(-1).arguments.decision, 'request_changes');
  assert.equal(r.calls.at(-1).arguments.revision, 2);
  assert.equal(r.panel.dataset.state, 'changes_requested');
});

test('lost reply reconciles the recorded decision instead of automatically sending another approval', async () => {
  const r = rig(); await r.review.refresh(); r.state.loseReply = true;
  await r.decisions()[0].click(true);
  assert.equal(r.decisions().length, 0);
  assert.equal(r.panel.dataset.state, 'uncertain');
  assert.match(r.banner.allText(), /저장 결과 불확실/);
  assert.match(r.panel.allText(), /응답은 다시 보내지 않습니다/);
  await r.actions('reread')[0].click(true);
  assert.match(r.panel.allText(), /승인을 기록/);
  assert.equal(r.panel.dataset.state, 'approved');
  assert.equal(r.decides(), 1);
  assert.equal(r.messages.length, 0, 'reconciling a receipt must not impersonate a fresh user click');
  assert.match(r.panel.allText(), /채팅 전달 여부를 확인하지 못했습니다/);
  assert.equal(r.actions('notify').length, 1, 'the person can still ask the chat to continue by an explicit click');
});

test('a reply dropped before saving shows the proposal again with fresh buttons, without a second automatic send', async () => {
  const r = rig(); await r.review.refresh(); r.state.dropBeforeSave = true;
  await r.decisions()[0].click(true);
  assert.equal(r.panel.dataset.state, 'uncertain');
  r.state.dropBeforeSave = false;
  await r.review.refresh();
  assert.equal(r.panel.dataset.state, 'requested');
  assert.equal(r.decisions().length, 2);
  assert.equal(r.decides(), 1);
  assert.equal(r.messages.length, 0);
});

test('disconnect and teardown remove actionable buttons; legacy projects have no approval panel', async () => {
  const r = rig(); await r.review.refresh(); r.state.error = true;
  await r.review.refresh();
  assert.equal(r.decisions().length, 0);
  assert.equal(r.panel.dataset.state, 'retrying');
  assert.match(r.panel.allText(), /마지막으로 읽은 상태: 승인 요청 · 계획 v1/);
  assert.match(r.banner.allText(), /다시 확인 중/);
  assert.equal(r.actions('reread').length, 1);
  r.state.error = false; r.state.value = { supported: false };
  await r.review.refresh(); assert.equal(r.panel.hidden, true); assert.equal(r.banner.hidden, true);
  r.review.stop(); const n = r.calls.length; await r.review.refresh(); assert.equal(r.calls.length, n);
  assert.equal(r.banner.hidden, true); assert.equal(r.banner.children.length, 0);
});

test('a saved approval remains a confirmed historical fact through transient and persistent read failure', async () => {
  const r = rig(); await r.review.refresh();
  await r.decisions()[0].click(true);
  assert.equal(r.decides(), 1); assert.equal(r.messages.length, 1);
  r.state.error = true;
  for (let n = 1; n <= 5; n++) {
    const result = await r.review.refresh();
    assert.equal(result.status, 'failed');
    assert.equal(result.retryAfterMs, [2000, 4000, 8000, 15000, 15000][n - 1]);
    assert.match(r.banner.allText(), /계획 v1 승인 기록을 확인했습니다/);
    assert.equal(r.panel.dataset.state, n < 3 ? 'retrying' : 'unreadable');
    assert.equal(r.decisions().length, 0);
    assert.equal(r.actions('reread').length, 1);
  }
  assert.equal(r.decides(), 1); assert.equal(r.messages.length, 1);
  r.state.error = false;
  r.state.value = { ...r.state.value, proposal: proposal({ revision: 2, digest: 'b'.repeat(64), base_revision: 1 }), active_revision: 1 };
  await r.review.refresh();
  assert.equal(r.panel.dataset.state, 'reapproval', 'old confirmed approval must not approve a replacement proposal');
  assert.match(r.banner.allText(), /계획 v2/);
  r.state.error = true; const next = await r.review.refresh();
  assert.equal(next.retryAfterMs, 2000, 'a successful read resets the failure ladder');
  assert.match(r.banner.allText(), /계획 v1 승인 기록/);
  assert.doesNotMatch(r.banner.allText(), /계획 v2 승인 기록/);
});

test('unknown save results stay uncertain during read failures; polling never repeats a decision', async () => {
  const r = rig(); await r.review.refresh(); r.state.loseReply = true;
  await r.decisions()[0].click(true);
  r.state.error = true;
  for (let i = 0; i < 3; i++) await r.review.refresh();
  assert.equal(r.panel.dataset.state, 'uncertain');
  assert.doesNotMatch(r.banner.allText(), /승인 기록을 확인했습니다/);
  assert.equal(r.decides(), 1); assert.equal(r.messages.length, 0);
  r.state.error = false; await r.review.refresh();
  assert.equal(r.panel.dataset.state, 'approved');
  assert.equal(r.decides(), 1); assert.equal(r.messages.length, 0);
});

test('first-read failures are neutral retries first, never a saved-approval claim', async () => {
  const r = rig(); r.state.error = true;
  await r.review.refresh();
  assert.equal(r.panel.dataset.state, 'retrying');
  assert.doesNotMatch(r.banner.allText(), /승인 기록을 확인했습니다/);
  assert.equal(r.decisions().length, 0);
  await r.review.refresh(); await r.review.refresh();
  assert.equal(r.panel.dataset.state, 'unreadable');
  assert.match(r.panel.allText(), /offline/);
  r.state.error = false; await r.actions('reread')[0].click(true);
  assert.equal(r.panel.dataset.state, 'requested');
});

test('a successful transport reply without the matching saved receipt cannot claim approval or send chat', async () => {
  const r = rig(); await r.review.refresh(); r.state.badAck = true;
  await r.decisions()[0].click(true);
  assert.equal(r.panel.dataset.state, 'uncertain');
  assert.equal(r.messages.length, 0); assert.equal(r.decides(), 1);
  await r.review.refresh();
  assert.equal(r.panel.dataset.state, 'approved');
  assert.equal(r.messages.length, 0); assert.equal(r.decides(), 1);
});

test('pending first read says checking; stale detached approval buttons cannot write after a read failure', async () => {
  const r = rig(); let finish;
  const review = makePlanReview({ app: { callServerTool: () => new Promise(resolve => { finish = resolve; }) },
    scope: 'project:checking', panel: r.panel, banner: r.banner, document: r.doc });
  const pending = review.refresh();
  assert.equal(r.banner.dataset.state, 'checking');
  assert.equal(r.decisions().length, 0);
  finish({ structuredContent: { supported: false } }); await pending;
  assert.equal(r.banner.hidden, true);
  await r.review.refresh(); const old = r.decisions()[0];
  r.state.error = true; await r.review.refresh(); await old.click(true);
  assert.equal(r.decides(), 0);
});

test('polling retries a handled read error after two seconds rather than waiting thirty seconds', async () => {
  const r = rig(); let time = 0;
  const poll = makePlanPolling(r.review, { now: () => time });
  r.state.error = true; await poll.tick();
  assert.equal(r.calls.length, 1);
  time = 1999; await poll.tick(); assert.equal(r.calls.length, 1);
  r.state.error = false; time = 2000; await poll.tick();
  assert.equal(r.calls.length, 2); assert.equal(r.panel.dataset.state, 'requested');
  time = 31999; await poll.tick(); assert.equal(r.calls.length, 2);
  time = 32000; await poll.tick(); assert.equal(r.calls.length, 3);
});

test('polling preserves hints arriving during a read and does not mark a busy read as successful', async () => {
  let time = 0, calls = 0, finish;
  const poll = makePlanPolling({ refresh() { calls++; return new Promise(resolve => { finish = resolve; }); } }, { now: () => time });
  const first = poll.tick(); poll.request(); finish({ status: 'ok' }); await first;
  const second = poll.tick(); assert.equal(calls, 2); finish({ status: 'busy' }); await second;
  time = 1999; await poll.tick(); assert.equal(calls, 2);
  time = 2000; const third = poll.tick(); assert.equal(calls, 3); finish({ status: 'ok' }); await third;
});

test('a late decide reply after stop paints nothing and sends nothing', async () => {
  const r = rig(); await r.review.refresh();
  let release;
  const original = r.state;
  const app = { async sendMessage() { throw new Error('must not run'); }, callServerTool: () => new Promise(resolve => { release = resolve; }) };
  const doc = r.doc, panel = new Element('section', doc), banner = new Element('section', doc);
  const review = makePlanReview({ app, scope: 'project:one', panel, banner, document: doc });
  const pending = review.refresh();
  release({ structuredContent: structuredClone(original.value), _meta: { 'gil/planApprovalToken': 'private-token' } });
  await pending;
  const approve = panel.querySelectorAll('button').find(b => b.dataset.decision === 'approve');
  const clicked = approve.click(true);
  review.stop();
  release({ structuredContent: { ...structuredClone(original.value), proposal: { ...original.value.proposal, receipt: { decision: 'approve', source: 'monitor_button' } } } });
  await clicked;
  assert.equal(panel.children.length, 0);
  assert.equal(panel.hidden, true);
});

test('the top banner names the waiting decision and jumps to the panel only on the person\'s click', async () => {
  const r = rig(); await r.review.refresh();
  assert.equal(r.banner.hidden, false);
  assert.equal(r.banner.dataset.state, 'requested');
  assert.match(r.banner.allText(), /승인 대기 · 계획 v1/);
  const jump = r.banner.querySelectorAll('button')[0];
  assert.equal(jump.dataset.action, 'jump');
  const before = r.panel.scrolled;
  await r.review.refresh(); await r.review.refresh();
  assert.equal(r.panel.scrolled, before, 'polling does not scroll or steal the view');
  assert.equal(r.banner.querySelectorAll('button')[0], jump, 'an unchanged state keeps the same banner node');
  jump.focus();
  await jump.click(true);
  assert.equal(r.panel.scrolled, before + 1);
  assert.equal(r.doc.activeElement.dataset.decision, 'approve', 'the jump lands on the approve button');
  // Focus held on the banner survives a state change.
  jump.focus();
  r.state.tokenMissing = true;
  await r.review.refresh();
  assert.equal(r.banner.dataset.state, 'no_token');
  assert.equal(r.doc.activeElement, r.banner.querySelectorAll('button')[0]);
  assert.ok(r.decisions().every(b => b.disabled), 'without the private capability the buttons are disabled, not hidden');
  // Focus outside the panel is never touched.
  const elsewhere = new Element('input', r.doc); elsewhere.focus();
  r.state.tokenMissing = false;
  await r.review.refresh();
  assert.equal(r.doc.activeElement, elsewhere);
});

test('a revised proposal says that the earlier approval does not carry over', async () => {
  const r = rig({ value: { supported: true, active_revision: 1, pending: [{ id: 'B', question: 'Question B', done_when: 'File B' }],
    executed_cycles: [{ cycle_ref: 'cycle:C2', revision: 1, item_id: 'A' }], reinterview_requests: [], current_item: null,
    current_cycle: { ref: 'cycle:C3', state: 'open' },
    proposal: proposal({ revision: 2, base_revision: 1, digest: 'c'.repeat(64), synthesis_ref: 'step:C3/S3' }) } });
  await r.review.refresh();
  assert.equal(r.panel.dataset.state, 'reapproval');
  assert.match(r.banner.allText(), /새 판 재승인 필요 · 계획 v2 — 이전 승인\(v1\)은 이 판에 적용되지 않습니다/);
  assert.match(r.panel.allText(), /이전 승인 판 v1/);
  const items = r.panel.querySelectorAll('li');
  assert.deepEqual(items.map(li => li.dataset.status), ['executed', 'pending']);
  assert.match(items[0].allText(), /A 실행함 기록 cycle:C2 Question A 완료 판단 File A/);
  assert.match(r.panel.allText(), /실행함 1개 · 남음 1개/);
  assert.doesNotMatch(r.panel.allText(), /묶임 0개/, 'a zero count is not written');
  assert.equal(r.decisions().length, 2);
});

test('an approved plan without an open proposal lists executed, bound and remaining items from records only', async () => {
  const r = rig({ value: { supported: true, active_revision: 2,
    pending: [{ id: 'B', question: 'Question B', done_when: 'File B' }, { id: 'C', question: 'Question C', done_when: 'File C' }],
    executed_cycles: [{ cycle_ref: 'cycle:C2', revision: 2, item_id: 'A' }, { cycle_ref: 'cycle:C4', revision: 2, item_id: 'D' }],
    reinterview_requests: [{ cycle_ref: 'cycle:C4', reason: '범위가 바뀌었다' }],
    current_item: { id: 'D', question: 'Question D', done_when: 'File D' }, current_cycle: { ref: 'cycle:C4', state: 'open' }, proposal: null } });
  await r.review.refresh();
  assert.equal(r.panel.dataset.state, 'executing');
  assert.equal(r.banner.hidden, true, 'nothing waits for the person');
  assert.match(r.panel.allText(), /지금 열린 Cycle cycle:C4에 묶인 항목: D Question D 완료 판단 File D/);
  assert.match(r.panel.allText(), /실행함 1개 · 지금 열린 Cycle에 묶임 1개 · 남음 2개/);
  const items = r.panel.querySelectorAll('li');
  assert.deepEqual(items.map(li => [li.dataset.status, li.dataset.item]), [['executed', 'A'], ['current', 'D'], ['pending', 'B'], ['pending', 'C']]);
  assert.match(items[0].allText(), /^A 실행함 기록 cycle:C2\s*$/, 'a record-only executed item has no invented text');
  assert.match(r.panel.allText(), /재인터뷰 요청 기록 \(cycle:C4\): 범위가 바뀌었다/);
  assert.doesNotMatch(r.panel.allText(), /작업 중입니다|진행 중/);
  assert.match(r.panel.allText(), /에이전트가 지금 작업 중인지는 여기서 알 수 없습니다/);
  assert.equal(r.panel.querySelectorAll('button').length, 0);
});

test('an undelivered continuation stays visible across polls and can be re-sent only by an explicit click, never re-approved', async () => {
  const r = rig(); await r.review.refresh(); r.state.refuseMessage = true;
  await r.decisions()[0].click(true);
  assert.equal(r.panel.dataset.state, 'approved');
  assert.match(r.banner.allText(), /채팅에 전달하지 못했습니다/);
  assert.match(r.panel.allText(), /GIL 승인 결과를 확인하고 계속해 줘/);
  await r.review.refresh(); await r.review.refresh();
  assert.match(r.panel.allText(), /채팅에 전달하지 못했습니다/, 'the guidance is not wiped by the next poll');
  assert.equal(r.messages.length, 0);
  const retry = r.actions('notify')[0];
  await retry.click(false); assert.equal(r.messages.length, 0);
  r.state.refuseMessage = false;
  await retry.click(true);
  assert.equal(r.messages.length, 1);
  // The continuation addresses the Agent, so it is English whatever language the Monitor shows.
  assert.match(r.messages[0].content[0].text, /recorded an approval for plan v1 \(step:C1\/S3\)/);
  assert.equal(r.decides(), 1, 'retrying the message never repeats the write');
  assert.equal(r.actions('notify').length, 0);
  assert.match(r.panel.allText(), /채팅에 계속 요청을 전달했습니다/);
  assert.doesNotMatch(r.banner.allText(), /전달하지 못했습니다/);
});

// What the Agent is told after a decision (Cycle Model §18–19). The text travels the same path a
// person's click takes: trusted button → gil_plan_decide → saved receipt → sendMessage. These
// cases check the message this App sends; they do not show what a model does with it.
const sent = (r, n = 0) => r.messages[n].content[0].text;
const closingValue = (verdict, extra = {}) => {
  const value = { supported: true, active_revision: 1, pending: [], executed_cycles: [{ cycle_ref: 'cycle:C2', revision: 1, item_id: 'A' }],
    reinterview_requests: [], current_item: null, current_cycle: { ref: 'cycle:C3', state: 'open' },
    proposal: proposal({ revision: 2, base_revision: 1, digest: 'd'.repeat(64), synthesis_ref: 'step:C3/S3', ...extra }) };
  if (verdict !== undefined) value.proposal.draft.synthesis.chain_verdict = verdict;
  return value;
};
const READS_RECORD_FIRST = /First read the actual record with gil_plan_read: .* If the record differs from this message, follow the record\./;

test('an approved execution plan is continued toward the Experiment, after reading the record', async () => {
  const r = rig(); await r.review.refresh();
  await r.decisions()[0].click(true);
  assert.equal(r.messages.length, 1);
  const text = sent(r);
  assert.match(text, /recorded an approval for plan v1 \(step:C1\/S3\)\./);
  assert.match(text, READS_RECORD_FIRST);
  assert.ok(text.indexOf('gil_plan_read') < text.indexOf('close the Synthesis'), 'the record is read before anything is closed');
  assert.match(text, /close the Synthesis with its close_report unchanged, then the Outcome and the Interview Cycle Report, and follow each receipt's next call to the Experiment/);
  assert.doesNotMatch(text, /Chain Report|close_chain|chain_verdict|change request/);
});

test('an approved closing proposal is continued to the Chain Report with the approved verdict, never to an Experiment', async () => {
  for (const verdict of ['success', 'failure']) {
    const r = rig({ value: closingValue(verdict) }); await r.review.refresh();
    assert.match(r.panel.allText(), verdict === 'success' ? /성공으로 마치자는 제안/ : /실패로 마치자는 제안/);
    await r.decisions()[0].click(true);
    assert.equal(r.decides(), 1);
    assert.deepEqual(r.calls.at(-1).arguments, { scope_id: 'project:one', revision: 2, digest: 'd'.repeat(64), token: 'private-token', decision: 'approve' });
    assert.equal(r.messages.length, 1);
    const text = sent(r);
    assert.match(text, new RegExp(`recorded an approval for plan v2 \\(step:C3/S3\\): a proposal to close the Chain with chain_verdict "${verdict}"\\.`));
    assert.match(text, READS_RECORD_FIRST);
    assert.ok(text.indexOf('gil_plan_read') < text.indexOf('follow the closing procedure'), 'the record is read before the closing procedure');
    assert.match(text, /next_direction\.action: close_chain/);
    assert.match(text, new RegExp(`submit the Chain Report whose verdict is the approved chain_verdict \\(${verdict}\\), with closing_synthesis_ref step:C3/S3 and a handoff_summary`));
    // The Interview's success is not the Chain's: said outright, for a failure approval as much as a success one.
    assert.match(text, /The Closing Interview's own success only means this approval was obtained; it is not the Chain verdict\./);
    assert.match(text, /Do not open an Experiment or a new Chain\. After the Chain Report, tell the user the verdict, the deliverables and the remaining questions, and stop\./);
    assert.doesNotMatch(text, /proceed to the Experiment|next call to the Experiment|change request/);
    assert.ok(!text.includes('private-token'));
    // The other verdict is never named: the message carries the approved one only.
    assert.ok(!text.includes(verdict === 'success' ? '"failure"' : '"success"'), text);
  }
});

test('a change request is never worded as approval and closes nothing, for a plan and for a closing proposal alike', async () => {
  for (const value of [undefined, closingValue('success'), closingValue('failure')]) {
    const r = rig({ value }); await r.review.refresh();
    await r.decisions()[1].click(true);
    assert.equal(r.calls.at(-1).arguments.decision, 'request_changes');
    assert.equal(r.panel.dataset.state, 'changes_requested');
    assert.equal(r.messages.length, 1);
    const text = sent(r);
    assert.match(text, /recorded a change request, not an approval, for plan v\d \(step:C\d\/S3\)\./);
    assert.match(text, READS_RECORD_FIRST);
    assert.match(text, /close the Synthesis with its close_report \(approved: no\) and ask the user what to change in a new Question/);
    assert.match(text, /Do not treat this as approval: open no Experiment, submit no Chain Report and reuse no earlier approval\./);
    assert.doesNotMatch(text, /recorded an approval|closing procedure|close_chain|close_report unchanged|"success"|"failure"/);
  }
});

test('a proposal this screen cannot classify sends the Agent to the record instead of guessing', async () => {
  // An unknown chain_verdict value is quoted on screen and never read as success, failure or a plan.
  const r = rig({ value: closingValue('maybe <b>x</b>') }); await r.review.refresh();
  assert.match(r.panel.allText(), /성공이나 실패로 해석하지 않습니다/);
  await r.decisions()[0].click(true);
  const text = sent(r);
  assert.match(text, /recorded a decision for plan v2 \(step:C3\/S3\), and this screen could not tell what follows it\./);
  assert.match(text, READS_RECORD_FIRST);
  assert.match(text, /Do not guess the next action or assume an approval: do what gil_plan_read's next says\./);
  assert.doesNotMatch(text, /maybe|Experiment|Chain Report|close_chain|recorded an approval/, 'the unknown value is not carried into the chat');
  // The classification itself: only the two recorded decisions and the two verdicts are named.
  const p = proposal();
  assert.deepEqual(decisionKind(p, 'approve'), { kind: 'plan' });
  assert.deepEqual(decisionKind(p, 'request_changes'), { kind: 'changes' });
  assert.deepEqual(decisionKind(closingValue('failure').proposal, 'approve'), { kind: 'closing', verdict: 'failure' });
  assert.deepEqual(decisionKind(closingValue('failure').proposal, 'request_changes'), { kind: 'changes' });
  for (const [broken, decision] of [[{ ...p, draft: null }, 'approve'], [{ ...p, draft: { synthesis: 'text' } }, 'approve'], [null, 'approve'],
    [p, 'approved'], [p, undefined], [closingValue('Success').proposal, 'approve'], [closingValue(true).proposal, 'approve']]) {
    assert.deepEqual(decisionKind(broken, decision), { kind: 'unknown' });
    assert.match(continuation(broken ?? {}, decision), /Do not guess the next action or assume an approval/);
  }
  for (const empty of [undefined, null, '']) assert.deepEqual(decisionKind(closingValue(empty).proposal, 'approve'), { kind: 'plan' });
});

test('an undelivered closing approval is re-sent with the same meaning, only by an explicit click, and never re-approved', async () => {
  const r = rig({ value: closingValue('failure') }); await r.review.refresh(); r.state.refuseMessage = true;
  await r.decisions()[0].click(true);
  assert.equal(r.panel.dataset.state, 'approved');
  assert.match(r.banner.allText(), /채팅에 전달하지 못했습니다/);
  assert.equal(r.messages.length, 0);
  await r.review.refresh(); await r.review.refresh();
  assert.equal(r.messages.length, 0, 'polling never sends the continuation');
  const retry = r.actions('notify')[0];
  await retry.click(false); assert.equal(r.messages.length, 0, 'a synthetic click sends nothing');
  r.state.refuseMessage = false;
  await retry.click(true);
  assert.equal(r.messages.length, 1);
  assert.equal(sent(r), continuation(closingValue('failure').proposal, 'approve'), 'the re-send is the closing continuation, word for word');
  assert.match(sent(r), /chain_verdict "failure"/);
  assert.equal(r.decides(), 1, 're-sending the message never repeats the approval');
  assert.equal(r.actions('notify').length, 0);
});

test('after a restart the saved receipt alone decides what a re-sent continuation says', async () => {
  // A fresh screen (new App instance) reads a proposal whose decision is already saved: it has
  // no approval capability, sends nothing by itself, and an explicit re-send says what the
  // first click would have said for that exact proposal and decision.
  for (const [verdict, decision, expected] of [['failure', 'approve', /close the Chain with chain_verdict "failure"/], ['success', 'approve', /close the Chain with chain_verdict "success"/],
    ['success', 'request_changes', /a change request, not an approval/], [undefined, 'approve', /next call to the Experiment/]]) {
    const value = closingValue(verdict);
    value.proposal.receipt = { decision, source: 'monitor_button' };
    const r = rig({ value }); await r.review.refresh();
    assert.equal(r.panel.dataset.state, decision === 'approve' ? 'approved' : 'changes_requested');
    assert.equal(r.decisions().length, 0, 'a decided proposal offers no decision button');
    assert.equal(r.messages.length, 0, 'reading a saved receipt is not a click');
    assert.match(r.panel.allText(), /채팅 전달 여부를 확인하지 못했습니다/);
    await r.review.refresh();
    assert.equal(r.messages.length, 0);
    await r.actions('notify')[0].click(true);
    assert.equal(r.messages.length, 1);
    assert.equal(sent(r), continuation(value.proposal, decision));
    assert.match(sent(r), expected);
    assert.equal(r.decides(), 0, 'the restart path never writes a decision');
    if (decision === 'request_changes') assert.doesNotMatch(sent(r), /recorded an approval|closing procedure/);
  }
});

test('closing proposals say which Chain verdict the approval agrees to, keeping the full text', async () => {
  const value = { supported: true, active_revision: 1, pending: [], executed_cycles: [{ cycle_ref: 'cycle:C2', revision: 1, item_id: 'A' }],
    reinterview_requests: [], current_item: null, proposal: proposal({ revision: 2, base_revision: 1, digest: 'd'.repeat(64) }) };
  value.proposal.draft.synthesis.chain_verdict = 'failure';
  const r = rig({ value }); await r.review.refresh();
  assert.match(r.panel.allText(), /전체 결과\(Chain\)를 실패로 마치자는 제안입니다/);
  assert.match(r.panel.allText(), /"chain_verdict": "failure"/);
  assert.match(r.panel.allText(), /SHA-256: d{64}/);
  assert.equal(verdictSentence({ chain_verdict: 'success' }, 1), '이 제안은 전체 결과(Chain)를 성공으로 마치자는 제안입니다. 승인하면 성공으로 끝내는 데 동의합니다.');
  assert.match(verdictSentence({}, 3), /항목 3개를 담은 계획입니다/);
  assert.match(verdictSentence({ chain_verdict: 'maybe <b>x</b>' }, 1), /"maybe <b>x<\/b>" 입니다\. 성공이나 실패로 해석하지 않습니다/);
});

test('pure state table covers every wire shape and never invents activity', () => {
  const p = proposal();
  assert.equal(planSummary(null).state, 'unreadable');
  assert.equal(planSummary({ supported: false }).state, 'unsupported');
  assert.equal(planSummary({ supported: true, proposal: null, active_revision: null }).state, 'idle');
  assert.deepEqual(planSummary({ supported: true, proposal: null, active_revision: 3 }), { state: 'executing', revision: 3 });
  assert.equal(planSummary({ supported: true, proposal: p }, { hasToken: true }).state, 'requested');
  assert.equal(planSummary({ supported: true, proposal: p }, { hasToken: false }).state, 'no_token');
  assert.equal(planSummary({ supported: true, proposal: { ...p, base_revision: 1 } }, { hasToken: true }).state, 'reapproval');
  assert.equal(planSummary({ supported: true, active_revision: 1, proposal: p }, { hasToken: true }).state, 'reapproval');
  assert.equal(planSummary({ supported: true, proposal: { ...p, receipt: { decision: 'approve', source: 'monitor_button' } } }).state, 'approved');
  assert.equal(planSummary({ supported: true, proposal: { ...p, receipt: { decision: 'request_changes', source: 'monitor_button' } } }).state, 'changes_requested');
  assert.equal(planSummary({ supported: true, proposal: p }, { hasToken: true, uncertain: { revision: 1, decision: 'approve' } }).state, 'uncertain');
  for (const state of Object.keys(PLAN_STATE)) {
    const line = bannerLine({ state, revision: 1, base_revision: null });
    assert.doesNotMatch(line, /작업 중|진행 중/, state);
    assert.equal(line !== '', !['idle', 'executing', 'unsupported'].includes(state), state);
  }
  assert.match(bannerLine({ state: 'approved', revision: 1 }, 'undelivered'), /전달하지 못했습니다/);
  assert.deepEqual(planItems({ proposal: p, executed_cycles: [{ item_id: 'B', cycle_ref: 'cycle:C2', revision: 1 }], current_item: null }).map(i => i.status), ['pending', 'executed']);
  const bound = { proposal: null, executed_cycles: [{ item_id: 'A', cycle_ref: 'cycle:C2', revision: 1 }], current_item: { id: 'A', question: 'q', done_when: 'd' }, pending: [{ id: 'A' }, { id: 'B' }] };
  assert.deepEqual(planItems({ ...bound, current_cycle: { ref: 'cycle:C2', state: 'open' } }).map(i => `${i.id}:${i.status}`), ['A:current', 'B:pending']);
  assert.deepEqual(planItems({ ...bound, current_cycle: { ref: 'cycle:C2', state: 'closed' } }).map(i => `${i.id}:${i.status}`), ['A:executed', 'B:pending']);
  assert.deepEqual(planItems(bound).map(i => `${i.id}:${i.status}`), ['A:bound', 'B:pending'], 'no field: never assume open');
  const retried = {...bound, retry_pending:['A'], current_cycle:{ref:'cycle:C3',state:'closed'},
    executed_cycles:[...bound.executed_cycles,{item_id:'A',cycle_ref:'cycle:C3',revision:1}]};
  assert.deepEqual(planItems(retried).map(i => `${i.id}:${i.status}`), ['A:retry','B:pending']);
  assert.equal(planItems(retried)[0].cycle_ref,'cycle:C3','latest attempt, not the original failed Cycle');
  assert.deepEqual(planItems({ ...bound, current_cycle: { ref: 'cycle:C2', state: 'running' } }).map(i => `${i.id}:${i.status}`), ['A:bound', 'B:pending'], 'unknown state: never assume open');
  assert.deepEqual(currentCycle({ current_cycle: { ref: 'cycle:C2', state: 'closed' } }), { ref: 'cycle:C2', state: 'closed' });
  assert.deepEqual(currentCycle({}), { ref: null, state: 'unknown' });
  assert.deepEqual(currentCycle({ current_cycle: { ref: 7, state: 'open' } }), { ref: null, state: 'open' });
});

// Wave 1 review R3: Core keeps current_item bound after the Experiment closes (regressions.rs,
// review_closed_cycle_still_has_current_item_binding). The screen must not call that an open Cycle.
const R3 = { supported: true, proposal: null, active_revision: 1, reinterview_requests: [],
  current_item: { id: 'A', question: 'Question A', done_when: 'File A' },
  executed_cycles: [{ cycle_ref: 'cycle:C2', item_id: 'A', revision: 1 }],
  pending: [{ id: 'B', question: 'Question B', done_when: 'File B' }] };

test('a closed current Cycle is reported closed: its bound item is executed, not "open now", and not a success', async () => {
  const r = rig({ value: { ...R3, current_cycle: { ref: 'cycle:C2', state: 'closed' } } }); await r.review.refresh();
  assert.equal(r.panel.dataset.state, 'executing');
  const items = r.panel.querySelectorAll('li');
  assert.deepEqual(items.map(li => li.dataset.status), ['executed', 'pending']);
  assert.match(items[0].allText(), /A 실행함 기록 cycle:C2/);
  assert.match(r.panel.allText(), /현재 Cycle cycle:C2은 닫혔습니다\. 지금 열린 Cycle은 없습니다\. 마지막으로 연결된 항목: A Question A 완료 판단 File A 결과는 그 Cycle Report가 진실원이며 여기서 성공으로 보지 않습니다/);
  assert.match(r.panel.allText(), /실행함 1개 · 남음 1개/);
  assert.doesNotMatch(r.panel.allText(), /지금 열린 Cycle(에| cycle:)/, 'no sentence claims an open Cycle');
  assert.doesNotMatch(r.panel.allText(), /성공(함|했|으로 마쳤)|완료했/, 'executed is not success');
  assert.doesNotMatch(r.panel.allText(), /작업 중입니다|진행 중/);
  assert.equal(r.panel.querySelectorAll('button').length, 0);
});

test('without current_cycle (older Core or unknown state) the screen marks the binding unverified instead of open', async () => {
  for (const value of [R3, { ...R3, current_cycle: { ref: 'cycle:C2', state: 'running' } }, { ...R3, current_cycle: null }]) {
    const r = rig({ value }); await r.review.refresh();
    const items = r.panel.querySelectorAll('li');
    assert.deepEqual(items.map(li => li.dataset.status), ['bound', 'pending']);
    assert.match(items[0].allText(), /A 현재 Cycle에 연결됨 \(열림 여부 미확인\) 기록 cycle:C2/);
    assert.match(r.panel.allText(), /현재 Cycle에 연결된 항목: A Question A 완료 판단 File A/);
    assert.match(r.panel.allText(), /Cycle 열림 여부가 없어 열려 있다고 보지 않습니다/);
    assert.match(r.panel.allText(), /연결됨\(열림 미확인\) 1개 · 남음 1개/);
    assert.doesNotMatch(r.panel.allText(), /지금 열린 Cycle/);
  }
});

test('the next Experiment opens: the new binding is the open Cycle and the earlier item stays executed', async () => {
  const value = { ...R3, current_item: { id: 'B', question: 'Question B', done_when: 'File B' }, pending: [],
    executed_cycles: [...R3.executed_cycles, { cycle_ref: 'cycle:C3', item_id: 'B', revision: 1 }], current_cycle: { ref: 'cycle:C3', state: 'open' } };
  const r = rig({ value }); await r.review.refresh();
  const items = r.panel.querySelectorAll('li');
  assert.deepEqual(items.map(li => [li.dataset.status, li.dataset.item]), [['executed', 'A'], ['current', 'B']]);
  assert.match(r.panel.allText(), /지금 열린 Cycle cycle:C3에 묶인 항목: B Question B/);
  assert.match(r.panel.allText(), /실행함 1개 · 지금 열린 Cycle에 묶임 1개 · 남음 0개/);
  // The same screen after the Cycle closes, before any proposal: polling moves it to closed without a rebuild of meaning.
  r.state.value = { ...value, current_cycle: { ref: 'cycle:C3', state: 'closed' } };
  await r.review.refresh();
  assert.deepEqual(r.panel.querySelectorAll('li').map(li => li.dataset.status), ['executed', 'executed']);
  assert.match(r.panel.allText(), /현재 Cycle cycle:C3은 닫혔습니다/);
  assert.equal(r.panel.querySelectorAll('button').length, 0);
});

test('a re-interview proposal after a closed Experiment lists that item as executed, never as the open Cycle', async () => {
  const value = { ...R3, current_cycle: { ref: 'cycle:C2', state: 'closed' }, reinterview_requests: [{ cycle_ref: 'cycle:C2', reason: '범위가 바뀌었다' }],
    proposal: proposal({ revision: 2, base_revision: 1, digest: 'e'.repeat(64), synthesis_ref: 'step:C3/S3' }) };
  const r = rig({ value }); await r.review.refresh();
  assert.equal(r.panel.dataset.state, 'reapproval');
  assert.deepEqual(r.panel.querySelectorAll('li').map(li => li.dataset.status), ['executed', 'pending']);
  assert.match(r.panel.allText(), /실행함 1개 · 남음 1개/);
  assert.match(r.panel.allText(), /재인터뷰 요청 기록 \(cycle:C2\)/);
  assert.equal(r.decisions().length, 2, 'the decision buttons are unchanged');
  // The proposal view with no field still never assumes open.
  r.state.value = { ...value, current_cycle: undefined };
  await r.review.refresh();
  assert.deepEqual(r.panel.querySelectorAll('li').map(li => li.dataset.status), ['bound', 'pending']);
});

test('no proposal and nothing executed: idle stays idle whatever the Cycle state says', async () => {
  for (const current_cycle of [{ ref: 'cycle:C1', state: 'open' }, { ref: 'cycle:C1', state: 'closed' }, undefined]) {
    const r = rig({ value: { supported: true, active_revision: null, pending: [], executed_cycles: [], reinterview_requests: [], current_item: null, current_cycle, proposal: null } });
    await r.review.refresh();
    assert.equal(r.panel.dataset.state, 'idle');
    assert.equal(r.banner.hidden, true);
    assert.doesNotMatch(r.panel.allText(), /지금 열린 Cycle/);
  }
});

// Reading a long proposal (Cycle Model §16, Monitor Model §1.1): each item is read as its own
// problem and done-when condition, nothing is shortened, and a fresh proposal carries no tally.
const byClass = (node, name) => node.children.find(n => n.className === name);
const LONG = Array.from({ length: 4 }, (_, i) => `문단 ${i + 1}: 월별 집계에 쓸 날짜·금액 열의 형식을 확인하고 누락·중복 행을 센다.`).join('\n');

test('a long proposal keeps every word apart: label, statement, scope, each item problem and done-when; no tally without records', async () => {
  const items = Array.from({ length: 10 }, (_, i) => ({ id: `I${i + 1}`, question: `${LONG}\n질문 ${i + 1}`, done_when: `${LONG}\n완료 ${i + 1}` }));
  const value = { supported: true, active_revision: null, pending: [], executed_cycles: [], reinterview_requests: [], current_item: null,
    current_cycle: { ref: 'cycle:C1', state: 'open' }, proposal: proposal({ draft: { scope: proposal().draft.scope, items, synthesis: { statement: `${LONG}\n명제`, basis_refs: 'step:C1/S1' } } }) };
  const r = rig({ value }); await r.review.refresh();
  assert.equal(r.panel.dataset.state, 'requested');
  const statement = byClass(r.panel, 'plan-statement');
  assert.equal(statement.children[0].textContent, '확인할 내용');
  assert.equal(statement.children[1].textContent, `${LONG}\n명제`, 'the statement is the agent\'s full sentence');
  assert.equal(byClass(r.panel, 'plan-counts'), undefined, 'no executed/bound/retry record: the verdict sentence already counts the items');
  assert.match(r.panel.allText(), /실행할 항목 10개를 담은 계획입니다/);
  const lis = r.panel.querySelectorAll('li');
  assert.equal(lis.length, 10);
  for (const [i, li] of lis.entries()) {
    assert.equal(li.dataset.item, `I${i + 1}`); assert.equal(li.dataset.status, 'pending');
    const head = byClass(li, 'plan-item-head');
    assert.deepEqual(head.children.map(n => n.textContent), [`I${i + 1}`, '남음'], 'id and recorded status, no Cycle record for a pending item');
    assert.equal(byClass(li, 'plan-item-question').textContent, items[i].question, 'the problem text is complete');
    const done = byClass(li, 'plan-item-done');
    assert.deepEqual(done.children.map(n => n.textContent), ['완료 판단', items[i].done_when], 'the done-when text is complete under its label');
  }
  // The full original text and the exact revision/digest stay reachable below the controls.
  assert.match(r.panel.allText(), new RegExp(`"statement": ${JSON.stringify(`${LONG}\n명제`).replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}`));
  assert.match(r.panel.allText(), /SHA-256: a{64}/);
  assert.equal(r.decisions().length, 2);
  for (const status of ['executed', 'current', 'bound', 'pending', 'retry']) {
    assert.doesNotMatch(PLAN_ITEM_WORD(status), /완료|성공|실패함/, `${status} is a record word, not a verdict`);
  }
});
import { t as word } from '../ui/locale.js';
const PLAN_ITEM_WORD = (status) => word(`plan.item.${status}`);

test('many items with mixed records: every card carries its own status and Cycle record; a retry shows its latest attempt', async () => {
  const items = ['A', 'B', 'C', 'D', 'E', 'F'].map(id => ({ id, question: `Question ${id}`, done_when: `File ${id}` }));
  const value = { supported: true, active_revision: 1, reinterview_requests: [], current_item: null, current_cycle: { ref: 'cycle:C5', state: 'open' },
    pending: items.filter(i => i.id !== 'A'), retry_pending: ['B'],
    executed_cycles: [{ cycle_ref: 'cycle:C2', item_id: 'A', revision: 1 }, { cycle_ref: 'cycle:C3', item_id: 'B', revision: 1 }, { cycle_ref: 'cycle:C4', item_id: 'B', revision: 1 }],
    proposal: proposal({ revision: 2, base_revision: 1, digest: 'f'.repeat(64), synthesis_ref: 'step:C5/S3', draft: { scope: proposal().draft.scope, items, synthesis: { statement: 'B 를 다시 정의한다', basis_refs: 'step:C5/S1' } } }) };
  const r = rig({ value }); await r.review.refresh();
  const lis = r.panel.querySelectorAll('li');
  assert.deepEqual(lis.map(li => [li.dataset.status, li.dataset.item]), [['executed', 'A'], ['retry', 'B'], ['pending', 'C'], ['pending', 'D'], ['pending', 'E'], ['pending', 'F']]);
  assert.deepEqual(byClass(lis[0], 'plan-item-head').children.map(n => n.textContent), ['A', '실행함', '기록 cycle:C2']);
  assert.deepEqual(byClass(lis[1], 'plan-item-head').children.map(n => n.textContent), ['B', '재시도 대기 (실패 기록 유지)', '기록 cycle:C4'], 'latest attempt, failure record kept');
  assert.deepEqual(byClass(lis[2], 'plan-item-head').children.map(n => n.textContent), ['C', '남음']);
  assert.equal(byClass(r.panel, 'plan-counts').textContent, '실행함 1개 · 남음 4개 · 재시도 대기 1개');
  assert.doesNotMatch(r.panel.allText(), /묶임 0개/);
  assert.equal(r.decisions().length, 2);
});

test('an empty item list renders no cards and no tally; the closing verdict still says what approval means', async () => {
  const value = { supported: true, active_revision: 1, pending: [], executed_cycles: [{ cycle_ref: 'cycle:C2', item_id: 'A', revision: 1 }], reinterview_requests: [], current_item: null,
    current_cycle: { ref: 'cycle:C3', state: 'open' }, proposal: proposal({ revision: 2, base_revision: 1, digest: 'd'.repeat(64), draft: { scope: proposal().draft.scope, items: [], synthesis: { statement: '끝내자', basis_refs: 'step:C3/S1', chain_verdict: 'success' } } }) };
  const r = rig({ value }); await r.review.refresh();
  assert.equal(r.panel.querySelectorAll('li').length, 0);
  assert.equal(r.panel.querySelectorAll('ol').length, 1, 'the list element stays, empty');
  assert.match(r.panel.allText(), /전체 결과\(Chain\)를 성공으로 마치자는 제안입니다/);
  assert.equal(byClass(r.panel, 'plan-counts'), undefined, 'a proposal that lists no item has nothing to tally');
  assert.equal(r.decisions().length, 2);
});

test('a decision made from the keyboard leaves focus on the panel heading, never on the document', async () => {
  const r = rig(); await r.review.refresh();
  const approve = r.decisions()[0]; approve.focus();
  await approve.click(true);
  assert.equal(r.panel.dataset.state, 'approved');
  assert.equal(r.doc.activeElement?.tag, 'h2', 'the pressed button is gone; the heading takes focus');
  assert.equal(r.panel.scrolled, 0);
  // The same when the reply is lost: the uncertain view keeps the keyboard inside the panel.
  const u = rig(); await u.review.refresh(); u.state.loseReply = true;
  const b = u.decisions()[0]; b.focus(); await b.click(true);
  assert.equal(u.panel.dataset.state, 'uncertain');
  assert.equal(u.doc.activeElement?.tag, 'h2');
});

// Exploration contract (Approved Plan "후속 구현 이행"): the Core adds `contract`, `requirements`
// and `current_experiment` beside the fields this panel already reads. The panel is unchanged,
// so this pins what the existing statuses mean for that wire shape: "executed" is the recorded
// fact that an Experiment was linked to the requirement, never that the requirement is done,
// and a failed Experiment produces no "retry" row because nothing is re-bound.
test('an exploration-contract view lists requirements by recorded links only and never as retries or completion', () => {
  const requirement = (id) => ({ id, question: `Q ${id}`, done_when: `criteria ${id}` });
  const base = {
    supported: true, contract: 'exploration', proposal: null, active_revision: 1,
    requirement_resolution: 'user_approved_closing_interview_only',
    executed_cycles: [
      { cycle_ref: 'cycle:C2', revision: 1, item_id: 'R1', selection: { relevance: 'a', readiness: 'b', uncertainty: 'c' } },
      { cycle_ref: 'cycle:C3', revision: 1, item_id: 'R1', selection: { relevance: 'a', readiness: 'b', uncertainty: 'c' } },
    ],
    retry_pending: [], pending: [requirement('R2')], current_item: requirement('R1'),
  };
  // Two Experiments linked to R1 (the second failed and is closed), none to R2.
  const closed = planItems({ ...base, current_cycle: { ref: 'cycle:C3', state: 'closed' } });
  assert.deepEqual(closed.map(i => `${i.id}:${i.status}:${i.cycle_ref}`), ['R1:executed:cycle:C3', 'R2:pending:null']);
  assert.equal(closed.filter(i => i.status === 'retry').length, 0, 'a failure is followed by a different Define, not a retry row');
  // A third Experiment on the same requirement is the open Cycle; R1 is one row, not three.
  const open = planItems({ ...base, executed_cycles: [...base.executed_cycles, { cycle_ref: 'cycle:C4', revision: 1, item_id: 'R1' }], current_cycle: { ref: 'cycle:C4', state: 'open' } });
  assert.deepEqual(open.map(i => `${i.id}:${i.status}:${i.cycle_ref}`), ['R1:current:cycle:C4', 'R2:pending:null']);
  assert.deepEqual(planSummary({ ...base, current_cycle: { ref: 'cycle:C4', state: 'open' } }), { state: 'executing', revision: 1 });
});

// Approved requirement text in an exploration Chain. The responses are what the real
// exploration-flow Core returned from gil_plan_review at each position of one Chain
// (tests/exploration_plan_panel.rs writes and re-checks the file); nothing in them is edited
// here. They are synthetic harness input: no person approved anything, and a rendered browser
// screen or a reader's understanding is not what these cases show.
import { readFileSync } from 'node:fs';
const CORE_TEXT = readFileSync(new URL('./fixtures/exploration-plan-review.json', import.meta.url), 'utf8');
const CORE = JSON.parse(CORE_TEXT);
const core = (name) => structuredClone(CORE.checkpoints.find(c => c.name === name).review);
// What the panel must show for one id: the proposal under review when there is one, else the approved requirements.
const serverText = (value) => new Map((value.proposal ? value.proposal.draft.items : value.requirements).map(i => [i.id, i]));
const rowText = (li) => ({ question: byClass(li, 'plan-item-question')?.textContent ?? null,
  done: byClass(li, 'plan-item-done')?.children.map(n => n.textContent) ?? null });
const rows = (panel) => panel.querySelectorAll('li').map(li => `${li.dataset.item}:${li.dataset.status}:${byClass(li, 'plan-item-head').children.find(n => n.className === 'plan-item-cycle')?.textContent ?? ''}`);
const CORE_ROWS = {
  'approved-nothing-linked': ['R1:pending:', 'R2:pending:'],
  'r1-open': ['R1:current:기록 cycle:C2', 'R2:pending:'],
  'r1-closed': ['R1:executed:기록 cycle:C2', 'R2:pending:'],
  'r1-failed': ['R1:executed:기록 cycle:C3', 'R2:pending:'],
  'moved-to-r2-open': ['R1:executed:기록 cycle:C3', 'R2:current:기록 cycle:C4'],
  'r2-closed': ['R1:executed:기록 cycle:C3', 'R2:executed:기록 cycle:C4'],
  'interview-open': ['R1:executed:기록 cycle:C3', 'R2:executed:기록 cycle:C4'],
  'proposal-waiting': ['R1:executed:기록 cycle:C3', 'R2:executed:기록 cycle:C4', 'R3:pending:'],
  'proposal-approved': ['R1:executed:기록 cycle:C3', 'R2:executed:기록 cycle:C4', 'R3:pending:'],
  'reapproved-r3-open': ['R1:executed:기록 cycle:C3', 'R2:executed:기록 cycle:C4', 'R3:current:기록 cycle:C6'],
};

test('real Core responses: the kept file is the App-path view only, with no capability and no local path', () => {
  assert.deepEqual(CORE.checkpoints.map(c => c.name), Object.keys(CORE_ROWS));
  assert.doesNotMatch(CORE_TEXT, /gil\/planApprovalToken|approval_token|\/Users\/|\/private\/|[A-Z]:\\\\Users\\\\/);
  for (const { name, review } of CORE.checkpoints) {
    assert.equal(review.supported, true, name);
    assert.equal(review.contract, 'exploration', name);
    assert.deepEqual(review.retry_pending, [], name);
  }
});

test('real Core, current Experiment moved from R1 to R2: both requirements keep their question and judgment criteria', async () => {
  const value = core('moved-to-r2-open');
  // The position as the server reports it: R1 is no longer current_item; its text is in requirements only.
  assert.equal(value.proposal, null);
  assert.equal(value.current_item.id, 'R2');
  assert.deepEqual(value.executed_cycles.map(e => `${e.cycle_ref}>${e.item_id}`), ['cycle:C2>R1', 'cycle:C3>R1', 'cycle:C4>R2']);
  const r = rig({ value }); await r.review.refresh();
  assert.equal(r.panel.dataset.state, 'executing');
  const lis = r.panel.querySelectorAll('li');
  assert.deepEqual(rows(r.panel), CORE_ROWS['moved-to-r2-open'], 'one row per requirement, the earlier one first, each with its latest linked Cycle');
  const [r1, r2] = value.requirements;
  assert.deepEqual(rowText(lis[0]), { question: r1.question, done: ['완료 판단', r1.done_when] }, 'R1 keeps the approved words, unchanged');
  assert.deepEqual(rowText(lis[1]), { question: r2.question, done: ['완료 판단', r2.done_when] });
  assert.notEqual(r1.question, r2.question);
  assert.deepEqual(byClass(lis[0], 'plan-item-head').children.map(n => n.textContent), ['R1', '실행함', '기록 cycle:C3'], 'status and Cycle record mean what they meant');
  assert.equal(byClass(r.panel, 'plan-counts').textContent, '실행함 1개 · 지금 열린 Cycle에 묶임 1개 · 남음 0개');
  // A failed and closed Experiment is the latest link of R1; that is not a reason to drop or relabel its text.
  assert.equal(value.requirements[0].experiments.at(-1).verdict, 'failure');
  assert.doesNotMatch(r.panel.allText(), /재시도|충족|완료했|성공(함|했)/, 'no retry row, no completion claim');
  assert.equal(r.panel.querySelectorAll('button').length, 0);
});

test('real Core, every position: each requirement is one row carrying the exact text the server gave for that id', async () => {
  for (const { name } of CORE.checkpoints) {
    const value = core(name), frozen = JSON.stringify(value);
    const r = rig({ value }); await r.review.refresh();
    assert.deepEqual(rows(r.panel), CORE_ROWS[name], name);
    const expected = serverText(value), lis = r.panel.querySelectorAll('li');
    assert.equal(new Set(lis.map(li => li.dataset.item)).size, lis.length, `${name}: several Experiments on one requirement are still one row`);
    assert.deepEqual(lis.map(li => li.dataset.item), [...expected.keys()], name);
    for (const li of lis) {
      const source = expected.get(li.dataset.item);
      assert.deepEqual(rowText(li), { question: source.question, done: ['완료 판단', source.done_when] }, `${name} ${li.dataset.item}`);
    }
    assert.deepEqual(planItems(value).map(i => [i.id, i.question, i.done_when]), [...expected.values()].map(i => [i.id, i.question, i.done_when]), name);
    assert.equal(JSON.stringify(value), frozen, `${name}: reading changes nothing`);
  }
});

test('real Core, the current Cycle is an Interview: no bound item is shown, and the approved text is still listed', async () => {
  const value = core('interview-open');
  assert.equal(value.current_item, null);
  assert.deepEqual(value.current_cycle, { ref: 'cycle:C5', state: 'open' });
  const r = rig({ value }); await r.review.refresh();
  assert.equal(r.panel.dataset.state, 'executing');
  const current = byClass(r.panel, 'plan-current');
  assert.equal(current.children.length, 1, 'only the sentence about the Cycle: no requirement is presented as bound to it');
  assert.equal(current.querySelectorAll('div').length, 0);
  const lis = r.panel.querySelectorAll('li');
  assert.deepEqual(lis.map(li => rowText(li).question), value.requirements.map(q => q.question));
  assert.deepEqual(lis.map(li => rowText(li).done?.[1]), value.requirements.map(q => q.done_when));
  assert.deepEqual(lis.map(li => li.dataset.status), ['executed', 'executed'], 'still the record that a Cycle was linked, nothing more');
});

test('real Core, a new proposal: the rows are the exact proposal under review, never the approved list', async () => {
  const value = core('proposal-waiting');
  assert.deepEqual(value.proposal.draft.items.map(i => i.id), ['R1', 'R2', 'R3']);
  assert.deepEqual(value.requirements.map(i => i.id), ['R1', 'R2'], 'the approved revision does not hold R3');
  const r = rig({ value }); await r.review.refresh();
  assert.equal(r.panel.dataset.state, 'reapproval');
  assert.deepEqual(r.panel.querySelectorAll('li').map(li => rowText(li).question), value.proposal.draft.items.map(i => i.question));
  assert.equal(r.decisions().length, 2);
  // The approval is sent for the displayed revision and digest of that proposal.
  await r.decisions()[0].click(true);
  assert.deepEqual(r.calls.at(-1).arguments, { scope_id: 'project:one', revision: 2, digest: value.proposal.digest, token: 'private-token', decision: 'approve' });
  assert.equal(r.panel.dataset.state, 'approved');
  assert.equal(sent(r), continuation(value.proposal, 'approve'));

  // If the approved list ever said something else for the same id, the proposal's words stay:
  // the two sources are not blended. (Core refuses such a redefinition; this response is altered.)
  const altered = core('proposal-waiting');
  for (const q of altered.requirements) { q.question = `APPROVED-ONLY ${q.id}`; q.done_when = `APPROVED-ONLY criteria ${q.id}`; }
  altered.requirements.push({ id: 'R9', question: 'APPROVED-ONLY R9', done_when: 'APPROVED-ONLY criteria R9', experiments: [] });
  const a = rig({ value: altered }); await a.review.refresh();
  assert.doesNotMatch(a.panel.allText(), /APPROVED-ONLY/);
  assert.deepEqual(a.panel.querySelectorAll('li').map(li => li.dataset.item), ['R1', 'R2', 'R3'], 'no row is added from the approved list');
  assert.deepEqual(a.panel.querySelectorAll('li').map(li => rowText(li).done[1]), value.proposal.draft.items.map(i => i.done_when));
  // And the other way round: a draft is never the text of an approved requirement once it is not under review.
  const gone = core('moved-to-r2-open');
  gone.requirements = gone.requirements.filter(q => q.id !== 'R1');
  gone.proposal = null;
  assert.deepEqual(planItems(gone)[0], { id: 'R1', question: null, done_when: null, status: 'executed', cycle_ref: 'cycle:C3' });
});

test('an incomplete exploration response gets no invented, borrowed or remembered requirement text', async () => {
  const blank = { id: 'R1', question: null, done_when: null, status: 'executed', cycle_ref: 'cycle:C3' };
  // The panel shows an item's id, question and deciding observation; the record's `source` is not shown.
  const { source: _source, ...r2 } = core('moved-to-r2-open').current_item;
  const cases = {
    'no requirements field': v => { delete v.requirements; },
    'requirements is not a list': v => { v.requirements = { R1: v.requirements[0] }; },
    'R1 is not listed': v => { v.requirements = v.requirements.filter(q => q.id !== 'R1'); },
    'R1 is listed twice with different words': v => { v.requirements.push({ ...v.requirements[0], question: 'another wording' }); },
    'the entry has no id': v => { delete v.requirements[0].id; },
    'the texts are not text': v => { v.requirements[0].question = { text: 'x' }; v.requirements[0].done_when = 7; },
    'the texts are empty': v => { v.requirements[0].question = ''; v.requirements[0].done_when = ''; },
    'no contract marker (an item-queue response is read as before)': v => { delete v.contract; },
    'an unknown contract value': v => { v.contract = 'exploration-v2'; },
  };
  for (const [name, change] of Object.entries(cases)) {
    const value = core('moved-to-r2-open'); change(value);
    const items = planItems(value);
    assert.deepEqual(items[0], blank, name);
    assert.deepEqual(items[1], { ...r2, status: 'current', cycle_ref: 'cycle:C4' }, `${name}: the current requirement is untouched`);
    const r = rig({ value }); await r.review.refresh();
    const li = r.panel.querySelectorAll('li')[0];
    assert.match(li.allText(), /^R1 실행함 기록 cycle:C3\s*$/, `${name}: id, recorded status and Cycle only`);
    assert.equal(r.panel.querySelectorAll('li').length, 2, name);
  }
  // One of the two texts is missing: the other is shown, and nothing stands in for the missing one.
  const half = core('moved-to-r2-open'); delete half.requirements[0].done_when;
  assert.deepEqual(planItems(half)[0], { ...blank, question: half.requirements[0].question });
  const h = rig({ value: half }); await h.review.refresh();
  assert.deepEqual(rowText(h.panel.querySelectorAll('li')[0]), { question: half.requirements[0].question, done: null });
  // A later read that no longer carries the list: the text shown a moment ago is not kept as approved fact.
  const full = core('moved-to-r2-open');
  const s = rig({ value: full }); await s.review.refresh();
  assert.equal(rowText(s.panel.querySelectorAll('li')[0]).question, full.requirements[0].question);
  s.state.value = core('moved-to-r2-open'); delete s.state.value.requirements;
  await s.review.refresh();
  assert.deepEqual(rowText(s.panel.querySelectorAll('li')[0]), { question: null, done: null });
  assert.doesNotMatch(s.panel.querySelectorAll('li')[0].allText(), /합성 자료|evidence/);
});

test('an item-queue response is read exactly as before, even if it carries fields this panel does not use for it', () => {
  // No marker: active_plan.items and a requirements-shaped list are not consulted (the queue's rows are unchanged).
  const queue = { supported: true, proposal: null, active_revision: 1, reinterview_requests: [], retry_pending: [],
    active_plan: { revision: 1, digest: DIGEST, items: [{ id: 'A', question: 'Question A', done_when: 'File A' }, { id: 'B', question: 'Question B', done_when: 'File B' }] },
    requirements: [{ id: 'A', question: 'Question A', done_when: 'File A', experiments: [] }],
    executed_cycles: [{ cycle_ref: 'cycle:C2', item_id: 'A', revision: 1 }, { cycle_ref: 'cycle:C3', item_id: 'B', revision: 1 }],
    current_item: { id: 'B', question: 'Question B', done_when: 'File B' }, current_cycle: { ref: 'cycle:C3', state: 'open' }, pending: [] };
  assert.deepEqual(planItems(queue), [
    { id: 'A', question: null, done_when: null, status: 'executed', cycle_ref: 'cycle:C2' },
    { id: 'B', question: 'Question B', done_when: 'File B', status: 'current', cycle_ref: 'cycle:C3' }]);
});
