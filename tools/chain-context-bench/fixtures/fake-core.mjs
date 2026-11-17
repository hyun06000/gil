// Test-only stand-in for `gil mcp --serve`. It is NOT a Core: it answers the MCP handshake and
// then behaves as `fake-core.json` next to the copied script says:
//   { "mode": "refuse" }  every tools/call is a typed refusal (ok:false)
//   { "mode": "hang" }    tools/call is never answered (timeouts, interrupts)
// It writes `pid-<pid>` and `called-<pid>` markers into "markDir" so a test can see it ran and
// whether it is still alive. Self-tests copy it into a temporary folder with an absolute
// node shebang; it never touches anything else.
import { readFileSync, writeFileSync } from 'node:fs';
import { createInterface } from 'node:readline';

const config = JSON.parse(readFileSync(new URL('./fake-core.json', import.meta.url), 'utf8'));
const mark = (name) => { if (config.markDir) writeFileSync(`${config.markDir}/${name}-${process.pid}`, String(process.pid)); };
mark('pid');
const send = (frame) => process.stdout.write(JSON.stringify(frame) + '\n');
const TOOLS = ['gil_start', 'gil_open', 'gil_close', 'gil_context', 'gil_status', 'gil_plan_read', 'gil_plan_propose', 'gil_plan_review', 'gil_plan_decide', 'gil_revisit'];

createInterface({ input: process.stdin }).on('line', (line) => {
  let msg;
  try { msg = JSON.parse(line); } catch { return; }
  if (msg.id === undefined) return;
  if (msg.method === 'initialize') return send({ jsonrpc: '2.0', id: msg.id, result: { protocolVersion: '2025-06-18', capabilities: { tools: {} }, serverInfo: { name: 'fake-core', version: '0' } } });
  if (msg.method === 'tools/list') return send({ jsonrpc: '2.0', id: msg.id, result: { tools: TOOLS.map((name) => ({ name, inputSchema: { type: 'object' } })) } });
  if (msg.method === 'tools/call') {
    mark('called');
    if (config.mode === 'hang') return;
    return send({ jsonrpc: '2.0', id: msg.id, result: { content: [{ type: 'text', text: 'synthetic refusal' }], structuredContent: { ok: false, said: '', problem: 'synthetic refusal' }, isError: true } });
  }
  send({ jsonrpc: '2.0', id: msg.id, error: { code: -32601, message: 'not supported by fake-core' } });
});
process.stdin.on('end', () => process.exit(0));
