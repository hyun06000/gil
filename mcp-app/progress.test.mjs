import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { progressTracker, progressSummary } from '../ui/progress.mjs';
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
