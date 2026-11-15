#!/usr/bin/env node
// Offline context-transfer metrics for saved tool-call recordings (tools/scenarios events.jsonl).
//
//   node tools/context-metrics/cli.mjs analyze --events <file> [--json <out>] [--min-block-chars N] [--quiet]
//   node tools/context-metrics/cli.mjs compare --before <file> --after <file> [--json <out>] [--min-block-chars N] [--quiet]
//
// Reads only the files named on the command line. Writes only to --json. Runs no model, Core,
// App or network. Exit 0 on success, 2 on usage/input errors. There is no pass/fail threshold.
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { resolve, basename, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseRecording } from './lib/events.mjs';
import { analyzeRecording, DEFAULT_MIN_BLOCK_CODE_POINTS } from './lib/analyze.mjs';
import { compareAnalyses } from './lib/compare.mjs';
import { formatAnalysis, formatComparison } from './lib/format.mjs';
import { sha256 } from './lib/size.mjs';

export function parseArgs(argv) {
  const o = { command: argv[0], events: null, before: null, after: null, json: null, minBlockChars: DEFAULT_MIN_BLOCK_CODE_POINTS, quiet: false };
  if (!['analyze', 'compare'].includes(o.command)) throw new Error('usage: cli.mjs analyze --events <file> | compare --before <file> --after <file>');
  for (let i = 1; i < argv.length; i += 1) {
    const a = argv[i];
    const next = () => { if (i + 1 >= argv.length) throw new Error(`${a} needs a value`); return argv[++i]; };
    if (a === '--events') o.events = resolve(next());
    else if (a === '--before') o.before = resolve(next());
    else if (a === '--after') o.after = resolve(next());
    else if (a === '--json') o.json = resolve(next());
    else if (a === '--min-block-chars') { o.minBlockChars = Number(next()); if (!Number.isInteger(o.minBlockChars) || o.minBlockChars < 1) throw new Error('--min-block-chars takes a positive integer (code points)'); }
    else if (a === '--quiet') o.quiet = true;
    else throw new Error(`unknown argument ${a}`);
  }
  if (o.command === 'analyze' && !o.events) throw new Error('analyze needs --events <file>');
  if (o.command === 'compare' && (!o.before || !o.after)) throw new Error('compare needs --before <file> and --after <file>');
  return o;
}

export function analyzeFile(path, { minBlockCodePoints } = {}) {
  const text = readFileSync(path, 'utf8');
  return analyzeRecording(parseRecording(text), { source: { file_name: basename(path), sha256: sha256(text) }, minBlockCodePoints });
}

export function run(o) {
  const opts = { minBlockCodePoints: o.minBlockChars };
  let result; let summary;
  if (o.command === 'analyze') { result = analyzeFile(o.events, opts); summary = formatAnalysis(result); }
  else { result = compareAnalyses(analyzeFile(o.before, opts), analyzeFile(o.after, opts)); summary = formatComparison(result); }
  if (o.json) { mkdirSync(dirname(o.json), { recursive: true }); writeFileSync(o.json, JSON.stringify(result, null, 2) + '\n'); }
  return { result, summary };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const o = parseArgs(process.argv.slice(2));
    const { summary } = run(o);
    if (!o.quiet) process.stdout.write(summary + '\n');
    if (o.json && !o.quiet) process.stdout.write(`\nJSON: ${o.json}\n`);
  } catch (err) {
    process.stderr.write(`[context-metrics] ${err.message}\n`);
    process.exitCode = 2;
  }
}
