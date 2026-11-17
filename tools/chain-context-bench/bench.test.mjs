// Self-tests for tools/chain-context-bench. No real Core, model or network: Core behavior is
// replaced by fixtures/fake-core.mjs. The real-Core path is exercised by the baseline run
// (README) and, when CHAIN_CONTEXT_BENCH_GIL names a built gil, by the last test.
import { test, after } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync, readdirSync, copyFileSync, chmodSync, rmSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { tmpdir } from 'node:os';
import { spawn, spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { parseArgs, sanitize, refuseSecrets, recoveryConditions, compare, compactResult, UsageError } from './cli.mjs';
import { buildPlan, estimateCalls, recoveryForFeatures, BASE, AXES, CALL_BUDGET } from './lib/plan.mjs';
import { measureReadPhase, errorKind } from './lib/run.mjs';
import { preservation } from './lib/bench.mjs';
import { formatMarkdown } from './lib/report.mjs';
import { snapshotReadRoots, compareReadRoots, readOnlyUnchanged } from './lib/measure.mjs';
import { treeDigest, diffTrees, segmentContext, sectionMarkers, evaluateResolution, evaluateLineage, lineageHolds, inspectPreserved, compareRepeats, blockProfile, invariantAcross, templateOf, normalizedSha, lineRepeats } from './lib/measure.mjs';
import { field, detailLines, TagBook, generate, GenerationError, UnsupportedError, RECOVERY, recoveryPathFor } from './lib/scenario.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const cli = join(here, 'cli.mjs');
const made = [];
const scratch = (name) => { const d = mkdtempSync(join(tmpdir(), `ccb-test-${name}-`)); made.push(d); return d; };
after(() => { for (const d of made) rmSync(d, { recursive: true, force: true }); });

function fakeCore(mode) {
  const dir = scratch(`fake-${mode}`);
  const marks = join(dir, 'marks'); mkdirSync(marks);
  const script = join(dir, 'fake-core.mjs');
  writeFileSync(script, `#!${process.execPath}\n` + readFileSync(join(here, 'fixtures', 'fake-core.mjs'), 'utf8'));
  chmodSync(script, 0o755);
  writeFileSync(join(dir, 'fake-core.json'), JSON.stringify({ mode, markDir: marks }));
  return { dir, script, marks };
}
const alive = (pid) => { try { process.kill(pid, 0); return true; } catch { return false; } };
const runCli = (args, opts = {}) => spawnSync(process.execPath, [cli, ...args], { encoding: 'utf8', timeout: 60000, ...opts });
const small = ['--axes', 'chains', '--levels', 'chains=0', '--repeat', '1'];

// ── arguments and limits ───────────────────────────────────────────────────────────────────

test('argument errors are usage errors (exit 2) and nothing is created', () => {
  const root = scratch('args');
  const out = join(root, 'out');
  const cases = [
    [['bogus']],
    [['run', '--out', out]], // neither --gil nor --build
    [['run', '--build', '--gil', '/bin/true', '--out', out]],
    [['run', '--gil', '/bin/true']], // no --out
    [['run', '--gil', join(root, 'missing-gil'), '--out', out]],
    [['plan', '--axes', 'nope']],
    [['plan', '--levels', 'chains=-1']],
    [['plan', '--levels', 'chains=1.5']],
    [['plan', '--levels', 'chains=0', '--levels', 'chains=1']],
    [['plan', '--axes', 'chains', '--levels', 'failures=1']],
    [['plan', '--reads', 'gil_close']],
    [['plan', '--features', 'visual-authoring']],
    [['plan', '--features', 'approved-plan,runtime-testbed']],
    [['plan', '--timeout-ms', '10']],
    [['plan', '--out']],
    [['compare', '--before', join(root, 'a.json')]],
  ];
  for (const [args] of cases) {
    const r = runCli(args);
    assert.equal(r.status, 2, `${args.join(' ')} → ${r.status} ${r.stderr}`);
    assert.match(r.stderr, /\[bench\]/);
  }
  assert.equal(existsSync(out), false);
});

test('scale caps and the call budget refuse a plan before any Core runs', () => {
  for (const [axis, spec] of Object.entries(AXES)) {
    assert.throws(() => parseArgs(['plan', '--levels', `${axis}=${spec.cap + 1}`]), UsageError, axis);
    assert.doesNotThrow(() => parseArgs(['plan', '--axes', axis, '--levels', `${axis}=${spec.cap}`, '--max-calls', String(CALL_BUDGET.cap)]));
  }
  assert.throws(() => parseArgs(['plan', '--repeat', '4']), /--repeat/);
  assert.throws(() => parseArgs(['plan', '--max-calls', String(CALL_BUDGET.cap + 1)]), /--max-calls/);
  assert.throws(() => parseArgs(['plan', '--max-calls', '50']), /above --max-calls 50/);
  // The refusal happens in argument parsing: a fake Core given to `run` is never started.
  const fake = fakeCore('refuse');
  const r = runCli(['run', '--gil', fake.script, '--out', join(fake.dir, 'out'), '--max-calls', '10']);
  assert.equal(r.status, 2);
  assert.deepEqual(readdirSync(fake.marks), []);
  assert.equal(existsSync(join(fake.dir, 'out')), false);
});

test('plan is deterministic, shares the base case and estimates each block', () => {
  const a = buildPlan({}); const b = buildPlan({});
  assert.deepEqual(a, b);
  const baseCases = a.cases.filter((c) => c.key === Object.entries(BASE).map(([k, v]) => `${k}=${v}`).join(','));
  assert.equal(baseCases.length, 1, 'the base point is generated once');
  assert.equal(baseCases[0].axes.length, 6, 'every axis points at the shared base case');
  assert.equal(a.cases.length, 1 + 6 * 2);
  assert.equal(estimateCalls({ ...BASE, chains: 1 }, ['gil_context']) - estimateCalls(BASE, ['gil_context']), 42);
  assert.equal(estimateCalls({ ...BASE, failures: 2 }, []) - estimateCalls(BASE, []), 28);
  // Matches the generation calls counted in the 2026-11-17 baseline (21 for the base point + 3 reads).
  assert.equal(estimateCalls(BASE, ['gil_context', 'gil_status', 'gil_plan_read']), 24);
});

// ── results never overwrite ────────────────────────────────────────────────────────────────

test('an existing --out (directory or file) is refused and left untouched', () => {
  const fake = fakeCore('refuse');
  const dirOut = join(fake.dir, 'existing'); mkdirSync(dirOut); writeFileSync(join(dirOut, 'bench.json'), 'keep');
  const fileOut = join(fake.dir, 'file-out'); writeFileSync(fileOut, 'keep');
  for (const out of [dirOut, fileOut]) {
    const r = runCli(['run', '--gil', fake.script, '--out', out, ...small]);
    assert.equal(r.status, 2, r.stderr);
    assert.match(r.stderr, /already exists/);
  }
  assert.equal(readFileSync(join(dirOut, 'bench.json'), 'utf8'), 'keep');
  assert.equal(readFileSync(fileOut, 'utf8'), 'keep');
  assert.deepEqual(readdirSync(fake.marks), [], 'no Core was started');
});

test('compare writes only a new --json file', () => {
  const root = scratch('cmp-json');
  const r = { schema: 'gil-chain-context-bench/0.1', core: { sha256: 'x', features_declared: 'approved-plan' }, conditions: { locale: 'ko-KR', reads: ['gil_context'] }, plan: { base: BASE }, cases: [] };
  writeFileSync(join(root, 'a.json'), JSON.stringify(r));
  writeFileSync(join(root, 'out.json'), 'keep');
  assert.throws(() => compare({ before: join(root, 'a.json'), after: join(root, 'a.json'), json: join(root, 'out.json') }), /EEXIST/);
  assert.equal(readFileSync(join(root, 'out.json'), 'utf8'), 'keep');
});

// ── generation failure, missing responses, interrupt ───────────────────────────────────────

test('a refusing Core is recorded as generation_failed with its step, exit 1, no temp left', () => {
  const fake = fakeCore('refuse');
  const work = join(fake.dir, 'work'); mkdirSync(work);
  const out = join(fake.dir, 'out');
  const r = runCli(['run', '--gil', fake.script, '--features', 'approved-plan', '--out', out, '--work-root', work, '--quiet', ...small]);
  assert.equal(r.status, 1, r.stderr);
  const result = JSON.parse(readFileSync(join(out, 'bench.json'), 'utf8'));
  assert.equal(result.status, 'incomplete');
  assert.equal(result.cases[0].status, 'generation_failed');
  assert.equal(result.incomplete[0].error.step, 'start');
  assert.match(readFileSync(join(out, 'bench.md'), 'utf8'), /generation_failed/);
  assert.equal(JSON.parse(readFileSync(join(out, 'summary.json'), 'utf8')).cases[0].status, 'generation_failed');
  assert.deepEqual(readdirSync(work), [], 'temporary project removed');
  assert.ok(!JSON.stringify(result).includes(tmpdir()), 'no machine path in the result');
});

test('an unanswered call times out into a recorded failure instead of hanging', () => {
  const fake = fakeCore('hang');
  const work = join(fake.dir, 'work'); mkdirSync(work);
  const out = join(fake.dir, 'out');
  const r = runCli(['run', '--gil', fake.script, '--out', out, '--work-root', work, '--timeout-ms', '1000', '--quiet', ...small]);
  assert.equal(r.status, 1, r.stderr);
  const result = JSON.parse(readFileSync(join(out, 'bench.json'), 'utf8'));
  assert.equal(result.cases[0].runs[0].status, 'generation_failed');
  assert.match(result.cases[0].runs[0].error.detail, /timeout/);
  assert.deepEqual(readdirSync(work), []);
  for (const f of readdirSync(fake.marks).filter((n) => n.startsWith('pid-'))) assert.equal(alive(Number(f.slice(4))), false, 'fake Core exited');
});

test('a read without a result is measured as missing, never dropped', () => {
  const tags = new TagBook();
  const t = tags.add('current_step', 'cur', 'define');
  const ok = { content: [{ type: 'text', text: `context body ${t}` }], structuredContent: { ok: true, said: `context body ${t}` } };
  const events = [
    { seq: 1, at: null, actor: 'harness', synthetic: true, request: { name: 'gil_context', arguments: {} }, result: ok, error: null },
    { seq: 2, at: null, actor: 'harness', synthetic: true, request: { name: 'gil_status', arguments: {} }, result: null, error: null },
  ];
  const m = measureReadPhase({ events, rawReads: [{ raw_text: `context body ${t}`, ms: 1 }, { raw_text: null, ms: 1000, note: 'timeout' }], tags: tags.rows, markers: [], expectedReads: ['gil_context', 'gil_status', 'gil_plan_read'] });
  assert.equal(m.missing, true);
  assert.deepEqual(m.fields.missing_reads, ['gil_status', 'gil_plan_read']);
  assert.equal(m.fields.reads[1].status, 'missing_result');
  assert.equal(m.fields.reads[1].text_response, null, 'absent text is null, not 0');
  assert.equal(m.fields.metrics.status_totals.missing_result, 1);
  assert.equal(m.fields.resolution.rows.find((r) => r.layer === 'current_step').verdict, 'ok');
  // Without a usable gil_context the expectations are "not measured", not "ok".
  const noCtx = measureReadPhase({ events: [{ ...events[0], result: null }], rawReads: [{}], tags: tags.rows, markers: [], expectedReads: ['gil_context'] });
  assert.equal(noCtx.fields.resolution.rows[0].verdict, 'not_measured');
});

test('SIGINT writes an interrupted result, kills the Core and removes the temp project', async () => {
  const fake = fakeCore('hang');
  const work = join(fake.dir, 'work'); mkdirSync(work);
  const out = join(fake.dir, 'out');
  const child = spawn(process.execPath, [cli, 'run', '--gil', fake.script, '--out', out, '--work-root', work, '--timeout-ms', '60000', '--quiet', ...small], { stdio: ['ignore', 'pipe', 'pipe'] });
  const deadline = Date.now() + 20000;
  while (!readdirSync(fake.marks).some((n) => n.startsWith('called-'))) {
    assert.ok(Date.now() < deadline, 'fake Core was never called');
    await new Promise((r) => setTimeout(r, 50));
  }
  assert.equal(readdirSync(work).length, 1, 'a temporary project exists while running');
  child.kill('SIGINT');
  const code = await new Promise((r) => child.on('exit', (c) => r(c)));
  assert.equal(code, 130);
  const result = JSON.parse(readFileSync(join(out, 'bench.json'), 'utf8'));
  assert.equal(result.status, 'interrupted');
  assert.equal(result.interrupted_by, 'SIGINT');
  assert.ok(result.incomplete.some((x) => x.status === 'interrupted'));
  assert.deepEqual(readdirSync(work), [], 'temporary project removed');
  await new Promise((r) => setTimeout(r, 200));
  for (const f of readdirSync(fake.marks).filter((n) => n.startsWith('pid-'))) assert.equal(alive(Number(f.slice(4))), false, 'fake Core killed');
});

// ── measurement helpers ────────────────────────────────────────────────────────────────────

test('read-only check sees content, added and removed entries', () => {
  const root = scratch('tree');
  mkdirSync(join(root, '.gil'));
  writeFileSync(join(root, '.gil', 'state.yaml'), 'a');
  const a = treeDigest(root);
  assert.deepEqual(diffTrees(a, treeDigest(root)), []);
  writeFileSync(join(root, '.gil', 'state.yaml'), 'b');
  writeFileSync(join(root, 'new.txt'), 'x');
  assert.deepEqual(diffTrees(a, treeDigest(root)), [{ path: '.gil/state.yaml', change: 'modified' }, { path: 'new.txt', change: 'added' }]);
});

test('read-only checks include external records and settings, not just project files', () => {
  const root = scratch('external-record');
  const paths = Object.fromEntries(['project', 'settings', 'record'].map(key => {
    const path = join(root, key); mkdirSync(path); return [key, path];
  }));
  writeFileSync(join(paths.record, 'state.yaml'), 'before');
  const before = snapshotReadRoots(paths);
  assert.equal(readOnlyUnchanged(compareReadRoots(before, snapshotReadRoots(paths))), true);
  writeFileSync(join(paths.record, 'state.yaml'), 'after');
  const changed = compareReadRoots(before, snapshotReadRoots(paths));
  assert.equal(changed.project.unchanged, true);
  assert.equal(changed.record.unchanged, false);
  assert.equal(readOnlyUnchanged(changed), false);
  writeFileSync(join(paths.record, 'state.yaml'), 'before');
  writeFileSync(join(paths.settings, 'changed'), 'after');
  assert.equal(readOnlyUnchanged(compareReadRoots(before, snapshotReadRoots(paths))), false);
  assert.equal(readOnlyUnchanged({ project: { unchanged: true }, settings: { unchanged: true } }), null);
});

test('sections come from the locale headings; text parts before the body are kept apart', () => {
  const table = { 'ctx.preamble': 'GIL context — head.\n', 'ctx.previous_head': '\n═══ 이전 Cycle ═══\n({n}개)\n', 'ctx.chain_previous': '\n이전 Chain: {id} · {verdict}\n', 'ctx.plan_profile': '\n승인 계획 프로필: 설명 {x}\n' };
  const { markers, missing } = sectionMarkers(table);
  assert.ok(missing.includes('ctx.here_head'));
  assert.deepEqual(markers.map((m) => m.prefix), ['GIL context — head.', '승인 계획 프로필:', '이전 Chain:', '═══ 이전 Cycle ═══']);
  const s = segmentContext(['monitor nudge', 'GIL context — head.\n\n이전 Chain: chain:C1\na\n\n이전 Chain: chain:C2\nb\n═══ 이전 Cycle ═══\nx'], markers);
  const byName = Object.fromEntries(s.map((x) => [x.name, x]));
  assert.equal(byName.text_part_1.size.utf8_bytes, Buffer.byteLength('monitor nudge'));
  assert.equal(byName.chain_previous.occurrences, 2);
  assert.equal(byName.previous_cycles.occurrences, 1);
});

test('resolution check flags expanded inner Steps and missing projections', () => {
  const tags = new TagBook();
  const proj = tags.add('prev_cycle_projection', 'k1', 'define');
  const inner = tags.add('prev_cycle_inner', 'k1', 'verify');
  const constraint = tags.add('plan_constraint', 'plan', 'constraint');
  const good = evaluateResolution(tags.rows, [{ tool: 'gil_context', status: 'ok', text: proj, structured: null }, { tool: 'gil_plan_read', status: 'ok', text: constraint, structured: null }]);
  assert.deepEqual(good.violations, []);
  const bad = evaluateResolution(tags.rows, [{ tool: 'gil_context', status: 'ok', text: inner, structured: null }]);
  assert.deepEqual(bad.violations.map((v) => v.kind).sort(), ['expanded_in_context', 'missing_in_context', 'missing_in_reads']);
  assert.ok(!'ccbT-k10-define'.includes(tags.rows[0].tag + 'x'));
  assert.throws(() => tags.add('prev_cycle_projection', 'k1', 'define'), /duplicate/);
});

test('repeat comparison separates size, content and missing runs', () => {
  const run = (bytes, sha, status = 'complete') => ({ status, reads: [{ tool: 'gil_context', text_response: { utf8_bytes: bytes }, structured_response: { utf8_bytes: 10 }, text_sha: sha }] });
  assert.equal(compareRepeats([run(100, 'a'), run(100, 'a')]).identical, true);
  const diff = compareRepeats([run(100, 'a'), run(104, 'b')]);
  assert.equal(diff.identical, false);
  assert.equal(diff.rows[0].max_text_delta_utf8_bytes, 4);
  assert.equal(compareRepeats([run(100, 'a'), run(0, null, 'generation_failed')]).identical, null);
  assert.equal(normalizedSha('x project:' + 'a'.repeat(64)), normalizedSha('x project:' + 'b'.repeat(64)));
});

test('compare flags differing conditions and reports per-call deltas', () => {
  const root = scratch('cmp');
  const mk = (bytes, locale) => ({ schema: 'gil-chain-context-bench/0.1', core: { sha256: 'x', features_declared: 'approved-plan' }, conditions: { locale, reads: ['gil_context'] }, plan: { base: BASE }, cases: [{ id: 'case-01', scale: BASE, runs: [{ status: 'complete', reads: [{ tool: 'gil_context', text_response: { utf8_bytes: bytes }, structured_response: { utf8_bytes: 5 }, text_sha: String(bytes) }] }] }] });
  writeFileSync(join(root, 'a.json'), JSON.stringify(mk(100, 'ko-KR')));
  writeFileSync(join(root, 'b.json'), JSON.stringify(mk(120, 'ko-KR')));
  writeFileSync(join(root, 'c.json'), JSON.stringify(mk(120, 'en')));
  const same = JSON.parse(compare({ before: join(root, 'a.json'), after: join(root, 'b.json') }).output);
  assert.deepEqual(same.conditions_differ, []);
  assert.equal(same.cases[0].reads[0].text_utf8_bytes.delta, 20);
  // The compact summary.json is enough for compare.
  writeFileSync(join(root, 'a-summary.json'), JSON.stringify(compactResult(mk(100, 'ko-KR'))));
  const fromSummary = JSON.parse(compare({ before: join(root, 'a-summary.json'), after: join(root, 'b.json') }).output);
  assert.equal(fromSummary.cases[0].reads[0].text_utf8_bytes.delta, 20);
  const other = JSON.parse(compare({ before: join(root, 'a.json'), after: join(root, 'c.json') }).output);
  assert.deepEqual(other.conditions_differ, ['locale']);
  assert.match(other.caution, /같은 조건의 재실행 비교가 아니다/);
});

test('repetition evidence: exact and template repeats, blocks shared by every level', () => {
  const p = blockProfile(['Cycle 2 · failure\n\n  실험 목적 ccbT-f1-define\n\n  원본 Cycle: cycle:C2\n\nsame paragraph\n\nsame paragraph\n\n  원본 Cycle: cycle:C3']);
  assert.equal(p.exact_repeats.extra_occurrences, 1);
  assert.ok(p.template_repeats.occurrences >= 3, 'cycle:C2 / cycle:C3 lines share one template');
  assert.equal(templateOf('원본 Cycle: cycle:C12 ccbT-f2-x'), '원본 Cycle: cycle:C# <tag>');
  const inv = invariantAcross([blockProfile(['fixed text\n\nlevel 0']), blockProfile(['fixed text\n\nlevel 1 longer'])]);
  assert.equal(inv.shared_blocks, 1);
  assert.equal(inv.per_level[1].varying_utf8_bytes, Buffer.byteLength('level 1 longer'));
});

test('line repeats catch a sentence repeated across calls inside different paragraphs', () => {
  const rule = '프로젝트 파일 변경은 실제로 열린 Verify에서만 수행한다. 이는 OS 쓰기 차단이 아니다.';
  const r = lineRepeats([{ tool: 'gil_context', parts: [`재개 안내\n${rule}`] }, { tool: 'gil_status', parts: [`위치 안내\n${rule}`, 'short'] }]);
  assert.equal(r.repeated_lines, 1);
  assert.equal(r.lines[0].across_tools, true);
  assert.equal(r.extra_utf8_bytes, Buffer.byteLength(rule));
  assert.ok(!JSON.stringify(r).includes('Verify'), 'line text is not reported');
});

test('report fields keep one-line and block forms; detail lines are distinct', () => {
  assert.equal(field('a', 'x'), 'a: x\n');
  assert.equal(field('a', ['x', 'y']), 'a: |\n  x\n  y\n');
  const lines = detailLines('ccbT-t', '항목', 3);
  assert.equal(new Set(lines).size, 3);
  assert.ok(lines[0].startsWith('ccbT-t '));
});

test('results are sanitized of machine paths or not written at all', () => {
  const home = '/Users/someone';
  assert.equal(sanitize(`{"p":"${home}/x"}`, [['<home>', home]]), '{"p":"<home>/x"}');
});

// ── recovery after a failed Experiment ─────────────────────────────────────────────────────

const PLAN_TOKEN = 'plan-capability-0123456789abcdef';
const RECOVERY_TOKEN = (cycle) => `recovery-capability-for-C${cycle}-0123456789abcdef`;

/**
 * In-process stand-in for the Core's answers along the generator's walk (not a Core: it only
 * hands out refs). `revisit(name, args, state)` answers the recovery tools.
 */
function stubWalk({ recovery, revisit, blocked = false }) {
  const calls = []; const secrets = [];
  let cycle = 1; let step = 0;
  const reply = (structured, meta = null, ok = true) => ({ ok, structured, meta, result: { content: [{ type: 'text', text: structured.said ?? '' }] }, error: null });
  const answer = async (name, args) => {
    calls.push({ name, args });
    if (name === 'gil_start') return reply({ said: `started project:${'a'.repeat(64)}` });
    if (name === 'gil_open') {
      if (['experiment', 'interview', 'chain'].includes(args.kind)) { cycle += 1; step = 0; return reply({ said: `opened cycle:C${cycle}` }); }
      step += 1; return reply({ said: `opened step:C${cycle}/S${step}` });
    }
    if (name === 'gil_plan_review') return reply({ proposal: { revision: 1, digest: 'plan-digest' } }, { 'gil/planApprovalToken': PLAN_TOKEN });
    if (name === 'gil_plan_read') return reply({ close_report: 'approved: yes', current_item: { id: 'I1' }, document: '# plan' });
    if (name.startsWith('gil_revisit')) return revisit(name, args, { cycle, reply });
    return reply({ said: 'ok' });
  };
  const g = { root: '/stub/project', locale: 'ko-KR', typedVerify: false, recovery, modelRevisitBlockedByProfile: blocked, model: answer, app: answer, writeProjectFile: async () => {}, onSecret: (s) => secrets.push(s) };
  return { g, calls, secrets, names: () => calls.map((c) => c.name) };
}
const approvalCore = (name, args, { cycle, reply }) => {
  if (name === 'gil_revisit_review') return reply({ supported: true, state: 'awaiting_approval', digest: `digest-C${cycle}`, from: `cycle:C${cycle}`, target: 'cycle:C1', changes: [{ path: 'plan/approved-plan.md', action: 'preserve' }, { path: 'evidence/x.md', action: 'preserve' }], pending_state_sha256: `key-C${cycle}`, policy: 'legacy-whole-tree-protected-difference-v1' }, { 'gil/revisitApprovalToken': RECOVERY_TOKEN(cycle) });
  if (name === 'gil_revisit_apply') return args.digest === `digest-C${cycle}` && args.token === RECOVERY_TOKEN(cycle) ? reply({ state: 'completed', approval_source: 'monitor_button' }) : reply({ ok: false, said: '', problem: 'approve in Monitor' }, null, false);
  return reply({ ok: false, said: '', problem: 'approve in Monitor' }, null, false);
};
const twoFailures = { ...BASE, failures: 2 };

test('the recovery path follows the tools the Core lists', () => {
  assert.equal(recoveryPathFor(['gil_revisit', 'gil_revisit_review', 'gil_revisit_apply']), RECOVERY.app_approval);
  assert.equal(recoveryPathFor(['gil_revisit']), RECOVERY.model_revisit);
  assert.equal(recoveryPathFor(['gil_revisit', 'gil_revisit_review']), RECOVERY.model_revisit, 'half of the approval path is not a path');
  assert.equal(recoveryPathFor(['gil_context']), RECOVERY.none);
  assert.equal(recoveryForFeatures('approved-plan,visual-authoring,product-safety'), 'app_approval');
  assert.equal(recoveryForFeatures('approved-plan,visual-authoring'), 'model_revisit');
  // review + apply instead of one gil_revisit: one more call per failure, known before any Core runs.
  assert.equal(estimateCalls(twoFailures, [], 'app_approval') - estimateCalls(BASE, [], 'app_approval'), 30);
  assert.equal(estimateCalls(BASE, [], 'app_approval'), estimateCalls(BASE, []));
  assert.equal(parseArgs(['plan', '--axes', 'failures', '--features', 'approved-plan,product-safety']).plan.recovery_assumed, 'app_approval');
  assert.equal(parseArgs(['plan', '--axes', 'failures']).plan.recovery_assumed, 'model_revisit');
});

test('a product Core is recovered through review → apply with the Core\'s own digest and capability', async () => {
  const w = stubWalk({ recovery: RECOVERY.app_approval, revisit: approvalCore });
  const got = await generate(w.g, twoFailures);
  assert.ok(!w.names().includes('gil_revisit'), 'a model revisit is never attempted on the approval path');
  const applies = w.calls.filter((c) => c.name === 'gil_revisit_apply');
  assert.deepEqual(applies.map((c) => c.args.digest), ['digest-C2', 'digest-C3']);
  assert.deepEqual(applies.map((c) => c.args.token), [RECOVERY_TOKEN(2), RECOVERY_TOKEN(3)], 'each approval uses the capability of its own review');
  for (const c of w.calls.filter((x) => x.name.startsWith('gil_revisit_'))) assert.equal(c.args.scope_id, `project:${'a'.repeat(64)}`);
  assert.equal(w.names().filter((n) => n === 'gil_revisit_review').length, 2);
  assert.deepEqual([got.counts.failures, got.counts.revisits, got.counts.recovery_reviews, got.counts.recovery_approvals], [2, 2, 2, 2]);
  assert.deepEqual(got.failed.map((f) => [f.cycle, f.item]), [[2, 'I1'], [3, 'I1']]);
  assert.deepEqual(got.recoveries.map((r) => [r.path, r.from, r.target, r.changes.preserve, r.preserved_key]), [['app_approval', 'cycle:C2', 'cycle:C1', 2, 'key-C2'], ['app_approval', 'cycle:C3', 'cycle:C1', 2, 'key-C3']]);
  assert.equal(got.current.cycle, 4, 'the retry opens after both recoveries');
  // The capability is reported to the redactor and is not part of what generation returns.
  assert.deepEqual(w.secrets.filter((s) => s.startsWith('recovery-')), [RECOVERY_TOKEN(2), RECOVERY_TOKEN(3)]);
  const serialized = JSON.stringify(got);
  for (const s of w.secrets) assert.ok(!serialized.includes(s), 'no capability in the generation result');
});

test('an unsupported condition and a failed recovery call are different results', async () => {
  const run = (opts) => generate(stubWalk(opts).g, { ...BASE, failures: 1 });
  const refused = (_n, _a, { reply }) => reply({ ok: false, said: '', problem: 'refused' }, null, false);
  // The Core answers the review but reports that approved recovery does not exist here.
  const unsupported = await run({ recovery: RECOVERY.app_approval, revisit: (name, _a, { reply }) => reply({ supported: false, reason: 'no approved recovery on this platform' }) }).catch((e) => e);
  assert.ok(unsupported instanceof UnsupportedError);
  assert.equal(unsupported.kind, 'unsupported_condition');
  assert.match(unsupported.step, /revisit review/);
  // The Core refuses the review (for example unrecorded file changes): a failed call, not "unsupported".
  const reviewRefused = await run({ recovery: RECOVERY.app_approval, revisit: refused }).catch((e) => e);
  assert.ok(reviewRefused instanceof GenerationError && !(reviewRefused instanceof UnsupportedError));
  assert.equal(reviewRefused.kind, 'core_refusal');
  // The approval itself is refused.
  const applyRefused = await run({ recovery: RECOVERY.app_approval, revisit: (name, args, st) => (name === 'gil_revisit_review' ? approvalCore(name, args, st) : refused(name, args, st)) }).catch((e) => e);
  assert.equal(applyRefused.kind, 'core_refusal');
  assert.match(applyRefused.step, /revisit apply/);
  // A review without a capability, or for another Cycle, is never turned into an approval.
  for (const broken of [
    (name, _a, { cycle, reply }) => reply({ supported: true, state: 'awaiting_approval', digest: 'd', from: `cycle:C${cycle}`, target: 'cycle:C1' }),
    (name, _a, { reply }) => reply({ supported: true, state: 'not_needed' }, { 'gil/revisitApprovalToken': RECOVERY_TOKEN(0) }),
    (name, _a, { reply }) => reply({ supported: true, state: 'awaiting_approval', digest: 'd', from: 'cycle:C9', target: 'cycle:C1' }, { 'gil/revisitApprovalToken': RECOVERY_TOKEN(0) }),
  ]) {
    const w = stubWalk({ recovery: RECOVERY.app_approval, revisit: broken });
    const err = await generate(w.g, { ...BASE, failures: 1 }).catch((e) => e);
    assert.equal(err.kind, 'harness', err.message);
    assert.ok(!w.names().includes('gil_revisit_apply'), 'no approval is sent for an unusable review');
    assert.ok(!err.message.includes('recovery-capability'), 'the capability is not quoted in the error');
  }
  // Without product protection the model path is used; its refusal is a failed call …
  const direct = stubWalk({ recovery: RECOVERY.model_revisit, revisit: (_n, _a, { reply }) => reply({ said: 'revisited' }) });
  const got = await generate(direct.g, twoFailures);
  assert.equal(direct.names().filter((n) => n === 'gil_revisit').length, 2);
  assert.ok(!direct.names().some((n) => n.startsWith('gil_revisit_')));
  assert.deepEqual([got.counts.revisits, got.counts.recovery_approvals], [2, 0]);
  assert.equal((await run({ recovery: RECOVERY.model_revisit, revisit: refused }).catch((e) => e)).kind, 'core_refusal');
  // … unless the declared profile blocks it by design and the Core offers no approval tools.
  assert.ok((await run({ recovery: RECOVERY.model_revisit, revisit: refused, blocked: true }).catch((e) => e)) instanceof UnsupportedError);
  assert.ok((await run({ recovery: RECOVERY.none, revisit: refused }).catch((e) => e)) instanceof UnsupportedError);
  assert.equal(errorKind(new Error('read: timeout after 1000 ms waiting for tools/call gil_open')), 'transport');
  assert.equal(errorKind(new Error('receipt names no step ref')), 'harness');
});

test('lineage check: failure evidence, the current item and the retry link must survive', () => {
  const tags = new TagBook();
  const walk = { failed: [], current: { cycle: 4, item: 'I1' } };
  for (const f of [1, 2]) { walk.failed.push({ attempt: f, cycle: f + 1, item: 'I1', prefix: `f${f}` }); for (const what of ['define', 'lesson', 'handoff', 'reason']) tags.add('failure_projection', `f${f}`, what); }
  const ctxText = `${tags.rows.map((t) => t.tag).join(' ')} 원본 Cycle: cycle:C2 … cycle:C3 … cycle:C4`;
  const plan = (executed, item = 'I1', cycle = 'cycle:C4') => JSON.stringify({ current_item: { id: item }, current_cycle: { ref: cycle }, executed_cycles: executed.map(([c, i]) => ({ cycle_ref: c, item_id: i, revision: 1 })) });
  const all = [['cycle:C2', 'I1'], ['cycle:C3', 'I1'], ['cycle:C4', 'I1']];
  const reads = (text, structured) => [{ tool: 'gil_context', status: 'ok', text, structured: null }, { tool: 'gil_status', status: 'ok', text: '갈래 출처: cycle:C3', structured: null }, { tool: 'gil_plan_read', status: 'ok', text: '', structured }];
  const sections = [{ name: 'failed_cycles' }, { name: 'current_cycle' }];
  const good = evaluateLineage(walk, tags.rows, reads(ctxText, plan(all)), sections);
  assert.deepEqual(good.violations, []);
  assert.equal(lineageHolds(good), true);
  assert.equal(good.status_names_revisit_source, true);
  assert.deepEqual(good.checks.map((c) => [c.name, c.observed, c.expected]), [['failure_refs', 2, 2], ['failure_tags', 8, 8], ['failure_section', true, true], ['current_item', 'I1@cycle:C4', 'I1@cycle:C4'], ['retry_links', 3, 3], ['retry_links_in_context', 3, 3]]);
  // A dropped failed attempt, a lost tag, another item or a missing section are each reported.
  const lost = evaluateLineage(walk, tags.rows, reads(ctxText.replace('ccbT-f1-lesson', '').replace('cycle:C2', 'cycle:C21'), plan([['cycle:C3', 'I1'], ['cycle:C4', 'I2']], 'I2')), [{ name: 'current_cycle' }]);
  assert.deepEqual(lost.violations.map((v) => v.check), ['failure_refs', 'failure_tags', 'failure_section', 'current_item', 'retry_links', 'retry_links_in_context']);
  assert.deepEqual(lost.violations[0].missing, ['cycle:C2'], 'cycle:C21 does not count as cycle:C2');
  assert.equal(lineageHolds(lost), false);
  // No failures: nothing to preserve, and a failed-Cycle section would be a phantom.
  const none = { failed: [], current: { cycle: 2, item: 'I1' } };
  assert.equal(lineageHolds(evaluateLineage(none, [], reads('cycle:C2', plan([['cycle:C2', 'I1']], 'I1', 'cycle:C2')), [{ name: 'current_cycle' }])), true);
  assert.deepEqual(evaluateLineage(none, [], reads('cycle:C2', plan([['cycle:C2', 'I1']], 'I1', 'cycle:C2')), sections).violations.map((v) => v.check), ['failure_section']);
  // Reads that were not requested or have no result are "not measured", never "ok".
  const partial = evaluateLineage(walk, tags.rows, [{ tool: 'gil_context', status: 'ok', text: ctxText, structured: null }], null);
  assert.deepEqual(partial.not_measured, ['failure_section', 'current_item', 'retry_links']);
  assert.equal(lineageHolds(partial), null);
  assert.equal(partial.status_names_revisit_source, null);
});

test('preserved originals are checked by bytes and a stored capability is reported', () => {
  const record = join(scratch('preserved'), '.gil');
  const sha = (text) => createHash('sha256').update(text).digest('hex');
  const recoveries = [{ path: 'app_approval', from: 'cycle:C2', target: 'cycle:C1', preserved_key: 'k1' }, { path: 'app_approval', from: 'cycle:C3', target: 'cycle:C1', preserved_key: 'k2' }];
  const make = (key, from, files, source = { approval_source: 'monitor_button', human_click: true }) => {
    const dir = join(record, 'revisit-preserved', key); mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, 'approval.json'), JSON.stringify({ state: 'approved', from, target: 'cycle:C1' }));
    writeFileSync(join(dir, 'files.json'), JSON.stringify(files.map((text, i) => ({ path: `p${i}`, saved_as: `file-${i}`, sha256: sha(text) }))));
    writeFileSync(join(dir, 'source.json'), JSON.stringify(source));
    files.forEach((text, i) => writeFileSync(join(dir, `file-${i}`), text));
    return dir;
  };
  make('k1', 'cycle:C2', ['plan', 'evidence']);
  const second = make('k2', 'cycle:C3', ['plan', 'evidence 2']);
  const ok = inspectPreserved(record, recoveries, ['capability-0123456789']);
  assert.deepEqual([ok.expected, ok.archives, ok.complete, ok.original_files, ok.originals_verified, ok.approvals_stored_as_approved], [2, 2, 2, 4, 4, 2]);
  assert.deepEqual(ok.core_source_records, { monitor_button: 2 });
  assert.equal(ok.core_records_claim_human_click, 2, 'what the Core wrote is reported as the Core\'s record, not as a person\'s click');
  assert.equal(ok.capability_in_storage, false);
  assert.deepEqual(ok.problems, []);
  // A changed original, a missing folder and a capability on disk are all visible.
  writeFileSync(join(second, 'file-1'), 'tampered');
  writeFileSync(join(second, 'approval.json'), JSON.stringify({ state: 'approved', from: 'cycle:C3', target: 'cycle:C1', token: 'capability-0123456789' }));
  const bad = inspectPreserved(record, [...recoveries, { path: 'app_approval', from: 'cycle:C4', target: 'cycle:C1', preserved_key: 'k3' }], ['capability-0123456789']);
  assert.equal(bad.originals_verified, 3);
  assert.equal(bad.archives, 2);
  assert.equal(bad.capability_in_storage, true);
  assert.equal(bad.problems.length, 2);
  // The model path preserves nothing through this contract and is not inspected.
  assert.equal(inspectPreserved(record, [{ path: 'model_revisit', preserved_key: null }]).expected, 0);
});

