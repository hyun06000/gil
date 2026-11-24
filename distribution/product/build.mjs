// Build + verify the product install artifact of THIS machine's platform. No install into a real
// profile, no credential use, upload, commit, push, tag or release. One reviewed source tree is
// built natively on each platform; the two binaries are different bytes and each is recorded by
// its own hash, with the same source snapshot and Git tree beside it.
//
//   macOS arm64  → the existing Plugin marketplace archive, profile 'product' (../codex/build-preview.mjs)
//   Linux x86-64 → a per-user install archive (bin/gil + notices + INSTALL.md + release.json)
import assert from 'node:assert/strict';
import {spawn, spawnSync} from 'node:child_process';
import {mkdir, mkdtemp, readFile, realpath, rm, writeFile} from 'node:fs/promises';
import {homedir} from 'node:os';
import {join, resolve} from 'node:path';
import {hash, inventory, repository, sourceSnapshot} from '../codex/artifact.mjs';
import {buildVersion, selectedPreview} from '../codex/build-version.mjs';
import {cargoFeatures} from '../codex/compatibility.mjs';
import {TARGETS, collect, assertCoverage, regular} from '../compliance/notices.mjs';
import {DARWIN, LINUX, LINUX_ARCHIVE_FLAGS, PROFILE, createLinuxTree, gitIdentity, verifyLinuxTree} from './artifact.mjs';
import {verifyLinuxArchive} from './verify-install.mjs';

const [destination, ...extra] = process.argv.slice(2);
assert.ok(destination && extra.length === 0, 'Usage: build.mjs <new-output-directory>');
const host = `${process.platform}-${process.arch}`;
assert.ok(['darwin-arm64', 'linux-x64'].includes(host), 'the product build is made natively on macOS arm64 or Linux x86-64 only');
const output = resolve(destination);
assert.ok(!output.startsWith(repository + '/') || output.startsWith(join(repository, 'target') + '/'),
  'the output must be outside the source inventory (or under the ignored target/ folder)');
const run = (command, args, env = process.env) => new Promise((resolve, reject) => {
  const child = spawn(command, args, {cwd: repository, env, stdio: 'inherit'});
  child.on('error', reject);
  child.on('exit', code => code === 0 ? resolve() : reject(new Error(`${command} failed (${code})`)));
});
const said = (command, args) => {
  const got = spawnSync(command, args, {cwd: repository, encoding: 'utf8', timeout: 60000});
  assert.equal(got.status, 0, `${command} ${args.join(' ')} failed`);
  return got.stdout.trim();
};
// Where and with what this was made. Environment values are copied only from this fixed list of
// CI identifiers; nothing secret and no local path is recorded.
function provenance(target, source) {
  const ci = Object.fromEntries(['GITHUB_ACTIONS', 'GITHUB_REPOSITORY', 'GITHUB_SHA', 'GITHUB_REF', 'GITHUB_WORKFLOW', 'GITHUB_RUN_ID',
    'GITHUB_RUN_ATTEMPT', 'GITHUB_JOB', 'RUNNER_OS', 'RUNNER_ARCH', 'ImageOS', 'ImageVersion'].map(key => [key, process.env[key] ?? null]));
  return {schema: 1, profile: PROFILE, platform: target.platform, target: target.triple, cargo_features: cargoFeatures(PROFILE),
    source, git: gitIdentity(repository), toolchain: {rustc: said('rustc', ['-vV']).split('\n'), cargo: said('cargo', ['-V']), node: process.version},
    commands: [`node mcp-app/build.mjs`, `cargo build --locked --release -p gil --target ${target.triple} --features ${cargoFeatures(PROFILE)}`],
    ci, built_on: host};
}

