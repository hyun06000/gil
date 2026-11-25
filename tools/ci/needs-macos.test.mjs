import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {join} from 'node:path';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {decide, suitesOf, JOBS, OS_CONDITION} from './needs-macos.mjs';

const root = fileURLToPath(new URL('../../', import.meta.url));
const tool = join(root, 'tools/ci/needs-macos.mjs');
const workflow = name => readFileSync(join(root, `.github/workflows/${name}.yml`), 'utf8').replace(/\r\n/g, '\n');
const NEUTRAL = 'pub fn value() -> u8 { 1 }\n', CONDITIONAL = '#[cfg(unix)]\npub fn value() -> u8 { 1 }\n';
const files = map => path => map[path] ?? null;

test('a change of OS-neutral source alone does not need the macOS job; an OS condition, now or before, does', () => {
  const suites = ['tracking_start'];
  const run = (changed, now, old = {}) => decide('product', changed, {current: files(now), before: files(old), suites});
  assert.equal(run(['src/a.rs', 'README.md', 'spec/x.md'], {'src/a.rs': NEUTRAL}), null);
  assert.match(run(['src/a.rs'], {'src/a.rs': CONDITIONAL}).reason, /with an OS condition/);
  assert.match(run(['src/a.rs'], {'src/a.rs': NEUTRAL}, {'src/a.rs': CONDITIONAL}).reason, /had an OS condition before/);
  assert.match(run(['src/gone.rs'], {}).reason, /removed or renamed/);
  for (const text of ['#[cfg(all(test, unix))]', 'if cfg!(target_os = "macos") {', '#[cfg(not(windows))]', '#[cfg(any(\n    target_family = "unix"))]'])
    assert.match(text, OS_CONDITION, text);
  for (const text of ['#[cfg(test)]', '#[cfg(feature = "tracking-policy")]', '// unix only']) assert.doesNotMatch(text, OS_CONDITION, text);
});

test('the inputs of a job always need it: its suites, shared helpers, build inputs, packaging and its own workflow', () => {
  const run = (name, path) => decide(name, [path], {current: () => NEUTRAL, suites: ['tracking_start']});
  for (const path of ['tests/tracking_start.rs', 'tests/common/mod.rs', 'tests/fixtures/a.json', 'Cargo.lock', 'Cargo.toml',
    'tools/ci/needs-macos.mjs', 'tools/ci/expect-tests.mjs', 'tools/ci/tracking-start/expect-test-groups.mjs', 'mcp-app/build.mjs'])
    for (const name of Object.keys(JOBS)) assert.ok(run(name, path), `${name}: ${path}`);
  assert.ok(run('product', 'distribution/product/build.mjs') && run('product', 'plugins/gil-companion-prototype/core.mjs'));
  assert.ok(run('product', '.github/workflows/product-candidate.yml') && !run('product', '.github/workflows/tracking-start-macos.yml'));
  assert.ok(run('first-start', 'ui/layout.mjs') && run('first-start', 'plugins/gil-companion-prototype/assets/monitor.html'));
  assert.ok(run('first-start', '.github/workflows/tracking-start-macos.yml') && !run('first-start', 'distribution/product/build.mjs'));
  // A suite the job does not run, and a file that only starts like a listed one, do not count.
  assert.equal(run('product', 'tests/other.rs'), null);
  assert.equal(run('product', 'Cargo.toml.bak'), null);
});

test('the suites are read from the job itself, and each such job is gated by the decision and nothing else', () => {
  assert.deepEqual(suitesOf('first-start'), ['tracking_scope_change', 'tracking_start']);
  for (const suite of ['unified_start', 'tracking_start', 'tracking_scope_change', 'exploration_flow', 'visual_start', 'mcp', 'approved_plan_wire_wave1'])
    assert.ok(suitesOf('product').includes(suite), suite);
  for (const [name, {workflow: file, job}] of Object.entries(JOBS)) {
    const text = workflow(file);
    assert.ok(text.includes(`  ${job}:\n    needs: macos-scope\n    if: needs.macos-scope.outputs.run == 'true'\n    runs-on: macos-15\n`), file);
    const scope = text.slice(text.indexOf('\n  macos-scope:\n'), text.indexOf(`\n  ${job}:\n`));
    assert.match(scope, /^    runs-on: ubuntu-24\.04$/m);
    assert.match(scope, /^          fetch-depth: 0$/m);
    // Anything but a pull request (a manual run) runs the job; a pull request is judged from its own diff.
    assert.ok(scope.includes(`if [ "$EVENT" != pull_request ]; then echo 'run=true' >> "$GITHUB_OUTPUT"; exit 0; fi\n`));
    assert.ok(scope.includes(`node tools/ci/needs-macos.mjs ${name} "$base" < "$RUNNER_TEMP/changed.txt" | tee -a "$GITHUB_OUTPUT"\n`));
    assert.equal((text.match(/needs-macos\.mjs/g) || []).length, 2, 'one comment and one call');
  }
  // Every change the macOS jobs can skip is checked on Linux: the product job has its Linux twin in
  // the same workflow, the first-start profiles have their rows in the Chain matrix.
  assert.match(workflow('product-candidate'), /\n  linux-x64-product:\n    runs-on: ubuntu-24\.04\n/);
  const chain = workflow('chain-core');
  for (const features of ['tracking-policy', 'tracking-policy,approved-plan,visual-authoring,product-safety'])
    assert.ok(chain.includes(`            features: ${features}\n            lib: all\n            minimum: 10\n            suites: tracking_start tracking_scope_change\n`), features);
});

test('the tool prints one output line; no changed path or an unknown job never skips silently', () => {
  const run = (args, input) => spawnSync(process.execPath, [tool, ...args], {input, encoding: 'utf8'});
  const neutral = run(['product'], 'README.md\nspec/GIL_Roadmap.md\n');
  assert.equal(neutral.status, 0, neutral.stderr);
  assert.equal(neutral.stdout, 'run=false\n');
  const sensitive = run(['product'], 'README.md\nsrc/lock.rs\n');
  assert.equal(sensitive.stdout, 'run=true\n');
  assert.match(sensitive.stderr, /src\/lock\.rs is a source file with an OS condition/);
  assert.equal(run(['first-start'], '').stdout, 'run=true\n');
  const unknown = run(['other'], 'README.md\n');
  assert.equal(unknown.status, 2);
  assert.equal(unknown.stdout, '');
});
