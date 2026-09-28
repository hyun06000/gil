// Maintainer-only release-record checks. Never downloads, installs, runs a Core or changes a Project.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {resolve} from 'node:path';
import {fileURLToPath} from 'node:url';

const number = '(?:0|[1-9][0-9]*)';
const identifier = '(?:0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*)';
const versionPattern = new RegExp(`^(${number})\\.(${number})\\.(${number})(?:-(${identifier}(?:\\.${identifier})*))?$`);
const contracts = ['protocol', 'action_surface', 'storage_format', 'monitor_view_schema', 'node_detail_schema'];
const fields = ['schema', 'plugin', 'platform', 'version', 'source_commit', 'source_snapshot_sha256',
  'archive_sha256', 'core_sha256', 'ui_sha256', 'compatibility', 'migration'];
function exactKeys(value, keys) {
  assert.ok(value && typeof value === 'object' && !Array.isArray(value), 'expected record');
  assert.ok(JSON.stringify(Object.keys(value).sort()) === JSON.stringify([...keys].sort()), 'unexpected or missing record field');
}
function parsedVersion(value) {
  assert.ok(typeof value === 'string' && value.length <= 80, 'invalid release version');
  const parsed = versionPattern.exec(value);
  // Published versions cannot reuse a development cachebuster or add semver build metadata.
  assert.ok(parsed && !value.includes('codex'), 'release version must be strict semver without development/build metadata');
  return {numbers: parsed.slice(1, 4).map(BigInt), pre: parsed[4]?.split('.') ?? null};
}
export function compareVersion(a, b) {
  const left = parsedVersion(a), right = parsedVersion(b);
  for (let i = 0; i < 3; i++) if (left.numbers[i] !== right.numbers[i]) return left.numbers[i] > right.numbers[i] ? 1 : -1;
  if (!left.pre || !right.pre) return left.pre === right.pre ? 0 : left.pre ? -1 : 1;
  for (let i = 0; i < Math.max(left.pre.length, right.pre.length); i++) {
    const l = left.pre[i], r = right.pre[i];
    if (l === r) continue;
    if (l === undefined || r === undefined) return l === undefined ? -1 : 1;
    const ln = /^\d+$/.test(l), rn = /^\d+$/.test(r);
    if (ln && rn) return BigInt(l) > BigInt(r) ? 1 : -1;
    if (ln !== rn) return ln ? -1 : 1;
    return l > r ? 1 : -1;
  }
  return 0;
}
const canonical = value => value && typeof value === 'object'
  ? Object.fromEntries(Object.keys(value).sort().map(key => [key, canonical(value[key])])) : value;
const equal = (a, b) => JSON.stringify(canonical(a)) === JSON.stringify(canonical(b));

export function validateRecord(record) {
  exactKeys(record, fields);
  assert.ok(record.schema === 1 && record.plugin === 'gil-companion-prototype', 'release identity mismatch');
  assert.ok(record.platform === 'darwin-arm64', 'only the scoped release target is allowed');
  parsedVersion(record.version);
  assert.ok(typeof record.source_commit === 'string' && /^[a-f0-9]{40}$/.test(record.source_commit), 'invalid source commit');
  for (const key of ['source_snapshot_sha256', 'archive_sha256', 'core_sha256', 'ui_sha256']) {
    assert.ok(typeof record[key] === 'string' && /^[a-f0-9]{64}$/.test(record[key]), 'invalid SHA-256');
  }
  exactKeys(record.compatibility, contracts);
  for (const range of Object.values(record.compatibility)) {
    exactKeys(range, ['min', 'max']);
    assert.ok(Number.isSafeInteger(range.min) && Number.isSafeInteger(range.max)
      && range.min >= 1 && range.max >= range.min, 'invalid compatibility range');
  }
  assert.ok(['none', 'requires_review'].includes(record.migration), 'migration statement required');
  return record;
}

export function checkTransition(previous, next, direction = 'update') {
  validateRecord(previous); validateRecord(next);
  assert.ok(['update', 'rollback'].includes(direction), 'unknown transition');
  const answer = (status, reason) => ({schema: 1, status, reason,
    install_authorized: false, publishable: false,
    limits: 'Record consistency only. Hashes are not publisher authentication; compatible ranges do not prove pair acceptance.'});
  if (previous.version === next.version) {
    return equal(previous, next) ? answer('unchanged', 'same_immutable_record')
      : answer('blocked', 'same_version_has_different_bytes_or_claims');
  }
  const order = compareVersion(next.version, previous.version);
  if ((direction === 'update' && order < 0) || (direction === 'rollback' && order > 0)) {
    return answer('blocked', 'version_direction_mismatch');
  }
  if (previous.migration !== 'none' || next.migration !== 'none'
    || !equal(previous.compatibility, next.compatibility)) {
    return answer('blocked', 'explicit_contract_and_migration_review_required');
  }
  return answer('pair_test_required', direction === 'update' ? 'update_pair_not_yet_accepted' : 'rollback_pair_not_yet_accepted');
}

export async function main(args = process.argv.slice(2)) {
  if (args.length !== 3 || !['update', 'rollback'].includes(args[0])) {
    console.error('Usage: update-policy.mjs update|rollback <previous-record.json> <next-record.json>');
    return 2;
  }
  try {
    const previous = JSON.parse(await readFile(args[1], 'utf8'));
    const next = JSON.parse(await readFile(args[2], 'utf8'));
    const result = checkTransition(previous, next, args[0]);
    console.log(JSON.stringify(result, null, 2));
    return result.status === 'blocked' ? 1 : 0;
  } catch {
    console.error('Invalid or unreadable release record; no installation or publication was attempted.');
    return 2;
  }
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) process.exitCode = await main();
