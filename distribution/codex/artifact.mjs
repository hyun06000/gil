// Maintainer tooling only. Nothing in this directory is an installed runtime dependency.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {chmod, lstat, mkdir, readFile, readdir, writeFile} from 'node:fs/promises';
import {dirname, join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {OUTPUTS as NOTICE_FILES, loadBundle, validatePolicy} from '../compliance/notices.mjs';

export const NAME = 'gil-companion-prototype';
export const PREFIX = `plugins/${NAME}`;
export const CORE = `${PREFIX}/core/darwin-arm64/gil`;
export const MANIFEST = `${PREFIX}/.codex-plugin/plugin.json`;
export const CATALOG = '.agents/plugins/marketplace.json';
export const FILES = [CATALOG, MANIFEST, CORE, `${PREFIX}/skills/gil-companion/SKILL.md`,
  `${PREFIX}/LICENSE`, ...NOTICE_FILES.map(name => `${PREFIX}/${name}`), 'PREVIEW.md'].sort();
export const hash = bytes => createHash('sha256').update(bytes).digest('hex');
// macOS bsdtar: no developer username/group, ACL, xattr or resource-fork metadata.
export const ARCHIVE_FLAGS = ['--format', 'ustar', '--uid', '0', '--gid', '0',
  '--uname', 'root', '--gname', 'wheel', '--no-acls', '--no-xattrs', '--no-mac-metadata'];
export const repository = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const json = data => JSON.stringify(data, null, 2) + '\n';
const preview = `# GIL Codex native marketplace — DEVELOPMENT PREVIEW ONLY

macOS arm64 only. NOT a public release; NOT Developer ID signed/notarized by this pipeline.
Do not publish this tree or register it as a user distribution channel.
No Node/npm/Cargo/source checkout is required by the installed MCP runtime.
The Rust executable contains Core, 17 MCP tools and the Monitor UI.
Companion is not bundled or automatically launched; it remains an optional separate fallback.
Third-party licenses/notices are included beside LICENSE; standard library notices have their own HTML file.
This catalog is a packaging test, not evidence of marketplace installation or fullscreen display.
See release.json for hashes, source snapshot and unresolved release gates.
`;

export function checkManifest(value) {
  assert.equal(value.name, NAME, 'stable plugin identity');
  assert.match(value.version, /^\d+\.\d+\.\d+(?:[-+][a-zA-Z0-9.+-]+)?$/);
  assert.equal(value.skills, './skills/');
  assert.deepEqual(value.mcpServers, {'gil-companion': {
    command: './core/darwin-arm64/gil', args: ['mcp', '--serve'], cwd: '.',
  }}, 'one relative native entry point; no environment, shell or download hook');
  // Packaging is allowlisted. A future optional component requires an explicit review.
  for (const key of Object.keys(value)) assert.ok([
    'name', 'version', 'description', 'author', 'skills', 'interface',
    'mcpServers'].includes(key), `unpackaged manifest component: ${key}`);
}

export function checkCatalog(value) {
  assert.equal(value.name, 'gil-preview-macos-arm64');
  assert.deepEqual(value.plugins, [{name: NAME,
    source: {source: 'local', path: `./${PREFIX}`},
    policy: {installation: 'AVAILABLE', authentication: 'ON_INSTALL'}, category: 'Productivity',
  }], 'catalog must resolve entirely within the built marketplace root');
}

export function checkMachO(bytes) {
  assert.ok(bytes.length >= 32, 'Mach-O header missing');
  assert.equal(bytes.readUInt32LE(0), 0xfeedfacf, 'expected thin little-endian Mach-O 64');
  assert.equal(bytes.readUInt32LE(4), 0x0100000c, 'expected arm64, not Intel/universal');
  assert.equal(bytes.readUInt32LE(12), 2, 'expected executable');
}

export function checkNoLocalPaths(bytes) {
  assert.ok(!/\/(?:Users|home)\//.test(bytes.toString('utf8')),
    'artifact contains a developer home path; rebuild with path remapping');
}

export function checkLibraries(otoolOutput) {
  const dependencies = otoolOutput.trimEnd().split('\n').slice(1).map(line => line.trim().split(' (')[0]);
  assert.ok(dependencies.length > 0, 'no load-command evidence');
  assert.ok(dependencies.every(path => path.startsWith('/System/Library/') || path.startsWith('/usr/lib/')),
    'native artifact depends on an unbundled non-system library');
  return dependencies;
}

async function regular(path) {
  // Check every component, not only the leaf: a symlinked skills directory is unsafe too.
  const parts = resolve(path).split('/').filter(Boolean);
  let at = '/';
  for (const part of parts) {
    at = join(at, part);
    assert.equal((await lstat(at)).isSymbolicLink(), false, 'symlink input refused');
  }
  assert.ok((await lstat(path)).isFile(), 'expected regular input file');
  return readFile(path);
}

export async function inventory(root) {
  const files = [];
  async function walk(at, prefix = '') {
    for (const name of (await readdir(at)).sort()) {
      const path = join(at, name), rel = prefix ? `${prefix}/${name}` : name;
      const stat = await lstat(path);
      assert.equal(stat.isSymbolicLink(), false, 'symlink artifact refused');
      if (stat.isDirectory()) await walk(path, rel);
      else {
        assert.ok(stat.isFile(), 'non-file artifact refused');
        const bytes = await readFile(path);
        files.push({path: rel, bytes: bytes.length, mode: stat.mode & 0o777, sha256: hash(bytes)});
      }
    }
  }
  assert.ok((await lstat(root)).isDirectory());
  await walk(root);
  return files.sort((a, b) => a.path.localeCompare(b.path, 'en'));
}

export async function sourceSnapshot(root) {
  const git = (...args) => execFileSync('git', args, {cwd: root, encoding: 'utf8'});
  const paths = [...new Set(git('ls-files', '-z', '--cached', '--others', '--exclude-standard')
    .split('\0').filter(Boolean))].sort();
  const rows = [];
  for (const path of paths) {
    try { rows.push([path, hash(await regular(join(root, path)))]); }
    catch (error) { if (error.code !== 'ENOENT') throw error; rows.push([path, null]); }
  }
  return {head: git('rev-parse', 'HEAD').trim(),
    dirty: git('status', '--porcelain=v1', '--untracked-files=all').length !== 0,
    files: rows.length, snapshot_sha256: hash(JSON.stringify(rows))};
}

export async function createTree({root = repository, binary, output}) {
  const destination = resolve(output);
  // Existing paths, including dangling symlinks, are never replaced. Failure leaves no receipt.
  await mkdir(destination, {recursive: false, mode: 0o755});
  const source = await sourceSnapshot(root);
  const inputs = new Map([
    [CATALOG, join(root, 'distribution/codex/catalog.json')],
    [MANIFEST, join(root, PREFIX, '.codex-plugin/plugin.json')],
    [CORE, resolve(binary)],
    [`${PREFIX}/skills/gil-companion/SKILL.md`, join(root, PREFIX, 'skills/gil-companion/SKILL.md')],
    [`${PREFIX}/LICENSE`, join(root, 'LICENSE')],
  ]);
  const content = new Map();
  for (const [path, input] of inputs) {
    const bytes = await regular(input);
    checkNoLocalPaths(bytes);
    content.set(path, bytes);
  }
  checkManifest(JSON.parse(content.get(MANIFEST)));
  checkCatalog(JSON.parse(content.get(CATALOG)));
  checkMachO(content.get(CORE));
  content.set('PREVIEW.md', Buffer.from(preview));
  // Locked inputs and original legal-text hashes are checked before any notice can be packaged.
  for (const [name, bytes] of await loadBundle(root)) {
    checkNoLocalPaths(bytes);
    content.set(`${PREFIX}/${name}`, bytes);
  }
  for (const [path, bytes] of content) {
    await mkdir(dirname(join(destination, path)), {recursive: true});
    await writeFile(join(destination, path), bytes, {flag: 'wx', mode: path === CORE ? 0o755 : 0o644});
    await chmod(join(destination, path), path === CORE ? 0o755 : 0o644);
  }
  // A snapshot is not an attestation that an arbitrary --binary was built from this source.
  // The orchestration script builds it first; this API deliberately makes no such claim.
  const receipt = {schema: 1, channel: 'development_unsigned', publishable: false,
    platform: 'darwin-arm64', plugin: NAME, version: JSON.parse(content.get(MANIFEST)).version,
    source, binary_origin: 'supplied_binary; source correspondence requires build pipeline evidence',
    gates: {developer_id: 'not_verified', notarization: 'not_verified',
      clean_machine_install: 'not_verified', marketplace_install: 'not_verified',
      first_launch_timeout: 'unresolved'}, files: await inventory(destination)};
  assert.deepEqual(await sourceSnapshot(root), source, 'source changed during packaging');
  await writeFile(join(destination, 'release.json'), json(receipt), {flag: 'wx', mode: 0o644});
  await verifyTree(destination);
  return receipt;
}

export async function verifyTree(root) {
  const receipt = JSON.parse(await regular(join(root, 'release.json')));
  assert.equal(receipt.schema, 1);
  assert.equal(receipt.channel, 'development_unsigned');
  assert.equal(receipt.publishable, false, 'preview cannot promote itself to release');
  assert.equal(receipt.platform, 'darwin-arm64');
  assert.equal(receipt.plugin, NAME);
  assert.deepEqual(receipt.gates, {developer_id: 'not_verified', notarization: 'not_verified',
    clean_machine_install: 'not_verified', marketplace_install: 'not_verified',
    first_launch_timeout: 'unresolved'});
  const files = (await inventory(root)).filter(file => file.path !== 'release.json');
  assert.deepEqual(files.map(file => file.path).sort(), FILES, 'unexpected or missing runtime files');
  assert.deepEqual(files, receipt.files, 'artifact integrity or executable mode changed');
  for (const file of files) {
    assert.equal(file.mode, file.path === CORE ? 0o755 : 0o644);
    checkNoLocalPaths(await readFile(join(root, file.path)));
  }
  const manifest = JSON.parse(await regular(join(root, MANIFEST)));
  checkManifest(manifest);
  assert.equal(receipt.version, manifest.version);
  checkCatalog(JSON.parse(await regular(join(root, CATALOG))));
  checkMachO(await regular(join(root, CORE)));
  const notices = JSON.parse(await regular(join(root, PREFIX, 'THIRD-PARTY-NOTICES.json')));
  validatePolicy(notices);
  assert.equal(hash(await regular(join(root, PREFIX, 'RUST-STDLIB-NOTICES.html'))), notices.stdlib.sha256,
    'standard library notice differs from the pinned original');
  const legalText = (await regular(join(root, PREFIX, 'THIRD-PARTY-NOTICES.txt'))).toString('utf8');
  for (const p of notices.packages) for (const f of p.files) {
    const marker = `===== ORIGINAL LEGAL TEXT SHA-256 ${f.sha256} =====\n\n`;
    assert.ok(legalText.includes(marker), 'missing original legal text section');
  }
  return receipt;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [command, ...args] = process.argv.slice(2);
  if (command === 'create' && args.length === 2) {
    const receipt = await createTree({binary: args[0], output: args[1]});
    console.log(json({channel: receipt.channel, files: receipt.files.length, source: receipt.source}));
  } else if (command === 'verify' && args.length === 1) {
    const receipt = await verifyTree(resolve(args[0]));
    console.log(json({integrity: 'passed', channel: receipt.channel, publishable: false}));
  } else if (command === 'publish') {
    throw new Error('Publishing is deliberately not implemented: signing, notarization and installation acceptance are open');
  } else throw new Error('Usage: artifact.mjs create <built-binary> <new-directory> | verify <directory>');
}
