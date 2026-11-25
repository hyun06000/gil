import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {join} from 'node:path';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {tally, shortfall} from './expect-tests.mjs';

const root = fileURLToPath(new URL('../../', import.meta.url));
const guard = join(root, 'tools/ci/expect-tests.mjs');
const summary = (passed, failed = 0, ignored = 0) =>
  `test result: ${failed ? 'FAILED' : 'ok'}. ${passed} passed; ${failed} failed; ${ignored} ignored; 0 measured; 0 filtered out; finished in 0.01s\n`;
/// Runs the guard around a child that prints `output` and exits with `status`.
const guarded = (minimum, output, status = 0) => spawnSync(process.execPath,
  [guard, String(minimum), process.execPath, '-e', `process.stdout.write(${JSON.stringify(output)}); process.exit(${status})`], {encoding: 'utf8'});

test('a suite compiled away by its gate is not a pass', () => {
  const excluded = `\nrunning 0 tests\n\n${summary(0)}`;
  assert.deepEqual(tally(excluded), {suites: 1, passed: 0, failed: 0, ignored: 0});
  assert.match(shortfall(4, 0, tally(excluded)), /0 test\(s\) passed where at least 4 must run/);
  const ran = guarded(4, excluded);
  assert.equal(ran.status, 1, ran.stderr);
  assert.match(ran.stderr, /not counted as a pass/);
  assert.ok(ran.stdout.includes('running 0 tests'), 'the command output is passed through');
});

test('summaries are added across suites and the minimum is a lower bound', () => {
  const two = `${summary(4)}${summary(3, 0, 1)}`;
  assert.deepEqual(tally(two), {suites: 2, passed: 7, failed: 0, ignored: 1});
  assert.equal(shortfall(7, 0, tally(two)), null);
  assert.equal(guarded(4, two).status, 0);
  assert.match(guarded(4, two).stdout, /expect-tests: 7 passed in 2 suite\(s\), 1 ignored \(minimum 4\)/);
  assert.equal(guarded(8, two).status, 1);
});

test('a failing command keeps its failure and no summary is not a pass', () => {
  const failed = guarded(1, summary(3, 1), 101);
  assert.equal(failed.status, 101, failed.stderr);
  assert.equal(guarded(1, 'compiled, nothing ran\n').status, 1);
  assert.match(shortfall(1, 0, tally(summary(3, 1))), /1 test\(s\) failed/);
  assert.equal(spawnSync(process.execPath, [guard, '0', 'cargo'], {encoding: 'utf8'}).status, 2, 'a minimum of zero would defeat the guard');
});

test('the profile-only exploration suites run behind the guard in the Linux row of their profile', () => {
  const chain = readFileSync(join(root, '.github/workflows/chain-core.yml'), 'utf8').replace(/\r\n/g, '\n');
  const row = chain.match(/^          - group: exploration\n            features: exploration-flow,visual-authoring,product-safety\n            lib: all\n            minimum: (\d+)\n            suites: ([\w ]+)\n/m);
  assert.ok(row, 'the exploration profile has its Linux row');
  for (const suite of ['exploration_flow', 'exploration_tools', 'exploration_evidence', 'exploration_plan_panel', 'closing_handoff']) assert.ok(row[2].split(' ').includes(suite), suite);
  // 4 + 4 + 6 + 1 + 5: the minimums the separate macOS steps used to carry.
  assert.ok(Number(row[1]) >= 20);
  assert.match(chain, /run: \|\n(?: {10}.*\n)*? {10}node tools\/ci\/expect-tests\.mjs "\$MINIMUM" cargo test --locked -p gil --no-fail-fast --features "\$FEATURES" "\$\{tests\[@\]\}"\n/);
});

test('the guard keeps a long output whole and does not wait for a process the command left behind', () => {
  // 20,000 lines exceed a pipe buffer; the background sleep outlives the command and keeps the
  // command's stdout open (its stderr is closed, or this test's own stderr pipe would wait for it).
  const script = 'i=0; while [ $i -lt 20000 ]; do echo "line $i of a long libtest listing"; i=$((i+1)); done; echo "test result: ok. 5 passed; 0 failed; 0 ignored;"; (sleep 30 2>/dev/null &)';
  const started = Date.now();
  const ran = spawnSync(process.execPath, [guard, '3', 'sh', '-c', script], {encoding: 'utf8', maxBuffer: 64 * 1024 * 1024});
  assert.equal(ran.status, 0, ran.stderr);
  assert.ok(Date.now() - started < 20000, 'the guard returned when the command did, not when its child did');
  const lines = ran.stdout.trimEnd().split('\n');
  assert.equal(lines.length, 20002);
  assert.equal(lines[19999], 'line 19999 of a long libtest listing');
  assert.equal(lines.at(-1), 'expect-tests: 5 passed in 1 suite(s), 0 ignored (minimum 3)');
});
