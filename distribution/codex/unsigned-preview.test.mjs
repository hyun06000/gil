import test from 'node:test';
import assert from 'node:assert/strict';
import {execFileSync, spawnSync} from 'node:child_process';
import {chmod, mkdir, mkdtemp, readFile, realpath, rm, symlink, unlink, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {dirname, join} from 'node:path';
import {CORE, FILES, MANIFEST, PREFIX, createTree, hash, inventory, repository, verifyTree} from './artifact.mjs';
import {stageUnsigned, verifyUnsignedTree, TRUST} from './unsigned-preview.mjs';
import {noticeFixture} from '../compliance/test-fixture.mjs';

// Fake Mach-O is only for static packaging tests. It is never executed as a native acceptance test.
async function fixture(t, dirty = false) {
  const dir = await realpath(await mkdtemp(join(tmpdir(), 'gil-unsigned-unit-')));
  t.after(() => rm(dir, {recursive: true, force: true}));
  const root = join(dir, 'source'); await mkdir(root);
  const binary = Buffer.alloc(40);
  for (const [offset, value] of [[0, 0xfeedfacf], [4, 0x0100000c], [12, 2]]) binary.writeUInt32LE(value, offset);
  const files = new Map([
    [MANIFEST, await readFile(join(repository, MANIFEST))],
    ['distribution/codex/catalog.json', await readFile(join(import.meta.dirname, 'catalog.json'))],
    [`${PREFIX}/skills/gil-companion/SKILL.md`, '---\nname: gil-companion\ndescription: fixture\n---\n'],
    ['LICENSE', 'fixture license'], ['binary', binary],
  ]);
  for (const [path, bytes] of files) {
    await mkdir(dirname(join(root, path)), {recursive: true}); await writeFile(join(root, path), bytes);
  }
  await noticeFixture(root);
  const git = (...args) => execFileSync('git', args, {cwd: root, stdio: 'ignore'});
  git('init', '-q'); git('add', '.');
  git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', 'commit', '-qm', 'fixture');
  if (dirty) await writeFile(join(root, 'unreviewed'), 'dirty');
  const input = join(dir, 'development'), output = join(dir, 'candidate');
  await createTree({root, binary: join(root, 'binary'), output: input});
  return {dir, input, output, stage: (version = '0.2.1-preview.1') => stageUnsigned({input, output, version})};
}
async function rewriteReceipt(root, change) {
  const path = join(root, 'release.json'), receipt = JSON.parse(await readFile(path));
  change(receipt); await writeFile(path, JSON.stringify(receipt));
}

test('explicit unsigned candidate preserves runtime/legal bytes and input, changes only version and warning', async t => {
  const f = await fixture(t), before = await inventory(f.input);
  const prior = await verifyTree(f.input), result = await f.stage();
  assert.equal(result.channel, 'preview_unsigned'); assert.equal(result.publishable, false);
  assert.deepEqual(result.trust, TRUST); assert.deepEqual(result.source, prior.source);
  assert.deepEqual(result.files.map(f => f.path).sort(), FILES);
  for (const file of prior.files.filter(f => ![MANIFEST, 'PREVIEW.md'].includes(f.path))) {
    assert.deepEqual(result.files.find(f => f.path === file.path), file);
  }
  const oldManifest = JSON.parse(await readFile(join(f.input, MANIFEST)));
  const newManifest = JSON.parse(await readFile(join(f.output, MANIFEST)));
  assert.deepEqual(newManifest, {...oldManifest, version: '0.2.1-preview.1'});
  assert.deepEqual(await inventory(f.input), before);
  await verifyTree(f.input); await verifyUnsignedTree(f.output);
  await assert.rejects(verifyTree(f.output)); await assert.rejects(verifyUnsignedTree(f.input));
});
test('dirty source cannot be promoted and leaves no output', async t => {
  const f = await fixture(t, true);
  await assert.rejects(f.stage(), /dirty/);
  await assert.rejects(readFile(join(f.output, 'release.json')), {code: 'ENOENT'});
});
test('stable, dev-cachebuster, malformed and path-shaped versions are refused', async t => {
  const f = await fixture(t);
  for (const version of ['0.2.0', '0.2.0+codex.1', '0.2.0-beta.1', '0.2.0-preview.0',
    '01.2.0-preview.1', '0.2.0-preview.01', '0.2.0-preview.1/escape', null]) {
    await assert.rejects(f.stage(version), /explicit/);
  }
});
test('existing input or output is never overwritten', async t => {
  const f = await fixture(t); await f.stage();
  const before = await inventory(f.output);
  await assert.rejects(f.stage(), {code: 'EEXIST'});
  await assert.rejects(stageUnsigned({input: f.input, output: f.input, version: '0.2.1-preview.2'}), {code: 'EEXIST'});
  assert.deepEqual(await inventory(f.output), before); await verifyTree(f.input);
});
test('prerelease version must sort above the input development base, not silently downgrade', async t => {
  const f = await fixture(t);
  await assert.rejects(f.stage('0.2.0-preview.1'), /sort after/);
  await assert.rejects(f.stage('0.1.9-preview.1'), /sort after/);
});
test('tampered or non-executable input cannot be promoted', async t => {
  const f = await fixture(t);
  await chmod(join(f.input, CORE), 0o644); await assert.rejects(f.stage());
  await chmod(join(f.input, CORE), 0o755);
  await writeFile(join(f.input, CORE), 'bad'); await assert.rejects(f.stage());
});
test('missing notices and extra files remain rejected in the unsigned channel', async t => {
  const f = await fixture(t); await f.stage();
  const path = join(f.output, PREFIX, 'THIRD-PARTY-NOTICES.txt'), saved = await readFile(path);
  await unlink(path); await assert.rejects(verifyUnsignedTree(f.output));
  await writeFile(path, saved, {mode: 0o644});
  await writeFile(join(f.output, 'extra'), 'extra'); await assert.rejects(verifyUnsignedTree(f.output));
});
test('unsigned candidate may not assert publication, Apple trust or fresh-Mac success', async t => {
  const f = await fixture(t); await f.stage();
  const saved = await readFile(join(f.output, 'release.json'));
  for (const change of [r => r.publishable = true, r => r.trust.developer_id = 'passed',
    r => r.trust.notarization = 'passed', r => r.trust.clean_machine_install = 'passed',
    r => r.trust.os_security_override = 'allowed', r => r.trust.automatic_update = 'allowed',
    r => r.source.dirty = true, r => r.trust = {}, r => r.channel = 'release_signed_notarized']) {
    await writeFile(join(f.output, 'release.json'), saved);
    await rewriteReceipt(f.output, change); await assert.rejects(verifyUnsignedTree(f.output));
  }
});
test('warning text is mandatory even if its checksum is rewritten', async t => {
  const f = await fixture(t); await f.stage();
  const bytes = Buffer.from('safe for every Mac');
  await writeFile(join(f.output, 'PREVIEW.md'), bytes);
  await rewriteReceipt(f.output, r => Object.assign(r.files.find(f => f.path === 'PREVIEW.md'),
    {sha256: hash(bytes), bytes: bytes.length}));
  await assert.rejects(verifyUnsignedTree(f.output));
});
test('symlinked receipt cannot be used', async t => {
  const f = await fixture(t); await f.stage();
  const path = join(f.output, 'release.json'), elsewhere = join(f.dir, 'saved.json');
  await writeFile(elsewhere, await readFile(path)); await unlink(path); await symlink(elsewhere, path);
  await assert.rejects(verifyUnsignedTree(f.output), /symlink/);
});
test('CLI cannot publish or leak caller paths on failure', () => {
  for (const args of [['publish'], ['prepare', '/private/unreadable-input', '0.2.0-preview.1', '/private/output']]) {
    const run = spawnSync(process.execPath, [join(import.meta.dirname, 'unsigned-preview.mjs'), ...args], {encoding: 'utf8'});
    assert.notEqual(run.status, 0); assert.doesNotMatch(run.stderr, /unreadable-input|\/private\/output/);
  }
});
test('candidate CI is opt-in, read-only and passes the version through a quoted environment variable', async () => {
  const workflow = await readFile(join(repository, '.github/workflows/codex-preview.yml'), 'utf8');
  assert.match(workflow, /unsigned_preview_version:/);
  assert.match(workflow, /if: inputs\.unsigned_preview_version != ''/);
  assert.match(workflow, /GIL_PREVIEW_VERSION: \$\{\{ inputs\.unsigned_preview_version \}\}/);
  assert.match(workflow, /prepare target\/codex-preview\/marketplace "\$GIL_PREVIEW_VERSION"/);
  assert.doesNotMatch(workflow, /run:.*\$\{\{ inputs\./);
  assert.match(workflow, /contents: read/);
  assert.doesNotMatch(workflow, /contents: write|secrets\.|gh release|git push|codex plugin add/);
  assert.match(workflow, /distribution\/codex\/unsigned-preview\.test\.mjs/);
});
