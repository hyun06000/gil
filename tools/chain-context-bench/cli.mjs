#!/usr/bin/env node
// Chain context bench — generate scale-controlled real Core records and measure what the read
// calls hand to a new session. Development tool; not part of the shipped runtime.
//
//   node tools/chain-context-bench/cli.mjs plan [--axes a,b] [--levels axis=0,1,2]... [--repeat N] [--reads t1,t2] [--max-calls N]
//   python3 coordination/parallel-wave-2/cargo-queue.py -- node tools/chain-context-bench/cli.mjs run --build --out <new dir> [plan options]
//   node tools/chain-context-bench/cli.mjs run --gil <path to gil> --features <as built> --out <new dir> [plan options]
//   node tools/chain-context-bench/cli.mjs compare --before <bench.json> --after <bench.json> [--json <new file>]
//
// Exit: 0 complete · 1 finished with generation failures / missing measurements · 2 usage or
// input error · 130 interrupted. No exit code depends on a measured size.
import { mkdirSync, writeFileSync, existsSync, readFileSync, statSync } from 'node:fs';
import { resolve, dirname, join } from 'node:path';
import { tmpdir, homedir } from 'node:os';
import { fileURLToPath } from 'node:url';
import { buildPlan, parseLevelList, AXIS_NAMES, DEFAULT_READS, DEFAULT_FEATURES, REPEAT, CALL_BUDGET } from './lib/plan.mjs';
import { newResult, runBench, finalize } from './lib/bench.mjs';
import { formatMarkdown } from './lib/report.mjs';
import { buildCore, sourceInfo, binaryLabel, fileSha256, live, cleanupSync } from './lib/run.mjs';
import { sectionMarkers } from './lib/measure.mjs';
import { leaks } from '../scenarios/lib/redact.mjs';
import { TOOL_VERSION as METRICS_VERSION } from '../context-metrics/lib/analyze.mjs';

export const TOOL_VERSION = '0.1.0';
const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, '..', '..');
const KNOWN_FEATURES = ['chain-core', 'approved-plan', 'visual-authoring', 'product-safety', 'recovery-auto-approval'];
const TIMEOUT = { default: 20000, min: 1000, max: 120000 };

export class UsageError extends Error {}

