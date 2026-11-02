// Only CI build outputs, after tests and artifact uploads. Dependency outputs stay cached.
import {existsSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
import assert from 'node:assert/strict';
assert.equal(process.env.GITHUB_ACTIONS, 'true', 'CI-only cache maintenance');
for (const target of ['target', 'target/plugin-core-build']) {
  if (!existsSync(target)) continue;
  const result = spawnSync('cargo', ['clean', '--locked', '-p', 'gil', '-p', 'gil-companion', '--target-dir', target], {stdio:'inherit'});
  if (result.error) throw result.error;
  assert.equal(result.status, 0, `workspace cache cleanup failed: ${target}`);
}
