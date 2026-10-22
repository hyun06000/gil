import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp, readFile, writeFile, readdir} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {once} from 'node:events';
import {createInterface} from 'node:readline';
import {initialize, prepare, activate, launch, sha256} from './launcher.mjs';

const contract = {skillSha256: sha256('fixed installed Skill'), protocol: 1, storage: 5};
const fixture = (version, broken = false) => Buffer.from(`
import {createInterface} from 'node:readline';
if (process.argv.includes('--probe')) {
  ${broken ? 'process.exit(7);' : `console.log(JSON.stringify({version:'${version}'}));`}
} else {
  console.log('${version}:ready');
  createInterface({input:process.stdin}).on('line', text => console.log('${version}:'+text));
}
`);
async function setup(t) {
  const parent = await mkdtemp(join(tmpdir(), 'gil-launcher-spike-'));
  const root = join(parent, 'runtime');
  await initialize(root);
  const project = join(parent, 'synthetic-project-record.txt');
  await writeFile(project, 'DO NOT MODIFY');
  t.after(async () => assert.equal(await readFile(project, 'utf8'), 'DO NOT MODIFY'));
  const bytes = fixture('v1');
  const first = await prepare(root, {version: 'v1', ...contract}, bytes, sha256(bytes));
  await activate(root, first, contract, {approved: true});
  return {root, first};
}
async function candidate(root, overrides = {}, broken = false) {
  const bytes = fixture('v2', broken);
  return prepare(root, {version:'v2', ...contract, ...overrides}, bytes, sha256(bytes));
}
async function active(root) {return JSON.parse(await readFile(join(root, 'active.json'), 'utf8')).version;}
async function session(root, t) {
  const child = await launch(root, contract);
  const lines = createInterface({input: child.stdout})[Symbol.asyncIterator]();
  t.after(async () => {if (child.exitCode === null) {const ended = once(child,'exit'); child.stdin.end(); await ended;}});
  return {child, lines};
}

test('running v1 survives preparation and activation; new launch selects v2', async t => {
  const {root} = await setup(t);
  const oldHash = sha256(await readFile(join(root,'versions/v1/core.mjs')));
  const old = await session(root, t);
  assert.equal((await old.lines.next()).value, 'v1:ready');
  const next = await candidate(root);
  assert.equal(await active(root), 'v1');
  await activate(root, next, contract, {approved:true});
  old.child.stdin.write('still-alive\n');
  assert.equal((await old.lines.next()).value, 'v1:still-alive');
  const newer = await session(root, t);
  assert.equal((await newer.lines.next()).value, 'v2:ready');
  assert.equal(sha256(await readFile(join(root,'versions/v1/core.mjs'))), oldHash);
});
test('bad download cannot replace active selection', async t => {
  const {root} = await setup(t);
  await assert.rejects(prepare(root,{version:'v2',...contract},fixture('v2'),'0'.repeat(64)), /hash/);
  assert.equal(await active(root),'v1');
  assert.deepEqual(await readdir(join(root,'versions')),['v1']);
});
test('Skill mismatch blocks activation', async t => {
  const {root} = await setup(t);
  const next = await candidate(root,{skillSha256:sha256('different Skill')});
  await assert.rejects(activate(root,next,contract,{approved:true}), /contract/);
  assert.equal(await active(root),'v1');
});
test('approval is required', async t => {
  const {root} = await setup(t);
  await assert.rejects(activate(root,await candidate(root),contract), /approval/);
  assert.equal(await active(root),'v1');
});
test('preflight failure leaves old selection usable', async t => {
  const {root} = await setup(t);
  await assert.rejects(activate(root,await candidate(root,{},true),contract,{approved:true}), /probe/);
  assert.equal(await active(root),'v1');
  const old = await session(root,t);
  assert.equal((await old.lines.next()).value,'v1:ready');
});
test('injected failure before pointer commit preserves prior selection', async t => {
  const {root} = await setup(t);
  await assert.rejects(activate(root,await candidate(root),contract,{approved:true,
    beforeCommit:()=>{throw new Error('injected storage failure');}}), /injected/);
  assert.equal(await active(root),'v1');
});
test('existing version cannot be overwritten', async t => {
  const {root} = await setup(t);
  const before = await readFile(join(root,'versions/v1/core.mjs'));
  await assert.rejects(prepare(root,{version:'v1',...contract},fixture('v2'),sha256(fixture('v2'))));
  assert.deepEqual(await readFile(join(root,'versions/v1/core.mjs')),before);
});
test('tampered selected Core is refused, not silently rolled back', async t => {
  const {root} = await setup(t);
  await activate(root,await candidate(root),contract,{approved:true});
  await writeFile(join(root,'versions/v2/core.mjs'),'tampered');
  await assert.rejects(launch(root,contract), /hash/);
  assert.equal(await active(root),'v2');
});
test('storage contract change is rejected without migration', async t => {
  const {root} = await setup(t);
  await assert.rejects(activate(root,await candidate(root,{storage:6}),contract,{approved:true}), /contract/);
  assert.equal(await active(root),'v1');
});
test('concurrent activation cannot steal the update lock', async t => {
  const {root} = await setup(t);
  const next = await candidate(root);
  let entered, release;
  const atCommit = new Promise(resolve=>{entered=resolve;});
  const gate = new Promise(resolve=>{release=resolve;});
  const pending = activate(root,next,contract,{approved:true,beforeCommit:async()=>{entered();await gate;}});
  await atCommit;
  try {
    await assert.rejects(activate(root,next,contract,{approved:true}), {code:'EEXIST'});
    assert.equal(await active(root),'v1');
  } finally {release();await pending;}
  assert.equal(await active(root),'v2');
});
