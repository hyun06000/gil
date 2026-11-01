import { App, applyDocumentTheme, applyHostStyleVariables, applyHostFonts } from '@modelcontextprotocol/ext-apps';
import { makePlanReview } from './plans.mjs';
import { makeHost, unwrap, retryAfter, connectionMessage } from './host.mjs';
import { graphOrientation, initialFullscreen, hostIdentity, hostStyles, styleEvidence, updateNotice, updateGuide } from './presentation.mjs';

const app = new App({ name: 'GIL Monitor', version: '0.1.0' },
  { availableDisplayModes: ['inline', 'fullscreen'] });
const el = (id) => document.getElementById(id);
const raw = (v) => v === undefined ? 'undefined' : JSON.stringify(v, null, 2);
let context = {}, connected = false, pending = false, booted = false, stopped = false;
let rendererReady = false, planReview = null, planNeedsRefresh = true, planReadAt = 0;
let response = '요청 전', eventMode = '이벤트 전', timer, pollFailures = 0;
let suspended = false, polling = false, refreshOnReturn = false;
let connectionIssue = null;
let layoutChoice = null;
let presenceSequence = 0, renderFailed = false;
// Host identity is recorded as reported, never inferred; policy does not branch on it.
const evidence = { declared: ['inline', 'fullscreen'], instance: crypto.randomUUID(), host: null, styles: null };
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

// Report only this rendered App's observations. Host display mode is not OS occlusion proof.
function presence() {
  const presentation = stopped ? 'closed' : suspended || document.hidden ? 'hidden'
    : renderFailed ? 'error' : !rendererReady ? 'loading'
    : ['inline', 'fullscreen'].includes(context.displayMode) ? context.displayMode : 'loading';
  return { instance_id: evidence.instance, sequence: String(++presenceSequence), presentation };
}
function reportPresence() {
  if (!connected || !window.GIL_HOST) return Promise.resolve();
  return window.GIL_HOST.reportPresence().catch(() => {});
}

function modes(message) {
  document.body.dataset.displayMode = context.displayMode || 'unknown';
  const direction = graphOrientation(context.displayMode, layoutChoice);
  window.GIL_COMPANION?.setOrientation(direction);
  el('layout').textContent = direction === 'horizontal' ? '세로로 보기' : '가로로 보기';
  el('layout').disabled = !rendererReady;
  for (const [id, mode] of [['expand', 'fullscreen'], ['inline', 'inline']]) {
    el(id).disabled = !connected || pending || !context.availableDisplayModes?.includes(mode);
  }
  el('native').disabled = !connected || pending;
  if (connectionIssue) el('host-say').textContent = connectionMessage(connectionIssue);
  else if (message) el('host-say').textContent = message;
  el('host-evidence').textContent = raw({ ...evidence, advertised: context.availableDisplayModes,
    current: context.displayMode, response, event: eventMode });
}

el('layout').onclick = () => {
  layoutChoice = graphOrientation(context.displayMode, layoutChoice) === 'horizontal' ? 'vertical' : 'horizontal';
  modes();
};

