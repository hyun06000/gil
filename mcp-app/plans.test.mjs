import test from 'node:test';
import assert from 'node:assert/strict';
import { makePlanReview } from './plans.mjs';

// Deterministic controller fixture. Synthetic trusted flags test event routing, not physical user identity.
class Element {
  constructor(tag) { this.tag = tag; this.children = []; this.listeners = {}; this.textContent = ''; }
  append(...nodes) { this.children.push(...nodes); }
  replaceChildren(...nodes) { this.children = nodes; }
  addEventListener(name, fn) { this.listeners[name] = fn; }
  querySelectorAll(tag) { return this.children.flatMap(n => [...(n.tag === tag ? [n] : []), ...n.querySelectorAll(tag)]); }
  allText() { return this.textContent + this.children.map(n => n.allText()).join(' '); }
  click(trusted) { return this.listeners.click?.({ isTrusted: trusted }); }
}
function rig() {
  const panel = new Element('section'), calls = [], messages = [];
  let change = 0;
  const value = { supported:true, active_revision:null, pending:[], proposal: { revision:1, digest:'a'.repeat(64), synthesis_ref:'step:C1/S3', receipt:null,
    draft:{ scope:{goal:'Safe <img onerror=attack>',in_scope:'in',out_of_scope:'out',constraints:'none',success_criteria:'proof'}, items:[{id:'A',question:'Question A',done_when:'File A'}], synthesis:{statement:'Proposed scope', basis_refs:'step:C1/S1\nstep:C1/S2'} } } };
  const state = { value, token:'private-token', error:false, loseReply:false };
  const app = { async sendMessage(m) { messages.push(m); return {}; }, async callServerTool(request) {
    calls.push(structuredClone(request));
    if (state.error) throw new Error('offline');
    if (request.name === 'gil_plan_decide') {
      assert.equal(request.arguments.token, state.token);
      assert.equal(request.arguments.digest, state.value.proposal.digest);
      state.value = structuredClone(state.value);
      state.value.proposal.receipt = { decision:request.arguments.decision,source:'monitor_button' };
      if (state.loseReply) throw new Error('reply lost');
    }
    return { structuredContent:structuredClone(state.value), _meta: { 'gil/planApprovalToken':state.value.proposal?.receipt ? undefined : state.token } };
  } };
  const review = makePlanReview({app,scope:'project:one',panel,document:{createElement:tag => new Element(tag)},changed:()=>change++});
  return { panel, calls, messages, state, review, changes:()=>change };
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
  const buttons = r.panel.querySelectorAll('button');
  await buttons[0].click(false); assert.equal(r.calls.length, 1);
  await buttons[0].click(true);
  assert.equal(r.calls[1].name, 'gil_plan_decide');
  assert.deepEqual(r.calls[1].arguments, {scope_id:'project:one',revision:1,digest:'a'.repeat(64),token:'private-token',decision:'approve'});
  assert.equal(r.messages.length, 1);
  assert.ok(!JSON.stringify(r.messages).includes("private-token"));
  assert.equal(r.changes(), 1); assert.equal(r.panel.querySelectorAll('button').length, 0);
  await buttons[0].click(true); assert.equal(r.calls.length, 2);
});
test('proposal changes replace buttons; polling preserves focus and cannot reuse the previous proposal', async () => {
  const r = rig(); await r.review.refresh();
  const first = r.panel.querySelectorAll('button')[0];
  await r.review.refresh(); assert.equal(r.panel.querySelectorAll('button')[0], first);
  r.state.value.proposal.revision = 2; r.state.value.proposal.digest = 'b'.repeat(64); r.state.token = 'new-token';
  await r.review.refresh();
  await first.click(true); assert.ok(r.calls.every(c => c.name === 'gil_plan_review'));
  await r.panel.querySelectorAll('button')[1].click(true);
  assert.equal(r.calls.at(-1).arguments.decision, 'request_changes');
  assert.equal(r.calls.at(-1).arguments.revision, 2);
});
test('lost reply reconciles the recorded decision instead of automatically sending another approval', async () => {
  const r = rig(); await r.review.refresh(); r.state.loseReply = true;
  await r.panel.querySelectorAll('button')[0].click(true);
  assert.equal(r.panel.querySelectorAll('button').length, 0);
  await r.review.refresh();
  assert.match(r.panel.allText(), /승인을 기록/);
  assert.equal(r.calls.filter(c => c.name === 'gil_plan_decide').length, 1);
  assert.equal(r.messages.length, 0, 'reconciling a receipt must not impersonate a fresh user click');
});
test('disconnect and teardown remove actionable buttons; legacy projects have no approval panel', async () => {
  const r = rig(); await r.review.refresh(); r.state.error = true;
  await r.review.refresh(); assert.equal(r.panel.querySelectorAll('button').length, 0);
  r.state.error = false; r.state.value = { supported:false };
  await r.review.refresh(); assert.equal(r.panel.hidden, true);
  r.review.stop(); const n = r.calls.length; await r.review.refresh(); assert.equal(r.calls.length, n);
});
