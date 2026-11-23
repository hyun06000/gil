// A MOCK HOST UI regression check of the start preparation panel, not proof of any real Host.
// It loads the built monitor.html in headless Chromium, answers the App's tool calls with synthetic
// records and checks: no graph is drawn before the formal start, the exact rules are shown as text,
// a synthetic (untrusted) click never approves, a trusted click sends the exact revision/digest with
// the capability, and the capability never appears in the DOM.
// GIL_PLAYWRIGHT_MODULE may point to a bundled Playwright package.
import { createRequire } from 'node:module';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { createServer } from 'node:http';
import assert from 'node:assert/strict';
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.GIL_PLAYWRIGHT_MODULE || 'playwright');
const html = await readFile(resolve(import.meta.dirname, '../plugins/gil-companion-prototype/assets/monitor.html'), 'utf8');
const scope = 'project:' + 'a'.repeat(64);
const proposal = {
  revision: 1, digest: 'd'.repeat(64), engine: 'gil-gilignore/1', rules: 'node_modules/\n<b>not markup</b>\n', rules_digest: 'r'.repeat(64),
  tracked_count: 1, tracked: ['README.md'], tracked_truncated: false,
  excluded: [{ path: 'node_modules', kind: 'directory', reason: 'line 1: node_modules/', interior: 'not_explored' }], excluded_count: 1,
  blockers: [], notices: [], recommendations: [], policy_file: { disk: 'absent', effect: 'create' }, impact: ['impact line'],
  system_protections: [{ code: 'git_metadata', shown: '.git/', selectable: false }], combination: null, response: null, approvable: true,
};
// The same mock-host origin as browser-check.mjs: an http page, not about:blank.
const server = createServer((req, res) => res.end('<html><head><meta charset="utf-8"></head><body><h1>Mock Host — start preparation regression only</h1></body></html>'));
await new Promise((r) => server.listen(0, '127.0.0.1', r));
const browser = await chromium.launch({ headless: true, ...(process.env.GIL_BROWSER_EXECUTABLE ? { executablePath: process.env.GIL_BROWSER_EXECUTABLE } : {}) });
const page = await browser.newPage({ viewport: { width: 1100, height: 900 } });
const errors = []; page.on('pageerror', (e) => errors.push(e.message));
try {
  await page.goto(`http://127.0.0.1:${server.address().port}`);
  await page.evaluate(({ html, scope, proposal }) => {
    window.requests = []; window.decided = [];
    const frame = document.createElement('iframe'); frame.style.cssText = 'width:100%;height:800px;border:0';
    const post = (message) => frame.contentWindow.postMessage({ jsonrpc: '2.0', ...message }, '*');
    window.addEventListener('message', (event) => {
      if (event.source !== frame.contentWindow) return;
      const p = event.data; window.requests.push(p);
      if (p.method === 'ui/initialize') post({ id: p.id, result: { protocolVersion: p.params.protocolVersion, hostInfo: { name: 'Mock Host', version: '1' },
        hostCapabilities: { serverTools: {} }, hostContext: { displayMode: 'inline', availableDisplayModes: ['inline', 'fullscreen'], locale: 'ko-KR' } } });
      else if (p.method === 'ui/notifications/initialized') post({ method: 'ui/notifications/tool-result', params: { content: [], structuredContent: {
        scope_id: scope, label: 'start fixture', start_review_supported: true, start_preparation: { phase: 'awaiting_approval' }, revision: '1', watching: false } } });
      else if (p.method === 'tools/call' && p.params.name === 'gil_start_review') post({ id: p.id, result: { content: [], structuredContent: {
        phase: 'awaiting_approval', said: '최신 제안이 사용자의 Monitor 승인을 기다린다.', proposal }, _meta: { 'gil/startApprovalToken': 'PRIVATE-CAPABILITY' } } });
      else if (p.method === 'tools/call' && p.params.name === 'gil_start_decide') {
        window.decided.push(p.params.arguments);
        post({ id: p.id, result: { content: [], structuredContent: { phase: 'started', said: '시작했다', started: { policy: 'policy:P1', event: 'event:M1', snapshot: 'snapshot:A1', chain: 'chain:C1', first_interview: 'cycle:C1', policy_file_sync_base: 'present' } } } });
      } else if (p.id !== undefined) post({ id: p.id, result: {} });
    });
    frame.srcdoc = html; document.body.append(frame);
  }, { html, scope, proposal });
  const frame = page.frameLocator('iframe');
  const panel = frame.locator('#start-review');
  await panel.getByText("<b>not markup</b>", { exact: false }).waitFor({ timeout: 20000 });
  assert.equal(await frame.locator('[data-step]').count(), 0, 'no graph before the formal start');
  assert.ok(!(await page.frames()[1].content()).includes('PRIVATE-CAPABILITY'), 'capability never in the DOM');
  const approve = panel.getByRole('button', { name: '이 추적 범위 승인' });
  await approve.dispatchEvent('click'); // synthetic, untrusted
  assert.equal(await page.evaluate(() => window.decided.length), 0, 'an untrusted click never approves');
  await approve.click(); // trusted input from Playwright
  await panel.getByText('policy policy:P1').waitFor();
  const decided = await page.evaluate(() => window.decided);
  assert.deepEqual(decided, [{ scope_id: scope, revision: 1, digest: 'd'.repeat(64), token: 'PRIVATE-CAPABILITY', decision: 'approve' }]);
  assert.ok(!(await page.frames()[1].content()).includes('PRIVATE-CAPABILITY'));
  await page.screenshot({ path: process.env.GIL_START_SCREENSHOT || '/dev/null', fullPage: true });
  assert.deepEqual(errors, []);
  console.log('start preparation mock-host check: ok (synthetic Host, not a real Host acceptance)');
} finally { await browser.close(); server.close(); }
