// Runs only against an installed product launcher on native Windows.
import assert from 'node:assert/strict';
import {spawn, spawnSync} from 'node:child_process';
import {open, readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {resolve} from 'node:path';

assert.equal(process.platform, 'win32', 'Windows is required');
assert.equal(process.arch, 'x64', 'The first product cohort is x64 only');
const launcher = resolve(process.argv[2] || '');
const core = resolve(process.argv[3] || '');
const installer = resolve(process.argv[4] || '');
const receiptPath = resolve(process.argv[5] || '');
const expectedVersion = process.argv[6] || '';
assert.ok(process.argv[2] && process.argv[3] && process.argv[4] && process.argv[5] && expectedVersion,
  'Usage: node product-launcher-smoke.mjs <launcher> <running-core> <installer> <receipt> <expected-version>');

const version = spawnSync(launcher, ['--version'], {encoding: 'utf8', timeout: 30_000});
assert.equal(version.status, 0, version.stderr);
assert.match(version.stdout.trim(), /^gil-launcher [0-9A-Za-z.+-]+$/);

const before = createHash('sha256').update(await readFile(core)).digest('hex');
const child = spawn(launcher, ['mcp', '--serve'], {stdio: ['pipe', 'pipe', 'pipe']});
let serial = 0, buffer = '', stderr = '';
const pending = new Map();
child.stderr.setEncoding('utf8');
child.stderr.on('data', chunk => { stderr = (stderr + chunk).slice(-8192); });
child.stdout.setEncoding('utf8');
child.stdout.on('data', chunk => {
  buffer += chunk;
  while (buffer.includes('\n')) {
    const at = buffer.indexOf('\n');
    const line = buffer.slice(0, at); buffer = buffer.slice(at + 1);
    const message = JSON.parse(line);
    const waiting = pending.get(message.id);
    if (!waiting) continue;
    pending.delete(message.id); clearTimeout(waiting.timer);
    if (message.error) waiting.reject(new Error(JSON.stringify(message.error)));
    else waiting.resolve(message.result);
  }
});
const request = (method, params) => new Promise((resolve, reject) => {
  const id = ++serial;
  const timer = setTimeout(() => { pending.delete(id); reject(new Error(`timeout: ${method}; ${stderr}`)); }, 30_000);
  pending.set(id, {resolve, reject, timer});
  child.stdin.write(`${JSON.stringify({jsonrpc: '2.0', id, method, params})}\n`);
});

try {
  await request('initialize', {protocolVersion: '2025-06-18', capabilities: {},
    clientInfo: {name: 'gil-product-installer-smoke', version: '1'}});
  child.stdin.write(`${JSON.stringify({jsonrpc: '2.0', method: 'notifications/initialized'})}\n`);
  const listed = await request('tools/list', {});
  assert.ok(listed.tools.some(one => one.name === 'gil_status'));

  // The launcher keeps a read-only, non-delete-sharing handle open from digest verification until
  // the child exits. An installer must therefore add a new version, never modify these bytes.
  await assert.rejects(open(core, 'r+'), error => ['EBUSY', 'EACCES', 'EPERM'].includes(error.code));
  assert.equal(createHash('sha256').update(await readFile(core)).digest('hex'), before);

  // Codex does not need to close for an update. Keep this initialized MCP process alive while NSIS
  // repairs the Companion and republishes the selection receipt. Installation must neither stop this
  // connection nor replace the immutable Core bytes mapped by it.
  const repair = spawnSync(installer, ['/S'], {
    encoding: 'utf8',
    timeout: 120_000,
    windowsHide: true,
  });
  assert.equal(repair.status, 0,
    `installer failed while MCP stayed connected: ${repair.error || repair.stderr || repair.stdout}`);
  const afterUpdate = await request('tools/list', {});
  assert.ok(afterUpdate.tools.some(one => one.name === 'gil_status'));
  assert.equal(createHash('sha256').update(await readFile(core)).digest('hex'), before);
  const selected = JSON.parse(await readFile(receiptPath, 'utf8'));
  assert.equal(selected.version, expectedVersion);
} finally {
  const exited = child.exitCode === null ? new Promise((resolveExit, reject) => {
    const timer = setTimeout(() => { child.kill(); reject(new Error(`launcher did not exit; ${stderr}`)); }, 10_000);
    child.once('exit', code => { clearTimeout(timer); resolveExit(code); });
  }) : Promise.resolve(child.exitCode);
  child.stdin.end();
  assert.equal(await exited, 0);
}

console.log(JSON.stringify({installation: 'passed', launcher: 'verified', mcp: 'initialized',
  version_switch_while_mcp_connected: 'passed', immutable_running_core: 'write refused',
  architecture: process.arch}));
