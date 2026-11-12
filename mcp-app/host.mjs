// Transport adapter only: no layout, .gil parsing, domain joins, or filesystem paths.
import { mountVisuals } from './visuals.mjs';
import { t } from '../ui/locale.js';
export const RETRY_MS = [2000, 4000, 8000, 16000, 30000, 60000];
export const retryAfter = (failures) => RETRY_MS[Math.min(Math.max(failures - 1, 0), RETRY_MS.length - 1)];
export const RECONCILE_MS = 300_000;

export function connectionMessage(error) {
  switch (error?.code) {
    case 'unknown_scope': case 'reconnect_required':
      return t('host.no_binding');
    case 'project_missing':
      return t('host.project_missing');
    case 'project_moved':
      return t('host.project_moved');
    default:
      return error?.said || t('host.rechecking');
  }
}

export function unwrap(result) {
  const data = result?.structuredContent;
  if (result?.isError || !data) {
    throw { code: data?.code || 'unavailable', said: data?.said || t('host.unreadable') };
  }
  return data;
}

export function makeHost(app, initial, { now = Date.now, presence = () => ({}) } = {}) {
  const scope = initial.scope_id;
  const listeners = new Map();
  let seed = initial;
  let cursor = initial.revision;
  let worked = now();
  let nextRead = 0;
  let failures = 0;
  let detailFailures = 0, detailNextRead = 0, detailRequest = 0;
  let watchWasLive = initial.watching;
  let disconnected = false;
  const emit = async (event, payload) => { for (const fn of listeners.get(event) || []) await fn(payload); };
  const call = async (name, extra = {}) => {
    try {
      const data = unwrap(await app.callServerTool({ name, arguments: { scope_id: scope, ...extra } }, { timeout: 10000 }));
      if (data.scope_id !== scope) throw { code: 'unknown_scope', said: t('host.other_project') };
      if (disconnected) { disconnected = false; await emit('gil://connection', null); }
      return data;
    } catch (err) {
      disconnected = true; await emit('gil://connection', err); throw err;
    }
  };
  const watchNotice = async (live) => {
    if (watchWasLive === live) return;
    watchWasLive = live;
    await emit('gil://say', live ? { code: 'watch_live' }
      : { code: 'watch_unavailable', said: t('host.watch_unavailable') });
  };
  return {
    mountVisuals(container, wanted, owner) {
      if (wanted !== scope) throw {code:'unknown_scope'};
      // Visual errors stay in their own panel, not the graph connection state.
      const visualCall = async (name, args) => unwrap(await app.callServerTool({name, arguments:{scope_id:scope,...args}}));
      const dispose = mountVisuals(container,scope,owner,visualCall);
      const onConnection = (error) => { if (error) dispose(); };
      if (!listeners.has('gil://connection')) listeners.set('gil://connection',new Set());
      listeners.get('gil://connection').add(onConnection);
      return () => { dispose(); listeners.get('gil://connection').delete(onConnection); };
    },
    listProjects: async () => [{ scope_id: scope, label: initial.label }],
    opening: async () => ({ last_selected: scope }),
    async loadView(wanted) {
      if (wanted !== scope) throw { code: 'unknown_scope', said: t('host.not_this_project') };
      try {
        const data = seed || await call('gil_monitor_read');
        seed = null;
        if (data.view?.schema_version !== 1) throw { code: 'unsupported_vocabulary', said: t('host.view_unreadable') };
        cursor = data.revision;
        worked = now(); failures = 0; nextRead = 0;
        await watchNotice(data.watching);
        return data.view;
      } catch (err) {
        failures += 1; nextRead = now() + retryAfter(failures);
        throw err;
      }
    },
    async loadDetail(wanted, stepRef) {
      if (wanted !== scope) throw { code: 'unknown_scope', said: t('host.not_this_project') };
      const request = ++detailRequest;
      try {
        const data = await call('gil_monitor_detail', { step_ref: stepRef });
        if (request === detailRequest) { detailFailures = 0; detailNextRead = 0; }
        return data.detail;
      } catch (err) {
        // A successful View already advanced the cursor. Retry a failed selected detail
        // even when there is no later revision, independently of View's retry budget.
        if (request === detailRequest) {
          detailFailures += 1; detailNextRead = now() + retryAfter(detailFailures);
        }
        throw err;
      }
    },
    listen(event, fn) {
      if (!listeners.has(event)) listeners.set(event, new Set());
      listeners.get(event).add(fn);
      return () => listeners.get(event).delete(fn);
    },
    reportPresence: () => call('gil_monitor_poll', presence()),
    async tick() {
      const hint = await call('gil_monitor_poll', presence()); // counter only; no world scan
      await watchNotice(hint.watching);
      if (hint.update) await emit('gil://update', hint.update);
      const detailDue = detailFailures > 0 && now() >= detailNextRead;
      if (now() >= nextRead && (failures > 0 || hint.revision !== cursor || now() - worked >= RECONCILE_MS || detailDue)) {
        // If selection was cleared, there is no detail request during refresh: retire the retry.
        const before = detailRequest;
        await emit('gil://refresh');
        if (detailDue && detailRequest === before) { detailFailures = 0; detailNextRead = 0; }
      }
    },
    refresh: () => emit('gil://refresh'),
  };
}
