import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtempSync, readFileSync, rmSync, writeFileSync, existsSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {freshTemp} from './fresh-temp.mjs';

test('a profile group gets a new temporary folder for TMPDIR, TEMP and TMP', t => {
  const base = mkdtempSync(join(tmpdir(), 'gil-fresh-temp-'));
  t.after(() => rmSync(base, {recursive: true, force: true}));
  const envFile = join(base, 'env');
  writeFileSync(envFile, 'KEEP=1\n');
  const folder = freshTemp('end-history', {base, envFile});
  assert.equal(folder, join(base, 'gil-ci-end-history'));
  assert.ok(existsSync(folder));
  assert.equal(readFileSync(envFile, 'utf8'), `KEEP=1\nTMPDIR=${folder}\nTEMP=${folder}\nTMP=${folder}\n`);
  assert.throws(() => freshTemp('end-history', {base, envFile}), /already exists/, 'never reuses a folder');
  assert.throws(() => freshTemp('../x', {base, envFile}), /name must be/);
  assert.throws(() => freshTemp('x', {base, envFile: ''}), /required/);
  assert.equal(freshTemp('second', {base, envFile}), join(base, 'gil-ci-second'), 'inside the given temporary folder');
});
