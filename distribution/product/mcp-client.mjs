// Dependency-free stdio MCP client for maintainer checks. The child gets an explicit, minimal
// environment: no inherited credentials, no Node on PATH, an isolated HOME. One request has one
// 30-second deadline and is never retried, so a slow or lost answer is reported, not hidden.
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';

export const DEADLINE_MS = 30000;

export class Mcp {
  constructor(binary, {cwd, env}) {
    this.child = spawn(binary, ['mcp', '--serve'], {cwd, env, stdio: ['pipe', 'pipe', 'pipe']});
    this.serial = 0; this.pending = new Map(); this.buffer = ''; this.fault = undefined;
    const fail = error => {
      this.fault ??= error;
      for (const {reject, timer} of this.pending.values()) { clearTimeout(timer); reject(error); }
      this.pending.clear();
    };
    this.child.on('error', () => fail(new Error('native MCP launch failed')));
    this.child.on('exit', () => fail(new Error('native MCP exited')));
    this.child.stderr.resume(); // Never copy runtime diagnostics or local paths into receipts.
    this.child.stdin.on('error', () => fail(new Error('native MCP input closed')));
    this.child.stdout.setEncoding('utf8');
    this.child.stdout.on('data', chunk => {
      this.buffer += chunk;
      if (this.buffer.length > 16 * 1024 * 1024) return fail(new Error('MCP stdout exceeded bound'));
      while (this.buffer.includes('\n')) {
        const end = this.buffer.indexOf('\n'), line = this.buffer.slice(0, end);
        this.buffer = this.buffer.slice(end + 1);
        if (!line.trim()) continue;
        let response;
        try { response = JSON.parse(line); assert.equal(response.jsonrpc, '2.0'); }
        catch { return fail(new Error('non-protocol output on MCP stdout')); }
        const waiting = this.pending.get(response.id);
        if (!waiting) continue;
        this.pending.delete(response.id); clearTimeout(waiting.timer);
        waiting.resolve(response);
      }
    });
  }
  // The whole JSON-RPC response: callers decide whether an `error` member is the expected answer.
  frame(method, params) {
    return new Promise((resolve, reject) => {
      if (this.fault) return reject(this.fault);
      const id = ++this.serial;
      const timer = setTimeout(() => {
        this.pending.delete(id); reject(new Error(`MCP ${method} timed out at ${DEADLINE_MS / 1000} seconds; no automatic retry`));
      }, DEADLINE_MS);
      this.pending.set(id, {resolve, reject, timer});
      this.child.stdin.write(JSON.stringify({jsonrpc: '2.0', id, method, params}) + '\n');
    });
  }
  async request(method, params) {
    const response = await this.frame(method, params);
    assert.equal(response.error, undefined, `MCP ${method} returned a protocol error`);
    return response.result;
  }
  async initialize() {
    const result = await this.request('initialize', {protocolVersion: '2025-06-18',
      capabilities: {extensions: {'io.modelcontextprotocol/ui': {mimeTypes: ['text/html;profile=mcp-app']}}},
      clientInfo: {name: 'gil-product-check', version: '1'}});
    this.child.stdin.write(JSON.stringify({jsonrpc: '2.0', method: 'notifications/initialized'}) + '\n');
    return result;
  }
  // A tool result (never a protocol error). `ok` is the Core's own verdict when it states one.
  async call(name, args) {
    return this.request('tools/call', {name, arguments: args});
  }
  // The call's raw response, for inputs the wire schema itself must refuse.
  raw(name, args) {
    return this.frame('tools/call', {name, arguments: args});
  }
  async close() {
    const child = this.child;
    if (child.exitCode !== null || child.signalCode !== null) return;
    const stopped = new Promise(resolve => child.once('exit', resolve));
    child.stdin.end(); // EOF is the ordinary end of a stdio session
    const timer = setTimeout(() => child.kill('SIGKILL'), 5000);
    await stopped; clearTimeout(timer);
  }
}

export const succeeded = result => result.isError !== true && result.structuredContent?.ok !== false;
export function mustSucceed(result, what) {
  assert.ok(succeeded(result), `${what}: expected success, got ${JSON.stringify(result.structuredContent ?? result.content).slice(0, 2000)}`);
  return result;
}
export function mustRefuse(result, what) {
  assert.ok(!succeeded(result), `${what}: expected a refusal but the call succeeded`);
  return result;
}
// Everything a result says, guidance blocks included.
export const said = result => [...(result.content ?? []).map(block => block.text ?? ''), result.structuredContent?.said ?? ''].join('\n');
