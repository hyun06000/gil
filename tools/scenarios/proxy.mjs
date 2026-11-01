#!/usr/bin/env node
// MCP stdio proxy for repeatable model evaluations of one scenario in one temporary project.
//
//   node tools/scenarios/proxy.mjs --core <gil binary> --root <abs temp project> --events <jsonl>
//        [--core-args-json <JSON string array>] [--settings <dir>] [--private-root <dir>] [--scenario <dir>] [--synthetic-app-decision approve|request_changes]
//   node tools/scenarios/proxy.mjs --control --root <abs temp project> --events <jsonl> [--scenario <dir>]
//
// App-only tools (gil_plan_review/decide) are removed from the model-facing tools/list and refused if called.
// --control serves only the scenario tools (no Core, every gil_* call refused): the control arm of
// the same task. Otherwise it forwards JSON-RPC between a Host (stdin/stdout) and the real Core, logs every tools/call as a
// redacted event line, and adds scenario tools (read_project_file, save_project_file, optional
// run_scenario_command) that stay inside --root. App-only approval tools are never offered to the
// model side. With --synthetic-app-decision the HARNESS (not the model) performs review+decide
// through the Core's real App path after each successful proposal; events mark it synthetic and
// the scorer never counts it as user consent. Nothing in the Core is bypassed or disabled.
// --private-root <dir> gives the Core child an isolated HOME (Unix) / LOCALAPPDATA (Windows) below
// that folder, so a Windows record never lands in the real %LOCALAPPDATA%.
import { createInterface } from 'node:readline';
import { appendFileSync, existsSync, readFileSync, mkdirSync, writeFileSync, statSync } from 'node:fs';
import { join, resolve, isAbsolute, normalize, dirname } from 'node:path';
import { CoreClient, runCommand, isolatedEnv } from './lib/rpc.mjs';
import { makeRedactor } from './lib/redact.mjs';

function parseArgs(argv) {
  const o = { core: null, coreArgs: undefined, root: null, events: null, settings: null, privateRoot: null, scenario: null, synthetic: null, timeoutMs: 20000, control: false };
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === '--core') o.core = resolve(argv[++i]);
    else if (a === '--core-args-json') o.coreArgs = JSON.parse(argv[++i]);
    else if (a === '--root') o.root = resolve(argv[++i]);
    else if (a === '--events') o.events = resolve(argv[++i]);
    else if (a === '--settings') o.settings = resolve(argv[++i]);
    else if (a === '--private-root') o.privateRoot = resolve(argv[++i]);
    else if (a === '--scenario') o.scenario = resolve(argv[++i]);
    else if (a === '--synthetic-app-decision') o.synthetic = argv[++i];
    else if (a === '--timeout-ms') o.timeoutMs = Number(argv[++i]);
    else if (a === '--control') o.control = true;
    else throw new Error(`unknown argument ${a}`);
  }
  if (o.coreArgs !== undefined && (!Array.isArray(o.coreArgs) || o.coreArgs.length > 32 || o.coreArgs.some(a => typeof a !== 'string' || a.length > 4096 || a.includes('\0')))) throw new Error('--core-args-json takes a bounded string array');
  if (o.control && (o.core || o.synthetic || o.coreArgs)) throw new Error('--control takes no --core and no synthetic decision');
  if ((!o.core && !o.control) || !o.root || !o.events) throw new Error('--core, --root and --events are required');
  if (o.synthetic && !['approve', 'request_changes'].includes(o.synthetic)) throw new Error('--synthetic-app-decision takes approve or request_changes');
  if (!isAbsolute(o.root) || !existsSync(o.root) || !statSync(o.root).isDirectory()) throw new Error('--root must be an existing absolute directory');
  // The Core creates this directory itself with private permissions; a pre-made 0755 dir is refused.
  if (!o.settings) o.settings = join(dirname(o.root), 'bindings');
  mkdirSync(dirname(o.settings), { recursive: true });
  return o;
}

const APP_ONLY = new Set(['gil_plan_review', 'gil_plan_decide']);
const MAX_READ = 256 * 1024;

function safePath(root, rel) {
  if (typeof rel !== 'string' || !rel || isAbsolute(rel) || rel.includes('\\') || rel.includes('\0')) return null;
  const parts = rel.split('/');
  if (parts.some((p) => p === '' || p === '.' || p === '..')) return null;
  if (parts[0] === '.gil') return null;
  return join(root, normalize(rel));
}

