// Maintainer-only, offline, fail-closed notice assembly. Never installs or edits a policy.
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {readFile, readdir, lstat} from 'node:fs/promises';
import {dirname, join, relative, resolve, parse, sep} from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';

export const TARGET = 'aarch64-apple-darwin';
export const DIRECTORY = 'distribution/compliance';
export const INPUTS = ['Cargo.toml', 'Cargo.lock', 'mcp-app/package-lock.json', 'mcp-app/build.mjs',
  'plugins/gil-companion-prototype/assets/monitor.html'];
export const OUTPUTS = ['THIRD-PARTY-NOTICES.txt', 'THIRD-PARTY-NOTICES.json', 'RUST-STDLIB-NOTICES.html'];
export const repository = resolve(import.meta.dirname, '../..');
export const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const order = (a, b) => a < b ? -1 : a > b ? 1 : 0;
const json = value => JSON.stringify(value, null, 2) + '\n';
const key = p => `${p.ecosystem}:${p.name}@${p.version}`;
export function lockChecksums(text) {
  // Cargo-generated lock format only; refuse unfamiliar quoted forms instead of guessing TOML.
  const rows = new Map();
  for (const block of text.split(/^\[\[package\]\]\s*$/m).slice(1)) {
    const field = name => { const match = block.match(new RegExp(`^${name} = ("[^"\\n]*")$`, 'm')); return match && JSON.parse(match[1]); };
    const name = field('name'), version = field('version'), checksum = field('checksum');
    assert.ok(name && version, 'unrecognized Cargo.lock package');
    if (field('source') === 'registry+https://github.com/rust-lang/crates.io-index') {
      assert.match(checksum ?? '', /^[a-f0-9]{64}$/);
      assert.ok(!rows.has(`${name}@${version}`), 'ambiguous locked source');
      rows.set(`${name}@${version}`, checksum);
    }
  }
  assert.ok(rows.size, 'empty registry lock');
  return rows;
}
function localPath(value) {
  assert.ok(typeof value === 'string' && value && !value.startsWith('/') && !value.includes('\\') &&
    value.split('/').every(p => p && p !== '.' && p !== '..'), 'unsafe notice path');
  return value;
}
export async function regular(path) {
  const absolute = resolve(path);
  let at = parse(absolute).root;
  for (const part of relative(at, absolute).split(sep).filter(Boolean)) {
    at = join(at, part);
    assert.equal((await lstat(at)).isSymbolicLink(), false, 'symlink notice input refused');
  }
  assert.ok((await lstat(path)).isFile(), 'notice input must be a regular file');
  return readFile(path);
}
export async function legalFiles(root) {
  const files = [];
  async function walk(dir, prefix = '') {
    for (const name of (await readdir(dir)).sort(order)) {
      if (name === '.git' || name === 'node_modules') continue;
      const path = join(dir, name), rel = prefix + name, info = await lstat(path);
      assert.equal(info.isSymbolicLink(), false, 'symlink in dependency package refused');
      if (info.isDirectory()) await walk(path, rel + '/');
      else if (/^(?:licen[cs]e|copying|copyright|notice)(?:$|[._-])/i.test(name)) {
        const bytes = await regular(path);
        assert.ok(bytes.length && !bytes.includes(0), 'empty/binary legal text requires review');
        files.push({path: rel, sha256: sha256(bytes)});
      }
    }
  }
  await walk(root);
  return files.sort((a, b) => order(a.path, b.path));
}

