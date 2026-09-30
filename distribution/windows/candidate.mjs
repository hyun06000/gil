// Maintainer-only Windows candidate. Never installs, publishes, or edits a marketplace registry.
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {readFile, writeFile, mkdir, readdir, lstat} from 'node:fs/promises';
import {join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {collect, assertCoverage, loadBundle, sha256, regular} from '../compliance/notices.mjs';

export const TARGET = 'x86_64-pc-windows-msvc';
const ROOT = resolve(import.meta.dirname, '../..');
const PROFILE = {directory: 'distribution/windows/compliance', target: TARGET};
const NAME = 'gil-companion-prototype';
const PREFIX = `plugins/${NAME}`;
export function inspectPE(bytes) {
  assert.ok(bytes.length >= 256 && bytes.toString('ascii', 0, 2) === 'MZ', 'not a PE executable');
  const pe = bytes.readUInt32LE(0x3c);
  assert.ok(pe + 264 <= bytes.length, 'truncated PE');
  assert.equal(bytes.readUInt32LE(pe), 0x4550, 'bad PE signature');
  assert.equal(bytes.readUInt16LE(pe + 4), 0x8664, 'not Windows x64');
  assert.equal(bytes.readUInt16LE(pe + 24), 0x20b, 'not PE32+');
  const count = bytes.readUInt16LE(pe + 6), optional = bytes.readUInt16LE(pe + 20);
  const sections = pe + 24 + optional;
  assert.ok(sections + count * 40 <= bytes.length);
  const offset = rva => {
    for (let i = 0; i < count; i++) {
      const s = sections + i * 40, start = bytes.readUInt32LE(s + 12);
      const size = bytes.readUInt32LE(s + 16), raw = bytes.readUInt32LE(s + 20);
      if (rva >= start && rva - start < size) {
        const at = raw + rva - start;
        assert.ok(at < bytes.length); return at;
      }
    }
    throw Error('unmapped PE import address');
  };
  const directory = pe + 24 + 112;
  assert.ok(optional >= 240 && bytes.readUInt32LE(pe + 24 + 108) >= 16);
  // No hidden delayed dependency surface in this candidate.
  assert.equal(bytes.readUInt32LE(directory + 13 * 8), 0, 'delay imports require review');
  const imports = [], table = bytes.readUInt32LE(directory + 8);
  assert.ok(table, 'missing imports');
  let row = offset(table), terminated = false;
  for (let i = 0; i < 256; i++, row += 20) {
    assert.ok(row + 20 <= bytes.length);
    if (bytes.subarray(row, row + 20).every(x => x === 0)) { terminated = true; break; }
    const name = offset(bytes.readUInt32LE(row + 12));
    const end = bytes.indexOf(0, name);
    assert.ok(end > name && end - name < 256);
    imports.push(bytes.toString('ascii', name, end).toLowerCase());
  }
  assert.ok(terminated, 'unterminated import table');
  const allowed = new Set(['kernel32.dll', 'ntdll.dll', 'advapi32.dll', 'bcrypt.dll',
    'userenv.dll', 'ws2_32.dll', 'ole32.dll', 'shell32.dll', 'user32.dll', 'crypt32.dll',
    'secur32.dll', 'iphlpapi.dll', 'normaliz.dll', 'ucrtbase.dll']);
  for (const name of imports) assert.ok(allowed.has(name) || /^api-ms-win-[a-z0-9-]+\.dll$/.test(name),
    `non-system dependency requires review: ${name}`);
  return [...new Set(imports)].sort();
}
export function noBuildPaths(bytes) {
  for (const encoding of ['utf8', 'utf16le']) {
    const text = bytes.toString(encoding);
    assert.ok(!/(?:[A-Za-z]:[\\/](?:Users|a|hostedtoolcache)[\\/]|\/Users\/|\/home\/)/i.test(text),
      'build-machine path in candidate');
  }
}
export async function inventory(root, prefix = '') {
  const rows = [];
  for (const name of (await readdir(root)).sort()) {
    const path = join(root, name), info = await lstat(path), rel = prefix + name;
    assert.ok(!info.isSymbolicLink(), 'symlink refused');
    if (info.isDirectory()) rows.push(...await inventory(path, rel + '/'));
    else {
      assert.ok(info.isFile(), 'non-file refused');
      const bytes = await readFile(path);
      rows.push({path: rel, bytes: bytes.length, sha256: sha256(bytes)});
    }
  }
  return rows;
}
export async function prepare(output) {
  assert.equal(process.platform, 'win32', 'native Windows execution required');
  assert.equal(process.arch, 'x64');
  const policy = JSON.parse(await regular(join(ROOT, PROFILE.directory, 'notice-policy.json')));
  assertCoverage(policy, await collect(ROOT, {target: TARGET}), TARGET);
  const notices = await loadBundle(ROOT, PROFILE);
  const run = (command, args, options = {}) => execFileSync(command, args,
    {cwd: ROOT, encoding: 'utf8', timeout: 900000, maxBuffer: 32e6, ...options});
  const head = run('git', ['rev-parse', 'HEAD']).trim();
  assert.match(head, /^[a-f0-9]{40}$/);
  assert.equal(run('git', ['status', '--porcelain', '--untracked-files=no']).trim(), '', 'dirty source refused');
  const out = resolve(output);
  await mkdir(out); // Refuse overwriting an old candidate.
  const env = {...process.env, CARGO_ENCODED_RUSTFLAGS: [
    '-Ctarget-feature=+crt-static', `--remap-path-prefix=${ROOT}=/gil`,
    `--remap-path-prefix=${process.env.USERPROFILE}=/builder`,
    ...(process.env.CARGO_HOME ? [`--remap-path-prefix=${process.env.CARGO_HOME}=/cargo`] : [])
  ].join('\x1f')};
  run('cargo', ['build', '--locked', '--release', '-p', 'gil', '--bin', 'gil', '--target', TARGET], {env, stdio: 'inherit'});
  const binary = await regular(join(ROOT, 'target', TARGET, 'release', 'gil.exe'));
  const imports = inspectPE(binary); noBuildPaths(binary);
  const manifest = JSON.parse(await regular(join(ROOT, PREFIX, '.codex-plugin/plugin.json')));
  assert.equal(manifest.name, NAME);
  manifest.version = `0.2.1-preview.3+windows.${head.slice(0, 12)}`;
  manifest.mcpServers = {'gil-companion': {command: './core/windows-x64/gil.exe', args: ['mcp', '--serve'], cwd: '.'}};
  const catalog = JSON.parse(await regular(join(ROOT, 'distribution/codex/catalog.json')));
  catalog.name = 'gil-preview-windows-x64';
  catalog.interface.displayName = 'GIL Windows x64 — review candidate';
  const json = value => Buffer.from(JSON.stringify(value, null, 2) + '\n');
  const files = new Map([
    ['.agents/plugins/marketplace.json', json(catalog)],
    [`${PREFIX}/.codex-plugin/plugin.json`, json(manifest)],
    [`${PREFIX}/core/windows-x64/gil.exe`, binary],
    [`${PREFIX}/skills/gil-companion/SKILL.md`, await regular(join(ROOT, PREFIX, 'skills/gil-companion/SKILL.md'))],
    [`${PREFIX}/LICENSE`, await regular(join(ROOT, 'LICENSE'))],
    ...[...notices].map(([name, bytes]) => [`${PREFIX}/${name}`, bytes])
  ]);
  const tree = join(out, 'marketplace'); await mkdir(tree);
  for (const [name, bytes] of files) {
    noBuildPaths(bytes);
    const path = join(tree, name);
    await mkdir(resolve(path, '..'), {recursive: true});
    await writeFile(path, bytes, {flag: 'wx'});
  }
  const before = await inventory(tree);
  assert.deepEqual(before.map(x => x.path).sort(), [...files.keys()].sort());
  const archive = join(out, 'gil-windows-x64-candidate.tar.gz');
  run('tar.exe', ['-czf', archive, '-C', tree, '.']);
  const extracted = join(out, '한글 roundtrip'); await mkdir(extracted);
  run('tar.exe', ['-xzf', archive, '-C', extracted]);
  assert.deepEqual(await inventory(extracted), before, 'archive bytes changed');
  // The driver launches only the extracted executable, with a system-only child PATH.
  const smoke = JSON.parse(run(process.execPath, [join(ROOT, 'distribution/windows/spike.mjs'),
    join(extracted, PREFIX, 'core/windows-x64/gil.exe')]));
  const digest = sha256(await readFile(archive));
  await writeFile(join(out, 'SHA256SUMS'), `${digest}  gil-windows-x64-candidate.tar.gz\n`, {flag: 'wx'});
  const checks = {schema: 1, source: head, target: TARGET, version: manifest.version,
    publishable: false, channel: 'windows_unsigned_review_candidate', archive_sha256: digest,
    imports, notice_packages: policy.packages.length, files: before, smoke,
    gates: {archive_roundtrip: 'passed', native_relocated_execution: 'passed',
      authenticode: 'not signed', ordinary_user_codex_install: 'not tested',
      fullscreen: 'not tested', marketplace_publication: 'not performed',
      msvc_runtime_redistribution_review: 'pending'}};
  await writeFile(join(out, 'checks.json'), json(checks), {flag: 'wx'});
  console.log(JSON.stringify(checks, null, 2));
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  assert.equal(process.argv.length, 3, 'Usage: node candidate.mjs <fresh-output-directory>');
  await prepare(process.argv[2]);
}
