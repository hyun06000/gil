#!/usr/bin/env node
// GIL U2 capability probe — isolated, single test area, file-level operations only.
// Roles: `node gil-u2-probe.mjs` (harness) and `node gil-u2-probe.mjs --server <store>` (synthetic
// server context: a separate process speaking newline-delimited JSON-RPC over stdio; it is NOT a
// full MCP implementation and never connects to Codex, the Desktop app or an existing GIL).
// It creates nothing outside one new unique folder under the current working directory, never
// renames directories, never overwrites a selection file in place, never elevates, never changes
// ACLs or settings, never touches an existing GIL installation or Project.
import { createHash, randomBytes } from 'node:crypto';
import { spawn, execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import readline from 'node:readline';
import { fileURLToPath } from 'node:url';

const SCHEMA = 'gil-u2-probe/1';
const SELF = fileURLToPath(import.meta.url);
const sha256 = (buf) => createHash('sha256').update(buf).digest('hex');
// present → digest; absent only for ENOENT; every other read failure (EACCES, EPERM, EBUSY, …) is
// an error and is never judged as "absent" or as "preserved".
const digestOf = (p) => {
  // Harness self-test only: after the replace attempt in op3, reads of the named file fail.
  if (globalThis.__gilReadErrorArmed && path.basename(p) === process.env.GIL_U2_PROBE_SELFTEST_READ_ERROR_AFTER_RENAME) {
    return { state: 'error', digest: null, error: 'EACCES: simulated by harness self-test' };
  }
  try { return { state: 'present', digest: sha256(fs.readFileSync(p)), error: null }; }
  catch (e) { return e.code === 'ENOENT' ? { state: 'absent', digest: null, error: 'ENOENT' } : { state: 'error', digest: null, error: `${e.code ?? e.name}: ${e.message}` }; }
};
const sameBytes = (a, b) => a.state === 'present' && b.state === 'present' && a.digest === b.digest;
const isDigest = (d, hex) => d.state === 'present' && d.digest === hex;
const isAbsent = (d) => d.state === 'absent';
const redact = (s) => {
  if (typeof s !== 'string') return s;
  const home = os.homedir();
  return home && s.toLowerCase().startsWith(home.toLowerCase()) ? '<home>' + s.slice(home.length) : s;
};
// Every lookup records whether it succeeded; comparisons require both sides to have succeeded.
const identity = (p) => {
  const out = { given: redact(p), realpath: null, realpath_error: null, dev: null, ino: null, stat_error: null };
  if (process.env.GIL_U2_PROBE_SELFTEST_IDENTITY_BLIND === '1') { out.realpath_error = 'simulated by harness self-test'; out.stat_error = 'simulated by harness self-test'; return out; }
  try { out.realpath = redact(fs.realpathSync.native(p)); } catch (e) { out.realpath_error = `${e.code ?? e.name}: ${e.message}`; }
  try { const st = fs.statSync(p, { bigint: true }); out.dev = st.dev.toString(); out.ino = st.ino.toString(); out.isDirectory = st.isDirectory(); out.size = Number(st.size); } catch (e) { out.stat_error = `${e.code ?? e.name}: ${e.message}`; }
  return out;
};
const identityComplete = (v) => !!(v && v.realpath && v.realpath_error === null && v.dev && v.ino && v.stat_error === null);
const who = () => {
  const out = { username: os.userInfo().username, env_USERNAME: process.env.USERNAME ?? null, env_USERDOMAIN: process.env.USERDOMAIN ?? null, pid: process.pid, ppid: process.ppid };
  if (process.platform === 'win32') {
    // Absolute path: under an MSYS/Git-Bash PATH a different `whoami` would answer.
    const whoami = path.join(process.env.SystemRoot ?? 'C:\\Windows', 'System32', 'whoami.exe');
    try { out.whoami_user = execFileSync(whoami, ['/user', '/fo', 'csv', '/nh'], { encoding: 'utf8', timeout: 10000 }).trim(); }
    catch (e) { out.whoami_user = `unavailable:${e.code ?? e.message}`; }
  }
  return out;
};

// ───────────────────────── digest role (tests only) ─────────────────────────
if (process.argv[2] === '--digest') {
  process.stdout.write(JSON.stringify(digestOf(process.argv[3])) + '\n');
  process.exit(0);
}
// ───────────────────────── synthetic server role ─────────────────────────
if (process.argv[2] === '--server') {
  const store = process.argv[3];
  const rl = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
  const reply = (id, result) => process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id, result }) + '\n');
  rl.on('line', (line) => {
    let msg; try { msg = JSON.parse(line); } catch { return; }
    if (msg.method === 'initialize') return reply(msg.id, { schema: SCHEMA, role: 'server', pid: process.pid, node: process.version, who: who() });
    if (msg.method === 'tools/call' && msg.params?.name === 'store_identity') {
      if (process.env.GIL_U2_PROBE_SELFTEST_SERVER_GARBAGE === '1') return process.stdout.write('this is not json\n{"jsonrpc":"2.0"\n');
      return reply(msg.id, { store: identity(store), marker: { ...digestOf(path.join(store, 'marker.bin')), ...identity(path.join(store, 'marker.bin')) } });
    }
    if (msg.method === 'tools/call' && msg.params?.name === 'read_selection') {
      const p = path.join(store, 'selection.json');
      let text = null, parsed = null, error = null;
      try { text = fs.readFileSync(p, 'utf8'); parsed = JSON.parse(text); } catch (e) { error = `${e.code ?? e.name}: ${e.message}`; }
      return reply(msg.id, { ...digestOf(p), text, parsed, error, pid: process.pid });
    }
    reply(msg.id, { error: 'unknown method' });
  });
  // Self-test only: delay the exit after stdin closes so the harness's bounded wait expires.
  const slow = Number(process.env.GIL_U2_PROBE_SELFTEST_SERVER_SLOW_EXIT_MS ?? 0);
  rl.on('close', () => setTimeout(() => process.exit(0), slow));
} else {
  await harness();
}

