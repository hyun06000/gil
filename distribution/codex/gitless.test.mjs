import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync, mkdtempSync, writeFileSync, existsSync, rmSync, realpathSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {spawnSync} from 'node:child_process';

const script = new URL('./prepare-gitless-macos.sh', import.meta.url).pathname;
const source = readFileSync(script, 'utf8');
test('preparation pins the published bytes and does not install or weaken trust', () => {
  assert.match(source, /f484c09919c3c58e722ef12a466158bcb478e56bada2e216b7cbbcdfc3102d1c/);
  assert.match(source, /releases\/download\/v0\.2\.1-preview\.3/);
  assert.ok(source.indexOf('SHA-256 mismatch') < source.indexOf('/usr/bin/tar -xzf'));
  for (const line of source.split('\n').filter(l => !l.trim().startsWith('#'))) {
    assert.doesNotMatch(line, /^\s*(?:git|xcode-select|xattr|spctl|sudo|npm|node|cargo|brew|codex)\s/);
  }
  assert.doesNotMatch(source, /config\.toml|plugins\/cache|curl[^\n]*\|/);
});
test('shell syntax is valid', () => {
  assert.equal(spawnSync('/bin/sh', ['-n', script]).status, 0);
});
test('missing arguments fail without downloading', () => {
  assert.notEqual(spawnSync('/bin/sh', [script, '--destination']).status, 0);
});
test('bad archives and existing destinations are refused without mutation', {
  skip: process.platform !== 'darwin' || process.arch !== 'arm64',
}, () => {
  const root = realpathSync(mkdtempSync(join(tmpdir(), 'gil-gitless-test-')));
  try {
    const archive = join(root, 'invalid.tar.gz');
    writeFileSync(archive, 'not the approved archive');
    const next = join(root, '새 폴더');
    const run = dest => spawnSync('/bin/sh', [script, '--destination', dest, '--archive', archive], {encoding:'utf8'});
    const bad = run(next);
    assert.notEqual(bad.status, 0);
    assert.match(bad.stderr, /SHA-256 mismatch/);
    assert.equal(existsSync(next), false);
    const existing = run(root);
    assert.notEqual(existing.status, 0);
    assert.match(existing.stderr, /already exists/);
    assert.equal(readFileSync(archive, 'utf8'), 'not the approved archive');
  } finally { rmSync(root, {recursive:true, force:true}); }
});
