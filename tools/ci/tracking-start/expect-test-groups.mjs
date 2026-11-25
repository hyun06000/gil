#!/usr/bin/env node
// Run a libtest command and count it as a pass only when every named group of tests ran exactly as
// expected — not when the run as a whole passed enough tests.
//
//   node tools/ci/tracking-start/expect-test-groups.mjs <prefix>=<count> [<prefix>=<count> ...] -- <command> [args...]
//
// A group is every reported test whose full name starts with <prefix> (`*` = every test this command
// reported). Each group must report exactly <count> tests, all `ok`: a group that a feature or
// platform gate compiled away, a missing or extra test, an ignored test or a failure is not a pass.
// The run must also exit 0 and print a libtest summary, and the per-test lines must add up to the
// summaries, so a truncated or interleaved log is not read as complete. The command's stdout is
// passed through unchanged. This only reads output; it never decides which tests run.
import {spawnSync} from 'node:child_process';
import {pathToFileURL} from 'node:url';
import {tally, shortfall} from '../expect-tests.mjs';

/// `<prefix>=<count>` arguments as [{prefix, count}]. Overlapping prefixes would count a test twice.
export function parseGroups(args) {
  const groups = args.map((arg) => {
    const at = arg.lastIndexOf('=');
    const prefix = arg.slice(0, at), count = Number(arg.slice(at + 1));
    if (at < 1 || !Number.isInteger(count) || count < 1) throw new Error(`bad group ${JSON.stringify(arg)}: want <prefix>=<count >= 1>`);
    return {prefix, count};
  });
  if (groups.length === 0) throw new Error('at least one group is required');
  for (const a of groups) for (const b of groups) {
    if (a !== b && (a.prefix === '*' || b.prefix === '*' || a.prefix.startsWith(b.prefix))) {
      throw new Error(`groups ${a.prefix} and ${b.prefix} overlap`);
    }
  }
  return groups;
}

/// Every `test <name> ... <result>` line libtest printed, in order. Result is ok, FAILED or ignored.
export function outcomes(output) {
  return [...output.matchAll(/^test (\S+) \.\.\. (ok|FAILED|ignored)\b.*$/gm)].map((m) => ({name: m[1], result: m[2]}));
}

/// The problems that keep this run from counting, and a per-group report. No problems = a pass.
export function judge(groups, status, output) {
  const problems = [];
  const total = tally(output);
  const base = shortfall(1, status, total);
  if (base) problems.push(base);
  const seen = outcomes(output);
  if (seen.length !== total.passed + total.failed + total.ignored) {
    problems.push(`${seen.length} per-test line(s) but the summaries count ${total.passed + total.failed + total.ignored}`);
  }
  const names = new Set();
  for (const t of seen) {
    if (names.has(t.name)) problems.push(`${t.name} was reported more than once`);
    names.add(t.name);
  }
  const report = groups.map(({prefix, count}) => {
    const members = seen.filter((t) => prefix === '*' || t.name.startsWith(prefix));
    const ok = members.filter((t) => t.result === 'ok');
    const failed = members.filter((t) => t.result === 'FAILED').map((t) => t.name);
    const ignored = members.filter((t) => t.result === 'ignored').map((t) => t.name);
    if (members.length !== count) problems.push(`${prefix}: ${members.length} test(s) reported where exactly ${count} must run (a gate, filter or rename may have changed the set)`);
    if (failed.length) problems.push(`${prefix}: failed ${failed.join(', ')}`);
    if (ignored.length) problems.push(`${prefix}: ignored ${ignored.join(', ')}`);
    return {prefix, expected: count, reported: members.length, ok: ok.length, names: ok.map((t) => t.name)};
  });
  return {problems, report, total};
}

function main(argv) {
  const split = argv.indexOf('--');
  if (split < 0 || split === argv.length - 1) {
    console.error('usage: node tools/ci/tracking-start/expect-test-groups.mjs <prefix>=<count> ... -- <command> [args...]');
    return 2;
  }
  let groups;
  try { groups = parseGroups(argv.slice(0, split)); } catch (e) { console.error(`expect-test-groups: ${e.message}`); return 2; }
  const command = argv.slice(split + 1);
  const ran = spawnSync(command[0], command.slice(1), {stdio: ['inherit', 'pipe', 'inherit'], encoding: 'utf8', maxBuffer: 256 * 1024 * 1024});
  process.stdout.write(ran.stdout ?? '');
  if (ran.error) {
    console.error(`expect-test-groups: could not run ${command[0]}: ${ran.error.message}`);
    return 1;
  }
  const {problems, report, total} = judge(groups, ran.status, ran.stdout ?? '');
  for (const g of report) {
    console.log(`expect-test-groups: group ${g.prefix}: ${g.ok} ok of ${g.reported} reported, exactly ${g.expected} required`);
    for (const name of g.names) console.log(`  ok ${name}`);
  }
  if (problems.length) {
    for (const p of problems) console.error(`expect-test-groups: not counted as a pass: ${p}`);
    return ran.status || 1;
  }
  console.log(`expect-test-groups: all ${report.length} group(s) exact; run total ${total.passed} passed, ${total.ignored} ignored in ${total.suites} suite(s)`);
  return 0;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  process.exit(main(process.argv.slice(2)));
}