app.onhostcontextchanged = (change) => {
  context = { ...context, ...change };
  if ('displayMode' in change) { eventMode = change.displayMode; initialDisplay.observe(change.displayMode); }
  if ('theme' in change || 'styles' in change) adoptHostStyles();
  modes();
  maybeFullscreen();
  void reportPresence();
};
let retryTimer;
async function requestMode(mode, { automatic = false } = {}) {
  if (stopped || pending || !connected || !context.availableDisplayModes?.includes(mode)) return;
  pending = true; modes('Host 에 표시 전환을 요청하는 중…');
  let got;
  try {
    got = await app.requestDisplayMode({ mode }, { timeout: 10000 });
    if (stopped) return;
    response = raw(got);
    // Response is the applied mode; events are recorded separately. No timer-based guess.
    if (got?.mode) context = { ...context, displayMode: got.mode };
    modes(got?.mode === 'fullscreen' ? '펼친 Monitor 입니다. Host 의 채팅 입력창에서 대화를 이어가세요.'
      : got?.mode === 'inline' ? '대화 안의 미리보기입니다. 계속 보려면 모니터 펼치기를 눌러 주세요.'
      : 'Host 가 적용 모드를 반환하지 않았습니다. 화면이 그대로라면 모니터 펼치기를 눌러 주세요.');
  } catch {
    if (!stopped) { response = '요청 오류'; modes('전환을 확인하지 못했습니다. 모니터 펼치기를 눌러 다시 요청할 수 있습니다.'); }
  } finally {
    pending = false;
    if (!stopped) {
      if (automatic) scheduleRetry(got?.mode ?? null);
      modes(); void reportPresence();
    }
  }
}
// Automatic requests only. A manual choice cancels the intent, so this never fights the user.
function scheduleRetry(appliedMode) {
  clearTimeout(retryTimer);
  const delay = initialDisplay.settle(appliedMode);
  if (delay !== null) {
    retryTimer = setTimeout(() => { retryTimer = undefined; maybeFullscreen(); }, delay);
  } else if (initialDisplay.awaitingGesture) {
    modes('대화 안의 미리보기입니다. Monitor 를 한 번 클릭하면 펼칩니다. 모니터 펼치기도 사용할 수 있습니다.');
  }
}
function maybeFullscreen() {
  if (stopped || suspended || document.hidden || pending) return;
  if (initialDisplay.retrying && context.displayMode !== 'fullscreen' && !retryTimer) {
    void requestMode('fullscreen', { automatic: true });
    return;
  }
  const mode = initialDisplay.next({ connected, ready: rendererReady, context });
  if (mode) void requestMode(mode, { automatic: true });
}
document.addEventListener('pointerdown', (event) => {
  // Explicit controls own their gesture; never preempt their click with fullscreen.
  if (event.target?.closest?.('button, a, input, select, textarea, [role="button"], [contenteditable]')) return;
  if (initialDisplay.gesture() === 'fullscreen') void requestMode('fullscreen');
}, true);
for (const [id, mode] of [['expand', 'fullscreen'], ['inline', 'inline']]) {
  el(id).onclick = async () => {
    initialDisplay.cancel(); clearTimeout(retryTimer); retryTimer = undefined;
    await requestMode(mode);
  };
}
// Update guidance: the Core's metadata becomes one button and a text panel. Opening the panel
// runs nothing; the only action inside is the existing "open Companion" tool on Windows.
let updateInfo = null, updateKey = null;
function renderUpdate(update) {
  if (!update || typeof update !== 'object') return;
  const key = JSON.stringify([update.state, update.latest?.version, update.installed, update.error]);
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
  const list = document.createElement('ol');
  for (const step of guide.steps) { const li = document.createElement('li'); li.textContent = step; list.appendChild(li); }
  panel.appendChild(list);
  if (guide.companion) {
    const open = document.createElement('button'); open.type = 'button'; open.textContent = 'Companion 열기';
    open.onclick = () => el('native').onclick();
    panel.appendChild(open);
  }
  const small = document.createElement('small');
  small.textContent = '이 화면은 안내만 합니다. 다운로드·설치는 사용자가 Companion 또는 설치 안내의 절차로 직접 진행합니다. 확인을 끄려면 Host 의 서버 환경에 GIL_UPDATE_CHECK=off 를 두거나 Monitor 상태 폴더에 update-check.off 파일을 둡니다.';
  panel.appendChild(small);
  panel.hidden = false;
};

el('native').onclick = async () => {
  initialDisplay.cancel(); clearTimeout(retryTimer); retryTimer = undefined;
  pending = true; modes('별도 Monitor 창을 요청하는 중…');
  try {
    const data = unwrap(await app.callServerTool({ name: 'show_gil_companion', arguments: {} }));
    modes(data.said + ' 별도 창의 Project 선택은 이 화면과 독립입니다.');
  } catch (err) { modes(err.said || '별도 창을 열지 못했습니다.'); }
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
    if (planReview && (planNeedsRefresh || Date.now() - planReadAt >= 30000)) {
      await planReview.refresh(); planNeedsRefresh = false; planReadAt = Date.now();
    }
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
    window.GIL_HOST = makeHost(app, data, { presence });
    window.GIL_HOST.listen('gil://update', renderUpdate);
    if (planReview) window.GIL_HOST.listen('gil://refresh', () => { planNeedsRefresh = true; });
    void reportPresence();
    window.GIL_HOST.listen('gil://connection', (error) => {
      connectionIssue = error;
      modes(error ? undefined : '프로젝트 연결을 다시 확인했습니다.');
    });
    // One shared renderer. The native and fixture entrypoints use this exact module too.
    await import('../ui/companion.js');
    if (stopped) return;
    window.GIL_COMPANION.setOrientation(graphOrientation(context.displayMode, layoutChoice));
    await window.GIL_COMPANION.ready;
    if (stopped) return;
    rendererReady = true;
    modes();
    if (!data.watching) modes('변화 감시를 시작하지 못했습니다. 수동 새로고침과 5분 재조회는 사용할 수 있습니다.');
    renderUpdate(data.update);
    poll();
    maybeFullscreen();
  } catch (err) { renderFailed = true; void reportPresence(); modes(err?.said || 'Monitor 데이터를 받지 못했습니다. Project 를 지정해 다시 열어 주세요.'); }
};
function stop() { stopped = true; planReview?.stop(); initialDisplay.cancel(); clearTimeout(timer); clearTimeout(retryTimer); window.GIL_COMPANION?.clearVisuals?.(); return reportPresence(); }
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
  modes(context.availableDisplayModes?.includes('fullscreen')
    ? 'Monitor 가 준비되면 전체 화면을 자동 요청합니다. 그대로라면 모니터 펼치기를 눌러 주세요.'
    : Array.isArray(context.availableDisplayModes)
      ? '이 표면은 펼친 Monitor 를 지원하지 않습니다. fullscreen 을 지원하는 Host(Codex, Claude Desktop 의 Claude Code 탭)에서 다시 열거나 별도 창을 사용하세요.'
      : 'Host 의 표시 모드 지원 정보를 기다리고 있습니다.');
  void poll();
  maybeFullscreen();
}).catch(() => modes('MCP Apps 연결에 실패했습니다. Agent 에게 별도 GIL Monitor 창을 요청하세요.'));
