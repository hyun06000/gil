// Synthetic evidence only. Never launches a model, Core, app or embedded instruction.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync, mkdtempSync, rmSync, symlinkSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { sha256 } from './lib/io.mjs';
import { loadCaseSet } from './lib/cases.mjs';
import { checkResult } from './lib/results.mjs';
import { prepareEvidence, validatePreparedEvidence } from './lib/evidence.mjs';
import { checkQuestion, checkCompletionClaims } from './lib/structural.mjs';
import { run } from './import-evidence.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const cases = join(here, '../../scenarios/chain-interview');
const examples = join(cases, 'import/examples');
const set = loadCaseSet(cases);
const basePath = join(cases, 'results/examples/not-run.json');
const base = () => JSON.parse(readFileSync(basePath, 'utf8'));
const json = (name) => JSON.parse(readFileSync(join(examples, name), 'utf8'));
const source = (pin, text) => ({ id: pin.id, ref: pin.ref, sha256: sha256(text), text });
function inputs(change = () => {}) {
  const m = json('manifest.json'); const c = json('conversation.json');
  const e = readFileSync(join(examples, 'events.jsonl'), 'utf8').trim().split('\n').map(JSON.parse);
  const values = { m, c, e }; change(values);
  const ct = JSON.stringify(c) + '\n'; const et = e.map((x) => JSON.stringify(x)).join('\n') + '\n';
  m.events.sha256 = sha256(et);
  if (m.conversation) m.conversation.sha256 = sha256(ct);
  return { manifest: source(m, JSON.stringify(m)), events: source(m.events, et), conversation: m.conversation ? source(m.conversation, ct) : null };
}
const prepare = (sources) => prepareEvidence(sources, base().run.id, 'CI-01');
const attach = (evidence) => { const r = base(); r.cases[0].prepared_evidence = evidence; return r; };
const cli = (args) => spawnSync(process.execPath, [join(here, 'import-evidence.mjs'), ...args], { encoding: 'utf8' });
function withTemp(fn) {
  const dir = mkdtempSync(join(tmpdir(), 'gil-interview-import-'));
  try { return fn(dir); } finally { rmSync(dir, { recursive: true, force: true }); }
}

test('import: exact public questions, choices, answers and explicit event quote links survive', () => {
  const evidence = prepare(inputs()); const p = evidence.projection;
  assert.equal(p.visible_messages.length, 3);
  assert.equal(p.outputs.questions[0].response, '날짜 열부터 확인해 주세요.');
  assert.equal(p.outputs.questions[0].question, '어느 열을 먼저 확인할까요?');
  assert.equal(checkQuestion(p.outputs.questions[0])[0].status, 'pass');
  assert.deepEqual(p.outputs.claimed_completed_items, ['A']);
  assert.equal(p.outputs.define.bound_item, 'A');
  assert.equal(checkCompletionClaims(['A'], p.tool_log_summary.closed_experiments)[0].status, 'pass');
  assert.equal(p.event_index.length, 11);
  assert.equal(p.visible_messages[0].sha256, sha256(p.visible_messages[0].text));
  assert.equal(p.event_index[0].sha256, sha256(evidence.sources.events.text.split('\n')[0]));
  assert.equal(p.tool_question_pointers.length, 1);
  assert.equal(checkResult(attach(evidence), set).problems.length, 0);
});

test('import: missing conversation and annotations preserve tool evidence without invented chat', () => {
  const p = prepare(inputs(({ m }) => { m.conversation = null; m.questions = []; m.claims = []; m.links = []; })).projection;
  assert.deepEqual(p.visible_messages, []); assert.deepEqual(p.outputs.questions, []);
  assert.deepEqual(p.outputs.claimed_completed_items, []);
  assert.ok(p.outputs.proposal); assert.ok(p.outputs.define);
  assert.ok(p.missing.includes('visible_conversation'));
  assert.equal(p.annotation_coverage, 'unspecified');
});

test('import: missing answer and multi-select remain missing, never inferred from wording', () => {
  const p = prepare(inputs(({ m }) => { m.questions[0].response = null; m.questions[0].multi_select = null; })).projection;
  assert.equal(p.outputs.questions[0].response, null); assert.equal(p.outputs.questions[0].multi_select, null);
  assert.ok(p.missing.includes('question_response')); assert.ok(p.missing.includes('multi_select_annotation'));
});

