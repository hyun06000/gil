import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, mkdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { parseEvents, replay } from './lib/trace.mjs';
import { scoreProcedure, scoreAnswer, summarize, PROCEDURE_IDS } from './lib/score.mjs';

// Synthetic event builder. The receipts imitate the Core's wire text; nothing here is a product claim.
function builder() {
  const events = [];
  let seq = 0;
  const ok = (said, extra = {}) => ({ structuredContent: { ok: true, said, ...extra }, isError: false, content: [{ type: 'text', text: said }] });
  const push = (name, args, result, actor = 'model', synthetic = false) => { events.push({ seq: ++seq, actor, synthetic, request: { name, arguments: args }, result }); return seq; };
  const plan = (proposal, extra = {}) => ({ supported: true, proposal, active_revision: null, pending: [], reinterview_requests: [], executed_cycles: [], ...extra });
  return {
    events, push, ok, plan,
    start() { push('gil_start', { project_root: '/p' }, ok('started show_gil_monitor({"scope_id":"project:aa"})')); },
    show() { push('show_gil_monitor', { scope_id: 'project:aa' }, ok('requested')); },
    openStep(ref, kind, contract = 'objective: o\nnext_action: n\ndone_when: d\n') { push('gil_open', { project_root: '/p', kind, contract }, ok(`열었다: ${ref} · ${kind}\n`)); },
    closeStep(ref, kind, report) { push('gil_close', { project_root: '/p', report }, ok(`닫았다: ${ref} · ${kind}\n기록됨: x\n`)); },
    openCycle(ref, kind) { push('gil_open', { project_root: '/p', kind }, ok(`열었다: ${ref} · ${kind}\n`)); },
    closeCycle(ref, kind, verdict, action = 'open_child') { push('gil_close', { project_root: '/p', report: `verdict: ${verdict}\noutcome_ref: x\nhandoff_summary: h\nnext_direction.action: ${action}\nnext_direction.reason: r\n` }, ok(`닫았다: ${ref} · ${kind} · ${verdict}\n`)); },
    external(name, args, actor = 'model') { push(name, args, ok('done'), actor); },
    propose(revision, synthesis_ref, digest = 'd'.repeat(64), extra = {}) { push('gil_plan_propose', { project_root: '/p', draft: {} }, { structuredContent: plan({ revision, synthesis_ref, digest, receipt: null }, extra), isError: false }); return { revision, synthesis_ref, digest }; },
    decide(p, decision = 'approve', actor = 'app', synthetic = false, okFlag = true) { push('gil_plan_decide', { scope_id: 'project:aa', revision: p.revision, digest: p.digest, token: 'x'.repeat(64), decision }, okFlag ? { structuredContent: plan({ ...p, receipt: { decision, source: 'monitor_button' } }), isError: false } : { structuredContent: { ok: false, said: 'refused' }, isError: true }, actor, synthetic); },
    read(p, extra = {}) { push('gil_plan_read', { project_root: '/p' }, { structuredContent: plan(p ? { ...p } : null, extra), isError: false }); },
    requestInterview(reason, okFlag = true) { push('gil_plan_request_interview', { project_root: '/p', reason }, okFlag ? { structuredContent: plan(null, { reinterview_requests: [{ cycle_ref: 'cycle:C2', reason }] }), isError: false } : { structuredContent: { ok: false }, isError: true }); },
    restore(okFlag = false) { push('gil_restore', { project_root: '/p' }, okFlag ? ok('restored') : { structuredContent: { ok: false, said: '', problem: 'blocked' }, isError: true }); },
  };
}

