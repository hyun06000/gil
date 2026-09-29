import test from 'node:test';
import assert from 'node:assert/strict';
import {execFileSync, spawnSync} from 'node:child_process';
import {mkdtemp, realpath, readFile, writeFile, mkdir, rm, chmod, symlink, unlink, cp} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {dirname, join} from 'node:path';
import {ARCHIVE_FLAGS, CATALOG, CORE, FILES, MANIFEST, PREFIX, checkCatalog, checkManifest, checkMachO, checkLibraries,
  checkNoLocalPaths, createTree, hash, inventory, repository, verifyTree} from './artifact.mjs';
import {noticeFixture} from '../compliance/test-fixture.mjs';

const manifest = JSON.parse(await readFile(join(repository, MANIFEST)));
const catalog = JSON.parse(await readFile(join(import.meta.dirname, 'catalog.json')));
function macho() {
  const bytes = Buffer.alloc(40);
  bytes.writeUInt32LE(0xfeedfacf, 0); bytes.writeUInt32LE(0x0100000c, 4); bytes.writeUInt32LE(2, 12);
  return bytes;
}
async function fixture(t) {
  const temporary = await realpath(await mkdtemp(join(tmpdir(), 'gil-dist-unit-')));
  t.after(() => rm(temporary, {recursive: true, force: true}));
  const root = join(temporary, 'source');
  await mkdir(root);
  const git = (...args) => execFileSync('git', args, {cwd: root, stdio: 'ignore'});
  git('init', '-q');
  git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', 'commit', '-qm', 'fixture', '--allow-empty');
  const files = new Map([[MANIFEST, JSON.stringify(manifest)],
    ['distribution/codex/catalog.json', JSON.stringify(catalog)],
    [`${PREFIX}/skills/gil-companion/SKILL.md`, '---\nname: gil-companion\ndescription: fixture\n---\n'],
    ['LICENSE', 'fixture license'], ['binary', macho()]]);
  for (const [path, bytes] of files) {
    await mkdir(dirname(join(root, path)), {recursive: true});
    await writeFile(join(root, path), bytes);
  }
  await noticeFixture(root);
  const output = join(temporary, 'marketplace');
  return {temporary, root, output, binary: join(root, 'binary'),
    build: () => createTree({root, output, binary: join(root, 'binary')})};
}

