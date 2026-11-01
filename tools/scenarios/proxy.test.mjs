import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, writeFileSync, mkdirSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CoreClient } from './lib/rpc.mjs';
import { parseEvents, replay } from './lib/trace.mjs';
import { scoreProcedure } from './lib/score.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const fake = join(here, 'fixtures', 'fake-core.mjs');
const DRAFT = { scope: { goal: 'g', in_scope: 'i', out_of_scope: 'o', constraints: 'c', success_criteria: 's' }, items: [{ id: 'A', question: 'q', done_when: 'd' }], synthesis: { statement: 's', basis_refs: 'step:C1/S1', exploration_checklist: 'A' } };

function rig(synthetic, control = false) {
  const work = mkdtempSync(join(tmpdir(), 'gil-proxy-'));
  const root = join(work, 'project'); mkdirSync(root);
  const scenario = join(work, 'scenario'); mkdirSync(scenario);
  writeFileSync(join(scenario, 'rubric.json'), JSON.stringify({ id: 't', commands: { version: ['node', '--version'] } }));
  const events = join(work, 'events.jsonl');
  const args = [join(here, 'proxy.mjs'), '--core', process.execPath, '--root', root, '--events', events, '--scenario', scenario, '--settings', join(work, 'bindings')];
  if (synthetic) args.push('--synthetic-app-decision', synthetic);
  if (control) { args.splice(args.indexOf('--core'), 2); args.push('--control'); }
  // Spawn the Node fixture directly on both OSes, without a Unix shell wrapper.
  // Production invocation keeps CoreClient's default `mcp --serve` arguments.
  if (!control) args.push('--core-args-json', JSON.stringify([fake]));
  const proxy = new CoreClient({ binary: process.execPath, args, requestTimeoutMs: 5000, label: 'proxy' });
  return { work, root, events, proxy: proxy.start() };
}

test('proxy forwards, augments tools/list, confines paths, hides App-only tools and logs redacted events', async () => {
  const { root, events, proxy } = rig(null);
  try {
    await proxy.initialize();
    const names = (await proxy.request('tools/list', {})).result.tools.map((t) => t.name);
    assert.ok(names.includes('read_project_file') && names.includes('save_project_file') && names.includes('run_scenario_command'));
    assert.ok(names.includes('gil_plan_propose') && !names.includes('gil_plan_review') && !names.includes('gil_plan_decide'), 'App-only tools are hidden from the model-facing list');
    assert.equal((await proxy.call('gil_start', { project_root: root })).ok, true);
    const other = await proxy.call('gil_status', { project_root: join(root, '..') });
    assert.match(other.error.message, /only permits the selected temporary project/);
    const app = await proxy.call('gil_plan_review', { scope_id: 'project:' + 'a'.repeat(64) });
    assert.match(app.error.message, /App-only/);
    assert.equal((await proxy.call('save_project_file', { path: 'evidence/x.md', text: 'hello', item_id: 'A' })).ok, true);
    assert.equal((await proxy.call('save_project_file', { path: 'evidence/x.md', text: 'again' })).ok, false, 'no silent overwrite');
    assert.equal((await proxy.call('save_project_file', { path: 'evidence/x.md', text: 'again', overwrite: true })).ok, true);
    for (const path of ['../out.md', '/abs.md', '.gil/state.yaml', 'a/../../b.md']) assert.equal((await proxy.call('save_project_file', { path, text: 'x' })).ok, false, path);
    const read = await proxy.call('read_project_file', { path: 'evidence/x.md' });
    assert.equal(read.result.content[0].text, 'again');
    const ran = await proxy.call('run_scenario_command', { name: 'version' });
    assert.equal(ran.structured.exit_code, 0);
    assert.equal((await proxy.call('run_scenario_command', { name: 'rm' })).ok, false);
  } finally { assert.equal((await proxy.close()).code, 0); }
  const log = readFileSync(events, 'utf8');
  assert.ok(!log.includes(root), 'absolute project root leaked into the event log');
  const trace = parseEvents(log);
  assert.ok(trace.every((e) => e.actor === 'model'));
  assert.ok(trace.some((e) => e.request.name === 'save_project_file' && e.request.arguments.item_id === 'A'));
});

test('explicit synthetic App decision goes through review+decide as harness input and is labelled, never as the model or a person', async () => {
  const { root, events, proxy } = rig('approve');
  try {
    await proxy.initialize();
    await proxy.call('gil_start', { project_root: root });
    await proxy.call('gil_open', { project_root: root, kind: 'synthesis', contract: 'c' });
    const proposed = await proxy.call('gil_plan_propose', { project_root: root, draft: DRAFT });
    assert.equal(proposed.ok, true);
    const text = proposed.result.content.map((c) => c.text).join('\n');
    assert.match(text, /Synthetic App decision recorded for revision 1: approve/);
    assert.match(text, /not user consent/);
    const read = await proxy.call('gil_plan_read', { project_root: root });
    assert.equal(read.structured.proposal.receipt.decision, 'approve');
    const closed = await proxy.call('gil_close', { project_root: root, report: read.structured.close_report });
    assert.equal(closed.ok, true);
  } finally { await proxy.close(); }
  const trace = parseEvents(readFileSync(events, 'utf8'));
  const harness = trace.filter((e) => e.actor === 'harness');
  assert.deepEqual(harness.map((e) => e.request.name), ['gil_plan_review', 'gil_plan_decide']);
  assert.ok(harness.every((e) => e.synthetic));
  assert.equal(harness[1].request.arguments.token, '<redacted-token>');
  assert.ok(!JSON.stringify(trace).match(/[0-9a-f]{64}"?,"decision/), 'token leaked');
  const judged = Object.fromEntries(scoreProcedure(replay(trace), {}).map((x) => [x.id, x.status]));
  assert.equal(judged.human_approval_real, 'unknown');
});

test('without the flag the proxy records no decision at all', async () => {
  const { root, events, proxy } = rig(null);
  try {
    await proxy.initialize();
    await proxy.call('gil_start', { project_root: root });
    await proxy.call('gil_open', { project_root: root, kind: 'synthesis', contract: 'c' });
    await proxy.call('gil_plan_propose', { project_root: root, draft: DRAFT });
    const read = await proxy.call('gil_plan_read', { project_root: root });
    assert.equal(read.structured.proposal.receipt, null);
  } finally { await proxy.close(); }
  assert.ok(!parseEvents(readFileSync(events, 'utf8')).some((e) => e.actor === 'harness'));
  assert.ok(existsSync(events));
});

test('control arm serves only scenario tools and logs refused GIL calls without any Core', async () => {
  const { root, events, proxy } = rig(null, true);
  try {
    const init = await proxy.initialize();
    assert.equal(init.serverInfo.name, 'gil-scenario-control');
    const names = (await proxy.request('tools/list', {})).result.tools.map((t) => t.name);
    assert.deepEqual(names, ['read_project_file', 'save_project_file', 'run_scenario_command']);
    const gil = await proxy.call('gil_start', { project_root: root });
    assert.match(gil.error.message, /control arm/);
    assert.equal((await proxy.call('save_project_file', { path: 'evidence/r.md', text: 'control result' })).ok, true);
  } finally { assert.equal((await proxy.close()).code, 0); }
  const trace = parseEvents(readFileSync(events, 'utf8'));
  assert.deepEqual(trace.map((e) => e.request.name), ['gil_start', 'save_project_file']);
  assert.ok(trace[0].error);
});
