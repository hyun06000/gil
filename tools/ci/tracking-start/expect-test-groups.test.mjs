// Contracts of the per-group libtest gate (pure Node, synthetic libtest text; no Core process).
import test from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {parseGroups, outcomes, judge} from './expect-test-groups.mjs';

const gate = fileURLToPath(new URL('./expect-test-groups.mjs', import.meta.url));
const line = (name, result = 'ok') => `test ${name} ... ${result}`;
const summary = (passed, failed = 0, ignored = 0) =>
  `test result: ${failed ? 'FAILED' : 'ok'}. ${passed} passed; ${failed} failed; ${ignored} ignored; 0 measured; 9 filtered out; finished in 0.10s`;
const run = (...lines) => lines.join('\n') + '\n';
const groups = parseGroups(['a::tests::=2', 'b::tests::=1']);

test('exact groups that all pass are a pass and name every test', () => {
  const out = run(line('a::tests::one'), line('a::tests::two'), line('b::tests::three'), summary(3));
  const {problems, report} = judge(groups, 0, out);
  assert.deepEqual(problems, []);
  assert.deepEqual(report.map((g) => g.names), [['a::tests::one', 'a::tests::two'], ['b::tests::three']]);
});

test('a missing, extra, ignored or failed group member is not a pass even when the total is large', () => {
  const missing = run(line('a::tests::one'), line('b::tests::three'), line('c::other'), summary(3));
  assert.match(judge(groups, 0, missing).problems.join('\n'), /a::tests::: 1 test\(s\) reported where exactly 2/);
  const extra = run(line('a::tests::one'), line('a::tests::two'), line('a::tests::new'), line('b::tests::three'), summary(4));
  assert.match(judge(groups, 0, extra).problems.join('\n'), /a::tests::: 3 test\(s\) reported/);
  const ignored = run(line('a::tests::one'), line('a::tests::two', 'ignored'), line('b::tests::three'), summary(2, 0, 1));
  assert.match(judge(groups, 0, ignored).problems.join('\n'), /ignored a::tests::two/);
  const failed = run(line('a::tests::one'), line('a::tests::two', 'FAILED'), line('b::tests::three'), summary(2, 1));
  assert.match(judge(groups, 101, failed).problems.join('\n'), /failed a::tests::two/);
});

test('zero tests, no summary, a failing exit or lines that do not add up are not a pass', () => {
  assert.ok(judge(groups, 0, run('running 0 tests', summary(0))).problems.length > 0);
  assert.ok(judge(groups, 0, run(line('a::tests::one'), line('a::tests::two'), line('b::tests::three'))).problems.some((p) => /no test summary/.test(p)));
  const good = run(line('a::tests::one'), line('a::tests::two'), line('b::tests::three'), summary(3));
  assert.ok(judge(groups, 1, good).problems.some((p) => /exited with 1/.test(p)));
  const truncated = run(line('a::tests::one'), line('a::tests::two'), line('b::tests::three'), summary(5));
  assert.ok(judge(groups, 0, truncated).problems.some((p) => /per-test line/.test(p)));
  const twice = run(line('a::tests::one'), line('a::tests::one'), line('b::tests::three'), summary(3));
  assert.ok(judge(groups, 0, twice).problems.some((p) => /more than once/.test(p)));
});

test('the * group is every reported test, and bad or overlapping groups are refused', () => {
  const all = parseGroups(['*=2']);
  assert.deepEqual(judge(all, 0, run(line('x'), line('y'), summary(2))).problems, []);
  assert.ok(judge(all, 0, run(line('x'), summary(1))).problems.length > 0);
  for (const bad of [[], ['a='], ['=2'], ['a=0'], ['a=1.5'], ['a::=1', 'a::b::=1'], ['*=1', 'a=1']]) {
    assert.throws(() => parseGroups(bad), bad.join(' '));
  }
  assert.deepEqual(outcomes('test z ... ignored, slow\n'), [{name: 'z', result: 'ignored'}]);
});

test('the command line wrapper keeps the command output and fails on a shortfall', () => {
  const echo = (text) => [process.execPath, '-e', `process.stdout.write(${JSON.stringify(text)})`];
  const pass = spawnSync(process.execPath, [gate, '*=1', '--', ...echo(run(line('only'), summary(1)))], {encoding: 'utf8'});
  assert.equal(pass.status, 0, pass.stderr);
  assert.match(pass.stdout, /test only \.\.\. ok/);
  assert.match(pass.stdout, /ok only/);
  const short = spawnSync(process.execPath, [gate, '*=2', '--', ...echo(run(line('only'), summary(1)))], {encoding: 'utf8'});
  assert.notEqual(short.status, 0);
  assert.match(short.stderr, /exactly 2 must run/);
  assert.equal(spawnSync(process.execPath, [gate, '*=1'], {encoding: 'utf8'}).status, 2);
});
