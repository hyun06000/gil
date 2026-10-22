// Isolated design spike. Never discovers or edits Codex, real GIL, or projects.
import {mkdir, readFile, writeFile, open, rename, unlink} from 'node:fs/promises';
import {join} from 'node:path';
import {createHash, randomUUID} from 'node:crypto';
import {spawn} from 'node:child_process';

export const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const validVersion = value => typeof value === 'string' && /^[a-z0-9][a-z0-9.-]{0,63}$/.test(value);
function requireValue(ok, message) { if (!ok) throw new Error(message); }
function compatible(actual, required) {
  return ['skillSha256', 'protocol', 'storage'].every(key => actual[key] === required[key]);
}
async function withLock(root, action) {
  const path = join(root, 'update.lock');
  const file = await open(path, 'wx', 0o600); // Busy means stop, never steal a lock.
  try { return await action(); }
  finally { await file.close(); await unlink(path); }
}
export async function initialize(root) {
  await mkdir(root, {recursive: false, mode: 0o700});
  await mkdir(join(root, 'versions'), {mode: 0o700});
}
export async function prepare(root, release, coreBytes, expectedCoreHash) {
  requireValue(validVersion(release.version), 'invalid version');
  requireValue(release.runtime === undefined || ['node', 'native'].includes(release.runtime), 'invalid runtime');
  requireValue(sha256(coreBytes) === expectedCoreHash, 'download hash mismatch');
  const manifest = {...release, coreSha256: expectedCoreHash};
  requireValue(typeof manifest.skillSha256 === 'string', 'skill contract missing');
  return withLock(root, async () => {
    const final = join(root, 'versions', release.version);
    const staging = join(root, `staging-${randomUUID()}`);
    await mkdir(staging, {mode: 0o700});
    // Failed staging is retained, not made active. No old-version cleanup exists.
    await writeFile(join(staging, manifest.runtime === 'native' ? 'core.exe' : 'core.mjs'), coreBytes,
      {flag: 'wx', mode: manifest.runtime === 'native' ? 0o700 : 0o600});
    const bytes = JSON.stringify(manifest);
    await writeFile(join(staging, 'manifest.json'), bytes, {flag: 'wx', mode: 0o600});
    await rename(staging, final); // Existing published versions are nonempty: refuse replacement.
    return {version: release.version, manifestSha256: sha256(bytes)};
  });
}
async function verify(root, selection, contract) {
  requireValue(validVersion(selection.version), 'invalid selected version');
  const directory = join(root, 'versions', selection.version);
  const bytes = await readFile(join(directory, 'manifest.json'));
  requireValue(sha256(bytes) === selection.manifestSha256, 'manifest hash mismatch');
  const manifest = JSON.parse(bytes);
  requireValue(manifest.version === selection.version, 'version mismatch');
  requireValue(compatible(manifest, contract), 'Core/Skill contract mismatch');
  requireValue(manifest.runtime === undefined || ['node', 'native'].includes(manifest.runtime), 'invalid runtime');
  const path = join(directory, manifest.runtime === 'native' ? 'core.exe' : 'core.mjs');
  requireValue(sha256(await readFile(path)) === manifest.coreSha256, 'Core hash mismatch');
  return manifest.runtime === 'native' ? {command: path, args: []} : {command: process.execPath, args: [path]};
}
async function probe(execution, expectedVersion) {
  // Fixtures only: no project path is ever provided to the probe.
  const child = spawn(execution.command, [...execution.args, '--probe'], {stdio: ['ignore', 'pipe', 'pipe']});
  let output = '', error = '';
  child.stdout.on('data', b => { output += b; if (output.length > 4096) child.kill(); });
  child.stderr.on('data', b => { error += b; if (error.length > 4096) child.kill(); });
  const timer = setTimeout(() => child.kill(), 3000);
  try {
    await new Promise((resolve, reject) => {
      child.once('error', reject);
      child.once('close', code => code === 0 ? resolve() : reject(new Error('probe failed')));
    });
    requireValue(JSON.parse(output).version === expectedVersion, 'probe version mismatch');
  } finally { clearTimeout(timer); }
}
export async function activate(root, selection, contract, {approved = false, beforeCommit} = {}) {
  requireValue(approved === true, 'explicit approval required');
  return withLock(root, async () => {
    const execution = await verify(root, selection, contract);
    await probe(execution, selection.version);
    if (beforeCommit) await beforeCommit(); // Spike-only fault injection, not production API.
    const temporary = join(root, `active-${randomUUID()}.tmp`);
    await writeFile(temporary, JSON.stringify(selection), {flag: 'wx', mode: 0o600});
    await rename(temporary, join(root, 'active.json'));
    // No automatic fallback after a Core has been handed a session.
  });
}
export async function launch(root, contract) {
  return withLock(root, async () => {
    const selection = JSON.parse(await readFile(join(root, 'active.json')));
    const execution = await verify(root, selection, contract);
    const child = spawn(execution.command, execution.args, {stdio: ['pipe', 'pipe', 'pipe']});
    await new Promise((resolve, reject) => {child.once('spawn', resolve); child.once('error', reject);});
    return child; // Existing child remains pinned to its chosen version.
  });
}
