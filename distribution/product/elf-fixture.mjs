// Test data only: a minimal synthetic ELF64 x86-64 image for the reader and the packaging checks.
import {INTERPRETER} from './elf.mjs';

// A minimal synthetic ELF64 x86-64 image: one PT_LOAD covering the whole file (vaddr == offset),
// PT_INTERP, PT_DYNAMIC, a string table and one verneed chain. It is data for the reader, not a
// runnable program.
export function image({machine = 62, cls = 2, type = 3, interp = INTERPRETER, needed = ['libc.so.6', 'libgcc_s.so.1'],
  versions = {'libc.so.6': ['GLIBC_2.2.5', 'GLIBC_2.34'], 'libgcc_s.so.1': ['GCC_3.0']}, rpath, runpath, dynamic = true} = {}) {
  const strings = ['\0'];
  let size = 1;
  const intern = text => { const at = size; strings.push(text + '\0'); size += Buffer.byteLength(text) + 1; return at; };
  const dyn = [];
  for (const name of needed) dyn.push([1, intern(name)]);
  if (rpath) dyn.push([15, intern(rpath)]);
  if (runpath) dyn.push([29, intern(runpath)]);
  const verneed = [];
  const files = Object.entries(versions);
  files.forEach(([file, names], i) => {
    const head = Buffer.alloc(16);
    head.writeUInt16LE(1, 0); head.writeUInt16LE(names.length, 2); head.writeUInt32LE(intern(file), 4);
    head.writeUInt32LE(16, 8); head.writeUInt32LE(i + 1 < files.length ? 16 + 16 * names.length : 0, 12);
    verneed.push(head);
    names.forEach((name, j) => {
      const aux = Buffer.alloc(16);
      aux.writeUInt32LE(intern(name), 8); aux.writeUInt32LE(j + 1 < names.length ? 16 : 0, 12);
      verneed.push(aux);
    });
  });
  const strtab = Buffer.from(strings.join(''), 'latin1');
  const interpBytes = Buffer.from(interp + '\0', 'latin1');
  const phnum = dynamic ? 3 : 2, phoff = 64;
  const interpAt = phoff + 56 * phnum, strAt = interpAt + interpBytes.length;
  const verAt = strAt + strtab.length, verBytes = Buffer.concat(verneed);
  const dynAt = verAt + verBytes.length;
  dyn.push([5, strAt], [10, strtab.length]);
  if (files.length) dyn.push([0x6ffffffe, verAt], [0x6fffffff, files.length]);
  dyn.push([0, 0]);
  const dynBytes = Buffer.alloc(16 * dyn.length);
  dyn.forEach(([tag, value], i) => { dynBytes.writeBigUInt64LE(BigInt(tag), 16 * i); dynBytes.writeBigUInt64LE(BigInt(value), 16 * i + 8); });
  const total = dynAt + dynBytes.length;
  const header = Buffer.alloc(64);
  header.writeUInt32BE(0x7f454c46, 0); header[4] = cls; header[5] = 1; header[6] = 1;
  header.writeUInt16LE(type, 16); header.writeUInt16LE(machine, 18);
  header.writeBigUInt64LE(BigInt(phoff), 32); header.writeUInt16LE(56, 54); header.writeUInt16LE(phnum, 56);
  const ph = (kind, offset, filesz) => {
    const b = Buffer.alloc(56);
    b.writeUInt32LE(kind, 0); b.writeBigUInt64LE(BigInt(offset), 8); b.writeBigUInt64LE(BigInt(offset), 16); b.writeBigUInt64LE(BigInt(filesz), 32);
    return b;
  };
  const headers = [ph(1, 0, total), ph(3, interpAt, interpBytes.length)];
  if (dynamic) headers.push(ph(2, dynAt, dynBytes.length));
  return Buffer.concat([header, ...headers, interpBytes, strtab, verBytes, dynBytes]);
}
