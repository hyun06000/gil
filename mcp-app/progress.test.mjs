import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { progressTracker, progressSummary, visualCounts } from '../ui/progress.mjs';
// These assertions read the Korean table; the language-specific cases live in locale.test.mjs.
import { setLocale } from '../ui/locale.js';
setLocale('ko');
const fixture = () => JSON.parse(readFileSync(new URL('../ui/fixtures/first-interview/view.json', import.meta.url)));

test('polls and Artifact changes do not impersonate GIL record progress; scopes stay isolated', () => {
  const observe = progressTracker(), view = fixture();
  assert.equal(observe('a', view, 10).changed, false);
  view.captured_at_unix_ms = 500; view.world.state = 'clean';
  assert.deepEqual(observe('a', view, 20), observe('a', view, 30));
  assert.equal(observe('a', view, 40).observedAt, 10);
  view.timeline[0].steps[0].state = 'closed';
  const next = observe('a', view, 50);
  assert.equal(next.changed, true); assert.equal(next.observedAt, 50);
  assert.equal(observe('b', view, 60).changed, false);
  assert.equal(observe('a', view, 70).observedAt, 50);
});

test('open contract, completion criterion and closed count come from the View, not a timer', () => {
  const view = fixture(), summary = progressSummary(view);
  assert.equal(summary.action, view.current_will.next_action);
  assert.equal(summary.done, view.current_will.done_when);
  assert.match(summary.position, /질문.*열려 있음/);
  assert.match(summary.records, /닫힌 단계 0개/);
  view.current_will = null; view.current.step_ref = null;
  view.timeline[0].steps[0].state = 'closed';
  assert.match(progressSummary(view).records, /닫힌 단계 1개/);
  assert.match(progressSummary(view).action, /열린 행동 계약이 없습니다/);
});

test('Chain closure is visible progress even when Cycle, Step and Journey do not change', () => {
  const view = fixture(), observe = progressTracker();
  view.current.chain_ref = 'chain:C1';
  view.chain = {state: 'open'};
  view.current_will = null;
  observe('chain', view, 10);
  view.chain = {state: 'closed', report: {verdict: 'failure', handoff_summary: '남은 질문을 보존했다'}};
  assert.equal(observe('chain', view, 20).observedAt, 20);
  const summary = progressSummary(view);
  assert.equal(summary.position, 'Chain 종료');
  assert.equal(summary.done, '남은 질문을 보존했다');
  assert.match(summary.records, /닫힘 \(failure\)/);
  assert.match(summary.action, /최종 판정: failure/);
});

test('visual counts come only from recorded declarations; none-declared and missing are listed apart', () => {
  const view = fixture();
  const cycle = view.timeline.find(c => c.cycle_ref === view.current.cycle_ref);
  assert.match(progressSummary(view).visuals, /시각 자료 기록 없음/);
  cycle.steps[0].visuals = { state: 'attached', count: 2 };
  view.timeline.push({ cycle_ref: 'cycle:C9', kind: 'experiment', state: 'closed', relation_to_current: 'ancestor',
    parent_cycle_ref: null, revisit_from_cycle_ref: null, experiment_definition: null, interview_question: null, report: null,
    steps: [{ step_ref: 'step:C9/S3', kind: 'verify', state: 'closed', summary: null, visuals: { state: 'not_needed', count: 0 } },
      { step_ref: 'step:C9/S4', kind: 'verify', state: 'closed', summary: null, visuals: { state: 'missing', count: 1 } },
      { step_ref: 'step:C9/S5', kind: 'analysis', state: 'closed', summary: null }] });
  assert.deepEqual(visualCounts(view), { total: 3, here: 2, declaredNone: 1, missing: 1 });
  assert.match(progressSummary(view).visuals, /시각 자료 3개 \(이 Cycle 2개\) · 없음 선언 1건 · 미완 1건/);
});


test('zero-image missing declaration remains visible instead of claiming no records', () => {
  const view = fixture();
  view.timeline[0].steps[0].visuals = { state: 'missing', count: 0 };
  assert.match(progressSummary(view).visuals, /미완 1건/);
  assert.doesNotMatch(progressSummary(view).visuals, /기록 없음/);
});
