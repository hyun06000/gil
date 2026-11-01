// Poll scheduling consumes the actual read outcome. A handled read error is not a successful read.
// The outer App still suspends polling when hidden/disconnected and serializes its own ticks.
export function makePlanPolling(review, { now = Date.now } = {}) {
  let requested = true, nextAt = 0;
  return {
    request() { requested = true; },
    async tick() {
      if (!requested && now() < nextAt) return;
      requested = false; // A new hint while awaiting this read must survive its completion.
      const result = await review.refresh();
      const delay = result?.status === 'ok' ? 30000
        : result?.status === 'stopped' ? Infinity
        : result?.status === 'failed' ? result.retryAfterMs : 2000;
      nextAt = now() + delay;
    },
  };
}
