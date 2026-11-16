import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { chainModel, chainLine, chainContrast, chainListEntries, chainDetail, chainOfCycle, verdictText } from '../ui/chain.mjs';
// These assertions read the Korean table; key parity across languages lives in locale.test.mjs.
import { setLocale, LOCALES } from '../ui/locale.js';
setLocale('ko');

const legacy = () => JSON.parse(readFileSync(new URL('../ui/fixtures/reading-one/view.json', import.meta.url)));

// Synthetic Chain facts shaped like the M6-C projection (Chain storage contract, "읽기와 UI").
// No fixture under ui/fixtures carries Chains: those are what the default build produces.
const cycle = (ref, kind, state, extra = {}) => ({ cycle_ref: ref, kind, state, relation_to_current: 'other',
  parent_cycle_ref: null, revisit_from_cycle_ref: null, experiment_definition: null, interview_question: null,
  report: null, steps: [], ...extra });
const closedReport = (verdict, handoff) => ({ verdict, handoff_summary: handoff, outcome_lesson: null, next_direction: null });
const chain = (id, state, cycles, extra = {}) => ({ chain_ref: `chain:${id}`, existence_ref: 'existence:X1', state,
  parent_chain_ref: null, entry_snapshot_ref: 'snapshot:A1', exit_snapshot_ref: state === 'closed' ? 'snapshot:A9' : null,
  current_cycle_ref: cycles.at(-1), cycle_refs: cycles, ...extra });

const LONG = Array.from({ length: 40 }, (_, i) => `인계 ${i + 1}행 — 탐색한 경로와 실패, 남은 질문을 그대로 보존한다.`).join('\n');
const INJECTION = '<img src=x onerror="alert(1)"> & <script>window.evil=1</script>';

/** Three Chains: C1 closed success, C2 closed failure with a long handoff, C3 open and current. */
function several({ currentState = 'open', closedCycle = false, actions = [] } = {}) {
  return {
    schema_version: 1, captured_at_unix_ms: 0,
    current: { chain_ref: 'chain:C3', existence_ref: 'existence:X1', journey_ref: 'journey:X1@J3', cycle_ref: 'cycle:C5', step_ref: closedCycle ? null : 'step:C5/S1' },
    chain: chain('C3', currentState, ['cycle:C4', 'cycle:C5'], { parent_chain_ref: 'chain:C2', ...(currentState === 'closed'
      ? { journey_ref: 'journey:X1@J3', report: { verdict: 'success', closing_synthesis_ref: 'step:C5/S3', handoff_summary: '세 번째 Chain 마무리' } } : {}) }),
    chains: [
      chain('C1', 'closed', ['cycle:C1', 'cycle:C2'], { journey_ref: 'journey:X1@J1',
        report: { verdict: 'success', closing_synthesis_ref: 'step:C2/S3', handoff_summary: INJECTION } }),
      chain('C2', 'closed', ['cycle:C3'], { parent_chain_ref: 'chain:C1', journey_ref: 'journey:X1@J2',
        report: { verdict: 'failure', closing_synthesis_ref: 'step:C3/S3', handoff_summary: LONG, extra_note: 'stored as written' } }),
      chain('C3', currentState, ['cycle:C4', 'cycle:C5'], { parent_chain_ref: 'chain:C2', ...(currentState === 'closed'
        ? { journey_ref: 'journey:X1@J3', report: { verdict: 'success', closing_synthesis_ref: 'step:C5/S3', handoff_summary: '세 번째 Chain 마무리' } } : {}) }),
    ],
    timeline: [
      cycle('cycle:C1', 'interview', 'closed', { report: closedReport('success', '첫 인터뷰') }),
      cycle('cycle:C2', 'experiment', 'closed', { report: closedReport('success', '첫 실험 성공') }),
      cycle('cycle:C3', 'interview', 'closed', { report: closedReport('success', '실패로 닫기로 합의') }),
      cycle('cycle:C4', 'interview', 'closed', { report: closedReport('success', '새 주제 합의') }),
      cycle('cycle:C5', 'experiment', closedCycle ? 'closed' : 'open', { relation_to_current: 'active_path',
        ...(closedCycle ? { report: closedReport('success', '실험은 성공') } : {}),
        steps: closedCycle ? [] : [{ step_ref: 'step:C5/S1', kind: 'define', state: 'open', summary: null }] }),
    ],
    current_will: null,
    world: { baseline_snapshot_ref: 'snapshot:A1', state: 'clean', reason: null, verify_can_confirm: false },
    next_actions: actions.map((kind) => ({ kind, command: null, cycle_kind: null, step_kind: null, help_ref: null, reason: null })),
  };
}

