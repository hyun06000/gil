// Build + verify only. No install, cache edit, credential use, upload, commit or push.
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {mkdir, mkdtemp, readFile, realpath, rm, writeFile} from 'node:fs/promises';
import {homedir} from 'node:os';
import {join, resolve} from 'node:path';
import {ARCHIVE_FLAGS, createTree, hash, inventory, repository, sourceSnapshot, verifyTree} from './artifact.mjs';
import {smoke} from './smoke.mjs';
import {check as checkNotices} from '../compliance/notices.mjs';

assert.equal(process.platform, 'darwin');
assert.equal(process.arch, 'arm64', 'only tested architecture may be packaged');
const [destination, ...extra] = process.argv.slice(2);
assert.ok(destination && extra.length === 0, 'Usage: node distribution/codex/build-preview.mjs <new-output-directory>');
const output = resolve(destination);
await mkdir(output, {recursive: false}); // Existing output is never overwritten.
const run = (command, args, env = process.env) => new Promise((resolve, reject) => {
  const child = spawn(command, args, {cwd: repository, env, stdio: 'inherit'});
  child.on('error', reject);
  child.on('exit', code => code === 0 ? resolve() : reject(new Error(`${command} failed (${code})`)));
});
// Dependencies must already be restored with npm ci. Never install during packaging.
await run(process.execPath, ['mcp-app/build.mjs']);
// Recompute the actual Cargo/UI scope, not merely trust a previously generated notice file.
const notices = await checkNotices(repository);
const source = await sourceSnapshot(repository);
const target = join(repository, 'target/plugin-core-build');
const cargoHome = process.env.CARGO_HOME || join(homedir(), '.cargo');
const rustflags = `--remap-path-prefix=${cargoHome}=/cargo --remap-path-prefix=${repository}=/gil`;
assert.ok(!/\s/.test(repository + cargoHome), 'build roots containing whitespace need encoded rustflags support');
await run('cargo', ['build', '--locked', '--release', '-p', 'gil', '--target', 'aarch64-apple-darwin'], {
  ...process.env, RUSTFLAGS: rustflags, CARGO_TARGET_DIR: target,
});
assert.deepEqual(await sourceSnapshot(repository), source, 'source changed during build');
const marketplace = join(output, 'marketplace');
const receipt = await createTree({binary: join(target, 'aarch64-apple-darwin/release/gil'), output: marketplace});
assert.deepEqual(receipt.source, source);
const archive = join(output, 'gil-codex-macos-arm64-preview.tar.gz');
// tar retains dot-directories and executable bits; upload-artifact's raw file mode loss cannot damage it.
await run('/usr/bin/tar', ['-czf', archive, ...ARCHIVE_FLAGS, '-C', marketplace, '.'], {...process.env, COPYFILE_DISABLE: '1'});
const extracted = await realpath(await mkdtemp(join(output, 'roundtrip-')));
try {
  // Only extract the archive just built from our allowlisted tree, never an untrusted downloaded archive.
  await run('/usr/bin/tar', ['-xzf', archive, '-C', extracted]);
  await verifyTree(extracted);
  assert.deepEqual(await inventory(extracted), await inventory(marketplace), 'archive roundtrip changed files/modes');
  const check = await smoke(extracted, join(repository, 'plugins/gil-companion-prototype/assets/monitor.html'));
  const archiveHash = hash(await readFile(archive));
  await writeFile(join(output, 'SHA256SUMS'), `${archiveHash}  gil-codex-macos-arm64-preview.tar.gz\n`, {flag: 'wx'});
  await writeFile(join(output, 'checks.json'), JSON.stringify({channel: 'development_unsigned', publishable: false,
    source, third_party_notices: notices, archive_sha256: archiveHash, roundtrip: 'passed', smoke: check}, null, 2) + '\n', {flag: 'wx'});
  console.log(JSON.stringify({channel: 'development_unsigned', publishable: false,
    archive_sha256: archiveHash, roundtrip: 'passed', tools: check.tools, source_dirty: source.dirty}));
} finally {
  await rm(extracted, {recursive: true, force: true}); // Only this freshly allocated roundtrip directory.
}
