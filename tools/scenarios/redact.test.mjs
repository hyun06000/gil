import test from 'node:test';
import assert from 'node:assert/strict';
import { homedir, tmpdir } from 'node:os';
import { join } from 'node:path';
import { makeRedactor, leaks } from './lib/redact.mjs';

test('tokens, labelled roots, home paths and oversize strings are redacted', () => {
  const root = join(tmpdir(), 'gil-wave1-xyz', 'project');
  const token = 'f'.repeat(64);
  const r = makeRedactor({ roots: { project_root: root }, secrets: [token], maxString: 80 });
  const value = { arguments: { token, project_root: root, nested: { 'gil/planApprovalToken': token } }, said: `saved under ${root}/evidence and home ${join(homedir(), 'secret')} with ${token}`, long: 'x'.repeat(100), digest: 'a'.repeat(64), keep: 5, list: [root] };
  const out = r.redact(value);
  assert.equal(out.arguments.token, '<redacted-token>');
  assert.equal(out.arguments.nested['gil/planApprovalToken'], '<redacted-token>');
  assert.equal(out.arguments.project_root, '<project_root>');
  assert.equal(out.list[0], '<project_root>');
  assert.ok(!out.said.includes(token) && !out.said.includes(root) && !out.said.includes(homedir()), out.said);
  assert.match(out.long, /^x{80}…<truncated 20 chars>$/);
  assert.equal(out.digest, 'a'.repeat(64), 'content digests are not secrets');
  assert.equal(out.keep, 5);
  assert.deepEqual(leaks(JSON.stringify(out), { secrets: [token], roots: [root] }), []);
  assert.deepEqual(leaks(JSON.stringify(value), { secrets: [token], roots: [root] }), [token, root]);
});

test('secrets added later are redacted too and null token keys stay null', () => {
  const r = makeRedactor();
  const late = '0123456789abcdef'.repeat(4);
  assert.equal(r.text(`t=${late}`), `t=${late}`);
  r.addSecret(late);
  assert.equal(r.text(`t=${late}`), 't=<redacted-token>');
  assert.deepEqual(r.redact({ token: null, 'gil/planApprovalToken': undefined }), { token: null, 'gil/planApprovalToken': undefined });
});
