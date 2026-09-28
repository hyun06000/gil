import { App } from '@modelcontextprotocol/ext-apps';
import { makeHost, unwrap, retryAfter, connectionMessage } from './host.mjs';
import { graphOrientation, initialFullscreen } from './presentation.mjs';

const app = new App({ name: 'GIL Monitor', version: '0.1.0' },
  { availableDisplayModes: ['inline', 'fullscreen'] });
const el = (id) => document.getElementById(id);
const raw = (v) => v === undefined ? 'undefined' : JSON.stringify(v, null, 2);
let context = {}, connected = false, pending = false, booted = false, stopped = false;
let rendererReady = false;
let response = '요청 전', eventMode = '이벤트 전', timer, pollFailures = 0;
let suspended = false, polling = false, refreshOnReturn = false;
let connectionIssue = null;
let layoutChoice = null;
const evidence = { declared: ['inline', 'fullscreen'], instance: crypto.randomUUID() };
const initialDisplay = initialFullscreen();

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
  if ('displayMode' in change) eventMode = change.displayMode;
  modes();
  maybeFullscreen();
};
async function requestMode(mode) {
  if (stopped || pending || !connected || !context.availableDisplayModes?.includes(mode)) return;
  pending = true; modes('Host 에 표시 전환을 요청하는 중…');
  try {
    const got = await app.requestDisplayMode({ mode }, { timeout: 10000 });
    if (stopped) return;
    response = raw(got);
    // Response is the applied mode; events are recorded separately. No timer-based guess.
    if (got?.mode) context = { ...context, displayMode: got.mode };
    modes(got?.mode === 'fullscreen' ? '펼친 Monitor 입니다. Host 의 채팅 입력창에서 대화를 이어가세요.'
      : got?.mode === 'inline' ? '대화 안의 미리보기입니다. 계속 보려면 모니터 펼치기를 눌러 주세요.'
      : 'Host 가 적용 모드를 반환하지 않았습니다. 화면이 그대로라면 모니터 펼치기를 눌러 주세요.');
  } catch {
    if (!stopped) { response = '요청 오류'; modes('전환을 확인하지 못했습니다. 모니터 펼치기를 눌러 다시 요청할 수 있습니다.'); }
  } finally { pending = false; if (!stopped) modes(); }
}
function maybeFullscreen() {
  if (stopped || suspended || document.hidden || pending) return;
  const mode = initialDisplay.next({ connected, ready: rendererReady, context });
  if (mode) void requestMode(mode);
}
for (const [id, mode] of [['expand', 'fullscreen'], ['inline', 'inline']]) {
  el(id).onclick = async () => {
    initialDisplay.cancel();
    await requestMode(mode);
  };
}
el('native').onclick = async () => {
  initialDisplay.cancel();
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
    window.GIL_HOST = makeHost(app, data);
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
    poll();
    maybeFullscreen();
  } catch (err) { modes(err?.said || 'Monitor 데이터를 받지 못했습니다. Project 를 지정해 다시 열어 주세요.'); }
};
function stop() { stopped = true; initialDisplay.cancel(); clearTimeout(timer); }
function resume() {
  if (stopped || suspended || document.hidden) return;
  refreshOnReturn = true;
  void poll();
  maybeFullscreen();
}
app.onteardown = async () => { stop(); return {}; };
// A cached page is suspended, not destroyed. Only true unload/teardown is terminal.
window.addEventListener('pagehide', (event) => {
  if (!event.persisted) { stop(); return; }
  suspended = true; clearTimeout(timer);
});
window.addEventListener('pageshow', (event) => {
  if (!event.persisted || !suspended || stopped) return;
  suspended = false; resume();
});
document.addEventListener('visibilitychange', () => {
  if (document.hidden) clearTimeout(timer);
  else resume();
});
app.connect().then(() => {
  if (stopped) return;
  connected = true; context = { ...app.getHostContext(), ...context };
  modes(context.availableDisplayModes?.includes('fullscreen')
    ? 'Monitor 가 준비되면 전체 화면을 자동 요청합니다. 그대로라면 모니터 펼치기를 눌러 주세요.'
    : Array.isArray(context.availableDisplayModes)
      ? '이 표면은 펼친 Monitor 를 지원하지 않습니다. Codex 또는 Claude Desktop Cowork 에서 다시 열거나 별도 창을 사용하세요.'
      : 'Host 의 표시 모드 지원 정보를 기다리고 있습니다.');
  void poll();
  maybeFullscreen();
}).catch(() => modes('MCP Apps 연결에 실패했습니다. Agent 에게 별도 GIL Monitor 창을 요청하세요.'));
