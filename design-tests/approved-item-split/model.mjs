// Candidate R only. Pure design experiment, NOT GIL code or a storage/API proposal.
import { createHash } from 'node:crypto';

const copy = value => structuredClone(value);
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const need = (condition, code) => { if (!condition) throw new Error(code); };
const canonical = value => Array.isArray(value) ? value.map(canonical)
  : value && typeof value === 'object'
    ? Object.fromEntries(Object.keys(value).sort().map(k => [k, canonical(value[k])])) : value;
export const digest = value => createHash('sha256').update(JSON.stringify(canonical(value))).digest('hex');
export const item = id => ({ id, question: `Question ${id}`, done_when: `Observe all of ${id}` });
export const active = s => s.proposals.find(p => p.revision === s.active);
const latest = (s, id) => s.cycles.findLast(c => c.item === id);
const relations = p => p.draft.replacements;
const live = p => p.draft.items.filter(i => !relations(p).some(r => r.source === i.id));
export const pending = s => live(active(s)).filter(i => !latest(s, i.id) || latest(s, i.id).verdict === 'failure').map(i => i.id);
export const retryPending = s => pending(s).filter(id => latest(s, id));
export const currentItem = s => s.cycles.find(c => c.ref === s.current)?.item ?? null;

export function initial() {
  const draft = { scope: 'Synthetic Chain scope', items: [item('A'), item('B')], replacements: [], synthesis: 'Approve A then B' };
  const p = { revision: 1, base: null, at: 1, draft };
  p.digest = digest(p);
  p.receipt = 'approve';
  return { active: 1, proposals: [p], activated: [1], cycles: [], current: null,
    boundary: 'ready', interview: null, nextInterview: 2, recoveryFrom: null, journal: [] };
}

export function draftFrom(s, additions = [], changes = []) {
  const draft = copy(active(s).draft);
  draft.items.push(...additions.map(item));
  draft.replacements.push(...copy(changes));
  draft.synthesis = 'Review replacements, residual scope, evidence and downstream assumptions';
  return draft;
}

function leaves(id, rs, seen = new Set()) {
  need(!seen.has(id), 'relation-cycle');
  const r = rs.find(r => r.source === id);
  if (!r) return [id];
  return r.targets.flatMap(t => leaves(t, rs, new Set([...seen, id])));
}

function validateDraft(s, draft) {
  const base = active(s), ids = draft.items.map(i => i.id), rs = draft.replacements;
  need(new Set(ids).size === ids.length && ids.every(id => /^[A-Za-z0-9_-]{1,64}$/.test(id)), 'item-ids');
  need(draft.scope?.trim() && draft.synthesis?.trim() && draft.items.every(i => i.question?.trim() && i.done_when?.trim()), 'definition');
  for (const old of base.draft.items) need(same(old, draft.items.find(i => i.id === old.id)), 'old-definition');
  need(new Set(rs.map(r => r.source)).size === rs.length, 'duplicate-replacement');
  const usedTargets = new Set();
  for (const r of rs) {
    need(ids.includes(r.source) && r.targets.length > 0 && r.reason?.trim(), 'relation-reference');
    for (const target of r.targets) {
      need(ids.includes(target), 'relation-reference');
      need(!usedTargets.has(target), 'shared-target');
      usedTargets.add(target);
    }
  }
  for (const id of ids) leaves(id, rs); // Explicitly reject cycles, including self-links.
  for (const old of relations(base)) need(same(old, rs.find(r => r.source === old.source)), 'old-relation');
  const added = rs.filter(r => !relations(base).some(old => old.source === r.source));
  for (const r of added) {
    need(live(base).some(i => i.id === r.source), 'source-not-live');
    need(latest(s, r.source)?.verdict !== 'success', 'successful-source');
    need(r.targets.every(t => !base.draft.items.some(i => i.id === t)), 'target-not-fresh');
  }
  // Replacement revisions substitute in-place. No silent reordering of other obligations.
  if (added.length) {
    const expanded = live(base).flatMap(i => added.find(r => r.source === i.id)?.targets ?? [i.id]);
    const actual = live({ draft }).map(i => i.id).filter(id => expanded.includes(id));
    need(same(expanded, actual), 'replacement-order');
  }
  // Conservative serial barriers, derived from each relation's introducing revision.
  // No general dependency graph. Later reorderings cannot move B before A's replacement leaves.
  const introducing = [...s.proposals.filter(p => s.activated.includes(p.revision)), { base: s.active, draft }];
  const order = live({ draft }).map(i => i.id);
  for (const p of introducing) {
    const before = s.proposals.find(b => b.revision === p.base);
    if (!before) continue;
    for (const r of relations(p).filter(r => !relations(before).some(old => old.source === r.source))) {
      const priorOrder = live(before).map(i => i.id);
      const tail = priorOrder.slice(priorOrder.indexOf(r.source) + 1);
      for (const left of leaves(r.source, rs)) for (const right of tail.flatMap(id => leaves(id, rs))) {
        need(order.indexOf(left) < order.indexOf(right), 'downstream-order');
      }
    }
  }
}

