import test from 'node:test';
import assert from 'node:assert/strict';
import { makeHost, retryAfter, RECONCILE_MS, connectionMessage } from './host.mjs';

function rig() {
  let time = 0, revision = '1', fail = false;
  const calls = [];
  const initial = { scope_id: 'project:test', label: '시험', view: { schema_version: 1 }, revision, watching: true };
  const app = { async callServerTool({ name, arguments: args }) {
    calls.push({ name, args });
    if (fail && name === 'gil_monitor_read') return { isError: true, structuredContent: { code: 'busy', said: 'busy' } };
    return { structuredContent: { ...initial, revision, detail: { step_ref: args.step_ref } } };
  } };
  const host = makeHost(app, initial, { now: () => time });
  return { host, calls, time: (v) => { time = v; }, change: () => { revision = '2'; }, fail: (v) => { fail = v; } };
}

test('initial View is reused; quiet polling never scans before five minutes', async () => {
  const r = rig(); await r.host.loadView('project:test');
  r.host.listen('gil://refresh', () => r.host.loadView('project:test'));
  for (const at of [1000, 60_000, 299_999]) { r.time(at); await r.host.tick(); }
  assert.equal(r.calls.filter(c => c.name === 'gil_monitor_read').length, 0);
  r.time(RECONCILE_MS); await r.host.tick();
  assert.equal(r.calls.filter(c => c.name === 'gil_monitor_read').length, 1);
});

test('hint re-reads the whole View, not a delta, without waiting five minutes', async () => {
  const r = rig(); await r.host.loadView('project:test');
  r.host.listen('gil://refresh', () => r.host.loadView('project:test'));
  r.change(); r.time(2000); await r.host.tick(); await r.host.tick();
  assert.equal(r.calls.filter(c => c.name === 'gil_monitor_read').length, 1);
  assert.ok(r.calls.every(c => JSON.stringify(c.args) === '{"scope_id":"project:test"}'));
});

test('failed reads back off, new hints do not bypass the deadline, success resets it', async () => {
  const r = rig(); await r.host.loadView('project:test');
  r.host.listen('gil://refresh', async () => { try { await r.host.loadView('project:test'); } catch {} });
  r.fail(true); r.change();
  await r.host.tick();
  r.time(1999); await r.host.tick();
  assert.equal(r.calls.filter(c => c.name === 'gil_monitor_read').length, 1);
  r.time(2000); await r.host.tick();
  r.time(5999); await r.host.tick();
  assert.equal(r.calls.filter(c => c.name === 'gil_monitor_read').length, 2);
  r.fail(false); r.time(6000); await r.host.tick();
  assert.equal(r.calls.filter(c => c.name === 'gil_monitor_read').length, 3);
  assert.deepEqual([1,2,3,4,5,6,100].map(retryAfter), [2000,4000,8000,16000,30000,60000,60000]);
});

test('scope is bound to this App and detail carries only scope + StepRef', async () => {
  const r = rig(); await assert.rejects(r.host.loadView('project:other'));
  await assert.rejects(r.host.loadDetail('project:other', 'step:C1/S1'));
  assert.equal(r.calls.length, 0);
  assert.deepEqual(await r.host.loadDetail('project:test', 'step:C1/S1'), { step_ref: 'step:C1/S1' });
  assert.deepEqual(r.calls[0].args, { scope_id: 'project:test', step_ref: 'step:C1/S1' });
});

test('a response for another scope is refused, never painted', async () => {
  const host = makeHost({ callServerTool: async () => ({ structuredContent: { scope_id: 'other' } }) },
    { scope_id: 'project:test', label: 'a', revision: '1', view: { schema_version: 1 } });
  await assert.rejects(host.loadDetail('project:test', 'step:C1/S1'), e => e.code === 'unknown_scope');
});

test('replayed View refreshes immediately when a restarted server returns a new epoch', async () => {
  const scope = 'project:saved';
  const old = { scope_id: scope, revision: 'old:1', view: { schema_version: 1, marker: 'stale' }, watching: true };
  const calls = [];
  const host = makeHost({ async callServerTool(request) {
    calls.push(request);
    return { structuredContent: { ...old, revision: 'new:1', view: { schema_version: 1, marker: 'current' } } };
  } }, old, { now: () => 0 });
  assert.equal((await host.loadView(scope)).marker, 'stale');
  let fresh;
  host.listen('gil://refresh', async () => { fresh = await host.loadView(scope); });
  await host.tick();
  assert.equal(fresh.marker, 'current');
  assert.deepEqual(calls.map(c => c.name), ['gil_monitor_poll', 'gil_monitor_read']);
  assert.ok(calls.every(c => JSON.stringify(c.arguments) === '{"scope_id":"project:saved"}'));
});

test('a temporary lost connection can recover without recreating the App or preparing a root', async () => {
  let absent = true;
  const host = makeHost({ async callServerTool({ arguments: args }) {
    return absent ? { isError: true, structuredContent: { code: 'project_missing', said: 'missing' } }
      : { structuredContent: { scope_id: args.scope_id, detail: { step_ref: args.step_ref } } };
  } }, { scope_id: 'project:saved' });
  const events = []; host.listen('gil://connection', e => events.push(e?.code || 'ready'));
  await assert.rejects(host.loadDetail('project:saved', 'step:C1/S1'), e => e.code === 'project_missing');
  absent = false;
  assert.deepEqual(await host.loadDetail('project:saved', 'step:C1/S1'), { step_ref: 'step:C1/S1' });
  assert.deepEqual(events, ['project_missing', 'ready']);
  assert.match(connectionMessage({ code: 'project_missing' }), /자동으로/);
  assert.match(connectionMessage({ code: 'project_moved' }), /다시 선택/);
  assert.doesNotMatch(connectionMessage({ code: 'reconnect_required' }), /이 창이 연 Project 가 아니다/);
});
