import assert from 'node:assert/strict';
import test from 'node:test';
import {readFile} from 'node:fs/promises';
import {inspectPE, noBuildPaths, TARGET} from './candidate.mjs';
import {loadBundle, validatePolicy, collect, assertCoverage} from '../compliance/notices.mjs';

function pe(dll = 'KERNEL32.dll') {
  const b = Buffer.alloc(1024); b.write('MZ'); b.writeUInt32LE(128, 60);
  b.writeUInt32LE(0x4550, 128); b.writeUInt16LE(0x8664, 132); b.writeUInt16LE(1, 134);
  b.writeUInt16LE(240, 148); b.writeUInt16LE(0x20b, 152); b.writeUInt32LE(16, 260);
  b.writeUInt32LE(0x1000, 272); b.writeUInt32LE(0x1000, 404);
  b.writeUInt32LE(512, 408); b.writeUInt32LE(512, 412);
  b.writeUInt32LE(0x1080, 524); b.write(dll, 640); return b;
}
test('x64 PE system imports accepted', () => assert.deepEqual(inspectPE(pe()), ['kernel32.dll']));
test('external runtime and unknown imports refused', () => {
  for (const name of ['VCRUNTIME140.dll', 'node.dll', 'unexpected.dll']) assert.throws(() => inspectPE(pe(name)));
});
test('ARM, malformed and delayed imports refused', () => {
  const arm = pe(); arm.writeUInt16LE(0xaa64, 132); assert.throws(() => inspectPE(arm));
  const delayed = pe(); delayed.writeUInt32LE(0x1000, 368); assert.throws(() => inspectPE(delayed));
  assert.throws(() => inspectPE(Buffer.alloc(5)));
  assert.throws(() => inspectPE(pe().subarray(0, 550)));
});
test('machine paths rejected in UTF-8 and UTF-16', () => {
  for (const s of ['C:\\Users\\alice', 'D:\\a\\gil', '/Users/alice', '/home/runner'])
    for (const encoding of ['utf8', 'utf16le']) assert.throws(() => noBuildPaths(Buffer.from(s, encoding)));
  noBuildPaths(Buffer.from('/gil/src/main.rs'));
});
test('Windows has its own pinned notice inventory and original texts', async () => {
  const directory = 'distribution/windows/compliance';
  const policy = JSON.parse(await readFile(new URL('./compliance/notice-policy.json', import.meta.url)));
  validatePolicy(policy, TARGET);
  assert.throws(() => validatePolicy(policy));
  assert.equal(policy.packages.length, 89);
  const bundle = await loadBundle(undefined, {directory, target: TARGET});
  assert.equal(bundle.size, 3);
  assert.match(bundle.get('THIRD-PARTY-NOTICES.txt').toString(), /windows-sys@0.61.2/);
  assertCoverage(policy, await collect(undefined, {target: TARGET}), TARGET);
});
