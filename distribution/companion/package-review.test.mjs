import {test} from 'node:test';
import assert from 'node:assert/strict';
import {peMachine,receipt,packageReview} from './package-review.mjs';
import {readFile,mkdtemp,writeFile,access,rm,realpath} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {sha256} from '../compliance/notices.mjs';
function image(machine){const b=Buffer.alloc(300);b.write('MZ');b.writeUInt32LE(128,0x3c);b.writeUInt32LE(0x4550,128);b.writeUInt16LE(machine,132);b.writeUInt16LE(0x20b,152);return b;}
test('native PE CPU is explicit; malformed and mismatched inputs refuse',()=>{
  peMachine(image(0x8664),'x86_64');peMachine(image(0xaa64),'aarch64');
  for(const b of [Buffer.alloc(0),Buffer.alloc(300),image(0xaa64)])assert.throws(()=>peMachine(b,'x86_64'));
  const b=image(0x8664);b.writeUInt32LE(0xffffffff,0x3c);assert.throws(()=>peMachine(b,'x86_64'));
});
test('receipt binds exact Companion bytes without adding product schema fields',()=>{
  const r=receipt(Buffer.from('one'));assert.deepEqual(Object.keys(r),['schema','protocol','companion_file','companion_sha256']);
  assert.equal(r.companion_file,'gil-companion.exe');assert.match(r.companion_sha256,/^[a-f0-9]{64}$/);assert.notEqual(r.companion_sha256,receipt(Buffer.from('two')).companion_sha256);
});
test('review app identity is separate and installer bundling is disabled',async()=>{
  const c=JSON.parse(await readFile(new URL('./review-tauri.json',import.meta.url)));assert.equal(c.identifier,'dev.ariadne.gil.companion.review');assert.equal(c.bundle.active,false);
});
test('wrong build identity or tampered executable refuses before output creation',async()=>{
  const root=await mkdtemp(join(await realpath(tmpdir()),'gil-review-pack-'));
  try {
    const b=image(0x8664),files={};for(const name of ['gil.exe','gil-companion.exe','gil-runtime-testbed.exe']){await writeFile(join(root,name),b);files[name]=sha256(b);}
    const args={repo:root,bins:root,out:join(root,'output'),arch:'x86_64',source:'a'.repeat(40)};
    await writeFile(join(root,'review-build.json'),JSON.stringify({identifier:'dev.ariadne.gil.companion',source:args.source,files}));
    await assert.rejects(packageReview(args));await assert.rejects(access(args.out));
    await writeFile(join(root,'review-build.json'),JSON.stringify({identifier:'dev.ariadne.gil.companion.review',source:args.source,files}));
    await writeFile(join(root,'gil.exe'),image(0xaa64));
    await assert.rejects(packageReview(args));await assert.rejects(access(args.out));
  } finally {await rm(root,{recursive:true});}
});
test('offline fixture assembly covers both binaries, receipt, original notices and every member hash',async()=>{
  const root=await mkdtemp(join(await realpath(tmpdir()),'gil-review-pack-'));
  try {
    const arch=process.arch==='arm64'?'aarch64':'x86_64';
    const b=image(arch==='aarch64'?0xaa64:0x8664),files={};for(const name of ['gil.exe','gil-companion.exe','gil-runtime-testbed.exe']){await writeFile(join(root,name),b);files[name]=sha256(b);}
    const source='a'.repeat(40),out=join(root,'output');
    await writeFile(join(root,'review-build.json'),JSON.stringify({identifier:'dev.ariadne.gil.companion.review',source,files}));
    const args={repo:resolve(import.meta.dirname,'../..'),bins:root,out,arch,source};
    await packageReview(args);
    const manifest=JSON.parse(await readFile(join(out,'review-bundle.json')));assert.equal(manifest.publishable,false);
    for(const line of (await readFile(join(out,'SHA256SUMS'),'utf8')).trim().split('\n')){const [hash,name]=line.split('  ');assert.equal(sha256(await readFile(join(out,name))),hash);}
    const notice=JSON.parse(await readFile(join(out,'THIRD-PARTY-NOTICES.json')));
    assert.ok(notice.packages.some(p=>p.name==='tauri'));assert.ok(notice.packages.some(p=>p.name==='webview2-com'));
    assert.equal(JSON.parse(await readFile(join(out,'companion-install.json'))).companion_sha256,files['gil-companion.exe']);
    const before=await readFile(join(out,'review-bundle.json'));await assert.rejects(packageReview(args));assert.deepEqual(await readFile(join(out,'review-bundle.json')),before);
  } finally {await rm(root,{recursive:true});}
});