test('a result with an approval capability is not written; recovery conditions never claim a person', () => {
  assert.equal(refuseSecrets('{"a":1}', new Set(['capability-0123456789'])), '{"a":1}');
  assert.throws(() => refuseSecrets('{"token":"capability-0123456789"}', new Set(['capability-0123456789'])), /nothing written/);
  const lineage = { failed_attempts: 1, failed_cycles: ['cycle:C2'], current: { cycle: 'cycle:C3', item: 'I1' }, checks: [{ name: 'failure_refs', expected: 1, observed: 1, verdict: 'ok' }], violations: [], not_measured: [], status_names_revisit_source: true };
  const preserved = { expected: 1, archives: 1, complete: 1, original_files: 4, originals_verified: 4, core_source_records: { monitor_button: 1 }, core_records_claim_human_click: 1, approvals_stored_as_approved: 1, capability_in_storage: false, problems: [] };
  const run = { run: 1, status: 'complete', error: null, reads: [], lineage, readonly: { project: { unchanged: true }, settings: { unchanged: true }, record: { unchanged: true } }, storage: { preserved }, generation: { calls: 36, refused_calls: 0, recovery: { path: 'app_approval', recoveries: 1, synthetic_approvals: 1 } } };
  const result = { schema: 'gil-chain-context-bench/0.1', status: 'complete', core: { observed: { recovery_path: 'app_approval', tools: 33, revisit_approval_tools: true } }, conditions: {}, plan: { base: BASE }, cases: [{ id: 'case-02', scale: { ...BASE, failures: 1 }, axes: [], status: 'complete', runs: [run] }], incomplete: [], limits: [] };
  const c = recoveryConditions(result, 'app_approval');
  assert.deepEqual([c.path_offered_by_core, c.matches_declared_features, c.recoveries, c.synthetic_approvals, c.human_approvals], ['app_approval', true, 1, 1, 0]);
  assert.match(c.approval_input, /synthetic harness input/);
  assert.match(c.core_source_note, /not evidence of a person/);
  assert.equal(recoveryConditions(result, 'model_revisit').matches_declared_features, false, 'declared features that promise the other path are flagged');
  const rows = preservation(result);
  assert.deepEqual([rows.cases, rows.cases_with_failures, rows.violations], [1, 1, 0]);
  assert.equal(rows.rows[0].preserved_ok_in_every_run, true);
  result.conditions.recovery = c; result.summary = { axes: [], steepest_context_growth: [], preservation: rows };
  const md = formatMarkdown(result);
  assert.match(md, /harness 합성 승인 1회 · 사람의 승인 0회/);
  assert.match(md, /사람 클릭의 증거가 아니다/);
  const compact = compactResult(result).cases[0].runs[0];
  assert.deepEqual([compact.lineage.failed_attempts, compact.lineage.holds, compact.generation.recovery.synthetic_approvals], [1, true, 1]);
  // An incomplete run says which kind of failure it was.
  result.incomplete = [{ case_id: 'case-03', run: 1, status: 'unsupported', error: { step: 'x', kind: 'unsupported_condition', detail: 'd' } }, { case_id: 'case-04', run: 1, status: 'generation_failed', error: { step: 'y', kind: 'core_refusal', detail: 'd' } }];
  const md2 = formatMarkdown(result);
  assert.match(md2, /unsupported \| 지원되지 않는 조건/);
  assert.match(md2, /generation_failed \| Core가 호출을 거절/);
});

