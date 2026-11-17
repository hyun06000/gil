#!/usr/bin/env node
// Run a test command and refuse to call it a pass unless tests actually ran.
//
//   node tools/ci/expect-tests.mjs <minimum> <command> [args...]
//
// `cargo test --test <name>` exits 0 when a `#![cfg(...)]` gate compiles the whole suite away:
// it prints `running 0 tests` and reports success. For suites that exist only in one feature
// profile, that silent zero would read as "verified". This wrapper passes the command's output
// through unchanged, keeps its exit code when it fails, and additionally fails when fewer than
// <minimum> tests passed or the command printed no libtest summary at all.
import {spawnSync} from 'node:child_process';
import {pathToFileURL} from 'node:url';

/// Sum of the `test result:` summaries in libtest output: {suites, passed, failed, ignored}.
export function tally(output) {
  const total = {suites: 0, passed: 0, failed: 0, ignored: 0};
  for (const line of output.matchAll(/^test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;/gm)) {
    total.suites += 1;
    total.passed += Number(line[1]);
    total.failed += Number(line[2]);
    total.ignored += Number(line[3]);
  }
  return total;
}

/// Why this run does not count, or null when it does.
export function shortfall(minimum, status, total) {
  if (status !== 0) return `the command exited with ${status}`;
  if (total.suites === 0) return 'the command printed no test summary';
  if (total.failed > 0) return `${total.failed} test(s) failed`;
  if (total.passed < minimum) return `${total.passed} test(s) passed where at least ${minimum} must run (a feature or platform gate may have excluded the suite)`;
  return null;
}

function main(argv) {
  const minimum = Number(argv[0]);
  const command = argv.slice(1);
  if (!Number.isInteger(minimum) || minimum < 1 || command.length === 0) {
    console.error('usage: node tools/ci/expect-tests.mjs <minimum >= 1> <command> [args...]');
    return 2;
  }
  const ran = spawnSync(command[0], command.slice(1), {stdio: ['inherit', 'pipe', 'inherit'], encoding: 'utf8', maxBuffer: 256 * 1024 * 1024});
  process.stdout.write(ran.stdout ?? '');
  if (ran.error) {
    console.error(`expect-tests: could not run ${command[0]}: ${ran.error.message}`);
    return 1;
  }
  const total = tally(ran.stdout ?? '');
  const problem = shortfall(minimum, ran.status, total);
  if (problem) {
    console.error(`expect-tests: not counted as a pass: ${problem}`);
    return ran.status || 1;
  }
  console.log(`expect-tests: ${total.passed} passed in ${total.suites} suite(s), ${total.ignored} ignored (minimum ${minimum})`);
  return 0;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  process.exit(main(process.argv.slice(2)));
}
