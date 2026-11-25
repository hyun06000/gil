#!/usr/bin/env node
// Decides whether a pull request needs a macOS job that also has a Linux counterpart
// (tools/ci/CHAIN-CORE-TIMING.md §12). A change of OS-neutral logic is checked on Linux; the macOS
// job runs when a changed file can behave differently on macOS or is an input of that job itself.
//
//   git diff --name-only <base> <head> | node tools/ci/needs-macos.mjs <job> [<base>]
//
// Prints `run=true` or `run=false` (the GitHub output line) and the reason on stderr. With <base>,
// a source file that carried an OS condition before the change also counts, so removing the last
// condition from a file still runs the job. Anything it cannot judge runs the job.
import {readFileSync, existsSync} from 'node:fs';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';

const root = fileURLToPath(new URL('../../', import.meta.url));

/// A cfg on unix, windows, target_os or target_family (attribute or macro form, also nested).
export const OS_CONDITION = /cfg!?\((?:all\(|any\(|not\()*[^)]*\b(?:unix|windows|target_os|target_family)\b/;

/// What every such job is built from, whatever the change is about.
const SHARED = ['tests/common/', 'tests/support_wave1/', 'tests/guidance_support/', 'tests/fixtures/', 'Cargo.toml', 'Cargo.lock',
  'tools/ci/needs-macos.mjs', 'tools/ci/expect-tests.mjs', 'tools/ci/tracking-start/'];

/// Per job: the workflow that defines it, the job's id there, and its own inputs besides the sources.
export const JOBS = {
  product: {workflow: 'product-candidate', job: 'macos-arm64-product',
    inputs: ['mcp-app/', 'plugins/gil-companion-prototype/', 'distribution/product/', 'distribution/codex/', 'distribution/compliance/']},
  'first-start': {workflow: 'tracking-start-macos', job: 'first-start-macos',
    inputs: ['mcp-app/', 'ui/', 'plugins/gil-companion-prototype/assets/']},
};

/// The integration suites a job runs, read from the Cargo test commands of its workflow text (`--test <suite>`).
export function suitesOf(name, read = file => readFileSync(join(root, file), 'utf8')) {
  const {workflow, job} = JOBS[name];
  const text = read(`.github/workflows/${workflow}.yml`).replace(/\r\n/g, '\n');
  const block = text.slice(text.indexOf('\njobs:\n')).split(/\n(?=  [\w-]+:\n)/).find(b => b.startsWith(`  ${job}:\n`));
  if (!block) throw new Error(`${workflow}.yml has no job ${job}`);
  return [...new Set([...block.matchAll(/cargo test [^\n]*/g)].flatMap(m => [...m[0].matchAll(/--test (\w+)/g)].map(s => s[1])))].sort();
}

/// The first changed file that needs the macOS job, with the reason; null when none does.
/// `current(path)` and `before(path)` return the file's text now and before the change (null: absent).
export function decide(name, changed, {current, before = () => null, suites}) {
  const {workflow, inputs} = JOBS[name];
  const prefixes = [...SHARED, ...inputs, `.github/workflows/${workflow}.yml`, ...suites.map(s => `tests/${s}.rs`)];
  for (const path of changed) {
    const prefix = prefixes.find(p => p.endsWith('/') ? path.startsWith(p) : path === p);
    if (prefix) return {path, reason: `an input of the job (${prefix})`};
    if (!path.startsWith('src/')) continue;
    const now = current(path);
    if (now === null) return {path, reason: 'a source file that was removed or renamed'};
    if (OS_CONDITION.test(now)) return {path, reason: 'a source file with an OS condition'};
    const old = before(path);
    if (old !== null && OS_CONDITION.test(old)) return {path, reason: 'a source file that had an OS condition before the change'};
  }
  return null;
}

function main([name, base]) {
  if (!JOBS[name]) { console.error(`usage: needs-macos.mjs <${Object.keys(JOBS).join('|')}> [<base>] (changed paths on stdin)`); return 2; }
  const changed = readFileSync(0, 'utf8').split('\n').map(l => l.trim()).filter(Boolean);
  // No changed path at all is not a judgement that nothing changed: run the job.
  if (changed.length === 0) { console.error('no changed path was given: the macOS job runs'); console.log('run=true'); return 0; }
  const current = path => existsSync(join(root, path)) ? readFileSync(join(root, path), 'utf8') : null;
  const before = !base ? () => null : path => {
    const shown = spawnSync('git', ['show', `${base}:${path}`], {cwd: root, encoding: 'utf8', maxBuffer: 64 << 20});
    return shown.status === 0 ? shown.stdout : null;
  };
  const found = decide(name, changed, {current, before, suites: suitesOf(name)});
  console.error(found ? `${name}: the macOS job runs: ${found.path} is ${found.reason}`
    : `${name}: none of the ${changed.length} changed files can behave differently on macOS or is an input of the job; Linux checks this change`);
  console.log(`run=${found ? 'true' : 'false'}`);
  return 0;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) process.exitCode = main(process.argv.slice(2));
