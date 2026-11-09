// Layout is independent of the Host's applied display mode.
export function graphOrientation(mode, choice = null) {
  if (choice === 'horizontal' || choice === 'vertical') return choice;
  return 'horizontal';
}

// One initial intent per App instance, not a rule that forces every mode event.
// The same sequence goes to every Host; nothing here reads the Host's name. Each automatic
// attempt is logged (phase, time since the App was created, the Host's applied mode) so the
// 연결 정보 panel can show whether the automatic requests were actually sent and what the Host
// answered. Phases, in order:
//   connect — the Host advertised fullscreen before the Monitor rendered: ask once at once
//             (some Hosts apply a request only while they are still mounting the card).
//   ready   — the Monitor rendered: ask once (this is the request Codex applies).
//   retry   — the Host left the App inline without the user choosing so: a few bounded retries.
//   gesture — the user's first click anywhere in the App carries one last request (some Hosts
//             apply display-mode changes only inside a user action).
// Any manual choice, a Host-confirmed fullscreen, or teardown ends the intent for good.
export const RETRY_DELAYS_MS = [1500, 4000];
// A Host may explicitly require a fresh press inside the App's own frame. Timer retries cannot
// satisfy that, so a refusal arms the gesture at once, and a refused gesture re-arms it a few
// times: the user's natural act is to click the card. An applied `inline` answer keeps the
// bounded timer retries and one gesture, as before.
export const GESTURE_ATTEMPTS = 3;
// What the App reports to the Core about its automatic fullscreen intent (gil_monitor_poll
// `automatic_fullscreen`). Observations only; the Core never requests fullscreen itself.
export const AUTOMATIC_FULLSCREEN = ['pending', 'applied', 'declined', 'refused', 'error', 'unavailable', 'manual'];
// Only an explicit gesture-required response is evidence of that policy. A timeout,
// disconnected transport or unknown exception cannot establish why the Host stayed inline.
// Recognize the observed message, not a Host name; new/unknown wording stays an error.
export function fullscreenFailure(error) {
  if (error?.code === -32000 || error?.code === -32001) return 'error'; // MCP transport errors
  const message = error?.message;
  return typeof message === 'string' && message.length <= 1000 &&
    message.includes("fullscreen request refused: it needs a fresh press in this app's own frame")
    ? 'refused' : 'error';
}
export function initialFullscreen({ retries = RETRY_DELAYS_MS, gestures = GESTURE_ATTEMPTS, now = () => Date.now() } = {}) {
  let early = false, consumed = false, done = false, attempts = 0, gestureArmed = false;
  let gesturesUsed = 0, manual = false, lastAnswer = null;
  const born = now(), log = [];
  let inflight = null;
  const finish = () => { done = true; gestureArmed = false; };
  const begin = (phase) => { inflight = { phase, at_ms: Math.max(0, now() - born), applied: undefined }; log.push(inflight); return 'fullscreen'; };
  return {
    // An explicit user choice (expand, inline or the native window) ends the automatic intent.
    cancel() { manual = true; finish(); },
    next({ connected, ready, context }) {
      if (done || !connected) return null;
      if (context.displayMode === 'fullscreen') { finish(); consumed = true; return null; }
      if (!Array.isArray(context.availableDisplayModes)) return null;
      // Capability updates may arrive after the initial inline-only context. Wait without
      // sending unsupported requests; only an explicit choice or applied fullscreen cancels.
      if (!context.availableDisplayModes.includes('fullscreen')) return null;
      if (!ready) { if (early || inflight) return null; early = true; return begin('connect'); }
      if (consumed || inflight) return null;
      consumed = true; attempts = 1;
      return begin('ready');
    },
    // The retry the App scheduled after `settle` returned a delay.
    retry() {
      if (done || !consumed || gestureArmed || inflight) return null;
      return begin('retry');
    },
    // Called with the mode the Host applied (or null when it gave none); `refused` marks a
    // request explicitly rejected for needing a fresh press; `failed` is an unknown/transport error.
    // Returns the delay before another automatic
    // request, or null when no timer is due now.
    settle(appliedMode, { refused = false, failed = false } = {}) {
      const phase = inflight?.phase;
      if (inflight) { inflight.applied = appliedMode ?? null; if (refused) inflight.refused = true; if (failed) inflight.failed = true; inflight = null; }
      lastAnswer = refused ? 'refused' : failed ? 'error' : appliedMode ?? 'error';
      if (done) return null;
      if (appliedMode === 'fullscreen') { finish(); return null; }
      // A declined connect-time request is not a retry: the ready-time request follows by itself.
      if (!consumed) return null;
      if (phase === 'gesture') {
        // Only a refusal re-arms: a Host that applied inline to a user press made its decision.
        if (refused && gesturesUsed < gestures) gestureArmed = true; else finish();
        return null;
      }
      if (!refused && attempts - 1 < retries.length) { const delay = retries[attempts - 1]; attempts += 1; return delay; }
      gestureArmed = true;
      return null;
    },
    // A Host-reported fullscreen (via a context event) also ends the intent.
    observe(mode) { if (mode === 'fullscreen') finish(); },
    // The user's interaction anywhere in the App may carry the remaining request(s).
    gesture() {
      if (!gestureArmed || done) return null;
      gestureArmed = false; gesturesUsed += 1;
      return begin('gesture');
    },
    get retrying() { return consumed && !done && !gestureArmed; },
    get awaitingGesture() { return gestureArmed && !done; },
    /** Evidence only: copies of every attempt so far. */
    get attempts() { return log.map((entry) => ({ ...entry })); },
    /** One word for the Core: the state of the automatic intent given the Host context. */
    summary(context = {}) {
      if (manual) return 'manual';
      if (context.displayMode === 'fullscreen') return 'applied';
      if (Array.isArray(context.availableDisplayModes) && !context.availableDisplayModes.includes('fullscreen')) return 'unavailable';
      if (lastAnswer === null) return 'pending';
      if (lastAnswer === 'fullscreen') return 'applied';
      return ['refused', 'error'].includes(lastAnswer) ? lastAnswer : 'declined';
    },
  };
}

