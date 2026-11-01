// Wave 1 scenario harness: stdio JSON-RPC client for one native `gil mcp --serve` Core.
// Node standard library only. The binary path is always passed explicitly by the caller;
// this module never consults PATH, GIL_BINARY or an installed Core.
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { tmpdir } from 'node:os';

const children = new Set();
let hooked = false;
function hookCleanup() {
  if (hooked) return;
  hooked = true;
  const sweep = () => { for (const c of children) killGroup(c, 'SIGKILL'); };
  process.on('exit', sweep);
  for (const sig of ['SIGINT', 'SIGTERM', 'SIGHUP']) {
    process.on(sig, () => { sweep(); process.exit(130); });
  }
}

/** Kill a detached child's whole process group; falls back to the pid. */
export function killGroup(child, signal = 'SIGTERM') {
  if (!child || child.exitCode !== null || child.signalCode !== null) return false;
  try { process.kill(-child.pid, signal); return true; } catch {}
  try { child.kill(signal); return true; } catch { return false; }
}

const wait = (ms) => new Promise((r) => setTimeout(r, ms));

/** Wait for exit; escalate SIGTERM → SIGKILL. Resolves with {code, signal}. */
export async function reap(child, graceMs = 5000) {
  if (child.exitCode !== null || child.signalCode !== null) return { code: child.exitCode, signal: child.signalCode };
  const exited = new Promise((r) => child.once('exit', (code, signal) => r({ code, signal })));
  const timeout = (ms) => Promise.race([exited, wait(ms).then(() => null)]);
  let got = await timeout(graceMs);
  if (!got) { killGroup(child, 'SIGTERM'); got = await timeout(3000); }
  if (!got) { killGroup(child, 'SIGKILL'); got = await exited; }
  return got;
}

/**
 * One Core process. `binary` is the exact executable to run. `env` is the complete extra
 * environment; PATH is reduced to system directories so no repository or global gil is found.
 */