test('catalog resolves from the built marketplace root and preserves identity', () => {
  checkManifest(manifest); checkCatalog(catalog);
  assert.equal(catalog.plugins[0].source.path, `./${PREFIX}`);
});
test('absolute, Node, shell, alternate cwd and duplicate server entries are refused', () => {
  for (const change of [entry => entry.command = '/tmp/gil', entry => entry.command = 'node',
    entry => entry.args = ['server.mjs'], entry => entry.cwd = '..', entry => entry.env = {A: 'B'}]) {
    const altered = structuredClone(manifest); change(altered.mcpServers['gil-companion']);
    assert.throws(() => checkManifest(altered));
  }
  const altered = structuredClone(manifest); altered.mcpServers.other = {};
  assert.throws(() => checkManifest(altered));
});
test('new optional manifest components cannot silently be omitted from packaging', () => {
  for (const field of ['apps', 'hooks', 'commands', 'scripts']) {
    assert.throws(() => checkManifest({...manifest, [field]: './extra'}));
  }
});
test('catalog cannot escape the artifact or select a different plugin', () => {
  for (const path of ['../source', '/tmp/plugin', './plugins/other']) {
    const altered = structuredClone(catalog); altered.plugins[0].source.path = path;
    assert.throws(() => checkCatalog(altered));
  }
});
test('non-arm64, non-Mach-O and library payloads fail before execution', () => {
  checkMachO(macho());
  assert.throws(() => checkMachO(Buffer.from('#!/bin/sh\n')));
  for (const [offset, value] of [[0, 0xcafebabe], [4, 0x01000007], [12, 6]]) {
    const wrong = macho(); wrong.writeUInt32LE(value, offset); assert.throws(() => checkMachO(wrong));
  }
});
test('developer home paths are rejected, canonical StepRefs are not', () => {
  checkNoLocalPaths(Buffer.from('step:C1/S2 /gil/src/main.rs'));
  for (const path of ['/Users/example/code', '/home/runner/work']) {
    assert.throws(() => checkNoLocalPaths(Buffer.from(path)));
  }
});
test('Homebrew and unbundled dylibs are refused, system frameworks are allowed', () => {
  const first = 'gil:\n\t/usr/lib/libSystem.B.dylib (compatibility version 1.0.0)\n';
  assert.deepEqual(checkLibraries(first), ['/usr/lib/libSystem.B.dylib']);
  for (const path of ['/opt/homebrew/lib/x.dylib', '@rpath/x.dylib', './x.dylib']) {
    assert.throws(() => checkLibraries(`${first}\t${path} (compatibility version 1.0.0)\n`));
  }
  assert.throws(() => checkLibraries('gil:\n'));
});
test('self-contained preview has an exact allowlist, integrity receipt and dirty provenance', async t => {
  const f = await fixture(t); const receipt = await f.build();
  assert.deepEqual(receipt.files.map(f => f.path).sort(), FILES);
  assert.equal(receipt.source.dirty, true);
  assert.equal(receipt.publishable, false);
  assert.match(receipt.source.snapshot_sha256, /^[a-f0-9]{64}$/);
  assert.equal(receipt.files.find(f => f.path === CORE).mode, 0o755);
  await verifyTree(f.output);
  assert.equal(JSON.stringify(receipt).includes(f.root), false);
});
test('existing outputs are not overwritten', async t => {
  const f = await fixture(t); await f.build();
  const before = await inventory(f.output);
  await assert.rejects(f.build, {code: 'EEXIST'});
  assert.deepEqual(await inventory(f.output), before);
});
test('altered executable and lost executable bit fail integrity checks', async t => {
  const f = await fixture(t); await f.build();
  await chmod(join(f.output, CORE), 0o644);
  await assert.rejects(verifyTree(f.output));
  await chmod(join(f.output, CORE), 0o755);
  const bad = macho(); bad[35] = 1; await writeFile(join(f.output, CORE), bad);
  await assert.rejects(verifyTree(f.output));
});
test('missing Skill and extra node_modules are rejected', async t => {
  const f = await fixture(t); await f.build();
  const skill = join(f.output, PREFIX, 'skills/gil-companion/SKILL.md');
  const saved = await readFile(skill); await unlink(skill);
  await assert.rejects(verifyTree(f.output));
  await writeFile(skill, saved, {mode: 0o644});
  await mkdir(join(f.output, PREFIX, 'node_modules'));
  await writeFile(join(f.output, PREFIX, 'node_modules/dependency.js'), 'unexpected');
  await assert.rejects(verifyTree(f.output));
});
test('all three notice files survive packaging and cannot be omitted', async t => {
  const f = await fixture(t); await f.build();
  for (const name of ['THIRD-PARTY-NOTICES.txt', 'THIRD-PARTY-NOTICES.json', 'RUST-STDLIB-NOTICES.html']) {
    const path = join(f.output, PREFIX, name), saved = await readFile(path);
    await unlink(path); await assert.rejects(verifyTree(f.output));
    await writeFile(path, saved, {mode: 0o644});
  }
  const text = await readFile(join(f.output, PREFIX, 'THIRD-PARTY-NOTICES.txt'), 'utf8');
  assert.ok(text.includes('Fixture copyright\r\nFixture permission text (no final newline)'));
  await verifyTree(f.output);
});
test('stale notice policy stops packaging after dependency or UI changes', async t => {
  const f = await fixture(t);
  await writeFile(join(f.root, 'Cargo.lock'), 'changed lock');
  await assert.rejects(f.build, /notice policy stale/);
  await assert.rejects(readFile(join(f.output, 'release.json')), {code: 'ENOENT'});
});
test('symlinked source and artifact files are refused', async t => {
  const f = await fixture(t); await f.build();
  await unlink(join(f.output, CORE)); await symlink(f.binary, join(f.output, CORE));
  await assert.rejects(verifyTree(f.output), /symlink/);
  const aliased = join(f.temporary, 'alias'); await symlink(f.binary, aliased);
  await assert.rejects(createTree({root: f.root, binary: aliased, output: join(f.temporary, 'other')}), /symlink/);
});
test('symlinked source directory is refused, not only its leaf', async t => {
  const f = await fixture(t);
  const dir = join(f.root, PREFIX, 'skills/gil-companion');
  const elsewhere = join(f.temporary, 'skill'); await cp(dir, elsewhere, {recursive: true});
  await rm(dir, {recursive: true}); await symlink(elsewhere, dir);
  await assert.rejects(f.build, /symlink/);
});
test('receipt cannot mark an unsigned preview publishable', async t => {
  const f = await fixture(t); const receipt = await f.build(); receipt.publishable = true;
  await writeFile(join(f.output, 'release.json'), JSON.stringify(receipt));
  await assert.rejects(verifyTree(f.output));
});
test('public publishing is unavailable and cannot be enabled with a CLI flag', () => {
  const run = spawnSync(process.execPath, [join(import.meta.dirname, 'artifact.mjs'), 'publish'], {encoding: 'utf8'});
  assert.notEqual(run.status, 0); assert.match(run.stderr, /deliberately not implemented/);
});
test('unchanged inputs yield identical tree receipts; archive roundtrip retains executable mode', async t => {
  const f = await fixture(t); await f.build();
  const another = join(f.temporary, 'second');
  await createTree({root: f.root, binary: f.binary, output: another});
  assert.deepEqual(await inventory(f.output), await inventory(another));
  const archive = join(f.temporary, 'preview.tar.gz');
  execFileSync('/usr/bin/tar', ['-czf', archive, ...ARCHIVE_FLAGS, '-C', f.output, '.'], {env: {...process.env, COPYFILE_DISABLE: '1'}});
  const listing = execFileSync('/usr/bin/tar', ['-tvzf', archive], {encoding: 'utf8'});
  assert.ok(listing.split('\n').filter(Boolean).every(line => /root\s+wheel/.test(line)), 'archive owner identity normalized');
  assert.ok(!listing.includes('._'), 'no resource-fork sidecars');
  const extracted = join(f.temporary, 'extracted'); await mkdir(extracted);
  execFileSync('/usr/bin/tar', ['-xzf', archive, '-C', extracted]);
  await verifyTree(extracted);
  assert.deepEqual(await inventory(f.output), await inventory(extracted));
  assert.match(hash(await readFile(archive)), /^[a-f0-9]{64}$/);
});
test('workflow is read-only and preserves archive dotfiles and modes', async () => {
  const workflow = await readFile(join(repository, '.github/workflows/codex-preview.yml'), 'utf8');
  assert.match(workflow, /workflow_dispatch:/);
  assert.match(workflow, /contents: read/);
  assert.doesNotMatch(workflow, /secrets\.|pull_request_target:|contents: write|git push|gh release/);
  assert.match(workflow, /\.tar\.gz/);
  assert.match(workflow, /persist-credentials: false/);
  assert.match(workflow, /distribution\/compliance\/notices\.mjs check/);
  assert.match(workflow, /distribution\/compliance\/notices\.test\.mjs/);
  assert.match(workflow, /cargo fetch --locked/);
  for (const line of workflow.split('\n').filter(line => line.includes('uses:'))) {
    assert.match(line, /@[a-f0-9]{40}(?:\s|$)/, 'third-party actions pinned to full commits');
  }
});

