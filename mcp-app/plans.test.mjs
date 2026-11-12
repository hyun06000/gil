import test from 'node:test';
import assert from 'node:assert/strict';
import { makePlanReview, planSummary, planItems, currentCycle, verdictSentence, bannerLine, PLAN_STATE } from './plans.mjs';
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
  assert.match(items[0].textContent, /실행함 \(cycle:C2\) · A/);
  assert.match(r.panel.allText(), /실행함 1개 · 지금 열린 Cycle 0개 · 남음 1개/);
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
  assert.match(r.panel.allText(), /지금 열린 Cycle cycle:C4에 묶인 항목: D — Question D/);
  assert.match(r.panel.allText(), /실행함 1개 · 지금 열린 Cycle 1개 · 남음 2개/);
  const items = r.panel.querySelectorAll('li');
  assert.deepEqual(items.map(li => [li.dataset.status, li.textContent.split(' · ')[1].split(':')[0]]), [['executed', 'A'], ['current', 'D'], ['pending', 'B'], ['pending', 'C']]);
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
  assert.match(items[0].textContent, /실행함 \(cycle:C2\) · A/);
  assert.match(r.panel.allText(), /현재 Cycle cycle:C2은 닫혔습니다\. 지금 열린 Cycle은 없습니다\. 마지막으로 연결된 항목: A — Question A/);
  assert.match(r.panel.allText(), /실행함 1개 · 지금 열린 Cycle 0개 · 남음 1개/);
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
    assert.match(items[0].textContent, /현재 Cycle에 연결됨 \(열림 여부 미확인\) · A/);
    assert.match(r.panel.allText(), /현재 Cycle에 연결된 항목: A — Question A/);
    assert.match(r.panel.allText(), /Cycle 열림 여부가 없어 열려 있다고 보지 않습니다/);
    assert.match(r.panel.allText(), /실행함 0개 · 연결됨\(열림 미확인\) 1개 · 남음 1개/);
    assert.doesNotMatch(r.panel.allText(), /지금 열린 Cycle/);
  }
});

test('the next Experiment opens: the new binding is the open Cycle and the earlier item stays executed', async () => {
  const value = { ...R3, current_item: { id: 'B', question: 'Question B', done_when: 'File B' }, pending: [],
    executed_cycles: [...R3.executed_cycles, { cycle_ref: 'cycle:C3', item_id: 'B', revision: 1 }], current_cycle: { ref: 'cycle:C3', state: 'open' } };
  const r = rig({ value }); await r.review.refresh();
  const items = r.panel.querySelectorAll('li');
  assert.deepEqual(items.map(li => [li.dataset.status, li.textContent.split(' · ')[1].split(':')[0]]), [['executed', 'A'], ['current', 'B']]);
  assert.match(r.panel.allText(), /지금 열린 Cycle cycle:C3에 묶인 항목: B — Question B/);
  assert.match(r.panel.allText(), /실행함 1개 · 지금 열린 Cycle 1개 · 남음 0개/);
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
  assert.match(r.panel.allText(), /실행함 1개 · 지금 열린 Cycle 0개 · 남음 1개/);
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