export class CoreClient {
  constructor({ binary, args = ['mcp', '--serve'], settingsDir, env = {}, cwd = tmpdir(), requestTimeoutMs = 20000, onFrame = null, label = 'core' }) {
    if (!binary) throw new Error('CoreClient requires an explicit binary path');
    this.binary = binary;
    this.args = args;
    this.settingsDir = settingsDir;
    this.env = env;
    this.cwd = cwd;
    this.requestTimeoutMs = requestTimeoutMs;
    this.onFrame = onFrame;
    this.label = label;
    this.next = 1;
    this.waiting = new Map();
    this.stdoutNoise = [];
    this.stderr = '';
    this.child = null;
    this.exit = null;
    this.frames = 0;
  }
  start() {
    hookCleanup();
    const env = { PATH: process.platform === 'win32' ? process.env.PATH : '/usr/bin:/bin', ...this.env };
    if (this.settingsDir) env.GIL_MONITOR_STATE_DIR = this.settingsDir;
    if (process.env.TMPDIR) env.TMPDIR = process.env.TMPDIR;
    if (process.env.HOME) env.HOME = process.env.HOME;
    const child = spawn(this.binary, this.args, { cwd: this.cwd, env, stdio: ['pipe', 'pipe', 'pipe'], detached: process.platform !== 'win32' });
    this.child = child;
    children.add(child);
    child.on('exit', (code, signal) => {
      this.exit = { code, signal };
      children.delete(child);
      for (const [, entry] of this.waiting) entry.reject(new Error(`${this.label}: Core exited (code ${code}, signal ${signal}) before answering ${entry.method}`));
      this.waiting.clear();
    });
    child.stderr.on('data', (b) => { if (this.stderr.length < 65536) this.stderr += b.toString(); });
    createInterface({ input: child.stdout }).on('line', (line) => {
      if (!line.trim()) { this.stdoutNoise.push(line); return; }
      let frame;
      try { frame = JSON.parse(line); } catch { this.stdoutNoise.push(line.slice(0, 500)); return; }
      this.frames += 1;
      if (frame.id !== undefined && this.waiting.has(frame.id)) {
        const entry = this.waiting.get(frame.id);
        this.waiting.delete(frame.id);
        clearTimeout(entry.timer);
        entry.resolve(frame);
      } else if (this.onFrame) {
        this.onFrame({ unsolicited: frame });
      }
    });
    return this;
  }
  send(frame) {
    if (!this.child || !this.child.stdin.writable) throw new Error(`${this.label}: Core stdin is closed`);
    this.child.stdin.write(JSON.stringify(frame) + '\n');
  }
  notify(method, params) { this.send({ jsonrpc: '2.0', method, params }); }
  /** Returns the whole response frame; JSON-RPC errors are returned, not thrown. Timeouts throw. */
  request(method, params, { timeoutMs = this.requestTimeoutMs } = {}) {
    const id = this.next++;
    const started = Date.now();
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.waiting.delete(id);
        reject(new Error(`${this.label}: timeout after ${timeoutMs} ms waiting for ${method}${params?.name ? ` ${params.name}` : ''}`));
      }, timeoutMs);
      this.waiting.set(id, { method, resolve: (frame) => { this.onFrame?.({ method, params, frame, ms: Date.now() - started }); resolve(frame); }, reject, timer });
      try { this.send({ jsonrpc: '2.0', id, method, params }); } catch (err) { clearTimeout(timer); this.waiting.delete(id); reject(err); }
    });
  }
  async initialize(clientInfo = { name: 'gil-wave1-scenarios', version: '0' }) {
    const frame = await this.request('initialize', { protocolVersion: '2025-06-18', capabilities: {}, clientInfo });
    if (frame.error) throw new Error(`initialize failed: ${JSON.stringify(frame.error)}`);
    this.notify('notifications/initialized', {});
    return frame.result;
  }
  /** tools/call convenience: {frame, result, error, ok, structured} */
  async call(name, args, options) {
    const frame = await this.request('tools/call', { name, arguments: args }, options);
    const result = frame.result ?? null;
    const structured = result?.structuredContent ?? null;
    const ok = !frame.error && result && result.isError !== true && (structured === null || structured.ok !== false);
    return { frame, result, error: frame.error ?? null, ok, structured, meta: result?._meta ?? null };
  }
  /** Close stdin and wait for a clean exit. */
  async close(graceMs = 5000) {
    if (!this.child) return null;
    try { this.child.stdin.end(); } catch {}
    const got = await reap(this.child, graceMs);
    return { ...got, stdoutNoise: this.stdoutNoise, stderr: this.stderr, frames: this.frames };
  }
  /** Abrupt termination (used only for cleanup after failures). */
  async kill() {
    if (!this.child) return null;
    killGroup(this.child, 'SIGKILL');
    return reap(this.child, 1000);
  }
}

/** Run one command with a timeout; the whole process group is killed on timeout. */
export function runCommand(command, args, { cwd, env = process.env, timeoutMs = 600000, onStdout = null, onStderr = null } = {}) {
  hookCleanup();
  return new Promise((resolve) => {
    const child = spawn(command, args, { cwd, env, stdio: ['ignore', 'pipe', 'pipe'], detached: process.platform !== 'win32' });
    children.add(child);
    let stdout = '', stderr = '', timedOut = false;
    child.stdout.on('data', (b) => { const s = b.toString(); if (stdout.length < 4 * 1024 * 1024) stdout += s; onStdout?.(s); });
    child.stderr.on('data', (b) => { const s = b.toString(); if (stderr.length < 1024 * 1024) stderr += s; onStderr?.(s); });
    const timer = setTimeout(() => { timedOut = true; killGroup(child, 'SIGTERM'); setTimeout(() => killGroup(child, 'SIGKILL'), 3000); }, timeoutMs);
    child.on('error', (err) => { clearTimeout(timer); children.delete(child); resolve({ code: null, signal: null, stdout, stderr: stderr + String(err), timedOut, error: err.message }); });
    child.on('exit', (code, signal) => { clearTimeout(timer); children.delete(child); resolve({ code, signal, stdout, stderr, timedOut }); });
  });
}
