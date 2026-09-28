// Development acceptance: the actual packaged bridge + bundled Core, no Host UI claims.
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';
import { mkdtemp, readFile, readdir, rm, writeFile, unlink } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';

const project = await mkdtemp(join(tmpdir(), 'gil-monitor-mcp-check-'));
const settings = await mkdtemp(join(tmpdir(), 'gil-monitor-mcp-settings-'));
let client;
async function connect() {
  client = new Client({ name: 'gil-monitor-check', version: '1' }, { capabilities: {
  extensions: { 'io.modelcontextprotocol/ui': { mimeTypes: ['text/html;profile=mcp-app'] } },
  } });
  const native = process.env.GIL_NATIVE_MCP;
  // Optional macOS acceptance sandbox: deny source/cache/toolchains to the copied binary.
  // A profile is a developer test input, never part of the installed MCP surface.
  const profile = native && process.env.GIL_NATIVE_SANDBOX_PROFILE;
  const transport = new StdioClientTransport({ command: profile ? '/usr/bin/sandbox-exec' : native || process.execPath,
    args: profile ? ['-p', profile, native, 'mcp', '--serve'] : native ? ['mcp', '--serve'] : [join(import.meta.dirname, 'server.mjs')],
    cwd: native ? settings : import.meta.dirname, stderr: 'pipe',
    env: { GIL_MONITOR_STATE_DIR: join(settings, 'bindings'), ...(native ? { PATH: '/usr/bin:/bin' } : {}) } });
  await client.connect(transport); transport.stderr?.resume();
}
async function fingerprint(root) {
  const rows = [];
  async function walk(at, rel = '') {
    for (const entry of await readdir(at, { withFileTypes: true })) {
      const next = join(rel, entry.name), path = join(at, entry.name);
      if (entry.isDirectory()) await walk(path, next);
      else rows.push([next, createHash('sha256').update(await readFile(path)).digest('hex')]);
    }
  }
  await walk(root); return rows.sort((a,b) => a[0].localeCompare(b[0]));
}
const call = async (name, args) => {
  const got = await client.callTool({ name, arguments: args }, undefined, { timeout: 30000 });
  assert.notEqual(got.isError, true, JSON.stringify(got)); return got.structuredContent;
};
try {
  await connect();
  const tools = (await client.listTools()).tools;
  assert.equal(tools.length, 17);
  for (const name of ['gil_start','gil_open','gil_close','gil_restore','gil_revisit']) {
    assert.deepEqual(tools.find(t => t.name === name)._meta.ui.visibility, ['model']);
  }
  await call('gil_start', { project_root: project });
  await call('gil_open', { project_root: project, kind: 'question', contract: 'objective: Monitor 인수\nnext_action: 질문한다\ndone_when: 답을 기록했다' });
  await call('gil_close', { project_root: project, report: 'question: 화면에 나타나는가\nchoices: 예 / 아니오\nresponse: 읽기 전용 인수' });
  const before = await fingerprint(project);
  const prepared = await call('gil_monitor_prepare', { project_root: project });
  assert.deepEqual(Object.keys(prepared).sort(), ['label', 'scope_id']);
  const args = { scope_id: prepared.scope_id };
  const shown = await call('show_gil_monitor', args);
  assert.equal(shown.view.timeline[0].steps[0].state, 'closed');
  const detail = await call('gil_monitor_detail', { ...args, step_ref: 'step:C1/S1' });
  assert.equal(detail.detail.report.fields.find(f => f.name === 'response').value, '읽기 전용 인수');
  const hint = await call('gil_monitor_poll', args);
  assert.equal(hint.view, undefined);
  await call('gil_monitor_read', args);
  const render = tools.find(t => t.name === 'show_gil_monitor');
  const resource = await client.readResource({ uri: render._meta.ui.resourceUri });
  assert.equal(resource.contents[0].mimeType, 'text/html;profile=mcp-app');
  assert.ok(resource.contents[0].text.includes('GIL Monitor'));
  assert.deepEqual(await fingerprint(project), before);
  assert.equal(JSON.stringify([prepared, shown, detail, hint]).includes(project), false);
  // FULL Plugin + Rust child restart, not just a new UI object. Do not call prepare
  // again and do not give the restarted process a project path.
  await client.close();
  await connect();
  const restoredDetail = await call('gil_monitor_detail', { ...args, step_ref: 'step:C1/S1' });
  assert.deepEqual(restoredDetail, detail);
  const restoredHint = await call('gil_monitor_poll', args);
  assert.notEqual(restoredHint.revision, hint.revision, 'process epoch invalidates the cached cursor');
  const restoredView = await call('gil_monitor_read', args);
  assert.deepEqual(restoredView.view.timeline, shown.view.timeline);
  assert.deepEqual(await fingerprint(project), before);
  console.log('Full Plugin restart -> old scope only -> automatic View/Report recovery; new hint epoch; project hashes unchanged PASS');
  const status = await call('gil_status', { project_root: project });
  assert.equal(status.ok, true, 'watcher must not hold the Project lock');
  assert.equal(hint.watching, true, 'real OS watcher must be available in this acceptance run');
  const change = join(project, 'monitor-check.txt');
  await writeFile(change, '사용자가 고친 작업 파일');
  const deadline = Date.now() + 45000;
  let next = restoredHint;
  while (next.revision === restoredHint.revision && Date.now() < deadline) {
    await new Promise(r => setTimeout(r, 200));
    next = await call('gil_monitor_poll', args);
  }
  assert.notEqual(next.revision, restoredHint.revision, 'real filesystem change hint after restart');
  const changed = await call('gil_monitor_read', args);
  assert.equal(changed.view.world.state, 'dirty');
  await unlink(change);
  assert.deepEqual(await fingerprint(project), before);
  console.log('Packaged stdio: 17 tools · Agent walk · prepare/render/read/detail/hint · ui:// resource · all project file hashes unchanged PASS');
  console.log('Real filesystem change -> hint -> complete dirty View, Agent status unblocked PASS');
} finally {
  await client.close();
  // Only the exact temporary project created above; no user project is removed.
  await rm(project, { recursive: true, force: true });
  await rm(settings, { recursive: true, force: true });
}
