import { App, applyDocumentTheme, applyHostStyleVariables, applyHostFonts } from '@modelcontextprotocol/ext-apps';
import { makePlanReview } from './plans.mjs';
import { makePlanPolling } from './plan-poll.mjs';
import { makeRecoveryReview } from './recovery.mjs';
import { makeHost, unwrap, retryAfter, connectionMessage } from './host.mjs';
import { graphOrientation, initialFullscreen, fullscreenFailure, hostIdentity, hostStyles, styleEvidence, updateNotice, updateGuide } from './presentation.mjs';
import { setLocale, getLocale, normalizeLocale, t, applyStaticText } from '../ui/locale.js';

const app = new App({ name: 'GIL Monitor', version: '0.1.0' },
  { availableDisplayModes: ['inline', 'fullscreen'] });
const el = (id) => document.getElementById(id);
const raw = (v) => v === undefined ? 'undefined' : JSON.stringify(v, null, 2);
let context = {}, connected = false, pending = false, booted = false, stopped = false;
let rendererReady = false, planReview = null, planPolling = null;
let recoveryReview = null, recoveryReadAt = 0;
let response = null, eventMode = null, timer, pollFailures = 0;
let suspended = false, polling = false, refreshOnReturn = false;
let connectionIssue = null;
let layoutChoice = null;
let presenceSequence = 0, renderFailed = false;
// Host identity is recorded as reported, never inferred; policy does not branch on it.
const evidence = { declared: ['inline', 'fullscreen'], instance: crypto.randomUUID(), host: null, styles: null, locale: null };
const initialDisplay = initialFullscreen();

// The Host's theme, standard style variables and font CSS are adopted as given. The
// stylesheet reads them through var(--color-*, fallback); without them the neutral
// defaults apply. This never branches on the Host's identity.
function adoptHostStyles() {
  const got = hostStyles(context);
  try {
    if (got.theme) applyDocumentTheme(got.theme);
    if (got.variables) applyHostStyleVariables(got.variables);
    if (got.fonts) applyHostFonts(got.fonts);
  } catch { /* a Host that rejects style application still gets the neutral defaults */ }
  evidence.styles = styleEvidence(context);
}

// The Host's locale (MCP Apps hostContext.locale, BCP 47) chooses the Monitor's language; a Host
// that says nothing gets English. The language is adopted as given and never inferred from the
// Host's name. A change re-renders the static labels and, once the renderer is up, the graph.
let hostLocale = null;
function adoptLocale(source) {
  const given = typeof source?.locale === 'string' && source.locale.trim() ? source.locale : null;
  const before = getLocale();
  hostLocale = given;
  const chosen = setLocale(given);
  applyStaticText(document);
  evidence.locale = { host: given, chosen };
  return chosen !== before;
}

// Report only this rendered App's observations. Host display mode is not OS occlusion proof.
function presence() {
  const presentation = stopped ? 'closed' : suspended || document.hidden ? 'hidden'
    : renderFailed ? 'error' : !rendererReady ? 'loading'
    : ['inline', 'fullscreen'].includes(context.displayMode) ? context.displayMode : 'loading';
  // The automatic fullscreen intent travels with the same report, so the Core can say why a
  // Monitor is inline (Host refused, surface without fullscreen, user choice) instead of guessing.
  // The Host's locale travels with the same report so the Core can answer the user in the language
  // the App shows. Omitted when the Host gave none: the Core keeps its own default or the Agent's choice.
  return { instance_id: evidence.instance, sequence: String(++presenceSequence), presentation,
    automatic_fullscreen: initialDisplay.summary(context),
    ...(hostLocale ? { locale: normalizeLocale(hostLocale) } : {}) };
}
function reportPresence() {
  if (!connected || !window.GIL_HOST) return Promise.resolve();
  return window.GIL_HOST.reportPresence().catch(() => {});
}

function modes(message) {
  document.body.dataset.displayMode = context.displayMode || 'unknown';
  const direction = graphOrientation(context.displayMode, layoutChoice);
  window.GIL_COMPANION?.setOrientation(direction);
  el('layout').textContent = direction === 'horizontal' ? t('app.layout_vertical') : t('app.layout_horizontal');
  el('layout').disabled = !rendererReady;
  for (const [id, mode] of [['expand', 'fullscreen'], ['inline', 'inline']]) {
    el(id).disabled = !connected || pending || !context.availableDisplayModes?.includes(mode);
  }
  el('native').disabled = !connected || pending;
  if (connectionIssue) el('host-say').textContent = connectionMessage(connectionIssue);
  else if (message) el('host-say').textContent = message;
  el('host-evidence').textContent = raw({ ...evidence, advertised: context.availableDisplayModes,
    current: context.displayMode, response: response ?? t('app.response_none'), event: eventMode ?? t('app.event_none'),
    automatic: initialDisplay.attempts });
}

