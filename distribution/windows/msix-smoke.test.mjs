import test from 'node:test';
import assert from 'node:assert/strict';
import {smoke} from './msix/smoke.mjs';

// Executable fixture: exercise subprocess exit/timeout and stdout framing, not regex copies
// of the harness. No installed GIL, Windows package or user settings are touched here.
const fixture = `
const {createInterface} = require('node:readline');
const {createHash} = require('node:crypto');
const args = process.argv.slice(1), mode = process.env.SMOKE_FIXTURE;
if (args[0] === '--version') { console.log('gil 1.2.3'); process.exit(0); }
if (args[0] !== 'mcp') {
  if (!args.length && mode === 'bad-cli') process.exit(7);
  console.log('GIL help'); process.exit(0);
}
const text = '<html>GIL Monitor</html>';
const uri = 'ui://gil-monitor/' + createHash('sha256').update(text).digest('hex').slice(0,20) + '.html';
const input = createInterface({input: process.stdin});
input.on('line', line => {
  const req = JSON.parse(line);
  if (req.id === undefined) return;
  if (mode === 'timeout') return;
  if (mode === 'crash') process.exit(9);
  if (mode === 'noise') console.log('debug noise');
  let result;
  if (req.method === 'initialize') result = {serverInfo: {version: mode === 'wrong-version' ? '0' : '1.2.3'}};
  if (req.method === 'tools/list') result = {tools: ['gil_start', 'gil_status', 'show_gil_monitor'].map(name => ({name, _meta:{ui:{resourceUri:uri}}}))};
  if (req.method === 'resources/read') result = {contents:[{uri, mimeType:'text/html;profile=mcp-app', text: mode === 'bad-hash' ? 'corrupted' : text}]};
  console.log(JSON.stringify({jsonrpc:'2.0', id:req.id, result}));
});
input.on('close', () => { if (mode === 'hang-eof') setInterval(() => {}, 1000); });
`;
const run = (mode = '') => smoke(process.execPath, '1.2.3', {
  prefix: ['-e', fixture, '--'], timeoutMs: 1000, extraEnv: {SMOKE_FIXTURE: mode},
});

test('CLI and MCP pass only with matching version, valid Monitor and clean EOF', async () => {
  const result = await run();
  assert.equal(result.passed, true);
  assert.match(result.scope, /no actual Host UI/);
});
for (const [mode, reason] of [
  ['bad-cli', /must exit 0/], ['wrong-version', /same build/],
  ['noise', /non-protocol/], ['crash', /MCP closed/],
  ['timeout', /timed out/], ['bad-hash', /Expected values/],
  ['hang-eof', /stdin EOF/],
]) test(`rejects ${mode} without retrying or leaving the server running`, async () => {
  await assert.rejects(run(mode), reason);
});

test('missing executable fails instead of waiting for MCP timeout', async () => {
  await assert.rejects(smoke('/nonexistent-gil-msix-test/gil', '1.2.3'), /ENOENT/);
});