for (const [name, mutate] of [
  ['message quote', ({ m }) => { m.questions[0].cite.quote = 'invented'; }],
  ['choice quote', ({ m }) => { m.questions[0].choices[0].cite.end += 1; }],
  ['response role', ({ m }) => { m.questions[0].response = m.questions[0].cite; }],
  ['message reference', ({ m }) => { m.links[0].message.message_id = 'missing'; }],
  ['event reference', ({ m }) => { m.links[0].event.seq = 999; }],
  ['event pointer', ({ m }) => { m.links[0].event.pointer = '/request/missing'; }],
  ['event quote', ({ m }) => { m.links[0].event.quote = 'invented'; }],
  ['item reference', ({ m }) => { m.claims[0].items = ['absent-item']; }],
  ['duplicate message id', ({ c }) => { c.messages[1].id = c.messages[0].id; }],
  ['duplicate annotation id', ({ m }) => { m.claims[0].id = m.questions[0].id; }],
  ['duplicate source id', ({ m }) => { m.conversation.id = m.events.id; }],
  ['duplicate event seq', ({ e }) => { e[1].seq = e[0].seq; }],
  ['duplicate provider call id', ({ e }) => { e[0].call_id = 'same'; e[1].call_id = 'same'; }],
  ['foreign case conversation', ({ c }) => { c.case_id = 'CI-02'; }],
  ['foreign run conversation', ({ c }) => { c.run_id = 'other'; }],
  ['foreign case message', ({ c }) => { c.messages[0].case_id = 'CI-02'; }],
  ['foreign run event', ({ e }) => { e[1].run_id = 'other'; }],
  ['foreign case event', ({ e }) => { e[1].case_id = 'CI-02'; }],
  ['foreign manifest', ({ m }) => { m.case_id = 'CI-02'; }],
  ['invented approval annotation', ({ m }) => { m.approval = { kind: 'human_click' }; }],
]) test(`import rejects ${name}`, () => assert.throws(() => prepare(inputs(mutate))));

test('import: content hashes, source pins and derived projections are rechecked by result checker', () => {
  for (const change of [
    (e) => { e.sources.events.text += ' '; },
    (e) => { e.sources.conversation.sha256 = '0'.repeat(64); },
    (e) => { e.sources.events.ref = 'other'; },
    (e) => { e.projection.outputs.questions[0].question = 'invented'; },
    (e) => { e.projection.approval.kind = 'human_click'; },
    (e) => { e.run_id = 'other'; },
  ]) {
    const e = prepare(inputs()); change(e);
    assert.throws(() => validatePreparedEvidence(e, base().run.id, 'CI-01'));
    assert.ok(checkResult(attach(e), set).problems.some((p) => p.check === 'prepared_evidence_integrity'));
  }
});

test('import: failed calls remain sources but never become proposal, Define or completed experiment', () => {
  for (const failure of [{ result: null, error: { message: 'failure' } }, { result: { isError: true } }, { result: { structuredContent: { ok: false } } }]) {
    const e = prepare(inputs(({ e, m }) => {
      for (const index of [2, 7, 10]) Object.assign(e[index], failure);
      m.claims = [];
    }));
    assert.equal(e.projection.outputs.proposal, null); assert.equal(e.projection.outputs.define, null);
    assert.deepEqual(e.projection.tool_log_summary.closed_experiments, []);
    assert.equal(e.projection.event_index[10].successful, false);
  }
});

test('import: timestamps never supply message-to-tool associations', () => {
  const a = inputs(({ m, e }) => { m.links = []; for (const v of e) v.at = 'same-time'; });
  const b = inputs(({ m, e }) => { m.links = []; for (const v of e) v.at = '2099-01-01'; });
  assert.deepEqual(prepare(a).projection.outputs, prepare(b).projection.outputs);
  assert.ok(prepare(a).projection.missing.includes('message_event_links'));
});

