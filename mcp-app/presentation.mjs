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
