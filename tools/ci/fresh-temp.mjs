// Give the following CI steps a fresh temporary folder (TMPDIR on Unix, TEMP/TMP on Windows).
//
// Some Core test fixtures keep a synthetic project at a fixed path under the temporary folder and
// reuse it when it already exists. A build that does not read another profile's records (for
// example an approved-plan Chain read by a build without approved-plan) then fails on what an
// earlier step left behind, not on its own code. A fresh folder per profile group keeps each
// group's fixtures its own. Nothing is deleted.
//
// The new folder is made **inside the default temporary folder**, never elsewhere: some fixtures
// assert properties of the real temporary location (on macOS it lies under the /var symlink, and
// ownership of its ancestors is checked), so moving it to another root would change what they test.
//
// Usage: node tools/ci/fresh-temp.mjs <name>   (inside GitHub Actions)
import {appendFileSync, existsSync, mkdirSync} from 'node:fs';
import {join} from 'node:path';
import {tmpdir} from 'node:os';

export function freshTemp(name, {base = tmpdir(), envFile = process.env.GITHUB_ENV} = {}) {
  if (!/^[a-z0-9-]+$/.test(name ?? '')) throw new Error('fresh-temp: name must be lowercase letters, digits or dashes');
  if (!base || !envFile) throw new Error('fresh-temp: a temporary folder and GITHUB_ENV are required');
  const folder = join(base, `gil-ci-${name}`);
  if (existsSync(folder)) throw new Error(`fresh-temp: ${folder} already exists; a group gets a folder of its own`);
  mkdirSync(folder, {recursive: true});
  appendFileSync(envFile, ['TMPDIR', 'TEMP', 'TMP'].map(key => `${key}=${folder}\n`).join(''));
  return folder;
}

if (import.meta.url === `file://${process.argv[1]}` || process.argv[1]?.endsWith('fresh-temp.mjs')) {
  console.log(`fresh temporary folder: ${freshTemp(process.argv[2])}`);
}