test('required build runs for every main PR against its exact head without a privileged event', async () => {
  const workflow = await readFile(join(repository, '.github/workflows/codex-preview.yml'), 'utf8');
  const events = workflow.slice(workflow.indexOf('\non:'), workflow.indexOf('\npermissions:'));
  assert.match(events, /pull_request:\n    branches: \[main\]/);
  assert.doesNotMatch(events, /paths(?:-ignore)?:|pull_request_target:|push:|tags:/);
  assert.match(workflow, /ref: \$\{\{ github\.event\.pull_request\.head\.sha \|\| github\.sha \}\}/);
  assert.match(workflow, /macos-arm64-preview:\n    runs-on:/);
  assert.match(workflow, /DEVELOPMENT-\$\{\{ github\.event\.pull_request\.head\.sha \|\| github\.sha \}\}/);
  assert.doesNotMatch(workflow, /secrets\.|write-all|contents: write|pull-requests: write/);
});
test('fresh checkout gets an output parent before the non-overwriting build', async t => {
  const workflow = await readFile(join(repository, '.github/workflows/codex-preview.yml'), 'utf8');
  const preparation = 'run: mkdir -p target';
  const build = 'run: node distribution/codex/build-preview.mjs target/codex-preview';
  assert.ok(workflow.includes(preparation) && workflow.includes(build));
  assert.ok(workflow.indexOf(preparation) < workflow.indexOf(build));
  const fresh = await realpath(await mkdtemp(join(tmpdir(), 'gil-ci-fresh-')));
  t.after(() => rm(fresh, {recursive: true, force: true}));
  await assert.rejects(readFile(join(fresh, 'target')), {code: 'ENOENT'});
  execFileSync('/bin/sh', ['-c', preparation.slice('run: '.length)], {cwd: fresh});
  const output = join(fresh, 'target/codex-preview');
  await mkdir(output, {recursive: false}); // Same first operation as build-preview; parent now exists.
  await assert.rejects(mkdir(output, {recursive: false}), {code: 'EEXIST'});
});
