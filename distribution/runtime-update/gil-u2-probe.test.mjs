// Automated verification of the U2 capability probe (Windows contract track T3.2 / U2).
// Every run happens inside a fresh isolated folder under the OS temp directory; nothing outside
// it is read for writing. Evidence (stdout, stderr, result.json, exit code) of every run is copied
// to target/u2-probe-evidence/<case>/ in the same test run so CI can keep it as an artifact.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const PROBE = path.join(HERE, 'gil-u2-probe.mjs');
const EVIDENCE = path.resolve(HERE, '..', '..', 'target', 'u2-probe-evidence');
const sha256 = (b) => createHash('sha256').update(b).digest('hex');

function isolated(label) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), `gil-u2-${label}-`));
  return dir;
}
// Raw evidence (stdout, stderr, spawn error, exit code/signal) is written BEFORE any parsing, so a
// malformed result block or a crashed harness still leaves the original output in the artifact.
function run(label, env = {}, cwd = isolated(label)) {
  const started = Date.now();
  const r = spawnSync(process.execPath, [PROBE], { cwd, env: { ...process.env, ...env }, encoding: 'utf8', windowsHide: true, timeout: 120000 });
  const out = r.stdout ?? '', err = r.stderr ?? '';
  const dir = path.join(EVIDENCE, label);
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(path.join(dir, 'stdout.log'), out);
  fs.writeFileSync(path.join(dir, 'stderr.log'), err);
  const meta = { label, cwd_kind: 'isolated temp folder', exit_code: r.status, signal: r.signal, spawn_error: r.error ? `${r.error.code ?? r.error.name}: ${r.error.message}` : null, env_injected: Object.keys(env), node: process.version, platform: `${process.platform}/${process.arch}`, elapsed_ms: Date.now() - started, result_present: false, parse_error: null };
  fs.writeFileSync(path.join(dir, 'run.json'), JSON.stringify(meta, null, 1));
  let result = null;
  const m = out.match(/GIL_U2_PROBE_RESULT_BEGIN\n([\s\S]*?)\nGIL_U2_PROBE_RESULT_END/);
  if (!m) meta.parse_error = 'no result block';
  else { try { result = JSON.parse(m[1]); meta.result_present = true; } catch (e) { meta.parse_error = `${e.name}: ${e.message}`; } }
  fs.writeFileSync(path.join(dir, 'run.json'), JSON.stringify(meta, null, 1));
  if (result) fs.writeFileSync(path.join(dir, 'result.json'), JSON.stringify(result, null, 1));
  return { cwd, code: r.status, signal: r.signal, spawn_error: meta.spawn_error, parse_error: meta.parse_error, out, err, result, dir };
}
const digestRole = (file) => JSON.parse(spawnSync(process.execPath, [PROBE, '--digest', file], { encoding: 'utf8', windowsHide: true }).stdout);
// The test process's own token privileges (read only). A token with backup/restore privileges
// enabled (for example a child of an MSYS shell running as administrator) bypasses DACL denies,
// which would make the ACL fixtures below non-enforcing; that state is recorded and asserted.
function tokenPrivileges() {
  if (process.platform !== 'win32') return { platform: process.platform, privileges: null };
  const whoami = path.join(process.env.SystemRoot ?? 'C:\\Windows', 'System32', 'whoami.exe');
  const r = spawnSync(whoami, ['/priv', '/fo', 'csv', '/nh'], { encoding: 'utf8', windowsHide: true });
  const enabled = (r.stdout ?? '').split(/\r?\n/).filter((l) => /Enabled/.test(l)).map((l) => l.split(',')[0].replace(/"/g, ''));
  return { platform: 'win32', exit_code: r.status, enabled, raw: r.stdout ?? r.stderr };
}
function recordFixture(label, record) {
  fs.mkdirSync(path.join(EVIDENCE, label), { recursive: true });
  fs.writeFileSync(path.join(EVIDENCE, label, 'fixture.json'), JSON.stringify(record, null, 1));
}
const FAST = { GIL_U2_PROBE_SELFTEST_CALL_TIMEOUT_MS: '1500', GIL_U2_PROBE_SELFTEST_EXIT_WAIT_MS: '1500' };
const entries = (dir) => fs.readdirSync(dir, { withFileTypes: true });
const probeFolder = (cwd) => entries(cwd).filter((e) => e.isDirectory() && e.name.startsWith('gil-u2-probe-')).map((e) => path.join(cwd, e.name));

test('plan is printed before anything is created and names one location only', () => {
  const r = run('plan');
  const planLine = r.out.split('\n').find((l) => l.startsWith('GIL_U2_PROBE_PLAN '));
  assert.ok(planLine, r.out);
  const plan = JSON.parse(planLine.slice('GIL_U2_PROBE_PLAN '.length));
  assert.match(plan.location, /one new unique folder under the current working directory/);
  assert.ok(plan.stop_conditions.some((s) => /no other location/.test(s)));
  assert.ok(plan.not_claimed.some((s) => /persistent-storage/.test(s)) && plan.not_claimed.some((s) => /durability/.test(s)));
  assert.ok(r.out.indexOf('GIL_U2_PROBE_PLAN') < r.out.indexOf('GIL_U2_PROBE_OP'), 'plan precedes operations');
});

test('normal path: exclusive create, write/fsync/reread, file-rename replace, new-process read, preservation, children exit', () => {
  const r = run('normal');
  assert.equal(r.code, 0, r.out + r.err);
  const j = r.result; assert.ok(j);
  assert.deepEqual(Object.values(j.summary.ops), ['ok', 'ok', 'ok', 'ok', 'ok', 'ok']);
  const id = j.ops.find((o) => o.name === 'identity');
  assert.equal(id.same.realpath, true); assert.equal(id.same.dev_ino, true); assert.equal(id.same.marker_digest, true); assert.equal(id.same.marker_dev_ino, true);
  assert.notEqual(id.server_view.server_pid, j.harness.pid, 'the server view comes from a separate process');
  const op1 = j.ops.find((o) => o.name === 'op1_exclusive_create_and_conflict');
  assert.equal(op1.conflict_error, 'EEXIST'); assert.equal(op1.preserved_after_conflict, true); assert.equal(op1.created.state, 'present');
  const op3 = j.ops.find((o) => o.name === 'op3_replace_selection_by_file_rename');
  assert.equal(op3.replaced, true); assert.deepEqual(op3.selection_after, { state: 'present', digest: sha256(Buffer.from(JSON.stringify({ selection: 'v2', generation: 2 }) + '\n')), error: null });
  assert.equal(op3.candidate_after.state, 'absent'); assert.equal(op3.candidate_after.error, 'ENOENT');
  const op4 = j.ops.find((o) => o.name === 'op4_read_selection_from_new_connection');
  assert.equal(op4.read.state, 'present'); assert.equal(op4.read.parsed.selection, 'v2'); assert.notEqual(op4.new_server_pid, id.server_view.server_pid, 'a new connection is a new process');
  assert.ok(Object.values(id.complete).every(Boolean));
  assert.equal(j.children.length, 2); assert.ok(j.children.every((c) => c.exit_code === 0 && c.exit_confirmed === true && c.spawn_error === null)); assert.equal(j.children_all_exited, true);
  assert.equal(j.summary.validated_persistent_store, false);
  // Evidence written in the same run, identical to stdout; nothing outside the probe folder.
  const folders = probeFolder(r.cwd); assert.equal(folders.length, 1);
  assert.equal(entries(r.cwd).length, 1, 'only the probe folder exists in the isolated area');
  const onDisk = JSON.parse(fs.readFileSync(path.join(folders[0], 'result.json'), 'utf8'));
  assert.deepEqual(onDisk.summary, j.summary);
  assert.equal(sha256(fs.readFileSync(path.join(folders[0], 'selection.json'))), j.summary.final_digests.selection.digest);
  assert.equal(j.harness.script_sha256, sha256(fs.readFileSync(PROBE)));
});

test('rename-failure injection: replace is reported unsupported, originals preserved, new connection still reads v1, exit 1', () => {
  const r = run('rename-fails', { GIL_U2_PROBE_SELFTEST_RENAME_FAILS: '1' });
  assert.equal(r.parse_error, null);
  assert.equal(r.code, 1, r.out);
  const j = r.result; assert.ok(j);
  assert.equal(j.summary.ops.op3_replace_selection_by_file_rename, 'failed');
  assert.equal(j.summary.replace_supported_here, false);
  const op3 = j.ops.find((o) => o.name === 'op3_replace_selection_by_file_rename');
  assert.equal(op3.replaced, false); assert.equal(op3.preserved_on_failure, true); assert.match(op3.error, /EPERM/);
  assert.deepEqual(op3.selection_after, op3.selection_before); assert.deepEqual(op3.candidate_after, op3.candidate_before);
  const op4 = j.ops.find((o) => o.name === 'op4_read_selection_from_new_connection');
  assert.equal(op4.status, 'ok'); assert.equal(op4.read.parsed.selection, 'v1');
  assert.equal(j.summary.ops.op5_preservation_check, 'ok');
  const folder = probeFolder(r.cwd)[0];
  assert.equal(sha256(fs.readFileSync(path.join(folder, 'selection.json'))), op3.selection_before.digest);
  assert.equal(sha256(fs.readFileSync(path.join(folder, 'selection.candidate.json'))), op3.candidate_before.digest, 'the candidate bytes stay on disk');
  assert.ok(j.children.every((c) => c.exit_code === 0));
});

test('a harness failure is never reported as success (thrown operation, non-zero exit, failed status kept)', () => {
  for (const name of ['op2_write_fsync_close_reread', 'identity']) {
    const r = run(`throws-${name}`, { GIL_U2_PROBE_SELFTEST_OP_THROWS: name });
    assert.equal(r.code, 1, r.out);
    const j = r.result; assert.ok(j);
    assert.equal(j.summary.ops[name], 'failed');
    const op = j.ops.find((o) => o.name === name);
    assert.match(op.error, /simulated by harness self-test/);
    assert.ok(Object.values(j.summary.ops).includes('failed'));
  }
});

// The only permission change these tests make is on a freshly created isolated temp folder of
// their own; it is restored afterwards and the restore's success is recorded and asserted.
test('stop condition: an unwritable working directory stops with exit 2 and creates nothing elsewhere', () => {
  const cwd = isolated('unwritable');
  const icacls = process.platform === 'win32' ? path.join(process.env.SystemRoot, 'System32', 'icacls.exe') : null;
  const record = { folder: 'fresh isolated temp folder created by this test', applied: null, restored: null };
  if (icacls) {
    const d = spawnSync(icacls, [cwd, '/deny', '*S-1-1-0:(AD,WD)'], { encoding: 'utf8' });
    record.applied = { command: 'icacls <folder> /deny *S-1-1-0:(AD,WD)', exit_code: d.status, stderr: d.stderr };
    assert.equal(d.status, 0, d.stderr);
  } else {
    fs.chmodSync(cwd, 0o555); record.applied = { command: 'chmod 0555 <folder>', exit_code: 0 };
  }
  try {
    if (process.platform !== 'win32' && process.getuid?.() === 0) return; // root ignores modes
    // Precondition: the deny must actually be enforced for this test's own token before the
    // harness is judged. If it is not, the fixture is reported as non-enforcing (with the token's
    // enabled privileges) instead of silently passing or skipping.
    let direct = null;
    try { const fd = fs.openSync(path.join(cwd, 'fixture-direct-probe.txt'), 'wx'); fs.closeSync(fd); direct = 'created (deny NOT enforced)'; }
    catch (e) { direct = e.code; }
    record.precondition = { direct_create_attempt: direct, token: tokenPrivileges() };
    recordFixture('unwritable', record);
    assert.ok(direct === 'EPERM' || direct === 'EACCES', `fixture must be enforced for this token: ${JSON.stringify(record.precondition)}`);
    const r = run('unwritable', {}, cwd);
    assert.equal(r.code, 2, r.out + r.err);
    assert.equal(r.result.stop.where, 'test_area');
    assert.equal(r.result.ops.length, 0);
    assert.equal(entries(cwd).length, 0, 'nothing was created in the area');
  } finally {
    if (icacls) {
      const u = spawnSync(icacls, [cwd, '/remove:d', '*S-1-1-0'], { encoding: 'utf8' });
      record.restored = { command: 'icacls <folder> /remove:d *S-1-1-0', exit_code: u.status, stderr: u.stderr };
    } else {
      fs.chmodSync(cwd, 0o755); record.restored = { command: 'chmod 0755 <folder>', exit_code: 0 };
    }
    fs.mkdirSync(path.join(EVIDENCE, 'unwritable'), { recursive: true });
    fs.writeFileSync(path.join(EVIDENCE, 'unwritable', 'acl-restore.json'), JSON.stringify(record, null, 1));
    try { fs.rmSync(path.join(cwd, 'fixture-direct-probe.txt'), { force: true }); } catch {}
    assert.equal(record.restored.exit_code, 0, `ACL restore must succeed on the isolated folder: ${JSON.stringify(record.restored)}`);
  }
});

test('digest: ENOENT is absent, a read error is an error, never absent or preserved', () => {
  const dir = isolated('digest');
  assert.deepEqual(digestRole(path.join(dir, 'missing.bin')), { state: 'absent', digest: null, error: 'ENOENT' });
  const f = path.join(dir, 'locked.bin'); fs.writeFileSync(f, 'x');
  const icacls = process.platform === 'win32' ? path.join(process.env.SystemRoot, 'System32', 'icacls.exe') : null;
  const record = { file: 'fresh file in an isolated temp folder created by this test', applied: null, restored: null, precondition: null, digest_role: null };
  let denied = null;
  if (icacls) { const d = spawnSync(icacls, [f, '/deny', '*S-1-1-0:(R)'], { encoding: 'utf8' }); record.applied = { command: 'icacls <file> /deny *S-1-1-0:(R)', exit_code: d.status, stderr: d.stderr }; assert.equal(d.status, 0, d.stderr); }
  else if (process.getuid?.() !== 0) { fs.chmodSync(f, 0o000); record.applied = { command: 'chmod 0000 <file>', exit_code: 0 }; }
  try {
    if (icacls || process.getuid?.() !== 0) {
      // Precondition: the read deny must be enforced for this test's own token.
      let direct = null;
      try { fs.readFileSync(f); direct = 'read (deny NOT enforced)'; } catch (e) { direct = e.code; }
      record.precondition = { direct_read_attempt: direct, token: tokenPrivileges() };
      denied = digestRole(f); record.digest_role = denied;
      recordFixture('digest-read-error', record);
      assert.ok(direct === 'EPERM' || direct === 'EACCES', `fixture must be enforced for this token: ${JSON.stringify(record.precondition)}`);
      assert.equal(denied.state, 'error', JSON.stringify(denied)); assert.equal(denied.digest, null); assert.notEqual(denied.error, 'ENOENT');
    }
  } finally {
    if (icacls) { const u = spawnSync(icacls, [f, '/remove:d', '*S-1-1-0'], { encoding: 'utf8' }); record.restored = { command: 'icacls <file> /remove:d *S-1-1-0', exit_code: u.status, stderr: u.stderr }; recordFixture('digest-read-error', record); assert.equal(u.status, 0, u.stderr); }
    else { fs.chmodSync(f, 0o644); record.restored = { command: 'chmod 0644 <file>', exit_code: 0 }; recordFixture('digest-read-error', record); }
  }
  // Harness level: a read error on the selection file fails the preservation check instead of
  // being counted as absent or preserved.
  const r = run('read-error', { GIL_U2_PROBE_SELFTEST_READ_ERROR_AFTER_RENAME: 'selection.json', GIL_U2_PROBE_SELFTEST_RENAME_FAILS: '1' });
  assert.equal(r.code, 1, r.out);
  const op3 = r.result.ops.find((o) => o.name === 'op3_replace_selection_by_file_rename');
  assert.equal(op3.status, 'failed'); assert.equal(op3.replaced, false);
  assert.equal(op3.selection_after.state, 'error'); assert.equal(op3.preserved_on_failure, false, 'a read error must not be reported as preservation');
  const op4 = r.result.ops.find((o) => o.name === 'op4_read_selection_from_new_connection');
  assert.equal(op4.status, 'ok', 'the separate process (not armed) still reads v1');
  const op5 = r.result.ops.find((o) => o.name === 'op5_preservation_check');
  assert.equal(op5.status, 'failed'); assert.equal(op5.selection.state, 'error');
  assert.equal(r.result.summary.final_digests.selection.state, 'error');
});

test('identity: incomplete lookups on both sides never count as sameness', () => {
  const r = run('identity-blind', { GIL_U2_PROBE_SELFTEST_IDENTITY_BLIND: '1' });
  assert.equal(r.code, 1, r.out);
  const id = r.result.ops.find((o) => o.name === 'identity');
  assert.equal(id.status, 'failed'); assert.equal(id.same, null); assert.match(id.reason, /incomplete/);
  assert.equal(id.complete.harness_store, false); assert.equal(id.complete.server_store, false);
  assert.equal(r.result.summary.identity_same_store, null);
  assert.ok(r.result.children.every((c) => c.exit_confirmed), 'the connection is still closed cleanly');
});

test('child lifecycle: a server that does not exit in time is reported, not killed; a garbage reply times out and is still closed', () => {
  const slow = run('server-slow-exit', { ...FAST, GIL_U2_PROBE_SELFTEST_SERVER_SLOW_EXIT_MS: '4000' });
  assert.equal(slow.code, 1, slow.out);
  const idOp = slow.result.ops.find((o) => o.name === 'identity');
  assert.equal(idOp.status, 'failed'); assert.match(idOp.error, /exit not confirmed .* \(not killed\)/);
  assert.equal(slow.result.children_all_exited, false);
  assert.ok(slow.result.children.some((c) => c.exit_confirmed === false && c.signal === null), 'no signal was sent to the child');
  const garbage = run('server-garbage', { ...FAST, GIL_U2_PROBE_SELFTEST_SERVER_GARBAGE: '1' });
  assert.equal(garbage.code, 1, garbage.out);
  const g = garbage.result.ops.find((o) => o.name === 'identity');
  assert.equal(g.status, 'failed'); assert.match(g.error, /timeout after 1500 ms/);
  const rec = garbage.result.children.find((c) => c.label === 'identity');
  assert.equal(rec.exit_confirmed, true, 'stdin was closed in finally and the exit was awaited'); assert.ok(rec.malformed_lines >= 1);
});

test('wrapper: a malformed result block still leaves raw stdout/stderr and exit evidence', () => {
  const r = run('corrupt-result', { GIL_U2_PROBE_SELFTEST_CORRUPT_RESULT: '1' });
  assert.equal(r.result, null); assert.match(r.parse_error, /SyntaxError/);
  assert.equal(r.code, 0, 'the probe itself ran its operations');
  const saved = fs.readFileSync(path.join(r.dir, 'stdout.log'), 'utf8');
  assert.ok(saved.includes('GIL_U2_PROBE_RESULT_BEGIN') && saved.includes('"truncated": tru'), 'raw block kept verbatim');
  const meta = JSON.parse(fs.readFileSync(path.join(r.dir, 'run.json'), 'utf8'));
  assert.equal(meta.result_present, false); assert.match(meta.parse_error, /SyntaxError/); assert.equal(meta.exit_code, 0);
  assert.ok(!fs.existsSync(path.join(r.dir, 'result.json')));
});

test('evidence index for the artifact', () => {
  const index = { schema: 'gil-u2-probe-evidence/1', node: process.version, platform: `${process.platform}/${process.arch}`, commit: process.env.GIL_PROBE_COMMIT ?? null, token: tokenPrivileges(), cases: fs.readdirSync(EVIDENCE), limits: ['ordinary runner token and filesystem; the Codex Desktop restricted token, sandbox, real MCP connection and persistent-store access are not reproduced', 'success is not a GIL update success and not an atomicity or power-loss durability guarantee', 'rename failure, read errors, blind identity, slow/garbage servers and the corrupt result are injected, not observed', 'the only permission changes are on fresh isolated temp folders/files of these tests and are restored (see unwritable/acl-restore.json)'] };
  fs.writeFileSync(path.join(EVIDENCE, 'INDEX.json'), JSON.stringify(index, null, 1));
  assert.ok(['normal', 'rename-fails', 'read-error', 'identity-blind', 'server-slow-exit', 'server-garbage', 'corrupt-result', 'unwritable'].every((c) => index.cases.includes(c)));
});
