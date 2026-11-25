import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtempSync, mkdirSync, readFileSync, readdirSync, existsSync, rmSync, writeFileSync, unlinkSync, symlinkSync, realpathSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';

const root = fileURLToPath(new URL('../../', import.meta.url));
const cleaner = join(root, 'tools/ci/prune-workspace-cache.mjs');
const workflows = ['codex-preview', 'chain-macos', 'companion-native', 'windows-spike'];
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

/// The workflow-level `env:` of a Chain workflow as {NAME: value}; every job of it inherits it.
const chainJobEnv = (name = 'chain-core') => {
  const block = workflowText(name).match(/\nenv:\n((?: {2}.*\n)+)/)?.[1] ?? '';
  return Object.fromEntries([...block.matchAll(/^ {2}([A-Z0-9_]+): '?([^'\n]*)'?$/gm)].map(m => [m[1], m[2]]));
};

test('Chain CI build-cost settings stay CI-only, never reach release or test semantics, and key the cache', () => {
  const env = chainJobEnv();
  assert.deepEqual(Object.keys(env).sort(), ['CARGO_INCREMENTAL', 'CARGO_PROFILE_DEV_DEBUG', 'RUSTUP_TOOLCHAIN']);
  assert.equal(env.CARGO_PROFILE_DEV_DEBUG, 'line-tables-only');
  assert.equal(env.CARGO_INCREMENTAL, '0');
  assert.deepEqual(chainJobEnv('chain-macos'), env, 'the macOS and Linux Chain workflows build with the same settings');
  for (const name of ['chain-core', 'chain-macos']) {
  const text = workflowText(name);
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
  }
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
  // Windows test and build CI is paused: the Chain checks are the Linux matrix and the Node
  // contracts in one workflow and one macOS job in a workflow of its own (it has its own path list).
  const macWorkflow = workflowText('chain-macos');
  const chainJobs = [...jobsOf(macWorkflow), ...jobsOf(chain)];
  assert.deepEqual(chainJobs.map(j => j.name), ['chain-macos', 'chain-linux', 'chain-node']);
  const mac = chainJobs[0].block, linux = chainJobs[1].block, node = chainJobs[2].block;
  assert.match(node, /^    runs-on: ubuntu-24\.04$/m);
  assert.match(mac, /^    runs-on: macos-15$/m);
  assert.doesNotMatch(mac, /^\s+(?:strategy|matrix):/m);
  assert.match(linux, /^    runs-on: ubuntu-24\.04$/m);
  assert.doesNotMatch(chain + macWorkflow, /runner: windows-|runs-on: windows-|shell: pwsh/);
  // A guarded step (`node tools/ci/expect-tests.mjs <minimum> cargo test …`) is the same Cargo check.
  const macCargo = [...mac.matchAll(/(?:node tools\/ci\/expect-tests\.mjs \d+ )?cargo (test|build)[^\n]*?--features ([\w,-]+)([^\n]*)/g)]
    .map(m => ({kind: m[1], features: m[2], rest: m[3]}));
  const macGroups = macCargo.map(c => c.features).filter((f, i, all) => f !== all[i-1]);
  // Each feature profile is one contiguous group on macOS, so no profile is compiled twice there.
  // The exploration contract and the Monitor-fixture profile are not built on macOS by this
  // workflow at all (the product candidate job runs the exploration suites on macOS).
  assert.deepEqual(macGroups, ['chain-core,visual-authoring,product-safety', 'approved-plan,visual-authoring,product-safety',
    'approved-plan,visual-authoring,recovery-auto-approval', 'will-end-history', 'will-end-history,visual-authoring,product-safety']);
  // Linux: one row per feature profile, the complete library where a profile has library
  // contracts of its own, and the last row's extra check for the Monitor-fixture profile.
  const rows = [...linux.matchAll(/^          - group: ([\w-]+)\n            features: ([\w,-]+)\n            lib: (\w+)\n            minimum: (\d+)\n            suites: ([\w ]+)\n/gm)]
    .map(r => ({group: r[1], features: r[2], lib: r[3], minimum: Number(r[4]), suites: r[5].split(' ')}));
  assert.equal((linux.match(/^          - group: /gm) || []).length, rows.length, 'every row has the full shape');
  assert.deepEqual(rows.map(r => [r.features, r.lib]), [['chain-core', 'none'], ['chain-core,visual-authoring,product-safety', 'visual_start'],
    ['approved-plan,visual-authoring,product-safety', 'all'], ['exploration-flow,visual-authoring,product-safety', 'all'],
    ['will-end-history', 'all'], ['will-end-history,visual-authoring,product-safety', 'all']]);
  assert.ok(linux.includes('            extra: --features will-end-history,approved-plan,visual-authoring,product-safety --test chain_monitor_fixtures\n'));
  // What stays on macOS alone: suites of macOS-only behaviour and the M4-R profile, whose
  // integration suite is compiled on macOS and Windows only. Nothing else may be missing on Linux:
  // every other integration suite macOS runs for a profile is in that profile's Linux row.
  const MAC_ONLY = ['automatic_guard', 'write_guard'];
  for (const row of rows) {
    assert.ok(row.minimum >= 1, row.group);
    for (const suite of row.suites) {
      assert.ok(existsSync(join(root, 'tests', `${suite}.rs`)), `${row.group}: ${suite}`);
      assert.ok(!MAC_ONLY.includes(suite), `${row.group}: ${suite} stays on macOS`);
    }
  }
  for (const {kind, features, rest} of macCargo) {
    if (kind !== 'test' || features.includes('recovery-auto-approval')) continue;
    const row = rows.find(r => r.features === features);
    assert.ok(row, `${features} has a Linux row`);
    for (const [, suite] of rest.matchAll(/--test (\w+)/g)) assert.ok(MAC_ONLY.includes(suite) || row.suites.includes(suite), `${features}: ${suite} also runs on Linux`);
  }
  // macOS keeps the suites the audit found to need it (gil-evidence/ci-os-sensitivity-audit-20261125).
  const macRuns = (features, suite) => macCargo.some(c => c.kind === 'test' && c.features === features && new RegExp(`--test ${suite}\\b`).test(c.rest));
  for (const [features, suites] of [
    ['chain-core,visual-authoring,product-safety', ['visual_start', 'mcp']],
    ['approved-plan,visual-authoring,product-safety', ['approved_plan_wire_wave1', 'chain_recovery_preservation', 'closing_handoff', 'resume_guidance', 'manual', ...MAC_ONLY]],
    ['approved-plan,visual-authoring,recovery-auto-approval', ['recovery_approval_mode']],
    ['will-end-history', ['will_end_session', 'mcp', 'format_transition']],
    ['will-end-history,visual-authoring,product-safety', ['will_end_visual', 'will_end_session', 'format_transition', 'visual_start', 'mcp', 'chain_handoff', 'visual_attachment']],
  ]) for (const suite of suites) assert.ok(macRuns(features, suite), `macOS keeps ${suite} with ${features}`);
  assert.match(mac, /--features approved-plan,visual-authoring,product-safety --lib -- restore:: lock:: install:: [^\n]* --test-threads=1\n/);
  // Both jobs prepare the same way and use one key expression and one path list (the caches differ
  // by runner OS). macOS restores and saves in one step; on Linux every row restores and one saves.
  const cacheOf = block => block.match(/^          key: .+\n          restore-keys: \|\n.+$/m)?.[0];
  const paths = block => block.match(/\n          path: \|\n((?: {12}.*\n)+)/)?.[1];
  assert.ok(cacheOf(mac) && paths(mac));
  // The key expression is the same text; each workflow hashes its own file into it.
  assert.equal(cacheOf(linux), cacheOf(mac).replaceAll('.github/workflows/chain-macos.yml', '.github/workflows/chain-core.yml'));
  assert.ok(cacheOf(mac).includes(".github/workflows/chain-macos.yml'"));
  assert.equal(paths(linux), paths(mac));
  const pin = mac.match(/uses: actions\/cache@([0-9a-f]{40}) /)?.[1];
  assert.ok(pin && linux.includes(`uses: actions/cache/restore@${pin} `) && linux.includes(`uses: actions/cache/save@${pin} `));
  assert.equal((linux.match(/^          key: .+$/gm) || []).filter(k => k === cacheOf(linux).split('\n')[0]).length, 2, 'restore and save use one key');
  assert.equal((linux.match(/^            save_cache: true$/gm) || []).length, 1);
  assert.equal((linux.match(/^        if: matrix\.save_cache && steps\.linux_cache\.outputs\.cache-hit != 'true'$/gm) || []).length, 2);
  assert.doesNotMatch(linux, /uses: actions\/cache@|build_cache/);
  for (const prepare of ['      - uses: actions/checkout@', "          GIT_CONFIG_VALUE_0: 'false'", '          persist-credentials: false', '      - run: rustup toolchain install 1.97.1 --profile minimal'])
    for (const {name, block} of chainJobs.slice(0, 2)) assert.ok(block.includes(prepare), `${name}: ${prepare}`);
  // The checks without a Core process run on Linux for every Chain change, never only when the
  // macOS path list matches.
  for (const step of ['Shared workflow text, Skill catalog and scenario evaluator contracts (pure Node, no Core process)', 'CI cache and ordering contracts',
    'Manual example source and reporting contracts (no Core process)', 'Chain interview evaluator contracts (no model evaluation)', 'Chain interview synthetic fixture integrity',
    'Chain closing evaluator and unapproved item-split model contracts', 'Chain closing synthetic fixture integrity (no model evaluation)', 'Resume measurement contracts',
    'Context growth measurement contracts (Unix process fixtures)']) {
    assert.ok(node.includes(`      - name: ${step}\n`), `chain-node: ${step}`);
    assert.ok(!mac.includes(`      - name: ${step}\n`), `chain-macos no longer carries: ${step}`);
  }
  assert.doesNotMatch(node, /if: runner\.os/);
});

/// Rust sources under `dir`, repo-relative with forward slashes.
const rustFiles = dir => readdirSync(join(root, dir), {withFileTypes: true}).flatMap(e =>
  e.isDirectory() ? rustFiles(`${dir}/${e.name}`) : e.name.endsWith('.rs') ? [`${dir}/${e.name}`] : []);

test('the macOS Chain job runs on a pull request exactly when a file that can behave differently on macOS changes', () => {
  const text = workflowText('chain-macos');
  const pull = text.match(/\non:\n  pull_request:\n    branches: \[main\]\n    paths:\n((?: {6}.*\n)+)/)?.[1];
  assert.ok(pull, 'the pull-request path list');
  const listed = [...pull.matchAll(/^      - '([^']+)'$/gm)].map(m => m[1]);
  assert.equal(new Set(listed).size, listed.length, 'no path twice');
  // Source: every file with an OS condition, and nothing else from src/. A new `#[cfg(unix)]` in a
  // file that is not listed fails here, so the list cannot fall behind the code silently.
  const osCondition = /cfg!?\((?:all\(|any\(|not\()*[^)]*\b(?:unix|windows|target_os|target_family)\b/;
  const conditional = rustFiles('src').filter(f => osCondition.test(readFileSync(join(root, f), 'utf8'))).sort();
  assert.deepEqual(listed.filter(p => p.startsWith('src/')).sort(), conditional);
  // Tests: every integration suite the job runs, and the helpers they share.
  const job = jobsOf(text)[0].block;
  const suites = [...new Set([...job.matchAll(/cargo test [^\n]*/g)].flatMap(m => [...m[0].matchAll(/--test (\w+)/g)].map(t => t[1])))].sort();
  assert.deepEqual(listed.filter(p => /^tests\/\w+\.rs$/.test(p)).sort(), suites.map(s => `tests/${s}.rs`));
  for (const shared of ['tests/common/**', 'tests/support_wave1/**', 'tests/guidance_support/**', 'tests/fixtures/**', 'Cargo.toml', 'Cargo.lock',
    'tools/manual-example-check/**', 'tools/chain-context-bench/**', 'tools/chain-resume-benchmark/**', 'tools/ci/**', '.github/workflows/chain-macos.yml'])
    assert.ok(listed.includes(shared), shared);
  // After a merge the job runs for every source or test change, so a miss of the list shows on main.
  assert.match(text, /\n  push:\n    branches: \[main\]\n    paths:\n      - 'src\/\*\*'\n      - 'tests\/\*\*'\n      - 'Cargo\.toml'\n      - 'Cargo\.lock'\n/);
  assert.match(text, /\n  workflow_dispatch:\n/);
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
