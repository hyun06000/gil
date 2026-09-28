import assert from 'node:assert/strict';
import {test} from 'node:test';
import {execFileSync} from 'node:child_process';
import {readFile} from 'node:fs/promises';
import {join} from 'node:path';
import {repository} from './artifact.mjs';
import {checkTransition, validateRecord} from './update-policy.mjs';

// All hashes and versions in these fixtures are synthetic, not a published release record.
const record = (version = '0.2.0') => ({schema: 1, plugin: 'gil-companion-prototype', platform: 'darwin-arm64', version,
  source_commit: 'a'.repeat(40), source_snapshot_sha256: 'b'.repeat(64), archive_sha256: 'c'.repeat(64),
  core_sha256: 'd'.repeat(64), ui_sha256: 'e'.repeat(64), migration: 'none',
  compatibility: {protocol: {min: 1, max: 1}, action_surface: {min: 1, max: 1},
    storage_format: {min: 4, max: 4}, monitor_view_schema: {min: 1, max: 1}, node_detail_schema: {min: 1, max: 1}}});

test('same version and same record is a no-op, including reordered JSON fields', () => {
  const before = record(), after = Object.fromEntries(Object.entries(before).reverse());
  assert.equal(checkTransition(before, after).status, 'unchanged');
});
test('same version can never silently change bytes, source or compatibility claims', () => {
  for (const key of ['archive_sha256', 'core_sha256', 'ui_sha256', 'source_snapshot_sha256', 'source_commit']) {
    const after = record(); after[key] = 'f'.repeat(key === 'source_commit' ? 40 : 64);
    assert.equal(checkTransition(record(), after).reason, 'same_version_has_different_bytes_or_claims');
  }
  const after = record(); after.migration = 'requires_review';
  assert.equal(checkTransition(record(), after).status, 'blocked');
});
test('development cachebusters, build metadata, malformed semver and leading zeroes are refused', () => {
  for (const version of ['0.2.0+codex.20260928', '0.2.0+build.1', '01.2.0', '0.2', '0.2.0-01', '0.2.0-', '', 'x'.repeat(81)]) {
    assert.throws(() => validateRecord(record(version)));
  }
  for (const version of ['0.2.0-rc.1', '0.2.0-beta.0', '0.2.0-1a']) assert.doesNotThrow(() => validateRecord(record(version)));
});
test('version ordering is numeric, not lexical, and follows prerelease semantics', () => {
  for (const [before, after] of [['0.2.9','0.2.10'], ['0.2.0-rc.9','0.2.0-rc.10'],
    ['0.2.0-rc.1','0.2.0'], ['0.2.0-1','0.2.0-alpha'], ['0.2.0-alpha','0.2.0-alpha.1']]) {
    assert.equal(checkTransition(record(before), record(after)).status, 'pair_test_required');
    assert.equal(checkTransition(record(after), record(before)).status, 'blocked');
    assert.equal(checkTransition(record(after), record(before), 'rollback').status, 'pair_test_required');
  }
});
test('rollback direction cannot be used to install a newer version', () => {
  assert.equal(checkTransition(record(), record('0.3.0'), 'rollback').reason, 'version_direction_mismatch');
  assert.throws(() => checkTransition(record(), record(), 'anything'));
});
test('any contract-range change requires explicit review, even overlapping ranges', () => {
  for (const key of Object.keys(record().compatibility)) {
    const after = record('0.3.0'); after.compatibility[key].max++;
    assert.equal(checkTransition(record(), after).reason, 'explicit_contract_and_migration_review_required');
    assert.equal(checkTransition(after, record(), 'rollback').status, 'blocked');
  }
});
test('a migration in either direction is not silently treated as a safe rollback', () => {
  for (const which of ['previous', 'next']) {
    const previous = record('0.3.0'), next = record();
    (which === 'previous' ? previous : next).migration = 'requires_review';
    assert.equal(checkTransition(previous, next, 'rollback').status, 'blocked');
  }
});
test('missing fields, unknown fields and nonrecords fail closed', () => {
  for (const value of [null, [], {}, {...record(), extra: true}]) assert.throws(() => validateRecord(value));
  for (const key of Object.keys(record())) { const value = record(); delete value[key]; assert.throws(() => validateRecord(value)); }
});
test('different identity or platform is not an update', () => {
  for (const patch of [{plugin: 'gil'}, {platform: 'win32-x64'}, {schema: 2}, {migration: 'yes'}]) {
    assert.throws(() => validateRecord({...record(), ...patch}));
  }
});
test('hashes and ranges are strict; strings and reversed ranges cannot pass', () => {
  for (const invalid of ['', 'g'.repeat(64), 'F'.repeat(64), null]) {
    assert.throws(() => validateRecord({...record(), core_sha256: invalid}));
  }
  for (const range of [{min: 0,max: 1}, {min: '1',max: 1}, {min: 2,max: 1}, {min: 1,max: Infinity}, {min: 1,max: 1,extra: true}]) {
    const value = record(); value.compatibility.protocol = range;
    assert.throws(() => validateRecord(value));
  }
});
test('consistent metadata never authorizes installation, publication or claims pair acceptance', () => {
  const before = record(), after = record('0.2.1');
  const original = JSON.stringify([before,after]);
  const result = checkTransition(before,after);
  assert.equal(result.publishable,false);
  assert.equal(result.install_authorized,false);
  assert.equal(result.status,'pair_test_required');
  assert.equal(JSON.stringify([before,after]),original);
});
test('CLI refuses malformed or missing inputs without printing their paths', () => {
  for (const args of [[], ['publish','private-secret','private-secret'], ['update','private-secret','private-secret']]) {
    assert.throws(() => execFileSync(process.execPath,[join(repository,'distribution/codex/update-policy.mjs'),...args],{stdio:'pipe'}),
      error => error.status === 2 && !error.stderr.toString().includes('private-secret'));
  }
});
test('read-only preview CI includes the policy regression', async () => {
  assert.match(await readFile(join(repository,'.github/workflows/codex-preview.yml'),'utf8'), /distribution\/codex\/update-policy\.test\.mjs/);
});
