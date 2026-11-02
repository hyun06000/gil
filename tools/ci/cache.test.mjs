import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtempSync, mkdirSync, readFileSync, existsSync, rmSync, writeFileSync, unlinkSync, symlinkSync, realpathSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';

const root = fileURLToPath(new URL('../../', import.meta.url));
const cleaner = join(root, 'tools/ci/prune-workspace-cache.mjs');
const workflows = ['codex-preview', 'chain-core', 'companion-native', 'windows-spike'];

test('dependency caches are stable across source commits; only cache maintenance is conditional', () => {
  for (const name of workflows) {
    const text = readFileSync(join(root, `.github/workflows/${name}.yml`), 'utf8');
    const key = text.match(/^          key: (.+)$/m)?.[1];
    assert.ok(key, name);
    assert.match(key, /runner.os.*runner.arch.*rust-1\.97\.1.*Cargo.lock.*package-lock.json.*deps-v2/);
    assert.ok(key.includes(`.github/workflows/${name}.yml`) && key.includes('tools/ci/prune-workspace-cache.mjs'));
    assert.doesNotMatch(key, /github\.sha|head\.sha|run_id/, 'source changes must not upload dependencies again');
    const conditional = text.split('      - ').filter(s => s.includes('if: steps.build_cache.outputs.cache-hit'));
    assert.equal(conditional.length, 1);
    assert.match(conditional[0], /if: steps.build_cache.outputs.cache-hit != 'true'\n        run: node tools\/ci\/prune-workspace-cache.mjs/);
  }
});

test('profile grouping retains both Windows CPUs and both profiles, and probes the product before testbed builds', () => {
  const text = readFileSync(join(root, '.github/workflows/windows-spike.yml'), 'utf8');
  assert.equal((text.match(/          - arch: x64/g) || []).length, 2);
  assert.equal((text.match(/          - arch: arm64/g) || []).length, 2);
  const at = name => text.indexOf(`      - name: ${name}`);
  assert.ok(at('Runtime isolated launch mechanism') < at('Companion sessions and Windows named pipes'));
  assert.ok(at('Image-report storage boundaries') < at('Companion sessions and Windows named pipes'));
  assert.ok(at('Native protocol, read-only Monitor') < at('Companion sessions and Windows named pipes'));
  const detector = text.slice(at('Automatic installer target detection'), at('Install maintainer toolchain'));
  assert.match(detector, /if: matrix.profile == 'default'/);
  const chain = readFileSync(join(root, '.github/workflows/chain-core.yml'), 'utf8');
  assert.match(chain, /runner: macos-15/); assert.match(chain, /runner: windows-2025/);
  const features = [...chain.matchAll(/run: cargo (?:test|build).*?--features ([\w,-]+)/g)].map(m => m[1]);
  const groups = features.filter((f, i) => f !== features[i-1]);
  assert.deepEqual(groups, ['chain-core', 'chain-core,visual-authoring,product-safety', 'approved-plan,visual-authoring,product-safety']);
});

test('cache pruning removes implicit and explicit target workspace outputs but preserves dependencies', t => {
  const folder = realpathSync(mkdtempSync(join(tmpdir(), 'gil-cache-test-')));
  t.after(() => rmSync(folder, {recursive:true, force:true}));
  writeFileSync(join(folder, 'Cargo.toml'), '[workspace]\nmembers=["gil","gil-companion","cache-dep"]\nresolver="2"\n');
  for (const name of ['gil', 'gil-companion', 'cache-dep']) {
    mkdirSync(join(folder, name, 'src'), {recursive:true});
    writeFileSync(join(folder, name, 'Cargo.toml'), `[package]\nname="${name}"\nversion="0.1.0"\nedition="2021"\n` +
      (name === 'cache-dep' ? '' : '[dependencies]\ncache-dep={path="../cache-dep"}\n'));
    writeFileSync(join(folder, name, 'src/lib.rs'), 'pub fn value() -> u8 { 1 }\n');
  }
  const run = (args, extra = {}) => {
    const result = spawnSync('cargo', args, {cwd:folder, encoding:'utf8', env:{...process.env, CARGO_TARGET_DIR:join(folder,'target'), ...extra}});
    assert.equal(result.status, 0, result.stderr); return result;
  };
  const version = spawnSync('rustc', ['-vV'], {encoding:'utf8'});
  assert.equal(version.status, 0, version.stderr);
  const triple = version.stdout.match(/^host: (.+)$/m)?.[1];
  assert.ok(triple);
  run(['build', '--offline', '--workspace']);
  const dirs = [join(folder, 'target/debug')];
  if (['aarch64-apple-darwin', 'x86_64-pc-windows-msvc', 'aarch64-pc-windows-msvc'].includes(triple)) {
    run(['build', '--offline', '--locked', '--workspace', '--target', triple]);
    dirs.push(join(folder, 'target', triple, 'debug'));
  }
  unlinkSync(join(folder, 'target/CACHEDIR.TAG')); // A restored partial cache may omit the root tag.
  const env = {...process.env, GITHUB_ACTIONS:'true', GITHUB_WORKSPACE:folder};
  const clean = spawnSync(process.execPath, [cleaner], {cwd:folder, env, encoding:'utf8'});
  assert.equal(clean.status, 0, clean.stderr);
  for (const dir of dirs) {
    assert.ok(!existsSync(join(dir, 'libgil.rlib')), dir);
    assert.ok(!existsSync(join(dir, 'libgil_companion.rlib')), dir);
    assert.ok(existsSync(join(dir, 'libcache_dep.rlib')), 'dependency must survive: ' + dir);
  }
  const wrong = spawnSync(process.execPath, [cleaner], {cwd:folder, env:{...env,GITHUB_WORKSPACE:tmpdir()}, encoding:'utf8'});
  assert.notEqual(wrong.status, 0, 'wrong workspace must be refused');
  mkdirSync(join(folder, 'outside'));
  const link = join(folder, 'target/plugin-core-build');
  symlinkSync(resolve(folder, 'outside'), link, 'junction');
  const linked = spawnSync(process.execPath, [cleaner], {cwd:folder, env, encoding:'utf8'});
  assert.notEqual(linked.status, 0, 'symlink build root must be refused');
});
