import { t } from '../ui/locale.js';
// Same App-only consent on every Host. A tool receipt is not a user click.
export const CONTINUATION = 'The restore approved in the GIL Monitor is complete. Read gil_context once with the same project_root you already selected and follow the next call of the current record. Do not repeat past approval-wait guidance or ask the user to press the button again. Keep the failure record, evidence and lessons; if the next Cycle is already open, continue from there. Answer the user in their language.';
export function makeRecoveryReview({app, scope, panel, document, changed = () => {}}) {
  let stopped = false, busy = false, reading = false, value = null, token = null, delivery = null;
  const text = (tag, content) => { const e = document.createElement(tag); e.textContent = content; return e; };
  function message(content) { panel.hidden = false; panel.replaceChildren(text('h2', t('recovery.title')), text('p', content)); }
  // Only a trusted click sends a continuation. Polling never starts a model turn.
  // A message is a hint to re-read Core state, not a substitute for its receipt. It is
  // addressed to the Agent, so it is English regardless of the Monitor's language.
  async function notify(completed) {
    try {
      const reply = await app.sendMessage({role:'user', content:[{type:'text', text: CONTINUATION}]}, {timeout:10000});
      if (reply?.isError) throw new Error('message refused');
      delivery = {digest:completed.digest, state:'delivered'};
    } catch { delivery = {digest:completed.digest, state:'undelivered'}; }
  }
  function completedNotice() {
    panel.append(text('p', t('recovery.completed')));
    const delivered = delivery?.digest === value.digest && delivery?.state === 'delivered';
    panel.append(text('p', delivered ? t('recovery.delivered') : t('recovery.undelivered')));
    if (delivered) return;
    const completed = value, button = text('button', t('recovery.send_continue'));
    button.type = 'button'; button.disabled = busy;
    button.addEventListener('click', async event => {
      if (!event.isTrusted || stopped || busy || value?.state !== 'completed' || value.digest !== completed.digest ||
          (delivery?.digest === completed.digest && delivery?.state === 'delivered')) return;
      busy = true; button.disabled = true;
      try { await notify(completed); } finally { busy = false; }
      if (!stopped) render();
    });
    panel.append(button);
  }
  function render() {
    if (value?.state === 'not_needed') { panel.hidden = true; panel.replaceChildren(); return; }
    message(value?.notice || value?.reason || t('recovery.checking'));
    if (!value?.supported) return;
    panel.append(text('p', `${value.from} → ${value.target}`));
    const list = document.createElement('ul');
    for (const change of value.changes || []) { const key = `recovery.action.${change.action}`, label = t(key); list.append(text('li', `${label === key ? change.action : label}: ${change.path}`)); }
    panel.append(list, text('p', t('recovery.archive', { archive: value.archive })));
    if (value.state === 'completed') { completedNotice(); return; }
    const button = text('button', value.state === 'resumable' ? t('recovery.resume') : t('recovery.apply'));
    button.type = 'button'; button.disabled = busy || !token;
    button.addEventListener('click', async event => {
      if (!event.isTrusted || stopped || busy || !token) return;
      busy = true; button.disabled = true;
      const request = { scope_id: scope, digest: value.digest, token };
      token = null;
      try {
        const result = await app.callServerTool({name:'gil_revisit_apply', arguments:request}, {timeout:120000});
        if (stopped) return;
        if (result.isError || !result.structuredContent) throw new Error(result.structuredContent?.problem || result.structuredContent?.said || t('recovery.apply_failed'));
        value = result.structuredContent; render(); changed();
        if (value.state === 'completed') await notify(value);
      } catch (e) {
        if (!stopped) message(t('recovery.result_unknown', { error: e.message || '' }));
      } finally { busy = false; if (!stopped && value?.state === 'completed') render(); }
    });
    panel.append(button, text('p', t('recovery.before_approval')));
  }
  return {
    async refresh() {
      if (stopped || busy || reading) return;
      reading = true;
      try {
        const result = await app.callServerTool({name:'gil_revisit_review', arguments:{scope_id:scope}}, {timeout:10000});
        if (stopped) return;
        if (result.isError || !result.structuredContent) throw new Error(result.structuredContent?.problem || result.structuredContent?.said || t('recovery.list_failed'));
        value = result.structuredContent; token = result._meta?.['gil/revisitApprovalToken'] || null; render();
      } catch (e) { if (!stopped) { token = null; message(e.message || t('recovery.list_failed')); } }
      finally { reading = false; }
    },
    stop() { stopped = true; token = null; panel.querySelectorAll('button').forEach(b => { b.disabled = true; }); },
  };
}
