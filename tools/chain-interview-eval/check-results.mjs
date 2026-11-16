#!/usr/bin/env node
// Consistency check of a chain-interview evaluation result file against the case set.
//
//   node tools/chain-interview-eval/check-results.mjs --result <file.json> [--cases <dir>] [--json <out.json>] [--quiet]
//
// Reads the result file and the case directory only. Writes only to --json (new file only).
// Runs no model, Core, App or network. Exit 0 = consistent (not_run/unknown are consistent too),
// 1 = inconsistencies found, 2 = usage/input error. A clean exit never means the model passed.
import { writeFileSync, mkdirSync, existsSync } from 'node:fs';
import { resolve, dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { readJson } from './lib/io.mjs';
import { loadCaseSet } from './lib/cases.mjs';
import { checkResult } from './lib/results.mjs';
import { formatResultReport } from './lib/format.mjs';
import { DEFAULT_CASES } from './check-fixtures.mjs';

export function parseArgs(argv) {
  const o = { result: null, cases: DEFAULT_CASES, json: null, quiet: false };
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    const next = () => { if (i + 1 >= argv.length) throw new Error(`${a} needs a value`); return argv[++i]; };
    if (a === '--result') o.result = resolve(next());
    else if (a === '--cases') o.cases = resolve(next());
    else if (a === '--json') o.json = resolve(next());
    else if (a === '--quiet') o.quiet = true;
    else throw new Error(`unknown argument ${a}`);
  }
  if (!o.result) throw new Error('usage: check-results.mjs --result <file.json> [--cases <dir>] [--json <out>]');
  return o;
}

export function run(o) {
  if (!existsSync(join(o.cases, 'cases'))) throw new Error(`${o.cases} has no cases/ directory`);
  const set = loadCaseSet(o.cases);
  if (set.problems.length) throw new Error(`case set has ${set.problems.length} fixture problem(s); run check-fixtures.mjs first`);
  const r = readJson(o.result);
  if (r.error) throw new Error(`result ${o.result}: ${r.error}`);
  const checked = checkResult(r.value, set);
  const result = { schema: 'gil.chain-interview.result-report/1', result_file: o.result, ...checked, caveats: ['pass/fail 개수는 기록된 판정의 수이며 모델 품질의 집계가 아니다.', 'not_run·unknown은 일관성 문제가 아니다. 그 상태를 pass로 바꾸는 것이 문제다.'] };
  if (o.json) { mkdirSync(dirname(o.json), { recursive: true }); writeFileSync(o.json, JSON.stringify(result, null, 2) + '\n', { flag: 'wx' }); }
  return { result, summary: formatResultReport(result), code: checked.problems.length ? 1 : 0 };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const o = parseArgs(process.argv.slice(2));
    const { summary, code } = run(o);
    if (!o.quiet) process.stdout.write(summary + '\n');
    process.exitCode = code;
  } catch (err) {
    process.stderr.write(`[chain-interview-eval] ${err.message}\n`);
    process.exitCode = 2;
  }
}
