#!/usr/bin/env node
import { writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { table } from '../scenarios/lib/markdown.mjs';
import { DEFAULT_CASES, loadCases, json, skeleton, checkResult } from './lib/package.mjs';

export function main(argv) {
  const [command, ...args] = argv;
  if (!['fixtures', 'result', 'skeleton'].includes(command)) throw new Error('usage: check.mjs fixtures|result|skeleton [--cases dir] [--result file] [--out new-file]');
  const opts = { cases: DEFAULT_CASES };
  for (let i = 0; i < args.length; i += 2) {
    const key = args[i]?.slice(2);
    if (!['cases', 'result', 'out'].includes(key) || !args[i].startsWith('--') || !args[i + 1] || args[i + 1].startsWith('--') || Object.hasOwn(opts, key) && key !== 'cases') throw new Error('invalid arguments');
    opts[key] = args[i + 1];
  }
  if (command === 'result' && !opts.result || command === 'skeleton' && !opts.out) throw new Error('result needs --result; skeleton needs --out');
  const set = loadCases(opts.cases);
  if (set.problems.length && command !== 'fixtures') throw new Error('invalid case set; run fixtures first');
  let report;
  if (command === 'fixtures') report = { case_count: set.cases.length, case_set_sha256: set.digest, problems: set.problems };
  if (command === 'result') report = checkResult(json(opts.result), set);
  if (command === 'skeleton') report = skeleton(set);
  if (opts.out) writeFileSync(opts.out, JSON.stringify(report, null, 2) + '\n', { flag: 'wx' });
  if (command === 'skeleton') console.log('Created result skeleton: 10 not_run; model/Host not_run.');
  else {
    console.log(`${command}: ${report.problems.length} consistency problem(s). Model/Host UX is not scored by this exit code.`);
    if (report.case_set_sha256) console.log(`case_set_sha256: ${report.case_set_sha256}`);
    if (report.model_task) console.log(`model_task: ${report.model_task}; host_task: ${report.host_task}`);
    if (report.semantic_counts) console.log(table(['pass', 'fail', 'unknown', 'not_run'], [Object.values(report.semantic_counts)]));
    for (const p of report.problems) console.log(`[${p.where}] ${p.detail}`);
  }
  return report.problems?.length ? 1 : 0;
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { process.exitCode = main(process.argv.slice(2)); }
  catch { console.error('Input/usage error; check arguments, JSON and fixture set. No input content was executed.'); process.exitCode = 2; }
}
