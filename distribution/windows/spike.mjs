// Maintainer-only native protocol check. No package installation or UI acceptance.
// GIL_RUNTIME_PROFILE selects the expected tool contract: default (19) or visual-authoring (24).
import assert from 'node:assert/strict';
import {spawn, spawnSync} from 'node:child_process';
import {createHash, randomUUID} from 'node:crypto';
import {mkdtemp, mkdir, readFile, readdir, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {checkAuthoringTools, checkDefaultTools} from '../codex/tool-contract.mjs';

const binary = resolve(process.argv[2] || '');
assert.ok(process.argv[2], 'Usage: node spike.mjs <native-gil-binary>');
assert.ok(['win32', 'darwin'].includes(process.platform));
assert.ok(['x64', 'arm64'].includes(process.arch), 'Unsupported native test architecture');
const profile = process.env.GIL_RUNTIME_PROFILE || 'default';
assert.ok(['default', 'visual-authoring'].includes(profile), 'Unknown runtime profile');
const state = await mkdtemp(join(tmpdir(), 'gil-native-spike-'));
const root = join(state, '한글 project');
await mkdir(root);
const env = {GIL_MONITOR_STATE_DIR: join(state, 'bindings'), TEMP: state, TMP: state};
if (process.platform === 'win32') {
  assert.ok(process.env.SystemRoot, 'Windows system directory is required');
  env.SystemRoot = process.env.SystemRoot;
  env.PATH = join(env.SystemRoot, 'System32');
  env.LOCALAPPDATA = join(state, 'local-app-data');
} else env.PATH = '/usr/bin:/bin';

// One MCP server process over stdio. Stopping it must not change any Project bytes.
function serve() {
  const child = spawn(binary, ['mcp', '--serve'], {cwd: root, env, stdio: ['pipe', 'pipe', 'pipe']});
  const pending = new Map();
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
  const call = (name, args) => request('tools/call', {name, arguments: args});
  const stop = async () => {
    for (const one of pending.values()) clearTimeout(one.timer);
    if (child.pid && child.exitCode === null && child.signalCode === null) {
      const stopped = new Promise(resolve => child.once('exit', resolve));
      child.kill();
      const timer = setTimeout(() => child.kill('SIGKILL'), 2000);
      await stopped; clearTimeout(timer);
    }
  };
  const initialize = async () => {
    await request('initialize', {protocolVersion: '2025-06-18', capabilities: {},
      clientInfo: {name: 'gil-windows-spike', version: '1'}});
    child.stdin.write(JSON.stringify({jsonrpc: '2.0', method: 'notifications/initialized'}) + '\n');
  };
  return {request, call, stop, initialize, faulted: () => fault};
}

let server;
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
  // The descriptor must say what the build really stores (Migration §5.6): 4..5 only for authoring.
  assert.equal(descriptor.core.action_surface.min, profile === 'visual-authoring' ? 2 : 1);
  assert.equal(descriptor.core.storage_format.max, profile === 'visual-authoring' ? 5 : 4);

  server = serve();
  await server.initialize();
  const {tools} = await server.request('tools/list', {});
  if (profile === 'visual-authoring') checkAuthoringTools(tools); else checkDefaultTools(tools);
  const uri = tools.find(t => t.name === 'show_gil_monitor')._meta.ui.resourceUri;
  const {contents} = await server.request('resources/read', {uri});
  assert.equal(contents[0].mimeType, 'text/html;profile=mcp-app');
  const hash = createHash('sha256').update(contents[0].text).digest('hex');
  assert.equal(uri, `ui://gil-monitor/${hash.slice(0, 20)}.html`);
  const started = await server.call('gil_start', {project_root: root});
  assert.equal(started.structuredContent.ok, true);
  if (profile === 'visual-authoring') {
    assert.equal(started.structuredContent.storage_format, 5);
    assert.equal(started.structuredContent.display_confirmed, false);
  }
  assert.equal((await server.call('gil_status', {project_root: root})).structuredContent.ok, true);
  let statePath = join(root, '.gil', 'state.yaml');
  if (process.platform === 'win32' && profile === 'visual-authoring') {
    const layout = join(env.LOCALAPPDATA, 'GIL', 'project-state-v1');
    const records = (await readdir(layout, {withFileTypes: true})).filter(one => one.isDirectory());
    assert.equal(records.length, 1, 'one isolated external Project record expected');
    statePath = join(layout, records[0].name, '.gil', 'state.yaml');
  }
  const before = await readFile(statePath);
  assert.ok(before.toString('utf8').startsWith(profile === 'visual-authoring' ? 'format: 5\n' : 'format: 4\n'),
    'stored format must match the profile');
  assert.equal((await server.call('gil_start', {project_root: root})).structuredContent.ok, false);
  assert.deepEqual(await readFile(statePath), before);
  const monitor = await server.call('gil_monitor_prepare', {project_root: root});
  assert.notEqual(monitor.isError, true);
  const scope = monitor.structuredContent.scope_id;
  assert.match(scope, /^project:[a-f0-9]{64}$/);
  const view = await server.call('show_gil_monitor', {scope_id: scope});
  assert.notEqual(view.isError, true);
  assert.ok(view.structuredContent.view);
  assert.equal(view.structuredContent.watching, true);
  assert.deepEqual(await readFile(statePath), before);
  if (server.faulted()) throw server.faulted();

  // Process restart: a fresh server reads the same records without recovery or a second start.
  await server.stop();
  server = serve();
  await server.initialize();
  assert.equal((await server.call('gil_status', {project_root: root})).structuredContent.ok, true);
  assert.equal((await server.call('gil_start', {project_root: root})).structuredContent.ok, false);
  const again = await server.call('show_gil_monitor', {scope_id: scope});
  assert.notEqual(again.isError, true);
  assert.equal(again.structuredContent.view.timeline.length, 1);
  assert.equal(again.structuredContent.watching, true);
  assert.deepEqual(await readFile(statePath), before);
  if (server.faulted()) throw server.faulted();
  console.log(JSON.stringify({platform: process.platform, arch: process.arch, profile,
    native_core: 'passed', mcp_tools: tools.length, embedded_ui: 'hash verified',
    stored_format: profile === 'visual-authoring' ? 5 : 4,
    monitor: 'prepare and read-only View passed; OS watcher active',
    restart: 'fresh process read the same records; no second start',
    fullscreen: 'not tested', installation: 'not tested', image_attachment: 'covered by cargo tests, not this smoke'}));
} finally {
  if (server) await server.stop();
  await rm(state, {recursive: true, force: true}); // Only our fresh temporary directory.
}
