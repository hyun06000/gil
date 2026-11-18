// Maintainer check of an explicitly selected executable/installed MSIX alias. Node is a test
// dependency only. No install, Host settings edits, project creation or visible UI acceptance.
import assert from 'node:assert/strict';
import {spawn, spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {mkdtemp, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';

export async function smoke(binary, version, {prefix = [], timeoutMs = 30000, extraEnv = {}} = {}) {
  assert.ok(binary && version, 'binary and expected version are required');
  const scratch = await mkdtemp(join(tmpdir(), 'gil-msix-smoke-'));
  const env = {...extraEnv, GIL_MONITOR_STATE_DIR: join(scratch, 'bindings')};
  for (const key of ['SystemRoot', 'SystemDrive', 'USERPROFILE', 'LOCALAPPDATA', 'APPDATA', 'TEMP', 'TMP', 'HOME']) {
    if (process.env[key]) env[key] = process.env[key];
  }
  env.PATH = process.platform === 'win32' ? join(env.SystemRoot, 'System32') : '/usr/bin:/bin';
  const options = {cwd: scratch, env, stdio: ['pipe', 'pipe', 'pipe']};
  let child, closed;
  const pending = new Map();
  let fault, buffer = '', serial = 0, timer;
  const fail = error => {
    fault ??= error;
    for (const waiter of pending.values()) { clearTimeout(waiter.timer); waiter.reject(error); }
    pending.clear();
  };
  try {
    const cli = args => {
      const got = spawnSync(binary, [...prefix, ...args], {...options, encoding: 'utf8', timeout: timeoutMs, maxBuffer: 4 * 1024 * 1024});
      assert.ifError(got.error);
      assert.equal(got.status, 0, `CLI ${args.join(' ') || '(no arguments)'} must exit 0`);
      assert.equal(got.stderr, '', 'CLI stderr must be empty');
      return got.stdout.trim();
    };
    assert.equal(cli(['--version']), `gil ${version}`);
    const help = cli(['--help']);
    assert.ok(help.length > 0, 'help must be nonempty');
    assert.equal(cli([]), help, 'no arguments must print help and exit normally');
    child = spawn(binary, [...prefix, 'mcp', '--serve'], options);
    closed = new Promise(resolve => child.once('close', (code, signal) => {
      fail(new Error(`MCP closed (code=${code}, signal=${signal})`));
      resolve({code, signal});
    }));
    child.on('error', error => fail(error));
    child.stdin.on('error', error => fail(error));
    child.stderr.resume(); // Runtime stderr may contain user paths; do not publish it.
    child.stdout.setEncoding('utf8');
    child.stdout.on('data', chunk => {
      buffer += chunk;
      if (buffer.length > 4 * 1024 * 1024) return fail(new Error('MCP stdout exceeded frame limit'));
      while (buffer.includes('\n')) {
        const end = buffer.indexOf('\n'), line = buffer.slice(0, end);
        buffer = buffer.slice(end + 1);
        try {
          const frame = JSON.parse(line);
          assert.equal(frame.jsonrpc, '2.0');
          const waiter = pending.get(frame.id);
          if (!waiter) continue;
          pending.delete(frame.id); clearTimeout(waiter.timer);
          if (frame.error || !('result' in frame)) waiter.reject(new Error('MCP returned an error or missing result'));
          else waiter.resolve(frame.result);
        } catch { fail(new Error('non-protocol output on MCP stdout')); }
      }
    });
    const send = frame => child.stdin.write(`${JSON.stringify(frame)}\n`);
    const request = (method, params) => new Promise((resolve, reject) => {
      if (fault) return reject(fault);
      const id = ++serial;
      const timer = setTimeout(() => { pending.delete(id); reject(new Error(`MCP ${method} timed out`)); }, timeoutMs);
      pending.set(id, {resolve, reject, timer});
      send({jsonrpc: '2.0', id, method, params});
    });
    const init = await request('initialize', {protocolVersion: '2025-06-18', capabilities: {},
      clientInfo: {name: 'gil-msix-check', version: '1'}});
    assert.equal(init.serverInfo?.version, version, 'MCP must be the same build as CLI');
    send({jsonrpc: '2.0', method: 'notifications/initialized'});
    const {tools} = await request('tools/list', {});
    for (const name of ['gil_start', 'gil_status', 'show_gil_monitor']) {
      assert.ok(tools?.some(t => t.name === name), `missing tool: ${name}`);
    }
    const uri = tools.find(t => t.name === 'show_gil_monitor')._meta?.ui?.resourceUri;
    assert.match(uri ?? '', /^ui:\/\/gil-monitor\/[a-f0-9]{20}\.html$/);
    const {contents} = await request('resources/read', {uri});
    assert.equal(contents?.length, 1);
    assert.equal(contents[0].uri, uri);
    assert.equal(contents[0].mimeType, 'text/html;profile=mcp-app');
    const digest = createHash('sha256').update(contents[0].text).digest('hex');
    assert.equal(uri, `ui://gil-monitor/${digest.slice(0, 20)}.html`);
    if (fault) throw fault;
    // EOF is a normal Host disconnect, not a reason to leave a server running.
    child.stdin.end();
    const exit = await Promise.race([closed, new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error('MCP did not exit after stdin EOF')), timeoutMs);
    })]);
    clearTimeout(timer);
    assert.deepEqual(exit, {code: 0, signal: null}, 'MCP EOF must exit 0');
    assert.equal(buffer, '', 'no incomplete stdout frame at EOF');
    // close itself records a fault; protocol faults before close must still fail.
    assert.match(fault?.message ?? '', /^MCP closed \(code=0, signal=null\)$/);
    return {passed: true, version, cli: 'version, help, no arguments: exit 0',
      mcp: 'initialize, tools/list, Monitor resource hash, clean EOF', ui_sha256: digest,
      scope: 'process/protocol only; no actual Host UI, WACK or Store certification'};
  } finally {
    clearTimeout(timer);
    for (const waiter of pending.values()) clearTimeout(waiter.timer);
    if (child?.pid && child.exitCode === null && child.signalCode === null) {
      child.kill('SIGKILL');
      await closed;
    }
    await rm(scratch, {recursive: true, force: true});
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [binary, version] = process.argv.slice(2);
  console.log(JSON.stringify(await smoke(binary && resolve(binary), version), null, 2));
}
