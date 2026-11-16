#!/usr/bin/env node
// Write a result-file skeleton for the whole case set: every case not_run, run.executed false.
// Optionally pull one case's recorded outputs out of a tools/scenarios events.jsonl recording
// (gil_plan_propose drafts, Define reports, chat is not in the log) so a judge can fill the rest.
//
//   node tools/chain-interview-eval/make-skeleton.mjs --out <result.json> [--cases <dir>] [--run-id <id>]
//        [--events <events.jsonl> --case <CI-NN>]
//
// Never marks anything evaluated: a case with extracted outputs is written as status "unknown"
// with reason "judgement pending" until a person or model judge records verdicts with basis.
import { writeFileSync, mkdirSync, existsSync, readFileSync } from 'node:fs';
import { resolve, dirname, join, basename } from 'node:path';
import { fileURLToPath } from 'node:url';
import { loadCaseSet } from './lib/cases.mjs';
import { sha256 } from './lib/io.mjs';
import { RESULT_SCHEMA } from './lib/vocab.mjs';
import { DEFAULT_CASES } from './check-fixtures.mjs';

export function parseArgs(argv) {
  const o = { out: null, cases: DEFAULT_CASES, runId: 'unset', events: null, caseId: null };
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    const next = () => { if (i + 1 >= argv.length) throw new Error(`${a} needs a value`); return argv[++i]; };
    if (a === '--out') o.out = resolve(next());
    else if (a === '--cases') o.cases = resolve(next());
    else if (a === '--run-id') o.runId = next();
    else if (a === '--events') o.events = resolve(next());
    else if (a === '--case') o.caseId = next();
    else throw new Error(`unknown argument ${a}`);
  }
  if (!o.out) throw new Error('usage: make-skeleton.mjs --out <result.json> [--events <events.jsonl> --case <CI-NN>]');
  if ((o.events && !o.caseId) || (!o.events && o.caseId)) throw new Error('--events and --case go together');
  return o;
}

/** Minimal Report-grammar reader: `key: value` lines and `key: |` block scalars. */
export function parseReport(text) {
  const out = {};
  if (typeof text !== 'string') return out;
  const lines = text.split('\n');
  for (let i = 0; i < lines.length; i += 1) {
    const m = /^([A-Za-z_][A-Za-z0-9_]*):\s*(.*)$/.exec(lines[i]);
    if (!m) continue;
    if (m[2] === '|') {
      const block = [];
      while (i + 1 < lines.length && (/^\s+/.test(lines[i + 1]) || lines[i + 1] === '')) { block.push(lines[i + 1].replace(/^\s{1,2}/, '')); i += 1; }
      while (block.length && block[block.length - 1] === '') block.pop();
      out[m[1]] = block.join('\n');
    } else out[m[1]] = m[2];
  }
  return out;
}