export async function collect(root = repository, {cargo = 'cargo', rustc = 'rustc', target = TARGET} = {}) {
  assert.ok([TARGET, 'x86_64-pc-windows-msvc', 'aarch64-pc-windows-msvc'].includes(target), 'unreviewed target');
  const run = (cmd, args) => execFileSync(cmd, args, {cwd: root, encoding: 'utf8', maxBuffer: 64e6, timeout: 120000});
  const metadata = JSON.parse(run(cargo, ['metadata', '--locked', '--offline', '--format-version', '1', '--filter-platform', target]));
  const tree = edges => new Set(run(cargo, ['tree', '--locked', '--offline', '--target', target,
    '-p', 'gil', '--edges', edges, '--prefix', 'none', '--format', '{p}']).split('\n')
    .map(s => s.match(/^(\S+) v([^\s]+)/)).filter(Boolean).map(m => `${m[1]}@${m[2]}`));
  const all = tree('normal,build'), normal = tree('normal,no-proc-macro');
  const checksums = lockChecksums((await regular(join(root, 'Cargo.lock'))).toString('utf8'));
  const upstream = JSON.parse(await regular(join(root, DIRECTORY, 'upstream.json')));
  const packages = [];
  for (const p of metadata.packages) {
    if (!all.has(`${p.name}@${p.version}`) || metadata.workspace_members.includes(p.id)) continue;
    assert.equal(p.source, 'registry+https://github.com/rust-lang/crates.io-index', 'new source needs provenance review');
    const dir = dirname(p.manifest_path);
    const checksum = checksums.get(`${p.name}@${p.version}`);
    assert.ok(checksum, 'dependency absent from Cargo.lock');
    let files = await legalFiles(dir), fallback;
    if (!files.length) {
      fallback = upstream[`${p.name}@${p.version}`];
      assert.ok(fallback, `no legal text: ${p.name}@${p.version}`);
      const vcs = JSON.parse(await regular(join(dir, '.cargo_vcs_info.json')));
      assert.equal(vcs.git.sha1, fallback.commit, 'upstream fallback revision drift');
      assert.equal(vcs.path_in_vcs, fallback.crate_path, 'upstream fallback path drift');
      files = fallback.files;
    }
    packages.push({ecosystem: 'cargo', name: p.name, version: p.version,
      role: normal.has(`${p.name}@${p.version}`) ? 'normal-candidate' : 'build-or-proc-macro',
      declared_license: p.license, source: `https://crates.io/crates/${p.name}/${p.version}`,
      integrity: checksum, files, ...(fallback ? {upstream: fallback} : {})});
  }
  const app = join(root, 'mcp-app');
  const {build} = await import(pathToFileURL(join(app, 'node_modules/esbuild/lib/main.js')));
  const result = await build({absWorkingDir: app, entryPoints: ['app.js'], bundle: true,
    write: false, format: 'iife', platform: 'browser', minify: true, legalComments: 'inline', metafile: true});
  const html = (await regular(join(root, INPUTS[4]))).toString('utf8');
  assert.ok(html.includes(result.outputFiles[0].text.replace(/<\/script/gi, '<\\/script')), 'stale embedded UI; rebuild before notice review');
  const lock = JSON.parse(await regular(join(app, 'package-lock.json'))), used = new Map();
  for (const output of Object.values(result.metafile.outputs)) for (const [path, detail] of Object.entries(output.inputs)) {
    if (!path.includes('node_modules/') || !detail.bytesInOutput) continue;
    let dir = dirname(resolve(app, path)), pkg;
    while (dir !== dirname(dir)) {
      try { pkg = JSON.parse(await regular(join(dir, 'package.json'))); if (pkg.name) break; }
      catch (error) { if (error.code !== 'ENOENT') throw error; }
      dir = dirname(dir);
    }
    assert.ok(pkg?.name, 'cannot identify bundled dependency');
    const id = `${pkg.name}@${pkg.version}`;
    if (!used.has(id)) {
      const locked = lock.packages[relative(app, dir).split(sep).join('/')];
      assert.equal(pkg.version, locked?.version, 'installed npm version differs from lock');
      assert.ok(locked.integrity, 'npm dependency has no pinned integrity');
      const files = await legalFiles(dir);
      assert.ok(files.length, `no legal text: ${id}`);
      used.set(id, {ecosystem: 'npm', name: pkg.name, version: pkg.version, role: 'bundled-ui',
        declared_license: pkg.license, source: `https://www.npmjs.com/package/${pkg.name}/v/${pkg.version}`,
        integrity: locked.integrity, files});
    }
  }
  packages.push(...used.values());
  packages.sort((a, b) => order(key(a), key(b)));
  const verbose = run(rustc, ['-vV']);
  const release = verbose.match(/^release: (.+)$/m)?.[1], commit = verbose.match(/^commit-hash: (.+)$/m)?.[1];
  assert.ok(release && /^[a-f0-9]{40}$/.test(commit), 'unidentified Rust toolchain');
  const inputs = Object.fromEntries(await Promise.all(INPUTS.map(async p => [p, sha256(await regular(join(root, p)))])));
  return {schema: 1, target, inputs, toolchain: {release, commit}, packages};
}