function scenarioTools(commands) {
  const tools = [
    { name: 'read_project_file', description: 'Read one UTF-8 text file inside the selected temporary project (project-relative path, up to 256 KiB). No other folder is reachable.', annotations: { readOnlyHint: true, destructiveHint: false, idempotentHint: true, openWorldHint: false }, inputSchema: { type: 'object', properties: { path: { type: 'string' } }, required: ['path'], additionalProperties: false } },
    { name: 'save_project_file', description: 'Save your actual observations, code or deliverable as a UTF-8 file inside the selected temporary project. Returns the project-relative path for evidence_files. Existing files need overwrite:true. item_id labels which approved plan item this work belongs to.', annotations: { readOnlyHint: false, destructiveHint: false, idempotentHint: false, openWorldHint: false }, inputSchema: { type: 'object', properties: { path: { type: 'string' }, text: { type: 'string', minLength: 1, maxLength: 262144 }, item_id: { type: 'string' }, overwrite: { type: 'boolean' } }, required: ['path', 'text'], additionalProperties: false } },
  ];
  const names = Object.keys(commands || {});
  if (names.length) tools.push({ name: 'run_scenario_command', description: `Run one pre-approved scenario command inside the temporary project and return its exit code and output tails. Allowed names: ${names.join(', ')}.`, annotations: { readOnlyHint: false, destructiveHint: false, idempotentHint: false, openWorldHint: false }, inputSchema: { type: 'object', properties: { name: { type: 'string', enum: names }, item_id: { type: 'string' } }, required: ['name'], additionalProperties: false } });
  return tools;
}

