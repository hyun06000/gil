import assert from 'node:assert/strict';
import test from 'node:test';
import {readFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {build} from 'esbuild';

// Regression floor for reviewed advisories, not a replacement for a live audit.
const app = JSON.parse(await readFile(new URL('./package-lock.json', import.meta.url)));
const bridge = JSON.parse(await readFile(new URL('../plugins/gil-companion-prototype/package-lock.json', import.meta.url)));
for (const [lock, name, floor] of [
  [app, '@modelcontextprotocol/client', '2.2.0'],
  [app, '@modelcontextprotocol/core', '2.2.0'],
  [bridge, '@modelcontextprotocol/sdk', '1.31.0'],
  [bridge, 'fast-uri', '3.1.8'],
  [bridge, 'ip-address', '10.7.3'],
  [bridge, 'proxy-addr', '2.0.8'],
]) {
  test(`reviewed security floor: ${name} >= ${floor}`, () => {
    const entry = lock.packages[`node_modules/${name}`];
    assert.match(entry.version, /^\d+\.\d+\.\d+$/);
    const actual = entry.version.split('.').map(Number), required = floor.split('.').map(Number);
    const difference = actual.map((n, i) => n - required[i]).find(n => n !== 0) ?? 0;
    assert.ok(difference >= 0, 'known vulnerable version restored');
    assert.ok(entry.resolved.startsWith('https://registry.npmjs.org/'));
    assert.match(entry.integrity, /^sha512-/);
  });
}

test('Monitor bundle does not include the SDK OAuth authentication implementation', async () => {
  const result = await build({absWorkingDir: fileURLToPath(new URL('.', import.meta.url)),
    entryPoints: ['app.js'], bundle: true, write: false, format: 'iife', platform: 'browser',
    minify: true, metafile: true});
  const authInputs = Object.values(result.metafile.outputs).flatMap(output =>
    Object.entries(output.inputs).filter(([path, info]) => info.bytesInOutput > 0 &&
      /node_modules\/@modelcontextprotocol\/.*(?:oauth|auth\.js|auth\/)/i.test(path)));
  assert.deepEqual(authInputs, [], 'OAuth transport requires separate exposure review');
});