export function parseArgs(argv) {
  const o = { command: argv[0], gil: null, build: false, unqueued: false, out: null, features: DEFAULT_FEATURES, axes: [...AXIS_NAMES], levels: {}, repeat: REPEAT.default, reads: [...DEFAULT_READS], locale: 'ko-KR', timeoutMs: TIMEOUT.default, maxCalls: CALL_BUDGET.default, keep: false, recordings: true, workRoot: null, before: null, after: null, json: null, quiet: false };
  if (!['plan', 'run', 'compare'].includes(o.command)) throw new UsageError('usage: cli.mjs plan|run|compare [options] (see tools/chain-context-bench/README.md)');
  const csv = (s) => s.split(',').map((x) => x.trim()).filter(Boolean);
  for (let i = 1; i < argv.length; i += 1) {
    const a = argv[i];
    const next = () => { if (i + 1 >= argv.length || argv[i + 1].startsWith('--')) throw new UsageError(`${a} needs a value`); return argv[++i]; };
    const int = (v, what) => { if (!/^\d+$/.test(v)) throw new UsageError(`${what} takes a positive integer`); return Number(v); };
    if (a === '--gil') o.gil = resolve(next());
    else if (a === '--build') o.build = true;
    else if (a === '--unqueued') o.unqueued = true;
    else if (a === '--out') o.out = resolve(next());
    else if (a === '--features') o.features = next();
    else if (a === '--axes') o.axes = csv(next());
    else if (a === '--levels') {
      const v = next(); const eq = v.indexOf('=');
      if (eq < 1) throw new UsageError('--levels takes axis=n,n,...');
      const axis = v.slice(0, eq);
      if (o.levels[axis]) throw new UsageError(`--levels ${axis} given twice`);
      try { o.levels[axis] = parseLevelList(axis, v.slice(eq + 1)); } catch (err) { throw new UsageError(err.message); }
    }
    else if (a === '--repeat') o.repeat = int(next(), '--repeat');
    else if (a === '--reads') o.reads = csv(next());
    else if (a === '--locale') { o.locale = next(); if (!/^[A-Za-z][A-Za-z0-9_-]{1,34}$/.test(o.locale)) throw new UsageError('--locale takes a BCP 47 tag such as ko-KR'); }
    else if (a === '--timeout-ms') { o.timeoutMs = int(next(), '--timeout-ms'); if (o.timeoutMs < TIMEOUT.min || o.timeoutMs > TIMEOUT.max) throw new UsageError(`--timeout-ms takes ${TIMEOUT.min}..${TIMEOUT.max}`); }
    else if (a === '--max-calls') o.maxCalls = int(next(), '--max-calls');
    else if (a === '--keep') o.keep = true;
    else if (a === '--no-recordings') o.recordings = false;
    else if (a === '--work-root') o.workRoot = resolve(next());
    else if (a === '--before') o.before = resolve(next());
    else if (a === '--after') o.after = resolve(next());
    else if (a === '--json') o.json = resolve(next());
    else if (a === '--quiet') o.quiet = true;
    else throw new UsageError(`unknown argument ${a}`);
  }
  if (o.command === 'compare') {
    if (!o.before || !o.after) throw new UsageError('compare needs --before <bench.json> and --after <bench.json>');
    return o;
  }
  const features = o.features.split(',').map((f) => f.trim()).filter(Boolean);
  for (const f of features) if (!KNOWN_FEATURES.includes(f)) throw new UsageError(`--features: ${f} is not a bench-supported Core feature (${KNOWN_FEATURES.join(', ')})`);
  if (!features.includes('approved-plan')) throw new UsageError('--features must include approved-plan: the bench measures Chains and approved plan items');
  o.features = [...new Set(features)].sort().join(',');
  try { o.plan = buildPlan({ axes: o.axes, levels: o.levels, repeat: o.repeat, reads: o.reads, maxCalls: o.maxCalls }); } catch (err) { throw new UsageError(err.message); }
  if (o.command === 'run') {
    if (o.build === !!o.gil) throw new UsageError('run needs exactly one of --build or --gil <path>');
    if (!o.out) throw new UsageError('run needs --out <new directory>');
    if (existsSync(o.out)) throw new UsageError(`--out ${o.out} already exists; results are only written to a new directory`);
    if (o.gil && !existsSync(o.gil)) throw new UsageError(`--gil ${o.gil} does not exist`);
    if (o.workRoot && !(existsSync(o.workRoot) && statSync(o.workRoot).isDirectory())) throw new UsageError(`--work-root ${o.workRoot} is not an existing directory`);
  }
  return o;
}

function localeTable(locale) {
  const lang = locale.toLowerCase();
  const file = lang.startsWith('zh') ? (/(hant|tw|hk|mo)/.test(lang) ? 'zh-Hant' : 'zh-Hans') : ['ko', 'ja', 'en'].find((l) => lang.startsWith(l)) ?? 'en';
  try { return { file: `src/i18n/${file}.json`, table: JSON.parse(readFileSync(join(repo, 'src', 'i18n', `${file}.json`), 'utf8')) }; } catch { return { file: null, table: null }; }
}

/** Replace machine paths before writing; refuse to write if any remains. */
export function sanitize(text, roots) {
  let s = text;
  for (const [label, p] of roots.filter(([, p]) => typeof p === 'string' && p.length > 1).sort((a, b) => b[1].length - a[1].length)) s = s.split(p).join(label);
  const found = leaks(s, { roots: roots.map(([, p]) => p).filter(Boolean) });
  if (found.length) throw new Error('result still contains a machine path after sanitizing; nothing written');
  return s;
}

function writeResults(o, result) {
  const roots = [['<worktree>', repo], ['<work-root>', o.workRoot ?? tmpdir()], ['<tmp>', tmpdir()], ['<home>', homedir()]];
  const json = sanitize(JSON.stringify(result, null, 2) + '\n', roots);
  const md = sanitize(formatMarkdown(result).trimEnd() + '\n', roots);
  const summary = sanitize(JSON.stringify(compactResult(result), null, 2) + '\n', roots);
  writeFileSync(join(o.out, 'bench.json'), json, { flag: 'wx' });
  writeFileSync(join(o.out, 'summary.json'), summary, { flag: 'wx' });
  writeFileSync(join(o.out, 'bench.md'), md, { flag: 'wx' });
}