async function main() {
  const o = parseArgs(process.argv.slice(2));
  const rubric = o.scenario && existsSync(join(o.scenario, 'rubric.json')) ? JSON.parse(readFileSync(join(o.scenario, 'rubric.json'), 'utf8')) : {};
  const commands = rubric.commands || {};
  const extra = scenarioTools(commands);
  const extraNames = new Set(extra.map((t) => t.name));
  const redactor = makeRedactor({ roots: { project_root: o.root, settings_dir: o.settings, scenario_dir: o.scenario || '' } });
  let seq = 0;
  const log = (actor, name, args, result, error, ms, synthetic = false) => {
    seq += 1;
    appendFileSync(o.events, JSON.stringify({ seq, at: new Date().toISOString(), actor, synthetic, request: { name, arguments: redactor.redact(args ?? {}) }, result: result ? redactor.redact(result) : null, error: error ? redactor.redact(error) : null, ms }) + '\n');
  };
  if (!existsSync(o.events)) { mkdirSync(dirname(o.events), { recursive: true }); writeFileSync(o.events, ''); }
  let isolated = null;
  if (o.privateRoot) {
    const home = join(o.privateRoot, 'home'); const localAppData = join(o.privateRoot, 'localappdata');
    mkdirSync(home, { recursive: true }); mkdirSync(localAppData, { recursive: true });
    isolated = isolatedEnv({ home, localAppData });
  }
  const core = o.control ? null : new CoreClient({ binary: o.core, args: o.coreArgs, settingsDir: o.settings, isolated, requestTimeoutMs: o.timeoutMs, label: 'core' }).start();
  core?.child.on('exit', () => setTimeout(() => process.exit(0), 50));
  let scope = null;
  const out = (frame) => process.stdout.write(JSON.stringify(frame) + '\n');

  async function local(name, args) {
    if (name === 'read_project_file') {
      const p = safePath(o.root, args.path);
      if (!p || !existsSync(p) || !statSync(p).isFile()) return { content: [{ type: 'text', text: 'File not found inside the project' }], isError: true };
      if (statSync(p).size > MAX_READ) return { content: [{ type: 'text', text: 'File larger than 256 KiB' }], isError: true };
      return { content: [{ type: 'text', text: readFileSync(p, 'utf8') }], isError: false };
    }
    if (name === 'save_project_file') {
      const p = safePath(o.root, args.path);
      if (!p) return { content: [{ type: 'text', text: 'Path must be project-relative, outside .gil, without .. segments' }], isError: true };
      if (typeof args.text !== 'string' || !args.text.trim() || Buffer.byteLength(args.text) > 262144) return { content: [{ type: 'text', text: 'Nonempty text up to 256 KiB required' }], isError: true };
      if (existsSync(p) && args.overwrite !== true) return { content: [{ type: 'text', text: 'File exists; pass overwrite:true to replace it' }], isError: true };
      mkdirSync(dirname(p), { recursive: true });
      writeFileSync(p, args.text);
      return { content: [{ type: 'text', text: JSON.stringify({ saved: true, path: args.path, item_id: args.item_id ?? null }) }], structuredContent: { saved: true, path: args.path, item_id: args.item_id ?? null }, isError: false };
    }
    if (name === 'run_scenario_command') {
      const command = commands[args.name];
      if (!command) return { content: [{ type: 'text', text: 'Unknown scenario command' }], isError: true };
      const [cmd, ...rest] = command;
      const r = await runCommand(cmd === 'node' ? process.execPath : cmd, rest, { cwd: o.root, timeoutMs: 120000, env: { ...process.env, PATH: process.env.PATH || '/usr/bin:/bin' } });
      const payload = { name: args.name, exit_code: r.code, timed_out: r.timedOut, stdout_tail: (r.stdout || '').slice(-4000), stderr_tail: (r.stderr || '').slice(-4000) };
      return { content: [{ type: 'text', text: JSON.stringify(payload) }], structuredContent: payload, isError: r.timedOut };
    }
    return { content: [{ type: 'text', text: 'Unknown tool' }], isError: true };
  }

  async function syntheticDecision(proposeResult) {
    const started = Date.now();
    if (!scope) {
      const prepared = await core.call('gil_monitor_prepare', { project_root: o.root });
      log('harness', 'gil_monitor_prepare', { project_root: o.root }, prepared.result, prepared.error, Date.now() - started, true);
      scope = prepared.structured?.scope_id ?? null;
      if (!scope) return '[harness] synthetic App decision skipped: Monitor scope unavailable.';
    }
    const review = await core.call('gil_plan_review', { scope_id: scope });
    log('harness', 'gil_plan_review', { scope_id: scope }, review.result, review.error, Date.now() - started, true);
    const p = review.structured?.proposal;
    const token = review.meta?.['gil/planApprovalToken'];
    if (!p || p.receipt || !token) return `[harness] synthetic App decision skipped (${!p ? 'no proposal' : p.receipt ? 'receipt already recorded' : 'no capability'}).`;
    redactor.addSecret(token);
    const args = { scope_id: scope, revision: p.revision, digest: p.digest, token, decision: o.synthetic };
    const decided = await core.call('gil_plan_decide', args);
    log('harness', 'gil_plan_decide', args, decided.result, decided.error, Date.now() - started, true);
    void proposeResult;
    return decided.ok
      ? `[harness] Synthetic App decision recorded for revision ${p.revision}: ${o.synthetic}. This is test-only harness input, not user consent (시험 전용 합성 입력이며 사용자 동의가 아니다). Call gil_plan_read for the recorded receipt and close_report before closing Synthesis.`
      : `[harness] Synthetic App decision was refused by the Core: ${decided.structured?.said || decided.error?.message || 'unknown'}.`;
  }

  for await (const line of createInterface({ input: process.stdin })) {
    if (!line.trim()) continue;
    let req;
    try { req = JSON.parse(line); } catch { out({ jsonrpc: '2.0', id: null, error: { code: -32700, message: 'Parse error' } }); continue; }
    if (req.id === undefined) { try { core.notify(req.method, req.params); } catch {} continue; }
    const started = Date.now();
    try {
      let result;
      if (req.method === 'tools/call') {
        const name = req.params?.name;
        const args = req.params?.arguments ?? {};
        if (extraNames.has(name)) {
          result = await local(name, args);
          log('model', name, args, result, null, Date.now() - started);
        } else if (o.control) {
          log('model', name, args, null, { code: -32602, message: 'control arm' }, Date.now() - started);
          throw Object.assign(new Error('GIL tools are not connected in the control arm of this evaluation'), { code: -32602 });
        } else if (APP_ONLY.has(name)) {
          throw Object.assign(new Error('App-only approval tools are not available to the model in this evaluation'), { code: -32602 });
        } else if (typeof args.project_root === 'string' && resolve(args.project_root) !== o.root) {
          throw Object.assign(new Error('Evaluation only permits the selected temporary project'), { code: -32602 });
        } else {
          const r = await core.call(name, args);
          if (name === 'gil_start' && r.ok) { const m = r.result.content?.map((c) => c.text || '').join('\n').match(/project:[0-9a-f]{64}/); if (m) scope = m[0]; }
          if (name === 'gil_monitor_prepare' && r.ok && r.structured?.scope_id) scope = r.structured.scope_id;
          log('model', name, args, r.result, r.error, Date.now() - started);
          if (r.error) throw Object.assign(new Error(r.error.message || 'error'), { code: r.error.code ?? -32000, data: r.error.data });
          result = r.result;
          if (name === 'gil_plan_propose' && r.ok && o.synthetic) {
            const text = await syntheticDecision(result);
            result = { ...result, content: [...(result.content || []), { type: 'text', text }] };
          }
        }
      } else if (o.control) {
        if (req.method === 'initialize') result = { protocolVersion: req.params?.protocolVersion || '2025-06-18', capabilities: { tools: {} }, serverInfo: { name: 'gil-scenario-control', version: '0' }, instructions: 'Control arm: scenario tools only.' };
        else if (req.method === 'tools/list') result = { tools: extra };
        else throw Object.assign(new Error('Method not found'), { code: -32601 });
      } else {
        const frame = await core.request(req.method, req.params ?? {});
        if (frame.error) throw Object.assign(new Error(frame.error.message || 'error'), { code: frame.error.code ?? -32000, data: frame.error.data });
        result = frame.result;
        if (req.method === 'tools/list') result = { ...result, tools: [...(result.tools || []).filter((t) => !APP_ONLY.has(t.name)), ...extra] };
      }
      out({ jsonrpc: '2.0', id: req.id, result });
    } catch (err) {
      out({ jsonrpc: '2.0', id: req.id, error: { code: err.code ?? -32000, message: err.message } });
    }
  }
  if (core) await core.close();
}

main().catch((err) => { process.stderr.write(`[proxy] fatal: ${err.message}\n`); process.exit(2); });