el('layout').onclick = () => {
  layoutChoice = graphOrientation(context.displayMode, layoutChoice) === 'horizontal' ? 'vertical' : 'horizontal';
  modes();
};

app.onhostcontextchanged = (change) => {
  context = { ...context, ...change };
  if ('displayMode' in change) { eventMode = change.displayMode; initialDisplay.observe(change.displayMode); }
  if ('theme' in change || 'styles' in change) adoptHostStyles();
  if ('locale' in change && adoptLocale(context) && rendererReady && !stopped) { refreshOnReturn = true; void poll(); }
  modes();
  maybeFullscreen();
  void reportPresence();
};
let retryTimer;
async function requestMode(mode, { automatic = false } = {}) {
  if (stopped || pending || !connected || !context.availableDisplayModes?.includes(mode)) return;
  pending = true; modes(t('app.requesting'));
  let got, failure = null;
  try {
    got = await app.requestDisplayMode({ mode }, { timeout: 10000 });
    if (stopped) return;
    response = raw(got);
    // Response is the applied mode; events are recorded separately. No timer-based guess.
    if (got?.mode) context = { ...context, displayMode: got.mode };
    modes(got?.mode === 'fullscreen' ? t('app.fullscreen_applied')
      : got?.mode === 'inline' ? t('app.inline_returned')
      : t('app.no_mode'));
  } catch (error) {
    failure = fullscreenFailure(error);
    if (!stopped) {
      response = failure === 'refused' ? t('app.response_refused') : t('app.response_error');
      modes(failure === 'refused' ? t('app.refused_click') : t('app.request_unconfirmed'));
    }
  } finally {
    pending = false;
    if (!stopped) {
      if (automatic) scheduleRetry(got?.mode ?? null, failure);
      modes(); void reportPresence();
      // A connect-time request may have been answered after the Monitor rendered: the
      // ready-time request then follows here instead of waiting for another Host event.
      if (automatic) maybeFullscreen();
    }
  }
}
// Automatic requests only. A manual choice cancels the intent, so this never fights the user.
function scheduleRetry(appliedMode, failure = null) {
  const refused = failure === 'refused';
  clearTimeout(retryTimer);
  const delay = initialDisplay.settle(appliedMode, { refused, failed: failure === 'error' });
  if (delay !== null) {
    retryTimer = setTimeout(() => { retryTimer = undefined; maybeFullscreen(); }, delay);
  } else if (initialDisplay.awaitingGesture) {
    modes(refused ? t('app.refused_click') : failure === 'error' ? t('app.auto_failed') : t('app.auto_declined'));
  }
}
function maybeFullscreen() {
  if (stopped || suspended || document.hidden || pending) return;
  if (initialDisplay.retrying && context.displayMode !== 'fullscreen' && !retryTimer) {
    if (!context.availableDisplayModes?.includes('fullscreen')) return;
    const mode = initialDisplay.retry();
    if (mode) void requestMode(mode, { automatic: true });
    return;
  }
  const mode = initialDisplay.next({ connected, ready: rendererReady, context });
  if (mode) void requestMode(mode, { automatic: true });
}
document.addEventListener('pointerdown', (event) => {
  // Explicit controls own their gesture; never preempt their click with fullscreen.
  if (event.target?.closest?.('button, a, input, select, textarea, [role="button"], [contenteditable]')) return;
  // Logged like the automatic attempts: the gesture's answer is evidence too.
  if (initialDisplay.gesture() === 'fullscreen') void requestMode('fullscreen', { automatic: true });
}, true);
for (const [id, mode] of [['expand', 'fullscreen'], ['inline', 'inline']]) {
  el(id).onclick = async () => {
    initialDisplay.cancel(); clearTimeout(retryTimer); retryTimer = undefined;
    await requestMode(mode);
  };
}
// Update guidance: the Core's metadata becomes one button and a text panel. Opening the panel
// runs nothing. Mac copies a request; Windows can invoke the existing "open Companion" tool.
let updateInfo = null, updateKey = null;
function renderUpdate(update) {
  if (!update || typeof update !== 'object') return;
  const key = JSON.stringify([update.state, update.platform, update.latest, update.installed, update.error]);
  if (key === updateKey) return;
  updateKey = key; updateInfo = update;
  evidence.update = { state: update.state, installed: update.installed ?? null, latest: update.latest?.version ?? null,
    checked_at: update.checked_at ?? null, error: update.error ?? null };
  const notice = updateNotice(update);
  const button = el('update');
  button.hidden = !notice.show;
  if (notice.show) button.textContent = notice.label;
  // New information collapses an open guide, so it is re-read from the current metadata.
  el('update-guide').hidden = true;
  if (notice.line && notice.show) modes(notice.line); else modes();
}
el('update').onclick = () => {
  const panel = el('update-guide');
  if (panel.hidden === false) { panel.hidden = true; return; }
  const guide = updateGuide(updateInfo);
  panel.replaceChildren();
  const h = document.createElement('h2'); h.textContent = guide.title; panel.appendChild(h);
  if (guide.summary) { const p = document.createElement('p'); p.textContent = guide.summary; panel.appendChild(p); }
  const list = document.createElement('ol');
  for (const step of guide.steps) { const li = document.createElement('li'); li.textContent = step; list.appendChild(li); }
  panel.appendChild(list);
  if (guide.targets) {
    const label = document.createElement('label'); label.textContent = t('app.update_target_label');
    const select = document.createElement('select');
    guide.targets.forEach((target, index) => {
      const option = document.createElement('option'); option.value = String(index); option.textContent = target.label;
      select.appendChild(option);
    });
    select.value = '0'; label.appendChild(select); panel.appendChild(label);
    const hint = document.createElement('p'); panel.appendChild(hint);
    const requestLabel = document.createElement('label'); requestLabel.textContent = t('app.request_label');
    const request = document.createElement('textarea'); request.readOnly = true; request.rows = 7;
    requestLabel.appendChild(request); panel.appendChild(requestLabel);
    const copy = document.createElement('button'); copy.type = 'button'; copy.textContent = t('app.copy');
    const status = document.createElement('p'); status.setAttribute('role', 'status');
    const choose = () => { hint.textContent = guide.targets[Number(select.value)].hint;
      request.value = guide.targets[Number(select.value)].request; status.textContent = ''; };
    select.onchange = choose; choose();
    copy.onclick = async () => {
      const value = request.value;
      try {
        await navigator.clipboard.writeText(value);
        if (value === request.value) status.textContent = t('app.copied');
      } catch {
        request.focus(); request.select();
        status.textContent = t('app.copy_failed');
      }
    };
    panel.appendChild(copy); panel.appendChild(status);
    const details = document.createElement('details');
    const summary = document.createElement('summary'); summary.textContent = t('app.release_details'); details.appendChild(summary);
    for (const line of guide.details) { const p = document.createElement('p'); p.textContent = line; details.appendChild(p); }
    panel.appendChild(details);
  }
  if (guide.companion) {
    const open = document.createElement('button'); open.type = 'button'; open.textContent = t('app.open_companion');
    open.onclick = () => el('native').onclick();
    panel.appendChild(open);
  }
  const small = document.createElement('small');
  small.textContent = guide.targets ? t('app.copy_note') : t('app.guide_note');
  panel.appendChild(small);
  panel.hidden = false;
};

