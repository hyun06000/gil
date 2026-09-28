// Developer-only differential acceptance. Never shipped as an MCP runtime dependency.
// GIL_JS_BASELINE points at an untouched, previously accepted JS plugin installation.
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';

const baseline = process.env.GIL_JS_BASELINE;
if (!baseline) throw new Error('Set GIL_JS_BASELINE to the accepted JS plugin root');
const binary = resolve(process.env.GIL_NATIVE_MCP || '../../target/debug/gil');
const scratch = await mkdtemp(join(tmpdir(), 'gil-native-parity-'));
const clients = [];
let comparisons = 0;
async function connect(command, args, name) {
  const client = new Client({name: 'native-parity', version: '1'});
  const transport = new StdioClientTransport({command, args, cwd: scratch, stderr: 'pipe',
    env: {GIL_MONITOR_STATE_DIR: join(scratch, `${name}-settings`), PATH: '/usr/bin:/bin'}});
  await client.connect(transport); transport.stderr?.resume(); clients.push(client); return client;
}
try {
  const js = await connect(process.execPath, [join(baseline, 'server.mjs')], 'js');
  const rust = await connect(binary, ['mcp', '--serve'], 'rust');
  const oldTools = (await js.listTools()).tools;
  const newTools = (await rust.listTools()).tools;
  // SDK ToolExecutionSchema explicitly defaults an absent taskSupport to forbidden.
  // rmcp 3.4 no longer serializes this legacy field. Normalize ONLY that default,
  // and fail if either side advertises task execution; do not mask product fields.
  const sorted = tools => tools.map(t => {
    assert.ok(t.execution === undefined || JSON.stringify(t.execution) === '{"taskSupport":"forbidden"}');
    const {execution, ...product} = t; return product;
  }).toSorted((a,b) => a.name.localeCompare(b.name));
  const before = sorted(oldTools), after = sorted(newTools);
  assert.equal(after.length, 17); assert.equal(before.length, 17);
  for (let i = 0; i < after.length; i++) assert.deepEqual(after[i], before[i], `${before[i].name}: full wire contract`);
  const uri = newTools.find(t => t.name === 'show_gil_monitor')._meta.ui.resourceUri;
  assert.deepEqual((await rust.readResource({uri})).contents, (await js.readResource({uri})).contents,
    'same embedded shared UI bytes, MIME and CSP');
  const roots = [join(scratch, 'old'), join(scratch, 'new')];
  for (const root of roots) await mkdir(root);
  const call = async (name, extra = {}, withRoot = true, expectedOk) => {
    const results = [];
    for (const [i, client] of [js, rust].entries()) {
      results.push(await client.callTool({name, arguments: {...(withRoot ? {project_root: roots[i]} : {}), ...extra}}));
    }
    // SDK versions can add protocol envelope metadata. Compare the product contract.
    for (const key of ['structuredContent', 'content', 'isError']) {
      assert.deepEqual(results[1][key], results[0][key], `${name} ${JSON.stringify(extra)}: ${key}`);
    }
    if (expectedOk !== undefined) assert.equal(results[1].structuredContent.ok, expectedOk, name);
    comparisons++;
  };
  await call('gil_help', {topic: 'step/verify/close'}, false, true);
  await call('gil_help', {}, false, true);
  await call('gil_start', {}, true, true);
  await call('gil_start', {}, true, false);
  for (const name of ['gil_status', 'gil_context', 'gil_story', 'gil_help']) await call(name, {}, true, true);
  for (const kind of ['--help', 'line\nbreak', '\u0000', '😀'.repeat(101)]) await call('gil_open', {kind}, true, false);
  await call('gil_cycle', {action: 'invalid'}, true, false);
  await call('gil_open', {kind: 'question'}, true, false);
  await call('gil_close', {}, true, false);
  await call('gil_revisit', {}, true, false);
  const contract = 'objective: 이전 동등성 검수\nnext_action: 확인한다\ndone_when: 보고서가 있다';
  const step = async (kind, report) => {
    await call('gil_open', {kind, contract}, true, true);
    await call('gil_open', {kind, contract}, true, false); // already open
    await call('gil_close', {report: 'invalid: body'}, true, false);
    await call('gil_close', {report}, true, true);
    await call('gil_status', {}, true, true);
  };
  await step('question', 'question: 무엇을 확인하는가\nchoices: 이전 / 유지\nresponse: 동등성');
  await step('interpretation', 'interpretation: 동작을 보존한다\nunresolved: 없음');
  await step('synthesis', 'statement: 같은 동작을 유지한다\nbasis_refs: |\n  step:C1/S1\n  step:C1/S2\napproved: yes');
  await step('outcome', 'verdict: success\nlesson: 입력과 출력이 같다\nsynthesis_ref: step:C1/S3\nnext_direction:\n  action: close_cycle\n  reason: 확인했다');
  await call('gil_close', {report: 'verdict: success\noutcome_ref: step:C1/S4\nhandoff_summary: 실험으로 이어간다\nnext_direction:\n  action: open_child\n  reason: 실험한다'}, true, true);
  await call('gil_cycle', {action: 'open', kind: 'experiment'}, true, true);
  await step('define', 'problem: 표면을 보존하는가\nsuccess_condition: 같은 결과');
  await step('hypothesis', 'hypothesis: 직접 호출도 같다\nrationale: 같은 Core\nguardrail: 기록 보존');
  await call('gil_open', {kind: 'verify', contract}, true, true);
  for (const root of roots) await writeFile(join(root, 'temporary-work.txt'), '검수에서만 만든 파일');
  await call('gil_restore', {}, true, true);
  await call('gil_close', {report: 'execution: 비교했다\nresult: 같지 않았다'}, true, true);
  await step('analysis', 'hypothesis_fit: 어긋났다\nproblem_solved: 아니다\nsuccess_condition_met: 아니다\nguardrail_triggered: 아니다\ninterpretation: 다른 갈래가 필요하다');
  await step('outcome', 'verdict: failure\nlesson: 갈래를 연다\nnext_direction:\n  action: close_cycle\n  reason: 실험 끝');
  await call('gil_cycle', {action: 'close'}, true, false); // absent Report stays absent
  await call('gil_close', {report: 'verdict: failure\noutcome_ref: step:C2/S5\nhandoff_summary: 다시 시작\nnext_direction:\n  action: revisit\n  reason: 다른 가설\n  target_cycle_ref: cycle:C1'}, true, true);
  await call('gil_revisit', {}, true, true);
  for (const name of ['gil_status', 'gil_context', 'gil_story', 'gil_help']) await call(name, {}, true, true);
  await call('gil_open', {kind: 'experiment'}, true, true);
  await call('gil_companion_status', {}, false);
  console.log(`PASS: 17 tool contracts + identical UI resource + ${comparisons} JS/Rust response comparisons; ten actions including restore/revisit; native MCP PATH has no Node`);
} finally {
  for (const client of clients) await client.close();
  await rm(scratch, {recursive: true, force: true}); // only this run's mkdtemp
}
