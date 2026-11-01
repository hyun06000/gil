#!/usr/bin/env node
// Score one scenario run: GIL procedure compliance and answer quality are reported separately.
//   node tools/scenarios/scorer.mjs --scenario <dir> --events <jsonl> [--root <project>] [--final <file>]
//        [--human <json>] [--out <dir>] [--run-commands]
// Statuses: pass | fail | unknown | expected_blocked | unsupported | not_run. Human checkpoint
// fields are read from --human only; they are never inferred from protocol success.
import { readFileSync, writeFileSync, mkdirSync, existsSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { parseEvents, replay } from './lib/trace.mjs';
import { scoreProcedure, scoreAnswer, summarize } from './lib/score.mjs';
import { runCommand } from './lib/rpc.mjs';
import { table } from './lib/markdown.mjs';

function parseArgs(argv) {
  const o = { scenario: null, events: null, root: null, final: null, human: null, out: null, runCommands: false };
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === '--scenario') o.scenario = resolve(argv[++i]);
    else if (a === '--events') o.events = resolve(argv[++i]);
    else if (a === '--root') o.root = resolve(argv[++i]);
    else if (a === '--final') o.final = resolve(argv[++i]);
    else if (a === '--human') o.human = resolve(argv[++i]);
    else if (a === '--out') o.out = resolve(argv[++i]);
    else if (a === '--run-commands') o.runCommands = true;
    else throw new Error(`unknown argument ${a}`);
  }
  if (!o.scenario || !o.events) throw new Error('--scenario and --events are required');
  return o;
}

export async function score(o) {
  const rubric = JSON.parse(readFileSync(join(o.scenario, 'rubric.json'), 'utf8'));
  const events = parseEvents(readFileSync(o.events, 'utf8'));
  const human = o.human && existsSync(o.human) ? JSON.parse(readFileSync(o.human, 'utf8')) : null;
  const finalText = o.final && existsSync(o.final) ? readFileSync(o.final, 'utf8') : '';
  const facts = replay(events, { externalTools: rubric.external_tools || [] });
  const procedure = scoreProcedure(facts, { human, root: o.root, rubric });
  const answer = await scoreAnswer(rubric, { root: o.root, finalText, human, runCommand: o.runCommands ? runCommand : null });
  const result = {
    scenario: rubric.id, title: rubric.title, generated_at: new Date().toISOString(), events: events.length,
    human_record: human ? 'provided' : 'absent', project_root_checked: !!o.root, commands_executed: o.runCommands,
    procedure: { judgements: procedure, summary: summarize(procedure) },
    answer: { judgements: answer, summary: summarize(answer) },
    // Two different claims, never merged: Core refusals observed on the wire (the product blocked
    // something) and the procedure/answer judgements about the model's work. A green Core gate
    // says nothing about the model; a model verdict says nothing about Core enforcement.
    claims: {
      core_contract: { refusals_observed: facts.rejected.length, restore_revisit: facts.restoreRevisit.map((r) => ({ seq: r.seq, name: r.name, ok: r.ok })), note: 'Core-side refusals seen in this log; not a model-work judgement' },
      model_task: { procedure_verdict: summarize(procedure).verdict, answer_verdict: summarize(answer).verdict, note: 'judgements about the model\'s actions and deliverable; synthetic App input and protocol success never count as human consent or display' },
    },
    trace: { experiments: facts.experimentOpens.length, verify_closes: facts.verifyCloses.length, external_calls: facts.externalCalls.length, decides: facts.decides.map((d) => ({ seq: d.seq, actor: d.actor, synthetic: d.synthetic, decision: d.decision, ok: d.ok })), rejected: facts.rejected.length, unknown_tools: facts.unknownTools },
  };
  const md = [`# ${rubric.title || rubric.id} — score`, '',
    `- events: ${events.length} · human record: ${result.human_record} · project root checked: ${result.project_root_checked} · commands executed: ${o.runCommands}`,
    `- GIL procedure: **${result.procedure.summary.verdict}** ${JSON.stringify(result.procedure.summary.counts)}`,
    `- answer quality: **${result.answer.summary.verdict}** ${JSON.stringify(result.answer.summary.counts)}`,
    `- Core contract (separate claim): ${facts.rejected.length} refusal(s) observed on the wire; restore/revisit: ${facts.restoreRevisit.length ? facts.restoreRevisit.map((r) => `${r.name}=${r.ok ? 'succeeded' : 'refused'}`).join(', ') : 'not called'}`,
    '', '## GIL procedure', '', table(['check', 'status', 'detail'], procedure.map((x) => [x.id, x.status, x.detail])),
    '', '## Answer quality', '', answer.length ? table(['check', 'status', 'detail'], answer.map((x) => [x.id, x.status, x.detail])) : '_rubric has no answer checks_',
    '', '## Human checkpoints', '', table(['id', 'what', 'field', 'value'], (rubric.human_checkpoints || []).map((h) => [h.id, h.what, h.field, human && h.field in human ? String(human[h.field]) : 'unfilled'])), ''].join('\n');
  if (o.out) { mkdirSync(o.out, { recursive: true }); writeFileSync(join(o.out, 'score.json'), JSON.stringify(result, null, 2)); writeFileSync(join(o.out, 'score.md'), md); }
  return { result, md };
}

if (process.argv[1] && resolve(process.argv[1]) === new URL(import.meta.url).pathname) {
  score(parseArgs(process.argv.slice(2))).then(({ result, md }) => {
    process.stdout.write(md + '\n');
    process.exitCode = result.procedure.summary.verdict === 'fail' || result.answer.summary.verdict === 'fail' ? 1 : 0;
  }).catch((err) => { process.stderr.write(`[scorer] ${err.message}\n`); process.exitCode = 2; });
}
