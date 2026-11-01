#!/usr/bin/env node
// Wave 1 E: real native `gil mcp --serve` approval-boundary regression over stdio JSON-RPC.
//
// Run inside the shared build queue so the Core built here is the one that runs here:
//   python3 coordination/parallel-wave-1/cargo-queue.py -- node tools/scenarios/wire-runner.mjs --build
// The runner never guesses an installed Core from PATH/GIL_BINARY. It builds the current branch
// (CARGO_TARGET_DIR comes from the queue) and runs that exact executable, or takes --gil <path>.
// It creates and touches only its own temporary folder, redacts tokens/paths in saved frames,
// enforces per-request timeouts, and kills its child process groups on exit.
import { mkdtemp, mkdir, writeFile, readFile, rm, cp } from 'node:fs/promises';
import { existsSync, readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { tmpdir } from 'node:os';
import { join, resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CoreClient, runCommand } from './lib/rpc.mjs';
import { makeRedactor, leaks } from './lib/redact.mjs';
import { formatReport } from './lib/report.mjs';
import { parseEvents, replay } from './lib/trace.mjs';
import { scoreProcedure, summarize } from './lib/score.mjs';
import { table } from './lib/markdown.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, '..', '..');
const DEFAULT_FEATURES = 'approved-plan,visual-authoring,product-safety';
const STATE_PATH = '.gil/state.yaml';

function parseArgs(argv) {
  const o = { build: false, gil: null, out: null, features: DEFAULT_FEATURES, keep: false, unqueued: false, timeoutMs: 20000 };
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === '--build') o.build = true;
    else if (a === '--keep') o.keep = true;
    else if (a === '--unqueued') o.unqueued = true;
    else if (a === '--gil') o.gil = resolve(argv[++i]);
    else if (a === '--out') o.out = resolve(argv[++i]);
    else if (a === '--features') o.features = argv[++i];
    else if (a === '--timeout-ms') o.timeoutMs = Number(argv[++i]);
    else if (a === '--help' || a === '-h') { console.log(readFileSync(fileURLToPath(import.meta.url), 'utf8').split('\n').slice(1, 10).join('\n')); process.exit(0); }
    else throw new Error(`unknown argument ${a}`);
  }
  if (o.build === !!o.gil) throw new Error('pass exactly one of --build or --gil <path>');
  return o;
}

class Refused extends Error {}
class Blocked extends Error {}
const expect = (cond, msg) => { if (!cond) throw new Error(msg); };