/** Pull proposals, Define reports and closed Experiments out of an events.jsonl text. */
export function extractOutputs(text) {
  const outputs = { questions: [], proposal: null, define: null, claimed_completed_items: [] };
  const summary = { calls: 0, unsupported_lines: 0, proposals: 0, closed_experiments: [] };
  let openKind = null; let boundItem = null; let inExperiment = false;
  for (const [n, line] of text.split('\n').entries()) {
    if (!line.trim()) continue;
    let ev; try { ev = JSON.parse(line); } catch { summary.unsupported_lines += 1; continue; }
    const name = ev?.request?.name; const args = ev?.request?.arguments || {};
    if (!name) { summary.unsupported_lines += 1; continue; }
    summary.calls += 1;
    const ok = ev.result && !ev.error && ev.result.isError !== true && ev.result.structuredContent?.ok !== false;
    if (!ok) continue;
    if (name === 'gil_plan_propose' && args.draft) { outputs.proposal = args.draft; summary.proposals += 1; }
    if (name === 'gil_plan_read') { const ci = ev.result.structuredContent?.current_item; if (ci) boundItem = typeof ci === 'string' ? ci : ci.id ?? null; }
    if (name === 'gil_open') {
      openKind = args.kind ?? null;
      if (openKind === 'experiment') inExperiment = true;
      if (openKind === 'question' && args.contract) outputs.questions.push({ opened_at_line: n + 1, contract: args.contract });
    }
    if (name === 'gil_close') {
      const rep = parseReport(args.report);
      if (openKind === 'define' && (rep.problem || rep.success_condition)) outputs.define = { bound_item: boundItem, problem: rep.problem ?? '', success_condition: rep.success_condition ?? '' };
      if (inExperiment && rep.verdict && (rep.verdict === 'success' || rep.verdict === 'failure') && rep.lesson) { summary.closed_experiments.push({ item: boundItem, verdict: rep.verdict }); inExperiment = false; }
      openKind = null;
    }
  }
  // Questions recorded as gil_open contracts are not the visible question text; keep them as pointers only.
  outputs.questions = outputs.questions.map((q) => ({ question: `(see events line ${q.opened_at_line}: visible text is not in the tool log)`, choices: [], multi_select: false, _pointer: q }));
  return { outputs, summary };
}

export function run(o) {
  if (!existsSync(join(o.cases, 'cases'))) throw new Error(`${o.cases} has no cases/ directory`);
  const set = loadCaseSet(o.cases);
  if (set.problems.length) throw new Error(`case set has ${set.problems.length} fixture problem(s); run check-fixtures.mjs first`);
  let extracted = null;
  if (o.events) {
    if (!set.cases.some((c) => c.value?.id === o.caseId)) throw new Error(`--case ${o.caseId} is not in the case set`);
    const text = readFileSync(o.events, 'utf8');
    extracted = { ...extractOutputs(text), ref: o.events, sha256: sha256(text) };
  }
  const result = {
    schema: RESULT_SCHEMA,
    run: {
      id: o.runId, recorded_at: null, executed: false, example: false,
      model: null, guidance: { workflow_md_sha256: null, repo_commit: null, profile: null },
      case_set: { name: basename(o.cases), sha256: set.digest }, harness: null,
      notes: 'skeleton: fill run.* and each case before judging; executed stays false until a model actually ran',
    },
    judges: [],
    cases: set.cases.map(({ value: c }) => {
      if (extracted && c.id === o.caseId) {
        return {
          case_id: c.id, status: 'unknown', reason: 'judgement pending: outputs were extracted from the tool log but no judge has recorded verdicts; visible chat text is not in the log',
          observation: { visible_messages: [], tool_log_ref: basename(extracted.ref), tool_log_sha256: extracted.sha256, transcript_ref: null, tool_log_summary: extracted.summary },
          outputs: extracted.outputs, approval: { kind: 'unknown', evidence: '' },
          automatic: [], semantic: (c.judgement?.semantic || []).map((s) => ({ criterion: s, status: 'unknown', judge: null, basis: [], rationale: 'judgement pending' })),
        };
      }
      return { case_id: c.id, status: 'not_run', reason: 'not executed', observation: null, outputs: null, approval: { kind: 'none', evidence: '' }, automatic: [], semantic: (c.judgement?.semantic || []).map((s) => ({ criterion: s, status: 'not_run' })) };
    }),
  };
  mkdirSync(dirname(o.out), { recursive: true });
  writeFileSync(o.out, JSON.stringify(result, null, 2) + '\n', { flag: 'wx' });
  return result;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const o = parseArgs(process.argv.slice(2));
    const r = run(o);
    process.stdout.write(`skeleton: ${r.cases.length} case(s) → ${o.out} (executed=false; every case not_run${o.events ? ` except ${o.caseId} = unknown` : ''})\n`);
  } catch (err) {
    process.stderr.write(`[chain-interview-eval] ${err.message}\n`);
    process.exitCode = 2;
  }
}
