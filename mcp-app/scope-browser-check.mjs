// A MOCK HOST UI regression check of the scope-change panel, not proof of any real Host.
//
// What it loads: the HTML bytes that the tracking-enabled `gil` executable itself returned from
// `resources/read` (written by tests/tracking_scope_change.rs when GIL_SCOPE_MONITOR_HTML_OUT is set).
// GIL_SCOPE_MONITOR_HTML is therefore REQUIRED: this check never falls back to the packaged file, to
// a default-profile binary's resource or to a hand-made DOM. GIL_SCOPE_MONITOR_SHA256, when given,
// must equal the SHA-256 of those bytes (the value the resource smoke linked to the URI).
//
// What it answers with: synthetic records for the App's tool calls. The Core is not running here, so
// this shows what the shipped panel does with those answers — nothing about Core behaviour:
//   1. the scope values are shown next to the graph (applied policy vs confirmed policy, proposal);
//   2. an untrusted click sends nothing;
//   3. while an approval is still running, the halt button is usable and a trusted click sends the
//      halt with its own capability; the halt answer is shown apart from the approval's answer, and
//      a second approval click sends nothing;
//   4. a release is sent only after both explicit inputs, with the shown halt id and state digest;
//   5. after the release the panel re-reads and shows the state the mock then returns;
//   6. no capability ever appears in the DOM.
// GIL_PLAYWRIGHT_MODULE may point to a bundled Playwright package.
import { createRequire } from 'node:module';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { createServer } from 'node:http';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.GIL_PLAYWRIGHT_MODULE || 'playwright');
const source = process.env.GIL_SCOPE_MONITOR_HTML;
assert.ok(source, 'GIL_SCOPE_MONITOR_HTML is required: the HTML returned by the tracking-enabled executable (no packaged-file fallback)');
const bytes = await readFile(resolve(source));
const sha = createHash('sha256').update(bytes).digest('hex');
if (process.env.GIL_SCOPE_MONITOR_SHA256) assert.equal(sha, process.env.GIL_SCOPE_MONITOR_SHA256, 'the HTML is not the bytes the resource smoke linked');
const html = bytes.toString('utf8');
assert.ok(html.includes('id="scope-review"'), 'the returned Monitor has no scope panel — the packaged asset was not regenerated');
const view = JSON.parse(await readFile(resolve(import.meta.dirname, '../ui/fixtures/reading-one/view.json'), 'utf8'));
const scope = 'project:' + 'a'.repeat(64);
const CAPABILITIES = ['PRIVATE-APPROVAL', 'PRIVATE-HALT', 'PRIVATE-RELEASE', 'PRIVATE-HALT-2'];
const tracking = (extra = {}) => ({
  chain: 'chain:C1', standing: { cycle: 'cycle:C1', state: 'closed_boundary' }, applied_policy: 'policy:P1', applied_by: 'event:M1',
  last_confirmed_world: 'snapshot:A1', confirmed_policy: 'policy:P1', basis_agrees: true, waiting: null, archive_halt: null, ...extra,
});
const proposal = {
  revision: 1, digest: 'd'.repeat(64), engine: 'gil-gilignore/1', review_interview: 'cycle:C1', policy_before: 'policy:P1',
  world_before: 'snapshot:A1', confirmed_policy: 'policy:P1', rules: 'scratch/\ndata/\n<b>not markup</b>\n',
  newly_tracked: [], newly_excluded: ['data/raw.csv'], effects: ['effect line'], response: null,
};
const HALT = { at: 'cycle:C1', id: '1'.repeat(32), state_digest: '2'.repeat(64) };
const server = createServer((req, res) => res.end('<html><head><meta charset="utf-8"></head><body><h1>Mock Host — scope change regression only</h1></body></html>'));
await new Promise((r) => server.listen(0, '127.0.0.1', r));
const browser = await chromium.launch({ headless: true, ...(process.env.GIL_BROWSER_EXECUTABLE ? { executablePath: process.env.GIL_BROWSER_EXECUTABLE } : {}) });
const page = await browser.newPage({ viewport: { width: 1280, height: 1100 } });
const errors = []; page.on('pageerror', (e) => errors.push(e.message));
const noCapabilityInDom = async () => {
  const content = await page.frames()[1].content();
  for (const capability of CAPABILITIES) assert.ok(!content.includes(capability), `capability ${capability} never in the DOM`);
};
try {
  await page.goto(`http://127.0.0.1:${server.address().port}`);
  await page.evaluate(({ html, scope, view, proposal, awaiting, HALT, halted, applied }) => {
    // The mock's whole state: which record a review returns, and the approval answer held back.
    window.sent = []; window.stage = 'awaiting'; window.pendingDecision = null;
    const frame = document.createElement('iframe'); frame.style.cssText = 'width:100%;height:1000px;border:0';
    const post = (message) => frame.contentWindow.postMessage({ jsonrpc: '2.0', ...message }, '*');
    const envelope = () => ({ content: [], structuredContent: { scope_id: scope, label: 'scope fixture', view, revision: '1', watching: true,
      scope_review_supported: true } });
    const review = () => {
      if (window.stage === 'awaiting') return { structuredContent: { phase: 'awaiting_approval', said: '최신 제안이 Monitor 승인을 기다린다.', policy_file: 'present',
        tracking: awaiting, proposal, cleanups: ['continue', 'end_unachieved', 'propose_scope_change'], halt_request_pending: false },
        _meta: { 'gil/scopeApprovalToken': 'PRIVATE-APPROVAL', 'gil/scopeHaltToken': 'PRIVATE-HALT' } };
      if (window.stage === 'halted') return { structuredContent: { phase: 'applied', said: '즉시 보관 중단 중이다.', policy_file: 'present', tracking: halted,
        halt: HALT, cleanups: ['continue', 'end_unachieved', 'propose_scope_change'], halt_request_pending: false },
        _meta: { 'gil/scopeReleaseToken': 'PRIVATE-RELEASE' } };
      return { structuredContent: { phase: 'applied', said: '해제 뒤 다시 읽은 상태다.', policy_file: 'present', tracking: applied,
        cleanups: ['continue', 'end_unachieved', 'propose_scope_change'], halt_request_pending: false },
        _meta: { 'gil/scopeHaltToken': 'PRIVATE-HALT-2' } };
    };
    window.addEventListener('message', (event) => {
      if (event.source !== frame.contentWindow) return;
      const p = event.data;
      if (p.method === 'ui/initialize') post({ id: p.id, result: { protocolVersion: p.params.protocolVersion, hostInfo: { name: 'Mock Host', version: '1' },
        hostCapabilities: { serverTools: {} }, hostContext: { displayMode: 'inline', availableDisplayModes: ['inline', 'fullscreen'], locale: 'ko-KR' } } });
      else if (p.method === 'ui/notifications/initialized') post({ method: 'ui/notifications/tool-result', params: envelope() });
      else if (p.method === 'tools/call') {
        const { name, arguments: args } = p.params;
        if (name.startsWith('gil_scope_') && name !== 'gil_scope_review') window.sent.push({ name, args });
        if (name === 'gil_scope_review') post({ id: p.id, result: { content: [], ...review() } });
        else if (name === 'gil_scope_decide') {
          // Held back: the approval stays "running" until the check lets it finish.
          window.pendingDecision = () => post({ id: p.id, result: { content: [], structuredContent: { phase: 'applied', said: '승인을 저장했다.', policy_file: 'present',
            tracking: halted, facts: { approval_response: 'recorded', policy_applied: true, policy_file: 'not_written_halt_requested',
              baseline: { result: 'halted', before: 'observe', failure_recorded: false } }, current_state: { reread: true }, halt: HALT } } });
        } else if (name === 'gil_scope_halt') {
          // Received while the other command holds the project: not applied yet.
          window.stage = 'halted';
          post({ id: p.id, result: { content: [], structuredContent: { said: '중단 요청을 받았다.', halt_request: { received: true, applied: false }, halt_request_pending: true } } });
        } else if (name === 'gil_scope_release') {
          window.stage = 'released';
          post({ id: p.id, result: { content: [], structuredContent: { phase: 'applied', said: '중단을 해제했다.', policy_file: 'present', tracking: applied, current_state: { reread: true } } } });
        } else post({ id: p.id, result: envelope() });
      } else if (p.id !== undefined) post({ id: p.id, result: {} });
    });
    frame.srcdoc = html; document.body.append(frame);
  }, { html, scope, view, proposal, awaiting: tracking(), HALT,
    halted: tracking({ applied_policy: 'policy:P2', applied_by: 'event:M2', basis_agrees: false, archive_halt: 'cycle:C1',
      waiting: { event: 'event:M2', policy: 'policy:P2', world_before: 'snapshot:A1', failed_attempts: [] } }),
    applied: tracking({ applied_policy: 'policy:P2', applied_by: 'event:M2', basis_agrees: false,
      waiting: { event: 'event:M2', policy: 'policy:P2', world_before: 'snapshot:A1', failed_attempts: [] } }) });
  const frame = page.frameLocator('iframe');
  const panel = frame.locator('#scope-review');
  const sent = () => page.evaluate(() => window.sent);

  // 1. Scope values and the exact proposal, as text.
  await panel.getByText('<b>not markup</b>', { exact: false }).waitFor({ timeout: 20000 });
  await panel.getByText('적용 정책: policy:P1', { exact: false }).waitFor();
  await panel.getByText('마지막 확정 세계: snapshot:A1, 확정 정책 policy:P1', { exact: false }).waitFor();
  assert.equal(await panel.locator('b').count(), 0, 'the rules are text, never markup');
  await noCapabilityInDom();

  // 2. An untrusted click sends nothing.
  const approve = panel.getByRole('button', { name: '이 범위 변경 승인' });
  const halt = panel.getByRole('button', { name: '지금 보관과 진행 중단' });
  await approve.dispatchEvent('click'); await halt.dispatchEvent('click');
  assert.deepEqual(await sent(), [], 'untrusted clicks send nothing');

  // 3. Approval in flight → the halt button is still usable, and a trusted click sends the halt.
  await approve.click();
  await page.waitForFunction(() => window.pendingDecision !== null);
  assert.equal(await approve.isDisabled(), true, 'a second approval is not offered while one is running');
  assert.equal(await halt.isEnabled(), true, 'the halt stays available while the approval is running');
  await approve.dispatchEvent('click');
  await halt.click();
  await panel.getByText('중단 요청: 받았다.', { exact: false }).waitFor();
  assert.deepEqual(await sent(), [
    { name: 'gil_scope_decide', args: { scope_id: scope, revision: 1, digest: 'd'.repeat(64), decision: 'approve', token: 'PRIVATE-APPROVAL' } },
    { name: 'gil_scope_halt', args: { scope_id: scope, token: 'PRIVATE-HALT' } },
  ], 'one approval and one halt, each with its own capability');
  await noCapabilityInDom();
  // The approval's own answer arrives afterwards and is shown as its own result (halted, no failure recorded).
  await page.evaluate(() => window.pendingDecision());
  await panel.getByText('cycle:C1 에서 중단됐다.', { exact: false }).waitFor();
  await panel.getByText('적용 정책: policy:P2', { exact: false }).waitFor();
  await panel.getByText('마지막 확정 세계: snapshot:A1, 확정 정책 policy:P1', { exact: false }).waitFor();

  // 4. Release: nothing without both explicit inputs; then exactly the shown halt and state.
  const release = panel.getByRole('button', { name: '중단 해제' });
  await release.click();
  assert.equal((await sent()).length, 2, 'no release without the confirmation and the cleanup choice');
  await panel.locator('#scope-release-confirm').check();
  await release.click();
  assert.equal((await sent()).length, 2, 'no release without the cleanup choice');
  await panel.locator('select').selectOption('continue');
  await release.dispatchEvent('click');
  assert.equal((await sent()).length, 2, 'an untrusted click never releases');
  await release.click();
  await panel.getByText('해제 뒤 다시 읽은 상태다.', { exact: false }).waitFor();
  assert.deepEqual((await sent())[2], { name: 'gil_scope_release', args: { scope_id: scope, at: HALT.at, halt_id: HALT.id, state_digest: HALT.state_digest,
    cleanup: 'continue', state_confirmed: true, token: 'PRIVATE-RELEASE' } });

  // 5. Re-read after the release: the halt is gone, the pending baseline is still shown, and the
  //    release started nothing (no retry, no further request).
  assert.equal(await panel.getByText('에서 중단됐다.', { exact: false }).count(), 0);
  await panel.getByText('기준점 보관 필요 — event:M2', { exact: false }).waitFor();
  assert.equal((await sent()).length, 3, 'the release sent no retry and no other request');
  await noCapabilityInDom();
  if (process.env.GIL_SCOPE_SCREENSHOT) await page.screenshot({ path: process.env.GIL_SCOPE_SCREENSHOT, fullPage: true });
  assert.deepEqual(errors, []);
  console.log(`scope change mock-host check: ok (synthetic Host and synthetic answers; HTML sha256 ${sha}; not a real Host acceptance)`);
} finally { await browser.close(); server.close(); }
