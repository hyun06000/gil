// Synthetic screen check for Wave 2 B. Renders the Monitor plan panel with the fixture DOM used by
// mcp-app/plans.test.mjs for the six screens named in B-MONITOR-STATE.md and writes B-screens.md.
// This is a fixture rendering, not a Codex/Claude Desktop acceptance. Run from the repository root:
//   node coordination/parallel-wave-2/results/B-screens.mjs
import { writeFileSync } from 'node:fs';
import { makePlanReview } from '../../../mcp-app/plans.mjs';

class Element {
  constructor(tag, doc) { this.tag = tag; this.doc = doc; this.children = []; this.listeners = {}; this.textContent = ''; this.dataset = {}; this.hidden = false; this.disabled = false; this.className = ''; }
  append(...n) { this.children.push(...n); }
  replaceChildren(...n) { this.children = n; }
  addEventListener(name, fn) { this.listeners[name] = fn; }
  querySelectorAll(tag) { return this.children.flatMap(n => [...(n.tag === tag ? [n] : []), ...n.querySelectorAll(tag)]); }
  contains(n) { return n === this || this.children.some(c => c.contains(n)); }
  lines(depth = 0) {
    const own = this.textContent ? [`${'  '.repeat(depth)}<${this.tag}${this.dataset.status ? ` status=${this.dataset.status}` : ''}${this.dataset.cycle ? ` cycle=${this.dataset.cycle}` : ''}${this.dataset.decision ? ` decision=${this.dataset.decision}` : ''}${this.disabled ? ' disabled' : ''}> ${this.textContent.replace(/\n/g, ' ⏎ ')}`] : [];
    return [...own, ...this.children.flatMap(c => c.lines(depth + 1))];
  }
}
const item = (id) => ({ id, question: `Question ${id}`, done_when: `File ${id}` });
const proposal = (extra) => ({ revision: 1, base_revision: null, digest: 'a'.repeat(64), synthesis_ref: 'step:C1/S3', receipt: null,
  draft: { scope: { goal: 'goal', in_scope: 'in', out_of_scope: 'out', constraints: 'none', success_criteria: 'proof' }, items: [item('A'), item('B')],
    synthesis: { statement: 'Proposed scope', basis_refs: 'step:C1/S1', exploration_checklist: 'A\nB' } }, ...extra });
const base = { supported: true, active_revision: 1, reinterview_requests: [], proposal: null };
const screens = [
  ['1 제안 없음 (idle)', { ...base, active_revision: null, pending: [], executed_cycles: [], current_item: null, current_cycle: { ref: 'cycle:C1', state: 'open' } }],
  ['2 실행 항목 있음: Experiment 열림', { ...base, pending: [item('B')], executed_cycles: [{ cycle_ref: 'cycle:C2', item_id: 'A', revision: 1 }], current_item: item('A'), current_cycle: { ref: 'cycle:C2', state: 'open' } }],
  ['3 Cycle 닫힘 (R3 fixture: current_item 바인딩 유지)', { ...base, pending: [item('B')], executed_cycles: [{ cycle_ref: 'cycle:C2', item_id: 'A', revision: 1 }], current_item: item('A'), current_cycle: { ref: 'cycle:C2', state: 'closed' } }],
  ['4 다음 Cycle 열림', { ...base, pending: [], executed_cycles: [{ cycle_ref: 'cycle:C2', item_id: 'A', revision: 1 }, { cycle_ref: 'cycle:C3', item_id: 'B', revision: 1 }], current_item: item('B'), current_cycle: { ref: 'cycle:C3', state: 'open' } }],
  ['5 재인터뷰: 닫힌 Experiment 뒤 새 판 제안', { ...base, pending: [item('B')], executed_cycles: [{ cycle_ref: 'cycle:C2', item_id: 'A', revision: 1 }], current_item: item('A'), current_cycle: { ref: 'cycle:C4', state: 'open' }, reinterview_requests: [{ cycle_ref: 'cycle:C2', reason: '범위가 바뀌었다' }], proposal: proposal({ revision: 2, base_revision: 1, synthesis_ref: 'step:C4/S3' }) }],
  ['6 Closing 이후: 마지막 Cycle 닫힘', { ...base, pending: [], executed_cycles: [{ cycle_ref: 'cycle:C2', item_id: 'A', revision: 1 }, { cycle_ref: 'cycle:C3', item_id: 'B', revision: 1 }], current_item: item('B'), current_cycle: { ref: 'cycle:C3', state: 'closed' } }],
  ['7 옛 Core: current_cycle 없음', { ...base, pending: [item('B')], executed_cycles: [{ cycle_ref: 'cycle:C2', item_id: 'A', revision: 1 }], current_item: item('A') }],
];
let out = '# B 합성 화면 검증 (fixture DOM)\n\n`node coordination/parallel-wave-2/results/B-screens.mjs`가 생성한다. mcp-app/plans.mjs를 시험용 DOM으로 렌더한 텍스트이며 실제 Host 화면 인수가 아니다.\n';
for (const [title, value] of screens) {
  const doc = { activeElement: null }; doc.createElement = t => new Element(t, doc);
  const panel = new Element('section', doc), banner = new Element('section', doc);
  const app = { async sendMessage() { return {}; }, async callServerTool() { return { structuredContent: structuredClone(value), _meta: { 'gil/planApprovalToken': value.proposal && !value.proposal.receipt ? 'fixture-token' : undefined } }; } };
  await makePlanReview({ app, scope: 'project:fixture', panel, banner, document: doc }).refresh();
  out += `\n## ${title}\n\nbanner: ${banner.hidden ? '(숨김)' : banner.lines().join(' | ')}\npanel state=${panel.dataset.state}\n\n\`\`\`text\n${panel.lines().join('\n')}\n\`\`\`\n`;
}
writeFileSync(new URL('./B-screens.md', import.meta.url), out);
console.log(`wrote B-screens.md (${screens.length} screens)`);
