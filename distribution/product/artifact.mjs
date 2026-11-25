// Maintainer tooling only. The Linux x86-64 GNU user install tree of the product build, and the
// provenance record shared by both platforms. The macOS tree stays the Plugin marketplace built by
// ../codex/artifact.mjs (profile 'product'); nothing here changes that tree or its checks.
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {chmod, lstat, mkdir, readFile, writeFile} from 'node:fs/promises';
import {dirname, join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {checkNoLocalPaths, hash, inventory, repository, sourceSnapshot} from '../codex/artifact.mjs';
import {cargoFeatures, compatibility, developmentFeatures, startKinds} from '../codex/compatibility.mjs';
import {OUTPUTS as NOTICE_FILES, loadBundle, validatePolicy} from '../compliance/notices.mjs';
import {checkElf} from './elf.mjs';

export const PROFILE = 'product';
export const LINUX = {platform: 'linux-x64', triple: 'x86_64-unknown-linux-gnu', core: 'bin/gil',
  archive: 'gil-linux-x64.tar.gz', notices: {directory: 'distribution/linux/compliance', target: 'x86_64-unknown-linux-gnu'}};
export const DARWIN = {platform: 'darwin-arm64', triple: 'aarch64-apple-darwin',
  core: 'plugins/gil-companion-prototype/core/darwin-arm64/gil', archive: 'gil-codex-macos-arm64-preview.tar.gz'};
export const LINUX_FILES = [LINUX.core, 'INSTALL.md', 'LICENSE', ...NOTICE_FILES].sort();
// GNU tar: fixed owner, name order and format; no ACL, xattr or SELinux metadata of the build machine.
export const LINUX_ARCHIVE_FLAGS = ['--format=ustar', '--owner=0', '--group=0', '--numeric-owner', '--sort=name',
  '--no-acls', '--no-xattrs', '--no-selinux'];
const GATES = {signature: 'none', clean_machine_install: 'not_verified', host_screen: 'not_verified', supported_distributions: 'not_stated'};
const json = data => JSON.stringify(data, null, 2) + '\n';
const VERSION = /^\d+\.\d+\.\d+(?:[-+][a-zA-Z0-9.+-]+)?$/;

async function regular(path) {
  const info = await lstat(path);
  assert.ok(info.isFile() && !info.isSymbolicLink(), 'expected a regular input file, not a link');
  return readFile(path);
}

// Head and tree of the checked-out commit. The snapshot covers the working files; this ties the
// same build to the Git objects a reviewer names.
export function gitIdentity(root = repository) {
  const git = (...args) => execFileSync('git', args, {cwd: root, encoding: 'utf8'}).trim();
  return {head: git('rev-parse', 'HEAD'), tree: git('rev-parse', 'HEAD^{tree}')};
}

export async function createLinuxTree({root = repository, binary, output, version, source}) {
  assert.match(version ?? '', VERSION, 'the compiled Core version is required');
  const destination = resolve(output);
  await mkdir(destination, {recursive: false, mode: 0o755}); // Existing paths are never replaced.
  const snapshot = source ?? await sourceSnapshot(root);
  const content = new Map();
  for (const [path, input] of [[LINUX.core, resolve(binary)], ['INSTALL.md', join(root, 'distribution/linux/INSTALL.md')], ['LICENSE', join(root, 'LICENSE')]]) {
    const bytes = await regular(input);
    checkNoLocalPaths(bytes);
    content.set(path, bytes);
  }
  const elf = checkElf(content.get(LINUX.core));
  // The Linux inventory is its own reviewed policy. Without it nothing is packaged: the macOS
  // notices are for another dependency graph and are never substituted.
  for (const [name, bytes] of await loadBundle(root, LINUX.notices)) {
    checkNoLocalPaths(bytes);
    content.set(name, bytes);
  }
  for (const [path, bytes] of content) {
    await mkdir(dirname(join(destination, path)), {recursive: true});
    await writeFile(join(destination, path), bytes, {flag: 'wx', mode: path === LINUX.core ? 0o755 : 0o644});
    await chmod(join(destination, path), path === LINUX.core ? 0o755 : 0o644);
  }
  const receipt = {schema: 1, channel: 'development_unsigned', publishable: false, platform: LINUX.platform,
    product: 'gil', version, profile: PROFILE, cargo_features: cargoFeatures(PROFILE), target: LINUX.triple,
    source: snapshot, binary_origin: 'supplied_binary; source correspondence requires build pipeline evidence',
    compatibility: compatibility(PROFILE), development_features: developmentFeatures(PROFILE), start_kinds: startKinds(PROFILE),
    migration: 'requires_review', elf, gates: GATES, files: await inventory(destination)};
  assert.deepEqual(await sourceSnapshot(root), snapshot, 'source changed during packaging');
  await writeFile(join(destination, 'release.json'), json(receipt), {flag: 'wx', mode: 0o644});
  await verifyLinuxTree(destination);
  return receipt;
}

export async function verifyLinuxTree(root) {
  const receipt = JSON.parse(await regular(join(root, 'release.json')));
  assert.equal(receipt.schema, 1);
  assert.equal(receipt.channel, 'development_unsigned');
  assert.equal(receipt.publishable, false, 'a development tree cannot promote itself to a release');
  assert.equal(receipt.platform, LINUX.platform);
  assert.equal(receipt.target, LINUX.triple);
  assert.equal(receipt.product, 'gil');
  assert.equal(receipt.profile, PROFILE, 'one user build per platform; no reduced edition');
  assert.match(receipt.version, VERSION);
  assert.equal(receipt.cargo_features, cargoFeatures(PROFILE));
  assert.deepEqual(receipt.compatibility, compatibility(PROFILE));
  assert.deepEqual(receipt.development_features, developmentFeatures(PROFILE));
  assert.deepEqual(receipt.start_kinds, startKinds(PROFILE));
  assert.equal(receipt.migration, 'requires_review');
  assert.deepEqual(receipt.gates, GATES);
  const files = (await inventory(root)).filter(file => file.path !== 'release.json');
  assert.deepEqual(files.map(file => file.path).sort(), LINUX_FILES, 'unexpected or missing files');
  assert.deepEqual(files, receipt.files, 'artifact integrity or executable mode changed');
  for (const file of files) {
    assert.equal(file.mode, file.path === LINUX.core ? 0o755 : 0o644);
    checkNoLocalPaths(await readFile(join(root, file.path)));
  }
  assert.deepEqual(checkElf(await regular(join(root, LINUX.core))), receipt.elf, 'the receipt states other run-time requirements than the binary');
  const notices = JSON.parse(await regular(join(root, 'THIRD-PARTY-NOTICES.json')));
  validatePolicy(notices, LINUX.notices.target);
  assert.equal(hash(await regular(join(root, 'RUST-STDLIB-NOTICES.html'))), notices.stdlib.sha256,
    'standard library notice differs from the pinned original');
  const legalText = (await regular(join(root, 'THIRD-PARTY-NOTICES.txt'))).toString('utf8');
  for (const p of notices.packages) for (const f of p.files) {
    assert.ok(legalText.includes(`===== ORIGINAL LEGAL TEXT SHA-256 ${f.sha256} =====\n\n`), 'missing original legal text section');
  }
  return receipt;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [command, ...args] = process.argv.slice(2);
  if (command === 'verify-linux' && args.length === 1) {
    const receipt = await verifyLinuxTree(resolve(args[0]));
    console.log(json({integrity: 'passed', platform: receipt.platform, version: receipt.version, publishable: false}));
  } else throw new Error('Usage: artifact.mjs verify-linux <directory>');
}
