#!/usr/bin/env node
// Offline integrity check of the chain-interview case set.
//
//   node tools/chain-interview-eval/check-fixtures.mjs [--cases <dir>] [--json <out.json>] [--quiet]
//
// Reads only the case directory (default scenarios/chain-interview) and, for spec_refs, checks
// that the referenced file exists in the repository. Writes only to --json (new file only).
// Runs no model, Core, App or network. Exit 0 = no problems, 1 = problems found, 2 = usage/input error.
// It checks structure, identifiers, references and declared violations. It does not judge wording.
import { writeFileSync, mkdirSync, existsSync } from 'node:fs';
import { resolve, dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { loadCaseSet } from './lib/cases.mjs';
import { formatFixtureReport } from './lib/format.mjs';

const here = dirname(fileURLToPath(import.meta.url));
export const DEFAULT_CASES = resolve(here, '..', '..', 'scenarios', 'chain-interview');

export function parseArgs(argv) {
  const o = { cases: DEFAULT_CASES, json: null, quiet: false };
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    const next = () => { if (i + 1 >= argv.length) throw new Error(`${a} needs a value`); return argv[++i]; };
    if (a === '--cases') o.cases = resolve(next());
    else if (a === '--json') o.json = resolve(next());
    else if (a === '--quiet') o.quiet = true;
    else throw new Error(`unknown argument ${a}`);
  }
  return o;
}

export function run(o) {
  if (!existsSync(join(o.cases, 'cases'))) throw new Error(`${o.cases} has no cases/ directory`);
  const set = loadCaseSet(o.cases);
  const result = {
    schema: 'gil.chain-interview.fixture-report/1',
    case_dir: o.cases,
    case_count: set.cases.length,
    comparison_count: set.comparisons.size,
    digest: set.digest,
    problems: set.problems,
    cases: set.cases.map(({ value: c }) => ({
      id: c?.id ?? null, category: c?.category ?? null, stages: Array.isArray(c?.stages_under_test) ? c.stages_under_test : [],
      good: Array.isArray(c?.good_examples) ? c.good_examples.length : 0, bad: Array.isArray(c?.bad_examples) ? c.bad_examples.length : 0,
      semantic: Array.isArray(c?.judgement?.semantic) ? c.judgement.semantic : [],
    })),
    caveats: ['이 검사는 구조·식별자·참조·선언된 위반만 본다. 질문의 필요성이나 제안의 명료함 같은 의미 품질은 판정하지 않는다.'],
  };
  if (o.json) { mkdirSync(dirname(o.json), { recursive: true }); writeFileSync(o.json, JSON.stringify(result, null, 2) + '\n', { flag: 'wx' }); }
  return { result, summary: formatFixtureReport(result), code: set.problems.length ? 1 : 0 };
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
