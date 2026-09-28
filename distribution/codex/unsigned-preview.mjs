// Maintainer-only opt-in preview staging. No downloads, installs, signing, security overrides or publishing.
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {chmod, mkdir, mkdtemp, readFile, realpath, rm, writeFile} from 'node:fs/promises';
import {dirname, join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {ARCHIVE_FLAGS, CORE, MANIFEST, hash, inventory, verifyPayload, verifyTree} from './artifact.mjs';
import {compareVersion, validateRecord} from './update-policy.mjs';
import {smoke} from './smoke.mjs';

const json = value => JSON.stringify(value, null, 2) + '\n';
const sourceKeys = ['dirty', 'files', 'head', 'snapshot_sha256'];
export const TRUST = Object.freeze({developer_id: 'not_provided', notarization: 'not_performed',
  clean_machine_install: 'deferred_not_passed', automatic_update: 'not_authorized',
  os_security_override: 'prohibited', publication: 'requires_separate_approval'});
const notice = version => `# GIL ${version} — unsigned opt-in preview candidate

macOS Apple Silicon / Codex only. Not a stable release; not yet published.
No Apple Developer ID signature or notarization is provided. Ad-hoc code signing may be present.
Fresh-Mac installation is deferred, NOT passed. macOS may refuse to run this software.
Do not disable Gatekeeper or remove quarantine to install it. Stop if the OS blocks execution.
SHA-256 checks integrity; it does not authenticate the publisher or replace Apple's checks.
Install/update only after informed user approval through the verified Plugin channel.
The installed runtime needs no Node, npm, Cargo, Homebrew or separate Companion app.
Core, MCP tools and Monitor UI are bundled; a successful tool response is not proof of visible fullscreen.
Windows, Intel Macs and Claude work-Plugin rendering are not accepted targets of this preview.
Removing the Plugin must not remove a Project or its .gil records.
See release.json for exact bytes and source; publication remains a separate reviewed operation.
`;

function checkVersion(version) {
  assert.ok(typeof version === 'string' && version.length < 80
    && /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)-preview\.[1-9]\d*$/.test(version),
  'unsigned candidates require an explicit X.Y.Z-preview.N version');
}
function checkSource(source) {
  assert.deepEqual(Object.keys(source ?? {}).sort(), sourceKeys);
  assert.equal(source.dirty, false, 'dirty development trees cannot become preview candidates');
  assert.match(source.head, /^[a-f0-9]{40}$/);
  assert.match(source.snapshot_sha256, /^[a-f0-9]{64}$/);
  assert.ok(Number.isSafeInteger(source.files) && source.files > 0);
}
export async function verifyUnsignedTree(root) {
  // Inventory first rejects symlinks, including a symlinked receipt.
  const all = await inventory(root);
  assert.equal(all.find(f => f.path === 'release.json')?.mode, 0o644);
  const receipt = JSON.parse(await readFile(join(root, 'release.json')));
  assert.deepEqual(Object.keys(receipt).sort(), ['binary_origin', 'channel', 'files', 'platform', 'plugin',
    'publishable', 'schema', 'source', 'trust', 'version']);
  assert.equal(receipt.schema, 2);
  assert.equal(receipt.channel, 'preview_unsigned');
  assert.equal(receipt.publishable, false, 'staging does not authorize publication');
  assert.equal(receipt.platform, 'darwin-arm64');
  assert.equal(receipt.plugin, 'gil-companion-prototype');
  assert.equal(receipt.binary_origin, 'preserved_from_verified_development_tree; CI provenance review still required');
  assert.deepEqual(receipt.trust, TRUST);
  checkSource(receipt.source); checkVersion(receipt.version);
  await verifyPayload(root, receipt);
  assert.equal(await readFile(join(root, 'PREVIEW.md'), 'utf8'), notice(receipt.version));
  return receipt;
}

export async function stageUnsigned({input, output, version}) {
  checkVersion(version);
  const before = await verifyTree(input);
  checkSource(before.source);
  assert.ok(compareVersion(version, before.version.split('+')[0]) > 0,
    'candidate must sort after the input development version');
  // A new tree only. Do not rewrite a development receipt or installed cache in place.
  await mkdir(output, {recursive: false, mode: 0o755});
  for (const file of before.files) {
    let bytes = await readFile(join(input, file.path));
    assert.equal(hash(bytes), file.sha256, 'input changed during staging');
    if (file.path === MANIFEST) {
      const manifest = JSON.parse(bytes);
      manifest.version = version; // Only output metadata changes; Core and Skill identities stay put.
      bytes = Buffer.from(json(manifest));
    } else if (file.path === 'PREVIEW.md') bytes = Buffer.from(notice(version));
    const path = join(output, file.path);
    await mkdir(dirname(path), {recursive: true});
    await writeFile(path, bytes, {flag: 'wx', mode: file.mode});
    await chmod(path, file.mode);
  }
  assert.deepEqual(await verifyTree(input), before, 'input changed during staging');
  const receipt = {schema: 2, channel: 'preview_unsigned', publishable: false,
    platform: before.platform, plugin: before.plugin, version, source: before.source,
    binary_origin: 'preserved_from_verified_development_tree; CI provenance review still required',
    trust: {...TRUST}, files: await inventory(output)};
  await writeFile(join(output, 'release.json'), json(receipt), {flag: 'wx', mode: 0o644});
  await chmod(join(output, 'release.json'), 0o644);
  await verifyUnsignedTree(output);
  return receipt;
}