el('native').onclick = async () => {
  initialDisplay.cancel(); clearTimeout(retryTimer); retryTimer = undefined;
  pending = true; modes(t('app.native_requesting'));
  try {
    const data = unwrap(await app.callServerTool({ name: 'show_gil_companion', arguments: {} }));
    modes(t('app.native_note', { said: data.said }));
  } catch (err) { modes(err.said || t('app.native_failed')); }
  finally { pending = false; modes(); }
};

async function poll() {
  if (stopped || suspended || document.hidden || !connected || !rendererReady || polling) return;
  clearTimeout(timer);
  polling = true;
  const refresh = refreshOnReturn;
  refreshOnReturn = false;
  try {
    // Resume uses the same scope-bound Host, not a new prepare/render call.
    if (refresh) await window.GIL_HOST.refresh();
    else await window.GIL_HOST.tick();
    // State-file hints drive approval refreshes too. Quiet polling never loads a Project every 2s.
    if (planPolling) await planPolling.tick();
    if (recoveryReview && Date.now() - recoveryReadAt >= 30000) { await recoveryReview.refresh(); recoveryReadAt = Date.now(); }
    pollFailures = 0;
  } catch (err) {
    // A restarted server restores the saved binding itself. All transport failures
    // use bounded 2..60s retry; a hidden App does not keep scheduling timers.
    pollFailures += 1;
    if (!stopped) { connectionIssue = err; modes(); }
  } finally {
    polling = false;
    if (!stopped && !suspended && !document.hidden) {
      // Several returns during one request coalesce into one full refresh after it.
      if (refreshOnReturn) void poll();
      else timer = setTimeout(poll, pollFailures ? retryAfter(pollFailures) : 2000);
    }
  }
}

