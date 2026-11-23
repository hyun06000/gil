import { t } from '../ui/locale.js';
// The start preparation panel (tracking-policy development profile). Same App-only consent as the
// plan and revisit panels: the capability stays in this closure, never in the DOM, GIL_HOST,
// host evidence or model-visible content. A tool receipt, a chat reply or a synthetic click is
// never an approval; only a trusted click on the exact proposal shown sends one.
//
// The continuation messages are addressed to the Agent, so they are English whatever the
// Monitor's language. They are hints to re-read Core state, never a substitute for it.
export const startedContinuation = (scope) => `The GIL start approved in the Monitor is committed: policy P1, event M1, the initial Snapshot, Chain C1 and the first Interview are saved. Call show_gil_monitor(${JSON.stringify({ scope_id: scope })}) once, then gil_open(kind: question) in the first Interview. Do not repeat start or ask the user to approve again. Answer the user in their language.`;
export const heldContinuation = (code) => `The user approved the start proposal in the GIL Monitor and the approval is recorded, but the formal start was held (${code}). Read gil_start_status once with the same project_root and follow its next guidance${code === 'combine_required' ? ': tell the user once that you will combine the approved rules and the .gilignore now on disk, then call gil_start_prepare with combine_with_revision; the combined text needs its own Monitor approval' : ''}. Nothing was started. Do not treat the earlier approval as approval of a new proposal. Answer the user in their language.`;

