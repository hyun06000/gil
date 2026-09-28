// Layout is independent of the Host's applied display mode.
export function graphOrientation(mode, choice = null) {
  if (choice === 'horizontal' || choice === 'vertical') return choice;
  return 'horizontal';
}

// One initial intent per App instance, not a rule that forces every mode event.
// Wait for both a rendered Monitor and an explicit Host capability list. Consume
// before requesting: rejection, inline replies and later refreshes must not retry.
export function initialFullscreen() {
  let consumed = false;
  return {
    cancel() { consumed = true; },
    next({ connected, ready, context }) {
      if (consumed || !connected || !ready) return null;
      if (context.displayMode === 'fullscreen') { consumed = true; return null; }
      if (!Array.isArray(context.availableDisplayModes)) return null;
      consumed = true;
      return context.availableDisplayModes.includes('fullscreen') ? 'fullscreen' : null;
    },
  };
}