app.ontoolresult = async (result) => {
  if (booted || stopped) return;
  try {
    const data = unwrap(result);
    if (!/^project:[0-9a-f]{64}$/.test(data.scope_id) || !data.view) throw new Error('scope');
    booted = true;
    if (data.plan_review_supported) planReview = makePlanReview({ app, scope: data.scope_id,
      panel: el('plan-review'), banner: el('plan-banner'), document, changed: () => { refreshOnReturn = true; } });
    if (planReview) planPolling = makePlanPolling(planReview);
    window.GIL_HOST = makeHost(app, data, { presence });
    window.GIL_HOST.listen('gil://update', renderUpdate);
    if (data.revisit_review_supported) {
      recoveryReview = makeRecoveryReview({app, scope:data.scope_id, panel:el('revisit-review'), document, changed:() => { refreshOnReturn = true; planPolling?.request(); }});
      window.GIL_HOST.listen('gil://refresh', () => { recoveryReadAt = 0; });
    }
    if (planReview) window.GIL_HOST.listen('gil://refresh', () => { planPolling?.request(); });
    void reportPresence();
    window.GIL_HOST.listen('gil://connection', (error) => {
      connectionIssue = error;
      modes(error ? undefined : t('app.connection_rechecked'));
    });
    // The language is chosen before the shared renderer draws its first label.
    adoptLocale(context);
    // One shared renderer. The native and fixture entrypoints use this exact module too.
    await import('../ui/companion.js');
    if (stopped) return;
    window.GIL_COMPANION.setOrientation(graphOrientation(context.displayMode, layoutChoice));
    await window.GIL_COMPANION.ready;
    if (stopped) return;
    rendererReady = true;
    modes();
    if (!data.watching) modes(t('app.watch_failed'));
    renderUpdate(data.update);
    poll();
    maybeFullscreen();
  } catch (err) { renderFailed = true; void reportPresence(); modes(err?.said || t('app.data_failed')); }
};
function stop() { stopped = true; planReview?.stop(); recoveryReview?.stop(); initialDisplay.cancel(); clearTimeout(timer); clearTimeout(retryTimer); window.GIL_COMPANION?.clearVisuals?.(); return reportPresence(); }
function resume() {
  if (stopped || suspended || document.hidden) return;
  refreshOnReturn = true;
  void poll();
  maybeFullscreen();
}
app.onteardown = async () => { await stop(); return {}; };
// A cached page is suspended, not destroyed. Only true unload/teardown is terminal.
window.addEventListener('pagehide', (event) => {
  if (!event.persisted) { stop(); return; }
  suspended = true; clearTimeout(timer); clearTimeout(retryTimer); retryTimer = undefined; void reportPresence();
});
window.addEventListener('pageshow', (event) => {
  if (!event.persisted || !suspended || stopped) return;
  suspended = false; resume();
});
document.addEventListener('visibilitychange', () => {
  if (document.hidden) { clearTimeout(timer); clearTimeout(retryTimer); retryTimer = undefined; void reportPresence(); }
  else resume();
});
app.connect().then(() => {
  if (stopped) return;
  connected = true; context = { ...app.getHostContext(), ...context };
  evidence.host = hostIdentity(app);
  adoptHostStyles();
  if (adoptLocale(context) && rendererReady) refreshOnReturn = true;
  modes(context.availableDisplayModes?.includes('fullscreen')
    ? t('app.connect_auto')
    : Array.isArray(context.availableDisplayModes)
      ? t('app.connect_no_fullscreen')
      : t('app.connect_waiting'));
  void poll();
  maybeFullscreen();
}).catch(() => modes(t('app.connect_failed')));
