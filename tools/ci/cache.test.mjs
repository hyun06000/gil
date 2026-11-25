import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtempSync, mkdirSync, readFileSync, readdirSync, existsSync, rmSync, writeFileSync, unlinkSync, symlinkSync, realpathSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';

const root = fileURLToPath(new URL('../../', import.meta.url));
const cleaner = join(root, 'tools/ci/prune-workspace-cache.mjs');
const workflows = ['codex-preview', 'chain-core', 'companion-native', 'windows-spike'];
const workflowText = name => readFileSync(join(root, `.github/workflows/${name}.yml`), 'utf8').replace(/\r\n/g, '\n');

test('dependency caches are stable across source commits; only cache maintenance is conditional', () => {
  for (const name of workflows) {
    const text = workflowText(name);
    const key = text.match(/^          key: (.+)$/m)?.[1];
    assert.ok(key, name);
    assert.match(key, /runner.os.*runner.arch.*rust-1\.97\.1.*Cargo.lock.*package-lock.json.*deps-v2/);
    assert.ok(key.includes(`.github/workflows/${name}.yml`) && key.includes('tools/ci/prune-workspace-cache.mjs'));
    assert.doesNotMatch(key, /github\.sha|head\.sha|run_id/, 'source changes must not upload dependencies again');
    const conditional = text.split('      - ').filter(s => s.includes('if: steps.build_cache.outputs.cache-hit'));
    assert.equal(conditional.length, 1);
    assert.match(conditional[0], /if: steps.build_cache.outputs.cache-hit != 'true'\n        run: node tools\/ci\/prune-workspace-cache.mjs/);
  }
});

