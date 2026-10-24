import test from 'node:test';
import assert from 'node:assert/strict';
import {EXECUTABLES, EMBEDS_BUILD_PATHS, bundleName, executablesFrom, manifest, CODEX_CLI_PIN} from './verify-bundle.mjs';

const artifact = (name, executable) => JSON.stringify({reason: 'compiler-artifact', target: {name}, executable});

test('bundle names are per native CPU and nothing else', () => {
  assert.equal(bundleName('x64'), 'gil-runtime-verify-windows-x64');
  assert.equal(bundleName('arm64'), 'gil-runtime-verify-windows-arm64');
  assert.throws(() => bundleName('x86'));
  assert.equal(CODEX_CLI_PIN, '0.163.0-alpha.11.1');
});
test('all three executables must come out of one cargo build, each once', () => {
  const lines = [artifact('gil', 'D:/x/gil.exe'), artifact('gil-runtime-verify', 'D:/x/gil-runtime-verify.exe'),
    artifact('gil-runtime-testbed', 'D:/x/gil-runtime-testbed.exe'), JSON.stringify({reason: 'compiler-artifact', target: {name: 'gil'}, executable: null}),
    artifact('runtime_integration', 'D:/x/deps/runtime_integration-abc.exe'), 'warning: noise'].join('\n');
  const found = executablesFrom(lines);
  assert.deepEqual([...found.keys()].sort(), Object.keys(EXECUTABLES).sort());
  assert.equal(found.get('runtime_integration'), 'D:/x/deps/runtime_integration-abc.exe');
  assert.throws(() => executablesFrom(lines.replace(artifact('runtime_integration', 'D:/x/deps/runtime_integration-abc.exe'), '')), /missing executable runtime_integration/);
  assert.throws(() => executablesFrom(lines + '\n' + artifact('gil-runtime-verify', 'D:/y/other.exe')), /two executables/);
});
test('the manifest lists every executable, never itself, and only plain file names', () => {
  const head = 'a'.repeat(40);
  const rows = Object.values(EXECUTABLES).map(path => ({path, bytes: 1, sha256: 'b'.repeat(64)}));
  const m = manifest(head, 'x64', [...rows, {path: 'VERIFY.md', bytes: 2, sha256: 'c'.repeat(64)}]);
  assert.equal(m.source, head); assert.equal(m.arch, 'x64'); assert.equal(m.kind, 'gil-runtime-verify');
  assert.equal(m.files.length, 4);
  assert.throws(() => manifest('short', 'x64', rows));
  assert.throws(() => manifest(head, 'x64', rows.slice(1)), /manifest lacks/);
  assert.throws(() => manifest(head, 'x64', [...rows, {path: 'bundle.json', bytes: 1, sha256: 'd'.repeat(64)}]));
  assert.throws(() => manifest(head, 'x64', [...rows, {path: 'sub/dir.txt', bytes: 1, sha256: 'd'.repeat(64)}]));
  assert.ok(EMBEDS_BUILD_PATHS.has('runtime_integration.exe') && EMBEDS_BUILD_PATHS.size === 1);
});