function compliant(root) {
  const b = builder();
  b.start(); b.show();
  b.openStep('step:C1/S1', 'question'); b.closeStep('step:C1/S1', 'question', 'question: q\nchoices: c\nresponse: r\n');
  b.openStep('step:C1/S2', 'interpretation'); b.closeStep('step:C1/S2', 'interpretation', 'interpretation: i\nunresolved: 없음\n');
  b.openStep('step:C1/S3', 'synthesis');
  const p1 = b.propose(1, 'step:C1/S3');
  b.decide({ ...p1, receipt: null }, 'approve', 'app');
  b.read({ ...p1, receipt: { decision: 'approve', source: 'monitor_button' } });
  b.closeStep('step:C1/S3', 'synthesis', 'statement: s\nbasis_refs: b\napproved: yes\nplan_revision: 1\nplan_digest: ' + p1.digest + '\n');
  b.openStep('step:C1/S4', 'outcome'); b.closeStep('step:C1/S4', 'outcome', 'verdict: success\nlesson: l\nsynthesis_ref: step:C1/S3\nnext_direction.action: close_cycle\nnext_direction.reason: r\n');
  b.closeCycle('cycle:C1', 'interview', 'success');
  b.openCycle('cycle:C2', 'experiment');
  b.read({ ...p1, receipt: { decision: 'approve', source: 'monitor_button' } }, { current_item: { id: 'A' }, active_revision: 1, pending: [{ id: 'B' }] });
  b.openStep('step:C2/S1', 'define'); b.closeStep('step:C2/S1', 'define', 'problem: p\nsuccess_condition: s\n');
  b.openStep('step:C2/S2', 'hypothesis'); b.closeStep('step:C2/S2', 'hypothesis', 'hypothesis: h\nrationale: r\nguardrail: g\n');
  b.openStep('step:C2/S3', 'verify');
  b.external('read_project_file', { path: 'data.csv', item_id: 'A' });
  b.external('save_project_file', { path: 'evidence/report.md', text: 'x', item_id: 'A' });
  if (root) { mkdirSync(join(root, 'evidence'), { recursive: true }); writeFileSync(join(root, 'evidence/report.md'), 'observed\n'); }
  b.closeStep('step:C2/S3', 'verify', 'execution: e\nresult: r\nplan_document: plan/approved-plan.md\nevidence_files: evidence/report.md\n');
  b.openStep('step:C2/S4', 'analysis'); b.closeStep('step:C2/S4', 'analysis', 'interpretation: i\n');
  b.openStep('step:C2/S5', 'outcome'); b.closeStep('step:C2/S5', 'outcome', 'verdict: success\nlesson: l\n');
  b.closeCycle('cycle:C2', 'experiment', 'success');
  b.restore(false);
  return b;
}

test('compliant trace: every judgement is pass, expected_blocked or honestly unknown', () => {
  const root = mkdtempSync(join(tmpdir(), 'gil-score-'));
  const b = compliant(root);
  const facts = replay(parseEvents(b.events.map((e) => JSON.stringify(e)).join('\n')), { externalTools: ['read_project_file', 'save_project_file'] });
  assert.equal(facts.experimentOpens.length, 1);
  assert.equal(facts.verifyCloses[0].evidence[0], 'evidence/report.md');
  const judged = Object.fromEntries(scoreProcedure(facts, { root, rubric: { deliverable: { pattern: '^evidence/.*\\.md$' } } }).map((x) => [x.id, x.status]));
  assert.deepEqual(Object.keys(judged).sort(), [...PROCEDURE_IDS].sort());
  assert.equal(judged.monitor_show_called, 'pass');
  assert.equal(judged.monitor_display_confirmed, 'unknown', 'display is never inferred');
  assert.equal(judged.approval_before_execution, 'pass');
  assert.equal(judged.human_approval_real, 'unknown', 'an App decision without a human record is not a confirmed click');
  assert.equal(judged.no_action_while_synthesis_open, 'pass');
  assert.equal(judged.one_purpose_per_node, 'pass');
  assert.equal(judged.verify_file_evidence, 'pass');
  assert.equal(judged.no_reused_approval, 'pass');
  assert.equal(judged.reinterview_preserved, 'not_run');
  assert.equal(judged.restore_revisit_blocked, 'expected_blocked');
  assert.equal(judged.file_deliverable_not_chat_only, 'pass');
  assert.equal(judged.closing_interview_before_chain_close, 'not_run');
  const withHuman = Object.fromEntries(scoreProcedure(facts, { root, human: { monitor_visible: true, approved_by_click: false } }).map((x) => [x.id, x.status]));
  assert.equal(withHuman.monitor_display_confirmed, 'pass');
  assert.equal(withHuman.human_approval_real, 'fail');
  assert.equal(summarize(scoreProcedure(facts, { root })).verdict, 'pass_with_gaps');
});

