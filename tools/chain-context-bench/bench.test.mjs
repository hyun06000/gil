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
import { parseArgs, sanitize, compare, compactResult, UsageError } from './cli.mjs';
import { buildPlan, estimateCalls, BASE, AXES, CALL_BUDGET } from './lib/plan.mjs';
import { measureReadPhase } from './lib/run.mjs';
import { snapshotReadRoots, compareReadRoots, readOnlyUnchanged } from './lib/measure.mjs';
import { treeDigest, diffTrees, segmentContext, sectionMarkers, evaluateResolution, compareRepeats, blockProfile, invariantAcross, templateOf, normalizedSha, lineRepeats } from './lib/measure.mjs';
import { field, detailLines, TagBook } from './lib/scenario.mjs';

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

// ── optional real Core ─────────────────────────────────────────────────────────────────────

test('real Core smoke run (set CHAIN_CONTEXT_BENCH_GIL to a gil built with approved-plan,visual-authoring)', { skip: !process.env.CHAIN_CONTEXT_BENCH_GIL }, () => {
  const root = scratch('real');
  const out = join(root, 'out');
  const r = runCli(['run', '--gil', process.env.CHAIN_CONTEXT_BENCH_GIL, '--out', out, '--work-root', root, '--quiet', '--axes', 'failures', '--levels', 'failures=0,2', '--repeat', '2'], { timeout: 300000 });
  assert.equal(r.status, 0, r.stderr);
  const result = JSON.parse(readFileSync(join(out, 'bench.json'), 'utf8'));
  assert.equal(result.status, 'complete');
  for (const c of result.cases) {
    assert.equal(c.repeat.identical, true, `${c.id} repeats differ`);
    for (const run of c.runs) {
      assert.equal(run.readonly.project.unchanged, true);
      assert.equal(readOnlyUnchanged(run.readonly), true);
      assert.deepEqual(run.resolution.violations, []);
    }
  }
  const two = result.summary.axes[0].levels.find((l) => l.level === 2);
  assert.ok(two.context_sections_utf8_bytes.failed_cycles > 0);
});