export function validatePolicy(policy, target = TARGET) {
  assert.ok([TARGET, 'x86_64-pc-windows-msvc', 'aarch64-pc-windows-msvc'].includes(target));
  assert.equal(policy.schema, 1); assert.equal(policy.target, target);
  assert.deepEqual(Object.keys(policy.inputs).sort(), [...INPUTS].sort());
  for (const hash of Object.values(policy.inputs)) assert.match(hash, /^[a-f0-9]{64}$/);
  assert.ok(policy.packages.length > 0, 'empty notice inventory refused');
  const keys = policy.packages.map(key);
  assert.equal(new Set(keys).size, keys.length, 'duplicate package');
  assert.deepEqual(keys, [...keys].sort(order), 'unstable inventory order');
  assert.match(policy.toolchain.commit, /^[a-f0-9]{40}$/);
  assert.ok(policy.toolchain.release);
  for (const p of policy.packages) {
    assert.ok(['cargo', 'npm'].includes(p.ecosystem));
    assert.ok(['normal-candidate', 'build-or-proc-macro', 'bundled-ui'].includes(p.role));
    assert.ok(p.declared_license && p.integrity && p.files.length, `missing notice coverage: ${key(p)}`);
    for (const f of p.files) { localPath(f.path); assert.match(f.sha256, /^[a-f0-9]{64}$/); }
  }
  assert.equal(policy.stdlib.path, 'COPYRIGHT-library.html');
  assert.match(policy.stdlib.sha256, /^[a-f0-9]{64}$/);
  assert.ok(policy.stdlib.source.startsWith('https://static.rust-lang.org/'));
}
export function assertCoverage(policy, observed, target = TARGET) {
  validatePolicy(policy, target);
  const {stdlib, ...expected} = policy;
  assert.deepEqual(observed, expected, 'notice coverage changed; review dependencies/legal texts then update policy explicitly');
}
export async function loadBundle(root = repository, {directory = DIRECTORY, target = TARGET, policyName = 'notice-policy.json'} = {}) {
  assert.ok(['notice-policy.json', 'notice-policy-arm64.json'].includes(policyName));
  const base = join(root, directory), policy = JSON.parse(await regular(join(base, policyName)));
  validatePolicy(policy, target);
  for (const [path, expected] of Object.entries(policy.inputs)) {
    assert.equal(sha256(await regular(join(root, path))), expected, `notice policy stale: ${path}`);
  }
  const hashes = [...new Set(policy.packages.flatMap(p => p.files.map(f => f.sha256)))].sort();
  const expectedNames = [...hashes, policy.stdlib.sha256].map(h => `${h}.json`).sort();
  assert.deepEqual((await readdir(join(base, 'texts'))).sort(), expectedNames, 'missing or unreferenced legal texts');
  const texts = new Map();
  for (const name of expectedNames) {
    // JSON preserves original CRLF and missing final newlines without normalizing upstream bytes.
    const stored = JSON.parse(await regular(join(base, 'texts', name)));
    assert.deepEqual(Object.keys(stored), ['text']);
    assert.equal(typeof stored.text, 'string');
    const bytes = Buffer.from(stored.text, 'utf8');
    assert.equal(sha256(bytes), name.split('.')[0], 'modified legal text');
    assert.ok(bytes.length && !bytes.includes(0));
    assert.ok(Buffer.from(bytes.toString('utf8')).equals(bytes), 'legal text must be lossless UTF-8');
    texts.set(name, bytes);
  }
  let notice = '# GIL native MCP / Monitor: third-party notices\n\n' +
    `Target: ${policy.target}\nRust: ${policy.toolchain.release} (${policy.toolchain.commit})\n\n` +
    'GIL retains its separate MIT license. Upstream license choices and conditions are not rewritten here.\n' +
    'This conservative inventory includes Core build/proc-macro dependencies as well as runtime candidates.\n' +
    'Companion and the legacy Node bridge are not in this artifact. Metadata is descriptive, not a license grant.\n' +
    'MCP SDK license files include transition terms; read the full originals, not metadata alone.\n' +
    'Rust standard library and its dependencies: see RUST-STDLIB-NOTICES.html (original distribution notice).\n\n';
  for (const p of policy.packages) {
    notice += `## ${key(p)}\nRole: ${p.role}\nDeclared license: ${p.declared_license}\nSource: ${p.source}\n`;
    if (p.upstream) notice += `Legal source: ${p.upstream.repository}/tree/${p.upstream.commit}\n`;
    for (const f of p.files) notice += `Original file: ${f.path} -> SHA-256 ${f.sha256}\n`;
    notice += '\n';
  }
  for (const h of hashes) notice += `\n===== ORIGINAL LEGAL TEXT SHA-256 ${h} =====\n\n` + texts.get(`${h}.json`).toString('utf8') + '\n';
  return new Map([['THIRD-PARTY-NOTICES.txt', Buffer.from(notice)],
    ['THIRD-PARTY-NOTICES.json', Buffer.from(json(policy))],
    ['RUST-STDLIB-NOTICES.html', texts.get(`${policy.stdlib.sha256}.json`)]]);
}
export async function check(root = repository, commands) {
  const policy = JSON.parse(await regular(join(root, DIRECTORY, 'notice-policy.json')));
  assertCoverage(policy, await collect(root, commands));
  const bundle = await loadBundle(root);
  return {target: TARGET, packages: policy.packages.length, files: [...bundle].map(([path, bytes]) =>
    ({path, bytes: bytes.length, sha256: sha256(bytes)}))};
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  assert.deepEqual(process.argv.slice(2), ['check'], 'Usage: node distribution/compliance/notices.mjs check');
  console.log(json(await check()));
}