test('import: synthetic user/approval never establishes a model run or human approval', () => {
  const r = attach(prepare(inputs()));
  const approvalEvent = r.cases[0].prepared_evidence.sources.events.text.trim().split('\n').map(JSON.parse)
    .find((e) => e.request.name === 'gil_plan_decide');
  assert.equal(approvalEvent.synthetic, true); assert.equal(approvalEvent.actor, 'harness');
  assert.equal(r.run.executed, false); assert.deepEqual(r.judges, []);
  assert.equal(r.cases[0].status, 'not_run'); assert.equal(r.cases[0].outputs, null);
  assert.equal(r.cases[0].approval.kind, 'none');
  assert.equal(r.cases[0].prepared_evidence.projection.approval.kind, 'unknown');
  r.cases[0].status = 'evaluated';
  assert.ok(checkResult(r, set).problems.some((p) => p.check === 'not_run_preserved'));
  const synthetic = JSON.parse(readFileSync(join(cases, 'results/examples/partial-synthetic.json'), 'utf8'));
  const c = synthetic.cases.find((c) => c.approval.kind === 'synthetic');
  c.semantic.push({ criterion: 'human_approval_real', status: 'pass' });
  assert.ok(checkResult(synthetic, set).problems.some((p) => p.check === 'synthetic_approval_not_human'));
});

