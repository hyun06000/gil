import test from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import { build } from 'esbuild';
import { retryAfter } from './host.mjs';

// Execute the real entrypoint, replacing only its Host and renderer boundaries.
// This checks callback ordering and button wiring, not merely the policy helper.
const bundled = await build({ entryPoints: [new URL('./app.js', import.meta.url).pathname],
  bundle: true, write: false, format: 'iife', plugins: [{ name: 'display-harness', setup(b) {
    b.onResolve({ filter: /^@modelcontextprotocol\/ext-apps$|^\.\/host\.mjs$|^\.\.\/ui\/companion\.js$/ },
      ({ path }) => ({ path, namespace: 'display-harness' }));
    b.onLoad({ filter: /.*/, namespace: 'display-harness' }, ({ path }) => ({ contents:
      path.startsWith('@') ? `export class App { constructor() { return harness.app; } }
        export const applyDocumentTheme = theme => harness.applied.push(['theme', theme]);
        export const applyHostStyleVariables = variables => harness.applied.push(['variables', variables]);
        export const applyHostFonts = css => harness.applied.push(['fonts', css]);` :
      path.includes('host.mjs') ? `export const makeHost = (_app, _data, options) => { harness.presence = options.presence; return harness.host; };
        export const unwrap = value => value;
        export const retryAfter = harness.retryAfter;
        export const connectionMessage = error => String(error);` :
      'window.GIL_COMPANION = harness.renderer;' }));
  } }] });
const code = bundled.outputFiles[0].text;
const deferred = () => {
  let resolve;
  const promise = new Promise(done => { resolve = done; });
  return { promise, resolve };
};
const settle = () => new Promise(done => setImmediate(done));
const data = { scope_id: `project:${'a'.repeat(64)}`, view: {}, watching: true };
function fixture({ modes = ['inline', 'fullscreen'], mode = 'inline', answer, hostContext = {} } = {}) {
  const connected = deferred(), rendered = deferred(), calls = [], directions = [], native = [], observations = [];
  const timers = new Map(), documentEvents = new Map(), windowEvents = new Map();
  let serial = 0, ticks = 0, refreshes = 0, tickWork = async () => {};
  const elements = new Map();
  const element = id => {
    if (!elements.has(id)) elements.set(id, { textContent: '', disabled: false });
    return elements.get(id);
  };
  const app = { connect: () => connected.promise,
    getHostVersion: () => ({ name: 'harness-host', version: '0.0.0' }),
    getHostContext: () => ({ availableDisplayModes: modes, displayMode: mode, ...hostContext }),
    requestDisplayMode: async request => { calls.push(request.mode); return answer ? answer(request) : request; },
    callServerTool: async request => { native.push(request.name); return { said: 'opened' }; } };
  const applied = [];
  const harness = { app, retryAfter, applied,
    host: { listen() {}, reportPresence: async () => { observations.push(harness.presence()); },
      tick: async () => { observations.push(harness.presence()); ticks++; await tickWork(); },
      refresh: async () => { refreshes++; } },
    renderer: { ready: rendered.promise, setOrientation: direction => directions.push(direction) } };
  const document = { body: { dataset: {} }, hidden: false, getElementById: element,
    addEventListener: (name, fn) => documentEvents.set(name, fn) };
  const window = { addEventListener: (name, fn) => windowEvents.set(name, fn) };
  vm.runInNewContext(code, { harness, document, window,
    crypto: { randomUUID: () => 'test' },
    setTimeout: (fn, ms) => { const id = ++serial; timers.set(id, { fn, ms }); return id; },
    clearTimeout: id => timers.delete(id) });
  return { app, calls, directions, native, observations, element, document, connected, rendered, timers, applied,
    async fireTimerOf(ms) {
      const found = [...timers.entries()].find(([, t]) => t.ms === ms);
      assert.ok(found, `a ${ms}ms timer is scheduled`);
      timers.delete(found[0]); await found[1].fn(); await settle();
    },
    async click(id) {
      const target = element(id);
      target.closest = () => target;
      documentEvents.get('pointerdown')?.({ target });
      if (!target.disabled) await target.onclick?.();
      await settle();
    },
    async pointerdown() { await documentEvents.get('pointerdown')?.({}); await settle(); },
    ticks: () => ticks, refreshes: () => refreshes, tickWork: fn => { tickWork = fn; },
    async visibility(hidden) { document.hidden = hidden; await documentEvents.get('visibilitychange')?.(); await settle(); },
    async page(name, persisted) { await windowEvents.get(name)?.({ persisted }); await settle(); },
    async fireTimer() {
      assert.equal(timers.size, 1, 'exactly one heartbeat chain');
      const [id, timer] = timers.entries().next().value;
      timers.delete(id); await timer.fn(); await settle();
    },
    async start(order = 'connection') {
      if (order === 'connection') { connected.resolve(); await settle(); }
      const boot = app.ontoolresult(data);
      await settle();
      rendered.resolve(); await boot;
      if (order !== 'connection') { connected.resolve(); }
      await settle();
    } };
}