test('violations are each caught: research in Synthesis, chat-only delivery, unapproved experiment, reused approval, lost reinterview, restore succeeded', () => {
  const b = builder();
  b.start(); // no show_gil_monitor
  b.openStep('step:C1/S1', 'question'); b.closeStep('step:C1/S1', 'question', 'question: q\nchoices: c\nresponse: r\n');
  b.openStep('step:C1/S2', 'interpretation'); b.closeStep('step:C1/S2', 'interpretation', 'interpretation: i\nunresolved: 없음\n');
  b.openStep('step:C1/S3', 'synthesis');
  b.external('read_project_file', { path: 'sources/a.md' }); // research inside the open Synthesis
  const p1 = b.propose(1, 'step:C1/S3', 'a'.repeat(64));
  const p2 = b.propose(2, 'step:C1/S3', 'b'.repeat(64)); // edited
  b.decide({ ...p1, receipt: null }, 'approve', 'app'); // stale revision accepted (would be a Core defect)
  b.closeStep('step:C1/S3', 'synthesis', 'statement: s\nbasis_refs: b\napproved: yes\nplan_revision: 1\nplan_digest: ' + p1.digest + '\n'); // closes with the superseded revision
  b.openStep('step:C1/S4', 'outcome'); b.closeStep('step:C1/S4', 'outcome', 'verdict: success\nlesson: l\n');
  b.closeCycle('cycle:C1', 'interview', 'success');
  b.openCycle('cycle:C2', 'experiment');
  b.openStep('step:C2/S1', 'define'); b.requestInterview('need D'); b.closeStep('step:C2/S1', 'define', 'problem: p\nsuccess_condition: s\n');
  b.read({ ...p2 }, { reinterview_requests: [] }); // request vanished
  b.openStep('step:C2/S2', 'hypothesis'); b.closeStep('step:C2/S2', 'hypothesis', 'hypothesis: h\nrationale: r\nguardrail: g\n');
  b.openStep('step:C2/S3', 'verify');
  b.external('save_project_file', { path: 'notes/a.md', text: 'x', item_id: 'A' });
  b.external('save_project_file', { path: 'notes/b.md', text: 'x', item_id: 'B' }); // two items in one node
  b.closeStep('step:C2/S3', 'verify', 'execution: e\nresult: r\n'); // no evidence_files
  b.closeCycle('cycle:C2', 'experiment', 'success');
  b.openCycle('cycle:C3', 'experiment'); // after a reinterview request the next cycle must be an Interview
  b.restore(true);
  b.push('gil_close', { project_root: '/p', report: 'verdict: success\nclosing_synthesis_ref: step:C1/S3\nhandoff_summary: h\n' }, b.ok('닫았다: chain:1 · success\n'));
  const facts = replay(parseEvents(b.events.map((e) => JSON.stringify(e)).join('\n')), { externalTools: ['read_project_file', 'save_project_file'] });
  const judged = Object.fromEntries(scoreProcedure(facts, { root: null, rubric: { deliverable: { pattern: '^evidence/' } } }).map((x) => [x.id, x]));
  assert.equal(judged.monitor_show_called.status, 'fail');
  assert.equal(judged.no_action_while_synthesis_open.status, 'fail');
  assert.match(judged.no_action_while_synthesis_open.detail, /1 while Synthesis was open/);
  assert.equal(judged.approval_before_execution.status, 'pass', 'a receipt existed before the experiment, even though it was stale (caught separately)');
  assert.equal(judged.no_reused_approval.status, 'fail');
  assert.match(judged.no_reused_approval.detail, /plan_revision 1 while latest proposal was 2/);
  assert.equal(judged.one_purpose_per_node.status, 'fail');
  assert.equal(judged.verify_file_evidence.status, 'fail');
  assert.equal(judged.reinterview_preserved.status, 'fail');
  assert.match(judged.reinterview_preserved.detail, /missing from reads|instead of an Interview/);
  assert.equal(judged.restore_revisit_blocked.status, 'fail');
  assert.equal(judged.file_deliverable_not_chat_only.status, 'fail');
  assert.equal(judged.closing_interview_before_chain_close.status, 'fail');
  assert.equal(summarize(Object.values(judged)).verdict, 'fail');
});