if (host === 'darwin-arm64') {
  // The macOS pipeline is unchanged: UI rebuild, notice recheck, source-change refusal, Mach-O and
  // system-library checks, archive roundtrip and smoke. It creates `output` itself.
  const source = await sourceSnapshot(repository);
  await run(process.execPath, ['distribution/codex/build-preview.mjs', output, '--product']);
  assert.deepEqual(await sourceSnapshot(repository), source, 'source changed during the build');
  const checks = JSON.parse(await readFile(join(output, 'checks.json'), 'utf8'));
  assert.deepEqual(checks.source, source, 'the packaged build saw another source than this run');
  const record = {...provenance(DARWIN, source), archive: {name: DARWIN.archive, sha256: checks.archive_sha256},
    signing: 'not signed or notarized by this pipeline; Gatekeeper is not bypassed and no credential was read'};
  await writeFile(join(output, 'provenance.json'), JSON.stringify(record, null, 2) + '\n', {flag: 'wx'});
  console.log(JSON.stringify({platform: DARWIN.platform, archive_sha256: checks.archive_sha256, git: record.git, source_dirty: source.dirty}));
} else {
  const coreVersion = buildVersion(await selectedPreview(process.env, readFile),
    JSON.parse(await readFile(join(repository, 'plugins/gil-companion-prototype/.codex-plugin/plugin.json'))).version);
  await mkdir(output, {recursive: false}); // Existing output is never overwritten.
  // Dependencies must already be restored with npm ci and cargo fetch. Never install during packaging.
  await run(process.execPath, ['mcp-app/build.mjs']);
  // The Monitor a locked UI build produces must be the committed asset the Core embeds, byte for byte.
  assert.equal(said('git', ['status', '--porcelain=v1', '--', 'plugins/gil-companion-prototype/assets/monitor.html']), '',
    'the rebuilt Monitor differs from the committed asset');
  // The UI inputs of the macOS policy are shared; the Cargo graph is this target's own inventory.
  assert.ok(TARGETS.includes(LINUX.triple));
  const policy = JSON.parse(await regular(join(repository, LINUX.notices.directory, 'notice-policy.json')));
  assertCoverage(policy, await collect(repository, {target: LINUX.triple}), LINUX.triple);
  const source = await sourceSnapshot(repository);
  const target = join(repository, 'target/product-core-build');
  const cargoHome = process.env.CARGO_HOME || join(homedir(), '.cargo');
  assert.ok(!/\s/.test(repository + cargoHome), 'build roots containing whitespace need encoded rustflags support');
  const rustflags = `--remap-path-prefix=${cargoHome}=/cargo --remap-path-prefix=${repository}=/gil`;
  await run('cargo', ['build', '--locked', '--release', '-p', 'gil', '--bin', 'gil', '--target', LINUX.triple, '--features', cargoFeatures(PROFILE)], {
    ...process.env, GIL_BUILD_VERSION: coreVersion, RUSTFLAGS: rustflags, CARGO_TARGET_DIR: target,
  });
  assert.deepEqual(await sourceSnapshot(repository), source, 'source changed during build');
  const binary = join(target, LINUX.triple, 'release/gil');
  const tree = join(output, 'gil-linux-x64');
  const receipt = await createLinuxTree({binary, output: tree, version: coreVersion, source});
  const archive = join(output, LINUX.archive);
  await run('/usr/bin/tar', ['-czf', archive, ...LINUX_ARCHIVE_FLAGS, '-C', tree, '.']);
  const extracted = await realpath(await mkdtemp(join(output, 'roundtrip-')));
  try {
    // Only the archive just built from the allowlisted tree is extracted here.
    await run('/usr/bin/tar', ['-xzf', archive, '--no-same-owner', '-C', extracted]);
    await verifyLinuxTree(extracted);
    assert.deepEqual(await inventory(extracted), await inventory(tree), 'archive roundtrip changed files/modes');
  } finally {
    await rm(extracted, {recursive: true, force: true}); // Only this freshly allocated roundtrip directory.
  }
  const archiveHash = hash(await readFile(archive));
  await writeFile(join(output, 'SHA256SUMS'), `${archiveHash}  ${LINUX.archive}\n`, {flag: 'wx'});
  // From here on only the archive bytes are used: checksum, members, a fresh extraction, an
  // isolated install, and every functional check through the INSTALLED binary.
  const installed = await verifyLinuxArchive({archive, sums: join(output, 'SHA256SUMS'), expectedVersion: coreVersion,
    expectedHtml: join(repository, 'plugins/gil-companion-prototype/assets/monitor.html')});
  const record = {...provenance(LINUX, source), archive: {name: LINUX.archive, sha256: archiveHash},
    binary_sha256: receipt.files.find(file => file.path === LINUX.core).sha256, elf: receipt.elf,
    signing: 'none', support: 'run-time floor is the glibc version read from the binary; supported distributions are not stated by this build'};
  await writeFile(join(output, 'provenance.json'), JSON.stringify(record, null, 2) + '\n', {flag: 'wx'});
  await writeFile(join(output, 'checks.json'), JSON.stringify({channel: 'development_unsigned', publishable: false, source, profile: PROFILE,
    archive_sha256: archiveHash, roundtrip: 'passed', installed}, null, 2) + '\n', {flag: 'wx'});
  console.log(JSON.stringify({platform: LINUX.platform, archive_sha256: archiveHash, glibc_required: receipt.elf.glibc_required,
    git: record.git, source_dirty: source.dirty}));
}
