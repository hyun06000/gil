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
