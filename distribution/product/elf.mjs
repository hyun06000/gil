// Maintainer tooling only. A dependency-free reader of the facts a Linux x86-64 executable
// declares about what it needs at run time: its interpreter, its NEEDED libraries and the
// versioned symbols it binds. It reads bytes; it never executes or loads the file.
import assert from 'node:assert/strict';

const PT_LOAD = 1, PT_DYNAMIC = 2, PT_INTERP = 3;
const DT_NEEDED = 1, DT_STRTAB = 5, DT_STRSZ = 10, DT_RPATH = 15, DT_RUNPATH = 29,
  DT_VERNEED = 0x6ffffffe, DT_VERNEEDNUM = 0x6fffffff;
const BOUND = 4096; // entries; a real executable is far below this, a malformed one must not loop

function u64(bytes, at) {
  assert.ok(at >= 0 && at + 8 <= bytes.length, 'ELF field outside the file');
  const value = bytes.readBigUInt64LE(at);
  assert.ok(value <= BigInt(Number.MAX_SAFE_INTEGER), 'ELF offset too large');
  return Number(value);
}
const u32 = (bytes, at) => { assert.ok(at >= 0 && at + 4 <= bytes.length, 'ELF field outside the file'); return bytes.readUInt32LE(at); };
const u16 = (bytes, at) => { assert.ok(at >= 0 && at + 2 <= bytes.length, 'ELF field outside the file'); return bytes.readUInt16LE(at); };
function cstring(bytes, at, limit = bytes.length) {
  assert.ok(at >= 0 && at < limit && limit <= bytes.length, 'ELF string outside its table');
  const end = bytes.indexOf(0, at);
  assert.ok(end >= at && end < limit, 'unterminated ELF string');
  const text = bytes.toString('latin1', at, end);
  assert.match(text, /^[\x20-\x7e]*$/, 'non-printable ELF string');
  return text;
}

// What the file declares. Throws on anything that is not a little-endian ELF64 x86-64 image.
export function readElf(bytes) {
  assert.ok(Buffer.isBuffer(bytes) && bytes.length >= 64, 'ELF header missing');
  assert.equal(bytes.readUInt32BE(0), 0x7f454c46, 'expected an ELF image');
  assert.equal(bytes[4], 2, 'expected ELF64');
  assert.equal(bytes[5], 1, 'expected little-endian');
  assert.equal(bytes[6], 1, 'unknown ELF version');
  const type = u16(bytes, 16), machine = u16(bytes, 18);
  assert.equal(machine, 62, 'expected x86-64, not another architecture');
  assert.ok(type === 2 || type === 3, 'expected an executable (ET_EXEC or position-independent ET_DYN)');
  const phoff = u64(bytes, 32), phentsize = u16(bytes, 54), phnum = u16(bytes, 56);
  assert.equal(phentsize, 56, 'unexpected program header size');
  assert.ok(phnum > 0 && phnum < BOUND, 'no program headers');
  const loads = [];
  let interpreter = null, dynamic = null;
  for (let i = 0; i < phnum; i++) {
    const at = phoff + i * phentsize;
    const kind = u32(bytes, at), offset = u64(bytes, at + 8), vaddr = u64(bytes, at + 16), filesz = u64(bytes, at + 32);
    assert.ok(offset + filesz <= bytes.length, 'segment outside the file');
    if (kind === PT_LOAD) loads.push({offset, vaddr, filesz});
    else if (kind === PT_INTERP) { assert.equal(interpreter, null, 'two interpreters'); interpreter = cstring(bytes, offset, offset + filesz); }
    else if (kind === PT_DYNAMIC) { assert.equal(dynamic, null, 'two dynamic segments'); dynamic = {offset, filesz}; }
  }
  const fileOffset = vaddr => {
    const segment = loads.find(s => vaddr >= s.vaddr && vaddr < s.vaddr + s.filesz);
    assert.ok(segment, 'dynamic address outside every loaded segment');
    return segment.offset + (vaddr - segment.vaddr);
  };
  const out = {type: type === 3 ? 'pie' : 'exec', interpreter, needed: [], rpath: null, runpath: null, versions: {}};
  if (!dynamic) return out; // statically linked: no interpreter facts beyond the header
  const entries = [];
  for (let at = dynamic.offset, n = 0; at + 16 <= dynamic.offset + dynamic.filesz; at += 16, n++) {
    assert.ok(n < BOUND, 'dynamic section too long');
    const tag = u64(bytes, at), value = u64(bytes, at + 8);
    if (tag === 0) break;
    entries.push([tag, value]);
  }
  const one = tag => entries.find(([t]) => t === tag)?.[1];
  const strtab = one(DT_STRTAB), strsz = one(DT_STRSZ);
  assert.ok(strtab !== undefined && strsz !== undefined, 'dynamic string table missing');
  const table = fileOffset(strtab);
  const name = offset => cstring(bytes, table + offset, table + strsz);
  for (const [tag, value] of entries) {
    if (tag === DT_NEEDED) out.needed.push(name(value));
    else if (tag === DT_RPATH) out.rpath = name(value);
    else if (tag === DT_RUNPATH) out.runpath = name(value);
  }
  const verneed = one(DT_VERNEED), count = one(DT_VERNEEDNUM) ?? 0;
  if (verneed !== undefined) {
    assert.ok(count < BOUND, 'too many version requirements');
    let at = fileOffset(verneed);
    for (let i = 0; i < count; i++) {
      const auxCount = u16(bytes, at + 2), file = name(u32(bytes, at + 4)), auxAt = u32(bytes, at + 8), next = u32(bytes, at + 12);
      assert.ok(auxCount < BOUND, 'too many versions for one library');
      const names = out.versions[file] ??= [];
      let aux = at + auxAt;
      for (let j = 0; j < auxCount; j++) {
        names.push(name(u32(bytes, aux + 8)));
        const step = u32(bytes, aux + 12);
        if (step === 0) break;
        aux += step;
      }
      if (next === 0) break;
      at += next;
    }
  }
  for (const names of Object.values(out.versions)) names.sort();
  return out;
}

