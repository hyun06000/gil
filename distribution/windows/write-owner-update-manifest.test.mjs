import assert from 'node:assert/strict';
import {mkdtemp, readFile, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import test from 'node:test';

const script = fileURLToPath(new URL('write-owner-update-manifest.mjs', import.meta.url));

test('writes the exact Tauri static manifest without exposing a signature path', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'gil-update-manifest-'));
  const signature = join(dir, 'candidate.exe.sig');
  const output = join(dir, 'owner-update.json');
  await writeFile(signature, 'trusted-signature-token\n');
  const url = 'https://github.com/hyun06000/gil/releases/download/windows-owner-update-2/candidate.exe';
  const result = spawnSync(process.execPath, [script, '0.1.1-owner.2', url, signature, output], {encoding: 'utf8'});
  assert.equal(result.status, 0, result.stderr);
  const manifest = JSON.parse(await readFile(output, 'utf8'));
  assert.equal(manifest.version, '0.1.1-owner.2');
  assert.deepEqual(Object.keys(manifest.platforms), ['windows-x86_64']);
  assert.equal(manifest.platforms['windows-x86_64'].url, url);
  assert.equal(manifest.platforms['windows-x86_64'].signature, 'trusted-signature-token');
  assert.ok(!JSON.stringify(manifest).includes(signature));
});

test('rejects non-monotonic owner versions and non-HTTPS assets', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'gil-update-manifest-reject-'));
  const signature = join(dir, 'candidate.sig');
  await writeFile(signature, 'one-token');
  for (const [version, url] of [
    ['0.1.1-owner.hash', 'https://example.test/update.exe'],
    ['0.1.1-owner.2', 'http://example.test/update.exe'],
  ]) {
    const result = spawnSync(process.execPath, [script, version, url, signature, join(dir, `${Math.random()}.json`)], {encoding: 'utf8'});
    assert.notEqual(result.status, 0);
  }
});