async function buildCore(features, unqueued) {
  if (!process.env.GIL_PARALLEL_TARGET_DIR && !unqueued) {
    throw new Error('refusing to build outside cargo-queue.py (GIL_PARALLEL_TARGET_DIR is unset). Run: python3 coordination/parallel-wave-1/cargo-queue.py -- node tools/scenarios/wire-runner.mjs --build  (or --unqueued where no shared queue exists)');
  }
  const args = ['build', '--locked', '-p', 'gil', '--features', features, '--message-format=json-render-diagnostics'];
  process.stderr.write(`[wire] cargo ${args.join(' ')} (CARGO_TARGET_DIR=${process.env.CARGO_TARGET_DIR || 'default'})\n`);
  const r = await runCommand('cargo', args, { cwd: repo, timeoutMs: 1800000, onStderr: (s) => process.stderr.write(s) });
  if (r.timedOut || r.code !== 0) throw new Error(`cargo build failed (exit ${r.code}${r.timedOut ? ', timeout' : ''})`);
  let executable = null;
  for (const line of r.stdout.split('\n')) {
    if (!line.startsWith('{')) continue;
    try {
      const m = JSON.parse(line);
      if (m.reason === 'compiler-artifact' && m.target?.name === 'gil' && (m.target.kind || []).includes('bin') && m.executable) executable = m.executable;
    } catch {}
  }
  if (!executable) throw new Error('cargo did not report the gil executable');
  return executable;
}

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  const startedAt = new Date();
  const binary = opts.gil ?? await buildCore(opts.features, opts.unqueued);
  expect(existsSync(binary), `binary not found: ${binary}`);
  const binarySha = createHash('sha256').update(await readFile(binary)).digest('hex');
  const head = (await runCommand('git', ['rev-parse', 'HEAD'], { cwd: repo, timeoutMs: 10000 })).stdout.trim() || 'unknown';
  const work = await mkdtemp(join(tmpdir(), 'gil-wave1-wire-'));
  const root = join(work, 'project');
  const settings = join(work, 'settings');
  await mkdir(root); await mkdir(settings);
  const out = opts.out ?? join(work, 'out');
  await mkdir(out, { recursive: true });
  const typedVerify = opts.features.split(',').includes('visual-authoring');

  const redactor = makeRedactor({ roots: { project_root: root, settings_dir: settings, work_dir: work, binary_path: dirname(binary), repo }, secrets: [] });
  const secrets = [];
  const frames = [];
  const events = [];
  let seq = 0;
  const cases = [];
  const notes = [];
  const note = (text) => { notes.push(text); process.stderr.write(`[wire] note: ${text}\n`); };
  const status = Object.create(null);
  let core = null;
  let generation = 0;

  const onFrame = ({ method, params, frame, ms, unsolicited }) => {
    if (unsolicited) { frames.push({ generation, unsolicited: redactor.redact(unsolicited) }); return; }
    frames.push({ generation, method, params: redactor.redact(params), response: redactor.redact(frame), ms });
  };
  const connect = async () => {
    generation += 1;
    core = new CoreClient({ binary, settingsDir: join(settings, 'bindings'), requestTimeoutMs: opts.timeoutMs, onFrame, label: `core#${generation}` }).start();
    const init = await core.initialize();
    return init;
  };
  const record = (actor, name, args, r, synthetic = false) => {
    seq += 1;
    events.push({ seq, at: new Date().toISOString(), actor, synthetic, request: { name, arguments: redactor.redact(args) }, result: r.result ? redactor.redact(r.result) : null, error: r.error ? redactor.redact(r.error) : null });
  };
  const model = async (name, args) => { const r = await core.call(name, args); record('model', name, args, r); return r; };
  const app = async (name, args) => { const r = await core.call(name, args); record('harness', name, args, r, true); return r; };
  const ok = (r, what) => { expect(r.ok, `${what}: expected success, got ${r.error ? `JSON-RPC error ${JSON.stringify(redactor.redact(r.error))}` : `refusal: ${redactor.text(said(r)).slice(0, 400)}`}`); return r; };
  const refused = (r, what) => { expect(!r.error, `${what}: expected a Core refusal, got JSON-RPC error ${JSON.stringify(redactor.redact(r.error))}`); expect(!r.ok, `${what}: expected a refusal but the call succeeded`); return r; };
  const rpcError = (r, what) => { expect(r.error, `${what}: expected a JSON-RPC schema error, got ${r.ok ? 'success' : 'a Core refusal'}`); return r; };
  const said = (r) => r.structured?.said || r.result?.content?.map((c) => c.text || '').join('\n') || '';
  const state = () => { try { return readFileSync(join(root, STATE_PATH)); } catch { return null; } };
  const sameState = (a, b) => a && b ? a.equals(b) : null;
  const serial = (v) => JSON.stringify(v ?? null);

  async function run(id, title, deps, fn) {
    const blockedBy = deps.filter((d) => status[d] !== 'pass');
    const entry = { id, title, depends_on: deps, status: 'not_run', detail: '', started: new Date().toISOString() };
    cases.push(entry);
    if (blockedBy.length) { entry.detail = `prerequisite not passed: ${blockedBy.join(', ')}`; status[id] = 'not_run'; process.stderr.write(`[wire] ${id} not_run (${entry.detail})\n`); return; }
    try {
      const detail = await fn();
      entry.status = 'pass'; entry.detail = typeof detail === 'string' ? detail : serial(detail);
    } catch (err) {
      entry.status = err instanceof Blocked ? 'expected_blocked' : 'fail';
      entry.detail = redactor.text(err.message || String(err));
      if (entry.status === 'fail') entry.repro = `node tools/scenarios/wire-runner.mjs --gil <same binary> --keep  # inspect ${id} in wire-run.json / wire-frames.jsonl`;
    }
    status[id] = entry.status === 'expected_blocked' ? 'pass' : entry.status;
    entry.finished = new Date().toISOString();
    process.stderr.write(`[wire] ${id} ${entry.status}${entry.status !== 'pass' ? `: ${entry.detail.slice(0, 200)}` : ''}\n`);
  }

  // ---- shared fixtures -------------------------------------------------------------------
  const CONTRACT = 'objective: 합성 작업 범위를 합의한다\nnext_action: 사용자에게 묻는다\ndone_when: 사용자의 답을 받았다\n';
  const QUESTION = 'question: 어떤 합성 작업인가\nchoices: 보고서 / 비교\nresponse: 합성 자료로 항목별 보고서를 작성해줘\n';
  const INTERPRETATION = 'interpretation: 항목별 파일 증거를 남기며 보고서를 만든다\nunresolved: 없음\n';
  const item = (id) => ({ id, question: `항목 ${id}을 합성 자료로 확인한다`, done_when: `evidence 파일에 항목 ${id} 결과가 저장된다` });
  const draft = (ids, basis, extra = {}) => ({
    scope: { goal: '합성 자료 기반 항목별 보고서', in_scope: '제공된 합성 자료의 비교', out_of_scope: '실시간 데이터·실제 개인정보', constraints: '외부 네트워크 없음', success_criteria: '항목마다 파일 증거가 남는다' },
    items: ids.map(item),
    synthesis: { statement: '합성 자료로 항목을 순서대로 확인하고 파일 증거를 남긴다', basis_refs: basis.join('\n'), exploration_checklist: ids.join('\n'), ...extra },
  });
  const outcomeReport = (synRef) => `verdict: success\nlesson: 합의한 범위를 기록했다\nsynthesis_ref: ${synRef}\nnext_direction.action: close_cycle\nnext_direction.reason: 승인된 범위를 실험으로 확인한다\n`;
  const interviewCycleReport = (outcomeRef, action = 'open_child') => `verdict: success\noutcome_ref: ${outcomeRef}\nhandoff_summary: 합성 범위를 Monitor 응답으로 승인받았다\nnext_direction.action: ${action}\nnext_direction.reason: ${action === 'close_chain' ? '승인된 종료 제안으로 Chain을 닫는다' : '승인 항목을 순서대로 실행한다'}\n`;
  const visuals = { attachments: [], uses: [], declaration: { state: 'not_needed', reason: '문서 증거만 필요한 합성 시험' } };
  const verifyArgs = (ref, report) => typedVerify ? { project_root: root, report, expected_owner: { kind: 'step', ref }, visuals } : { project_root: root, report };

  let scope = null;
  let tokens = { current: null, old: null };
  let proposal = { rev1: null, rev2: null };
  let cycle = { interview: 1 };
  let closeReportRev1 = null;
  let draftRev1 = null;

  /** Question + Interpretation in the current Interview cycle; returns their step refs. */
  async function askAndInterpret(n, firstStep = 1) {
    const refs = [];
    for (const [kind, report] of [['question', QUESTION], ['interpretation', INTERPRETATION]]) {
      const opened = ok(await model('gil_open', { project_root: root, kind, contract: CONTRACT }), `open ${kind}`);
      const m = said(opened).match(/^열었다: (step:C(\d+)\/S(\d+)) · (\w+)/m);
      expect(m && m[4] === kind && Number(m[2]) === n, `open ${kind}: unexpected receipt ${redactor.text(said(opened)).slice(0, 120)}`);
      refs.push(m[1]);
      ok(await model('gil_close', { project_root: root, report }), `close ${kind}`);
    }
    const syn = ok(await model('gil_open', { project_root: root, kind: 'synthesis', contract: 'objective: 범위 제안 승인\nnext_action: 제안을 Monitor로 보낸다\ndone_when: Monitor 응답을 받았다\n' }), 'open synthesis');
    const m = said(syn).match(/^열었다: (step:C(\d+)\/S(\d+)) · synthesis/m);
    expect(m, 'synthesis open receipt');
    void firstStep;
    return { basis: refs, synthesis: m[1] };
  }
  /** Approve via the App path with a fresh review token; returns the read view. */
  async function approveViaApp(expectedRevision) {
    const review = ok(await app('gil_plan_review', { scope_id: scope }), 'review');
    const token = review.meta?.['gil/planApprovalToken'];
    expect(typeof token === 'string' && /^[0-9a-f]{64}$/.test(token), 'review must carry a 64-hex private capability');
    secrets.push(token); redactor.addSecret(token);
    const p = review.structured.proposal;
    expect(p && p.revision === expectedRevision, `review shows revision ${p?.revision}, expected ${expectedRevision}`);
    ok(await app('gil_plan_decide', { scope_id: scope, revision: p.revision, digest: p.digest, token, decision: 'approve' }), 'decide approve');
    const read = ok(await model('gil_plan_read', { project_root: root }), 'read after approve');
    expect(read.structured.proposal?.receipt?.decision === 'approve' && read.structured.proposal.receipt.source === 'monitor_button', 'receipt not recorded as approve/monitor_button');
    return read.structured;
  }
  async function closeApprovedInterview(n, synRef, action = 'open_child') {
    const read = ok(await model('gil_plan_read', { project_root: root }), 'read close_report');
    ok(await model('gil_close', { project_root: root, report: read.structured.close_report }), 'close synthesis with close_report');
    const outcome = ok(await model('gil_open', { project_root: root, kind: 'outcome', contract: 'objective: 승인된 범위를 판정한다\nnext_action: 승인 제안으로 방향을 정한다\ndone_when: 판정과 방향이 정해졌다\n' }), 'open interview outcome');
    const m = said(outcome).match(/^열었다: (step:C\d+\/S\d+) · outcome/m);
    expect(m, 'outcome open receipt');
    ok(await model('gil_close', { project_root: root, report: outcomeReport(synRef) }), 'close interview outcome');
    const closed = ok(await model('gil_close', { project_root: root, report: interviewCycleReport(m[1], action) }), 'close interview cycle');
    expect(new RegExp(`^닫았다: cycle:C${n} · interview · success`, 'm').test(said(closed)), `interview cycle close receipt: ${redactor.text(said(closed)).slice(0, 160)}`);
  }
  /** Open an Experiment; returns its cycle number and bound item. */
  async function openExperiment(expectItem) {
    const opened = ok(await model('gil_open', { project_root: root, kind: 'experiment' }), 'open experiment');
    const m = said(opened).match(/^열었다: cycle:C(\d+) · experiment/m);
    expect(m, `experiment open receipt: ${redactor.text(said(opened)).slice(0, 160)}`);
    const n = Number(m[1]);
    const read = ok(await model('gil_plan_read', { project_root: root }), 'read after experiment open');
    expect(read.structured.current_item?.id === expectItem, `experiment C${n} bound to ${read.structured.current_item?.id}, expected ${expectItem}`);
    expect(typeof read.structured.document === 'string' && read.structured.document.includes('# GIL approved plan'), 'document missing after experiment open');
    return { n, document: read.structured.document, view: read.structured };
  }
  async function walkExperimentToVerify(n, itemId) {
    ok(await model('gil_open', { project_root: root, kind: 'define', contract: `objective: 항목 ${itemId}의 문제를 고정한다\nnext_action: 성공 조건을 적는다\ndone_when: 문제와 성공 조건이 기록된다\n` }), 'open define');
    ok(await model('gil_close', { project_root: root, report: `problem: 항목 ${itemId}을 합성 자료로 확인한다\nsuccess_condition: evidence 파일에 결과가 저장된다\n` }), 'close define');
    ok(await model('gil_open', { project_root: root, kind: 'hypothesis', contract: 'objective: 가설을 세운다\nnext_action: 검증 가능한 가설을 적는다\ndone_when: 가설과 guardrail이 기록된다\n' }), 'open hypothesis');
    ok(await model('gil_close', { project_root: root, report: `hypothesis: 합성 자료만으로 항목 ${itemId}을 확인할 수 있다\nrationale: 자료가 함께 제공됐다\nguardrail: 외부 데이터를 쓰지 않는다\n` }), 'close hypothesis');
    const v = ok(await model('gil_open', { project_root: root, kind: 'verify', contract: `objective: 항목 ${itemId}을 확인하고 저장한다\nnext_action: 합성 자료를 읽고 결과 파일을 쓴다\ndone_when: 결과 파일이 저장된다\n` }), 'open verify');
    const m = said(v).match(/^열었다: (step:C\d+\/S\d+) · verify/m);
    expect(m, 'verify open receipt');
    return m[1];
  }
  async function finishExperiment(n, itemId, verifyRef, document) {
    await mkdir(join(root, 'plan'), { recursive: true }); await mkdir(join(root, 'evidence'), { recursive: true });
    await writeFile(join(root, 'plan', 'approved-plan.md'), document);
    const evidence = `evidence/C${n}-${itemId}.md`;
    await writeFile(join(root, evidence), `# 합성 관측 C${n}\n\n항목 ${itemId}: 합성 자료 확인 결과 (시험 전용)\n`);
    ok(await model('gil_close', verifyArgs(verifyRef, `execution: 합성 자료를 확인하고 결과를 저장했다\nresult: 항목 ${itemId} 결과 파일을 남겼다\nplan_document: plan/approved-plan.md\nevidence_files: ${evidence}\n`)), 'close verify');
    ok(await model('gil_open', { project_root: root, kind: 'analysis', contract: 'objective: 관측을 해석한다\nnext_action: 가설과 대조한다\ndone_when: 해석이 기록된다\n' }), 'open analysis');
    ok(await model('gil_close', { project_root: root, report: 'hypothesis_fit: yes\nproblem_solved: yes\nsuccess_condition_met: yes\nguardrail_triggered: no\ninterpretation: 합성 결과가 가설과 일치했다\n' }), 'close analysis');
    const o = ok(await model('gil_open', { project_root: root, kind: 'outcome', contract: 'objective: 이 실험을 판정한다\nnext_action: 판정과 방향을 적는다\ndone_when: 판정이 기록된다\n' }), 'open experiment outcome');
    const m = said(o).match(/^열었다: (step:C\d+\/S\d+) · outcome/m);
    ok(await model('gil_close', { project_root: root, report: `verdict: success\nlesson: 항목 ${itemId}을 파일 증거로 확인했다\nnext_direction.action: close_cycle\nnext_direction.reason: 다음 항목으로 간다\n` }), 'close experiment outcome');
    const closed = ok(await model('gil_close', { project_root: root, report: `verdict: success\noutcome_ref: ${m[1]}\nhandoff_summary: 항목 ${itemId} 완료\nnext_direction.action: open_child\nnext_direction.reason: 남은 항목을 이어간다\n` }), 'close experiment cycle');
    expect(new RegExp(`^닫았다: cycle:C${n} · experiment · success`, 'm').test(said(closed)), 'experiment cycle close receipt');
    return evidence;
  }

  // ---- cases -------------------------------------------------------------------------------
  await run('W01', 'tools/list exposes the approval contract with App/model visibility', [], async () => {
    await connect();
    const listed = await core.request('tools/list', {});
    expect(!listed.error, `tools/list error ${serial(listed.error)}`);
    const tools = new Map(listed.result.tools.map((t) => [t.name, t]));
    const plan = ['gil_plan_propose', 'gil_plan_read', 'gil_plan_request_interview', 'gil_plan_review', 'gil_plan_decide'];
    const missing = plan.filter((n) => !tools.has(n));
    if (missing.length === plan.length) { note('approved-plan tools absent: the binary was built without the approved-plan feature'); throw new Error(`unsupported build: ${missing.join(', ')} missing`); }
    expect(!missing.length, `missing plan tools: ${missing.join(', ')}`);
    for (const n of plan) {
      const vis = tools.get(n)._meta?.ui?.visibility;
      const wantApp = n === 'gil_plan_review' || n === 'gil_plan_decide';
      expect(serial(vis) === serial(wantApp ? ['app'] : ['model']), `${n} visibility ${serial(vis)}`);
    }
    for (const n of ['gil_start', 'gil_open', 'gil_close', 'show_gil_monitor', 'gil_monitor_status', 'gil_restore', 'gil_revisit']) expect(tools.has(n), `${n} missing`);
    return `${tools.size} tools; plan tools present with correct visibility`;
  });

  await run('W02', 'gil_start once, then the returned show_gil_monitor; display stays unconfirmed', ['W01'], async () => {
    const started = ok(await model('gil_start', { project_root: root }), 'gil_start');
    const text = started.result.content.map((c) => c.text || '').join('\n');
    const m = text.match(/show_gil_monitor\((\{.*?\})\)/);
    expect(m, 'start receipt did not name show_gil_monitor(scope)');
    scope = JSON.parse(m[1]).scope_id;
    expect(/^project:[0-9a-f]{64}$/.test(scope), 'scope_id shape');
    const shown = ok(await model('show_gil_monitor', { scope_id: scope }), 'show_gil_monitor');
    expect(/표시.*판정하지 않는다|확인되지 않았다/.test(said(shown) + shown.result.content.map((c) => c.text).join('')), 'show receipt must not claim display');
    const ms = ok(await model('gil_monitor_status', { project_root: root }), 'gil_monitor_status');
    return { storage_format: started.structured.storage_format ?? null, display_confirmed: started.structured.display_confirmed ?? null, monitor_state: ms.structured.state ?? null, note: 'protocol success only; visibility is unknown without a human' };
  });

  await run('W03', 'Interview Question → Interpretation → open Synthesis', ['W02'], async () => {
    const got = await askAndInterpret(1);
    cycle.basis = got.basis; cycle.synthesis = got.synthesis;
    return got;
  });

  await run('W04', 'bare approved:yes close is refused; App review without a proposal carries no capability', ['W03'], async () => {
    const before = state();
    refused(await model('gil_close', { project_root: root, report: `statement: 합성 범위\nbasis_refs: ${cycle.basis.join('\n')}\napproved: yes\n` }), 'bare approved:yes');
    expect(sameState(before, state()) !== false, 'state changed after a refused bare close');
    const review = ok(await app('gil_plan_review', { scope_id: scope }), 'review before proposal');
    expect(review.structured.proposal === null && !review.meta?.['gil/planApprovalToken'], 'review without proposal must have no proposal and no token');
    return 'refused; state unchanged; no capability without a proposal';
  });

  await run('W05', 'propose; read shows revision 1 without receipt; identical re-propose is idempotent', ['W04'], async () => {
    const d = draft(['A', 'B'], cycle.basis); draftRev1 = d;
    const first = ok(await model('gil_plan_propose', { project_root: root, draft: d }), 'propose');
    expect(first.structured.proposal?.revision === 1 && first.structured.proposal.receipt === null, 'first proposal shape');
    expect(!serial(first.result).includes('planApprovalToken'), 'model propose result leaked a token');
    const again = ok(await model('gil_plan_propose', { project_root: root, draft: d }), 'propose again');
    expect(again.structured.proposal.revision === 1 && again.structured.proposal.digest === first.structured.proposal.digest, 'identical re-propose must return the same proposal');
    const read = ok(await model('gil_plan_read', { project_root: root }), 'read');
    expect(read.structured.supported === true && read.structured.active_revision === null && read.structured.pending.length === 0, 'read before approval');
    proposal.rev1 = read.structured.proposal;
    return { revision: 1, digest: proposal.rev1.digest.slice(0, 12) + '…' };
  });

  await run('W06', 'App review returns the private capability only in _meta and writes nothing', ['W05'], async () => {
    const before = state();
    const review = ok(await app('gil_plan_review', { scope_id: scope }), 'review');
    const token = review.meta?.['gil/planApprovalToken'];
    expect(typeof token === 'string' && /^[0-9a-f]{64}$/.test(token), 'token must be 64 hex in _meta');
    secrets.push(token); redactor.addSecret(token); tokens.current = token;
    expect(!serial(review.structured).includes(token) && !serial(review.result.content).includes(token), 'token leaked into content');
    expect(sameState(before, state()) !== false, 'review changed the state file');
    const again = ok(await app('gil_plan_review', { scope_id: scope }), 'review again');
    expect(again.meta['gil/planApprovalToken'] === token, 'same proposal must keep the same capability within one process');
    return 'capability only in _meta; state bytes unchanged; stable within process';
  });

  await run('W07', 'decide with another scope, a wrong token, or an extra field is refused/rejected', ['W06'], async () => {
    const before = state();
    const base = { scope_id: scope, revision: proposal.rev1.revision, digest: proposal.rev1.digest, token: tokens.current, decision: 'approve' };
    refused(await app('gil_plan_decide', { ...base, scope_id: `project:${'0'.repeat(64)}` }), 'other scope');
    refused(await app('gil_plan_decide', { ...base, token: 'f'.repeat(64) }), 'wrong token');
    refused(await app('gil_plan_decide', { ...base, digest: '0'.repeat(64) }), 'wrong digest');
    rpcError(await app('gil_plan_decide', { ...base, approved: true }), 'extra field');
    rpcError(await app('gil_plan_decide', { ...base, decision: 'yes' }), 'invalid decision enum');
    rpcError(await model('gil_plan_propose', { project_root: root, draft: { approved: true } }), 'malformed draft');
    expect(sameState(before, state()) !== false, 'state changed after refused decisions');
    const read = ok(await model('gil_plan_read', { project_root: root }), 'read');
    expect(read.structured.proposal.receipt === null, 'a refused decision must not record a receipt');
    return 'all refused; no receipt; state unchanged';
  });

  await run('W08', 'editing the open proposal creates revision 2 and voids the earlier capability and close_report', ['W07'], async () => {
    const read1 = ok(await model('gil_plan_read', { project_root: root }), 'read rev1');
    expect(read1.structured.close_report === null, 'an unanswered proposal must not offer a close_report');
    // What a forged "revision-1 approval" would look like: the Core must refuse it now and later.
    closeReportRev1 = formatReport({ ...draftRev1.synthesis, approved: 'yes', plan_revision: '1', plan_digest: proposal.rev1.digest });
    const edited = draft(['A', 'B'], cycle.basis); edited.scope.goal = '합성 자료 기반 항목별 보고서 (수정)';
    const second = ok(await model('gil_plan_propose', { project_root: root, draft: edited }), 'propose edited');
    expect(second.structured.proposal.revision === 2 && second.structured.proposal.receipt === null, 'edited proposal must be revision 2 without receipt');
    proposal.rev2 = second.structured.proposal;
    refused(await app('gil_plan_decide', { scope_id: scope, revision: 1, digest: proposal.rev1.digest, token: tokens.current, decision: 'approve' }), 'decide on superseded revision 1');
    const before = state();
    refused(await model('gil_close', { project_root: root, report: closeReportRev1 }), 'close with a forged revision-1 report');
    expect(sameState(before, state()) !== false, 'state changed after refused close');
    return 'revision 2 proposed; revision-1 decision and close refused';
  });

  await run('W09', 'process restart keeps the scope, reissues the capability, refuses the old one', ['W08'], async () => {
    const exit = await core.close();
    expect(exit.code === 0, `first Core did not exit cleanly: ${serial(exit)}`);
    expect(!exit.stdoutNoise.length, `non-JSON on stdout: ${exit.stdoutNoise.slice(0, 2).join(' | ')}`);
    await connect();
    const review = ok(await app('gil_plan_review', { scope_id: scope }), 'review after restart');
    const fresh = review.meta?.['gil/planApprovalToken'];
    expect(fresh && fresh !== tokens.current, 'restart must reissue a different capability');
    secrets.push(fresh); redactor.addSecret(fresh);
    expect(review.structured.proposal.revision === 2, 'restart must show revision 2');
    refused(await app('gil_plan_decide', { scope_id: scope, revision: 2, digest: proposal.rev2.digest, token: tokens.current, decision: 'approve' }), 'old capability after restart');
    tokens.old = tokens.current; tokens.current = fresh;
    const accepted = ok(await app('gil_plan_decide', { scope_id: scope, revision: 2, digest: proposal.rev2.digest, token: fresh, decision: 'approve' }), 'approve revision 2');
    expect(accepted.structured.proposal.receipt?.decision === 'approve', 'receipt missing after approve');
    return 'scope restored without prepare; new capability accepted; old refused';
  });

  await run('W10', 'duplicate approve is idempotent; flipping to request_changes is refused', ['W09'], async () => {
    const before = state();
    ok(await app('gil_plan_decide', { scope_id: scope, revision: 2, digest: proposal.rev2.digest, token: tokens.current, decision: 'approve' }), 'duplicate approve');
    expect(sameState(before, state()) !== false, 'duplicate approve rewrote the state');
    refused(await app('gil_plan_decide', { scope_id: scope, revision: 2, digest: proposal.rev2.digest, token: tokens.current, decision: 'request_changes' }), 'flip decision');
    const review = ok(await app('gil_plan_review', { scope_id: scope }), 'review after receipt');
    expect(!review.meta?.['gil/planApprovalToken'], 'no new capability once a receipt exists');
    const stateText = state()?.toString('utf8') ?? '';
    expect(!secrets.some((s) => stateText.includes(s)), 'a capability token was written into the project record');
    return 'idempotent; flip refused; no capability after receipt; no token in record';
  });

  await run('W11', 'Synthesis closes only with the exact close_report; a tampered statement is refused', ['W10'], async () => {
    const read = ok(await model('gil_plan_read', { project_root: root }), 'read');
    const report = read.structured.close_report;
    expect(typeof report === 'string' && /approved: \|\n  yes\n/.test(report) && report.includes('plan_revision'), 'close_report shape');
    refused(await model('gil_close', { project_root: root, report: report.replace('합성 자료로 항목을', '다른 문장으로 항목을') }), 'tampered close_report');
    refused(await model('gil_close', { project_root: root, report: closeReportRev1 }), 'forged revision-1 report after approval of revision 2');
    ok(await model('gil_close', { project_root: root, report }), 'close synthesis');
    return 'closed with the recorded report only';
  });

  await run('W12', 'Experiment is refused until Outcome and the Interview Cycle are closed, then binds item A', ['W11'], async () => {
    refused(await model('gil_open', { project_root: root, kind: 'experiment' }), 'experiment right after synthesis');
    const outcome = ok(await model('gil_open', { project_root: root, kind: 'outcome', contract: 'objective: 승인된 범위를 판정한다\nnext_action: 승인 제안으로 방향을 정한다\ndone_when: 판정과 방향이 정해졌다\n' }), 'open outcome');
    const m = said(outcome).match(/^열었다: (step:C1\/S\d+) · outcome/m);
    ok(await model('gil_close', { project_root: root, report: outcomeReport(cycle.synthesis) }), 'close outcome');
    refused(await model('gil_open', { project_root: root, kind: 'experiment' }), 'experiment before cycle close');
    refused(await model('gil_cycle', { project_root: root, action: 'close' }), 'legacy gil_cycle close');
    ok(await model('gil_close', { project_root: root, report: interviewCycleReport(m[1]) }), 'close interview cycle');
    const read = ok(await model('gil_plan_read', { project_root: root }), 'read after interview close');
    expect(read.structured.active_revision === 2 && serial(read.structured.pending.map((i) => i.id)) === serial(['A', 'B']), `active/pending after interview: ${serial(read.structured.active_revision)} ${serial(read.structured.pending)}`);
    const exp = await openExperiment('A');
    cycle.experimentA = exp.n; cycle.documentRev2 = exp.document;
    expect(serial(exp.view.pending.map((i) => i.id)) === serial(['B']), 'pending after binding A');
    return { experiment: `C${exp.n}`, bound: 'A', pending: ['B'] };
  });

  await run('W13', 'Verify needs the bound plan document bytes plus separate fresh evidence', ['W12'], async () => {
    const n = cycle.experimentA;
    const verifyRef = await walkExperimentToVerify(n, 'A');
    await mkdir(join(root, 'plan'), { recursive: true }); await mkdir(join(root, 'evidence'), { recursive: true });
    await writeFile(join(root, 'evidence', `C${n}-A.md`), '# 합성 관측\n\n항목 A 결과\n');
    const before = state();
    refused(await model('gil_close', verifyArgs(verifyRef, `execution: 확인했다\nresult: 저장했다\nevidence_files: evidence/C${n}-A.md\n`)), 'verify without plan_document');
    await writeFile(join(root, 'plan', 'approved-plan.md'), cycle.documentRev2);
    refused(await model('gil_close', verifyArgs(verifyRef, 'execution: 확인했다\nresult: 저장했다\nplan_document: plan/approved-plan.md\nevidence_files: plan/approved-plan.md\n')), 'plan document as the only evidence');
    await writeFile(join(root, 'plan', 'approved-plan.md'), cycle.documentRev2.replace('Revision: 2', 'Revision: 1'));
    refused(await model('gil_close', verifyArgs(verifyRef, `execution: 확인했다\nresult: 저장했다\nplan_document: plan/approved-plan.md\nevidence_files: evidence/C${n}-A.md\n`)), 'wrong document bytes');
    refused(await model('gil_close', verifyArgs(verifyRef, 'execution: 확인했다\nresult: 채팅으로 보고했다\n')), 'no evidence_files at all');
    expect(sameState(before, state()) !== false, 'state changed after refused Verify closes');
    await finishExperiment(n, 'A', verifyRef, cycle.documentRev2);
    const read = ok(await model('gil_plan_read', { project_root: root }), 'read after C2');
    expect(read.structured.executed_cycles.length === 1 && read.structured.executed_cycles[0].item_id === 'A' && read.structured.executed_cycles[0].revision === 2, 'executed binding for A');
    return `C${n} closed success; four refusals before the valid close`;
  });

  await run('W14', 'gil_restore / gil_revisit are blocked in this profile (expected_blocked)', ['W13'], async () => {
    const before = state();
    const restore = await model('gil_restore', { project_root: root });
    const revisit = await model('gil_revisit', { project_root: root });
    if (restore.ok || revisit.ok) throw new Error(`destructive rollback succeeded: restore=${restore.ok} revisit=${revisit.ok}`);
    expect(sameState(before, state()) !== false, 'blocked restore/revisit changed the state');
    throw new Blocked(`restore: ${redactor.text(said(restore)).split('\n')[0].slice(0, 120)} | revisit: ${redactor.text(said(revisit)).split('\n')[0].slice(0, 120)}`);
  });

  await run('W15', 'reinterview request is preserved through the current Experiment and forces an Interview next', ['W13'], async () => {
    const exp = await openExperiment('B');
    const n = exp.n; cycle.experimentB = n;
    ok(await model('gil_open', { project_root: root, kind: 'define', contract: 'objective: 항목 B의 문제를 고정한다\nnext_action: 성공 조건을 적는다\ndone_when: 기록된다\n' }), 'open define');
    refused(await model('gil_plan_request_interview', { project_root: root, reason: '   ' }), 'blank reason');
    ok(await model('gil_plan_request_interview', { project_root: root, reason: '항목 D가 B 다음에 필요하다' }), 'request interview');
    ok(await model('gil_plan_request_interview', { project_root: root, reason: '항목 D가 B 다음에 필요하다' }), 'request interview again');
    let read = ok(await model('gil_plan_read', { project_root: root }), 'read');
    expect(read.structured.reinterview_requests.length === 1, `duplicate request must collapse to one, got ${read.structured.reinterview_requests.length}`);
    refused(await model('gil_open', { project_root: root, kind: 'interview' }), 'interview while Define is open');
    ok(await model('gil_close', { project_root: root, report: 'problem: 항목 B을 합성 자료로 확인한다\nsuccess_condition: evidence 파일에 결과가 저장된다\n' }), 'close define');
    ok(await model('gil_open', { project_root: root, kind: 'hypothesis', contract: 'objective: 가설\nnext_action: 가설을 적는다\ndone_when: 기록된다\n' }), 'open hypothesis');
    ok(await model('gil_close', { project_root: root, report: 'hypothesis: 합성 자료만으로 항목 B을 확인할 수 있다\nrationale: 자료가 제공됐다\nguardrail: 외부 데이터를 쓰지 않는다\n' }), 'close hypothesis');
    const v = ok(await model('gil_open', { project_root: root, kind: 'verify', contract: 'objective: 항목 B 확인\nnext_action: 결과를 저장한다\ndone_when: 파일이 저장된다\n' }), 'open verify');
    const verifyRef = said(v).match(/^열었다: (step:C\d+\/S\d+) · verify/m)[1];
    await finishExperiment(n, 'B', verifyRef, cycle.documentRev2);
    read = ok(await model('gil_plan_read', { project_root: root }), 'read after C3');
    expect(read.structured.reinterview_requests.length === 1 && read.structured.pending.length === 0, 'request must survive the cycle close');
    refused(await model('gil_open', { project_root: root, kind: 'experiment' }), 'experiment while a reinterview request is pending');
    const opened = ok(await model('gil_open', { project_root: root, kind: 'interview' }), 'open interview');
    const m = said(opened).match(/^열었다: cycle:C(\d+) · interview/m);
    cycle.interview = Number(m[1]);
    return { experiment: `C${n}`, interview: `C${cycle.interview}`, preserved: read.structured.reinterview_requests.map((r) => r.reason) };
  });

  await run('W16', 'request_changes closes with approved:no, re-proposal must keep approved items, then re-approval', ['W15'], async () => {
    const n = cycle.interview;
    const first = await askAndInterpret(n);
    const d3 = draft(['A', 'B', 'D'], first.basis);
    const p3 = ok(await model('gil_plan_propose', { project_root: root, draft: d3 }), 'propose rev3');
    expect(p3.structured.proposal.revision === 3 && p3.structured.proposal.base_revision === 2, `rev3 base ${serial(p3.structured.proposal.base_revision)}`);
    const review = ok(await app('gil_plan_review', { scope_id: scope }), 'review rev3');
    const token = review.meta['gil/planApprovalToken']; secrets.push(token); redactor.addSecret(token);
    ok(await app('gil_plan_decide', { scope_id: scope, revision: 3, digest: p3.structured.proposal.digest, token, decision: 'request_changes' }), 'request_changes');
    let read = ok(await model('gil_plan_read', { project_root: root }), 'read after request_changes');
    // close_report uses the block form for every field: `approved: |\n  no`.
    expect(/approved: \|\n  no\n/.test(read.structured.close_report), `close_report must say approved: no, got ${read.structured.close_report.slice(0, 200)}`);
    const forged = read.structured.close_report.replace(/approved: \|\n  no\n/, 'approved: |\n  yes\n');
    expect(forged !== read.structured.close_report, 'forgery fixture did not change the report');
    refused(await model('gil_close', { project_root: root, report: forged }), 'forging approved:yes');
    refused(await model('gil_open', { project_root: root, kind: 'experiment' }), 'experiment with open Synthesis');
    const closed = ok(await model('gil_close', { project_root: root, report: read.structured.close_report }), 'close synthesis with approved:no');
    expect(/question/.test(said(closed)), 'after approved:no the next step must be a question');
    refused(await model('gil_open', { project_root: root, kind: 'outcome', contract: CONTRACT }), 'outcome after approved:no');
    const second = await askAndInterpret(n);
    const altered = draft(['A', 'B', 'D'], second.basis); altered.items[1].done_when = '바뀐 조건';
    refused(await model('gil_plan_propose', { project_root: root, draft: altered }), 'altered approved item');
    refused(await model('gil_plan_propose', { project_root: root, draft: draft(['A', 'D'], second.basis) }), 'removed approved item');
    refused(await model('gil_plan_propose', { project_root: root, draft: draft(['A', 'B', 'D'], second.basis, { exploration_checklist: 'A\nD\nB' }) }), 'checklist order mismatch');
    const p4 = ok(await model('gil_plan_propose', { project_root: root, draft: draft(['A', 'B', 'D'], second.basis) }), 'propose rev4');
    expect(p4.structured.proposal.revision === 4, 'revision 4');
    refused(await app('gil_plan_decide', { scope_id: scope, revision: 3, digest: p3.structured.proposal.digest, token, decision: 'approve' }), 'approve the superseded rev3');
    await approveViaApp(4);
    await closeApprovedInterview(n, second.synthesis);
    read = ok(await model('gil_plan_read', { project_root: root }), 'read after C4');
    expect(read.structured.active_revision === 4 && serial(read.structured.pending.map((i) => i.id)) === serial(['D']) && read.structured.reinterview_requests.length === 0, `after reapproval: active ${read.structured.active_revision} pending ${serial(read.structured.pending)} reinterview ${read.structured.reinterview_requests.length}`);
    return { interview: `C${n}`, revisions: [3, 4], pending: ['D'] };
  });

  await run('W17', 'the inserted item D executes under revision 4 with the revision-4 document', ['W16'], async () => {
    const exp = await openExperiment('D');
    expect(exp.document.includes('Revision: 4'), 'document must be the revision-4 export');
    const verifyRef = await walkExperimentToVerify(exp.n, 'D');
    refused(await model('gil_close', verifyArgs(verifyRef, `execution: 확인했다\nresult: 저장했다\nplan_document: plan/approved-plan.md\nevidence_files: evidence/C${exp.n}-D.md\n`)), 'stale revision-2 document bytes');
    await finishExperiment(exp.n, 'D', verifyRef, exp.document);
    const read = ok(await model('gil_plan_read', { project_root: root }), 'read after C5');
    expect(read.structured.pending.length === 0 && read.structured.executed_cycles.map((e) => `${e.item_id}@${e.revision}`).join(',') === 'A@2,B@2,D@4', `executed ${serial(read.structured.executed_cycles)}`);
    cycle.experimentD = exp.n;
    return { experiment: `C${exp.n}`, executed: 'A@2,B@2,D@4' };
  });

  await run('W18', 'closing Interview needs its own approval; Chain closes; the next Chain cannot reuse it', ['W17'], async () => {
    const opened = ok(await model('gil_open', { project_root: root, kind: 'interview' }), 'open closing interview');
    const n = Number(said(opened).match(/^열었다: cycle:C(\d+) · interview/m)[1]);
    const got = await askAndInterpret(n);
    const closing = draft(['A', 'B', 'D'], got.basis, { chain_verdict: 'success' });
    const p5 = ok(await model('gil_plan_propose', { project_root: root, draft: closing }), 'propose closing');
    expect(p5.structured.proposal.revision === 5, 'closing proposal revision 5');
    refused(await model('gil_close', { project_root: root, report: `statement: ${closing.synthesis.statement}\nbasis_refs: ${got.basis.join('\n')}\nexploration_checklist: |\n  A\n  B\n  D\nchain_verdict: success\napproved: yes\n` }), 'bare approved:yes on closing synthesis');
    await approveViaApp(5);
    await closeApprovedInterview(n, got.synthesis, 'close_chain');
    refused(await model('gil_open', { project_root: root, kind: 'experiment' }), 'experiment while Chain Report is due');
    const chainClosed = ok(await model('gil_close', { project_root: root, report: `verdict: success\nclosing_synthesis_ref: ${got.synthesis}\nhandoff_summary: 합성 항목 A,B,D를 파일 증거와 함께 완료했다\n` }), 'close chain');
    expect(/^닫았다: chain:/m.test(said(chainClosed)), `chain close receipt: ${redactor.text(said(chainClosed)).slice(0, 120)}`);
    refused(await model('gil_open', { project_root: root, kind: 'experiment' }), 'experiment after Chain closed');
    const next = ok(await model('gil_open', { project_root: root, kind: 'chain' }), 'open next chain');
    expect(/Opening Interview/.test(said(next)), 'next chain receipt');
    let read = ok(await model('gil_plan_read', { project_root: root }), 'read in new chain');
    expect(read.structured.supported === true && read.structured.proposal === null && read.structured.active_revision === null && read.structured.pending.length === 0 && read.structured.executed_cycles.length === 0, `new chain state ${serial(read.structured)}`.slice(0, 300));
    const m = Number(said(next).match(/cycle:C(\d+)/)[1]);
    const again = await askAndInterpret(m);
    const px = ok(await model('gil_plan_propose', { project_root: root, draft: draft(['X'], again.basis) }), 'propose in new chain');
    expect(px.structured.proposal.revision === 1 && px.structured.proposal.base_revision === null, 'new chain starts at revision 1 without base');
    refused(await app('gil_plan_decide', { scope_id: scope, revision: 1, digest: p5.structured.proposal.digest, token: tokens.current, decision: 'approve' }), 'old chain digest/capability');
    await approveViaApp(1);
    read = ok(await model('gil_plan_read', { project_root: root }), 'read after new approval');
    expect(read.structured.proposal.receipt.decision === 'approve', 'new chain approval recorded');
    return { closingInterview: `C${n}`, nextChainInterview: `C${m}` };
  });

  await run('W19', 'clean shutdown: EOF exits 0 and stdout carried only JSON-RPC frames', ['W01'], async () => {
    const exit = await core.close();
    expect(exit.code === 0, `exit ${serial(exit)}`);
    expect(!exit.stdoutNoise.length, `stdout noise: ${exit.stdoutNoise.slice(0, 3).join(' | ')}`);
    return { frames: exit.frames, generations: generation };
  });

  await run('W20', 'proxy with explicit synthetic App decision: recorded as harness input, never as a person', ['W19'], async () => {
    const proot = join(work, 'proxy-project'); await mkdir(proot);
    const pevents = join(work, 'proxy-events.jsonl');
    const psettings = join(work, 'proxy-settings'); await mkdir(psettings);
    const proxy = new CoreClient({ binary: process.execPath, args: [join(here, 'proxy.mjs'), '--core', binary, '--root', proot, '--events', pevents, '--settings', join(psettings, 'bindings'), '--synthetic-app-decision', 'approve'], requestTimeoutMs: opts.timeoutMs, label: 'proxy' }).start();
    try {
      await proxy.initialize();
      const listed = await proxy.request('tools/list', {});
      const names = new Set(listed.result.tools.map((t) => t.name));
      expect(names.has('save_project_file') && names.has('read_project_file') && names.has('gil_plan_propose') && !names.has('gil_plan_review') && !names.has('gil_plan_decide'), `proxy tool list must add scenario tools and hide App-only tools: ${[...names].join(',')}`);
      const started = await proxy.call('gil_start', { project_root: proot }); expect(started.ok, 'proxy gil_start');
      const other = await proxy.call('gil_status', { project_root: root }); expect(other.error, 'proxy must refuse another project root');
      const appTool = await proxy.call('gil_plan_review', { scope_id: 'project:' + '0'.repeat(64) }); expect(appTool.error, 'proxy must refuse App-only tools from the model side');
      for (const [kind, report] of [['question', QUESTION], ['interpretation', INTERPRETATION]]) {
        expect((await proxy.call('gil_open', { project_root: proot, kind, contract: CONTRACT })).ok, `proxy open ${kind}`);
        expect((await proxy.call('gil_close', { project_root: proot, report })).ok, `proxy close ${kind}`);
      }
      expect((await proxy.call('gil_open', { project_root: proot, kind: 'synthesis', contract: CONTRACT })).ok, 'proxy open synthesis');
      const proposed = await proxy.call('gil_plan_propose', { project_root: proot, draft: draft(['P'], ['step:C1/S1', 'step:C1/S2']) });
      expect(proposed.ok, 'proxy propose');
      const noteText = proposed.result.content.map((c) => c.text || '').join('\n');
      expect(/synthetic|합성/i.test(noteText) && /not user consent|사용자 동의가 아니다/i.test(noteText), 'proxy must label the synthetic decision in its note');
      const read = await proxy.call('gil_plan_read', { project_root: proot });
      expect(read.ok && read.structured.proposal.receipt?.decision === 'approve', 'synthetic decision not recorded');
      const saved = await proxy.call('save_project_file', { path: 'evidence/p.md', text: 'synthetic', item_id: 'P' }); expect(saved.ok, 'save_project_file');
      const escape = await proxy.call('save_project_file', { path: '../escape.md', text: 'x' }); expect(!escape.ok, 'path escape must be refused');
      const hidden = await proxy.call('save_project_file', { path: '.gil/state.yaml', text: 'x' }); expect(!hidden.ok, 'writing into .gil must be refused');
    } finally { await proxy.close(); }
    const trace = parseEvents(await readFile(pevents, 'utf8'));
    const harness = trace.filter((e) => e.actor === 'harness');
    expect(harness.length >= 2 && harness.every((e) => e.synthetic) && harness.some((e) => e.request.name === 'gil_plan_decide'), 'harness review/decide must be logged as synthetic harness events');
    expect(!trace.some((e) => JSON.stringify(e).match(/"token":"[0-9a-f]{64}"/)), 'proxy events leaked a capability token');
    const facts = replay(trace, { externalTools: ['save_project_file', 'read_project_file'] });
    const judged = Object.fromEntries(scoreProcedure(facts, { root: proot }).map((x) => [x.id, x.status]));
    expect(judged.human_approval_real === 'unknown', `human_approval_real must stay unknown for synthetic input, got ${judged.human_approval_real}`);
    expect(judged.no_action_while_synthesis_open === 'fail', `save during open Synthesis must be judged fail, got ${judged.no_action_while_synthesis_open}`);
    return { harnessEvents: harness.length, human_approval_real: judged.human_approval_real, no_action_while_synthesis_open: judged.no_action_while_synthesis_open };
  });

  await run('W21', 'scorer replays this run: synthetic approval never scores as human consent or display', ['W18'], async () => {
    const facts = replay(parseEvents(events.map((e) => JSON.stringify(e)).join('\n')), { externalTools: [] });
    const judged = Object.fromEntries(scoreProcedure(facts, { root }).map((x) => [x.id, x.status]));
    const want = { monitor_show_called: 'pass', monitor_display_confirmed: 'unknown', approval_before_execution: 'pass', human_approval_real: 'unknown', no_action_while_synthesis_open: 'unknown', verify_file_evidence: 'pass', no_reused_approval: 'pass', reinterview_preserved: 'pass', restore_revisit_blocked: 'expected_blocked', closing_interview_before_chain_close: 'pass' };
    const diff = Object.entries(want).filter(([k, v]) => judged[k] !== v).map(([k, v]) => `${k}: got ${judged[k]}, want ${v}`);
    expect(!diff.length, diff.join('; '));
    return judged;
  });

  if (core && !core.exit) await core.kill();

  // ---- outputs -----------------------------------------------------------------------------
  const counts = summarize(cases.map((c) => ({ status: c.status })));
  const result = {
    suite: 'wave1-approved-plan-wire', generated_at: startedAt.toISOString(), finished_at: new Date().toISOString(),
    repo_head: head, features: opts.features, binary_sha256: binarySha, built_here: opts.build, platform: `${process.platform}-${process.arch}`, node: process.version,
    queue: process.env.GIL_PARALLEL_TARGET_DIR ? 'cargo-queue.py' : 'none',
    observations: { monitor_display_confirmed: 'unknown', human_button_click: 'none (all App inputs were synthetic harness calls)', restore_revisit: 'expected_blocked' },
    summary: counts, cases, notes, frames: frames.length, events: events.length,
  };
  const serialized = JSON.stringify({ result, frames, events });
  const leaked = leaks(serialized, { secrets, roots: [root, settings, work] });
  if (leaked.length) { note(`redaction gap: ${leaked.length} value(s) still present; outputs withheld`); result.redaction_gap = leaked.length; }
  await writeFile(join(out, 'wire-run.json'), JSON.stringify(result, null, 2));
  if (!leaked.length) {
    await writeFile(join(out, 'wire-frames.jsonl'), frames.map((f) => JSON.stringify(f)).join('\n') + '\n');
    await writeFile(join(out, 'wire-events.jsonl'), events.map((e) => JSON.stringify(e)).join('\n') + '\n');
  }
  const md = [`# Approved-plan wire regression — ${startedAt.toISOString().slice(0, 10)}`, '',
    `- HEAD: \`${head}\` · features: \`${opts.features}\` · binary sha256: \`${binarySha.slice(0, 16)}…\` · built in this command: ${opts.build}`,
    `- platform: ${result.platform} · node ${process.version} · queue: ${result.queue}`,
    `- verdict: **${counts.verdict}** · ${Object.entries(counts.counts).filter(([, v]) => v).map(([k, v]) => `${k}=${v}`).join(', ')}`,
    `- Monitor display: unknown (protocol only). Human button click: none; every App decision was a synthetic harness call and earns no user-consent score.`,
    '', table(['case', 'status', 'title', 'detail'], cases.map((c) => [c.id, c.status, c.title, c.detail.slice(0, 220)])), '',
    notes.length ? `## Notes\n\n${notes.map((n) => `- ${n}`).join('\n')}\n` : '',
    `## Reproduce\n\n\`\`\`sh\npython3 coordination/parallel-wave-1/cargo-queue.py -- node tools/scenarios/wire-runner.mjs --build --out <dir>\n\`\`\`\n\nFailures keep the temporary work dir; inspect wire-frames.jsonl (redacted frames, per generation) and wire-events.jsonl.`].join('\n');
  await writeFile(join(out, 'wire-run.md'), md);
  process.stderr.write(`[wire] ${counts.verdict}: ${JSON.stringify(counts.counts)}\n[wire] results: ${out}\n`);
  const failed = cases.some((c) => c.status === 'fail');
  if (!failed && !opts.keep && !opts.out) { /* keep work dir only when something failed */ }
  if (!failed && !opts.keep) { await rm(root, { recursive: true, force: true }); await rm(settings, { recursive: true, force: true }); }
  else process.stderr.write(`[wire] work dir kept: ${work}\n`);
  process.exitCode = failed ? 1 : 0;
}

main().catch((err) => { process.stderr.write(`[wire] fatal: ${err.stack || err}\n`); process.exitCode = 2; });
