#!/usr/bin/env node
// Development check: do the Chain / approved-plan examples in the shared workflow and the Manual
// still run against the current Core/MCP contract?
//
//   node tools/manual-example-check/check.mjs sources [--json <new file>]
//   python3 coordination/parallel-wave-2/cargo-queue.py -- node tools/manual-example-check/check.mjs run --build [--out <dir>]
//   node tools/manual-example-check/check.mjs run --gil <path to a gil built with approved-plan,visual-authoring,product-safety> [--out <dir>]
//
// `sources` reads documents only. `run` additionally drives one fresh temporary project per
// reviewed case over the real `gil mcp --serve` stdio protocol. It never executes a document's
// code block or shell command, never runs `gil start` in the repository, never reads PATH for gil,
// and never edits a GIL record. Every App decision is synthetic harness input, not human approval.
import { mkdtemp, mkdir, readdir, writeFile, readFile, rm } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { runCommand } from '../scenarios/lib/rpc.mjs';
import { makeRedactor, leaks } from '../scenarios/lib/redact.mjs';
import { loadRegistry, evaluateSources, sourcesClean, RegistryError } from './lib/registry.mjs';
import { Rig, inside, CheckFailed } from './lib/rig.mjs';
import { CASES } from './lib/cases.mjs';
import { exampleStatuses, verdict, markdown } from './lib/report.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, '..', '..');
export const FEATURES = 'approved-plan,visual-authoring,product-safety';

class Usage extends Error {}

export function parseArgs(argv) {
  const o = { mode: argv[0], build: false, gil: null, out: null, json: null, keep: false, unqueued: false, only: null, timeoutMs: 20000, docsRoot: null, mapping: null };
  if (!['sources', 'run'].includes(o.mode)) throw new Usage('first argument must be `sources` or `run`');
  for (let i = 1; i < argv.length; i += 1) {
    const a = argv[i];
    const value = () => { if (i + 1 >= argv.length) throw new Usage(`${a} needs a value`); return argv[++i]; };
    if (a === '--build') o.build = true;
    else if (a === '--unqueued') o.unqueued = true;
    else if (a === '--keep') o.keep = true;
    else if (a === '--gil') o.gil = resolve(value());
    else if (a === '--out') o.out = resolve(value());
    else if (a === '--json') o.json = resolve(value());
    else if (a === '--case') o.only = value().split(',');
    else if (a === '--docs-root') o.docsRoot = resolve(value());
    else if (a === '--mapping') o.mapping = resolve(value());
    else if (a === '--timeout-ms') o.timeoutMs = Number(value());
    else throw new Usage(`unknown argument ${a}`);
  }
  if (o.mode === 'run' && o.build === !!o.gil) throw new Usage('run needs exactly one of --build or --gil <path>');
  if (o.mode === 'sources' && (o.build || o.gil || o.out || o.only)) throw new Usage('sources takes only --json, --docs-root and --mapping');
  return o;
}

/** The output folder must be new or empty and outside the source repository. */
export async function prepareOut(out, repoRoot) {
  if (inside(out, repoRoot)) throw new Usage(`--out must be outside the source repository (${out})`);
  if (existsSync(out) && (await readdir(out)).length) throw new Usage(`--out ${out} exists and is not empty; choose a new folder`);
  await mkdir(out, { recursive: true });
  return out;
}

async function buildCore(unqueued) {
  if (!process.env.GIL_PARALLEL_TARGET_DIR && !unqueued) throw new Usage('refusing to build outside cargo-queue.py (GIL_PARALLEL_TARGET_DIR is unset): python3 coordination/parallel-wave-2/cargo-queue.py -- node tools/manual-example-check/check.mjs run --build');
  const args = ['build', '--locked', '-p', 'gil', '--features', FEATURES, '--message-format=json-render-diagnostics'];
  process.stderr.write(`[example-check] cargo ${args.join(' ')}\n`);
  const r = await runCommand('cargo', args, { cwd: repo, timeoutMs: 1800000, onStderr: (s) => process.stderr.write(s) });
  if (r.timedOut || r.code !== 0) throw new Error(`cargo build failed (exit ${r.code})`);
  let exe = null;
  for (const line of r.stdout.split('\n')) {
    if (!line.startsWith('{')) continue;
    try { const m = JSON.parse(line); if (m.reason === 'compiler-artifact' && m.target?.name === 'gil' && m.target.kind?.includes('bin') && m.executable) exe = m.executable; } catch {}
  }
  if (!exe) throw new Error('cargo did not report the gil executable');
  return exe;
}

