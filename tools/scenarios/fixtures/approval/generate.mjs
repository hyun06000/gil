#!/usr/bin/env node
// Regenerates the committed approval fixtures from the builder in approval-link.test.mjs.
// File name: NN-<expected approval_before_execution status>-<case>.jsonl. Synthetic scorer input only.
import { writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { builder } from './builder.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const receipt = { decision: 'approve', source: 'monitor_button' };
const cases = {
  '01-pass-two-chains-own-approvals': (b) => { b.start(); const p1 = b.interview(1); b.experiment(2, { proposal: { ...p1, receipt } }); b.interview(3, { extra: { chain_verdict: 'success' } }); b.closeChain(1, 3); b.openChain(2, 4); const q = b.interview(4, { digest: 'f'.repeat(64) }); b.experiment(5, { proposal: { ...q, receipt }, item: 'X' }); },
  '02-fail-previous-chain-approval': (b) => { b.start(); const p1 = b.interview(1); b.experiment(2, { proposal: { ...p1, receipt } }); b.interview(3, { extra: { chain_verdict: 'success' } }); b.closeChain(1, 3); b.openChain(2, 4); b.experiment(5, { item: 'X' }); },
  '03-fail-approval-before-edit': (b) => { b.start(); b.interview(1, { editAfterDecision: true, closeRevision: 1 }); b.experiment(2); },
  '04-fail-request-changes-only': (b) => { b.start(); b.interview(1, { decision: 'request_changes' }); b.raw('gil_open', { project_root: '/p', kind: 'experiment' }, { structuredContent: { ok: true, said: '열었다: cycle:C2 · experiment\n' }, isError: false }); },
  '05-fail-lost-decide-reply': (b) => { b.start(); b.interview(1, { answered: false }); b.experiment(2); },
  '06-unknown-log-starts-after-approval': (b) => { b.raw('gil_plan_read', { project_root: '/p' }, { structuredContent: { supported: true, proposal: null, active_revision: 1, pending: [{ id: 'A' }], reinterview_requests: [], executed_cycles: [] }, isError: false }); b.experiment(2); },
  '07-unknown-no-read-inside-experiment': (b) => { b.start(); b.interview(1); b.experiment(2, { read: false }); },
  '08-fail-other-active-revision': (b) => { b.start(); b.interview(1); b.experiment(2, { active: 7 }); },
};
for (const [name, build] of Object.entries(cases)) {
  const b = builder(); build(b);
  writeFileSync(join(here, `${name}.jsonl`), b.events.map((e) => JSON.stringify(e)).join('\n') + '\n');
}
