import test from 'node:test';
import assert from 'node:assert/strict';
import { graphOrientation, initialFullscreen, hostIdentity, hostStyles, styleEvidence, RETRY_DELAYS_MS } from './presentation.mjs';

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

test('Host styles are adopted as reported and shape-checked, never inferred', () => {
  const fonts = '@font-face{font-family:host-sans;src:url(/assets/a.woff2)format("woff2")}';
  const context = { theme: 'dark', styles: { variables: {
    '--color-background-primary': 'light-dark(rgba(255, 255, 255, 1), rgba(48, 48, 46, 1))',
    '--font-sans': 'host-sans, sans-serif',
    'color-text-primary': '#000', '--Bad Name': '#000', '--color-text-primary': 42,
    '--color-border-primary': 'red; } body { display: none', '--shadow-sm': '',
  }, css: { fonts } } };
  assert.deepEqual(hostStyles(context), { theme: 'dark', variables: {
    '--color-background-primary': 'light-dark(rgba(255, 255, 255, 1), rgba(48, 48, 46, 1))',
    '--font-sans': 'host-sans, sans-serif' }, fonts });
  assert.deepEqual(styleEvidence(context), { theme: 'dark', variables: 2, fonts: true });
  // Nothing reported, or junk: neutral defaults and an evidence block that says so.
  for (const empty of [undefined, null, {}, { theme: 'blue', styles: 'x' },
    { styles: { variables: [], css: { fonts: '' } } }, { styles: { variables: { '--a': 'b;' } } }]) {
    assert.deepEqual(hostStyles(empty), { theme: null, variables: null, fonts: null });
    assert.deepEqual(styleEvidence(empty), { theme: null, variables: 0, fonts: false });
  }
  assert.equal(hostStyles({ styles: { css: { fonts: 'x</STYLE><script>' } } }).fonts, null);
  assert.equal(hostStyles({ styles: { css: { fonts: 'a'.repeat(65537) } } }).fonts, null);
});

import { updateNotice, updateGuide } from './presentation.mjs';
const latest = { version: '0.2.1-preview.9', release_url: 'https://github.com/x/r', install_doc: 'https://github.com/x/i', notes: 'n' };
test('update notice shows a button only when a newer or unknown-relative version is known', () => {
  assert.equal(updateNotice(undefined).show, false);
  assert.equal(updateNotice({ state: 'not_installed' }).show, false);
  assert.equal(updateNotice({ state: 'up_to_date', installed: '0.2.1-preview.9', latest }).show, false);
  assert.equal(updateNotice({ state: 'disabled' }).show, false);
  assert.equal(updateNotice({ state: 'checking' }).show, false);
  assert.equal(updateNotice({ state: 'unavailable', error: 'feed request failed (22)' }).show, false);
  const available = updateNotice({ state: 'available', installed: '0.2.1-preview.8', latest });
  assert.equal(available.show, true);
  assert.match(available.label, /0\.2\.1-preview\.9/);
  assert.match(available.line, /사용자가 직접/);
  const unknown = updateNotice({ state: 'unknown_installed', latest });
  assert.equal(unknown.show, true);
  assert.match(unknown.label, /최신 0\.2\.1-preview\.9/);
});
test('update guide lists both Hosts, never a command, and offers Companion only on Windows', () => {
  const win = updateGuide({ platform: 'windows-x64', installed: '0.1.1-owner.7', latest });
  assert.equal(win.companion, true);
  assert.ok(win.steps.some(s => /업데이트 확인/.test(s)) && win.steps.some(s => /Claude Desktop/.test(s)));
  const mac = updateGuide({ platform: 'darwin-arm64', installed: '0.2.1-preview.8', latest });
  assert.equal(mac.companion, false);
  assert.ok(mac.steps.some(s => /Codex/.test(s)) && mac.steps.some(s => /Claude Desktop/.test(s)));
  for (const guide of [win, mac]) {
    assert.ok(guide.steps.every(s => !/curl|npm|cargo|brew|\$ /.test(s)), 'guidance is prose, not commands');
    assert.ok(guide.steps.some(s => s.includes(latest.release_url)));
  }
});