// The Host's self-reported implementation, copied as evidence only. No display policy
// branches on it: Codex, Claude Desktop (Cowork/Code) and others get the same requests.
export function hostIdentity(app) {
  try {
    const got = typeof app?.getHostVersion === 'function' ? app.getHostVersion() : undefined;
    if (!got || typeof got !== 'object') return null;
    const text = (value) => (typeof value === 'string' && value.length <= 200 ? value : null);
    const identity = { name: text(got.name), version: text(got.version) };
    if (text(got.title)) identity.title = text(got.title);
    return identity;
  } catch { return null; }
}

// Host theme adoption. The App copies what the Host reports through the MCP Apps
// standard (theme, styles.variables, styles.css.fonts) and never infers a theme from the
// Host's name. Variable names and values are shape-checked only; policy does not read them.
const VARIABLE_NAME = /^--[a-z][a-z0-9-]{0,60}$/;
const VARIABLE_VALUE_MAX = 400;
const FONTS_CSS_MAX = 65536;
export function hostStyles(context) {
  const theme = context?.theme === 'light' || context?.theme === 'dark' ? context.theme : null;
  const given = context?.styles?.variables;
  let variables = null;
  if (given && typeof given === 'object' && !Array.isArray(given)) {
    variables = {};
    for (const [name, value] of Object.entries(given)) {
      if (VARIABLE_NAME.test(name) && typeof value === 'string' && value.length > 0 &&
          value.length <= VARIABLE_VALUE_MAX && !/[;{}<>]/.test(value)) variables[name] = value;
    }
    if (Object.keys(variables).length === 0) variables = null;
  }
  const css = context?.styles?.css?.fonts;
  const fonts = typeof css === 'string' && css.length > 0 && css.length <= FONTS_CSS_MAX &&
    !/<\/style/i.test(css) ? css : null;
  return { theme, variables, fonts };
}
/** What the evidence block records: presence and counts, never the values themselves. */
export function styleEvidence(context) {
  const got = hostStyles(context);
  return { theme: got.theme, variables: got.variables ? Object.keys(got.variables).length : 0,
    fonts: got.fonts !== null };
}

// Update guidance is metadata beside the Graph. The Core reports what it knows (installed
// version, the feed's latest entry, a state); the App turns that into one line, one button
// and OS-specific steps. Nothing here downloads, installs or changes a Host registration.
export function updateNotice(update) {
  if (!update || typeof update !== 'object') return { show: false, line: null };
  const latest = update.latest && typeof update.latest === 'object' ? update.latest : null;
  const version = typeof latest?.version === 'string' ? latest.version : null;
  switch (update.state) {
    case 'available':
      return { show: true, state: 'available', version, latest,
        label: `새 버전 ${version} 있음 · 업데이트 안내`,
        line: `설치된 GIL ${update.installed ?? '?'} → 새 버전 ${version} 이 있습니다. 업데이트 안내를 눌러 진행 방법을 확인하세요.` };
    case 'unknown_installed':
      return { show: true, state: 'unknown_installed', version, latest,
        label: `최신 ${version} · 업데이트 안내`,
        line: `최신 버전은 ${version} 입니다. 설치된 버전은 이 Core 가 확인하지 못했습니다.` };
    case 'up_to_date':
      return { show: false, state: 'up_to_date', version, latest, line: `GIL ${update.installed} 은 최신 버전입니다.` };
    case 'disabled':
      return { show: false, state: 'disabled', line: '업데이트 확인이 꺼져 있습니다.' };
    case 'checking':
      return { show: false, state: 'checking', line: '업데이트 정보를 확인하는 중입니다.' };
    case 'not_installed':
      return { show: false, state: 'not_installed', line: null };
    default:
      return { show: false, state: update.state ?? 'unavailable',
        line: update.error ? `업데이트 정보를 읽지 못했습니다 (${update.error}).` : null };
  }
}

