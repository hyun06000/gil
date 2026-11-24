import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdir, mkdtemp, readdir, rm, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {ARCHIVE_FLAGS} from '../codex/artifact.mjs';
import {PLATFORMS, PROFILES, cargoFeatures, checkCore, compatibility, developmentFeatures, profileFromFeatures, startKinds} from '../codex/compatibility.mjs';
import {checkDefaultTools, checkProductTools} from '../codex/tool-contract.mjs';
import {TARGETS} from '../compliance/notices.mjs';
import {DARWIN, LINUX, LINUX_ARCHIVE_FLAGS, LINUX_FILES, PROFILE, createLinuxTree, verifyLinuxTree} from './artifact.mjs';
import {image} from './elf-fixture.mjs';
import {png} from './verify-install.mjs';

test('the product profile is one explicit feature set, the same on both platforms', () => {
  assert.equal(PROFILE, 'product');
  assert.ok(PROFILES.includes(PROFILE));
  assert.equal(cargoFeatures(PROFILE), 'tracking-policy,exploration-flow,visual-authoring,product-safety');
  for (const excluded of ['runtime-testbed', 'recovery-auto-approval', 'all-features']) assert.ok(!cargoFeatures(PROFILE).includes(excluded));
  assert.equal(profileFromFeatures(developmentFeatures(PROFILE)), PROFILE);
  // Reads up to 11; a new project is written as 10 or 9 and no start writes 11.
  assert.deepEqual(compatibility(PROFILE).storage_format, {min: 4, max: 11});
  assert.deepEqual(startKinds(PROFILE), [{start_kind: 'tracked_scope', storage_format: 10}, {start_kind: 'image_reports', storage_format: 9}]);
  assert.equal(startKinds('exploration-flow'), undefined, 'existing profiles keep their receipts');
  // Existing profiles are unchanged by the addition.
  assert.equal(cargoFeatures('exploration-flow'), 'exploration-flow,visual-authoring,product-safety');
  assert.equal(cargoFeatures('default'), 'product-safety');
  assert.deepEqual(compatibility('exploration-flow').storage_format, {min: 4, max: 7});
});

test('a probe is checked against the platform it was packaged for', () => {
  const core = platform => ({schema_version: 1, product: 'gil_core', ...PLATFORMS[platform], ...compatibility(PROFILE)});
  checkCore(core('linux-x64'), PROFILE, 'linux-x64');
  checkCore(core('darwin-arm64'), PROFILE, 'darwin-arm64');
  checkCore(core('darwin-arm64'), PROFILE); // the default platform of every existing caller
  assert.throws(() => checkCore(core('linux-x64'), PROFILE, 'darwin-arm64'));
  assert.throws(() => checkCore(core('darwin-arm64'), PROFILE, 'linux-x64'));
  assert.throws(() => checkCore(core('linux-x64'), PROFILE, 'linux-arm64'), /unknown packaged platform/);
  assert.throws(() => checkCore(core('linux-x64'), 'exploration-flow', 'linux-x64'), 'a reduced build is not the Linux product');
});

test('each platform has its own target, archive, notice inventory and tar dialect', () => {
  assert.equal(LINUX.triple, 'x86_64-unknown-linux-gnu');
  assert.equal(DARWIN.triple, 'aarch64-apple-darwin');
  assert.notEqual(LINUX.archive, DARWIN.archive);
  assert.ok(TARGETS.includes(LINUX.notices.target));
  assert.notEqual(LINUX.notices.directory, 'distribution/compliance', 'the macOS inventory is never reused for Linux');
  // bsdtar-only options are not handed to GNU tar, and the other way round.
  for (const flag of ['--uid', '--gid', '--uname', '--gname', '--no-mac-metadata']) assert.ok(!LINUX_ARCHIVE_FLAGS.includes(flag), flag);
  for (const flag of ['--owner=0', '--group=0', '--numeric-owner', '--sort=name']) assert.ok(!ARCHIVE_FLAGS.includes(flag), flag);
  assert.deepEqual(LINUX_FILES, ['INSTALL.md', 'LICENSE', 'RUST-STDLIB-NOTICES.html', 'THIRD-PARTY-NOTICES.json', 'THIRD-PARTY-NOTICES.txt', 'bin/gil']);
});

