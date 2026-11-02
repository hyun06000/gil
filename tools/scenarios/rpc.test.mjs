import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CoreClient, runCommand } from './lib/rpc.mjs';

const fake = join(dirname(fileURLToPath(import.meta.url)), 'fixtures', 'fake-core.mjs');
// CI process startup is not the behavior under test. The hang case below still has its
// explicit 400 ms deadline. Always reap the fixture even when initialize/assertions fail,
// otherwise node --test waits indefinitely on the child after reporting a failed test.
const client = (t) => {
  const settingsDir = mkdtempSync(join(tmpdir(), 'gil-rpc-'));
  const c = new CoreClient({ binary: process.execPath, args: [fake], settingsDir, requestTimeoutMs: 5000 }).start();
  t.after(async () => { try { await c.close(100); } finally { rmSync(settingsDir, { recursive: true, force: true }); } });
  return c;
};
const alive = (pid) => { try { process.kill(pid, 0); return true; } catch { return false; } };
const wait = (ms) => new Promise((r) => setTimeout(r, ms));

test('initialize, call, JSON-RPC errors are returned, clean EOF exits 0', async (t) => {
  const c = client(t);
  const init = await c.initialize();
  assert.equal(init.serverInfo.name, 'fake-core');
  const r = await c.call('gil_status', { project_root: '/x' });
  assert.equal(r.ok, true);
  const bad = await c.call('nope', {});
  assert.equal(bad.ok, false); assert.equal(bad.error.code, -32602);
  const exit = await c.close();
  assert.equal(exit.code, 0); assert.deepEqual(exit.stdoutNoise, []);
});

test('a request that never gets an answer times out with the method name and does not block later calls', async (t) => {
  const c = client(t); await c.initialize();
  await assert.rejects(c.request('hang', {}, { timeoutMs: 400 }), /timeout after 400 ms waiting for hang/);
  const ok = await c.request('tools/list', {});
  assert.ok(Array.isArray(ok.result.tools));
  await c.close();
});

test('non-JSON stdout lines are captured as noise instead of crashing the reader', async (t) => {
  const c = client(t); await c.initialize();
  const r = await c.request('noise', {});
  assert.equal(r.result.noisy, true);
  const exit = await c.close();
  assert.deepEqual(exit.stdoutNoise, ['not json at all']);
});

test('kill() terminates the whole process group including grandchildren', async (t) => {
  const c = client(t); await c.initialize();
  const { result } = await c.request('spawn_grandchild', {});
  assert.ok(alive(result.pid));
  await c.kill();
  await wait(200);
  assert.equal(alive(result.pid), false, 'grandchild survived the group kill');
  assert.ok(c.exit);
});

test('runCommand enforces its timeout and reports it', async () => {
  const r = await runCommand(process.execPath, ['-e', 'setInterval(() => {}, 1000)'], { timeoutMs: 300 });
  assert.equal(r.timedOut, true);
  const okRun = await runCommand(process.execPath, ['-e', 'console.log("hi")'], { timeoutMs: 5000 });
  assert.equal(okRun.code, 0); assert.match(okRun.stdout, /hi/);
});

test('an initialization timeout still reaps the fixture without the success-path close', async (t) => {
  let c;
  await t.test('early startup failure', async (child) => {
    c = client(child);
    c.requestTimeoutMs = 1;
    await assert.rejects(c.initialize(), /timeout after 1 ms waiting for initialize/);
    // No close here: the registered test cleanup owns failed startup.
  });
  assert.ok(c.exit, 'fixture is reaped before its test finishes');
  assert.equal(alive(c.child.pid), false);
});
