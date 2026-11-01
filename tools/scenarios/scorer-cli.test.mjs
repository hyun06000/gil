import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, readFileSync, mkdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { runCommand } from './lib/rpc.mjs';

const here = dirname(fileURLToPath(import.meta.url));

test('scorer CLI writes score.json/score.md with separated procedure and answer sections and unfilled human checkpoints', async () => {
  const work = mkdtempSync(join(tmpdir(), 'gil-scorer-'));
  const scenario = join(work, 'scenario'); mkdirSync(scenario);
  writeFileSync(join(scenario, 'rubric.json'), JSON.stringify({ id: 'demo', title: 'Demo', external_tools: ['save_project_file'], deliverable: { pattern: '^evidence/' }, human_checkpoints: [{ id: 'HC1', what: 'Monitor visible?', field: 'monitor_visible' }], answer_checks: [{ id: 'report', kind: 'file_exists', path: 'evidence/report.md' }] }));
  const root = join(work, 'project'); mkdirSync(join(root, 'evidence'), { recursive: true });
  writeFileSync(join(root, 'evidence/report.md'), 'ok');
  const events = readFileSync(join(here, 'fixtures', 'minimal-events.jsonl'), 'utf8');
  writeFileSync(join(work, 'events.jsonl'), events);
  const out = join(work, 'out');
  const r = await runCommand(process.execPath, [join(here, 'scorer.mjs'), '--scenario', scenario, '--events', join(work, 'events.jsonl'), '--root', root, '--out', out], { timeoutMs: 20000 });
  assert.equal(r.code, 0, r.stderr);
  const score = JSON.parse(readFileSync(join(out, 'score.json'), 'utf8'));
  assert.equal(score.scenario, 'demo');
  assert.equal(score.human_record, 'absent');
  const proc = Object.fromEntries(score.procedure.judgements.map((x) => [x.id, x.status]));
  assert.equal(proc.monitor_display_confirmed, 'unknown');
  assert.equal(proc.verify_file_evidence, 'pass');
  assert.equal(score.answer.judgements[0].status, 'pass');
  const md = readFileSync(join(out, 'score.md'), 'utf8');
  assert.match(md, /## GIL procedure/); assert.match(md, /## Answer quality/); assert.match(md, /HC1 \| Monitor visible\? \| monitor_visible \| unfilled/);
});