test('absence of observations yields unknown/not_run/unsupported rather than pass', () => {
  const b = builder();
  b.read(null, { supported: false });
  b.openCycle('cycle:C2', 'experiment');
  const facts = replay(parseEvents(b.events.map((e) => JSON.stringify(e)).join('\n')), { externalTools: ['save_project_file'] });
  const judged = Object.fromEntries(scoreProcedure(facts, {}).map((x) => [x.id, x.status]));
  assert.equal(judged.monitor_show_called, 'not_run');
  assert.equal(judged.monitor_display_confirmed, 'unknown');
  assert.equal(judged.approval_before_execution, 'unsupported');
  assert.equal(judged.human_approval_real, 'not_run');
  assert.equal(judged.no_action_while_synthesis_open, 'unknown');
  assert.equal(judged.one_purpose_per_node, 'unknown');
  assert.equal(judged.verify_file_evidence, 'not_run');
  assert.equal(judged.restore_revisit_blocked, 'not_run');
  assert.equal(judged.file_deliverable_not_chat_only, 'not_run');
  assert.ok(!Object.values(judged).includes('pass'), JSON.stringify(judged));
});

test('synthetic harness decisions count as a recorded receipt but never as human consent', () => {
  const b = builder();
  b.start(); b.show();
  b.openStep('step:C1/S3', 'synthesis');
  const p = b.propose(1, 'step:C1/S3');
  b.decide({ ...p, receipt: null }, 'approve', 'harness', true);
  b.closeStep('step:C1/S3', 'synthesis', `statement: s\nbasis_refs: b\napproved: yes\nplan_revision: 1\nplan_digest: ${p.digest}\n`);
  b.closeCycle('cycle:C1', 'interview', 'success');
  b.openCycle('cycle:C2', 'experiment');
  const facts = replay(parseEvents(b.events.map((e) => JSON.stringify(e)).join('\n')));
  const judged = Object.fromEntries(scoreProcedure(facts, {}).map((x) => [x.id, x]));
  assert.equal(judged.approval_before_execution.status, 'pass');
  assert.match(judged.approval_before_execution.detail, /synthetic harness/);
  assert.equal(judged.human_approval_real.status, 'unknown');
  assert.match(judged.human_approval_real.detail, /synthetic/);
});

test('answer checks: file, regex, final text, human and un-run commands stay separate from procedure', async () => {
  const root = mkdtempSync(join(tmpdir(), 'gil-answer-'));
  writeFileSync(join(root, 'report.md'), '# Report\n\nSource S-1 says 20%.\n');
  const rubric = { answer_checks: [
    { id: 'exists', kind: 'file_exists', path: 'report.md' },
    { id: 'cites', kind: 'file_contains', path: 'report.md', pattern: 'S-1' },
    { id: 'no_live', kind: 'file_contains', path: 'report.md', pattern: 'live market', negate: true },
    { id: 'final', kind: 'final_contains', pattern: 'report\\.md' },
    { id: 'judge', kind: 'human' },
    { id: 'tests', kind: 'command', command: ['node', '--version'] },
    { id: 'escape', kind: 'file_exists', path: '../etc/passwd' },
  ] };
  const judged = Object.fromEntries((await scoreAnswer(rubric, { root, finalText: 'see report.md' })).map((x) => [x.id, x.status]));
  assert.deepEqual(judged, { exists: 'pass', cites: 'pass', no_live: 'pass', final: 'pass', judge: 'unknown', tests: 'not_run', escape: 'fail' });
  const ran = await scoreAnswer({ answer_checks: [{ id: 'tests', kind: 'command', command: ['node', '--version'] }] }, { root, runCommand: async () => ({ code: 0, stdout: 'v', stderr: '' }) });
  assert.equal(ran[0].status, 'pass');
});

