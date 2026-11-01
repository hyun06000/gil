// Review-only negative fixtures. A successful process exit does NOT mean the product passed.
import { readFileSync } from 'node:fs';
import { planItems } from '../../../mcp-app/plans.mjs';
import { parseEvents, replay } from '../../../tools/scenarios/lib/trace.mjs';
import { scoreProcedure } from '../../../tools/scenarios/lib/score.mjs';

const closedCycleView = {
  supported: true, proposal: null, active_revision: 1,
  current_item: { id: 'A', question: 'A', done_when: 'evidence' },
  executed_cycles: [{ cycle_ref: 'cycle:C2', item_id: 'A', revision: 1 }],
  pending: [{ id: 'B', question: 'B', done_when: 'evidence' }],
};
// regressions.rs proves that Core keeps this binding even after C2 is closed.
const item = planItems(closedCycleView).find(i => i.id === 'A');
console.log(JSON.stringify({ finding: 'C-closed-cycle', expected: 'must not assert an open Cycle from this binding alone', actual: item.status }));

const events = parseEvents(readFileSync(new URL('../../../tools/scenarios/fixtures/minimal-events.jsonl', import.meta.url), 'utf8'));
// Deliberately invalid negative trace: a second Chain executes without its own approval.
// This is a scorer fixture, NOT an observed Core authorization bypass.
events.push({ seq: 12, actor: 'model', request: { name: 'gil_open', arguments: { kind: 'chain' } },
  result: { structuredContent: { ok: true, said: '열었다: chain:H2 · Opening Interview cycle:C3\n' } } });
events.push({ seq: 13, actor: 'model', request: { name: 'gil_open', arguments: { kind: 'experiment' } },
  result: { structuredContent: { ok: true, said: '열었다: cycle:C4 · experiment\n' } } });
const verdict = scoreProcedure(replay(events)).find(x => x.id === 'approval_before_execution');
console.log(JSON.stringify({ finding: 'E-cross-chain-approval-score', expected: 'fail', actual: verdict }));