const numeric = version => version.split('.').map(part => { assert.match(part, /^\d+$/, 'unrecognized version'); return Number(part); });
export function compareVersions(a, b) {
  const x = numeric(a), y = numeric(b);
  for (let i = 0; i < Math.max(x.length, y.length); i++) if ((x[i] ?? 0) !== (y[i] ?? 0)) return (x[i] ?? 0) < (y[i] ?? 0) ? -1 : 1;
  return 0;
}

// The glibc/loader interface only. A library outside this list is an unbundled dependency the
// user's machine may not have, so the artifact is refused instead of documented as "needs X".
export const SYSTEM_LIBRARIES = ['ld-linux-x86-64.so.2', 'libc.so.6', 'libdl.so.2', 'libgcc_s.so.1', 'libm.so.6', 'libpthread.so.0', 'librt.so.1'];
export const INTERPRETER = '/lib64/ld-linux-x86-64.so.2';

// The reviewed shape of the Linux x86-64 GNU Core, and the run-time floor the bytes themselves
// state. `glibc_required` is read from the binary; it is recorded, never assumed from the Rust
// target's own minimum. `ceiling` (optional) refuses a binary that needs a newer glibc than a
// support statement allows; without it this function makes no support claim at all.
export function checkElf(bytes, {ceiling} = {}) {
  const elf = readElf(bytes);
  assert.equal(elf.interpreter, INTERPRETER, 'expected the x86-64 GNU dynamic loader');
  assert.ok(elf.needed.length > 0, 'no NEEDED evidence');
  assert.equal(new Set(elf.needed).size, elf.needed.length, 'duplicate NEEDED entry');
  for (const library of elf.needed) assert.ok(SYSTEM_LIBRARIES.includes(library), `native artifact depends on an unbundled non-system library: ${library}`);
  assert.equal(elf.rpath, null, 'RPATH would load libraries from a build-time location');
  assert.equal(elf.runpath, null, 'RUNPATH would load libraries from a build-time location');
  let floor = null;
  for (const [library, names] of Object.entries(elf.versions)) {
    assert.ok(elf.needed.includes(library), `versioned symbols from a library that is not NEEDED: ${library}`);
    for (const symbol of names) {
      // The DT_RELR marker is a requirement on the loader itself, introduced with glibc 2.36.
      const [family, version] = symbol === 'GLIBC_ABI_DT_RELR' ? ['GLIBC', '2.36'] : symbol.split('_');
      assert.ok(['GLIBC', 'GCC'].includes(family) && /^\d+(\.\d+)*$/.test(version ?? '') && symbol.split('_').length <= (symbol === 'GLIBC_ABI_DT_RELR' ? 4 : 2),
        `unreviewed symbol version: ${symbol}`);
      if (family === 'GLIBC' && (floor === null || compareVersions(version, floor) > 0)) floor = version;
    }
  }
  assert.ok(floor, 'no GLIBC version requirement found; cannot state a run-time floor');
  if (ceiling !== undefined) assert.ok(compareVersions(floor, ceiling) <= 0, `binary requires glibc ${floor}, above the stated ${ceiling}`);
  return {format: 'elf64-x86-64', type: elf.type, interpreter: elf.interpreter, needed: [...elf.needed].sort(), versions: elf.versions, glibc_required: floor};
}