test('the Linux tree is not assembled without its own reviewed notice inventory, and leaves no receipt', async () => {
  const scratch = await mkdtemp(join(tmpdir(), 'gil-product-artifact-'));
  try {
    const root = join(scratch, 'source');
    await mkdir(join(root, 'distribution/linux'), {recursive: true});
    await writeFile(join(root, 'LICENSE'), 'MIT\n');
    await writeFile(join(root, 'distribution/linux/INSTALL.md'), '# install\n');
    await writeFile(join(scratch, 'gil'), image());
    const output = join(scratch, 'out');
    await assert.rejects(createLinuxTree({root, binary: join(scratch, 'gil'), output, version: '0.2.0-preview.1',
      source: {head: '0'.repeat(40), dirty: false, files: 0, snapshot_sha256: '0'.repeat(64)}}));
    assert.ok(!(await readdir(output)).includes('release.json'), 'a failed assembly left a receipt');
    await assert.rejects(verifyLinuxTree(output), 'a tree without a receipt verified');
  } finally { await rm(scratch, {recursive: true, force: true}); }
});

test('inputs that are not the Linux Core are refused before anything is written', async () => {
  const scratch = await mkdtemp(join(tmpdir(), 'gil-product-artifact-'));
  try {
    const root = join(scratch, 'source');
    await mkdir(join(root, 'distribution/linux'), {recursive: true});
    await writeFile(join(root, 'LICENSE'), 'MIT\n');
    await writeFile(join(root, 'distribution/linux/INSTALL.md'), '# install\n');
    const cases = {
      'another-arch': image({machine: 183}), 'musl': image({interp: '/lib/ld-musl-x86_64.so.1'}),
      'unbundled': image({needed: ['libc.so.6', 'libwebkit2gtk-4.1.so.0'], versions: {'libc.so.6': ['GLIBC_2.17']}}),
      'mach-o': Buffer.concat([Buffer.from([0xcf, 0xfa, 0xed, 0xfe, 0x0c, 0x00, 0x00, 0x01]), Buffer.alloc(64)]),
    };
    for (const [name, bytes] of Object.entries(cases)) {
      await writeFile(join(scratch, name), bytes);
      const output = join(scratch, `out-${name}`);
      await assert.rejects(createLinuxTree({root, binary: join(scratch, name), output, version: '0.2.0-preview.1',
        source: {head: '0'.repeat(40), dirty: false, files: 0, snapshot_sha256: '0'.repeat(64)}}), `${name} was packaged`);
      assert.deepEqual(await readdir(output), [], `${name}: files were written before the refusal`);
    }
    await assert.rejects(createLinuxTree({root, binary: join(scratch, 'another-arch'), output: join(scratch, 'no-version')}), /compiled Core version/);
  } finally { await rm(scratch, {recursive: true, force: true}); }
});

test('the product tool contract refuses a catalog without the selection and approval surface', () => {
  assert.throws(() => checkProductTools([]));
  assert.throws(() => checkProductTools([{name: 'gil_start', inputSchema: {properties: {project_root: {type: 'string'}}, required: ['project_root']}}]));
  assert.equal(typeof checkDefaultTools, 'function');
});

test('the synthetic image used by the installed-binary check is a well-formed one-pixel PNG', () => {
  const bytes = png(91);
  assert.deepEqual([...bytes.subarray(0, 8)], [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  assert.equal(bytes.toString('latin1', 12, 16), 'IHDR');
  assert.equal(bytes.readUInt32BE(16), 1); assert.equal(bytes.readUInt32BE(20), 1);
  assert.equal(bytes[24], 8); assert.equal(bytes[25], 0);
  assert.equal(bytes.toString('latin1', bytes.length - 8, bytes.length - 4), 'IEND');
  assert.equal(bytes.readUInt32BE(bytes.length - 4), 0xae426082, 'the fixed CRC of an empty IEND chunk');
  assert.notDeepEqual(png(91), png(92));
});