/** The part of a result worth keeping in a repository: conditions, summaries, per-read sizes. */
export function compactResult(result) {
  const { cases, ...rest } = result;
  return {
    ...rest, view: 'summary',
    cases: cases.map((c) => ({
      id: c.id, scale: c.scale, axes: c.axes, status: c.status, estimated_calls: c.estimated_calls,
      repeat: c.repeat ? { compared: c.repeat.compared, identical: c.repeat.identical } : null,
      runs: c.runs.map((r) => ({
        run: r.run, status: r.status, error: r.error, generation: r.generation ? { calls: r.generation.calls, refused_calls: r.generation.refused_calls } : null, storage: r.storage,
        readonly_unchanged: r.readonly ? r.readonly.project.unchanged && r.readonly.settings.unchanged : null,
        violations: r.resolution?.violations ?? null,
        line_repeats: r.line_repeats ? { repeated_lines: r.line_repeats.repeated_lines, extra_utf8_bytes: r.line_repeats.extra_utf8_bytes, across_tools_extra_utf8_bytes: r.line_repeats.across_tools_extra_utf8_bytes } : null,
        reads: r.reads.map((x) => ({ tool: x.tool, status: x.status, text_response: x.text_response, structured_response: x.structured_response, said_in_text: x.said_in_text, raw_text_utf8_bytes: x.raw_text_utf8_bytes, text_sha: x.text_sha, structured_sha: x.structured_sha })),
      })),
    })),
  };
}

export async function run(o) {
  if (o.command === 'plan') return { code: 0, output: JSON.stringify(o.plan, null, 2) };
  if (o.command === 'compare') return compare(o);

  const workRoot = o.workRoot ?? tmpdir();
  live.keep = o.keep;
  const binary = o.gil ?? await buildCore({ repo, features: o.features, unqueued: o.unqueued });
  mkdirSync(dirname(o.out), { recursive: true });
  mkdirSync(o.out); // fails if it appeared meanwhile: never write into an existing directory
  const recordingsDir = o.recordings ? join(o.out, 'recordings') : null;
  if (recordingsDir) mkdirSync(recordingsDir);
  const { file: localeFile, table } = localeTable(o.locale);
  const { markers, missing } = sectionMarkers(table);
  const result = newResult({
    tool: 'tools/chain-context-bench', tool_version: TOOL_VERSION, context_metrics_version: METRICS_VERSION,
    started_at: new Date().toISOString(), finished_at: null,
    environment: { platform: process.platform, arch: process.arch, node: process.version },
    source: sourceInfo(repo),
    core: { binary: binaryLabel(repo, binary), sha256: fileSha256(binary), provenance: o.build ? 'built by this run with cargo --features (build queue target)' : 'given with --gil; build features and commit are declared by the caller, not verified', features_declared: o.features, observed: null },
    conditions: { locale: o.locale, reads: o.plan.reads, timeout_ms: o.timeoutMs, measurement: 'generation in one Core process, reads in a second fresh Core process (new session); isolated HOME/LOCALAPPDATA/settings per run', section_markers: { locale_file: localeFile, missing_keys: missing }, recordings: !!recordingsDir },
    plan: { base: o.plan.base, axes: o.plan.axes, cases: o.plan.cases.map(({ id, scale, axes, estimated_calls }) => ({ id, scale, axes, estimated_calls })), repeat: o.plan.repeat, estimated_calls: o.plan.estimated_calls, max_calls: o.plan.max_calls },
  });
  let interrupted = false;
  const onSignal = (sig) => {
    if (interrupted) return;
    interrupted = true;
    cleanupSync();
    for (const c of result.cases) if (c.status === 'running') c.status = 'interrupted';
    result.finished_at = new Date().toISOString();
    result.interrupted_by = sig;
    try { finalize(result, o.plan, 'interrupted'); writeResults(o, result); } catch (err) { process.stderr.write(`[bench] could not write the interrupted result: ${err.message}\n`); }
    process.stderr.write(`[bench] interrupted by ${sig}; partial result in ${o.out}\n`);
    process.exit(130);
  };
  for (const sig of ['SIGINT', 'SIGTERM', 'SIGHUP']) process.on(sig, () => onSignal(sig));

  const typedVerify = o.features.split(',').includes('visual-authoring');
  await runBench(result, o.plan, {
    binary, locale: o.locale, timeoutMs: o.timeoutMs, workRoot, keep: o.keep, markers, typedVerify, recordingsDir,
    onProgress: (s) => { if (!o.quiet) process.stderr.write(`[bench] ${s}\n`); },
  });
  result.core.observed = result.cases.flatMap((c) => c.runs).find((r) => r.observed?.tools)?.observed ?? null;
  result.finished_at = new Date().toISOString();
  writeResults(o, result);
  return { code: result.status === 'complete' ? 0 : 1, output: `${result.status}: ${result.cases.length} cases, ${result.incomplete.length} incomplete runs\nresults: ${o.out}` };
}