export async function prepare({input, output, version}) {
  assert.equal(process.platform, 'darwin'); assert.equal(process.arch, 'arm64');
  checkVersion(version);
  const prior = await verifyTree(input);
  checkSource(prior.source);
  // Capture the exact trusted-input inventory; evidence files are not blindly copied or accepted.
  const priorFiles = await inventory(input);
  await mkdir(output, {recursive: false});
  const marketplace = join(output, 'marketplace');
  const receipt = await stageUnsigned({input, output: marketplace, version});
  const archiveName = `gil-codex-macos-arm64-${version}.tar.gz`;
  const archive = join(output, archiveName);
  execFileSync('/usr/bin/tar', ['-czf', archive, ...ARCHIVE_FLAGS, '-C', marketplace, '.'],
    {env: {...process.env, COPYFILE_DISABLE: '1'}, stdio: 'pipe', timeout: 30000});
  const extracted = await realpath(await mkdtemp(join(output, 'roundtrip-')));
  try {
    // Extract only the archive we just created from the validated allowlist.
    execFileSync('/usr/bin/tar', ['-xzf', archive, '-C', extracted], {stdio: 'pipe', timeout: 30000});
    await verifyUnsignedTree(extracted);
    assert.deepEqual(await inventory(extracted), await inventory(marketplace));
    const check = await smoke(extracted, undefined, verifyUnsignedTree);
    const archiveHash = hash(await readFile(archive));
    const record = validateRecord({schema: 1, plugin: receipt.plugin, platform: receipt.platform, version,
      source_commit: receipt.source.head, source_snapshot_sha256: receipt.source.snapshot_sha256,
      archive_sha256: archiveHash, core_sha256: receipt.files.find(f => f.path === CORE).sha256,
      ui_sha256: check.ui_sha256, compatibility: {
        protocol: check.core.protocol, action_surface: check.core.action_surface,
        storage_format: check.core.storage_format,
        // Current reviewed Monitor wire contract, not a capability inferred from tool count.
        monitor_view_schema: {min: 1, max: 1}, node_detail_schema: {min: 1, max: 1}}, migration: 'none'});
    // This staging policy is only reviewed for the existing no-migration Core contract.
    assert.deepEqual(record.compatibility, {protocol: {min: 1, max: 1}, action_surface: {min: 1, max: 1},
      storage_format: {min: 4, max: 4}, monitor_view_schema: {min: 1, max: 1}, node_detail_schema: {min: 1, max: 1}});
    assert.deepEqual(await inventory(input), priorFiles, 'input changed during acceptance');
    await verifyUnsignedTree(marketplace);
    await writeFile(join(output, 'SHA256SUMS'), `${archiveHash}  ${archiveName}\n`, {flag: 'wx'});
    await writeFile(join(output, 'update-record.json'), json(record), {flag: 'wx'});
    const evidence = {schema: 1, channel: 'preview_unsigned', publishable: false, version,
      source: receipt.source, archive_sha256: archiveHash, trust: {...TRUST}, roundtrip: 'passed', smoke: check,
      limits: 'Local protocol and integrity only. CI provenance, Host install/UI, remote publication and fresh Mac are separate.'};
    await writeFile(join(output, 'checks.json'), json(evidence), {flag: 'wx'});
    return evidence;
  } finally {
    await rm(extracted, {recursive: true, force: true}); // Only this invocation's temporary roundtrip directory.
  }
}

export async function main(args = process.argv.slice(2)) {
  try {
    if (args.length === 2 && args[0] === 'verify') {
      const receipt = await verifyUnsignedTree(resolve(args[1]));
      console.log(json({integrity: 'passed', channel: receipt.channel, version: receipt.version, publishable: false}));
    } else if (args.length === 4 && args[0] === 'prepare') {
      const result = await prepare({input: resolve(args[1]), version: args[2], output: resolve(args[3])});
      console.log(json({channel: result.channel, version: result.version, publishable: false,
        archive_sha256: result.archive_sha256, tools: result.smoke.tools}));
    } else {
      console.error('Usage: unsigned-preview.mjs prepare <reviewed-tree> <X.Y.Z-preview.N> <new-output> | verify <tree>');
      return 2;
    }
    return 0;
  } catch {
    console.error('Unsigned preview preparation/verification failed. No install or publication attempted; preserve partial output for diagnosis.');
    return 1;
  }
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) process.exitCode = await main();
