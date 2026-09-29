import test from 'node:test';
import assert from 'node:assert/strict';
import {execFileSync, spawnSync} from 'node:child_process';
import {chmod, mkdir, mkdtemp, readFile, realpath, rm, symlink, unlink, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {dirname, join} from 'node:path';
import {ARCHIVE_FLAGS, CORE, FILES, MANIFEST, PREFIX, createTree, hash, inventory, repository, verifyTree} from './artifact.mjs';
import {stageUnsigned, verifyUnsignedTree, TRUST} from './unsigned-preview.mjs';
import {checkTransition} from './update-policy.mjs';
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
  return {dir, root, input, output, stage: (version = '0.2.1-preview.1') => stageUnsigned({input, output, version})};
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
test('a Skill-only change is receipted and requires a new version and pair acceptance, even with identical Core bytes', async t => {
  const f = await fixture(t), previous = await f.stage(), original = await inventory(f.output);
  const skill = `${PREFIX}/skills/gil-companion/SKILL.md`;
  const updated = Buffer.from('---\nname: gil-companion\ndescription: fixture\n---\nChanged fixture guidance.\n');
  // Editing an already staged Skill cannot pass the original receipt.
  const oldSkill = await readFile(join(f.output, skill));
  await writeFile(join(f.output, skill), updated);
  await assert.rejects(verifyUnsignedTree(f.output));
  await writeFile(join(f.output, skill), oldSkill);
  await verifyUnsignedTree(f.output);

  // A reviewed source change is packaged through the normal path, not a receipt rewrite.
  await writeFile(join(f.root, skill), updated);
  execFileSync('git', ['add', skill], {cwd: f.root, stdio: 'ignore'});
  execFileSync('git', ['-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
    'commit', '-qm', 'change fixture Skill'], {cwd: f.root, stdio: 'ignore'});
  const nextInput = join(f.dir, 'next-development'), nextOutput = join(f.dir, 'next-candidate');
  await createTree({root: f.root, binary: join(f.root, 'binary'), output: nextInput});
  const next = await stageUnsigned({input: nextInput, output: nextOutput, version: '0.2.1-preview.2'});
  assert.notEqual(next.source.snapshot_sha256, previous.source.snapshot_sha256);
  assert.deepEqual(next.files.find(file => file.path === CORE), previous.files.find(file => file.path === CORE));
  assert.equal(next.files.find(file => file.path === skill).sha256, hash(updated));
  for (const file of previous.files.filter(file => ![skill, MANIFEST, 'PREVIEW.md'].includes(file.path))) {
    assert.deepEqual(next.files.find(item => item.path === file.path), file);
  }
  const record = async (root, receipt, name) => {
    const archive = join(f.dir, name);
    execFileSync('/usr/bin/tar', ['-czf', archive, ...ARCHIVE_FLAGS, '-C', root, '.'],
      {env: {...process.env, COPYFILE_DISABLE: '1'}, stdio: 'pipe'});
    return {schema: 1, plugin: receipt.plugin, platform: receipt.platform, version: receipt.version,
      source_commit: receipt.source.head, source_snapshot_sha256: receipt.source.snapshot_sha256,
      archive_sha256: hash(await readFile(archive)), core_sha256: receipt.files.find(file => file.path === CORE).sha256,
      // Synthetic UI/contract fixtures: no fake native executable is run here.
      ui_sha256: 'e'.repeat(64), migration: 'none', compatibility: {
        protocol: {min: 1, max: 1}, action_surface: {min: 1, max: 1}, storage_format: {min: 4, max: 4},
        monitor_view_schema: {min: 1, max: 1}, node_detail_schema: {min: 1, max: 1}}};
  };
  const before = await record(f.output, previous, 'before.tar.gz');
  const after = await record(nextOutput, next, 'after.tar.gz');
  assert.notEqual(before.archive_sha256, after.archive_sha256);
  assert.equal(checkTransition(before, {...after, version: before.version}).reason, 'same_version_has_different_bytes_or_claims');
  const transition = checkTransition(before, after);
  assert.equal(transition.status, 'pair_test_required');
  assert.equal(transition.install_authorized, false); assert.equal(transition.publishable, false);
  assert.deepEqual(await inventory(f.output), original);
  await verifyUnsignedTree(f.output); await verifyUnsignedTree(nextOutput);
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
  const manualOnly = /if: github\.event_name == 'workflow_dispatch' && inputs\.unsigned_preview_version != ''/g;
  assert.equal([...workflow.matchAll(manualOnly)].length, 2, 'both candidate build and retention require explicit manual opt-in');
  assert.match(workflow, /GIL_PREVIEW_VERSION: \$\{\{ inputs\.unsigned_preview_version \}\}/);
  assert.match(workflow, /prepare target\/codex-preview\/marketplace "\$GIL_PREVIEW_VERSION"/);
  assert.doesNotMatch(workflow, /run:.*\$\{\{ inputs\./);
  assert.match(workflow, /contents: read/);
  assert.doesNotMatch(workflow, /contents: write|secrets\.|gh release|git push|codex plugin add/);
  assert.match(workflow, /distribution\/codex\/unsigned-preview\.test\.mjs/);
});
