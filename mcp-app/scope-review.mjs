import { t } from '../ui/locale.js';
// The scope-change panel (tracking-policy development profile): the exact proposal, the three values
// Core derived (applied policy / last confirmed world and the policy that confirmed it / pending
// baseline), the explicit archive retry and the archive halt marker. Same App-only consent as the
// start, plan and revisit panels: every capability stays in this closure, never in the DOM, GIL_HOST,
// host evidence or model-visible content. A tool receipt, a chat reply or a synthetic click is never
// an approval, a retry, a halt or a release; only a trusted click on what is shown sends one.
//
// The continuation messages are addressed to the Agent, so they are English whatever the Monitor's
// language. They are hints to re-read Core state, never a substitute for it.
export const scopeContinuation = (what) => `The user acted in the GIL Monitor (${what}). Read gil_scope_status once with the same project_root and follow its next guidance. Do not repeat an approval, do not start an archive retry, and do not treat a released halt as permission to continue by itself: wait for the user's explicit next instruction. Answer the user in their language.`;

export function makeScopeReview({ app, scope, panel, document, changed = () => {} }) {
  let stopped = false, busy = false, halting = false, reading = false, value = null, tokens = {}, uncertain = false, delivery = null;
  let haltNote = null, haltNoteLine = null;
  const text = (tag, content) => { const e = document.createElement(tag); e.textContent = content ?? ''; return e; };
  const list = (items, show) => { const ul = document.createElement('ul'); for (const item of items || []) ul.append(text('li', show(item))); return ul; };
  function head(content) { panel.hidden = false; panel.replaceChildren(text('h2', t('scope.title')), text('p', content)); }

  async function notify(what) {
    try {
      const reply = await app.sendMessage({ role: 'user', content: [{ type: 'text', text: scopeContinuation(what) }] }, { timeout: 10000 });
      if (reply?.isError) throw new Error('message refused');
      delivery = 'delivered';
    } catch { delivery = 'undelivered'; }
  }

  // One trusted click → one tool call with the capability for exactly this purpose and target. Only
  // THAT capability is spent; the halt capability stays so the halt button keeps working while an
  // approval or a retry is in flight. A second approve/retry click is ignored while one is running.
  // After an unknown result only a re-read decides.
  async function send(event, key, name, args, what, guard = () => true) {
    if (!event.isTrusted || stopped || busy || !tokens[key] || !guard()) return;
    busy = true; panel.querySelectorAll('button').forEach((b) => { if (!b.staysDuringWork) b.disabled = true; });
    const request = { scope_id: scope, ...args, token: tokens[key] };
    tokens[key] = null;
    try {
      const result = await app.callServerTool({ name, arguments: request }, { timeout: 180000 });
      if (stopped) return;
      if (result.isError || !result.structuredContent) throw Object.assign(new Error(result.structuredContent?.said || result.structuredContent?.problem || t('scope.action_failed')), { refused: true });
      value = result.structuredContent; uncertain = false; changed();
      await notify(what);
    } catch (e) {
      if (!stopped) { uncertain = !e.refused; head(e.refused ? e.message : t('scope.result_unknown')); haltLine(); rereadButton(); }
    } finally { busy = false; if (!stopped && !uncertain) await api.refresh(); }
  }

  // The halt request has its own path: it may be sent while another request of this panel is still
  // running, and its answer (received / applied) is kept apart from that request's answer.
  async function sendHalt(event, what) {
    if (!event.isTrusted || stopped || halting || !tokens.halt) return;
    halting = true; event.currentTarget && (event.currentTarget.disabled = true);
    const request = { scope_id: scope, token: tokens.halt };
    tokens.halt = null;
    try {
      const result = await app.callServerTool({ name: 'gil_scope_halt', arguments: request }, { timeout: 60000 });
      if (stopped) return;
      if (result.isError || !result.structuredContent) throw Object.assign(new Error(result.structuredContent?.said || result.structuredContent?.problem || t('scope.action_failed')), { refused: true });
      const got = result.structuredContent;
      haltNote = got.halt_request?.applied ? t('scope.halt_note_applied') : t('scope.halt_note_received');
      // While another request is running its own answer will arrive and be shown; this answer does not replace it.
      if (!busy) { value = got; changed(); await notify(what); }
    } catch (e) {
      if (!stopped) haltNote = t('scope.halt_note_failed', { why: e.refused ? e.message : t('scope.result_unknown') });
    } finally {
      halting = false;
      if (!stopped) { if (busy) haltLine(); else await api.refresh(); }
    }
  }

  // The latest answer to a halt request, shown as its own line.
  function haltLine() {
    if (!haltNote) return;
    if (!haltNoteLine) haltNoteLine = text('p', '');
    haltNoteLine.textContent = haltNote;
    if (!Array.from(panel.children).includes(haltNoteLine)) panel.append(haltNoteLine);
  }

  function button(label, onClick, enabled, staysDuringWork = false) {
    const b = text('button', label); b.type = 'button'; b.disabled = (staysDuringWork ? halting : busy) || !enabled;
    b.staysDuringWork = staysDuringWork;
    b.addEventListener('click', onClick);
    return b;
  }

  function valuesSection(k) {
    panel.append(text('h3', t('scope.values')));
    panel.append(list([
      t('scope.applied_policy', { policy: k.applied_policy, by: k.applied_by }),
      k.last_confirmed_world
        ? t('scope.last_confirmed', { world: k.last_confirmed_world, policy: k.confirmed_policy ?? t('scope.not_derived') })
        : t('scope.last_confirmed_not_derived'),
      t('scope.standing', { cycle: k.standing?.cycle, state: t(`scope.state.${k.standing?.state}`) }),
      t('scope.policy_file', { state: value.policy_file }),
    ], (line) => line));
    if (k.basis_agrees === false) panel.append(text('p', t('scope.basis_differs')));
  }

  function waitingSection(w, k) {
    panel.append(text('h3', t('scope.waiting_title', { event: w.event })));
    panel.append(text('p', t('scope.waiting_note', { policy: w.policy, world: w.world_before })));
    if ((w.failed_attempts || []).length) panel.append(list(w.failed_attempts, (cause) => t('scope.attempt_failed', { cause })));
    const retry = button(t('scope.retry'), (event) => send(event, 'retry', 'gil_scope_retry', { event: w.event }, 'baseline archive retry',
      () => value?.tracking?.waiting?.event === w.event), !!tokens.retry);
    panel.append(retry, text('p', k.standing?.state === 'closed_boundary' ? t('scope.retry_note') : t('scope.retry_needs_boundary')));
  }

  function haltSection(k) {
    panel.append(text('h3', t('scope.halt_title')));
    // A request that was received but could not be applied yet (another command holds the project).
    if (value.halt_request_pending) { panel.append(text('p', t('scope.halt_pending'))); return; }
    const h = value.halt;
    if (k.archive_halt && (!h || !h.id)) {
      // The marker exists but this halt has no record to confirm against: releasing is not offered.
      // The only button re-records this halt; the release then needs a fresh screen.
      panel.append(text('p', t('scope.halted_at', { at: k.archive_halt })), text('p', t('scope.halt_record_missing')));
      panel.append(button(t('scope.halt_register'), (event) => sendHalt(event, 'archive halt record'), !!tokens.halt, true));
      return;
    }
    if (k.archive_halt) {
      panel.append(text('p', t('scope.halted_at', { at: k.archive_halt })));
      panel.append(text('p', t('scope.halt_identity', { id: String(h.id).slice(0, 12), state: String(h.state_digest).slice(0, 12) })));
      // Release needs two explicit inputs made on this screen: the state was read, and how to clean up.
      const confirm = document.createElement('input'); confirm.type = 'checkbox'; confirm.id = 'scope-release-confirm';
      const label = text('label', t('scope.release_confirm', { at: k.archive_halt })); label.htmlFor = confirm.id;
      const select = document.createElement('select');
      const none = text('option', t('scope.cleanup.choose')); none.value = ''; select.append(none);
      for (const how of value.cleanups || []) { const o = text('option', t(`scope.cleanup.${how}`)); o.value = how; select.append(o); }
      const release = button(t('scope.release'), (event) => send(event, 'release', 'gil_scope_release',
        { at: h.at, halt_id: h.id, state_digest: h.state_digest, cleanup: select.value, state_confirmed: confirm.checked }, 'archive halt release',
        // Sent only for the halt and the state this screen is showing right now, with both explicit inputs made.
        () => confirm.checked && !!select.value && value?.halt?.id === h.id && value?.halt?.state_digest === h.state_digest
          && value?.tracking?.archive_halt === h.at), !!tokens.release);
      panel.append(confirm, label, select, release, text('p', t('scope.release_note')));
    } else {
      // Stays usable while an approval or a retry of this panel is running.
      const halt = button(t('scope.halt'), (event) => sendHalt(event, 'archive halt request'), !!tokens.halt, true);
      panel.append(halt, text('p', t('scope.halt_note')));
    }
  }

  function proposalSection(p) {
    panel.append(text('h3', t('scope.proposal', { revision: p.revision })));
    panel.append(text('p', t('scope.identity', { engine: p.engine, digest: String(p.digest).slice(0, 16), interview: p.review_interview })));
    panel.append(text('p', t('scope.compares', { policy: p.policy_before, world: p.world_before, confirmed: p.confirmed_policy })));
    panel.append(text('h4', t('scope.rules')), text('pre', p.rules));
    panel.append(text('h4', t('scope.newly_tracked', { n: (p.newly_tracked || []).length })), list(p.newly_tracked, (path) => path));
    panel.append(text('h4', t('scope.newly_excluded', { n: (p.newly_excluded || []).length })), list(p.newly_excluded, (path) => path));
    panel.append(text('h4', t('scope.effects')), list(p.effects, (line) => line));
    if (p.response) { panel.append(text('p', t('scope.responded', { decision: p.response }))); return; }
    if (value.phase !== 'awaiting_approval') { panel.append(text('p', t(`scope.phase.${value.phase}`))); return; }
    const decide = (decision) => (event) => send(event, 'decide', 'gil_scope_decide',
      { revision: p.revision, digest: p.digest, decision }, `scope change ${decision}`, () => value?.proposal?.digest === p.digest);
    panel.append(button(t('scope.approve'), decide('approve'), !!tokens.decide),
      button(t('scope.request_changes'), decide('request_changes'), !!tokens.decide), text('p', t('scope.before_approval')));
  }

  function rereadButton() {
    const b = text('button', t('scope.reread')); b.type = 'button';
    b.addEventListener('click', (event) => { if (event.isTrusted) void api.refresh(); });
    panel.append(b);
  }

  function render() {
    head(value?.said || t('scope.checking'));
    if (value?.next) panel.append(text('p', value.next));
    if (value?.application_held) panel.append(text('p', t('scope.held', { code: value.application_held.code, said: value.application_held.said })));
    if (value?.facts) panel.append(text('pre', JSON.stringify(value.facts, null, 2)));
    // After a final re-read failed the server sends no current state; what it last knew is shown as such.
    if (value?.current_state?.reread === false) panel.append(text('p', t('scope.state_unconfirmed')));
    const k = value?.tracking;
    if (k) {
      valuesSection(k);
      if (k.waiting) waitingSection(k.waiting, k);
      haltSection(k);
    }
    if (value?.proposal) proposalSection(value.proposal);
    if (delivery) panel.append(text('p', t(delivery === 'delivered' ? 'scope.delivered' : 'scope.undelivered')));
    haltLine();
    rereadButton();
  }

  const api = {
    async refresh() {
      // No re-read while a request is running: a re-render would drop the buttons that are waiting for it,
      // and the server could not read the record while that request holds the project anyway.
      if (stopped || busy || halting || reading) return;
      reading = true;
      try {
        const result = await app.callServerTool({ name: 'gil_scope_review', arguments: { scope_id: scope } }, { timeout: 10000 });
        if (stopped) return;
        if (result.isError || !result.structuredContent) throw new Error(result.structuredContent?.said || result.structuredContent?.problem || t('scope.read_failed'));
        value = result.structuredContent;
        const meta = result._meta || {};
        tokens = { decide: meta['gil/scopeApprovalToken'] || null, retry: meta['gil/scopeRetryToken'] || null,
          halt: meta['gil/scopeHaltToken'] || null, release: meta['gil/scopeReleaseToken'] || null };
        uncertain = false; render();
      } catch (e) { if (!stopped) { tokens = {}; head(e.message || t('scope.read_failed')); rereadButton(); } }
      finally { reading = false; }
    },
    stop() { stopped = true; tokens = {}; panel.querySelectorAll('button').forEach((b) => { b.disabled = true; }); },
  };
  return api;
}
