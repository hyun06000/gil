// Only CI build outputs, after tests and artifact uploads. Dependency outputs stay cached.
import {existsSync, lstatSync, realpathSync, writeFileSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
import {resolve} from 'node:path';
import assert from 'node:assert/strict';
assert.equal(process.env.GITHUB_ACTIONS, 'true', 'CI-only cache maintenance');
assert.ok(process.env.GITHUB_WORKSPACE, 'runner workspace required');
assert.equal(realpathSync(process.cwd()), realpathSync(process.env.GITHUB_WORKSPACE));
for (const target of ['target', 'target/plugin-core-build']) {
  if (!existsSync(target)) continue;
  assert.ok(lstatSync(target).isDirectory() && !lstatSync(target).isSymbolicLink());
  assert.equal(realpathSync(target), resolve(realpathSync(process.cwd()), target));
  // Older partial caches contain target/debug but omit Cargo's root tag. Cargo
  // does not recreate the tag when the directory already exists. Mark only
  // these fixed, non-symlink CI build directories; never overwrite a valid or
  // invalid existing tag, which cargo clean must validate itself.
  const tag = `${target}/CACHEDIR.TAG`;
  if (!existsSync(tag)) writeFileSync(tag,
    'Signature: 8a477f597d28d172789f06886806bc55\n# GIL CI Cargo build cache.\n', {flag:'wx'});
  const result = spawnSync('cargo', ['clean', '--locked', '-p', 'gil', '-p', 'gil-companion', '--target-dir', target], {stdio:'inherit'});
  if (result.error) throw result.error;
  assert.equal(result.status, 0, `workspace cache cleanup failed: ${target}`);
}