export function makeStartReview({ app, scope, panel, document, changed = () => {} }) {
  let stopped = false, busy = false, reading = false, value = null, token = null, applyToken = null, uncertain = null, delivery = null;
  const text = (tag, content) => { const e = document.createElement(tag); e.textContent = content ?? ''; return e; };
  const list = (items, show) => { const ul = document.createElement('ul'); for (const item of items || []) ul.append(text('li', show(item))); return ul; };
  function head(content) { panel.hidden = false; panel.replaceChildren(text('h2', t('start.title')), text('p', content)); }

  async function notify(kind, detail) {
    const content = kind === 'started' ? startedContinuation(scope) : heldContinuation(detail);
    try {
      const reply = await app.sendMessage({ role: 'user', content: [{ type: 'text', text: content }] }, { timeout: 10000 });
      if (reply?.isError) throw new Error('message refused');
      delivery = 'delivered';
    } catch { delivery = 'undelivered'; }
  }

  function proposalSection(p) {
    panel.append(text('h3', t('start.proposal', { revision: p.revision })));
    panel.append(text('p', t('start.identity', { engine: p.engine, digest: String(p.digest).slice(0, 16) })));
    panel.append(text('h4', t('start.rules')));
    panel.append(text('pre', p.rules));
    const showExcluded = (e) => `${e.path} — ${e.reason}${e.interior === 'not_explored' ? ` · ${t('start.not_explored')}` : ''}`;
    panel.append(text('h4', t('start.tracked', { n: p.tracked_count })));
    const tracked = list(p.tracked, (path) => path);
    panel.append(tracked);
    if (p.tracked_truncated) fullList(p, 'tracked', tracked, (path) => path);
    panel.append(text('h4', t('start.excluded', { n: p.excluded_count })));
    const excluded = list(p.excluded, showExcluded);
    panel.append(excluded);
    if (p.excluded_truncated) fullList(p, 'excluded', excluded, showExcluded);
    panel.append(text('h4', t('start.protections')));
    panel.append(list(p.system_protections, (s) => s.shown));
    panel.append(text('h4', t('start.recommendations')));
    panel.append(list(p.recommendations, (r) => `${r.in_rules ? '☑' : '☐'} ${r.rules.join(', ')} — ${r.reason} (${r.sources.join(', ')}; ${r.default_selected ? t('start.default_selected') : t('start.default_unselected')})`));
    panel.append(text('h4', t('start.policy_file')));
    const effect = p.policy_file?.effect;
    panel.append(text('p', t(`start.policy_file.${effect}`) === `start.policy_file.${effect}` ? effect : t(`start.policy_file.${effect}`)));
    if (Array.isArray(p.policy_file?.diff)) panel.append(text('pre', p.policy_file.diff.join('\n')));
    if (p.combination) {
      const c = p.combination;
      if ((c.disk_problems || []).length) panel.append(text('p', t('start.disk_problems', { problems: c.disk_problems.join('; ') })));
      panel.append(text('h4', t('start.combination', { revision: c.approved_revision })));
      panel.append(text('p', t('start.combination_note')));
      panel.append(text('h4', t('start.diff_vs_approved')), text('pre', (c.diff_vs_approved || []).join('\n')));
      panel.append(text('h4', t('start.diff_vs_disk')), text('pre', (c.diff_vs_disk || []).join('\n')));
      panel.append(text('h4', t('start.rule_effects')));
      panel.append(list(c.rule_order_effects, (e) => `${e.path}: ${e.decided}`));
    }
    panel.append(text('h4', t('start.impact')));
    panel.append(list(p.impact, (line) => line));
    if ((p.blockers || []).length) {
      panel.append(text('h4', t('start.blockers')));
      panel.append(list(p.blockers, (line) => line));
    }
  }

  // A shortened list says so, and the whole list of THIS stored revision can be read before approving.
  // Reading pages creates no approval, application or capability.
  function fullList(p, which, ul, show) {
    const note = text('p', t('start.truncated'));
    const more = text('button', t('start.list_all')); more.type = 'button';
    more.addEventListener('click', async (event) => {
      if (!event.isTrusted || stopped || busy) return;
      more.disabled = true;
      try {
        const items = []; let offset = 0;
        for (;;) {
          const result = await app.callServerTool({ name: 'gil_start_list', arguments: { scope_id: scope, revision: p.revision, digest: p.digest, list: which, offset } }, { timeout: 10000 });
          const page = result.structuredContent;
          if (result.isError || !page || page.digest !== p.digest || page.revision !== p.revision) throw new Error(page?.said || t('start.read_failed'));
          items.push(...page.items);
          if (page.next_offset == null) break;
          offset = page.next_offset;
        }
        if (stopped || value?.proposal?.digest !== p.digest) return;
        ul.replaceChildren(...items.map((item) => text('li', show(item))));
        note.textContent = t('start.list_complete', { n: items.length });
        more.remove();
      } catch (e) { if (!stopped) { note.textContent = e.message || t('start.read_failed'); more.disabled = false; } }
    });
    panel.append(note, more);
  }

  function buttons(p) {
    const approve = text('button', t('start.approve')), changes = text('button', t('start.request_changes'));
    for (const b of [approve, changes]) { b.type = 'button'; b.disabled = busy || !token || !p.approvable; }
    if (!p.approvable) changes.disabled = busy || !token;
    const send = async (event, decision) => {
      if (!event.isTrusted || stopped || busy || !token || value?.proposal?.digest !== p.digest) return;
      if (decision === 'approve' && !p.approvable) return;
      busy = true; approve.disabled = true; changes.disabled = true;
      const request = { scope_id: scope, revision: p.revision, digest: p.digest, token, decision };
      token = null;
      try {
        const result = await app.callServerTool({ name: 'gil_start_decide', arguments: request }, { timeout: 120000 });
        if (stopped) return;
        if (result.isError || !result.structuredContent) throw Object.assign(new Error(result.structuredContent?.said || result.structuredContent?.problem || t('start.decide_failed')), { refused: true });
        value = result.structuredContent; uncertain = null; render(); changed();
        if (value.phase === 'started') await notify('started');
        else if (value.application_held) await notify('held', value.application_held.code);
      } catch (e) {
        if (!stopped) { uncertain = e.refused ? null : { revision: p.revision, decision }; head(e.refused ? e.message : t('start.result_unknown')); rereadButton(); }
      } finally { busy = false; if (!stopped && !uncertain && value) render(); }
    };
    approve.addEventListener('click', (event) => send(event, 'approve'));
    changes.addEventListener('click', (event) => send(event, 'request_changes'));
    panel.append(approve, changes, text('p', t('start.before_approval')));
  }

  // The approval is already recorded; this button only retries the application of that exact proposal.
  function applyButton(p) {
    const b = text('button', t('start.apply_retry')); b.type = 'button'; b.disabled = busy || !applyToken;
    b.addEventListener('click', async (event) => {
      if (!event.isTrusted || stopped || busy || !applyToken || value?.proposal?.digest !== p.digest) return;
      busy = true; b.disabled = true;
      const request = { scope_id: scope, revision: p.revision, digest: p.digest, token: applyToken };
      applyToken = null;
      try {
        const result = await app.callServerTool({ name: 'gil_start_apply', arguments: request }, { timeout: 120000 });
        if (stopped) return;
        if (result.isError || !result.structuredContent) throw Object.assign(new Error(result.structuredContent?.said || result.structuredContent?.problem || t('start.decide_failed')), { refused: true });
        value = result.structuredContent; render(); changed();
        if (value.phase === 'started') await notify('started');
      } catch (e) {
        if (!stopped) { uncertain = e.refused ? null : { revision: p.revision, decision: 'apply' }; head(e.refused ? e.message : t('start.result_unknown')); rereadButton(); }
      } finally { busy = false; if (!stopped && !uncertain && value) render(); }
    });
    panel.append(b, text('p', t('start.apply_note')));
  }

  function rereadButton() {
    const b = text('button', t('start.reread')); b.type = 'button';
    b.addEventListener('click', (event) => { if (event.isTrusted) void api.refresh(); });
    panel.append(b);
  }

  function render() {
    head(value?.said || t('start.checking'));
    if (value?.next) panel.append(text('p', value.next));
    if (value?.application_held) panel.append(text('p', t('start.held', { code: value.application_held.code, said: value.application_held.said })));
    if (value?.phase === 'started') {
      const s = value.started || {};
      panel.append(list([`policy ${s.policy}`, `event ${s.event}`, `snapshot ${s.snapshot}`, `chain ${s.chain}`, `interview ${s.first_interview}`, `.gilignore ${s.policy_file_sync_base}`], (line) => line));
      if (delivery) panel.append(text('p', t(delivery === 'delivered' ? 'start.delivered' : 'start.undelivered')));
      rereadButton();
      return;
    }
    if (value?.proposal) {
      proposalSection(value.proposal);
      // The current disk is an observation, not part of what is approved; a difference means the application will hold.
      if (value.current && value.current.policy_file_matches_proposal === false) panel.append(text('p', t('start.current_differs', { now: value.current.policy_file })));
      if (value.proposal.response) {
        panel.append(text('p', t('start.responded', { decision: value.proposal.response.decision })));
        if (value.phase === 'approved' && value.proposal.response.decision === 'approve') applyButton(value.proposal);
      }
      else if (['awaiting_approval', 'blocked', 'policy_file_held'].includes(value.phase)) buttons(value.proposal);
    }
    rereadButton();
  }

  const api = {
    async refresh() {
      if (stopped || busy || reading) return;
      reading = true;
      try {
        const result = await app.callServerTool({ name: 'gil_start_review', arguments: { scope_id: scope } }, { timeout: 10000 });
        if (stopped) return;
        if (result.isError || !result.structuredContent) throw new Error(result.structuredContent?.said || result.structuredContent?.problem || t('start.read_failed'));
        value = result.structuredContent; token = result._meta?.['gil/startApprovalToken'] || null;
        applyToken = result._meta?.['gil/startApplyToken'] || null; uncertain = null; render();
      } catch (e) { if (!stopped) { token = null; applyToken = null; head(e.message || t('start.read_failed')); rereadButton(); } }
      finally { reading = false; }
    },
    stop() { stopped = true; token = null; applyToken = null; panel.querySelectorAll('button').forEach((b) => { b.disabled = true; }); },
  };
  return api;
}