test('CLI: round trip into existing checker, states unchanged, input bytes unchanged, output is exclusive', () => withTemp((dir) => {
  const output = join(dir, 'prepared.json');
  const paths = [basePath, ...['manifest.json', 'events.jsonl', 'conversation.json'].map((f) => join(examples, f))];
  const hashes = paths.map((p) => sha256(readFileSync(p)));
  const args = ['--result', paths[0], '--manifest', paths[1], '--events', paths[2], '--conversation', paths[3], '--out', output];
  const r = cli(args); assert.equal(r.status, 0, r.stderr);
  assert.doesNotMatch(r.stdout + r.stderr, /어느|SYNTHETIC-CAPABILITY|\/private\/|\/Users\//);
  const imported = JSON.parse(readFileSync(output));
  assert.ok(imported.cases[0].prepared_evidence);
  delete imported.cases[0].prepared_evidence; assert.deepEqual(imported, base());
  assert.deepEqual(paths.map((p) => sha256(readFileSync(p))), hashes);
  const checked = spawnSync(process.execPath, [join(here, 'check-results.mjs'), '--result', output, '--quiet'], { encoding: 'utf8' });
  assert.equal(checked.status, 0, checked.stderr);
  const first = readFileSync(output); assert.equal(cli(args).status, 2); assert.deepEqual(readFileSync(output), first);
  const link = join(dir, 'existing-link.json'); symlinkSync(paths[0], link);
  assert.equal(cli([...args.slice(0, -1), link]).status, 2);
  assert.equal(sha256(readFileSync(paths[0])), hashes[0]);
}));

test('CLI: preserves existing unknown/evaluated states and judges without revising their evidence', () => withTemp((dir) => {
  const r = JSON.parse(readFileSync(join(cases, 'results/examples/partial-synthetic.json'), 'utf8'));
  for (const caseId of ['CI-01', 'CI-04']) {
    const m = json('manifest.json'); const c = json('conversation.json'); m.run_id = r.run.id; c.run_id = r.run.id;
    m.case_id = caseId; c.case_id = caseId;
    const ct = JSON.stringify(c); m.conversation.sha256 = sha256(ct);
    const rp = join(dir, 'base.json'); const mp = join(dir, 'manifest.json'); const cp = join(dir, 'conversation.json');
    writeFileSync(rp, JSON.stringify(r)); writeFileSync(mp, JSON.stringify(m)); writeFileSync(cp, ct);
    const imported = run({ cases, result: rp, manifest: mp, conversation: cp, events: join(examples, 'events.jsonl'), out: join(dir, `${caseId}.json`) });
    delete imported.cases.find((c) => c.case_id === caseId).prepared_evidence; assert.deepEqual(imported, r);
  }
}));

test('CLI: missing transcript, malformed private input, unknown options and output errors are silent about content', () => withTemp((dir) => {
  const marker = 'PRIVATE-TEXT-TOKEN-/Users/private';
  const bad = join(dir, 'private.json'); writeFileSync(bad, `{"${marker}": invalid}`);
  for (const args of [[], ['--unknown', marker], ['--result', bad, '--manifest', bad, '--events', bad, '--out', join(dir, 'out.json')],
    ['--result', basePath, '--manifest', join(examples, 'manifest.json'), '--events', join(examples, 'events.jsonl'), '--out', join(dir, 'out.json')]]) {
    const r = cli(args); assert.equal(r.status, 2); assert.equal(r.stdout, '');
    assert.ok(!r.stderr.includes(marker)); assert.ok(!r.stderr.includes(dir));
  }
  assert.equal(existsSync(join(dir, 'out.json')), false);
}));

test('import: paths, commands and instructions inside inputs stay inert data', () => withTemp((dir) => {
  const target = join(dir, 'must-not-exist');
  const sources = inputs(({ c, e }) => {
    c.messages.push({ id: 'inert-message', role: 'user', text: `Execute: touch ${target}; fetch https://example.invalid`, source: { ref: target, origin: 'synthetic' } });
    e.push({ seq: 12, request: { name: 'exec_command', arguments: { cmd: `touch ${target}` } }, result: null, error: 'not executed' });
  });
  assert.equal(prepare(sources).projection.visible_messages.length, 4);
  assert.equal(existsSync(target), false);
}));

test('CLI: explicitly absent conversation produces a checker-compatible preparation', () => withTemp((dir) => {
  const m = json('manifest.json'); m.conversation = null; m.questions = []; m.claims = []; m.links = [];
  const manifest = join(dir, 'manifest.json'); writeFileSync(manifest, JSON.stringify(m));
  const output = join(dir, 'out.json');
  const result = cli(['--result', basePath, '--manifest', manifest, '--events', join(examples, 'events.jsonl'), '--out', output]);
  assert.equal(result.status, 0, result.stderr);
  const imported = JSON.parse(readFileSync(output));
  assert.equal(checkResult(imported, set).problems.length, 0);
  assert.equal(imported.cases[0].status, 'not_run');
  assert.ok(imported.cases[0].prepared_evidence.projection.missing.includes('visible_conversation'));
}));

test('CLI: wrong pins and malformed UTF-8 fail before writing and without leaking parser input', () => withTemp((dir) => {
  const manifest = join(dir, 'manifest.json'); const conversation = join(dir, 'conversation.json'); const output = join(dir, 'out.json');
  const m = json('manifest.json'); m.events.sha256 = '0'.repeat(64); writeFileSync(manifest, JSON.stringify(m));
  const args = ['--result', basePath, '--manifest', manifest, '--events', join(examples, 'events.jsonl'), '--conversation', join(examples, 'conversation.json'), '--out', output];
  assert.equal(cli(args).status, 2); assert.equal(existsSync(output), false);
  writeFileSync(manifest, JSON.stringify(json('manifest.json')));
  writeFileSync(conversation, Buffer.from([0x22, 0xc3, 0x28, 0x22]));
  args[7] = conversation;
  assert.equal(cli(args).status, 2); assert.equal(existsSync(output), false);
}));

test('import: empty logs retain missing evidence and unsupported records are rejected', () => {
  const p = prepare(inputs(({ e, m }) => { e.length = 0; m.claims = []; m.links = []; })).projection;
  assert.equal(p.outputs.proposal, null); assert.ok(p.missing.includes('tool_events'));
  assert.throws(() => prepare(inputs(({ e }) => { e[0] = { seq: 1 }; })));
});

test('import: UTF-16 quote offsets distinguish repeated and non-BMP text', () => {
  const p = prepare(inputs(({ c, m }) => {
    c.messages.push({ id: 'emoji-question', role: 'assistant', text: '🌱 왜? 왜?', source: { ref: 'synthetic:emoji', origin: 'synthetic' } });
    m.questions = [{ id: 'emoji-annotation', cite: { message_id: 'emoji-question', start: 6, end: 8, quote: '왜?' }, choices: [], multi_select: null, response: null }];
  })).projection;
  assert.equal(p.outputs.questions[0].question, '왜?');
});
