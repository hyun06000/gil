// The repository update feed is metadata the installed Core reads once a day. Shape only.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {compareVersion} from './update-policy.mjs';

const feed = JSON.parse(await readFile(new URL('../update-feed.json', import.meta.url), 'utf8'));

test('update feed has exactly the published shape and one entry per supported platform', async () => {
  assert.deepEqual(Object.keys(feed).sort(), ['note', 'platforms', 'schema', 'updated']);
  assert.equal(feed.schema, 1);
  assert.match(feed.updated, /^\d{4}-\d{2}-\d{2}$/);
  assert.deepEqual(Object.keys(feed.platforms).sort(), ['darwin-arm64', 'windows-x64']);
  for (const [platform, entry] of Object.entries(feed.platforms)) {
    assert.deepEqual(Object.keys(entry).sort(), ['install_doc', 'notes', 'release_url', 'version'], platform);
    compareVersion(entry.version, entry.version); // strict semver, throws otherwise
    for (const key of ['release_url', 'install_doc']) {
      const url = new URL(entry[key]);
      assert.equal(url.protocol, 'https:', `${platform}.${key} must be https`);
      assert.equal(url.hostname, 'github.com');
    }
    assert.ok(entry.notes.length > 0 && entry.notes.length <= 2000);
  }
});

test('update feed versions match the published install guide', async () => {
  // The root INSTALL.md is the short agent procedure (2026-11-12); the detailed guide carries
  // the per-channel pins. Either may name the published version.
  const install = (await readFile(new URL('../../INSTALL.md', import.meta.url), 'utf8'))
    + (await readFile(new URL('../INSTALL-GUIDE.md', import.meta.url), 'utf8'));
  assert.ok(install.includes(`\`${feed.platforms['darwin-arm64'].version}\``), 'the install docs name the macOS version');
  assert.ok(install.includes(`\`${feed.platforms['windows-x64'].version}\``), 'the install docs name the Windows version');
  assert.ok(install.includes(feed.platforms['darwin-arm64'].release_url));
  assert.ok(install.includes(feed.platforms['windows-x64'].release_url));
});

test('update feed names the active installer and archive pins, not a historical guide entry', async () => {
  // Windows: the single-executable channel pin (2026-11-12); owner-update.json is the retired Companion channel.
  const windows = JSON.parse(await readFile(new URL('../windows/current-release.json', import.meta.url), 'utf8'));
  assert.equal(feed.platforms['windows-x64'].release_url, windows.release_url);
  const mac = await readFile(new URL('./prepare-gitless-macos.sh', import.meta.url), 'utf8');
  assert.equal(feed.platforms['windows-x64'].version, windows.version);
  assert.equal(feed.platforms['darwin-arm64'].version, mac.match(/^version=(\S+)$/m)?.[1]);
});