test('real App waits for rendering in either callback order and keeps horizontal after inline return', async () => {
  for (const order of ['connection', 'data']) {
    const f = fixture();
    if (order === 'connection') { f.connected.resolve(); await settle(); }
    const boot = f.app.ontoolresult(data);
    await settle();
    assert.deepEqual(f.calls, []);
    f.rendered.resolve(); await boot;
    if (order === 'data') { assert.deepEqual(f.calls, []); f.connected.resolve(); }
    await settle();
    assert.deepEqual(f.calls, ['fullscreen']);
    assert.equal(f.document.body.dataset.displayMode, 'fullscreen');
    assert.match(f.element('host-evidence').textContent, /"host": \{\s*"name": "harness-host"/);
    assert.doesNotMatch(f.element('host-say').textContent, /Codex|Cowork/);
    await f.element('inline').onclick();
    f.app.onhostcontextchanged({ displayMode: 'inline' });
    await f.app.ontoolresult(data);
    assert.deepEqual(f.calls, ['fullscreen', 'inline']);
    assert.equal(f.directions.at(-1), 'horizontal');
    await f.element('layout').onclick();
    f.app.onhostcontextchanged({ displayMode: 'fullscreen' });
    assert.equal(f.directions.at(-1), 'vertical');
    assert.deepEqual(f.native, []);
  }
});

test('a hidden App stops scheduling, resumes with a full refresh and does not reopen fullscreen', async () => {
  const f = fixture(); await f.start();
  await f.element('inline').onclick();
  await f.element('layout').onclick();
  const before = f.ticks();
  await f.visibility(true);
  assert.equal(f.timers.size, 0);
  assert.equal(f.ticks(), before);
  await f.visibility(false);
  assert.equal(f.refreshes(), 1);
  assert.equal(f.timers.size, 1);
  await f.fireTimer();
  assert.equal(f.ticks(), before + 1);
  assert.deepEqual(f.calls, ['fullscreen', 'inline']);
  assert.equal(f.directions.at(-1), 'vertical');
});

test('cached pagehide/pageshow resumes the same App; a real teardown remains terminal', async () => {
  const f = fixture(); await f.start();
  const before = f.ticks();
  await f.page('pagehide', true);
  assert.equal(f.timers.size, 0);
  await f.page('pageshow', true);
  assert.equal(f.refreshes(), 1);
  assert.equal(f.timers.size, 1);
  await f.fireTimer();
  assert.equal(f.ticks(), before + 1);
  assert.deepEqual(f.calls, ['fullscreen']);
  await f.app.onteardown();
  await f.page('pageshow', true);
  await f.visibility(false);
  assert.equal(f.timers.size, 0);
  assert.equal(f.refreshes(), 1);
});

test('pause/resume during a pending heartbeat coalesces and teardown cancels late rescheduling', async () => {
  const f = fixture(); await f.start();
  const held = deferred();
  f.tickWork(() => held.promise);
  const inFlight = f.fireTimer();
  await settle();
  await f.visibility(true); await f.visibility(false);
  await f.visibility(true); await f.visibility(false);
  assert.equal(f.refreshes(), 0, 'resume waits for the in-flight read');
  held.resolve(); await inFlight;
  assert.equal(f.refreshes(), 1, 'multiple returns need one queued refresh');
  assert.equal(f.timers.size, 1);
  const last = deferred(); f.tickWork(() => last.promise);
  const pending = f.fireTimer(); await settle();
  await f.app.onteardown();
  last.resolve(); await pending;
  assert.equal(f.timers.size, 0);
});

test('an App first rendered while hidden does not request fullscreen until visible', async () => {
  const f = fixture();
  await f.visibility(true); await f.start();
  assert.deepEqual(f.calls, []);
  assert.equal(f.timers.size, 0);
  await f.visibility(false);
  assert.deepEqual(f.calls, ['fullscreen']);
  assert.equal(f.refreshes(), 1);
  assert.equal(f.timers.size, 1);
  await f.page('pagehide', false);
  await f.page('pageshow', true);
  assert.equal(f.timers.size, 0);
});

test('transport failures back off to one minute and recover without recreating or reopening the App', async () => {
  const f = fixture(); await f.start();
  f.tickWork(async () => { throw new Error('temporarily disconnected'); });
  for (const ms of [2000, 4000, 8000, 16000, 30000, 60000, 60000]) {
    await f.fireTimer();
    assert.equal(f.timers.values().next().value.ms, ms);
  }
  f.tickWork(async () => {});
  await f.fireTimer();
  assert.equal(f.timers.values().next().value.ms, 2000);
  assert.deepEqual(f.calls, ['fullscreen']);
  assert.deepEqual(f.native, []);
});

test('declined, undefined and rejected automatic requests retain a working manual expand button', async () => {
  for (const outcome of ['inline', 'undefined', 'error']) {
    let automatic = true;
    const f = fixture({ answer(request) {
      if (!automatic) return request;
      if (outcome === 'error') throw new Error('user gesture required');
      return outcome === 'inline' ? { mode: 'inline' } : undefined;
    } });
    await f.start();
    assert.deepEqual(f.calls, ['fullscreen']);
    assert.equal(f.document.body.dataset.displayMode, 'inline');
    assert.equal(f.element('expand').disabled, false);
    assert.match(f.element('host-say').textContent, /모니터 펼치기/);
    if (outcome === 'undefined') assert.equal(JSON.parse(f.element('host-evidence').textContent).response, 'undefined');
    f.app.onhostcontextchanged({ displayMode: 'inline' });
    await settle();
    assert.deepEqual(f.calls, ['fullscreen']);
    automatic = false;
    await f.element('expand').onclick();
    assert.deepEqual(f.calls, ['fullscreen', 'fullscreen']);
    assert.equal(f.document.body.dataset.displayMode, 'fullscreen');
    assert.deepEqual(f.native, []);
  }
});

test('inline-only and already-fullscreen Hosts receive no automatic request', async () => {
  for (const settings of [{ modes: ['inline'] }, { mode: 'fullscreen' }]) {
    const f = fixture(settings);
    await f.start();
    assert.deepEqual(f.calls, []);
    assert.equal(f.directions.at(-1), 'horizontal');
    f.app.onhostcontextchanged({ displayMode: 'inline', availableDisplayModes: ['inline', 'fullscreen'] });
    assert.deepEqual(f.calls, []);
    assert.deepEqual(f.native, []);
  }
});

test('late capabilities trigger once, but manual inline or teardown cancels that intent', async () => {
  for (const cancel of ['none', 'manual', 'teardown']) {
    const f = fixture({ modes: null });
    await f.start();
    assert.deepEqual(f.calls, []);
    if (cancel === 'manual') {
      // A manual choice before the renderer is ready must also win.
      const g = fixture();
      g.connected.resolve(); await settle();
      await g.element('inline').onclick();
      await g.start();
      assert.deepEqual(g.calls, ['inline']);
      continue;
    }
    if (cancel === 'teardown') await f.app.onteardown();
    f.app.onhostcontextchanged({ availableDisplayModes: ['inline', 'fullscreen'] });
    await settle();
    assert.deepEqual(f.calls, cancel === 'none' ? ['fullscreen'] : []);
  }
});


test('App reports rendering, actual modes, hidden and teardown independently of native availability', async () => {
  const f = fixture(); f.connected.resolve(); await settle();
  const boot = f.app.ontoolresult(data); await settle();
  assert.equal(f.observations.at(-1).presentation, 'loading');
  assert.ok(!f.observations.some(x => x.presentation === 'fullscreen'));
  f.rendered.resolve(); await boot; await settle();
  assert.equal(f.observations.at(-1).presentation, 'fullscreen');
  await f.visibility(true);
  assert.equal(f.observations.at(-1).presentation, 'hidden');
  await f.visibility(false); await f.fireTimer();
  assert.equal(f.observations.at(-1).presentation, 'fullscreen');
  await f.element('inline').onclick();
  assert.equal(f.observations.at(-1).presentation, 'inline');
  await f.app.onteardown();
  assert.equal(f.observations.at(-1).presentation, 'closed');
  assert.ok(f.observations.every((x,i) => Number(x.sequence) === i+1));
  assert.deepEqual(f.native, []);
});

test('a Host that keeps the App inline gets bounded automatic retries, then the first gesture requests fullscreen', async () => {
  const f = fixture({ answer: () => ({ mode: 'inline' }) }); await f.start();
  assert.deepEqual(f.calls, ['fullscreen']);
  assert.equal(f.document.body.dataset.displayMode, 'inline');
  await f.fireTimerOf(1500);
  assert.deepEqual(f.calls, ['fullscreen', 'fullscreen']);
  await f.fireTimerOf(4000);
  assert.deepEqual(f.calls, ['fullscreen', 'fullscreen', 'fullscreen']);
  assert.equal([...f.timers.values()].filter(t => t.ms === 1500 || t.ms === 4000).length, 0, 'no further automatic timer');
  assert.match(f.element('host-say').textContent, /클릭하면 펼칩니다/);
  await f.pointerdown();
  assert.deepEqual(f.calls, ['fullscreen', 'fullscreen', 'fullscreen', 'fullscreen']);
  await f.pointerdown();
  assert.equal(f.calls.length, 4, 'one gesture-carried request only');
  assert.equal(f.element('expand').disabled, false, 'manual expand stays available');
});

test('a manual inline choice or a Host fullscreen event cancels pending automatic retries', async () => {
  let f = fixture({ answer: () => ({ mode: 'inline' }) }); await f.start();
  await f.element('inline').onclick();
  assert.equal([...f.timers.values()].some(t => t.ms === 1500), false);
  await f.pointerdown();
  assert.deepEqual(f.calls, ['fullscreen', 'inline']);
  f = fixture({ answer: () => ({ mode: 'inline' }) }); await f.start();
  f.app.onhostcontextchanged({ displayMode: 'fullscreen' });
  await f.fireTimerOf(1500);
  assert.deepEqual(f.calls, ['fullscreen'], 'a Host-confirmed fullscreen ends the intent');
  await f.pointerdown();
  assert.deepEqual(f.calls, ['fullscreen']);
});

test('a Host that applies the automatic request schedules no retry and ignores later gestures', async () => {
  const f = fixture(); await f.start();
  assert.equal([...f.timers.values()].some(t => t.ms === 1500), false);
  await f.pointerdown();
  assert.deepEqual(f.calls, ['fullscreen']);
});


test('gesture fallback never captures explicit inline or native controls', async () => {
  for (const id of ['inline', 'native']) {
    const f = fixture({ answer: () => ({ mode: 'inline' }) }); await f.start();
    await f.fireTimerOf(1500); await f.fireTimerOf(4000);
    await f.click(id);
    assert.deepEqual(f.calls, id === 'inline' ? ['fullscreen','fullscreen','fullscreen','inline'] : ['fullscreen','fullscreen','fullscreen']);
    assert.deepEqual(f.native, id === 'native' ? ['show_gil_companion'] : []);
    await f.pointerdown();
    assert.equal(f.calls.filter(mode => mode === 'fullscreen').length, 3);
  }
});

test('Host theme, style variables and fonts are adopted on connect and on change; junk is dropped', async () => {
  const fonts = '@font-face{font-family:host-sans;src:url(/assets/a.woff2)}';
  const f = fixture({ hostContext: { theme: 'dark', styles: { variables: {
    '--color-background-primary': 'light-dark(#fff, #222)', 'bad key': '#000', '--color-text-primary': 42,
  }, css: { fonts } } } });
  await f.start();
  assert.deepEqual(JSON.parse(JSON.stringify(f.applied)), [['theme', 'dark'],
    ['variables', { '--color-background-primary': 'light-dark(#fff, #222)' }], ['fonts', fonts]]);
  assert.deepEqual(JSON.parse(f.element('host-evidence').textContent).styles,
    { theme: 'dark', variables: 1, fonts: true });
  f.app.onhostcontextchanged({ theme: 'light' });
  assert.deepEqual(JSON.parse(JSON.stringify(f.applied.slice(3))), [['theme', 'light'],
    ['variables', { '--color-background-primary': 'light-dark(#fff, #222)' }], ['fonts', fonts]]);
  assert.equal(JSON.parse(f.element('host-evidence').textContent).styles.theme, 'light');
  // Display-mode changes alone never re-apply styles.
  f.app.onhostcontextchanged({ displayMode: 'inline' });
  assert.equal(f.applied.length, 6);
});

test('without Host styles nothing is applied and the evidence says so', async () => {
  const f = fixture(); await f.start();
  assert.deepEqual(JSON.parse(JSON.stringify(f.applied)), []);
  assert.deepEqual(JSON.parse(f.element('host-evidence').textContent).styles,
    { theme: null, variables: 0, fonts: false });
});
