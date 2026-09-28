import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { registerMonitor, monitorBridge } from './monitor.mjs';

test('render tool alone links a content-addressed self-contained MCP App resource', async () => {
  const tools = new Map(), resources = new Map(), called = [];
  const fake = {
    registerTool(name, config, handler) { tools.set(name, { config, handler }); },
    registerResource(name, uri, config, handler) { resources.set(uri, { config, handler }); },
  };
  await registerMonitor(fake, { async call(name, args) { called.push({ name, args }); return { structuredContent: { scope_id: args.scope_id, view: { schema_version: 1 } } }; } });
  assert.equal(tools.size, 5);
  const render = tools.get('show_gil_monitor');
  const uri = render.config._meta.ui.resourceUri;
  assert.equal(render.config._meta['ui/resourceUri'], uri);
  const contents = (await resources.get(uri).handler()).contents[0];
  assert.equal(contents.mimeType, 'text/html;profile=mcp-app');
  const hash = createHash('sha256').update(contents.text).digest('hex');
  assert.equal(uri, `ui://gil-monitor/${hash.slice(0,20)}.html`);
  assert.match(contents.text, /GIL Monitor/);
  assert.doesNotMatch(contents.text, /<script[^>]+src=|<link[^>]+href=|fixtures\/reading-one|\/Users\/davi/);
  assert.deepEqual(contents._meta.ui.csp, { connectDomains: [], resourceDomains: [] });
  assert.deepEqual(tools.get('gil_monitor_prepare').config._meta.ui.visibility, ['model']);
  for (const name of ['gil_monitor_read','gil_monitor_detail','gil_monitor_poll']) {
    assert.deepEqual(tools.get(name).config._meta.ui.visibility, ['app']);
    assert.equal(tools.get(name).config._meta.ui.resourceUri, undefined);
    assert.equal(tools.get(name).config.inputSchema.safeParse({ scope_id: 'project:' + 'a'.repeat(64), project_root: '/private' }).success, false);
  }
  await render.handler({ scope_id: 'project:' + 'a'.repeat(64) });
  assert.equal(called[0].name, 'gil_monitor_read');
});

test('unusable Core and arbitrary tool dispatch fail closed without leaking paths', async () => {
  const bridge = monitorBridge({ probe: async () => { throw new Error('/private/sensitive'); } });
  for (const name of ['gil_monitor_prepare', 'gil_restore']) {
    const result = await bridge.call(name, {});
    assert.equal(result.isError, true);
    assert.doesNotMatch(JSON.stringify(result), /private|sensitive/);
  }
});

test('native renderer remains shared and MCP entry never includes fixture Host', async () => {
  const app = await readFile(new URL('../../mcp-app/app.js', import.meta.url), 'utf8');
  const builder = await readFile(new URL('../../mcp-app/build.mjs', import.meta.url), 'utf8');
  assert.match(app, /import\('\.\.\/ui\/companion\.js'\)/);
  assert.match(builder, /\.\.\/ui\/index\.html/);
  assert.match(builder, /\.\.\/ui\/companion\.css/);
});
