// Maintainer-only isolated verification bundle for a user's Windows PC: the crate's App Server
// integration test as a prebuilt test executable, the test-only testbed and the verify driver.
// Never installs, publishes, registers or edits anything; the bundle is not a product build.
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {readFile, writeFile, mkdir} from 'node:fs/promises';
import {join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {collect, assertCoverage, loadBundle, sha256, regular} from '../compliance/notices.mjs';
import {profileForArch, packTar, unpackTar, inspectPE, noBuildPaths, inventory} from './candidate.mjs';

const ROOT = resolve(import.meta.dirname, '../..');
/// The Codex CLI version the verify driver downloads when asked for the pinned source; it must
/// agree with `PIN_VERSION` in src/runtime_verify.rs (checked against the built driver below).
export const CODEX_CLI_PIN = '0.163.0-alpha.11.1';
export const EXECUTABLES = {
  'gil-runtime-verify': 'gil-runtime-verify.exe',
  'gil-runtime-testbed': 'gil-runtime-testbed.exe',
  'runtime_integration': 'runtime_integration.exe'
};
/// Carries `CARGO_BIN_EXE_*` / `CARGO_MANIFEST_DIR` defaults by design (overridden at run time
/// through GIL_RUNTIME_TESTBED / GIL_RUNTIME_INTEGRATION_EVIDENCE_DIR); the other two must not.
export const EMBEDS_BUILD_PATHS = new Set(['runtime_integration.exe']);
export function bundleName(arch) { profileForArch(arch); return `gil-runtime-verify-windows-${arch}`; }
/// The three executables from `cargo test --no-run --message-format=json` output, by target name.
export function executablesFrom(messages) {
  const found = new Map();
  for (const line of messages.split('\n')) {
    if (!line.startsWith('{')) continue;
    const m = JSON.parse(line);
    if (m.reason !== 'compiler-artifact' || !m.executable) continue;
    const name = m.target.name;
    if (!(name in EXECUTABLES)) continue;
    assert.ok(!found.has(name), `two executables for ${name}`);
    found.set(name, m.executable);
  }
  for (const name of Object.keys(EXECUTABLES)) assert.ok(found.has(name), `missing executable ${name}`);
  return found;
}
/// bundle.json: what the driver verifies at run time (every file but itself).
export function manifest(head, arch, rows) {
  assert.match(head, /^[a-f0-9]{40}$/);
  assert.ok(rows.length && rows.every(r => /^[A-Za-z0-9._-]+$/.test(r.path) && r.path !== 'bundle.json'), 'manifest rows');
  for (const exe of Object.values(EXECUTABLES)) assert.ok(rows.some(r => r.path === exe), `manifest lacks ${exe}`);
  return {schema: 1, kind: 'gil-runtime-verify', source: head, arch: profileForArch(arch).arch, codex_cli_pin: CODEX_CLI_PIN, files: rows};
}
export async function prepare(output) {
  assert.equal(process.platform, 'win32', 'native Windows execution required');
  const profile = profileForArch(process.arch);
  const {arch, target, policyName} = profile;
  const noticeProfile = {directory: 'distribution/windows/compliance', target, policyName};
  const policy = JSON.parse(await regular(join(ROOT, noticeProfile.directory, policyName)));
  assertCoverage(policy, await collect(ROOT, {target}), target);
  const notices = await loadBundle(ROOT, noticeProfile);
  const run = (command, args, options = {}) => execFileSync(command, args,
    {cwd: ROOT, encoding: 'utf8', timeout: 900000, maxBuffer: 64e6, ...options});
  const head = run('git', ['rev-parse', 'HEAD']).trim();
  assert.match(head, /^[a-f0-9]{40}$/);
  assert.equal(run('git', ['status', '--porcelain', '--untracked-files=no']).trim(), '', 'dirty source refused');
  const out = resolve(output);
  await mkdir(out); // Refuse overwriting an old bundle.
  const selected = JSON.parse(run('pwsh', ['-NoProfile', '-File', join(ROOT, 'distribution/windows/toolchain.ps1'), '-Architecture', arch]));
  assert.equal(selected.receipt.prerelease, false);
  assert.equal(selected.receipt.architecture, arch);
  assert.ok(selected.environment[profile.linkerKey]);
  const env = {...process.env, ...selected.environment, CARGO_ENCODED_RUSTFLAGS: [
    '-Ctarget-feature=+crt-static', `--remap-path-prefix=${ROOT}=/gil`,
    `--remap-path-prefix=${process.env.USERPROFILE}=/builder`,
    ...(process.env.CARGO_HOME ? [`--remap-path-prefix=${process.env.CARGO_HOME}=/cargo`] : [])
  ].join('\x1f')};
  // One build: the integration test (not run) and, through required-features, both test-only bins.
  const messages = run('cargo', ['test', '--locked', '--release', '-p', 'gil', '--features', 'runtime-testbed', '--test', 'runtime_integration',
    '--no-run', '--target', target, '--message-format=json'], {env, stdio: ['ignore', 'pipe', 'inherit']});
  const built = executablesFrom(messages);
  const name = bundleName(arch);
  const tree = join(out, 'stage'), dir = join(tree, name);
  await mkdir(dir, {recursive: true});
  const imports = {};
  const files = new Map();
  for (const [targetName, fileName] of Object.entries(EXECUTABLES)) {
    const bytes = await regular(built.get(targetName));
    imports[fileName] = inspectPE(bytes, arch);
    if (!EMBEDS_BUILD_PATHS.has(fileName)) noBuildPaths(bytes);
    files.set(fileName, bytes);
  }
  const driver = files.get('gil-runtime-verify.exe');
  assert.ok(driver.includes(Buffer.from(`codex-${CODEX_CLI_PIN}-win32-${arch}.tgz`)), 'driver pin differs from CODEX_CLI_PIN');
  files.set('VERIFY.md', await regular(join(ROOT, 'distribution/windows/runtime-verify/VERIFY.md')));
  files.set('LICENSE', await regular(join(ROOT, 'LICENSE')));
  for (const [noticeName, bytes] of notices) files.set(noticeName, bytes);
  for (const [fileName, bytes] of files) await writeFile(join(dir, fileName), bytes, {flag: 'wx'});
  const rows = await inventory(dir);
  const json = value => Buffer.from(JSON.stringify(value, null, 2) + '\n');
  await writeFile(join(dir, 'bundle.json'), json(manifest(head, arch, rows)), {flag: 'wx'});
  const before = await inventory(tree);
  const archiveName = `${name}.tar.gz`, archive = join(out, archiveName);
  const archiveBytes = packTar(run, tree);
  await writeFile(archive, archiveBytes, {flag: 'wx'});
  const extracted = join(out, 'roundtrip'); await mkdir(extracted);
  unpackTar(run, archiveBytes, extracted);
  assert.deepEqual(await inventory(extracted), before, 'archive bytes changed');
  const digest = sha256(archiveBytes);
  await writeFile(join(out, `SHA256SUMS-${arch}`), `${digest}  ${archiveName}\n`, {flag: 'wx'});
  const checks = {schema: 1, kind: 'gil-runtime-verify', source: head, target, architecture: arch,
    archive: archiveName, archive_sha256: digest, archive_bytes: archiveBytes.length,
    codex_cli_pin: `@openai/codex@${CODEX_CLI_PIN} (downloaded by the driver into the test folder; not included)`,
    publishable: false, files: before, imports, embedded_build_paths: [...EMBEDS_BUILD_PATHS],
    toolchain: selected.receipt, notice_packages: policy.packages.length,
    gates: {archive_roundtrip: 'passed', self_test_from_archive: 'pending (workflow step)', authenticode: 'not signed',
      user_pc_execution: 'not performed', product_manifest_or_mcp_wiring: 'none'}};
  await writeFile(join(out, `checks-${arch}.json`), json(checks), {flag: 'wx'});
  console.log(JSON.stringify(checks, null, 2));
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  assert.equal(process.argv.length, 3, 'Usage: node verify-bundle.mjs <fresh-output-directory>');
  await prepare(process.argv[2]);
}
