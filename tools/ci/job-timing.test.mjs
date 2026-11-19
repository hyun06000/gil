import test from 'node:test';
import assert from 'node:assert/strict';
import {timing} from './job-timing.mjs';

const at = (s, text) => `2026-11-19T12:00:${String(s).padStart(2, '0')}.0000000Z ${text}`;
// A synthetic job: setup, cache restore, one cargo test, one cargo build, a Node check, the
// dependency prune, then the cache save (archive until the upload starts) and checkout cleanup.
const log = '﻿' + [
  at(0, 'Current runner version'),
  at(1, '##[group]Run actions/checkout@abc'),
  at(3, '##[group]Run actions/cache@def'),
  at(5, 'Cache restored from key: x'),
  at(6, '##[group]Run cargo test --locked -p gil --features chain-core --test chains'),
  at(16, '    Finished `test` profile [unoptimized + debuginfo] target(s) in 9.50s'),
  at(17, 'test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.25s'),
  at(20, '##[group]Run cargo build --locked -p gil --all-targets --features chain-core'),
  at(30, '    Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.75s'),
  at(31, '##[group]Run node --test tools/ci/*.test.mjs'),
  at(33, '##[group]Run node tools/ci/prune-workspace-cache.mjs'),
  at(35, 'Post job cleanup.'),
  at(45, 'Sent 1048576 of 2097152 (50.0%), 1.0 MBs/sec'),
  at(46, 'Cache saved with key: y'),
  at(47, 'Post job cleanup.'),
  at(48, 'Cleaning up orphan processes'),
].join('\r\n');

test('categories split the log wall time without overlap', () => {
  const r = timing(log);
  assert.equal(r.log_wall, 48);
  assert.deepEqual(r.categories, {setup: 3, cache_restore: 3, cargo_compile: 21, test_run: 4, node_checks: 2, cache_prune: 2, cache_archive: 9, cache_upload: 3, post_other: 1});
  assert.equal(Object.values(r.categories).reduce((a, b) => a + b, 0), r.log_wall);
});

test('tool-reported Cargo and libtest durations are kept apart from wall time', () => {
  const r = timing(log);
  assert.deepEqual(r.reported, {cargo_build: 19.3, libtest: 4.3});
  assert.deepEqual(r.steps.map(s => s.kind), ['setup', 'cache_restore', 'cargo', 'cargo', 'node_checks', 'cache_prune', 'cache_save', 'post_other']);
  assert.deepEqual(r.steps[3].cargo_finished, ['9.75s']);
});

test('a file without job timestamps is refused', () => {
  assert.throws(() => timing('not a log\n'), /no timestamped lines/);
});