test('a record without Chains (format 4/5) yields nothing: no line, no list, no detail, no error', () => {
  const view = legacy();
  const before = JSON.stringify(view);
  assert.equal(chainModel(view), null);
  assert.equal(chainLine(view), '');
  assert.equal(chainContrast(view), '');
  assert.deepEqual(chainListEntries(view), []);
  assert.equal(chainDetail(view, 'chain:C1'), null);
  assert.equal(chainOfCycle(view, 'cycle:C1'), null);
  assert.equal(JSON.stringify(view), before, 'the View is not mutated');
  for (const odd of [null, undefined, {}, { chains: 'no' }, { current: {} }]) assert.equal(chainModel(odd), null);
});

test('one open Chain reads as the current Chain, first of its kind, with no verdict invented', () => {
  const view = several();
  view.chains = [chain('C1', 'open', ['cycle:C5'])];
  view.chain = view.chains[0]; view.current.chain_ref = 'chain:C1';
  const model = chainModel(view);
  assert.equal(model.current.ref, 'chain:C1');
  assert.deepEqual(model.previous, []);
  assert.equal(model.current.verdict, '');
  assert.equal(chainLine(view), '지금 작업 중인 Chain: C1 (열림) · 첫 Chain');
  assert.equal(chainContrast(view), '', 'an open Step inside an open Chain needs no contrast');
  const detail = chainDetail(view, 'chain:C1');
  assert.equal(detail.report, null);
  assert.equal(detail.noReport, '아직 열려 있어 Chain 보고서가 없습니다.');
  assert.equal(detail.isCurrent, true);
});

test('a View that carries only `chain` and `current.chain_ref` still reads (chains omitted)', () => {
  const view = several();
  view.chains = [];
  const model = chainModel(view);
  assert.equal(model.all.length, 1);
  assert.equal(model.current.ref, 'chain:C3');
  assert.equal(chainListEntries(view).length, 1);
  assert.ok(chainDetail(view, 'chain:C3'));
});

test('several Chains: the current one is named, the previous ones are counted in issue order', () => {
  const view = several();
  const model = chainModel(view);
  assert.deepEqual(model.all.map((one) => one.ref), ['chain:C1', 'chain:C2', 'chain:C3']);
  assert.deepEqual(model.previous.map((one) => one.id), ['C1', 'C2']);
  assert.equal(model.current.id, 'C3');
  assert.equal(model.current.ownsCurrentCycle, true);
  assert.equal(chainLine(view), '지금 작업 중인 Chain: C3 (열림) · 이전 Chain 2개');
  const entries = chainListEntries(view);
  assert.deepEqual(entries.map((one) => one.label), [
    'Chain C1 · 닫힘 · 성공 (success)', 'Chain C2 · 닫힘 · 성공 미확인 (failure)', 'Chain C3 · 열림 · 현재']);
  assert.deepEqual(entries.map((one) => one.isCurrent), [false, false, true]);
  assert.equal(chainOfCycle(view, 'cycle:C3').ref, 'chain:C2');
  assert.equal(chainOfCycle(view, 'cycle:C99'), null, 'ownership comes from cycle_refs, never from ancestry');
});

test('a closed Cycle inside an open Chain is contrasted; a Cycle success is never a Chain success', () => {
  const view = several({ closedCycle: true });
  assert.equal(chainLine(view), '지금 작업 중인 Chain: C3 (열림) · 이전 Chain 2개');
  assert.equal(chainContrast(view),
    '실험 C5은 닫혔지만(성공 (success)) Chain C3은 아직 열려 있습니다. 사이클 결과는 Chain의 결과가 아닙니다.');
  assert.equal(chainModel(view).current.state, 'open');
  assert.equal(chainModel(view).current.verdict, '');
});

test('an available close_chain is a possibility, not a closure; open_chain after a closed Chain is named as the next step', () => {
  const open = several({ closedCycle: true, actions: ['close_chain'] });
  assert.match(chainContrast(open), /Chain C3을 지금 닫을 수 있는 상태입니다\. 아직 닫히지 않았습니다\.$/);
  assert.equal(chainModel(open).current.state, 'open');
  const closed = several({ currentState: 'closed', closedCycle: true, actions: ['open_chain'] });
  assert.equal(chainLine(closed), 'Chain C3 닫힘 · 성공 (success) · 이전 Chain 2개');
  assert.equal(chainContrast(closed), 'Chain C3은 닫혔습니다. 다음 Chain은 새 인터뷰로 열 수 있습니다.');
  const closedQuiet = several({ currentState: 'closed', closedCycle: true });
  assert.equal(chainContrast(closedQuiet), '', 'without the action in next_actions nothing is promised');
});

