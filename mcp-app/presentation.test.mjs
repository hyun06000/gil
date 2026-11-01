import test from 'node:test';
import assert from 'node:assert/strict';
import { graphOrientation, initialFullscreen, hostIdentity, RETRY_DELAYS_MS } from './presentation.mjs';

test('all MCP display modes default to horizontal without claiming fullscreen', () => {
  for (const mode of ['fullscreen', 'inline', 'pip', undefined, null])
    assert.equal(graphOrientation(mode), 'horizontal');
});

const ready = { connected: true, ready: true,
  context: { displayMode: 'inline', availableDisplayModes: ['inline', 'fullscreen'] } };
test('initial fullscreen waits for both connection and rendered data, then requests once', () => {
  const policy = initialFullscreen();
  assert.equal(policy.next({ ...ready, connected: false }), null);
  assert.equal(policy.next({ ...ready, ready: false }), null);
  assert.equal(policy.next({ ...ready, context: { displayMode: 'inline' } }), null);
  assert.equal(policy.next(ready), 'fullscreen');
  // Includes rejected requests, inline replies and returning inline after success.
  for (let i = 0; i < 5; i++) assert.equal(policy.next(ready), null);
});
test('already fullscreen and inline-only Hosts do not receive automatic requests', () => {
  for (const context of [{ displayMode: 'fullscreen' }, { availableDisplayModes: ['inline'] }]) {
    const policy = initialFullscreen();
    assert.equal(policy.next({ ...ready, context }), null);
    assert.equal(policy.next(ready), null);
  }
});
test('a manual choice or teardown cancels a late initial request', () => {
  const policy = initialFullscreen();
  policy.cancel();
  assert.equal(policy.next(ready), null);
});
test('an explicit user choice survives Host mode changes', () => {
  for (const mode of ['fullscreen', 'inline', undefined]) {
    assert.equal(graphOrientation(mode, 'vertical'), 'vertical');
    assert.equal(graphOrientation(mode, 'horizontal'), 'horizontal');
  }
});
test('host identity is copied as reported and never decides display policy', () => {
  assert.equal(hostIdentity({}), null);
  assert.equal(hostIdentity({ getHostVersion: () => undefined }), null);
  assert.equal(hostIdentity({ getHostVersion: () => { throw new Error('before connect'); } }), null);
  assert.deepEqual(hostIdentity({ getHostVersion: () => ({ name: 'claude-desktop', version: '4.4212.0' }) }),
    { name: 'claude-desktop', version: '4.4212.0' });
  assert.deepEqual(hostIdentity({ getHostVersion: () => ({ name: 7, version: null, title: 'Codex' }) }),
    { name: null, version: null, title: 'Codex' });
  // The same one-shot fullscreen request applies regardless of the reported Host.
  const policy = initialFullscreen();
  assert.equal(policy.next(ready), 'fullscreen');
});
test('an inline reply to the automatic request retries a bounded number of times, then waits for a gesture', () => {
  const policy = initialFullscreen();
  assert.equal(policy.next(ready), 'fullscreen');
  assert.equal(policy.settle('inline'), RETRY_DELAYS_MS[0]);
  assert.equal(policy.retrying, true);
  assert.equal(policy.settle(null), RETRY_DELAYS_MS[1]);
  assert.equal(policy.settle('inline'), null, 'automatic attempts are exhausted');
  assert.equal(policy.awaitingGesture, true);
  assert.equal(policy.next(ready), null, 'the consumed intent never re-requests by itself');
  assert.equal(policy.gesture(), 'fullscreen', 'the first user gesture carries the last request');
  assert.equal(policy.gesture(), null);
  assert.equal(policy.settle('inline'), null);
});
test('a confirmed fullscreen, a manual choice or a Host event ends automatic retries', () => {
  let policy = initialFullscreen();
  policy.next(ready); assert.equal(policy.settle('fullscreen'), null);
  assert.equal(policy.retrying, false); assert.equal(policy.gesture(), null);
  policy = initialFullscreen();
  policy.next(ready); policy.settle('inline'); policy.cancel();
  assert.equal(policy.settle('inline'), null); assert.equal(policy.gesture(), null);
  policy = initialFullscreen();
  policy.next(ready); policy.settle('inline'); policy.observe('fullscreen');
  assert.equal(policy.retrying, false); assert.equal(policy.settle('inline'), null);
});
test('a gesture before any automatic attempt or on an inline-only Host requests nothing', () => {
  let policy = initialFullscreen();
  assert.equal(policy.gesture(), null);
  policy = initialFullscreen();
  assert.equal(policy.next({ ...ready, context: { displayMode: 'inline', availableDisplayModes: ['inline'] } }), null);
  assert.equal(policy.settle('inline'), null); assert.equal(policy.gesture(), null);
});
