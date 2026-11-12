import { t } from '../ui/locale.js';
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
        label: t('upd.available_label', { version }),
        line: t('upd.available_line', { installed: update.installed ?? '?', version }) };
    case 'unknown_installed':
      return { show: true, state: 'unknown_installed', version, latest,
        label: t('upd.unknown_label', { version }),
        line: t('upd.unknown_line', { version }) };
    case 'up_to_date':
      return { show: false, state: 'up_to_date', version, latest, line: t('upd.up_to_date', { installed: update.installed }) };
    case 'disabled':
      return { show: false, state: 'disabled', line: t('upd.disabled') };
    case 'checking':
      return { show: false, state: 'checking', line: t('upd.checking') };
    case 'not_installed':
      return { show: false, state: 'not_installed', line: null };
    default:
      return { show: false, state: update.state ?? 'unavailable',
        line: update.error ? t('upd.error', { error: update.error }) : null };
  }
}

// Steps per OS. Both Hosts are listed, because the Core does not know which one is showing it
// and the App never branches on the Host's name. Text only; no commands are executed.
export function updateGuide(update) {
  const latest = update?.latest ?? {};
  const version = latest.version ?? '?';
  const common = [
    t('upd.new_version', { version, installed: update?.installed ? t('upd.new_version_installed', { installed: update.installed }) : '' }),
    t('upd.release', { url: latest.release_url ?? '' }),
    t('upd.install_doc', { url: latest.install_doc ?? '' }),
    latest.notes ? t('upd.notes', { notes: latest.notes }) : null,
    t('upd.common_note'),
  ].filter(Boolean);
  if (update?.platform?.startsWith('windows')) {
    // Single-executable channel (v0.3.0+): the one-line install replaces the Companion button.
    return { title: t('upd.windows_title'), companion: false, steps: common.concat([
      t('upd.windows_step1'), t('upd.windows_step2'), t('upd.windows_step3'), t('upd.windows_step4'),
    ]) };
  }
  return { title: t('upd.mac_title'), companion: false,
    summary: t('upd.mac_summary', { version, installed: update?.installed ? t('upd.mac_summary_installed', { installed: update.installed }) : '' }),
    details: common.slice(1, -1),
    steps: [t('upd.mac_step1'), t('upd.mac_step2'), t('upd.mac_step3')],
    targets: ['Codex', 'Claude Desktop', t('upd.target_both')].map(target => ({
      label: target,
      hint: target === 'Codex' ? t('upd.hint_codex') : t('upd.hint_claude'),
      request: t('upd.request', { target, version,
        doc: latest.install_doc ?? 'https://github.com/hyun06000/gil/blob/main/INSTALL.md',
        claude_extra: target === 'Codex' ? '' : t('upd.request_claude_extra') }),
    })) };
}