// ───────────────────────── harness role ─────────────────────────
async function harness() {
  const result = { schema: SCHEMA, started: new Date().toISOString(), node: process.version, platform: `${process.platform}/${process.arch}`, os: `${os.type()} ${os.release()}`, harness: { pid: process.pid, script_sha256: sha256(fs.readFileSync(SELF)), who: who() }, plan: null, test_area: null, persistent_candidate: null, identity: null, ops: [], children: [], stop: null };
  const emit = (code) => {
    result.finished = new Date().toISOString();
    let text = JSON.stringify(result, null, 1);
    if (result.test_area?.path_real) { try { fs.writeFileSync(path.join(result.test_area.path_real_raw, 'result.json'), text); } catch {} }
    // Self-test only: emit an unparsable result block so the test wrapper proves raw evidence survives.
    if (process.env.GIL_U2_PROBE_SELFTEST_CORRUPT_RESULT === '1') text = '{ "schema": "gil-u2-probe/1", "truncated": tru';
    process.stdout.write(`\nGIL_U2_PROBE_RESULT_BEGIN\n${text}\nGIL_U2_PROBE_RESULT_END\n`);
    process.exit(code);
  };
  // 1. Plan first (printed before anything is created).
  const cwd = process.cwd();
  const areaName = `gil-u2-probe-${new Date().toISOString().replace(/[:.]/g, '-')}-${process.pid}`;
  const area = path.join(cwd, areaName);
  result.plan = {
    location: `one new unique folder under the current working directory: ${redact(area)}`,
    operations: ['identity: harness vs separate synthetic server process on the same store (realpath, dev/ino, marker bytes digest)', 'op1 exclusive create (wx) + conflict preservation', 'op2 write/fsync/close/re-read', 'op3 replace selection with a prepared small file via fs.renameSync (file rename with replace; no directory rename, no in-place overwrite)', 'op4 read selection from a NEW server connection', 'op5 preservation check of selection and candidate bytes'],
    change_scope: 'creates and modifies files only inside that folder; nothing else is read for writing; no existing GIL, Codex settings, Project or installation is touched',
    stop_conditions: ['working directory not writable → stop, no other location is tried', 'any policy/permission refusal is recorded as that operation\'s failure; no retry, no fallback to plain overwrite', 'unexpected exception → stop with partial results'],
    not_claimed: ['persistent-storage validation (the candidate location is only reported read-only, never written)', 'power-loss durability', 'concurrent-writer safety', 'real MCP protocol / real Desktop behaviour'],
  };
  process.stdout.write(`GIL_U2_PROBE_PLAN ${JSON.stringify(result.plan)}\n`);
  // Read-only report of the persistent-storage candidate location (never written here).
  const la = process.env.LOCALAPPDATA ?? null;
  result.persistent_candidate = { env_LOCALAPPDATA: la ? redact(la) : null, exists: la ? fs.existsSync(la) : null, validated: false, note: 'reported read-only only; this run validates a temporary folder, not a persistent store' };
  // 2. Test area (exclusive mkdir).
  try { fs.mkdirSync(area); } catch (e) { result.stop = { where: 'test_area', error: `${e.code ?? e.name}: ${e.message}` }; emit(2); }
  const areaReal = fs.realpathSync.native(area);
  result.test_area = { path: redact(area), path_real: redact(areaReal), path_real_raw: areaReal, cwd: redact(cwd), identity: identity(area) };
  const marker = path.join(area, 'marker.bin');
  fs.writeFileSync(marker, randomBytes(4096), { flag: 'wx' });
  const markerDigest = digestOf(marker);
  // Synthetic server client.
  const children = result.children;
  const CALL_TIMEOUT_MS = Number(process.env.GIL_U2_PROBE_SELFTEST_CALL_TIMEOUT_MS ?? 15000);
  const EXIT_WAIT_MS = Number(process.env.GIL_U2_PROBE_SELFTEST_EXIT_WAIT_MS ?? 15000);
  // One child per connection. Whatever happens inside `body` (timeout, malformed reply, early
  // exit, exception), stdin is closed and the exit is awaited for a bounded time. An unconfirmed
  // exit is reported as a failure; the child is never killed.
  const withServer = async (label, body) => {
    const rec = { label, pid: null, exit_code: null, signal: null, exit_confirmed: false, spawn_error: null, malformed_lines: 0, stderr: '' };
    children.push(rec);
    const child = spawn(process.execPath, [SELF, '--server', area], { stdio: ['pipe', 'pipe', 'pipe'], windowsHide: true });
    rec.pid = child.pid ?? null;
    let exited = false;
    const exitPromise = new Promise((res) => child.once('exit', (code, signal) => { exited = true; rec.exit_code = code; rec.signal = signal; res(); }));
    child.once('error', (e) => { rec.spawn_error = `${e.code ?? e.name}: ${e.message}`; });
    child.stderr.on('data', (d) => { rec.stderr += d.toString().slice(0, 2000); });
    const rl = readline.createInterface({ input: child.stdout, crlfDelay: Infinity });
    const pending = new Map(); let next = 1;
    rl.on('line', (line) => { let m; try { m = JSON.parse(line); } catch { rec.malformed_lines += 1; return; } const p = pending.get(m?.id); if (p) { pending.delete(m.id); p(m.result); } else { rec.malformed_lines += 1; } });
    const call = (method, params) => new Promise((res, rej) => {
      if (exited) return rej(new Error(`child already exited before ${method}`));
      const id = next++; const timer = setTimeout(() => { if (pending.has(id)) { pending.delete(id); rej(new Error(`timeout after ${CALL_TIMEOUT_MS} ms: ${method}`)); } }, CALL_TIMEOUT_MS);
      pending.set(id, (r) => { clearTimeout(timer); res(r); });
      child.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n', (e) => { if (e) { clearTimeout(timer); pending.delete(id); rej(e); } });
    });
    let bodyError = null, out;
    try { out = await body(call, rec); }
    catch (e) { bodyError = e; }
    finally {
      try { child.stdin.end(); } catch {}
      await Promise.race([exitPromise, new Promise((res) => setTimeout(res, EXIT_WAIT_MS))]);
      rec.exit_confirmed = exited;
    }
    if (bodyError) throw bodyError;
    if (!rec.exit_confirmed) throw new Error(`child ${rec.pid} exit not confirmed within ${EXIT_WAIT_MS} ms (not killed)`);
    return out;
  };
  const op = async (name, fn) => {
    const r = { name, status: 'pending', started: new Date().toISOString() };
    result.ops.push(r);
    // Harness self-test only: make a named operation throw, to prove failures are never
    // reported as success. Never set outside the test file.
    if (process.env.GIL_U2_PROBE_SELFTEST_OP_THROWS === name) fn = async () => { throw new Error('simulated by harness self-test'); };
    try { const out = await fn(); Object.assign(r, out); if (r.status === 'pending') r.status = 'ok'; }
    catch (e) { r.status = 'failed'; r.error = `${e.code ?? e.name}: ${e.message}`; }
    r.finished = new Date().toISOString();
    process.stdout.write(`GIL_U2_PROBE_OP ${JSON.stringify(r)}\n`);
    return r;
  };
  // 3. Identity: harness view vs separate process view of the same store.
  await op('identity', async () => {
    const h = { store: identity(area), marker: { ...markerDigest, ...identity(marker) }, who: result.harness.who };
    const { init, s } = await withServer('identity', async (call) => {
      const init = await call('initialize', {});
      const s = await call('tools/call', { name: 'store_identity' });
      return { init, s };
    });
    // Completeness first: both views must have succeeded in every lookup, otherwise no comparison.
    const complete = { harness_store: identityComplete(h.store), harness_marker: identityComplete(h.marker) && h.marker.state === 'present', server_store: identityComplete(s?.store), server_marker: identityComplete(s?.marker) && s?.marker?.state === 'present', server_init: !!(init && init.pid && init.who) };
    if (!Object.values(complete).every(Boolean)) {
      return { status: 'failed', reason: 'identity lookups incomplete on at least one side; sameness not evaluated', complete, harness_view: h, server_view: s ?? null, same: null };
    }
    const same = { realpath: h.store.realpath === s.store.realpath, dev_ino: h.store.dev === s.store.dev && h.store.ino === s.store.ino, marker_digest: h.marker.digest === s.marker.digest, marker_dev_ino: h.marker.dev === s.marker.dev && h.marker.ino === s.marker.ino, username: init.who.username === h.who.username, whoami_user: (init.who.whoami_user ?? null) === (h.who.whoami_user ?? null) };
    const status = same.realpath && same.dev_ino && same.marker_digest && same.marker_dev_ino ? 'ok' : 'failed';
    return { status, complete, harness_view: h, server_view: { ...s, who: init.who, server_pid: init.pid }, same, note: 'identity is judged by dev/ino and marker bytes, not by the path string alone; user/token differences are recorded as observed' };
  });
  // 4. Operations, each measured independently.
  const sel = path.join(area, 'selection.json');
  const cand = path.join(area, 'selection.candidate.json');
  const v1 = Buffer.from(JSON.stringify({ selection: 'v1', generation: 1 }) + '\n');
  const v2 = Buffer.from(JSON.stringify({ selection: 'v2', generation: 2 }) + '\n');
  const op1 = await op('op1_exclusive_create_and_conflict', async () => {
    const fd = fs.openSync(sel, 'wx'); fs.writeSync(fd, v1); fs.fsyncSync(fd); fs.closeSync(fd);
    const before = digestOf(sel);
    let conflict = null;
    try { const fd2 = fs.openSync(sel, 'wx'); fs.closeSync(fd2); conflict = 'NO_ERROR (unexpected: exclusive create succeeded on an existing file)'; }
    catch (e) { conflict = e.code; }
    const after = digestOf(sel);
    const preserved = sameBytes(before, after);
    const status = conflict === 'EEXIST' && preserved && isDigest(before, sha256(v1)) ? 'ok' : 'failed';
    return { status, created: before, after, conflict_error: conflict, preserved_after_conflict: preserved };
  });
  await op('op2_write_fsync_close_reread', async () => {
    const data = randomBytes(65536); const p = path.join(area, 'data.bin');
    const fd = fs.openSync(p, 'wx'); fs.writeSync(fd, data); fs.fsyncSync(fd); fs.closeSync(fd);
    const back = fs.readFileSync(p);
    return { status: back.equals(data) ? 'ok' : 'failed', bytes: data.length, digest: sha256(data), reread_digest: sha256(back) };
  });
  const op3 = await op('op3_replace_selection_by_file_rename', async () => {
    if (op1.status !== 'ok') return { status: 'skipped', reason: 'op1 failed' };
    const fd = fs.openSync(cand, 'wx'); fs.writeSync(fd, v2); fs.fsyncSync(fd); fs.closeSync(fd);
    const selBefore = digestOf(sel), candBefore = digestOf(cand);
    if (selBefore.state !== 'present' || candBefore.state !== 'present') return { status: 'failed', reason: 'could not read selection/candidate before the replace', selection_before: selBefore, candidate_before: candBefore };
    let error = null;
    // Harness self-test only (never set in the user prompt): exercise the failure branch locally.
    if (process.env.GIL_U2_PROBE_SELFTEST_RENAME_FAILS === '1') error = 'EPERM: simulated by harness self-test (no rename attempted)';
    else { try { fs.renameSync(cand, sel); } catch (e) { error = `${e.code ?? e.name}: ${e.message}`; } }
    if (process.env.GIL_U2_PROBE_SELFTEST_READ_ERROR_AFTER_RENAME) globalThis.__gilReadErrorArmed = true;
    const selAfter = digestOf(sel), candAfter = digestOf(cand);
    if (error === null) {
      const ok = isDigest(selAfter, sha256(v2)) && isAbsent(candAfter);
      return { status: ok ? 'ok' : 'failed', primitive: 'fs.renameSync(candidate → selection) (file rename with replace)', selection_before: selBefore, selection_after: selAfter, candidate_after: candAfter, replaced: ok };
    }
    // Preservation needs both files readable and byte-identical; a read error is not preservation.
    const preserved = sameBytes(selAfter, selBefore) && sameBytes(candAfter, candBefore);
    return { status: 'failed', primitive: 'fs.renameSync(candidate → selection)', error, replaced: false, selection_before: selBefore, selection_after: selAfter, candidate_before: candBefore, candidate_after: candAfter, preserved_on_failure: preserved, note: 'no fallback to plain overwrite; the failed primitive is recorded as unsupported here' };
  });
  await op('op4_read_selection_from_new_connection', async () => {
    const expected = op3.replaced ? sha256(v2) : (op1.status === 'ok' ? sha256(v1) : null);
    const { init, r } = await withServer('read_after_replace', async (call) => {
      const init = await call('initialize', {});
      const r = await call('tools/call', { name: 'read_selection' });
      return { init, r };
    });
    const ok = expected !== null && r?.state === 'present' && r.digest === expected && r.parsed?.selection === (op3.replaced ? 'v2' : 'v1');
    return { status: ok ? 'ok' : 'failed', new_server_pid: init?.pid ?? null, read: r ? { state: r.state, digest: r.digest, parsed: r.parsed, error: r.error } : null, expected_digest: expected, expected_selection: op3.replaced ? 'v2' : 'v1', note: op3.replaced ? 'read after a successful replace' : 'read after a failed/skipped replace: verifies the old selection is still what a new connection sees' };
  });
  await op('op5_preservation_check', async () => {
    const s = digestOf(sel), c = digestOf(cand);
    const expectSel = op3.replaced ? sha256(v2) : sha256(v1);
    const expectCand = op3.replaced ? 'absent' : (op3.status === 'skipped' ? 'absent' : sha256(v2));
    // A read error on either file is a failed check, never "absent" and never "preserved".
    const ok = isDigest(s, expectSel) && (expectCand === 'absent' ? isAbsent(c) : isDigest(c, expectCand));
    return { status: ok ? 'ok' : 'failed', selection: s, candidate: c, expected_selection: expectSel, expected_candidate: expectCand };
  });
  // 5. Children must have exited; nothing is killed.
  await new Promise((r) => setTimeout(r, 300));
  result.children_all_exited = children.length > 0 && children.every((c) => c.exit_confirmed && c.spawn_error === null);
  result.summary = { ops: Object.fromEntries(result.ops.map((o) => [o.name, o.status])), identity_same_store: result.ops.find((o) => o.name === 'identity')?.same ?? null, replace_supported_here: op3.replaced === true, children_all_exited: result.children_all_exited, final_digests: { selection: digestOf(sel), candidate: digestOf(cand), marker: digestOf(marker) }, validated_persistent_store: false };
  emit(result.ops.every((o) => o.status === 'ok') && result.children_all_exited ? 0 : 1);
}
