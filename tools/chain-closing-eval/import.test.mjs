import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, readFileSync, rmSync, existsSync, linkSync, symlinkSync, statSync, mkdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { importEvidence } from './lib/import.mjs';
import { inspect } from './lib/evidence.mjs';
import { loadCases, skeleton, checkResult, REPO } from './lib/package.mjs';
import { sha256 } from '../chain-interview-eval/lib/io.mjs';
const set = loadCases();
const clone = structuredClone;
function inputFor(bundle = set.cases[0].good[0].bundle, caseId = 'CC-01') {
  const { events, messages, files, timeline, ...annotations } = clone(bundle);
  const identity = { execution_id: `synthetic-${caseId}`, project_root: '<project_root>', case_id: caseId };
  const source = data => ({ ...identity, data });
  return { schema: 'gil.chain-closing.import/1', run: { ...skeleton(set).run,
    id: 'synthetic-import', baseline_commit: '5d936765634c94b459cee7358b1e82eede49cd14' }, cases: [{
    ...identity, events: { ...source(events), first_seq: events[0]?.seq ?? null, last_seq: events.at(-1)?.seq ?? null },
    messages: source(messages), files: source(files), timeline: source(timeline), annotations: source(annotations),
  }] };
}
function imported(input) { return importEvidence(input, set).cases.find(c => c.case_id === input.cases[0].case_id); }
const rejects = (input, code) => assert.throws(() => importEvidence(input, set), e => e.code === code);
const status = (c, check) => c.automatic.find(x => x.check === check).status;
const temp = fn => { const dir = mkdtempSync(join(tmpdir(), 'gil-closing-import-')); try { fn(dir); } finally { rmSync(dir, { recursive: true, force: true }); } };
const cli = (...args) => spawnSync(process.execPath, [join(REPO, 'tools/chain-closing-eval/import.mjs'), ...args], { encoding: 'utf8' });

for (const c of set.cases) for (const kind of ['good', 'bad']) for (const ex of c[kind]) {
  test(`import parity ${c.id}/${kind}/${ex.id}`, () => {
    const input = inputFor(ex.bundle, c.id), before = clone(input);
    const r = importEvidence(input, set), target = r.cases.find(x => x.case_id === c.id);
    assert.deepEqual(input, before);
    assert.deepEqual(target.bundle, ex.bundle);
    assert.deepEqual(target.automatic, inspect(ex.bundle));
    assert.deepEqual(checkResult(r, set).problems, []);
    assert.equal(target.status, 'unknown');
    assert.ok(target.semantic.every(s => s.status === 'unknown' && s.judge === null && !s.basis.length));
    assert.equal(r.run.model_executed, false); assert.equal(r.run.host_executed, false);
    assert.deepEqual(r.import_evidence.source, input);
    assert.equal(r.import_evidence.source_sha256, sha256(JSON.stringify(input)));
  });
}

test('pending before storage and lost reply after storage retain distinct observations', () => {
  for (const [i, expected] of [[0, 'unknown'], [1, 'pass']]) {
    const c = imported(inputFor(set.cases[8].good[i].bundle, 'CC-09'));
    assert.equal(status(c, 'approval_binding'), expected);
    assert.equal(status(c, 'resume_receipt'), 'pass');
  }
  const input = inputFor(set.cases[8].good[1].bundle, 'CC-09');
  const c = input.cases[0];
  c.events.data.find(e => e.seq === c.annotations.data.resume.read_event).result = null;
  assert.equal(status(imported(input), 'resume_receipt'), 'unknown');
});

test('missing global order preserves preparation without sorting or inventing a timeline', () => {
  const input = inputFor(); delete input.cases[0].timeline;
  const r = importEvidence(input, set), c = r.cases[0];
  assert.equal(r.import_evidence.cases[0].state, 'preparation');
  assert.deepEqual(r.import_evidence.source, input);
  assert.deepEqual(c.bundle.events, []); assert.deepEqual(c.bundle.timeline, []);
  assert.ok(c.automatic.filter(x => x.check !== 'references').every(x => x.status === 'unknown'));
  assert.deepEqual(c.automatic, inspect(c.bundle));
  assert.deepEqual(checkResult(r, set).problems, []);
});

