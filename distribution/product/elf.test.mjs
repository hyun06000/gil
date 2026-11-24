import test from 'node:test';
import assert from 'node:assert/strict';
import {checkElf, compareVersions, readElf, INTERPRETER} from './elf.mjs';
import {image} from './elf-fixture.mjs';

test('the reader reports the interpreter, NEEDED and versioned symbols the bytes declare', () => {
  const elf = readElf(image());
  assert.equal(elf.type, 'pie');
  assert.equal(elf.interpreter, INTERPRETER);
  assert.deepEqual(elf.needed, ['libc.so.6', 'libgcc_s.so.1']);
  assert.deepEqual(elf.versions, {'libc.so.6': ['GLIBC_2.2.5', 'GLIBC_2.34'], 'libgcc_s.so.1': ['GCC_3.0']});
  const report = checkElf(image());
  assert.equal(report.glibc_required, '2.34', 'the floor is the highest GLIBC version the binary binds');
  assert.equal(report.format, 'elf64-x86-64');
});

test('versions compare numerically, not as text', () => {
  assert.equal(compareVersions('2.9', '2.17'), -1);
  assert.equal(compareVersions('2.34', '2.34'), 0);
  assert.equal(compareVersions('2.2.5', '2.2'), 1);
  assert.equal(checkElf(image({versions: {'libc.so.6': ['GLIBC_2.9', 'GLIBC_2.17', 'GLIBC_2.2.5']}})).glibc_required, '2.17');
  assert.equal(checkElf(image({versions: {'libc.so.6': ['GLIBC_2.34', 'GLIBC_ABI_DT_RELR']}})).glibc_required, '2.36');
});

test('another architecture, class, loader or an unbundled library is refused', () => {
  assert.throws(() => readElf(image({machine: 183})), /x86-64/);
  assert.throws(() => readElf(image({cls: 1})), /ELF64/);
  assert.throws(() => readElf(Buffer.from('not an elf image, only some text that is long enough to pass the size check')), /ELF image/);
  assert.throws(() => checkElf(image({interp: '/lib/ld-musl-x86_64.so.1'})), /GNU dynamic loader/);
  assert.throws(() => checkElf(image({needed: ['libc.so.6', 'libssl.so.3'], versions: {'libc.so.6': ['GLIBC_2.17']}})), /unbundled non-system library: libssl\.so\.3/);
  assert.throws(() => checkElf(image({runpath: '/home/builder/lib'})), /RUNPATH/);
  assert.throws(() => checkElf(image({rpath: '$ORIGIN/lib'})), /RPATH/);
  assert.throws(() => checkElf(image({versions: {'libc.so.6': ['GLIBC_PRIVATE']}})), /unreviewed symbol version/);
  assert.throws(() => checkElf(image({versions: {'libfoo.so.1': ['GLIBC_2.17']}})), /not NEEDED/);
  assert.throws(() => checkElf(image({versions: {}})), /run-time floor/);
});

test('a stated ceiling refuses a binary that needs a newer glibc, and no ceiling makes no claim', () => {
  assert.equal(checkElf(image(), {ceiling: '2.35'}).glibc_required, '2.34');
  assert.throws(() => checkElf(image(), {ceiling: '2.31'}), /requires glibc 2\.34, above the stated 2\.31/);
});

test('truncated or inconsistent images fail instead of being read past their end', () => {
  const whole = image();
  assert.throws(() => readElf(whole.subarray(0, 60)));
  assert.throws(() => readElf(whole.subarray(0, whole.length - 20)));
  const noTerminator = Buffer.from(whole); noTerminator.fill(0x41, noTerminator.length - 16 * 6);
  assert.throws(() => readElf(noTerminator));
});
