// Only this MCP App adapter writes approval metadata. The shared/native renderer stays read-only.
// A token remains in this closure, never GIL_HOST, DOM, host-evidence or model-visible content.
//
// The panel speaks only about observable facts: a recorded receipt, an open proposal, a bound
// item, a reply this screen did or did not get. It never claims that the agent is working.

import { t } from '../ui/locale.js';

const PLAN_STATES = ['checking', 'retrying', 'idle', 'requested', 'reapproval', 'no_token', 'approved',
  'changes_requested', 'executing', 'uncertain', 'unreadable', 'unsupported'];
const planState = (state) => state === 'unsupported' ? '' : t(`plan.state.${state}`);
// Read like a plain table, but in the Monitor's current language.
export const PLAN_STATE = new Proxy({}, {
  get: (_, key) => typeof key === 'string' && PLAN_STATES.includes(key) ? planState(key) : undefined,
  has: (_, key) => PLAN_STATES.includes(key),
  ownKeys: () => PLAN_STATES,
  getOwnPropertyDescriptor: (_, key) => PLAN_STATES.includes(key)
    ? { value: planState(key), enumerable: true, configurable: true, writable: false } : undefined,
});
const itemStatus = (status) => t(`plan.item.${status}`);
const decisionWord = (decision) => t(decision === 'approve' ? 'plan.decision.approve' : 'plan.decision.request_changes');

/** The real state of the current Cycle, or 'unknown' when the wire does not say. Never inferred from current_item. */
export function currentCycle(value) {
  const c = value?.current_cycle;
  const ref = typeof c?.ref === 'string' && c.ref ? c.ref : null;
  const state = c?.state === 'open' || c?.state === 'closed' ? c.state : 'unknown';
  return { ref, state };
}

/** One state for the whole screen, derived from the wire value and this screen's own facts. */
export function planSummary(value, { hasToken = false, uncertain = null } = {}) {
  if (uncertain) return { state: 'uncertain', revision: uncertain.revision, decision: uncertain.decision };
  if (['checking', 'retrying', 'unreadable'].includes(value?.code)) return { state: value.code, said: value.said, confirmed: value.confirmed };
  if (!value || value.code === 'unreadable') return { state: 'unreadable', said: value?.said ?? null };
  if (!value.supported) return { state: 'unsupported' };
  const p = value.proposal;
  if (!p) {
    return value.active_revision == null ? { state: 'idle' } : { state: 'executing', revision: value.active_revision };
  }
  const base = { revision: p.revision, base_revision: p.base_revision ?? null };
  if (p.receipt) return { ...base, state: p.receipt.decision === 'approve' ? 'approved' : 'changes_requested' };
  if (!hasToken) return { ...base, state: 'no_token' };
  return { ...base, state: p.base_revision != null || value.active_revision != null ? 'reapproval' : 'requested' };
}

/** Items with their recorded status. The agent's intent is not a status. */
export function planItems(value) {
  const executed = value?.executed_cycles || [];
  const current = value?.current_item?.id ?? null;
  const cycleState = currentCycle(value);
  // A bound item counts as "open now" only when the server says the current Cycle is open.
  // A closed Cycle keeps its binding, so the item is simply executed; without the field, unknown.
  const bound = cycleState.state === 'open' ? 'current' : cycleState.state === 'closed' ? 'executed' : 'bound';
  const retry = new Set(value?.retry_pending || []);
  const status = (id) => id === current && cycleState.state === 'open' ? 'current' : retry.has(id) ? 'retry' : id === current ? bound : executed.some(e => e.item_id === id) ? 'executed' : 'pending';
  const cycle = (id) => executed.findLast(e => e.item_id === id)?.cycle_ref
    ?? (id === current && cycleState.state === 'closed' ? cycleState.ref : null);
  const p = value?.proposal;
  if (p) return p.draft.items.map(i => ({ ...i, status: status(i.id), cycle_ref: cycle(i.id) }));
  const seen = new Set();
  const out = [];
  for (const e of executed) {
    if (seen.has(e.item_id)) continue;
    seen.add(e.item_id);
    const item = e.item_id === current ? value.current_item : null;
    out.push({ id: e.item_id, question: item?.question ?? null, done_when: item?.done_when ?? null,
      status: status(e.item_id), cycle_ref: cycle(e.item_id) });
  }
  if (current && !seen.has(current)) {
    seen.add(current);
    out.push({ ...value.current_item, status: status(current), cycle_ref: cycle(current) });
  }
  for (const i of value?.pending || []) {
    if (seen.has(i.id)) continue;
    seen.add(i.id);
    out.push({ ...i, status: 'pending', cycle_ref: null });
  }
  return out;
}