// Events stand for already-valid GIL boundaries; Step grammar, files and authorization are external.
export function apply(input, event) {
  const s = copy(input);
  switch (event.type) {
    case 'interview':
      need(s.boundary === 'ready', 'legal-interview-boundary');
      s.interview = s.nextInterview++;
      s.current = null;
      s.boundary = 'interview';
      break;
    case 'propose': {
      need(s.boundary === 'interview', 'open-interview-required');
      need(event.base === s.active, 'stale-base');
      validateDraft(s, event.draft);
      const last = s.proposals.at(-1);
      if (last.at === s.interview && same(last.draft, event.draft)) break;
      const p = { revision: s.proposals.length + 1, base: s.active, at: s.interview, draft: copy(event.draft) };
      p.digest = digest(p);
      p.receipt = null;
      s.proposals.push(p);
      break;
    }
    case 'decide': {
      const p = s.proposals.at(-1);
      need(s.boundary === 'interview' && p.at === s.interview, 'open-proposal-required');
      need(p.revision === event.revision && p.digest === event.digest, 'stale-decision');
      need(['approve', 'request_changes'].includes(event.decision), 'decision');
      need(p.receipt === null || p.receipt === event.decision, 'decision-final');
      p.receipt = event.decision;
      break;
    }
    case 'activate': {
      // Abstracts closing matching Synthesis, Outcome and successful Interview, NOT the button.
      const p = s.proposals.at(-1);
      need(s.boundary === 'interview' && p.at === s.interview && p.revision === event.revision, 'open-proposal-required');
      need(p.receipt === 'approve', 'approval-required');
      validateDraft(s, p.draft);
      s.active = p.revision;
      s.activated.push(p.revision);
      s.interview = null;
      s.boundary = 'ready';
      break;
    }
    case 'open': {
      need(s.boundary === 'ready', 'execution-boundary');
      need(event.revision === s.active, 'stale-execution');
      need(pending(s)[0] === event.item, 'first-pending');
      const prior = latest(s, event.item);
      need(!prior || s.recoveryFrom === prior.ref, 'retry-lineage');
      const c = { ref: `cycle:${s.cycles.length + 1}`, revision: s.active, item: event.item, verdict: null, evidence: [] };
      s.cycles.push(c);
      s.current = c.ref;
      s.boundary = 'experiment';
      break;
    }
    case 'finish': {
      need(s.boundary === 'experiment', 'open-experiment-required');
      need(['success', 'failure'].includes(event.verdict), 'verdict');
      need(event.verdict !== 'success' || event.fullCoverage === true, 'partial-is-not-success');
      need(event.freshObservation?.trim(), 'fresh-observation-required');
      for (const ref of event.reuse ?? []) need(s.cycles.some(c => c.ref === ref && c.verdict !== null), 'evidence-reference');
      const c = s.cycles.at(-1);
      c.verdict = event.verdict;
      c.evidence = [{ observation: event.freshObservation, reused: copy(event.reuse ?? []) }];
      s.boundary = event.verdict === 'failure' ? 'failed' : 'ready';
      break;
    }
    case 'recovered':
      // Harness premise: external M4-R approval/preservation/revisit succeeded. No restore modeled.
      need(s.boundary === 'failed', 'closed-failure-required');
      s.recoveryFrom = s.current;
      s.current = null;
      s.boundary = 'ready';
      break;
    default: throw new Error('unknown-event');
  }
  s.journal.push(copy(event));
  return s;
}

export function restart(serialized) {
  const stored = JSON.parse(serialized);
  let replay = initial();
  for (const event of stored.journal) replay = apply(replay, event);
  need(same(stored, replay), 'invalid-restored-state');
  return replay;
}

export function summary(s) {
  return active(s).draft.items.map(i => ({ id: i.id,
    verdict: latest(s, i.id)?.verdict ?? null,
    replacedBy: relations(active(s)).find(r => r.source === i.id)?.targets ?? [],
    // Replacement resolution never changes the original item's verdict.
    unfinished: live(active(s)).some(x => x.id === i.id) && latest(s, i.id)?.verdict !== 'success' }));
}

export function immutableRecords(s) {
  return {
    proposals: s.proposals.map(({ receipt, ...p }) => p),
    receipts: s.proposals.filter(p => p.receipt !== null).map(p => [p.revision, p.receipt]),
    bindings: s.cycles.map(({ ref, revision, item }) => ({ ref, revision, item })),
    closed: s.cycles.filter(c => c.verdict !== null),
  };
}
