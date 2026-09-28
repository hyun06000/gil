// Transitional MCP Apps adapter. Canonical facts are read by the bundled Rust MCP,
// not CLI prose parsing or a second .gil implementation. No loopback HTTP server.
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';
import { readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { z } from 'zod';
import { probeCore } from './core.mjs';

const READ_TOOLS = ['gil_monitor_prepare', 'gil_monitor_read', 'gil_monitor_detail', 'gil_monitor_poll'];
const refuse = () => ({ isError: true, structuredContent: { code: 'monitor_unavailable',
  said: 'MCP Monitor 에 연결하지 못했다. Plugin 을 갱신하거나 별도 Companion 창을 사용한다.' },
  content: [{ type: 'text', text: 'MCP Monitor 연결 불가 — 마지막 화면은 보존한다.' }] });

export function monitorBridge({ probe = probeCore, createClient = () => new Client({ name: 'gil-monitor-adapter', version: '0.1.0' }) } = {}) {
  let connecting = null;
  async function connection() {
    if (!connecting) {
      connecting = (async () => {
        const seen = await probe();
        if (seen.state !== 'ready') throw new Error('core unavailable');
        const client = createClient();
        const transport = new StdioClientTransport({ command: seen.exe, args: ['mcp', '--serve'], stderr: 'pipe',
          // The SDK supplies its normal safe environment. Forward only this explicit
          // developer-test override, never all of process.env (which may contain secrets).
          env: process.env.GIL_MONITOR_STATE_DIR ? { GIL_MONITOR_STATE_DIR: process.env.GIL_MONITOR_STATE_DIR } : {},
        });
        try {
          await client.connect(transport);
          // Diagnostics may contain paths; drain but never send them into the Host or App.
          transport.stderr?.resume();
          const listed = await client.listTools();
          if (!READ_TOOLS.every((name) => listed.tools.some((tool) => tool.name === name))) throw new Error('old core');
          client.onclose = () => { connecting = null; };
          return client;
        } catch (err) { await client.close().catch(() => {}); throw err; }
      })().catch((err) => { connecting = null; throw err; });
    }
    return connecting;
  }
  return {
    async call(name, args) {
      if (!READ_TOOLS.includes(name)) return refuse();
      try { return await (await connection()).callTool({ name, arguments: args }, undefined, { timeout: 30_000 }); }
      catch { return refuse(); }
    },
    async close() {
      const active = connecting; connecting = null;
      if (active) await (await active.catch(() => null))?.close();
    },
  };
}

export async function registerMonitor(server, bridge = monitorBridge()) {
  const asset = JSON.parse(await readFile(new URL('./assets/monitor.json', import.meta.url), 'utf8'));
  const html = await readFile(new URL('./assets/monitor.html', import.meta.url), 'utf8');
  if (createHash('sha256').update(html).digest('hex') !== asset.sha256) throw new Error('Monitor bundle digest mismatch');
  const uri = asset.resource_uri;
  const scope = { scope_id: z.string().regex(/^project:[0-9a-f]{64}$/) };
  const annotations = { readOnlyHint: true, destructiveHint: false, openWorldHint: false };
  server.registerResource('gil-monitor', uri, { title: 'GIL Monitor', mimeType: 'text/html;profile=mcp-app' }, async () => ({
    contents: [{ uri, mimeType: 'text/html;profile=mcp-app', text: html,
      _meta: { ui: { prefersBorder: true, csp: { connectDomains: [], resourceDomains: [] } } } }],
  }));
  server.registerTool('gil_monitor_prepare', {
    description: 'Prepare the read-only GIL Monitor for an explicitly selected project root and remember that choice in local GIL settings for automatic restart recovery. Then call show_gil_monitor with the returned scope_id. Never guess the root. Does not display an App or write the project.',
    inputSchema: z.object({ project_root: z.string() }).strict(), annotations,
    _meta: { ui: { visibility: ['model'] } },
  }, (args) => bridge.call('gil_monitor_prepare', args));
  server.registerTool('show_gil_monitor', {
    title: 'GIL Monitor',
    description: 'Show the GIL MCP App for a prepared scope. It defaults to horizontal layout and requests fullscreen once when ready and supported; if it stays inline, the user can click 모니터 펼치기. A returned View is not proof of display or persistent success. Keep the native Companion fallback available.',
    inputSchema: z.object(scope).strict(), annotations,
    // Same normalized metadata as registerAppTool and the successful Desktop
    // direct-check bundle. Both keys refer to the one content-addressed resource.
    _meta: { ui: { resourceUri: uri, visibility: ['model', 'app'] }, 'ui/resourceUri': uri },
  }, async (args) => {
    const got = await bridge.call('gil_monitor_read', args);
    if (got.isError) return got;
    return { ...got, content: [{ type: 'text', text: 'GIL Monitor 데이터를 준비했다. App은 가로보기로 시작하고 지원되는 Host에 fullscreen을 한 번 자동 요청한다. 그대로라면 모니터 펼치기를 누른다. 이 응답만으로 화면 표시나 지속 표시 성공을 판정하지 않는다.' }] };
  });
  for (const name of ['gil_monitor_read', 'gil_monitor_detail', 'gil_monitor_poll']) {
    server.registerTool(name, {
      description: name === 'gil_monitor_poll' ? 'Read a change hint and renew the visible Monitor watch lease; no project scan.'
        : 'Read canonical GIL Monitor data. No write, recovery, path or layout operation.',
      inputSchema: z.object(name === 'gil_monitor_detail' ? { ...scope, step_ref: z.string() } : scope).strict(), annotations,
      _meta: { ui: { visibility: ['app'] } },
    }, (args) => bridge.call(name, args));
  }
  return bridge;
}
