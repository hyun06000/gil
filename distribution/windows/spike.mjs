// Maintainer-only native protocol check. No package installation or UI acceptance.
import assert from 'node:assert/strict';
import {spawn, spawnSync} from 'node:child_process';
import {createHash, randomUUID} from 'node:crypto';
import {mkdtemp, mkdir, readFile, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';

const binary = resolve(process.argv[2] || '');
assert.ok(process.argv[2], 'Usage: node spike.mjs <native-gil-binary>');
assert.ok(['win32', 'darwin'].includes(process.platform));
const state = await mkdtemp(join(tmpdir(), 'gil-native-spike-'));
const root = join(state, '한글 project');
await mkdir(root);
const env = {GIL_MONITOR_STATE_DIR: join(state, 'bindings'), TEMP: state, TMP: state};
if (process.platform === 'win32') {
  assert.ok(process.env.SystemRoot, 'Windows system directory is required');
  env.SystemRoot = process.env.SystemRoot;
  env.PATH = join(env.SystemRoot, 'System32');
} else env.PATH = '/usr/bin:/bin';
let child;
const pending = new Map();
try {
  const challenge = randomUUID();
  const probe = spawnSync(binary, ['--gil-agent-probe', challenge],
    {cwd: root, env, encoding: 'utf8', timeout: 30000, maxBuffer: 1048576});
  assert.equal(probe.status, 0, 'native Core probe failed');
  const descriptor = JSON.parse(probe.stdout);
  assert.equal(descriptor.challenge, challenge);
  assert.equal(descriptor.core.product, 'gil_core');
  assert.equal(descriptor.core.os, process.platform === 'win32' ? 'windows' : 'macos');
  assert.equal(descriptor.core.arch, process.arch === 'x64' ? 'x86_64' : 'aarch64');
  child = spawn(binary, ['mcp', '--serve'], {cwd: root, env, stdio: ['pipe', 'pipe', 'pipe']});
  let serial = 0, buffer = '', fault;
  const fail = error => {
    fault = error;
    for (const one of pending.values()) { clearTimeout(one.timer); one.reject(error); }
    pending.clear();
  };
  child.on('error', () => fail(new Error('MCP launch failed')));
  child.on('exit', () => fail(new Error('MCP exited')));
  child.stdin.on('error', () => fail(new Error('MCP input closed')));
  child.stderr.resume(); // Do not publish arbitrary diagnostics or machine paths.
  child.stdout.setEncoding('utf8');
  child.stdout.on('data', chunk => {
    buffer += chunk;
    if (buffer.length > 4194304) return fail(new Error('MCP frame too large'));
    while (buffer.includes('\n')) {
      const end = buffer.indexOf('\n'), line = buffer.slice(0, end);
      buffer = buffer.slice(end + 1);
      try {
        const response = JSON.parse(line);
        assert.equal(response.jsonrpc, '2.0');
        const one = pending.get(response.id);
        if (!one) continue;
        pending.delete(response.id); clearTimeout(one.timer);
        if (response.error) one.reject(new Error('MCP request rejected'));
        else one.resolve(response.result);
      } catch { fail(new Error('Non-protocol stdout')); }
    }
  });
  const request = (method, params) => new Promise((resolve, reject) => {
    if (fault) return reject(fault);
    const id = ++serial;
    const timer = setTimeout(() => {
      pending.delete(id); reject(new Error(`Timeout: ${method}`));
    }, 30000);
    pending.set(id, {resolve, reject, timer});
    child.stdin.write(JSON.stringify({jsonrpc: '2.0', id, method, params}) + '\n');
  });
  await request('initialize', {protocolVersion: '2025-06-18', capabilities: {},
    clientInfo: {name: 'gil-windows-spike', version: '1'}});
  child.stdin.write(JSON.stringify({jsonrpc: '2.0', method: 'notifications/initialized'}) + '\n');
  const {tools} = await request('tools/list', {});
  assert.equal(tools.length, 17);
  const uri = tools.find(t => t.name === 'show_gil_monitor')._meta.ui.resourceUri;
  const {contents} = await request('resources/read', {uri});
  assert.equal(contents[0].mimeType, 'text/html;profile=mcp-app');
  const hash = createHash('sha256').update(contents[0].text).digest('hex');
  assert.equal(uri, `ui://gil-monitor/${hash.slice(0, 20)}.html`);
  const call = (name, args) => request('tools/call', {name, arguments: args});
  const started = await call('gil_start', {project_root: root});
  assert.equal(started.structuredContent.ok, true);
  assert.equal((await call('gil_status', {project_root: root})).structuredContent.ok, true);
  const before = await readFile(join(root, '.gil', 'state.yaml'));
  assert.equal((await call('gil_start', {project_root: root})).structuredContent.ok, false);
  assert.deepEqual(await readFile(join(root, '.gil', 'state.yaml')), before);
  const monitor = await call('gil_monitor_prepare', {project_root: root});
  assert.notEqual(monitor.isError, true);
  const scope = monitor.structuredContent.scope_id;
  assert.match(scope, /^project:[a-f0-9]{64}$/);
  const view = await call('show_gil_monitor', {scope_id: scope});
  assert.notEqual(view.isError, true);
  assert.ok(view.structuredContent.view);
  assert.equal(view.structuredContent.watching, true);
  assert.deepEqual(await readFile(join(root, '.gil', 'state.yaml')), before);
  if (fault) throw fault;
  console.log(JSON.stringify({platform: process.platform, arch: process.arch,
    native_core: 'passed', mcp_tools: tools.length, embedded_ui: 'hash verified',
    monitor: 'prepare and read-only View passed; OS watcher active',
    fullscreen: 'not tested', installation: 'not tested'}));
} finally {
  for (const one of pending.values()) clearTimeout(one.timer);
  if (child?.pid && child.exitCode === null && child.signalCode === null) {
    const stopped = new Promise(resolve => child.once('exit', resolve));
    child.kill();
    const timer = setTimeout(() => child.kill('SIGKILL'), 2000);
    await stopped; clearTimeout(timer);
  }
  await rm(state, {recursive: true, force: true}); // Only our fresh temporary directory.
}