const git = async (args) => (await runCommand('git', args, { cwd: repo, timeoutMs: 20000 })).stdout.trim();

function printSources(result) {
  for (const e of result.examples) process.stdout.write(`${e.source_status === 'ok' ? 'ok     ' : 'PROBLEM'} ${e.id.padEnd(24)} ${e.kind.padEnd(20)} ${e.path}${e.lines ? `:${e.lines[0]}-${e.lines[1]}` : ''}${e.source_status === 'ok' ? '' : `\n        ${e.source_status}: ${e.detail}`}\n`);
  for (const u of result.unmapped) process.stdout.write(`PROBLEM unmapped ${u.detail}\n`);
  for (const c of result.consistency) process.stdout.write(`${c.status === 'consistent' ? 'ok     ' : c.status === 'known_mismatch' ? 'known  ' : 'PROBLEM'} consistency ${c.id}: ${c.status}${c.detail ? ` — ${c.detail}` : ''}\n`);
}

async function runCases({ binary, registry, sources, inputs, opts, workRoot, events }) {
  const ex = Object.fromEntries(sources.examples.map((e) => [e.id, e]));
  const served = [];
  for (const src of registry.sources.filter((s) => s.served)) {
    for (const e of registry.examples.filter((x) => x.source.path === src.path && x.served)) served.push({ id: e.id, via: src.served, excerpt: ex[e.id].excerpt });
  }
  const ctx = { ex, inputs, served, observations: {} };
  const results = [];
  for (const c of CASES) {
    if (opts.only && !opts.only.includes(c.id)) { results.push({ id: c.id, title: c.title, status: 'not_run', detail: 'not selected', checks: [] }); continue; }
    const rig = new Rig({ binary, workRoot, label: c.id, repo, timeoutMs: opts.timeoutMs, onEvent: (e) => events.push(e) });
    const row = { id: c.id, title: c.title, observation: !!c.observation, examples: c.examples === 'served' ? served.map((s) => s.id) : c.examples, checks: rig.checks };
    try {
      await rig.open();
      await c.run(rig, ctx);
      row.status = 'pass';
    } catch (err) {
      row.status = 'fail';
      row.failed_check = err instanceof CheckFailed ? err.check : 'harness';
      row.detail = err.message;
      rig.checks.push({ check: row.failed_check, status: 'fail', detail: err.message });
    } finally {
      const exit = await rig.close().catch((err) => ({ error: err.message }));
      if (exit && (exit.code !== 0 || exit.stdoutNoise?.some((l) => l.trim()))) {
        row.status = 'fail';
        row.detail = `${row.detail ? `${row.detail}; ` : ''}Core exit ${exit.code ?? exit.error}${exit.stdoutNoise?.length ? `, ${exit.stdoutNoise.length} non-JSON stdout lines` : ''}`;
      }
    }
    results.push(row);
  }
  return { results, observations: ctx.observations };
}

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  const registry = loadRegistry(opts.mapping ?? join(here, 'examples.json'));
  // --docs-root reads the same relative document paths from another checkout (e.g. a worktree
  // where the guidance is being edited). The Core under test must be built from that text too,
  // otherwise CX-00 reports the served text as different.
  const docsRoot = opts.docsRoot ?? repo;
  const sources = evaluateSources(registry, { repo: docsRoot });
  if (opts.mode === 'sources') {
    printSources(sources);
    if (opts.json) await writeFile(opts.json, `${JSON.stringify({ examples: sources.examples.map(({ excerpt, payload, ...r }) => r), unmapped: sources.unmapped, consistency: sources.consistency }, null, 2)}\n`, { flag: 'wx' });
    const clean = sourcesClean(sources);
    process.stdout.write(clean ? 'sources: mapping current\n' : 'sources: mapping needs review (nothing was executed)\n');
    return clean ? 0 : 1;
  }

  const head = await git(['rev-parse', 'HEAD']);
  const watched = registry.sources.map((s) => s.path);
  const docsDirty = docsRoot === repo ? (await git(['status', '--porcelain', '--', ...watched])) !== '' : null;
  const dirtyBefore = await git(['status', '--porcelain', '--', ...watched, 'tools/manual-example-check']);
  const repoGilBefore = existsSync(join(repo, '.gil'));
  const out = await prepareOut(opts.out ?? await mkdtemp(join(tmpdir(), 'gil-manual-example-out-')), repo);
  const workRoot = await mkdtemp(join(tmpdir(), 'gil-manual-example-work-'));
  if (inside(workRoot, repo)) throw new Usage(`the temporary root ${workRoot} is inside the repository; set TMPDIR elsewhere`);
  const binary = opts.gil ?? await buildCore(opts.unqueued);
  if (!existsSync(binary)) throw new Usage(`binary not found: ${binary}`);
  const binarySha = createHash('sha256').update(await readFile(binary)).digest('hex');
  const inputs = JSON.parse(await readFile(join(here, 'fixtures', 'synthetic-inputs.json'), 'utf8'));

  const events = [];
  let cases = [];
  let observations = {};
  if (sourcesClean(sources)) ({ results: cases, observations } = await runCases({ binary, registry, sources, inputs, opts, workRoot, events }));
  else cases = CASES.map((c) => ({ id: c.id, title: c.title, status: 'not_run', detail: 'source mapping needs review first; no case was executed against a changed document', checks: [] }));

  const isolation = [
    { check: 'no .gil created in the source repository', ok: repoGilBefore || !existsSync(join(repo, '.gil')), detail: repoGilBefore ? 'a .gil already existed before the run' : '' },
    { check: 'watched sources unchanged by the run', ok: (await git(['status', '--porcelain', '--', ...watched, 'tools/manual-example-check'])) === dirtyBefore },
    { check: 'temporary projects and output outside the repository', ok: !inside(workRoot, repo) && !inside(out, repo) },
  ];
  const examples = exampleStatuses(sources, cases);
  const report = {
    mode: 'run', head, docs_root: docsRoot === repo ? 'this repository' : 'given by --docs-root', dirty_sources: docsDirty, binary: { sha256: binarySha, features: opts.gil ? `given (${FEATURES} expected)` : FEATURES },
    claims: {
      core_contract: cases.every((c) => c.status === 'pass') ? 'pass (structural, synthetic inputs)' : 'fail or not_run',
      served_text_matches_documents: cases.find((c) => c.id === 'CX-00')?.status ?? 'not_run',
      human_approval: 'not_observed — every gil_plan_decide was synthetic harness input; Core records it as a monitor_button receipt',
      monitor_display: 'not_observed — protocol success is not a visible screen',
      model_behavior: 'not_run — no model was called',
      natural_language_quality: 'not_evaluated — question depth, proposal clarity and item size are outside this check',
    },
    observations, examples, unmapped: sources.unmapped, consistency: sources.consistency, cases, isolation,
  };
  report.verdict = verdict(report);

  const redactor = makeRedactor({ roots: { work_root: workRoot, out, repo, docs_root: docsRoot } });
  const savedEvents = events.map((e) => redactor.redact(e));
  const eventsText = savedEvents.map((e) => JSON.stringify(e)).join('\n') + '\n';
  const reportText = JSON.stringify(redactor.redact(report), null, 2) + '\n';
  const leaked = leaks(eventsText + reportText, { roots: [workRoot] });
  if (leaked.length) report.verdict.problems.push('redaction gap: an absolute temporary path survived redaction');
  await writeFile(join(out, 'report.json'), reportText, { flag: 'wx' });
  await writeFile(join(out, 'report.md'), markdown(redactor.redact(report)), { flag: 'wx' });
  if (!leaked.length) await writeFile(join(out, 'events.jsonl'), eventsText, { flag: 'wx' });

  for (const e of examples) process.stdout.write(`${e.status.padEnd(15)} ${e.id}\n`);
  for (const c of cases) process.stdout.write(`${c.status.padEnd(15)} ${c.id} ${c.title}${c.status === 'fail' ? `\n                ${c.detail}` : ''}\n`);
  process.stdout.write(`report: ${join(out, 'report.md')}\n`);
  if (report.verdict.ok && !opts.keep) await rm(workRoot, { recursive: true, force: true });
  else process.stderr.write(`[example-check] temporary projects kept at ${workRoot}\n`);
  return report.verdict.ok ? 0 : 1;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().then((code) => process.exit(code), (err) => {
    if (err instanceof Usage || err instanceof RegistryError) { process.stderr.write(`manual-example-check: ${err.message}\n`); process.exit(2); }
    process.stderr.write(`manual-example-check: ${err.stack || err}\n`);
    process.exit(2);
  });
}
