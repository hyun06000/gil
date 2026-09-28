import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp, realpath, rm, readFile, writeFile, mkdir, unlink, symlink} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {noticeFixture} from './test-fixture.mjs';
import {DIRECTORY, INPUTS, OUTPUTS, repository, sha256, lockChecksums, legalFiles,
  validatePolicy, assertCoverage, loadBundle, check} from './notices.mjs';

async function fixture(t) {
  const root = await realpath(await mkdtemp(join(tmpdir(), 'gil-notices-test-')));
  t.after(() => rm(root, {recursive: true, force: true}));
  return {root, ...await noticeFixture(root)};
}
function observed(policy) { const {stdlib, ...rest} = structuredClone(policy); return rest; }
test('deterministic assembly retains CRLF, missing final newline and attribution text', async t => {
  const f = await fixture(t), a = await loadBundle(f.root), b = await loadBundle(f.root);
  assert.deepEqual(a, b); assert.deepEqual([...a.keys()], OUTPUTS);
  assert.ok(a.get(OUTPUTS[0]).includes(Buffer.from(f.original)));
  assert.ok(a.get(OUTPUTS[2]).equals(Buffer.from(f.stdlib)));
  assert.equal(a.get(OUTPUTS[0]).toString().includes(f.root), false);
});
test('each locked input is enforced, including actual embedded UI and build recipe', async t => {
  const f = await fixture(t);
  for (const input of INPUTS) {
    const path = join(f.root, input), saved = await readFile(path);
    await writeFile(path, 'changed'); await assert.rejects(loadBundle(f.root), /notice policy stale/);
    await writeFile(path, saved);
  }
});
test('missing original blocks packaging; no metadata-only fallback', async t => {
  const f = await fixture(t);
  await unlink(join(f.root, DIRECTORY, 'texts', `${f.textHash}.json`));
  await assert.rejects(loadBundle(f.root), /missing or unreferenced/);
});
test('altered original text cannot keep its previous approval hash', async t => {
  const f = await fixture(t);
  await writeFile(join(f.root, DIRECTORY, 'texts', `${f.textHash}.json`), JSON.stringify({text: 'replacement'}));
  await assert.rejects(loadBundle(f.root), /modified legal text/);
});
test('unused legal files cannot silently grow the published surface', async t => {
  const f = await fixture(t);
  await writeFile(join(f.root, DIRECTORY, 'texts', 'extra.json'), '{"text":"unused"}');
  await assert.rejects(loadBundle(f.root), /missing or unreferenced/);
});
test('symlink originals cannot escape the reviewed directory', async t => {
  const f = await fixture(t), p = join(f.root, DIRECTORY, 'texts', `${f.textHash}.json`);
  await unlink(p); await symlink(join(f.root, 'Cargo.toml'), p);
  await assert.rejects(loadBundle(f.root), /symlink/);
});
test('recursive discovery includes nested NOTICE and Unicode license files', async t => {
  const root = await realpath(await mkdtemp(join(tmpdir(), 'gil-legal-tree-')));
  t.after(() => rm(root, {recursive: true, force: true}));
  await mkdir(join(root, 'src/unicode'), {recursive: true});
  await writeFile(join(root, 'LICENSE'), 'root text');
  await writeFile(join(root, 'src/unicode/LICENSE-UNICODE'), 'unicode text');
  await writeFile(join(root, 'src/NOTICE.txt'), 'attribution');
  await writeFile(join(root, 'src/code.rs'), 'not a legal file');
  assert.deepEqual((await legalFiles(root)).map(f => f.path), ['LICENSE', 'src/NOTICE.txt', 'src/unicode/LICENSE-UNICODE']);
  await symlink(join(root, 'LICENSE'), join(root, 'COPYING'));
  await assert.rejects(legalFiles(root), /symlink/);
});
test('new or removed dependency requires coverage review', async t => {
  const f = await fixture(t), next = observed(f.policy);
  assertCoverage(f.policy, next);
  next.packages.push({...next.packages[0], name: 'unreviewed'});
  assert.throws(() => assertCoverage(f.policy, next), /coverage changed/);
  next.packages = []; assert.throws(() => assertCoverage(f.policy, next), /coverage changed/);
});
test('package version, role, source integrity and declared license cannot drift', async t => {
  const f = await fixture(t);
  for (const [field, value] of [['version', '9.0.0'], ['role', 'build-or-proc-macro'],
    ['integrity', '3'.repeat(64)], ['declared_license', 'Apache-2.0'], ['source', 'https://example.invalid/other']]) {
    const next = observed(f.policy); next.packages[0][field] = value;
    assert.throws(() => assertCoverage(f.policy, next), /coverage changed/);
  }
});
test('new nested notice or changed license bytes require review even at the same version', async t => {
  const f = await fixture(t), next = observed(f.policy);
  next.packages[0].files.push({path: 'src/NOTICE', sha256: sha256('new notice')});
  assert.throws(() => assertCoverage(f.policy, next), /coverage changed/);
  next.packages[0].files = [{path: 'LICENSE', sha256: sha256('different license')}];
  assert.throws(() => assertCoverage(f.policy, next), /coverage changed/);
});
test('Rust toolchain changes require standard library notice review', async t => {
  const f = await fixture(t), next = observed(f.policy);
  next.toolchain.commit = '4'.repeat(40);
  assert.throws(() => assertCoverage(f.policy, next), /coverage changed/);
});
test('empty, duplicate, unsafe or unlicensed inventory cannot claim coverage', async t => {
  const f = await fixture(t);
  for (const alter of [p => p.packages = [], p => p.packages.push(p.packages[0]),
    p => p.packages[0].files = [], p => p.packages[0].declared_license = null,
    p => p.packages[0].files[0].path = '../LICENSE', p => p.packages[0].files[0].path = '/tmp/LICENSE']) {
    const p = structuredClone(f.policy); alter(p); assert.throws(() => validatePolicy(p));
  }
});
test('Cargo checksum parser refuses missing checksums and ambiguous registry versions', () => {
  const block = `[[package]]\nname = "x"\nversion = "1.0.0"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\nchecksum = "${'a'.repeat(64)}"\n`;
  assert.equal(lockChecksums(block).get('x@1.0.0'), 'a'.repeat(64));
  assert.throws(() => lockChecksums(block.replace(/^checksum.*$/m, '')));
  assert.throws(() => lockChecksums(block + block));
  assert.throws(() => lockChecksums(''));
});
test('real policy preserves MCP transition terms rather than replacing them with MIT metadata', async () => {
  const bundle = await loadBundle(repository), policy = JSON.parse(bundle.get(OUTPUTS[1]));
  const sdk = policy.packages.find(p => p.name === '@modelcontextprotocol/ext-apps');
  assert.equal(sdk.declared_license, 'MIT');
  const text = JSON.parse(await readFile(join(repository, DIRECTORY, 'texts', `${sdk.files[0].sha256}.json`))).text;
  assert.ok(bundle.get(OUTPUTS[0]).includes(Buffer.from(text)));
  assert.match(text, /licensing transition/); assert.match(text, /Apache-2\.0/); assert.match(text, /remain licensed under the MIT License/);
  const rmcp = policy.packages.find(p => p.name === 'rmcp');
  assert.equal(rmcp.upstream.commit, 'fd7811fdaa9fefa1c8034534b4d7a31c97204f89');
  assert.equal(rmcp.files[0].sha256, sdk.files[0].sha256);
  assert.ok(policy.packages.find(p => p.name === 'regex-syntax').files.some(f => f.path === 'src/unicode_tables/LICENSE-UNICODE'));
});
test('actual locked Core and bundled UI match reviewed offline coverage', async () => {
  const result = await check(repository);
  assert.equal(result.packages, 84); assert.equal(result.files.length, 3);
});