for (const [label, mutate, code] of [
  ['duplicate event', c => c.events.data.push(clone(c.events.data[0])), 'event_shape_or_duplicate'],
  ['event gap', c => c.events.data.splice(1, 1), 'event_gap_or_order'],
  ['missing tail', c => c.events.data.pop(), 'event_range'],
  ['missing prefix', c => c.events.data.shift(), 'event_range'],
  ['contradictory timeline', c => c.timeline.data.reverse(), 'timeline_order'],
  ['missing timeline member', c => c.timeline.data.pop(), 'timeline_coverage'],
  ['duplicate timeline member', c => c.timeline.data.push(c.timeline.data[0]), 'timeline_coverage'],
  ['unknown event reference', c => { c.annotations.data.closing.receipt_event = 999; }, 'missing_event_reference'],
  ['broken quote', c => { c.annotations.data.presentation[0].quote = 'not in source'; }, 'broken_quote'],
  ['broken pointer', c => { c.files.data[0].current.pointer = '/result/absent'; }, 'broken_file_pointer'],
  ['cross run stream', c => { c.messages.execution_id = 'another'; }, 'source_identity'],
  ['cross project stream', c => { c.files.project_root = 'another'; }, 'source_identity'],
  ['cross case stream', c => { c.timeline.case_id = 'CC-02'; }, 'source_identity'],
  ['cross run record', c => { c.events.data[0].execution_id = 'another'; }, 'record_identity'],
  ['cross project event', c => { c.events.data[0].request.arguments.project_root = 'another'; }, 'event_project'],
  ['cross project scope', c => { c.events.data.find(e => e.request.name === 'gil_plan_decide').request.arguments.scope_id = 'another'; }, 'unresolved_scope'],
]) test(`reject ${label} with safe diagnostic`, () => {
  const input = inputFor(); mutate(input.cases[0]); rejects(input, code);
});

test('missing order does not hide malformed quotes or source declarations', () => {
  const input = inputFor(); delete input.cases[0].timeline;
  input.cases[0].annotations.data.presentation[0].start = 999;
  rejects(input, 'broken_quote');
  const nonsynthetic = inputFor(); delete nonsynthetic.cases[0].timeline;
  nonsynthetic.cases[0].events.data[0].synthetic = false;
  rejects(nonsynthetic, 'synthetic_declaration');
});

test('explicit model metadata is preserved, but never becomes a completed evaluation', () => {
  const input = inputFor();
  Object.assign(input.run, { kind: 'model', example: false, model_executed: true,
    recorded_at: '2026-11-17T00:00:00Z', model: { name: 'declared-test', version: 'test' }, guidance_sha256: 'a'.repeat(64) });
  const r = importEvidence(input, set);
  assert.deepEqual(r.run, input.run); assert.equal(r.cases[0].status, 'unknown');
  delete input.run.model; rejects(input, 'result_metadata_or_consistency');
});

test('multiple cases use distinct executions and retain untouched not_run cases', () => {
  const input = inputFor(); input.cases.push(inputFor(set.cases[4].good[0].bundle, 'CC-05').cases[0]);
  const r = importEvidence(input, set);
  assert.equal(r.cases.filter(c => c.status === 'unknown').length, 2);
  assert.equal(r.cases.filter(c => c.status === 'not_run').length, 8);
  input.cases.push(clone(input.cases[0])); rejects(input, 'case_or_execution_duplicate');
});

