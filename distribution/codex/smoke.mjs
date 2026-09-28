// Dependency-free maintainer harness. The child gets no inherited credentials or Node PATH.
import assert from 'node:assert/strict';
import {spawn, spawnSync} from 'node:child_process';
import {randomUUID} from 'node:crypto';
import {mkdtemp, readFile, realpath, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {CORE, MANIFEST, PREFIX, checkLibraries, hash, verifyTree} from './artifact.mjs';

export async function smoke(root, expectedHtml, verify = verifyTree) {
  assert.equal(process.platform, 'darwin');
  assert.equal(process.arch, 'arm64');
  await verify(root); // Integrity and the selected channel policy before executing any artifact.
  const state = await realpath(await mkdtemp(join(tmpdir(), 'gil-dist-smoke-')));
  const binary = join(root, CORE);
  const env = {PATH: '/usr/bin:/bin', GIL_MONITOR_STATE_DIR: join(state, 'bindings')};
  let child;
  try {
    const loaded = spawnSync('/usr/bin/otool', ['-L', binary], {encoding: 'utf8', timeout: 10000});
    assert.equal(loaded.status, 0, 'load-command inspection failed');
    const libraries = checkLibraries(loaded.stdout);
    const started = performance.now();
    const challenge = randomUUID();
    const first = spawnSync(binary, ['--gil-agent-probe', challenge], {
      cwd: state, env, encoding: 'utf8', timeout: 30000, maxBuffer: 1024 * 1024,
    });
    assert.equal(first.error, undefined, 'first process failed/timed out; do not retry to hide it');
    assert.equal(first.status, 0, 'native challenge failed');
    const {core, challenge: echoed} = JSON.parse(first.stdout);
    assert.equal(echoed, challenge);
    assert.equal(core.product, 'gil_core');
    assert.equal(core.schema_version, 1);
    assert.equal(core.os, 'macos');
    assert.equal(core.arch, 'aarch64');
    const firstMs = Math.round(performance.now() - started);
    const entry = JSON.parse(await readFile(join(root, MANIFEST))).mcpServers['gil-companion'];
    const cwd = join(root, PREFIX, entry.cwd);
    child = spawn(resolve(cwd, entry.command), entry.args, {cwd, env, stdio: ['pipe', 'pipe', 'pipe']});
    let serial = 0, pending = new Map(), buffer = '', fault;
    const fail = error => {
      fault = error;
      for (const {reject, timer} of pending.values()) { clearTimeout(timer); reject(error); }
      pending.clear();
    };
    child.on('error', () => fail(new Error('native MCP launch failed')));
    child.on('exit', () => fail(new Error('native MCP exited before completion')));
    child.stderr.resume(); // Never copy arbitrary runtime diagnostics or local paths into receipts.
    child.stdin.on('error', () => fail(new Error('native MCP input closed')));
    child.stdout.setEncoding('utf8');
    child.stdout.on('data', chunk => {
      buffer += chunk;
      if (buffer.length > 4 * 1024 * 1024) return fail(new Error('MCP stdout exceeded bound'));
      while (buffer.includes('\n')) {
        const end = buffer.indexOf('\n'), line = buffer.slice(0, end);
        buffer = buffer.slice(end + 1);
        if (!line.trim()) continue;
        try {
          const response = JSON.parse(line);
          assert.equal(response.jsonrpc, '2.0', 'stdout is MCP frames only');
          const waiting = pending.get(response.id);
          if (!waiting) continue;
          pending.delete(response.id); clearTimeout(waiting.timer);
          if (response.error) waiting.reject(new Error('MCP returned an error'));
          else waiting.resolve(response.result);
        } catch { fail(new Error('non-protocol output on MCP stdout')); }
      }
    });
    const request = (method, params) => new Promise((resolve, reject) => {
      if (fault) return reject(fault);
      const id = ++serial;
      const timer = setTimeout(() => {
        pending.delete(id); reject(new Error(`MCP ${method} timed out at 30 seconds; no automatic retry`));
      }, 30000);
      pending.set(id, {resolve, reject, timer});
      child.stdin.write(JSON.stringify({jsonrpc: '2.0', id, method, params}) + '\n');
    });
    const at = performance.now();
    await request('initialize', {protocolVersion: '2025-06-18',
      capabilities: {extensions: {'io.modelcontextprotocol/ui': {mimeTypes: ['text/html;profile=mcp-app']}}},
      clientInfo: {name: 'gil-dist-check', version: '1'}});
    child.stdin.write(JSON.stringify({jsonrpc: '2.0', method: 'notifications/initialized'}) + '\n');
    const initMs = Math.round(performance.now() - at);
    const {tools} = await request('tools/list', {});
    assert.equal(tools.length, 17);
    assert.equal(new Set(tools.map(t => t.name)).size, 17);
    const show = tools.find(t => t.name === 'show_gil_monitor');
    const uri = show?._meta?.ui?.resourceUri;
    assert.match(uri, /^ui:\/\/gil-monitor\/[a-f0-9]{20}\.html$/);
    assert.equal(show._meta['ui/resourceUri'], uri);
    const resource = await request('resources/read', {uri});
    assert.equal(resource.contents.length, 1);
    const html = resource.contents[0];
    assert.equal(html.mimeType, 'text/html;profile=mcp-app');
    assert.equal(html.uri, uri);
    const uiHash = hash(html.text);
    assert.equal(uri, `ui://gil-monitor/${uiHash.slice(0, 20)}.html`);
    if (expectedHtml) assert.equal(uiHash, hash(await readFile(expectedHtml)), 'embedded UI is stale');
    assert.ok(html.text.includes('GIL Monitor'));
    if (fault) throw fault;
    return {passed: true, first_process_ms: firstMs, initialize_ms: initMs, deadline_ms: 30000,
      tools: tools.length, ui_sha256: uiHash, core, system_libraries: libraries, child_path: env.PATH,
      scope: 'protocol only; not marketplace installation, fresh-machine launch or visible fullscreen'};
  } finally {
    if (child?.pid && child.exitCode === null && child.signalCode === null) {
      const stopped = new Promise(resolve => child.once('exit', resolve));
      child.kill('SIGTERM');
      const timer = setTimeout(() => child.kill('SIGKILL'), 2000);
      await stopped; clearTimeout(timer);
    }
    await rm(state, {recursive: true, force: true}); // Exact directory created by this harness only.
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [root, expectedHtml] = process.argv.slice(2);
  assert.ok(root, 'Usage: smoke.mjs <marketplace-root> [expected-built-ui.html]');
  console.log(JSON.stringify(await smoke(resolve(root), expectedHtml), null, 2));
}
