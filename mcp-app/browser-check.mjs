// A MOCK HOST UI regression check, not proof of Codex/Cowork capability.
// GIL_PLAYWRIGHT_MODULE may point to the developer's bundled Playwright package.
import { createRequire } from 'node:module';
import { readFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { resolve, extname } from 'node:path';
import assert from 'node:assert/strict';
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.GIL_PLAYWRIGHT_MODULE || 'playwright');
const ui = resolve(import.meta.dirname, '../ui');
const html = await readFile(resolve(ui, '../plugins/gil-companion-prototype/assets/monitor.html'), 'utf8');
const view = JSON.parse(await readFile(resolve(ui, 'fixtures/reading-one/view.json'), 'utf8'));
const details = JSON.parse(await readFile(resolve(ui, 'fixtures/reading-one/details.json'), 'utf8'));
details['step:C3/S1'].report.fields.push({ name: 'unsafe', value: '<script>window.evil=1</script>' });
details['step:C3/S1'].report.fields.push({ name: 'long_report', value: Array.from({length:32}, (_,i)=>`보고서 ${i+1}행 — 전체 내용을 보존한다.`).join('\n') });
const server = createServer(async (req, res) => {
  if (req.url === '/') { res.end('<html><body><h1>Mock Host — regression only</h1></body></html>'); return; }
  const path = resolve(ui, '.' + new URL(req.url, 'http://test').pathname);
  if (!path.startsWith(ui + '/')) { res.writeHead(404).end(); return; }
  try {
    const types = { '.js': 'text/javascript', '.css': 'text/css', '.html': 'text/html', '.json': 'application/json' };
    res.setHeader('content-type', types[extname(path)] || 'text/plain'); res.end(await readFile(path));
  } catch { res.writeHead(404).end(); }
});
await new Promise(r => server.listen(0, '127.0.0.1', r));
const browser = await chromium.launch({ headless: true, ...(process.env.GIL_BROWSER_EXECUTABLE ? { executablePath: process.env.GIL_BROWSER_EXECUTABLE } : {}) });
const page = await browser.newPage({ viewport: { width: 1280, height: 1000 } });
const errors = []; page.on('pageerror', e => errors.push(e.message));
const base = `http://127.0.0.1:${server.address().port}`;
try {
  await page.goto(base);
  await page.evaluate(({ html, view, details }) => {
    const scope = 'project:' + 'a'.repeat(64);
    window.requests = []; window.revision = '1'; window.view = view;
    const frame = document.createElement('iframe'); frame.style.cssText = 'width:100%;height:850px;border:0';
    const post = (message) => frame.contentWindow.postMessage({ jsonrpc: '2.0', ...message }, '*');
    const envelope = () => ({ content: [], structuredContent: { scope_id: scope, label: 'UI 회귀 fixture', view: window.view, revision: window.revision, watching: true } });
    window.addEventListener('message', (event) => {
      if (event.source !== frame.contentWindow) return;
      const p = event.data; window.requests.push(p);
      if (p.method === 'ui/initialize') post({ id: p.id, result: {
        protocolVersion: p.params.protocolVersion, hostInfo: { name: 'Mock Host', version: '1' },
        hostCapabilities: { serverTools: {} }, hostContext: { displayMode: 'inline', availableDisplayModes: ['inline', 'fullscreen'] },
      } });
      else if (p.method === 'ui/notifications/initialized') post({ method: 'ui/notifications/tool-result', params: window.replayed || envelope() });
      else if (p.method === 'ui/request-display-mode') {
        const actual = window.forceInline ? 'inline' : p.params.mode;
        post({ id: p.id, result: { mode: actual } });
        post({ method: 'ui/notifications/host-context-changed', params: { displayMode: actual } });
      } else if (p.method === 'tools/call') {
        if (p.params.name === 'show_gil_companion') post({ id: p.id, result: { content: [], structuredContent: { said: 'native fallback requested' } } });
        else if (window.failResume) post({ id: p.id, result: { isError: true, content: [], structuredContent: { code: 'project_missing', said: '시험 폴더 없음' } } });
        else if (p.params.arguments.scope_id !== scope) post({ id: p.id, error: { code: -32602, message: 'scope' } });
        else if (p.params.name === 'gil_monitor_detail') post({ id: p.id, result: { content: [], structuredContent: { scope_id: scope, detail: details[p.params.arguments.step_ref] } } });
        else post({ id: p.id, result: envelope() });
      } else if (p.id !== undefined) post({ id: p.id, result: {} });
    });
    window.contextChange = (params) => post({ method: 'ui/notifications/host-context-changed', params });
    window.replayAfterRestart = () => {
      window.replayed = structuredClone(envelope());
      window.revision = 'new-server:2'; window.view.world.state = 'clean';
      frame.srcdoc = html;
    };
    frame.srcdoc = html; document.body.append(frame);
  }, { html, view, details });
  const frame = page.frameLocator('iframe');
  await frame.locator('[data-step="step:C3/S1"]').waitFor({ timeout: 20000 });
  await frame.locator('[data-step="step:C3/S1"]').click();
  await frame.locator('.detail').getByText('<script>window.evil=1</script>', { exact: true }).waitFor();
  assert.equal(await page.frames()[1].evaluate(() => window.evil), undefined);
  const before = await frame.locator('#host-evidence').textContent();
  const instance = JSON.parse(before).instance;
  await frame.locator('#expand').click();
  await frame.locator('body[data-display-mode="fullscreen"]').waitFor();
  assert.equal(JSON.parse(await frame.locator('#host-evidence').textContent()).instance, instance);
  assert.match(await frame.locator('.detail').innerText(), /step:C3\/S1/);
  // Host-owned composer OUTSIDE the iframe, not extra UI shipped in the App.
  // A full-length Report must scroll completely above this mock overlay.
  const iframeStyle = await page.locator('iframe').getAttribute('style');
  await page.evaluate(() => {
    document.querySelector('iframe').style.cssText='position:fixed;inset:0;width:100%;height:100%;border:0';
    const composer=document.createElement('aside'); composer.id='mock-composer';
    composer.textContent='Host 채팅 입력창 — 겹침 회귀시험용 (실제 Claude 화면 아님)';
    composer.style.cssText='position:fixed;bottom:0;left:0;right:0;background:#e9edf9;border-top:1px solid #8290ad;padding:20px;box-sizing:border-box;font:16px system-ui;z-index:1000';
    document.body.append(composer);
  });
  for (const [width,height,overlay] of [[1280,900,220],[900,650,160],[600,480,160]]) {
    await page.setViewportSize({width,height});
    await page.locator('#mock-composer').evaluate((el, height) => {el.style.height=`${height}px`;},overlay);
    await page.frames()[1].evaluate(async () => {
      await new Promise(done=>requestAnimationFrame(()=>requestAnimationFrame(done)));
      window.scrollTo(0,document.scrollingElement.scrollHeight);
    });
    const last=await frame.locator('.detail dd').last().boundingBox();
    const composer=await page.locator('#mock-composer').boundingBox();
    assert.ok(last.y+last.height<=composer.y-16,`${width}×${height}: Report tail must clear the Host composer`);
    assert.ok(last.y+last.height>20,'Report tail is still in the viewport');
    assert.ok(await page.evaluate(({x,y}) => document.elementFromPoint(x,y)===document.querySelector('iframe'),
      {x:last.x+4,y:last.y+last.height-2}),'Host overlay cannot intercept the last Report line');
    assert.match(await frame.locator('.detail dd').last().innerText(),/보고서 32행/,'Report is not truncated');
    if (width===900 && process.env.GIL_UI_OVERLAY_SCREENSHOT)
      await page.screenshot({path:process.env.GIL_UI_OVERLAY_SCREENSHOT});
  }
  await page.evaluate(style => {
    document.getElementById('mock-composer').remove();
    document.querySelector('iframe').setAttribute('style',style);
  },iframeStyle);
  await page.setViewportSize({width:1280,height:1000});
  await page.frames()[1].evaluate(()=>window.scrollTo(0,0));
  if (process.env.GIL_UI_LAYOUT_SCREENSHOT)
    await frame.locator('.map').screenshot({path:process.env.GIL_UI_LAYOUT_SCREENSHOT});
  console.log('Fullscreen Report clears a 160/220px external mock composer at 1280×900, 900×650, 600×480 PASS');
  await page.evaluate(() => { window.revision = '2'; window.view.world.state = 'dirty'; });
  await frame.locator('.world').filter({ hasText: 'dirty' }).waitFor({ timeout: 12000 });
  assert.match(await frame.locator('.detail').innerText(), /step:C3\/S1/);
  await frame.locator('#native').click();
  await frame.locator('#host-say').filter({ hasText: 'native fallback requested' }).waitFor();
  const screenshot = process.env.GIL_UI_SCREENSHOT || '/private/tmp/gil-monitor-mcp-ui.png';
  await frame.locator('body').screenshot({ path: screenshot });
  await page.evaluate(() => { window.forceInline = true; });
  await frame.locator('#expand').click();
  await frame.locator('body[data-display-mode="inline"]').waitFor();
  assert.equal(await frame.locator('body').evaluate(el=>getComputedStyle(el).paddingBottom),'0px','inline card has no composer tail');
  assert.match(await frame.locator('#host-say').textContent(), /미리보기/);
  await page.evaluate(() => window.contextChange({ availableDisplayModes: ['inline'] }));
  assert.equal(await frame.locator('#expand').isDisabled(), true);
  assert.equal(await frame.locator('#native').isEnabled(), true);
  // The Host replays an OLD tool result after reopening the App, but the new
  // server answers only scope-based reads. Its epoch must replace the stale View.
  await page.evaluate(() => window.replayAfterRestart());
  await frame.locator('.world').filter({ hasText: 'clean' }).waitFor({ timeout: 12000 });
  await frame.locator('[data-step="step:C3/S1"]').click();
  await frame.locator('.detail').getByText('<script>window.evil=1</script>', { exact: true }).waitFor();
  const lastReport = await frame.locator('.detail').innerText();
  await page.evaluate(() => { window.failResume = true; });
  await frame.locator('[data-step="step:C3/S1"]').click();
  await frame.locator('#host-say').filter({ hasText: '프로젝트 폴더를 찾을 수 없습니다' }).waitFor();
  assert.equal(await frame.locator('[data-step="step:C3/S1"]').count(), 1, 'last graph survives');
  await frame.locator('.notice').filter({ hasText: '상세를 읽지 못했다' }).waitFor();
  assert.equal(await frame.locator('.detail').innerText(), lastReport, 'last verified report survives a failed reread of the same step');
  await page.evaluate(() => { window.failResume = false; window.revision = 'new-server:3'; window.view.world.state = 'dirty'; });
  await frame.locator('.world').filter({ hasText: 'dirty' }).waitFor({ timeout: 12000 });
  await frame.locator('.notice[data-shown="false"]').waitFor({ state: 'attached' });
  assert.equal(await frame.locator('.detail').innerText(), lastReport, 'same report is available after automatic recovery');
  const calls = await page.evaluate(() => window.requests.filter(p => p.method === 'tools/call'));
  assert.ok(calls.some(c => c.params.name === 'gil_monitor_read'));
  assert.ok(calls.every(c => !('project_root' in c.params.arguments)));
  console.log(`MCP App mock-Host UI: graph, detail, escaping, fullscreen, live hint, native fallback, old-result replay, restart epoch refresh, reconnect retry, last-report retention and recovery PASS. Screenshot ${screenshot}`);
  assert.deepEqual(errors, []);
  await page.goto(base + '/selftest.html');
  await page.waitForFunction(() => window.GIL_SELFTEST_RESULT, null, { timeout: 120000 });
  const results = await page.evaluate(() => window.GIL_SELFTEST_RESULT);
  const bad = results.filter(r => !r.ok);
  assert.deepEqual(bad, []);
  console.log(`Shared native/fixture UI selftest: ${results.length}/${results.length} PASS`);
} finally { await browser.close(); server.close(); }