test('closed Chain detail keeps the Report literally: verdict word beside the stored value, long handoff whole, unknown keys shown', () => {
  const view = several();
  const before = JSON.stringify(view);
  const success = chainDetail(view, 'chain:C1');
  assert.deepEqual(success.owner, { kind: 'chain', ref: 'chain:C1' });
  assert.equal(success.head, 'Chain C1 · 닫힘');
  assert.equal(success.isCurrent, false);
  assert.equal(success.note, '이전 Chain을 읽기 전용으로 봅니다. 현재 작업 위치와 기록은 바뀌지 않습니다. 사이클을 누르면 그 보고서를 봅니다.');
  assert.deepEqual(success.report.fields, [
    { name: '판정', value: '성공 (success)' },
    { name: '인계 요약', value: INJECTION },
    { name: '종료 근거 (승인된 합의 확인)', value: 'step:C2/S3' }]);
  const failure = chainDetail(view, 'chain:C2');
  assert.equal(failure.report.fields[0].value, '성공 미확인 (failure)', 'failure is not a refutation (§1.2)');
  assert.equal(failure.report.fields[1].value, LONG, 'the handoff is neither cut nor re-summarized');
  assert.deepEqual(failure.report.fields[3], { name: 'extra_note', value: 'stored as written' });
  assert.equal(failure.refs, '기록 식별자 · 작업 주체 existence:X1 · 이전 Chain chain:C1 · 시작 세계 snapshot:A1 · 종료 세계 snapshot:A9 · 여정 journey:X1@J2');
  assert.equal(JSON.stringify(view), before, 'reading a Chain writes nothing');
});

test('owned Cycles are joined to the timeline; a Cycle the timeline lacks is named as unavailable, not replaced', () => {
  const view = several();
  view.chains[0].cycle_refs.push('cycle:C77');
  const detail = chainDetail(view, 'chain:C1');
  assert.equal(detail.cyclesHead, '이 Chain의 사이클 3개');
  assert.deepEqual(detail.cycles.map((one) => [one.ref, one.known, one.label]), [
    ['cycle:C1', true, '인터뷰 C1 · 닫힘 · 성공 (success)'],
    ['cycle:C2', true, '실험 C2 · 닫힘 · 성공 (success)'],
    ['cycle:C77', false, 'cycle:C77 · 기록 조회 불가']]);
  const current = chainDetail(view, 'chain:C3');
  assert.equal(current.cycles[1].label, '실험 C5 · 열림 · 현재 위치');
  assert.equal(current.cycles[1].isHere, true);
  assert.equal(current.note, '지금 작업 중인 Chain의 보고서와 사이클 목록입니다. 사이클을 누르면 그 보고서를 봅니다.');
});

test('a closed Chain whose record holds no Report says so instead of inventing one; unknown Chains are null', () => {
  const view = several();
  delete view.chains[0].report;
  assert.equal(chainListEntries(view)[0].label, 'Chain C1 · 닫힘 · 보고서 없음');
  const detail = chainDetail(view, 'chain:C1');
  assert.equal(detail.report, null);
  assert.equal(detail.noReport, '닫혔지만 기록에 Chain 보고서가 없습니다.');
  assert.equal(chainDetail(view, 'chain:C9'), null);
  assert.equal(chainDetail(view, ''), null);
});

test('a current.chain_ref missing from the list is reported, not patched over', () => {
  const view = several();
  view.current.chain_ref = 'chain:C8'; delete view.chain;
  assert.equal(chainModel(view).current, null);
  assert.equal(chainLine(view), 'Chain C8 · Chain 목록에 없습니다');
  assert.equal(chainContrast(view), '');
});

test('unknown verdict values are shown as stored; every language has a non-empty Chain line', () => {
  assert.equal(verdictText('odd'), 'odd');
  assert.equal(verdictText(null), '');
  const view = several();
  view.chains[0].report.verdict = 'maybe';
  assert.equal(chainListEntries(view)[0].label, 'Chain C1 · 닫힘 · maybe');
  for (const locale of LOCALES) {
    setLocale(locale);
    assert.ok(chainLine(view).includes('C3'), locale);
    assert.ok(chainDetail(view, 'chain:C2').head.includes('C2'), locale);
  }
  setLocale('ko');
});
