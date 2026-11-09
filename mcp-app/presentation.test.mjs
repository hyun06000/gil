import test from 'node:test';
import assert from 'node:assert/strict';
import { graphOrientation, initialFullscreen, hostIdentity, hostStyles, styleEvidence, RETRY_DELAYS_MS } from './presentation.mjs';

test('all MCP display modes default to horizontal without claiming fullscreen', () => {
  for (const mode of ['fullscreen', 'inline', 'pip', undefined, null])
    assert.equal(graphOrientation(mode), 'horizontal');
});

const ready = { connected: true, ready: true,
  context: { displayMode: 'inline', availableDisplayModes: ['inline', 'fullscreen'] } };
test('initial fullscreen asks once at connect when the Host already advertises it, once more when rendered, never without a connection or a capability list', () => {
  let t = 0;
  const policy = initialFullscreen({ now: () => t });
  assert.equal(policy.next({ ...ready, connected: false }), null);
  assert.equal(policy.next({ ...ready, context: { displayMode: 'inline' } }), null, 'no capability list yet');
  t = 10;
  assert.equal(policy.next({ ...ready, ready: false }), 'fullscreen', 'connect-time request');
  assert.equal(policy.next({ ...ready, ready: false }), null, 'only one connect-time request');
  assert.equal(policy.next(ready), null, 'the connect-time request is still in flight');
  assert.equal(policy.settle('inline'), null, 'a declined connect-time request schedules no retry');
  assert.equal(policy.retrying, false);
  t = 900;
  assert.equal(policy.next(ready), 'fullscreen', 'ready-time request');
  // Includes rejected requests, inline replies and returning inline after success.
  for (let i = 0; i < 5; i++) assert.equal(policy.next(ready), null);
  assert.deepEqual(policy.attempts, [
    { phase: 'connect', at_ms: 10, applied: 'inline' }, { phase: 'ready', at_ms: 900, applied: undefined }]);
  assert.equal(policy.settle('fullscreen'), null);
  assert.equal(policy.attempts.at(-1).applied, 'fullscreen');
  assert.equal(policy.retrying, false);
});
test('a Host that applies the connect-time request gets no further automatic request', () => {
  const policy = initialFullscreen();
  assert.equal(policy.next({ ...ready, ready: false }), 'fullscreen');
  assert.equal(policy.settle('fullscreen'), null);
  assert.equal(policy.next(ready), null);
  assert.equal(policy.gesture(), null);
  assert.equal(policy.attempts.length, 1);
});
test('already fullscreen ends the intent; late capability advertisement still opens by default', () => {
  const applied = initialFullscreen();
  assert.equal(applied.next({ ...ready, context: { displayMode: 'fullscreen' } }), null);
  assert.equal(applied.next(ready), null);
  for (const initiallyReady of [false, true]) {
    const policy = initialFullscreen();
    const inline = { ...ready, ready: initiallyReady, context: { displayMode: 'inline', availableDisplayModes: ['inline'] } };
    assert.equal(policy.next(inline), null);
    assert.equal(policy.next(inline), null);
    assert.equal(policy.attempts.length, 0);
    assert.equal(policy.next(ready), 'fullscreen');
    assert.equal(policy.settle('fullscreen'), null);
    assert.equal(policy.next(ready), null);
  }
});
test('a manual inline choice while capabilities are incomplete prevents a late automatic request', () => {
  const policy = initialFullscreen();
  policy.next({ ...ready, context: { displayMode: 'inline', availableDisplayModes: ['inline'] } });
  policy.cancel();
  assert.equal(policy.next(ready), null);
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
  assert.equal(policy.retry(), null, 'no retry while a request is in flight');
  assert.equal(policy.settle('inline'), RETRY_DELAYS_MS[0]);
  assert.equal(policy.retrying, true);
  assert.equal(policy.retry(), 'fullscreen');
  assert.equal(policy.settle(null), RETRY_DELAYS_MS[1]);
  assert.equal(policy.retry(), 'fullscreen');
  assert.equal(policy.settle('inline'), null, 'automatic attempts are exhausted');
  assert.equal(policy.awaitingGesture, true);
  assert.equal(policy.retry(), null);
  assert.equal(policy.next(ready), null, 'the consumed intent never re-requests by itself');
  assert.equal(policy.gesture(), 'fullscreen', 'the first user gesture carries the last request');
  assert.equal(policy.gesture(), null);
  assert.equal(policy.settle('inline'), null);
  assert.deepEqual(policy.attempts.map(a => [a.phase, a.applied]),
    [['ready', 'inline'], ['retry', null], ['retry', 'inline'], ['gesture', 'inline']]);
});
test('a confirmed fullscreen, a manual choice or a Host event ends automatic retries', () => {
  let policy = initialFullscreen();
  policy.next(ready); assert.equal(policy.settle('fullscreen'), null);
  assert.equal(policy.retrying, false); assert.equal(policy.gesture(), null);
  policy = initialFullscreen();
  policy.next(ready); policy.settle('inline'); policy.cancel();
  assert.equal(policy.retry(), null);
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
  assert.match(available.line, /업데이트 안내/);
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
  assert.deepEqual(mac.targets.map(t => t.label), ['Codex', 'Claude Desktop', 'Codex와 Claude Desktop']);
  for (const target of mac.targets) {
    assert.ok(target.request.includes(latest.install_doc));
    assert.ok(target.request.includes(latest.version));
    assert.ok(target.request.includes(target.label));
    assert.match(target.request, /훅은 설치하지 마/);
  }
  assert.match(mac.targets[1].hint, /Claude 밖/);
  assert.match(mac.targets[2].request, /종료를 확인한 뒤/);
  for (const guide of [win, mac]) {
    assert.ok(guide.steps.every(s => !/curl|npm|cargo|brew|\$ /.test(s)), 'guidance is prose, not commands');
    assert.ok([...guide.steps, ...(guide.details ?? [])].some(s => s.includes(latest.release_url)));
  }
});