// Steps per OS. Both Hosts are listed, because the Core does not know which one is showing it
// and the App never branches on the Host's name. Text only; no commands are executed.
export function updateGuide(update) {
  const latest = update?.latest ?? {};
  const version = latest.version ?? '?';
  const common = [
    `새 버전: ${version}${update?.installed ? ` (설치됨: ${update.installed})` : ''}`,
    `릴리스: ${latest.release_url ?? ''}`,
    `설치 안내: ${latest.install_doc ?? ''}`,
    latest.notes ? `변경: ${latest.notes}` : null,
    '열린 대화는 지금 Core 를 끝까지 쓰고, 새 대화부터 새 버전이 적용됩니다. Project 와 .gil 기록은 그대로 둡니다.',
  ].filter(Boolean);
  if (update?.platform?.startsWith('windows')) {
    return { title: 'Windows — GIL Companion 에서 업데이트', companion: true, steps: common.concat([
      '1. GIL Companion 을 엽니다(아래 "Companion 열기" 또는 시작 메뉴).',
      '2. 업데이트 확인 → 내용을 읽고 업데이트 를 누릅니다. 설치 창이 열립니다.',
      '3. Codex·Claude Desktop 은 미리 닫지 않아도 됩니다. 두 Host 모두 같은 고정 런처를 쓰므로 다시 연결할 필요가 없습니다.',
      '4. 새 대화에서 Monitor 연결 정보의 GIL 버전이 바뀌었는지 확인합니다.',
    ]) };
  }
  return { title: 'Mac에서 GIL 업데이트하기', companion: false,
    summary: `새 버전 ${version}${update?.installed ? ` · 현재 ${update.installed}` : ''}`,
    details: common.slice(1, -1),
    steps: [
      '아래에서 GIL을 업데이트할 앱을 선택하세요.',
      '요청문을 복사해 내 Mac의 파일을 다룰 수 있는 에이전트의 새 대화에 붙여넣으세요. 다운로드와 설정 변경은 에이전트가 안내에 따라 처리합니다.',
      '에이전트가 완료했다고 알려주면 업데이트한 앱을 완전히 종료했다가 다시 여세요. 새 대화에서 GIL Monitor를 열고 연결 정보의 버전을 확인하세요.',
    ], targets: ['Codex', 'Claude Desktop', 'Codex와 Claude Desktop'].map(target => ({
      label: target,
      hint: target === 'Codex'
        ? 'Codex의 새 대화에 붙여넣으세요. 업데이트가 끝난 뒤 Codex를 다시 여세요.'
        : 'Claude 안에서 진행하면 앱 종료 뒤 설정 변경을 마칠 수 없습니다. 먼저 Codex 등 Claude 밖의 로컬 에이전트에 요청문을 전달하세요. 종료 요청을 받으면 작업을 저장하고 Claude 메뉴 → Claude 종료(⌘Q)를 누르세요. 창만 닫으면 종료되지 않습니다.',
      request: `다음 공식 설치 안내를 읽고 이 Mac의 ${target}에서 GIL을 ${version} 버전으로 업데이트해 줘.\n${latest.install_doc ?? 'https://github.com/hyun06000/gil/blob/main/INSTALL.md'}\n터미널 명령과 설정 파일 편집을 내가 직접 하지 않도록 처리해 줘. 안내의 배포본과 버전이 다르면 임의로 설치하지 말고 차이를 알려줘. 선택한 앱의 GIL만 갱신하고 다른 설정, 기존 프로젝트, 안내 파일, .gil 기록과 Snapshot은 보존해 줘. 훅은 설치하지 마. 기존 설정을 백업하고 공식 설치 절차와 배포본 검증을 따라 줘.${target === 'Codex' ? '' : ' Claude 설정 변경 전에는 작업을 저장하고 앱을 완전히 종료하도록 안내하고, 종료를 확인한 뒤 변경해 줘.'} 완료하면 앱별 적용 버전과 다시 열어야 할 앱을 알려줘.`,
    })) };
}
