import assert from 'node:assert/strict';
import {test} from 'node:test';
import {execFileSync} from 'node:child_process';
import {readFile} from 'node:fs/promises';
import {join} from 'node:path';
import {repository} from './artifact.mjs';
import {compareSource, identitiesFrom, inspectCandidate, inspectEnvironment, reportFor} from './release-preflight.mjs';

// Synthetic identities, not Apple signing evidence.
const fingerprint = 'A'.repeat(40);
const identity = 'Developer ID Application: Synthetic Fixture (TESTONLY00)';
const listing = `  1) ${fingerprint} "${identity}"\n     1 valid identities found\n`;
const env = {APPLE_SIGNING_IDENTITY: fingerprint, APPLE_NOTARY_PROFILE: 'synthetic-profile'};
const source = {dirty: false, head: 'b'.repeat(40), snapshot_sha256: 'c'.repeat(64), files: 10};
const state = (checks, id) => checks.find(check => check.id === id);
function environment(options = {}) {
  return inspectEnvironment({platform: 'darwin', arch: 'arm64', env,
    run: file => file === '/usr/bin/security' ? listing : '/synthetic/tool', ...options});
}

test('only valid Developer ID Application identities count, not ad-hoc or Apple Development', () => {
  assert.deepEqual(identitiesFrom(listing), [{hash: fingerprint, name: identity}]);
  assert.deepEqual(identitiesFrom(`1) ${fingerprint} "Apple Development: Fixture"\n0 valid identities found`), []);
  assert.deepEqual(identitiesFrom(`1) ${fingerprint} "${identity}" (CSSMERR_TP_CERT_EXPIRED)`), []);
  assert.deepEqual(identitiesFrom('Signature=adhoc\n0 valid identities found'), []);
});
test('missing Apple enrollment prerequisites block instead of promoting development builds', () => {
  const checks = environment({env: {}, run: file => file === '/usr/bin/security' ? '0 valid identities found' : ''});
  for (const id of ['developer_id', 'signing_selection', 'notary_configuration']) assert.equal(state(checks, id).state, 'blocked');
  assert.equal(reportFor(checks).publishable, false);
});
test('an exact identity selector is required; missing and ambiguous names block', () => {
  assert.equal(state(environment(), 'signing_selection').state, 'passed');
  assert.equal(state(environment({env: {...env, APPLE_SIGNING_IDENTITY: identity}}), 'signing_selection').state, 'passed');
  for (const selector of ['', 'Developer ID', 'D'.repeat(40)]) {
    assert.equal(state(environment({env: {...env, APPLE_SIGNING_IDENTITY: selector}}), 'signing_selection').state, 'blocked');
  }
  const duplicate = `${listing}\n  2) ${'D'.repeat(40)} "${identity}"\n`;
  assert.equal(state(environment({env: {...env, APPLE_SIGNING_IDENTITY: identity},
    run: file => file === '/usr/bin/security' ? duplicate : ''}), 'signing_selection').state, 'blocked');
});
test('tool and identity failures are bounded diagnostics, not raw exception output', () => {
  const checks = environment({run: () => { throw new Error('private-path-and-secret'); }});
  assert.equal(state(checks, 'developer_id').reason, 'identity_query_unavailable');
  assert.equal(state(checks, 'notarytool').state, 'blocked');
  assert.ok(!JSON.stringify(reportFor(checks)).includes('private-path-and-secret'));
});
test('reports never contain identity, fingerprint, profile or credential values', () => {
  const report = JSON.stringify(reportFor(environment({env: {...env, APPLE_PASSWORD: 'synthetic-password'}})));
  for (const secret of [identity, fingerprint, env.APPLE_NOTARY_PROFILE, 'synthetic-password']) assert.ok(!report.includes(secret));
  assert.match(report, /profile_named_but_not_authenticated/);
  assert.match(report, /notary_authentication/);
});
test('unsupported OS or architecture does not run macOS commands', () => {
  for (const [platform, arch] of [['linux', 'arm64'], ['darwin', 'x64'], ['win32', 'x64']]) {
    const checks = environment({platform, arch, run: () => { assert.fail('must not run'); }});
    assert.equal(checks.length, 1);
    assert.equal(checks[0].state, 'blocked');
  }
});
test('all environmental commands are read-only; no sign, upload, keychain write or candidate execution', () => {
  const calls = [];
  environment({run: (file, args) => { calls.push([file, args]); return file === '/usr/bin/security' ? listing : ''; }});
  assert.deepEqual(calls, [
    ['/usr/bin/xcrun', ['--find', 'codesign']], ['/usr/bin/xcrun', ['--find', 'notarytool']],
    ['/usr/bin/xcrun', ['--find', 'ditto']], ['/usr/bin/security', ['find-identity', '-v', '-p', 'codesigning']],
  ]);
});
test('candidate source must be clean and match commit, bytes and file count', () => {
  assert.equal(compareSource({source}, source).state, 'passed');
  for (const change of [{dirty: true}, {head: 'd'.repeat(40)}, {snapshot_sha256: 'e'.repeat(64)}, {files: 11}, {head: 'invalid'}]) {
    assert.equal(compareSource({source}, {...source, ...change}).state, 'blocked');
    assert.equal(compareSource({source: {...source, ...change}}, source).state, 'blocked');
  }
  assert.equal(compareSource({}, source).state, 'blocked');
});
test('absent or invalid candidate never implies completion; verification precedes source access', async () => {
  assert.equal((await inspectCandidate(null))[0].state, 'not_checked');
  const failed = await inspectCandidate('/synthetic/tree', {verify: async () => { throw new Error('private-path'); },
    snapshot: async () => { assert.fail('must not run'); }});
  assert.equal(failed[0].reason, 'preview_integrity_check_failed');
  assert.ok(!JSON.stringify(failed).includes('private-path'));
});
test('source read failure is closed, and matching source is still not release evidence', async () => {
  const options = {verify: async () => ({source}), snapshot: async () => source};
  const good = await inspectCandidate('/synthetic/tree', options);
  const report = reportFor([...environment(), ...good]);
  assert.equal(report.status, 'preflight_only');
  assert.equal(report.publishable, false);
  assert.ok(report.not_verified.includes('developer_id_signature'));
  assert.ok(report.not_verified.includes('notarization'));
  assert.ok(report.not_verified.includes('fresh_mac_marketplace_install'));
  const bad = await inspectCandidate('/synthetic/tree', {...options, snapshot: async () => { throw new Error('private-path'); }});
  assert.equal(state(bad, 'candidate_source').state, 'blocked');
});
test('invalid CLI options fail without echoing caller-supplied private arguments', () => {
  for (const args of [[], ['publish', 'private-secret'], ['environment', 'private-secret'], ['candidate']]) {
    assert.throws(() => execFileSync(process.execPath, [join(repository, 'distribution/codex/release-preflight.mjs'), ...args],
      {stdio: 'pipe'}), error => error.status === 2 && !error.stderr.toString().includes('private-secret'));
  }
});
test('missing or duplicated check records cannot pass the preflight', async () => {
  assert.equal(reportFor([]).status, 'blocked_or_incomplete');
  assert.equal(reportFor(environment()).status, 'blocked_or_incomplete');
  const checks = [...environment(), ...await inspectCandidate('/synthetic/tree', {
    verify: async () => ({source}), snapshot: async () => source,
  })];
  assert.equal(reportFor([...checks, checks[0]]).status, 'blocked_or_incomplete');
});
test('preflight regression is included in the existing read-only preview CI', async () => {
  const workflow = await readFile(join(repository, '.github/workflows/codex-preview.yml'), 'utf8');
  assert.match(workflow, /distribution\/codex\/release-preflight\.test\.mjs/);
});