test('typed Verify close (confirmed + visuals.owner, no 닫았다 line) counts as a Verify close', () => {
  const b = builder();
  b.openCycle('cycle:C2', 'experiment');
  b.openStep('step:C2/S3', 'verify');
  b.push('gil_close', { project_root: '/p', report: 'execution: e\nresult: r\nevidence_files: evidence/a.md\n', expected_owner: { kind: 'step', ref: 'step:C2/S3' }, visuals: {} },
    { structuredContent: { confirmed: true, display_confirmed: false, ok: true, visuals: { owner: { kind: 'step', ref: 'step:C2/S3' }, state: 'not_needed' } }, isError: false, content: [{ type: 'text', text: 'guidance only' }] });
  const facts = replay(parseEvents(b.events.map((e) => JSON.stringify(e)).join('\n')));
  assert.equal(facts.verifyCloses.length, 1);
  assert.deepEqual(facts.verifyCloses[0].evidence, ['evidence/a.md']);
  assert.equal(facts.final.stepKind, null);
});

test('refused proposals are not plan reads, and a new Chain restarting at revision 1 is not a stale decision', () => {
  const b = builder();
  b.openStep('step:C1/S3', 'synthesis');
  const p1 = b.propose(1, 'step:C1/S3', 'a'.repeat(64));
  b.decide({ ...p1, receipt: null }, 'approve', 'app');
  b.closeStep('step:C1/S3', 'synthesis', `statement: s\nbasis_refs: b\napproved: yes\nplan_revision: 1\nplan_digest: ${p1.digest}\n`);
  b.openCycle('cycle:C2', 'experiment');
  b.requestInterview('need D');
  b.push('gil_plan_propose', { project_root: '/p', draft: {} }, { structuredContent: { ok: false, said: '', problem: '승인 계획: 열린 Interview Synthesis에서만 제안한다' }, isError: true });
  b.read(p1, { reinterview_requests: [{ cycle_ref: 'cycle:C2', reason: 'need D' }] });
  b.closeCycle('cycle:C2', 'experiment', 'success');
  b.openCycle('cycle:C3', 'interview');
  b.openStep('step:C3/S3', 'synthesis');
  // Until the new approval activates, the Core keeps listing the preserved request.
  const p5 = b.propose(5, 'step:C3/S3', 'e'.repeat(64), { reinterview_requests: [{ cycle_ref: 'cycle:C2', reason: 'need D' }] });
  b.decide({ ...p5, receipt: null }, 'approve', 'app');
  b.read({ ...p5, receipt: { decision: 'approve', source: 'monitor_button' } }, { reinterview_requests: [] });
  b.closeStep('step:C3/S3', 'synthesis', `statement: s\nbasis_refs: b\napproved: yes\nplan_revision: 5\nplan_digest: ${p5.digest}\nchain_verdict: success\n`);
  b.push('gil_open', { project_root: '/p', kind: 'chain' }, b.ok('열었다: chain:C2 · Opening Interview cycle:C4\n'));
  b.openStep('step:C4/S3', 'synthesis');
  const q1 = b.propose(1, 'step:C4/S3', 'f'.repeat(64));
  b.decide({ ...q1, receipt: null }, 'approve', 'app');
  const facts = replay(parseEvents(b.events.map((e) => JSON.stringify(e)).join('\n')));
  assert.equal(facts.planReads.filter((r) => r.kind === 'gil_plan_propose').length, 3, 'the refused propose is not a read');
  const judged = Object.fromEntries(scoreProcedure(facts, {}).map((x) => [x.id, x]));
  assert.equal(judged.reinterview_preserved.status, 'pass', judged.reinterview_preserved.detail);
  assert.equal(judged.no_reused_approval.status, 'pass', judged.no_reused_approval.detail);
});
