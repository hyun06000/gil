import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import { projectStatePath, externalStorageRoot, windowsCanonical, EXTERNAL_LAYOUT } from './lib/state-path.mjs';
import { isolatedEnv, WINDOWS_ESSENTIALS } from './lib/rpc.mjs';

test('non-Windows platforms keep the inline .gil/state.yaml layout', () => {
  const got = projectStatePath('/x/project', { platform: 'darwin', env: {} });
  assert.equal(got.layout, 'inline');
  assert.equal(got.path, join('/x/project', '.gil', 'state.yaml'));
});

test('Windows resolution needs LOCALAPPDATA from the child environment and never consults the parent', () => {
  assert.throws(() => projectStatePath(tmpdir(), { platform: 'win32', env: {} }), /LOCALAPPDATA is not set/);
});

// The hash contract is checked against the product's own rule on this machine's real paths: the
// key is sha256 over UTF-16LE code units of the canonical spelling with the verbatim prefix. The
// native Windows end-to-end run is not possible here and is recorded as such in D-RESULT.md.
test('external storage key is sha256 of the UTF-16LE canonical path with the \\\\?\\ prefix (string contract only on this OS)', () => {
  const work = mkdtempSync(join(tmpdir(), 'gil-state-path-'));
  const canonical = windowsCanonical(work);
  assert.ok(canonical.startsWith('\\\\?\\'), canonical);
  const expected = createHash('sha256').update(Buffer.from(canonical, 'utf16le')).digest('hex');
  const root = externalStorageRoot(work, join(work, 'LocalAppData'));
  assert.equal(root, join(work, 'LocalAppData', 'GIL', EXTERNAL_LAYOUT, expected));
  assert.equal(expected.length, 64);
  // preference order mirrors state_for_root: existing external wins, then existing inline, then external
  const env = { LOCALAPPDATA: join(work, 'LocalAppData') };
  assert.equal(projectStatePath(work, { platform: 'win32', env }).layout, 'private_app_data');
  mkdirSync(join(work, '.gil')); writeFileSync(join(work, '.gil', 'state.yaml'), 'inline');
  assert.equal(projectStatePath(work, { platform: 'win32', env }).layout, 'inline');
  mkdirSync(join(root, '.gil'), { recursive: true });
  assert.equal(projectStatePath(work, { platform: 'win32', env }).layout, 'private_app_data');
});

test('isolatedEnv passes only system essentials and the isolated private roots', () => {
  const env = isolatedEnv({ home: '/iso/home', localAppData: 'C:\\iso\\LocalAppData' });
  if (process.platform === 'win32') {
    assert.equal(env.LOCALAPPDATA, 'C:\\iso\\LocalAppData');
    assert.ok(env.PATH.includes('System32'));
    for (const k of Object.keys(env)) assert.ok([...WINDOWS_ESSENTIALS, 'PATH', 'LOCALAPPDATA'].includes(k), k);
  } else {
    assert.equal(env.HOME, '/iso/home');
    assert.equal(env.PATH, '/usr/bin:/bin');
    assert.ok(!('CARGO_TARGET_DIR' in env) && !('GIL_BINARY' in env));
  }
});
