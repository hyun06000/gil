// Maintainer-only, read-only checks. Never signs, uploads, executes the candidate or publishes it.
import {execFileSync} from 'node:child_process';
import {resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {repository, sourceSnapshot, verifyTree} from './artifact.mjs';

const tools = {
  codesign: ['/usr/bin/xcrun', ['--find', 'codesign']],
  notarytool: ['/usr/bin/xcrun', ['--find', 'notarytool']],
  ditto: ['/usr/bin/xcrun', ['--find', 'ditto']],
};
const checked = (id, state, reason) => ({id, state, reason});
const command = (file, args) => execFileSync(file, args, {
  encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], timeout: 10_000, maxBuffer: 1024 * 1024,
});

export function identitiesFrom(output) {
  // security find-identity -v reports valid identities only. Keep identifiers in memory, not the report.
  return [...output.matchAll(/^\s*\d+\)\s+([a-fA-F0-9]{40})\s+"(Developer ID Application:[^"\r\n]+)"\s*$/gm)]
    .map(([, hash, name]) => ({hash: hash.toUpperCase(), name}));
}

export function inspectEnvironment({platform = process.platform, arch = process.arch,
  env = process.env, run = command} = {}) {
  const checks = [checked('platform', platform === 'darwin' && arch === 'arm64' ? 'passed' : 'blocked',
    platform === 'darwin' && arch === 'arm64' ? 'macos_arm64' : 'only_macos_arm64_is_in_scope')];
  if (checks[0].state === 'blocked') return checks;
  for (const [id, [file, args]] of Object.entries(tools)) {
    try { run(file, args); checks.push(checked(id, 'passed', 'tool_found')); }
    catch { checks.push(checked(id, 'blocked', 'tool_unavailable')); }
  }
  try {
    const identities = identitiesFrom(run('/usr/bin/security', ['find-identity', '-v', '-p', 'codesigning']));
    checks.push(checked('developer_id', identities.length ? 'passed' : 'blocked',
      identities.length ? 'valid_application_identity_found' : 'no_valid_application_identity'));
    const selector = env.APPLE_SIGNING_IDENTITY?.trim();
    const matches = selector ? identities.filter(one => one.hash === selector.toUpperCase() || one.name === selector) : [];
    checks.push(checked('signing_selection', matches.length === 1 ? 'passed' : 'blocked',
      !selector ? 'identity_not_selected' : matches.length === 1 ? 'exactly_one_identity_selected' : 'identity_missing_or_ambiguous'));
  } catch {
    checks.push(checked('developer_id', 'blocked', 'identity_query_unavailable'));
    checks.push(checked('signing_selection', 'not_checked', 'identity_query_unavailable'));
  }
  // A profile name is configuration, NOT proof that a credential exists or can authenticate.
  checks.push(checked('notary_configuration', env.APPLE_NOTARY_PROFILE?.trim() ? 'passed' : 'blocked',
    env.APPLE_NOTARY_PROFILE?.trim() ? 'profile_named_but_not_authenticated' : 'keychain_profile_not_selected'));
  return checks;
}

export function compareSource(receipt, current) {
  const source = receipt?.source;
  if (!source || source.dirty !== false || current?.dirty !== false) {
    return checked('candidate_source', 'blocked', 'clean_source_required');
  }
  const valid = item => /^[a-f0-9]{40}$/.test(item.head) && /^[a-f0-9]{64}$/.test(item.snapshot_sha256)
    && Number.isSafeInteger(item.files) && item.files > 0;
  if (!valid(source) || !valid(current) || ['head', 'files', 'snapshot_sha256'].some(key => source[key] !== current[key])) {
    return checked('candidate_source', 'blocked', 'source_snapshot_mismatch');
  }
  // Hash agreement is not build provenance or publisher authentication.
  return checked('candidate_source', 'passed', 'clean_source_snapshot_matches_not_a_build_attestation');
}

export async function inspectCandidate(tree, {root = repository, verify = verifyTree, snapshot = sourceSnapshot} = {}) {
  if (!tree) return [checked('candidate', 'not_checked', 'no_preview_tree_supplied')];
  let receipt;
  try { receipt = await verify(resolve(tree)); }
  catch { return [checked('candidate', 'blocked', 'preview_integrity_check_failed')]; }
  const checks = [checked('candidate', 'passed', 'development_preview_integrity_only')];
  try { checks.push(compareSource(receipt, await snapshot(root))); }
  catch { checks.push(checked('candidate_source', 'blocked', 'source_snapshot_unavailable')); }
  return checks;
}

export function reportFor(checks) {
  const required = ['platform', 'codesign', 'notarytool', 'ditto', 'developer_id', 'signing_selection',
    'notary_configuration', 'candidate', 'candidate_source'];
  const complete = required.every(id => checks.filter(check => check.id === id).length === 1);
  return {schema: 1, purpose: 'macos_native_mcp_release_preflight',
    status: complete && checks.every(check => check.state === 'passed') ? 'preflight_only' : 'blocked_or_incomplete',
    publishable: false, checks,
    not_verified: ['notary_authentication', 'build_provenance', 'developer_id_signature', 'notarization',
      'fresh_mac_marketplace_install', 'fullscreen_and_restart', 'update_and_project_preservation',
      'public_source_review', 'publication_channel_and_version'],
    limits: 'Read-only preflight. No candidate execution, signing, credential import, notarization submission or publication. Passing checks never authorize release.'};
}

export async function main(args = process.argv.slice(2)) {
  if ((args.length !== 1 && args.length !== 2) || !['environment', 'candidate'].includes(args[0])
    || (args[0] === 'environment' && args.length !== 1) || (args[0] === 'candidate' && args.length !== 2)) {
    console.error('Usage: release-preflight.mjs environment | candidate <verified-preview-marketplace-directory>');
    return 2;
  }
  const checks = [...inspectEnvironment(), ...await inspectCandidate(args[0] === 'candidate' ? args[1] : null)];
  const report = reportFor(checks);
  console.log(JSON.stringify(report, null, 2));
  return report.status === 'preflight_only' ? 0 : 1;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  // Do not print an arbitrary system exception: it may include paths or credentials.
  try { process.exitCode = await main(); }
  catch { console.error('Release preflight failed; no signing or publication was attempted.'); process.exitCode = 2; }
}
