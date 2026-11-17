import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, readFileSync, rmSync, cpSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { inspect, pointer } from './lib/evidence.mjs';
import { loadCases, checkResult, skeleton, DEFAULT_CASES, REPO } from './lib/package.mjs';
const set = loadCases();
const good = (n = 1, variant = 0) => structuredClone(set.cases[n - 1].good[variant].bundle);
const statuses = b => Object.fromEntries(inspect(b).map(x => [x.check, x.status]));
const event = (b, id) => b.events.find(e => e.seq === id);
const result = (b = good()) => {
  const r = skeleton(set); r.cases[0].status = 'evaluated'; r.cases[0].reason = 'Synthetic example only.';
  r.cases[0].bundle = b; r.cases[0].automatic = inspect(b);
  r.cases[0].semantic.forEach(s => { s.status = 'unknown'; s.rationale = 'No semantic evaluation.'; });
  return r;
};
const temp = fn => { const dir = mkdtempSync(join(tmpdir(), 'gil-closing-eval-')); try { return fn(dir); } finally { rmSync(dir, { recursive: true, force: true }); } };
const cli = (...args) => spawnSync(process.execPath, [join(REPO, 'tools/chain-closing-eval/check.mjs'), ...args], { encoding: 'utf8' });

test('ten cases and all good examples have no automatic violations', () => {
  assert.deepEqual(set.problems, []); assert.equal(set.cases.length, 10);
  for (const c of set.cases) for (const ex of c.good) assert.ok(inspect(ex.bundle).every(x => x.status !== 'fail'), `${c.id}/${ex.id}`);
});
const expected = {
  question_first: ['presentation_order'], filenames_only: [], limits_hidden: [],
  work_inside_interview: ['interview_writes'], access_failure_hidden: [],
  stale_as_current: ['file_consistency'], 'deleted-as-current': ['file_consistency'],
  silence_as_approval: ['report_binding', 'resume_receipt'],
  'lost-reply-reapproval': ['resume_receipt'], 'wrong-revision': ['approval_binding', 'report_binding'],
  old_authority_reused: ['fresh_interview'], 'wrong-report-verdict': ['report_binding'],
};
for (const c of set.cases) for (const ex of c.bad) test(`${c.id}/${ex.id}: independent negative oracle`, () => {
  assert.deepEqual(inspect(ex.bundle).filter(x => x.status === 'fail').map(x => x.check).sort(), expected[ex.id].slice().sort());
});
test('exact quotation and JSON Pointer resolve; inherited keys never resolve', () => {
  assert.equal(pointer({ 'a/b': { '~': 4 } }, '/a~1b/~0'), 4);
  assert.equal(pointer({}, '/constructor'), undefined);
  const b = good(); b.presentation[0].quote = 'invented quote';
  assert.equal(statuses(b).references, 'fail');
  assert.equal(statuses(b).presentation_order, 'unknown');
});
for (const [label, mutate] of [
  ['duplicate event', b => b.events.push(structuredClone(b.events[0]))],
  ['missing timeline event', b => b.timeline.pop()],
  ['dangling annotation', b => { b.closing.proposal_event = 999; }],
  ['forged timeline order', b => { [b.timeline[0], b.timeline[1]] = [b.timeline[1], b.timeline[0]]; }],
]) test(`reference corruption: ${label}`, () => { const b = good(); mutate(b); assert.equal(statuses(b).references, 'fail'); });
test('missing public messages stays unknown rather than a display pass', () => {
  const b = { events: [], messages: [], timeline: [], files: [], presentation: [], capture_complete: false };
  assert.equal(statuses(b).presentation_order, 'unknown');
  assert.equal(statuses(b).approval_binding, 'unknown');
  assert.equal(statuses(b).file_consistency, 'unknown');
});
for (const field of ['revision', 'digest', 'synthesis_ref']) test(`receipt mismatch: ${field}`, () => {
  const b = good(); event(b, b.closing.receipt_event).result.structuredContent.proposal[field] = field === 'revision' ? 3 : 'wrong';
  assert.equal(statuses(b).approval_binding, 'fail'); assert.equal(statuses(b).report_binding, 'fail');
});
test('cross-project scope cannot bind an approval', () => {
  const b = good(); event(b, b.closing.receipt_event).request.arguments.scope_id = 'other-project';
  assert.equal(statuses(b).approval_binding, 'fail');
});
test('failed tool reply cannot prove saved approval', () => {
  const b = good(); event(b, b.closing.receipt_event).result.isError = true;
  assert.equal(statuses(b).approval_binding, 'fail');
});
test('report may not precede receipt even when both exist', () => {
  const b = good(); const a = b.timeline.indexOf(`e${b.closing.receipt_event}`), z = b.timeline.indexOf(`e${b.closing.report_event}`);
  [b.timeline[a], b.timeline[z]] = [b.timeline[z], b.timeline[a]];
  assert.equal(statuses(b).references, 'fail');
});
test('saved approval followed by a changed proposal invalidates the report binding', () => {
  const b = good(); const re = event(b, b.closing.report_event), old = re.seq;
  re.seq += 1; b.timeline[b.timeline.indexOf(`e${old}`)] = `e${re.seq}`; b.closing.report_event = re.seq;
  const changed = structuredClone(event(b, b.closing.proposal_event)); changed.seq = old;
  changed.result.structuredContent.proposal.digest = 'f'.repeat(64);
  b.events.splice(b.events.length - 1, 0, changed); b.timeline.splice(b.timeline.indexOf(`e${re.seq}`), 0, `e${old}`);
  assert.equal(statuses(b).report_binding, 'fail');
});
test('file bytes must match recorded SHA; a filename is not file evidence', () => {
  const b = good(); event(b, b.files[0].current.event).result.content[0].text += 'changed';
  assert.equal(statuses(b).file_consistency, 'fail');
  b.files[0].current = null; assert.equal(statuses(b).file_consistency, 'unknown');
});
test('file proof must come from tool result and the preceding Verify', () => {
  const b = good(); b.files[0].current.pointer = '/request/arguments/path';
  assert.equal(statuses(b).file_consistency, 'fail');
  const c = good(); event(c, c.files[0].verify_event).request.arguments.report = 'evidence_files: other.txt\n';
  assert.equal(statuses(c).file_consistency, 'fail');
});
test('incomplete capture cannot prove no Interview writes; explicit shell write is checked', () => {
  const b = good(); b.capture_complete = false; assert.equal(statuses(b).interview_writes, 'unknown');
  b.artifact_write_events = [b.closing.proposal_event];
  assert.equal(statuses(b).interview_writes, 'fail');
});
test('pending and saved response-loss variants stay distinct', () => {
  const pending = good(9), saved = good(9, 1);
  assert.equal(statuses(pending).approval_binding, 'unknown');
  assert.equal(statuses(pending).resume_receipt, 'pass');
  assert.equal(statuses(saved).approval_binding, 'pass');
  assert.equal(statuses(saved).resume_receipt, 'pass');
  event(saved, saved.resume.read_event).result = null;
  assert.equal(statuses(saved).resume_receipt, 'unknown');
});
test('result checker recomputes rather than trusting the reported automatic score', () => {
  const r = result(); assert.deepEqual(checkResult(r, set).problems, []);
  r.cases[0].automatic[0].status = 'unknown';
  assert.ok(checkResult(r, set).problems.some(p => /recomputation/.test(p.detail)));
});
test('not_run, unknown and synthetic metadata cannot become model/Host success', () => {
  const r = skeleton(set); assert.deepEqual(checkResult(r, set).problems, []);
  r.cases[0].semantic[0].status = 'pass'; assert.ok(checkResult(r, set).problems.length);
  const s = result(); s.run.host_executed = true; assert.ok(checkResult(s, set).problems.length);
  const u = result(); u.cases[0].status = 'unknown'; u.cases[0].semantic[0].status = 'fail';
  assert.ok(checkResult(u, set).problems.length);
});
test('semantic judgement needs a real exact quote and a human/model judge, not an auto scorer', () => {
  const r = result(); r.judges = [{ id: 'H1', kind: 'human' }];
  const s = r.cases[0].semantic[0]; s.status = 'pass'; s.judge = 'H1'; s.basis = [r.cases[0].bundle.presentation[0]];
  assert.deepEqual(checkResult(r, set).problems, []);
  s.basis = [{ message: 'm1', start: 0, quote: 'fabricated' }]; assert.ok(checkResult(r, set).problems.length);
  s.basis = [r.cases[0].bundle.presentation[0]]; r.judges[0].kind = 'auto'; assert.ok(checkResult(r, set).problems.length);
});
for (const criterion of ['host_display', 'user_opened_result', 'human_approval_real']) test(`${criterion}: tool success and synthetic approval cannot pass`, () => {
  const r = result(); r.judges = [{ id: 'H1', kind: 'human' }];
  const s = r.cases[0].semantic.find(s => s.criterion === criterion); s.status = 'pass'; s.judge = 'H1'; s.basis = [r.cases[0].bundle.presentation[0]];
  assert.ok(checkResult(r, set).problems.some(p => /Host|Synthetic/.test(p.detail)));
});
test('case coverage, criteria coverage and case-set hash are strict', () => {
  for (const mutate of [r => r.cases.pop(), r => r.cases[0].semantic.pop(), r => { r.run.case_set_sha256 = '0'.repeat(64); }, r => r.cases.push(r.cases[0])]) {
    const r = skeleton(set); mutate(r); assert.ok(checkResult(r, set).problems.length);
  }
});
test('fixture corruption rejects a dangling spec reference and changed expected failures', () => temp(dir => {
  cpSync(DEFAULT_CASES, dir, { recursive: true });
  const path = join(dir, 'cases/CC-01.json'); const c = JSON.parse(readFileSync(path));
  c.spec_refs = ['../secret']; c.bad[0].expected_failures = []; writeFileSync(path, JSON.stringify(c));
  assert.ok(loadCases(dir).problems.length >= 2);
}));
test('CLI normal, unknown, invalid, malformed input, missing evidence and exclusive output', () => temp(dir => {
  assert.equal(cli('fixtures').status, 0);
  for (const [name, expected] of [['not-run', 0], ['unknown-synthetic', 0], ['invalid-overclaim', 1]]) {
    assert.equal(cli('result', '--result', join(DEFAULT_CASES, `results/examples/${name}.json`)).status, expected, name);
  }
  const path = join(dir, 'out.json'); assert.equal(cli('skeleton', '--out', path).status, 0);
  const original = readFileSync(path, 'utf8'); assert.equal(cli('skeleton', '--out', path).status, 2);
  assert.equal(readFileSync(path, 'utf8'), original);
  assert.equal(cli('result', '--result', join(dir, 'absent.json')).status, 2);
  writeFileSync(join(dir, 'broken.json'), '{'); assert.equal(cli('result', '--result', join(dir, 'broken.json')).status, 2);
  assert.equal(cli('result', '--result').status, 2);
  const r = result(); r.cases[0].bundle = null;
  writeFileSync(join(dir, 'missing-evidence.json'), JSON.stringify(r));
  assert.equal(cli('result', '--result', join(dir, 'missing-evidence.json')).status, 1);
}));
test('typed visual Verify confirms its exact owner but never Host display', () => {
  const b = good(3); assert.equal(statuses(b).file_consistency, 'pass');
  const e = event(b, b.files[0].verify_event);
  assert.equal(e.result.structuredContent.display_confirmed, false);
  e.request.arguments.expected_owner.ref = 'step:C99/S3';
  assert.equal(statuses(b).file_consistency, 'fail');
});
test('reusing the old file read as current stays unknown', () => {
  const b = good(); b.files[0].current = structuredClone(b.files[0].verified);
  assert.equal(statuses(b).file_consistency, 'unknown');
});
test('unknown locale receipt stays unknown, not a fabricated closure success', () => {
  const b = good(); event(b, b.closing.report_event).result.structuredContent.said = 'Closed chain:C1';
  assert.equal(statuses(b).report_binding, 'unknown');
});
test('results must be in the first public closing message', () => {
  const b = good(); b.messages.unshift({ id: 'm2', role: 'assistant', synthetic: true, text: '닫을까요?' });
  b.timeline.splice(b.timeline.indexOf(`e${b.interview_open_event}`) + 1, 0, 'm2');
  assert.equal(statuses(b).presentation_order, 'fail');
});
test('a result file cannot cite an unknown approved item', () => {
  const b = good(); b.files[0].item_id = 'unapproved'; assert.equal(statuses(b).file_consistency, 'fail');
});
test('JSON object member order does not change exact proposal equality', () => {
  const b = good(); const p = event(b, b.closing.receipt_event).result.structuredContent.proposal;
  p.draft = Object.fromEntries(Object.entries(p.draft).reverse());
  b.closing.shown = Object.fromEntries(Object.entries(b.closing.shown).reverse());
  assert.equal(statuses(b).approval_binding, 'pass');
});
test('App review proposal resolves its project through the prepared scope', () => {
  const b = good(); const p = event(b, b.closing.proposal_event);
  p.request = { name: 'gil_plan_review', arguments: { scope_id: 'synthetic-project-scope' } };
  assert.equal(statuses(b).approval_binding, 'pass');
});
test('close receipt verdict must agree with the submitted Chain Report', () => {
  const b = good(); event(b, b.closing.report_event).result.structuredContent.said = '닫았다: chain:C1 · failure';
  assert.equal(statuses(b).report_binding, 'fail');
});
