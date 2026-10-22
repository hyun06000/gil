import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp, readFile, writeFile, readdir} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {once} from 'node:events';
import {createInterface} from 'node:readline';
import {initialize,prepare,activate,launch,sha256} from './launcher.mjs';
const fixtures=process.env.GIL_NATIVE_FIXTURES;
assert.ok(fixtures,'Run via node run-native.mjs');
const contract={skillSha256:sha256('fixed Skill'),protocol:1,storage:5};
async function stage(root,version='v2',overrides={},name=version) {
  const bytes=await readFile(join(fixtures,`${name}.exe`));
  return prepare(root,{version,...contract,...overrides,runtime:'native'},bytes,sha256(bytes));
}
async function setup(t) {
  const parent=await mkdtemp(join(tmpdir(),'gil-native-spike-'));
  const root=join(parent,'runtime');
  const sentinel=join(parent,'synthetic-record.txt');
  await writeFile(sentinel,'preserve');
  t.after(async()=>assert.equal(await readFile(sentinel,'utf8'),'preserve'));
  await initialize(root);
  await activate(root,await stage(root,'v1'),contract,{approved:true});
  return root;
}
async function active(root) {return JSON.parse(await readFile(join(root,'active.json'))).version;}
async function session(root,t) {
  const child=await launch(root,contract);
  const lines=createInterface({input:child.stdout})[Symbol.asyncIterator]();
  // Graceful EOF: no forced termination of any installed or external process.
  t.after(async()=>{if(child.exitCode===null){const ended=once(child,'exit');child.stdin.end();await ended;}});
  return {child,lines};
}
test('native old process survives activation; new process selects v2', {timeout:10000}, async t=>{
  const root=await setup(t);
  const oldBytes=await readFile(join(root,'versions/v1/core.exe'));
  const old=await session(root,t);
  assert.equal((await old.lines.next()).value,'v1:ready');
  const next=await stage(root);
  assert.equal(await active(root),'v1');
  await activate(root,next,contract,{approved:true});
  old.child.stdin.write('alive\n');
  assert.equal((await old.lines.next()).value,'v1:alive');
  const newer=await session(root,t);
  assert.equal((await newer.lines.next()).value,'v2:ready');
  assert.deepEqual(await readFile(join(root,'versions/v1/core.exe')),oldBytes);
  assert.deepEqual((await readdir(join(root,'versions'))).sort(),['v1','v2']);
  console.log(`OBSERVED platform=${process.platform} arch=${process.arch} oldPid=${old.child.pid} newPid=${newer.child.pid} oldAlive=true oldBytesUnchanged=true`);
});
test('native preparation hash mismatch preserves v1',async t=>{
  const root=await setup(t);
  await assert.rejects(prepare(root,{version:'v2',...contract,runtime:'native'},await readFile(join(fixtures,'v2.exe')),'0'.repeat(64)),/hash/);
  assert.equal(await active(root),'v1');
});
test('native Skill mismatch refuses switch',async t=>{
  const root=await setup(t);
  await assert.rejects(activate(root,await stage(root,'v2',{skillSha256:'other'}),contract,{approved:true}),/contract/);
  assert.equal(await active(root),'v1');
});
test('native failed probe preserves runnable v1',async t=>{
  const root=await setup(t);
  await assert.rejects(activate(root,await stage(root,'v2',{},'broken'),contract,{approved:true}),/probe/);
  assert.equal(await active(root),'v1');
  const old=await session(root,t);
  assert.equal((await old.lines.next()).value,'v1:ready');
});
test('native precommit failure preserves v1',async t=>{
  const root=await setup(t);
  await assert.rejects(activate(root,await stage(root),contract,{approved:true,beforeCommit:()=>{throw Error('injected');}}),/injected/);
  assert.equal(await active(root),'v1');
});
test('native switch still requires approval',async t=>{
  const root=await setup(t);
  await assert.rejects(activate(root,await stage(root),contract),/approval/);
  assert.equal(await active(root),'v1');
});