// ── optional real Core ─────────────────────────────────────────────────────────────────────

/** What every finished real-Core run must show, whichever recovery path the build offers. */
function assertPreserved(result, path) {
  assert.equal(result.status, 'complete');
  assert.equal(result.core.observed.recovery_path, path);
  assert.equal(result.conditions.recovery.matches_declared_features, true);
  assert.equal(result.conditions.recovery.human_approvals, 0);
  for (const c of result.cases) {
    assert.equal(c.repeat.identical, true, `${c.id} repeats differ`);
    for (const run of c.runs) {
      assert.equal(readOnlyUnchanged(run.readonly), true);
      assert.deepEqual(run.resolution.violations, []);
      assert.deepEqual(run.lineage.violations, [], `${c.id} lost failure evidence, the current item or a retry link`);
      assert.deepEqual(run.lineage.not_measured, []);
      assert.equal(run.lineage.failed_attempts, c.scale.failures);
      assert.equal(run.generation.recovery.recoveries, c.scale.failures);
      assert.equal(run.generation.refused_calls, 0, 'no refused call on the way');
      if (c.scale.failures) assert.equal(run.generation.recovery.path, path);
    }
  }
  assert.equal(result.summary.preservation.violations, 0);
}

test('real Core smoke run (set CHAIN_CONTEXT_BENCH_GIL to a gil built with approved-plan,visual-authoring)', { skip: !process.env.CHAIN_CONTEXT_BENCH_GIL }, () => {
  const root = scratch('real');
  const out = join(root, 'out');
  const r = runCli(['run', '--gil', process.env.CHAIN_CONTEXT_BENCH_GIL, '--out', out, '--work-root', root, '--quiet', '--axes', 'failures', '--levels', 'failures=0,1,2', '--repeat', '2'], { timeout: 300000 });
  assert.equal(r.status, 0, r.stderr);
  const result = JSON.parse(readFileSync(join(out, 'bench.json'), 'utf8'));
  assertPreserved(result, 'model_revisit');
  assert.equal(result.conditions.recovery.synthetic_approvals, 0);
  for (const run of result.cases.flatMap((c) => c.runs)) assert.equal(run.storage.preserved, null);
  const two = result.summary.axes[0].levels.find((l) => l.level === 2);
  assert.ok(two.context_sections_utf8_bytes.failed_cycles > 0);
});

