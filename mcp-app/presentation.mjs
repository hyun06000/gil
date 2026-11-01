// Layout is independent of the Host's applied display mode.
export function graphOrientation(mode, choice = null) {
  if (choice === 'horizontal' || choice === 'vertical') return choice;
  return 'horizontal';
}

// One initial intent per App instance, not a rule that forces every mode event.
// Wait for both a rendered Monitor and an explicit Host capability list. The intent is
// consumed before the first request. If the Host leaves the App inline without the user
// choosing so, the same intent may retry a few times (some Hosts ignore a request sent
// while they are still mounting the card) and finally ride on the user's first gesture
// (some Hosts apply display-mode changes only inside a user action). Any manual choice,
// a Host-confirmed fullscreen, or teardown ends the intent for good.
export const RETRY_DELAYS_MS = [1500, 4000];
export function initialFullscreen({ retries = RETRY_DELAYS_MS } = {}) {
  let consumed = false, done = false, attempts = 0, gestureArmed = false;
  const finish = () => { done = true; gestureArmed = false; };
  return {
    cancel: finish,
    next({ connected, ready, context }) {
      if (consumed || done || !connected || !ready) return null;
      if (context.displayMode === 'fullscreen') { finish(); consumed = true; return null; }
      if (!Array.isArray(context.availableDisplayModes)) return null;
      consumed = true;
      if (!context.availableDisplayModes.includes('fullscreen')) { finish(); return null; }
      attempts = 1;
      return 'fullscreen';
    },
    // Called with the mode the Host applied (or null when it gave none). Returns the delay
    // before another automatic request, or null when automatic requests are over.
    settle(appliedMode) {
      if (done || !consumed) return null;
      if (appliedMode === 'fullscreen') { finish(); return null; }
      if (attempts - 1 < retries.length) { const delay = retries[attempts - 1]; attempts += 1; return delay; }
      gestureArmed = true;
      return null;
    },
    // A Host-reported fullscreen (via a context event) also ends the intent.
    observe(mode) { if (mode === 'fullscreen') finish(); },
    // The user's first interaction anywhere in the App may carry the one remaining request.
    gesture() {
      if (!gestureArmed || done) return null;
      finish();
      return 'fullscreen';
    },
    get retrying() { return consumed && !done && !gestureArmed; },
    get awaitingGesture() { return gestureArmed && !done; },
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
        line: `설치된 GIL ${update.installed ?? '?'} → 새 버전 ${version} 이 있습니다. 업데이트는 사용자가 직접 진행합니다.` };
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
  return { title: 'macOS — 설치 안내의 절차로 업데이트', companion: false, steps: common.concat([
    'Codex: 새 대화에 "GIL 을 ' + version + ' 으로 업데이트해 줘. 설치 안내(INSTALL.md)의 공식 Plugin 절차로 GIL 등록만 교체하고, 다른 등록과 프로젝트·.gil 기록은 바꾸지 마." 라고 요청합니다.',
    'Claude Desktop(Claude Code 탭): 설치 안내대로 새 archive 를 새 폴더에 풀고, Claude Desktop 을 완전히 종료한 뒤 설정의 gil 항목을 새 Core 경로로 바꿉니다. 바꾸지 않으면 Claude 만 이전 버전에 남습니다.',
  ]) };
}