/** Same-condition rerun comparison of two bench.json files. */
export function compare(o) {
  const load = (p) => { try { return JSON.parse(readFileSync(p, 'utf8')); } catch (err) { throw new UsageError(`cannot read ${p}: ${err.message}`); } };
  const a = load(o.before); const b = load(o.after);
  for (const [name, r] of [['--before', a], ['--after', b]]) if (r.schema !== 'gil-chain-context-bench/0.1') throw new UsageError(`${name} is not a chain-context-bench result`);
  const cond = (r) => ({ features: r.core?.features_declared, locale: r.conditions?.locale, reads: r.conditions?.reads, base: r.plan?.base });
  const differs = Object.keys(cond(a)).filter((k) => JSON.stringify(cond(a)[k]) !== JSON.stringify(cond(b)[k]));
  const key = (c) => JSON.stringify(c.scale);
  const bCases = new Map(b.cases.map((c) => [key(c), c]));
  const first = (c) => c?.runs?.find((r) => r.status === 'complete') ?? null;
  const rows = a.cases.map((ca) => {
    const cb = bCases.get(key(ca));
    const ra = first(ca); const rb = first(cb);
    const tools = [...new Set([...(ra?.reads ?? []), ...(rb?.reads ?? [])].map((x) => x.tool))];
    return {
      scale: ca.scale, before_case: ca.id, after_case: cb?.id ?? null, comparable: !!(ra && rb),
      reads: tools.map((tool) => {
        const x = ra?.reads.find((r) => r.tool === tool); const y = rb?.reads.find((r) => r.tool === tool);
        const tb = x?.text_response?.utf8_bytes ?? null; const ta = y?.text_response?.utf8_bytes ?? null;
        const sb = x?.structured_response?.utf8_bytes ?? null; const sa = y?.structured_response?.utf8_bytes ?? null;
        return { tool, text_utf8_bytes: { before: tb, after: ta, delta: tb !== null && ta !== null ? ta - tb : null }, structured_utf8_bytes: { before: sb, after: sa, delta: sb !== null && sa !== null ? sa - sb : null }, text_content_identical: x?.text_sha && y?.text_sha ? x.text_sha === y.text_sha : null };
      }),
    };
  });
  const out = { schema: 'gil-chain-context-bench-compare/0.1', same_binary: a.core?.sha256 === b.core?.sha256, conditions_differ: differs, caution: differs.length ? `조건이 다르다(${differs.join(', ')}): 같은 조건의 재실행 비교가 아니다` : null, cases: rows };
  const text = JSON.stringify(out, null, 2) + '\n';
  if (o.json) { mkdirSync(dirname(o.json), { recursive: true }); writeFileSync(o.json, text, { flag: 'wx' }); }
  return { code: 0, output: text };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  let o;
  try { o = parseArgs(process.argv.slice(2)); } catch (err) { process.stderr.write(`[bench] ${err.message}\n`); process.exit(2); }
  run(o).then(({ code, output }) => { if (!o.quiet || o.command !== 'run') process.stdout.write(output + '\n'); process.exitCode = code; })
    .catch((err) => { cleanupSync(); process.stderr.write(`[bench] ${err.message}\n`); process.exitCode = err instanceof UsageError || err.code === 'EEXIST' ? 2 : 1; });
}