test('real product Core: accumulated failures through the approved recovery path (set CHAIN_CONTEXT_BENCH_GIL_PRODUCT to a gil built with approved-plan,visual-authoring,product-safety)', { skip: !process.env.CHAIN_CONTEXT_BENCH_GIL_PRODUCT }, () => {
  const root = scratch('real-product');
  const out = join(root, 'out');
  const r = runCli(['run', '--gil', process.env.CHAIN_CONTEXT_BENCH_GIL_PRODUCT, '--features', 'approved-plan,visual-authoring,product-safety', '--out', out, '--work-root', root, '--quiet', '--axes', 'failures', '--levels', 'failures=0,1,2', '--repeat', '2'], { timeout: 300000 });
  assert.equal(r.status, 0, r.stderr);
  const result = JSON.parse(readFileSync(join(out, 'bench.json'), 'utf8'));
  assertPreserved(result, 'app_approval');
  assert.equal(result.core.observed.revisit_approval_tools, true);
  // 0 + 1 + 2 failures, twice: every recovery is one synthetic harness approval, none a person's.
  assert.equal(result.conditions.recovery.recoveries, 6);
  assert.equal(result.conditions.recovery.synthetic_approvals, 6);
  assert.deepEqual(result.conditions.recovery.paths_used, ['app_approval']);
  for (const c of result.cases) for (const run of c.runs) {
    const kept = run.storage.preserved;
    if (!c.scale.failures) { assert.equal(kept, null); continue; }
    assert.deepEqual([kept.archives, kept.complete, kept.approvals_stored_as_approved], [c.scale.failures, c.scale.failures, c.scale.failures]);
    assert.equal(kept.originals_verified, kept.original_files);
    assert.ok(kept.original_files > 0);
    assert.equal(kept.capability_in_storage, false);
    assert.deepEqual(kept.problems, []);
    assert.equal(run.generation.calls, 21 + 15 * c.scale.failures, 'the estimate matches the walk');
  }
  // Nothing that looks like a 256-bit capability reached any written file.
  for (const name of ['bench.json', 'summary.json', 'bench.md']) assert.ok(!/revisitApprovalToken|planApprovalToken/.test(readFileSync(join(out, name), 'utf8')), name);
  assert.match(readFileSync(join(out, 'bench.md'), 'utf8'), /사람의 승인 0회/);
});