/** What approving this Synthesis means, in one plain sentence. Unknown verdicts are quoted, not interpreted. */
export function verdictSentence(synthesis, itemCount) {
  const verdict = synthesis?.chain_verdict;
  if (verdict === undefined || verdict === null || verdict === '') {
    return t('plan.verdict_items', { count: itemCount });
  }
  if (verdict === 'success') return t('plan.verdict_success');
  if (verdict === 'failure') return t('plan.verdict_failure');
  return t('plan.verdict_other', { verdict });
}

/** The one line at the top of the Monitor. Empty when nothing waits for the person. */
export function bannerLine(summary, delivery = null) {
  const plan = summary.revision != null ? t('plan.plan_v', { revision: summary.revision }) : t('plan.plan');
  const known = summary.confirmed ? t('plan.known', { revision: summary.confirmed.revision, decision: decisionWord(summary.confirmed.decision) }) : '';
  switch (summary.state) {
    case 'checking': return t('plan.banner.checking');
    case 'retrying': return t('plan.banner.retrying', { known });
    case 'requested': return t('plan.banner.requested', { plan });
    case 'reapproval': return t('plan.banner.reapproval', { plan,
      previous: summary.base_revision != null ? t('plan.banner.previous_v', { revision: summary.base_revision }) : t('plan.banner.previous') });
    case 'no_token': return t('plan.banner.no_token', { plan });
    case 'approved': return delivery === 'undelivered' ? t('plan.banner.approved_undelivered', { plan }) : t('plan.banner.approved', { plan });
    case 'changes_requested': return t('plan.banner.changes_requested', { plan });
    case 'uncertain': return t('plan.banner.uncertain', { plan });
    case 'unreadable': return t('plan.banner.unreadable', { known });
    default: return '';
  }
}

// Addressed to the Agent, so it stays English whatever language the Monitor shows.
export const continuation = (p, decision) =>
  `The GIL Monitor recorded ${decision === 'approve' ? 'an approval' : 'a change request'} for plan v${p.revision} (${p.synthesis_ref}). Read the actual record with gil_plan_read and close the Synthesis with its close_report to proceed. If approved, also close the Outcome and the Interview Cycle, then proceed to the Experiment. If changes were requested, ask about them in the next Question. Answer the user in their language.`;
const manualContinue = () => t('plan.manual_continue');

