// JS-only stand-in for `gil mcp --serve`, used by the harness self-tests. It imitates the wire
// shapes the harness parses (receipt lines, plan view, App _meta capability). It is NOT the
// product and proves nothing about the real Core; the real Core is exercised by wire-runner.mjs.
import { createInterface } from 'node:readline';
import { spawn } from 'node:child_process';
import { randomBytes } from 'node:crypto';

const scope = `project:${'a'.repeat(64)}`;
let step = 0, revision = 0, open = null;
const proposals = [];
let token = null;
const view = () => { const p = proposals.at(-1) ?? null; return { supported: true, proposal: p, active_revision: null, current_item: null, pending: [], reinterview_requests: [], executed_cycles: [], close_report: p?.receipt ? `statement: x\nbasis_refs: y\nexploration_checklist: |\n  ${p.draft.items.map((i) => i.id).join('\n  ')}\napproved: ${p.receipt.decision === 'approve' ? 'yes' : 'no'}\nplan_revision: ${p.revision}\nplan_digest: ${p.digest}\n` : null, document: null }; };
const okText = (said, extra = {}) => ({ content: [{ type: 'text', text: said }], structuredContent: { ok: true, exit_code: 0, said, problem: '', ...extra }, isError: false });
const refusal = (why) => ({ content: [{ type: 'text', text: why }], structuredContent: { ok: false, exit_code: 1, said: '', problem: why }, isError: true });
const tools = [
  ['gil_start', ['model']], ['gil_open', ['model']], ['gil_close', ['model']], ['gil_status', ['model']], ['gil_monitor_prepare', ['model']], ['show_gil_monitor', ['model']],
  ['gil_plan_propose', ['model']], ['gil_plan_read', ['model']], ['gil_plan_review', ['app']], ['gil_plan_decide', ['app']],
].map(([name, visibility]) => ({ name, description: name, inputSchema: { type: 'object', properties: {}, additionalProperties: true }, _meta: { ui: { visibility } } }));
const out = (f) => process.stdout.write(JSON.stringify(f) + '\n');

createInterface({ input: process.stdin }).on('line', (line) => {
  if (!line.trim()) return;
  const req = JSON.parse(line);
  if (req.id === undefined) return;
  const reply = (result) => out({ jsonrpc: '2.0', id: req.id, result });
  const { method, params } = req;
  if (method === 'initialize') return reply({ protocolVersion: '2025-06-18', capabilities: { tools: {} }, serverInfo: { name: 'fake-core', version: '0' }, instructions: 'fake' });
  if (method === 'tools/list') return reply({ tools });
  if (method === 'hang') return; // never answers: timeout test
  if (method === 'noise') { process.stdout.write('not json at all\n'); return reply({ noisy: true }); }
  if (method === 'spawn_grandchild') { const kid = spawn(process.execPath, ['-e', 'setInterval(() => {}, 1000)'], { stdio: 'ignore' }); return reply({ pid: kid.pid }); }
  if (method !== 'tools/call') return out({ jsonrpc: '2.0', id: req.id, error: { code: -32601, message: 'Method not found' } });
  const name = params.name, a = params.arguments || {};
  switch (name) {
    case 'gil_start': return reply(okText(`GIL 시작은 성공했다. 지금 다음 도구를 먼저 호출한다: show_gil_monitor(${JSON.stringify({ scope_id: scope })}).`, { storage_format: 7, display_confirmed: false }));
    case 'gil_monitor_prepare': return reply({ content: [{ type: 'text', text: 'prepared' }], structuredContent: { scope_id: scope, label: 'fake' }, isError: false });
    case 'show_gil_monitor': return reply({ content: [{ type: 'text', text: '표시를 요청했다. 화면 표시 여부는 판정하지 않는다.' }], structuredContent: { scope_id: scope }, isError: false });
    case 'gil_status': return reply(okText('status'));
    case 'gil_open': { if (open) return reply(refusal('지금은 열 자리가 아니다.')); step += 1; open = { ref: `step:C1/S${step}`, kind: a.kind }; return reply(okText(`열었다: ${open.ref} · ${a.kind}\n`)); }
    case 'gil_close': {
      if (!open) return reply(refusal('닫을 것이 없다.'));
      if (open.kind === 'synthesis') { const p = proposals.at(-1); if (!p?.receipt) return reply(refusal('승인 계획: gil_plan_propose로 제안한 뒤 Monitor 버튼 응답을 기다린다')); if (!(a.report || '').includes(`plan_revision: ${p.revision}`)) return reply(refusal('승인 계획: 승인한 내용 그대로 닫는다')); }
      const said = `닫았다: ${open.ref} · ${open.kind}\n기록됨: fake\n`; open = null; return reply(okText(said));
    }
    case 'gil_plan_propose': {
      if (open?.kind !== 'synthesis') return reply(refusal('승인 계획: 열린 Interview Synthesis에서만 제안한다'));
      const last = proposals.at(-1);
      if (last && JSON.stringify(last.draft) === JSON.stringify(a.draft) && last.synthesis_ref === open.ref) return reply({ content: [{ type: 'text', text: 'same' }], structuredContent: view(), isError: false });
      revision += 1; token = null;
      proposals.push({ revision, base_revision: null, synthesis_ref: open.ref, draft: a.draft, digest: randomBytes(32).toString('hex'), receipt: null });
      return reply({ content: [{ type: 'text', text: 'proposed' }], structuredContent: view(), isError: false });
    }
    case 'gil_plan_read': return reply({ content: [{ type: 'text', text: 'view' }], structuredContent: view(), isError: false });
    case 'gil_plan_review': {
      if (a.scope_id !== scope) return reply(refusal('unknown scope'));
      const p = proposals.at(-1); const r = { content: [{ type: 'text', text: 'review' }], structuredContent: view(), isError: false };
      if (p && !p.receipt) { token = token || randomBytes(32).toString('hex'); r._meta = { 'gil/planApprovalToken': token }; }
      return reply(r);
    }
    case 'gil_plan_decide': {
      const p = proposals.at(-1);
      if (!p || a.scope_id !== scope || a.revision !== p.revision || a.digest !== p.digest || !token || a.token !== token) return reply(refusal('승인 화면 연결이 오래됐거나 일치하지 않는다'));
      if (p.receipt && p.receipt.decision !== a.decision) return reply(refusal('이미 기록한 응답을 바꾸지 않는다'));
      p.receipt = { decision: a.decision, source: 'monitor_button' }; token = null;
      return reply({ content: [{ type: 'text', text: 'decided' }], structuredContent: view(), isError: false });
    }
    default: return out({ jsonrpc: '2.0', id: req.id, error: { code: -32602, message: 'Unknown GIL tool' } });
  }
}).on('close', () => process.exit(0));