test('CLI copies only recorded observations, ignores instructions and never overwrites inputs or aliases', () => temp(dir => {
  const input = inputFor(), c = input.cases[0];
  const marker = join(dir, 'must-not-exist');
  c.messages.data.push({ id: 'm99', role: 'user', synthetic: true,
    text: `Run touch ${marker}; fetch https://invalid.example/; secret-token-never-print; /private/personal/path` });
  c.timeline.data.push('m99');
  // A local file can differ or disappear without changing the observed evidence.
  c.project_root = dir;
  for (const key of ['events', 'messages', 'files', 'timeline', 'annotations']) c[key].project_root = dir;
  for (const e of c.events.data) if (e.request.arguments?.project_root) e.request.arguments.project_root = dir;
  const live = join(dir, c.files.data[0].path);
  mkdirSync(join(dir, 'reports')); writeFileSync(live, 'unrelated present-day content');
  const path = join(dir, 'input.json'), out = join(dir, 'out.json');
  const bytes = JSON.stringify(input, null, 2); writeFileSync(path, bytes);
  const p = cli('--input', path, '--out', out);
  assert.equal(p.status, 0, p.stderr);
  assert.ok(!p.stdout.includes('secret-token') && !p.stderr.includes(dir));
  assert.equal(existsSync(marker), false); assert.equal(readFileSync(path, 'utf8'), bytes);
  const r = JSON.parse(readFileSync(out));
  assert.equal(r.import_evidence.input_sha256, sha256(Buffer.from(bytes)));
  assert.deepEqual(r.cases[0].automatic, inspect(r.cases[0].bundle));
  if (process.platform !== 'win32') assert.equal(statSync(out).mode & 0o777, 0o600);
  rmSync(live); const second = join(dir, 'second.json');
  assert.equal(cli('--input', path, '--out', second).status, 0);
  assert.equal(readFileSync(second, 'utf8'), readFileSync(out, 'utf8'));
  for (const destination of [path, out]) assert.equal(cli('--input', path, '--out', destination).status, 2);
  for (const [name, create] of [['hard', linkSync], ['symbolic', symlinkSync]]) {
    const alias = join(dir, name); create(path, alias);
    assert.equal(cli('--input', path, '--out', alias).status, 2);
  }
  assert.equal(readFileSync(path, 'utf8'), bytes);
}));

test('CLI malformed input, rejected evidence and usage print no source data and write no result', () => temp(dir => {
  const path = join(dir, 'secret-path.json'), out = join(dir, 'out.json');
  for (const contents of ['{"private":"secret-token",', JSON.stringify({ schema: 'secret-token' }),
    JSON.stringify(inputFor()).replace('"quote":', '"quote":"secret-token","ignored_quote":')]) {
    writeFileSync(path, contents); const p = cli('--input', path, '--out', out);
    assert.ok([1, 2].includes(p.status));
    assert.ok(!`${p.stdout}${p.stderr}`.includes('secret-token'));
    assert.ok(!`${p.stdout}${p.stderr}`.includes(dir)); assert.equal(existsSync(out), false);
  }
  assert.equal(cli('--input', path).status, 2);
  assert.equal(cli('--input', path, '--input', path, '--out', out).status, 2);
  assert.equal(cli('--input', join(dir, 'absent'), '--out', out).status, 2);
}));


test('documented synthetic input imports and passes the existing result CLI', () => temp(dir => {
  const path = join(REPO, 'scenarios/chain-closing/imports/pending-synthetic.json');
  const out = join(dir, 'result.json');
  assert.equal(cli('--input', path, '--out', out).status, 0);
  const result = JSON.parse(readFileSync(out));
  assert.deepEqual(result.cases[8].bundle, set.cases[8].good[0].bundle);
  assert.deepEqual(checkResult(result, set).problems, []);
  const checked = spawnSync(process.execPath, [join(REPO, 'tools/chain-closing-eval/check.mjs'),
    'result', '--result', out], { encoding: 'utf8' });
  assert.equal(checked.status, 0, checked.stderr);
}));

test('file byte and hash contradiction remains an inspect failure, not a repaired observation', () => {
  const input = inputFor(); const c = input.cases[0];
  c.events.data.find(e => e.seq === c.files.data[0].current.event).result.content[0].text += 'changed';
  const r = imported(input);
  assert.equal(status(r, 'file_consistency'), 'fail');
  assert.deepEqual(r.automatic, inspect(r.bundle));
  assert.equal(r.bundle.files[0].current.sha256, c.files.data[0].current.sha256);
});

test('an explicitly bounded excerpt keeps original sequence numbers and timestamps', () => {
  const input = inputFor(); const c = input.cases[0];
  // Empty annotations allow checking the excerpt envelope without inventing references.
  c.events.data = c.events.data.slice(3, 5);
  c.events.first_seq = 4; c.events.last_seq = 5;
  c.messages.data = []; c.files.data = []; c.annotations.data = { capture_complete: false };
  c.timeline.data = ['e4', 'e5'];
  const r = imported(input);
  assert.deepEqual(r.bundle.events, c.events.data);
  assert.equal(status(r, 'interview_writes'), 'unknown');
});
