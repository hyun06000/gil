import test from 'node:test';
import assert from 'node:assert/strict';
import { graphOrientation, initialFullscreen } from './presentation.mjs';

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