export function makePlanReview({ app, scope, panel, banner = null, document, changed = () => {} }) {
  let current = null, token = null, busy = false, dead = false, rendered = null, heading = null;
  let last = null, lastState = null; // last value read successfully; named as text when a later read fails
  let uncertain = null;   // a decide call whose reply never arrived: { revision, decision, said }
  let delivery = null;    // { digest, state: 'delivered' | 'undelivered' } for a decision made on this screen
  let shown = null;       // digest of the proposal last shown without a receipt
  let replaced = false;   // that proposal was replaced before anyone answered it
  let readFailures = 0;
  const confirmedRecord = () => last?.proposal?.receipt
    ? { revision: last.proposal.revision, decision: last.proposal.receipt.decision }
    : last?.active_revision != null ? { revision: last.active_revision, decision: 'approve' } : null;
  const text = (tag, content, className) => {
    const n = document.createElement(tag);
    n.textContent = content;
    if (className) n.className = className;
    return n;
  };
  const button = (label, extra = {}) => {
    const b = text('button', label, extra.className);
    b.type = 'button';
    if (extra.decision) b.dataset.decision = extra.decision;
    if (extra.action) b.dataset.action = extra.action;
    return b;
  };
  const buttons = () => [...panel.querySelectorAll('button')];
  const key = (value) => JSON.stringify([value, !!token, delivery, uncertain]);
  function clear() { panel.replaceChildren(); heading = null; }

  // Focus survives a re-render: the same decision button, else the heading. Nothing scrolls here.
  function standing() {
    const active = document.activeElement;
    if (!active) return null;
    if (banner?.contains?.(active)) return { where: 'banner' };
    if (panel.contains?.(active)) return { where: 'panel', decision: active.dataset?.decision ?? null, action: active.dataset?.action ?? null };
    return null;
  }
  function restore(stood) {
    if (!stood) return;
    let target = null;
    if (stood.where === 'banner') target = banner && !banner.hidden ? [...banner.querySelectorAll('button')][0] : heading;
    else target = buttons().find(b => (stood.decision && b.dataset.decision === stood.decision) || (stood.action && b.dataset.action === stood.action)) || heading;
    target?.focus?.({ preventScroll: true });
  }
  // The only scroll this adapter ever performs, and only on the person's own click: the panel's
  // top comes into view, then the first decision button takes focus (the browser scrolls further
  // only if that button is still outside the viewport).
  function jump() {
    if (panel.hidden) return;
    panel.scrollIntoView?.({ block: 'start' });
    const target = buttons().find(b => b.dataset.decision) || heading;
    target?.focus?.();
  }
  function showBanner(summary) {
    if (!banner) return;
    const line = bannerLine(summary, delivery?.state ?? null);
    banner.replaceChildren();
    banner.hidden = !line;
    banner.dataset.state = summary.state;
    if (!line) return;
    banner.append(text('span', line, 'plan-banner-text'));
    if (summary.state !== 'checking') {
      const go = button(t('plan.jump'), { action: 'jump', className: 'plan-jump' });
      go.addEventListener('click', jump);
      banner.append(go);
    }
  }
  function head(summary, title) {
    heading = text('h2', '');
    heading.tabIndex = -1;
    const badge = text('span', planState(summary.state), 'plan-state');
    badge.dataset.state = summary.state;
    heading.append(badge, text('span', title));
    panel.append(heading);
  }
  function itemList(value) {
    const list = document.createElement('ol');
    list.className = 'plan-items';
    for (const item of planItems(value)) {
      const where = (item.status === 'executed' || item.status === 'current') && item.cycle_ref ? ` (${item.cycle_ref})` : '';
      const li = text('li', `${itemStatus(item.status)}${where} · ${item.id}${item.question ? `: ${item.question}` : ''}${item.done_when ? `\n${t('plan.check_label', { done_when: item.done_when })}` : ''}`);
      li.dataset.status = item.status;
      list.append(li);
    }
    return list;
  }
  function counts(value) {
    const items = planItems(value);
    const n = (status) => items.filter(i => i.status === status).length;
    const open = currentCycle(value).state === 'unknown' ? t('plan.counts_bound', { n: n('bound') }) : t('plan.counts_current', { n: n('current') });
    return t('plan.counts', { executed: n('executed'), open, pending: n('pending'), retry: n('retry') ? t('plan.counts_retry', { n: n('retry') }) : '' });
  }
  function reinterview(value) {
    for (const request of value.reinterview_requests || []) {
      panel.append(text('p', t('plan.reinterview', { cycle: request.cycle_ref, reason: request.reason }), 'plan-reinterview'));
    }
  }
  // This message is a consequence of an explicit button click, never of polling/rendering.
  // The persisted receipt remains authoritative; message failure must not repeat the write.
  async function notify(p, decision) {
    try {
      const message = await app.sendMessage({ role: 'user', content: [{ type: 'text', text: continuation(p, decision) }] }, { timeout: 10000 });
      if (message?.isError) throw new Error('message refused');
      delivery = { digest: p.digest, state: 'delivered' };
    } catch {
      delivery = { digest: p.digest, state: 'undelivered' };
    }
  }
  function deliveryNote(p, decision) {
    const state = delivery?.digest === p.digest ? delivery.state : null;
    if (state === 'delivered') { panel.append(text('p', t('plan.delivered'), 'plan-delivered')); return; }
    panel.append(text('p', state === 'undelivered'
      ? t('plan.undelivered', { manual: manualContinue() })
      : t('plan.delivery_unknown', { manual: manualContinue() }), 'plan-delivery'));
    const retry = button(t('plan.send_continue'), { action: 'notify' });
    retry.disabled = busy;
    retry.addEventListener('click', async (event) => {
      if (!event.isTrusted || busy || dead || current?.digest !== p.digest) return;
      busy = true; for (const b of buttons()) b.disabled = true;
      try { await notify(p, decision); } finally { busy = false; }
      if (!dead && last) { render(last); rendered = key(last); }
    });
    panel.append(retry);
  }
  function decisionButtons(p) {
    for (const [decision, label] of [['approve', t('plan.approve_button')], ['request_changes', t('plan.request_changes_button')]]) {
      const b = button(label, { decision });
      b.disabled = busy || !token;
      b.addEventListener('click', async (event) => {
        if (!event.isTrusted || busy || dead || !token || current?.digest !== p.digest) return;
        busy = true; for (const other of buttons()) other.disabled = true;
        const request = { scope_id: scope, revision: p.revision, digest: p.digest, token, decision };
        try {
          const result = await app.callServerTool({ name: 'gil_plan_decide', arguments: request }, { timeout: 10000 });
          if (dead) return;
          if (result.isError) throw new Error(result.structuredContent?.said || t('plan.decide_failed'));
          const saved = result.structuredContent?.proposal;
          if (saved?.revision !== p.revision || saved?.digest !== p.digest || saved?.receipt?.decision !== decision) {
            throw new Error(t('plan.decide_missing'));
          }
          token = null; current = result.structuredContent?.proposal; last = result.structuredContent;
          lastState = planSummary(last); readFailures = 0;
          busy = false;
          render(last); rendered = key(last); changed();
          busy = true;
          await notify(p, decision);
          if (dead) return;
          busy = false;
          render(last); rendered = key(last);
        } catch (err) {
          if (!dead) {
            token = null; current = null; rendered = null;
            uncertain = { revision: p.revision, decision, said: err?.message || t('plan.no_reply') };
            render(last);
          }
        } finally { busy = false; }
      });
      panel.append(b);
    }
  }
  function proposal(value, summary) {
    const p = value.proposal;
    head(summary, t('plan.head_revision', { revision: p.revision, base: summary.base_revision != null ? t('plan.head_base', { revision: summary.base_revision }) : '' }));
    if (replaced) panel.append(text('p', t('plan.replaced'), 'plan-replaced'));
    // What the decision means, then what it covers, then the controls. Full texts stay below as
    // collapsible, verifiable reference: nothing is dropped, it only comes after the decision.
    const synthesis = p.draft.synthesis || {};
    panel.append(text('p', verdictSentence(synthesis, p.draft.items.length), 'plan-verdict'));
    if (synthesis.statement) panel.append(text('p', synthesis.statement, 'plan-statement'));
    const dl = document.createElement('dl');
    for (const k of ['goal', 'in_scope', 'out_of_scope', 'constraints', 'success_criteria']) dl.append(text('dt', t(`plan.scope.${k}`)), text('dd', p.draft.scope[k]));
    panel.append(dl, text('h3', t('plan.items_heading')), text('p', counts(value), 'plan-counts'), itemList(value));
    reinterview(value);
    if (p.receipt) {
      const approve = p.receipt.decision === 'approve';
      panel.append(text('p', approve ? t('plan.receipt_approved') : t('plan.receipt_changes'), 'plan-receipt'));
      deliveryNote(p, p.receipt.decision);
    } else {
      panel.append(text('p', summary.state === 'no_token' ? t('plan.hint_no_token') : t('plan.hint_decide'), 'plan-hint'));
      decisionButtons(p);
    }
    const full = document.createElement('details');
    full.append(text('summary', t('plan.full_summary')));
    const rest = document.createElement('dl');
    for (const k of Object.keys(synthesis)) rest.append(text('dt', k), text('dd', synthesis[k]));
    full.append(rest, text('pre', JSON.stringify(synthesis, null, 2)));
    const trace = document.createElement('details');
    trace.append(text('summary', t('plan.trace_summary')), text('p', t('plan.trace', { base: p.base_revision ?? t('plan.none'), ref: p.synthesis_ref, digest: p.digest })));
    panel.append(full, trace);
  }
  function executing(value, summary) {
    head(summary, t('plan.last_state_revision', { revision: value.active_revision }));
    const item = value.current_item;
    const cycle = currentCycle(value);
    const where = cycle.ref ? ` ${cycle.ref}` : '';
    const bound = item ? t('plan.bound_item', { id: item.id, question: item.question, done_when: item.done_when }) : null;
    let line;
    if (cycle.state === 'open') line = bound ? t('plan.exec_open_bound', { where, bound }) : t('plan.exec_open_none', { where });
    else if (cycle.state === 'closed') line = bound ? t('plan.exec_closed_bound', { where, bound }) : t('plan.exec_closed_none', { where });
    else line = bound ? t('plan.exec_unknown_bound', { bound }) : t('plan.exec_unknown_none');
    const current = text('p', line, 'plan-current');
    current.dataset.cycle = cycle.state;
    panel.append(current);
    panel.append(text('p', counts(value), 'plan-counts'), itemList(value));
    reinterview(value);
    panel.append(text('small', t('plan.exec_note')));
  }
  function render(value) {
    const stood = standing();
    const summary = planSummary(value, { hasToken: !!token, uncertain });
    const p = summary.state === 'unreadable' ? null : value?.proposal;
    if (p && !p.receipt) { replaced = shown !== null && shown !== p.digest; shown = p.digest; }
    else { replaced = false; if (p?.receipt) shown = null; }
    clear();
    panel.hidden = summary.state === 'unsupported';
    panel.dataset.state = summary.state;
    showBanner(summary);
    if (panel.hidden) return;
    switch (summary.state) {
      case 'uncertain': {
        head(summary, t('plan.last_state_revision', { revision: uncertain.revision }));
        panel.append(text('p', t('plan.uncertain_sent', { decision: decisionWord(uncertain.decision), said: uncertain.said }), 'plan-uncertain'));
        panel.append(text('p', t('plan.uncertain_hint'), 'plan-hint'));
        const again = button(t('plan.reread'), { action: 'reread' });
        again.addEventListener('click', () => api.refresh());
        panel.append(again);
        break;
      }
      case 'checking':
        head(summary, '');
        panel.append(text('p', t('plan.checking_hint'), 'plan-hint'));
        break;
      case 'retrying':
      case 'unreadable': {
        head(summary, summary.said ? ` · ${summary.said}` : '');
        panel.append(text('p', bannerLine(summary), 'plan-hint'));
        panel.append(text('p', t('plan.unreadable_hint'), 'plan-hint'));
        if (lastState) {
          panel.append(text('p', t('plan.last_state', { state: planState(lastState.state) || lastState.state,
            revision: lastState.revision != null ? t('plan.last_state_revision', { revision: lastState.revision }) : '' }), 'plan-last'));
        }
        const again = button(t('plan.reread'), { action: 'reread' });
        again.addEventListener('click', () => api.refresh());
        panel.append(again);
        break;
      }
      case 'idle':
        head(summary, '');
        panel.append(text('p', t('plan.idle_hint'), 'plan-hint'));
        reinterview(value);
        break;
      case 'executing':
        executing(value, summary);
        break;
      default:
        proposal(value, summary);
    }
    restore(stood);
  }
  const api = {
    async refresh() {
      if (dead) return { status: 'stopped' };
      if (busy) return { status: 'busy' };
      busy = true;
      if (!last && !uncertain && readFailures === 0) render({ code: 'checking' });
      try {
        const result = await app.callServerTool({ name: 'gil_plan_review', arguments: { scope_id: scope } }, { timeout: 10000 });
        if (dead) return { status: 'stopped' };
        if (result.isError || typeof result.structuredContent?.supported !== 'boolean') throw new Error(result.structuredContent?.said || t('plan.review_failed'));
        const value = result.structuredContent;
        current = value.proposal; last = value; uncertain = null;
        token = result._meta?.['gil/planApprovalToken'] ?? null;
        lastState = planSummary(value, { hasToken: !!token });
        readFailures = 0;
        const k = key(value);
        busy = false;
        // Preserve focus while polling. A renewed token does not change the visible proposal.
        if (k !== rendered) { render(value); rendered = k; }
        return { status: 'ok' };
      } catch (err) {
        if (dead) return { status: 'stopped' };
        readFailures += 1;
        token = null; current = null; rendered = null;
        render({ code: readFailures < 3 ? 'retrying' : 'unreadable', said: err?.message, confirmed: confirmedRecord() });
        return { status: 'failed', retryAfterMs: Math.min(15000, 2000 * 2 ** Math.min(readFailures - 1, 3)) };
      } finally { busy = false; }
    },
    stop() {
      dead = true; token = null; current = null; clear(); panel.hidden = true;
      if (banner) { banner.replaceChildren(); banner.hidden = true; }
    },
  };
  return api;
}