/// The workflow-level `env:` of the Chain workflow as {NAME: value}; both Chain jobs inherit it.
const chainJobEnv = () => {
  const block = workflowText('chain-core').match(/\nenv:\n((?: {2}.*\n)+)/)?.[1] ?? '';
  return Object.fromEntries([...block.matchAll(/^ {2}([A-Z0-9_]+): '?([^'\n]*)'?$/gm)].map(m => [m[1], m[2]]));
};

test('Chain CI build-cost settings stay CI-only, never reach release or test semantics, and key the cache', () => {
  const env = chainJobEnv();
  assert.deepEqual(Object.keys(env).sort(), ['CARGO_INCREMENTAL', 'CARGO_PROFILE_DEV_DEBUG', 'RUSTUP_TOOLCHAIN']);
  assert.equal(env.CARGO_PROFILE_DEV_DEBUG, 'line-tables-only');
  assert.equal(env.CARGO_INCREMENTAL, '0');
  const text = workflowText('chain-core');
  // No job or step sets its own environment block of build settings: the workflow-level one is the only source.
  assert.doesNotMatch(text, /^ {4,}(?:CARGO_|RUSTUP_)\w+:/m);
  // Optimisation, assertions, overflow checks, panic strategy, codegen flags and the release
  // profile are never changed by CI, for the job or for a single step.
  assert.doesNotMatch(text, /CARGO_PROFILE_RELEASE|CARGO_PROFILE_\w+_(?:OPT_LEVEL|DEBUG_ASSERTIONS|OVERFLOW_CHECKS|PANIC|CODEGEN_UNITS|LTO|STRIP)|RUSTFLAGS|CARGO_BUILD|cargo [^\n]*(?:--release|--profile)/);
  assert.doesNotMatch(readFileSync(join(root, 'Cargo.toml'), 'utf8'), /^\[profile/m, 'a committed profile would override the measured CI setting');
  // The debuginfo level changes every cached dependency output, so a cache built with another
  // level is never restored, not even by prefix; incremental state is never cached.
  const key = text.match(/^          key: (.+)$/m)[1];
  const prefix = text.match(/^          restore-keys: \|\n {12}(.+)$/m)[1];
  for (const k of [key, prefix]) assert.match(k, /-rust-1\.97\.1-dev-debug-\$\{\{ env\.CARGO_PROFILE_DEV_DEBUG \}\}-\$\{\{ hashFiles\('Cargo\.lock'/);
  assert.ok(key.startsWith(prefix));
});

test('the Windows Core job keeps both x64 profiles, no ARM64 job, and no retired runtime-update steps', () => {
  const text = workflowText('windows-spike');
  assert.equal((text.match(/          - arch: x64/g) || []).length, 2);
  assert.equal((text.match(/          - arch: arm64/g) || []).length, 0);
  const at = name => text.indexOf(`      - name: ${name}`);
  assert.ok(at('Build native Core and MCP') < at('Binding security and native MCP integration'));
  assert.ok(at('Host registration (gil install') > at('Host guard scope'));
  assert.ok(at('Image-report storage boundaries') < at('Native protocol, read-only Monitor'));
  for (const retired of ['Runtime update state judge', 'Runtime App Server integration', 'Isolated verification bundle',
    'Companion sessions and Windows named pipes', 'Prepare unsigned review candidate', 'Automatic installer target detection']) {
    assert.equal(at(retired), -1, retired);
  }
  assert.doesNotMatch(text, /windows-11-arm|codex-cli|runtime-testbed/);
  const chain = workflowText('chain-core');
  // Windows test and build CI is paused: the Chain workflow is two macOS jobs and the Linux trial job.
  const allChainJobs = jobsOf(chain);
  assert.deepEqual(allChainJobs.map(j => j.name), ['chain-product', 'chain-opt-in', 'chain-linux']);
  const chainJobs = allChainJobs.slice(0, 2), linux = allChainJobs[2].block;
  for (const {name, block} of chainJobs) {
    assert.match(block, /^    runs-on: macos-15$/m, name);
    assert.doesNotMatch(block, /^\s+(?:strategy|matrix):/m, name);
  }
  assert.doesNotMatch(chain, /runner: windows-|runs-on: windows-|shell: pwsh/);
  // A guarded step (`node tools/ci/expect-tests.mjs <minimum> cargo test …`) is the same Cargo check.
  const groupsOf = block => {
    const features = [...block.matchAll(/run: (?:node tools\/ci\/expect-tests\.mjs \d+ )?cargo (?:test|build).*?--features ([\w,-]+)/g)].map(m => m[1]);
    return features.filter((f, i) => f !== features[i-1]);
  };
  // Each feature profile is one contiguous group in exactly one job, so no job compiles a profile
  // another job also compiles. The product profiles come first in their job; the M4-R opt-in
  // follows them there and never sits inside a product profile. The other opt-in profiles
  // (exploration contract, the single end history alone and with the visual registry, then its
  // Monitor fixtures in the approval profile) are the second job.
  assert.deepEqual(groupsOf(chainJobs[0].block), ['chain-core', 'chain-core,visual-authoring,product-safety', 'approved-plan,visual-authoring,product-safety', 'approved-plan,visual-authoring,recovery-auto-approval']);
  assert.deepEqual(groupsOf(chainJobs[1].block), ['exploration-flow,visual-authoring,product-safety', 'will-end-history', 'will-end-history,visual-authoring,product-safety', 'will-end-history,approved-plan,visual-authoring,product-safety']);
  // Both jobs prepare the same way and read one dependency cache through the same key and prefix;
  // only the first job saves it (the second uses the restore-only action and never prunes).
  const cacheOf = block => block.match(/^          key: .+\n          restore-keys: \|\n.+$/m)?.[0];
  assert.ok(cacheOf(chainJobs[0].block));
  assert.equal(cacheOf(chainJobs[1].block), cacheOf(chainJobs[0].block));
  const paths = block => block.match(/\n          path: \|\n((?: {12}.*\n)+)/)?.[1];
  assert.equal(paths(chainJobs[1].block), paths(chainJobs[0].block));
  const pin = chainJobs[0].block.match(/uses: actions\/cache@([0-9a-f]{40}) /)?.[1];
  assert.ok(pin && chainJobs[1].block.includes(`uses: actions/cache/restore@${pin} `));
  assert.doesNotMatch(chainJobs[1].block, /uses: actions\/cache@|run: node tools\/ci\/prune-workspace-cache|build_cache/);
  for (const prepare of ['      - uses: actions/checkout@', "          GIT_CONFIG_VALUE_0: 'false'", '          persist-credentials: false', '      - run: rustup toolchain install 1.97.1 --profile minimal'])
    for (const {name, block} of allChainJobs) assert.ok(block.includes(prepare), `${name}: ${prepare}`);
  // The Linux trial job: one row per feature profile of the macOS jobs, in the same order, each
  // profile once. It restores and saves its own (Linux) cache through the same key expression and
  // paths; one row alone prunes and saves. The suites kept on macOS are never listed for Linux.
  assert.match(linux, /^    runs-on: ubuntu-24\.04$/m);
  const rows = [...linux.matchAll(/^          - group: ([\w-]+)\n            features: ([\w,-]+)\n            lib: (\w+)\n            minimum: (\d+)\n            suites: ([\w ]+)\n/gm)];
  const macProfiles = [...groupsOf(chainJobs[0].block), ...groupsOf(chainJobs[1].block)];
  // The Monitor-fixture profile is one suite and rides on the last row as its extra check.
  // The M4-R profile has no Linux row: its only integration suite is compiled on macOS and Windows.
  assert.deepEqual(rows.map(r => r[2]), macProfiles.slice(0, -1).filter(f => !f.includes('recovery-auto-approval')));
  assert.ok(linux.includes(`            extra: --features ${macProfiles.at(-1)} --test chain_monitor_fixtures\n`));
  assert.equal((linux.match(/^          - group: /gm) || []).length, rows.length, 'every row has the full shape');
  for (const [, group, features, lib, minimum, suites] of rows) {
    assert.ok(Number(minimum) >= 1, group);
    for (const suite of suites.split(' ')) {
      assert.ok(existsSync(join(root, 'tests', `${suite}.rs`)), `${group}: ${suite}`);
      assert.ok(!['manual', 'automatic_guard', 'write_guard'].includes(suite), `${group}: ${suite} stays on macOS`);
      // Nothing is invented for Linux: the macOS jobs run the same suite with the same features.
      assert.match(chainJobs.map(j => j.block).join(''), new RegExp(`cargo test [^\\n]*--features ${features} [^\\n]*--test ${suite}\\b`), `${group}: ${suite}`);
    }
    if (lib !== 'none') assert.match(chainJobs.map(j => j.block).join(''), new RegExp(`--features ${features} --lib${lib === 'all' ? ' -- --test-threads=1' : ` ${lib}`}\\n`), group);
  }
  assert.equal(cacheOf(linux), cacheOf(chainJobs[0].block));
  assert.equal((linux.match(/^          key: .+$/gm) || []).filter(k => k === cacheOf(linux).split('\n')[0]).length, 2, 'restore and save use one key');
  assert.ok(linux.includes(`uses: actions/cache/restore@${pin} `) && linux.includes(`uses: actions/cache/save@${pin} `));
  assert.equal((linux.match(/^            save_cache: true$/gm) || []).length, 1);
  assert.equal((linux.match(/^        if: matrix\.save_cache && steps\.linux_cache\.outputs\.cache-hit != 'true'$/gm) || []).length, 2);
  assert.doesNotMatch(linux, /uses: actions\/cache@|build_cache/);
  const companion = workflowText('companion-native');
  assert.doesNotMatch(companion, /windows-2025|windows-11-arm/);
  assert.match(companion, /paths:\n\s+- 'companion\/\*\*'/);
});

/// Each job under `jobs:` as {name, block}, block being the job's own lines.
const jobsOf = text => text.slice(text.indexOf('\njobs:\n') + '\njobs:\n'.length).split(/\n(?=  [\w-]+:\n)/)
  .map(block => ({name: block.match(/^  ([\w-]+):\n/)?.[1], block}));
const PAUSED = '    if: ${{ false }}';

test('Windows test and build CI is paused: no job outside the release workflow can start a Windows runner', () => {
  const windowsRunner = /^\s+(?:runs-on|runner): windows-/m;
  for (const file of readdirSync(join(root, '.github/workflows')).filter(f => f.endsWith('.yml') && f !== 'windows-release.yml')) {
    for (const {name, block} of jobsOf(workflowText(file.slice(0, -4)))) {
      if (windowsRunner.test(block)) assert.ok(block.split('\n').includes(PAUSED), `${file} ${name} would start a Windows runner`);
    }
  }
  // The release workflow is not part of the pause: its definition keeps its tag trigger and runner.
  const release = jobsOf(workflowText('windows-release'));
  assert.ok(release.length > 0 && release.every(({block}) => !/^\s+if: \$\{\{ false \}\}$/m.test(block)));
  assert.match(workflowText('windows-release'), /^    runs-on: windows-2025$/m);
});

test('paused Windows jobs are disabled unconditionally at job level; their triggers and steps stay as inactive text', () => {
  // A disabled job still leaves a skipped record for its trigger; the file text is kept, but nothing in it runs.
  for (const [file, job, triggers, steps] of [
    ['windows-spike', 'windows-native', /\non:\n  pull_request:\n[\s\S]*\n  push:\n[\s\S]*\n  workflow_dispatch:\n/,
      ['Build native Core and MCP for the selected profile', 'Native protocol, read-only Monitor and process restart', 'MSIX packaging, installed CLI and MCP (development identity only)']],
    ['windows-msix', 'package', /\non:\n  workflow_dispatch:\n/,
      ['Build static x64 product and Start-menu guide', 'Verify installed development MSIX through its execution alias', 'Package unsigned Store submission and provenance']],
  ]) {
    const text = workflowText(file);
    const jobs = jobsOf(text);
    assert.deepEqual(jobs.map(j => j.name), [job], file);
    const lines = jobs[0].block.split('\n');
    assert.equal(lines.filter(l => /^    if:/.test(l)).length, 1, file);
    // The condition is the job's own first key (after its comment), a literal false: no event, input or variable enables it.
    assert.equal(lines.slice(1).find(l => !l.startsWith('    #')), PAUSED, file);
    assert.match(text, triggers, file);
    assert.match(jobs[0].block, /^    runs-on: (?:windows-2025|\$\{\{ matrix\.runner \}\})$/m, file);
    for (const step of steps) assert.ok(text.includes(`      - name: ${step}\n`), `${file}: ${step}`);
  }
});

test('the manual capability probe keeps only its macOS row and its manual trigger', () => {
  const probe = workflowText('u2-capability-probe');
  assert.match(probe, /\non:\n  workflow_dispatch:\npermissions:/);
  const rows = probe.match(/\n        include:\n((?: {10}.*\n)+)/)?.[1].split('\n').filter(l => !/^\s*(#|$)/.test(l));
  assert.deepEqual(rows, ['          - label: macos-arm64', '            runner: macos-15', '            arch: arm64']);
  assert.doesNotMatch(probe, /runner: windows-/);
  assert.ok(probe.includes('      - name: Probe tests in isolated temp folders\n'));
  assert.ok(!probe.split('\n').includes(PAUSED), 'the macOS probe stays runnable on demand');
});

// Dev/test build settings a job may set; the default-settings case removes inherited ones (the Chain
// job itself runs this file with its own settings in the environment).
const BUILD_SETTINGS = /^(CARGO_PROFILE_\w+|CARGO_INCREMENTAL)$/;
const defaultBuild = () => Object.fromEntries(Object.entries(process.env).filter(([k]) => !BUILD_SETTINGS.test(k)));

/// The prune contract for one set of build settings: workspace outputs (including incremental
/// state) of the implicit and explicit targets go, dependency outputs stay, unsafe roots are refused.
function pruneContract(t, buildEnv) {
  const folder = realpathSync(mkdtempSync(join(tmpdir(), 'gil-cache-test-')));
  t.after(() => rmSync(folder, {recursive:true, force:true}));
  writeFileSync(join(folder, 'Cargo.toml'), '[workspace]\nmembers=["gil","gil-companion","cache-dep"]\nresolver="2"\n');
  for (const name of ['gil', 'gil-companion', 'cache-dep']) {
    mkdirSync(join(folder, name, 'src'), {recursive:true});
    writeFileSync(join(folder, name, 'Cargo.toml'), `[package]\nname="${name}"\nversion="0.1.0"\nedition="2021"\n` +
      (name === 'cache-dep' ? '' : '[dependencies]\ncache-dep={path="../cache-dep"}\n'));
    writeFileSync(join(folder, name, 'src/lib.rs'), 'pub fn value() -> u8 { 1 }\n');
  }
  const run = (args, extra = {}) => {
    const result = spawnSync('cargo', args, {cwd:folder, encoding:'utf8', env:{...buildEnv, CARGO_TARGET_DIR:join(folder,'target'), ...extra}});
    assert.equal(result.status, 0, result.stderr); return result;
  };
  const version = spawnSync('rustc', ['-vV'], {encoding:'utf8'});
  assert.equal(version.status, 0, version.stderr);
  const triple = version.stdout.match(/^host: (.+)$/m)?.[1];
  assert.ok(triple);
  run(['build', '--offline', '--workspace']);
  const dirs = [join(folder, 'target/debug')];
  if (['aarch64-apple-darwin', 'x86_64-pc-windows-msvc', 'aarch64-pc-windows-msvc'].includes(triple)) {
    run(['build', '--offline', '--locked', '--workspace', '--target', triple]);
    dirs.push(join(folder, 'target', triple, 'debug'));
  }
  unlinkSync(join(folder, 'target/CACHEDIR.TAG')); // A restored partial cache may omit the root tag.
  const env = {...buildEnv, GITHUB_ACTIONS:'true', GITHUB_WORKSPACE:folder};
  const clean = spawnSync(process.execPath, [cleaner], {cwd:folder, env, encoding:'utf8'});
  assert.equal(clean.status, 0, clean.stderr);
  for (const dir of dirs) {
    assert.ok(!existsSync(join(dir, 'libgil.rlib')), dir);
    assert.ok(!existsSync(join(dir, 'libgil_companion.rlib')), dir);
    assert.ok(existsSync(join(dir, 'libcache_dep.rlib')), 'dependency must survive: ' + dir);
    const incremental = join(dir, 'incremental');
    if (existsSync(incremental)) assert.deepEqual(readdirSync(incremental).filter(e => /^gil(_companion)?-/.test(e)), [], incremental);
  }
  const wrong = spawnSync(process.execPath, [cleaner], {cwd:folder, env:{...env,GITHUB_WORKSPACE:tmpdir()}, encoding:'utf8'});
  assert.notEqual(wrong.status, 0, 'wrong workspace must be refused');
  mkdirSync(join(folder, 'outside'));
  const link = join(folder, 'target/plugin-core-build');
  symlinkSync(resolve(folder, 'outside'), link, 'junction');
  const linked = spawnSync(process.execPath, [cleaner], {cwd:folder, env, encoding:'utf8'});
  assert.notEqual(linked.status, 0, 'symlink build root must be refused');
}

test('cache pruning keeps dependencies and removes workspace outputs with default build settings (other workflows)', t => {
  pruneContract(t, defaultBuild());
});

test('cache pruning keeps dependencies and removes workspace outputs with the Chain job build settings', t => {
  const {RUSTUP_TOOLCHAIN: _toolchain, ...chainBuild} = chainJobEnv();
  pruneContract(t, {...defaultBuild(), ...chainBuild});
});
