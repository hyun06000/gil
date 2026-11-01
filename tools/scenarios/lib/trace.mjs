// Replay of a tool-call event log into observable GIL facts. Nothing here guesses what a model
// thought or did outside the log; absent observations stay absent so the scorer can say unknown.
import { parseReport, evidenceFiles } from './report.mjs';

/**
 * Event line shape (JSONL). `actor` defaults to "model"; the harness/App write "harness"/"app".
 * { seq, at, actor, request:{name, arguments}, result, error, ms, synthetic }
 */
export function parseEvents(text) {
  return text.split('\n').map((l) => l.trim()).filter(Boolean).map((line, i) => {
    const e = JSON.parse(line);
    return { seq: e.seq ?? i + 1, actor: e.actor ?? 'model', synthetic: e.synthetic === true, request: e.request ?? {}, result: e.result ?? null, error: e.error ?? null, at: e.at ?? null };
  });
}

export function okOf(event) {
  if (event.error) return false;
  const r = event.result;
  if (!r) return false;
  if (r.isError === true) return false;
  const s = r.structuredContent;
  if (s && s.ok === false) return false;
  return true;
}

export function saidOf(event) {
  return event.result?.structuredContent?.said || event.result?.content?.map((c) => c.text || '').join('\n') || '';
}

const OPEN_STEP = /^열었다: (step:C(\d+)\/S(\d+)) · (\w+)/m;
const OPEN_CYCLE = /^열었다: (cycle:C(\d+)) · (\w+)/m;
const OPEN_CHAIN = /^열었다: (chain:\S+) · Opening Interview (cycle:C(\d+))/m;
const CLOSE_STEP = /^닫았다: (step:C(\d+)\/S(\d+)) · (\w+)/m;
const CLOSE_CYCLE = /^닫았다: (cycle:C(\d+)) · (\w+) · (\w+)/m;
const CLOSE_CHAIN = /^닫았다: (chain:\S+) · (\w+)/m;

/** Walk the events and collect facts. `externalTools` are non-GIL tools whose calls are real work. */
export function replay(events, { externalTools = [] } = {}) {
  const external = new Set(externalTools);
  const st = { chainIndex: 1, cycleKind: 'interview', cycleRef: 'cycle:C1', stepKind: null, stepRef: null, interviewClosedInChain: false, experiments: 0, awaitingChainReport: false };
  const facts = {
    starts: [], shows: [], monitorStatus: [], externalCalls: [], steps: [], cycles: [], chains: [],
    proposals: new Map(), proposalOrder: [], decides: [], synthesisCloses: [], verifyCloses: [], experimentOpens: [],
    reinterviewRequests: [], planReads: [], restoreRevisit: [], rejected: [], unknownTools: new Set(), total: events.length,
  };
  const snapshot = () => ({ ...st });
  const notePlan = (seq, actor, value, kind) => {
    // Only a plan view counts as a read; a refusal's {ok:false, problem} is not one.
    if (!value || typeof value !== 'object' || !('supported' in value)) return;
    facts.planReads.push({ seq, actor, kind, supported: value.supported, active_revision: value.active_revision ?? null, pending: (value.pending || []).map((i) => i.id), current_item: value.current_item?.id ?? null, reinterview: (value.reinterview_requests || []).map((r) => r.reason), executed: (value.executed_cycles || []).map((e) => `${e.cycle_ref}:${e.item_id}@${e.revision}`), proposal: value.proposal ? { revision: value.proposal.revision, digest: value.proposal.digest, synthesis_ref: value.proposal.synthesis_ref, receipt: value.proposal.receipt ?? null } : null, phase: snapshot() });
    const p = value.proposal;
    if (p && typeof p.revision === 'number') {
      const key = `${p.synthesis_ref}#${p.revision}`;
      // A proposal belongs to the Chain it was first observed in; a later Chain never inherits it.
      if (!facts.proposals.has(key)) { facts.proposals.set(key, { revision: p.revision, digest: p.digest, synthesis_ref: p.synthesis_ref, receipt: null, firstSeen: seq, chainIndex: st.chainIndex, base_revision: p.base_revision ?? null }); facts.proposalOrder.push(key); }
      // Keep the FIRST observation of a receipt; later reads must not move it forward in time.
      if (p.receipt && !facts.proposals.get(key).receipt) facts.proposals.get(key).receipt = { ...p.receipt, seenAt: seq, chainIndex: st.chainIndex };
    }
  };
  for (const e of events) {
    const name = e.request?.name;
    const args = e.request?.arguments || {};
    const ok = okOf(e);
    const said = saidOf(e);
    if (!ok) facts.rejected.push({ seq: e.seq, name, phase: snapshot(), error: e.error ? 'jsonrpc' : 'refused' });
    if (external.has(name)) {
      facts.externalCalls.push({ seq: e.seq, name, ok, actor: e.actor, item: args.item_id ?? args.item ?? null, path: args.path ?? null, phase: snapshot() });
      continue;
    }
    switch (name) {
      case 'gil_start': facts.starts.push({ seq: e.seq, ok, display_confirmed: e.result?.structuredContent?.display_confirmed ?? null }); break;
      case 'show_gil_monitor': facts.shows.push({ seq: e.seq, ok }); break;
      case 'gil_monitor_status': facts.monitorStatus.push({ seq: e.seq, ok, state: e.result?.structuredContent?.state ?? null }); break;
      case 'gil_restore': case 'gil_revisit': facts.restoreRevisit.push({ seq: e.seq, name, ok, said: said.slice(0, 300) }); break;
      case 'gil_plan_propose': case 'gil_plan_read': notePlan(e.seq, e.actor, e.result?.structuredContent, name); break;
      case 'gil_plan_review': notePlan(e.seq, e.actor, e.result?.structuredContent, name); break;
      case 'gil_plan_decide': {
        // Revisions restart at 1 in every Chain, so a decision is identified by the Synthesis it answers.
        // A decide without any response (lost reply, timeout) is neither ok nor a refusal; it leaves no receipt.
        facts.decides.push({ seq: e.seq, ok, answered: !!(e.result || e.error), actor: e.actor, synthetic: e.synthetic || e.actor !== 'app', decision: args.decision, revision: args.revision, synthesis_ref: ok ? e.result?.structuredContent?.proposal?.synthesis_ref ?? null : null, scope_given: typeof args.scope_id === 'string', chainIndex: st.chainIndex, phase: snapshot() });
        if (ok) notePlan(e.seq, e.actor, e.result?.structuredContent, name);
        break;
      }
      case 'gil_plan_request_interview': facts.reinterviewRequests.push({ seq: e.seq, ok, reason: args.reason, phase: snapshot() }); if (ok) notePlan(e.seq, e.actor, e.result?.structuredContent, name); break;
      case 'gil_open': case 'gil_close': case 'gil_cycle': break;
      default: if (name && !name.startsWith('gil_') && name !== 'show_gil_monitor') facts.unknownTools.add(name);
    }
    if (!ok) continue;
    if (name === 'gil_open') {
      let m;
      if ((m = said.match(OPEN_STEP))) {
        st.stepRef = m[1]; st.stepKind = m[4];
        facts.steps.push({ seq: e.seq, action: 'open', ref: m[1], kind: m[4], cycleKind: st.cycleKind, contract: args.contract ?? null });
      } else if ((m = said.match(OPEN_CHAIN))) {
        st.chainIndex += 1; st.cycleRef = m[2]; st.cycleKind = 'interview'; st.stepKind = null; st.stepRef = null; st.interviewClosedInChain = false; st.awaitingChainReport = false;
        facts.chains.push({ seq: e.seq, action: 'open', ref: m[1], firstCycle: m[2], chainIndex: st.chainIndex });
      } else if ((m = said.match(OPEN_CYCLE))) {
        st.cycleRef = m[1]; st.cycleKind = m[3]; st.stepKind = null; st.stepRef = null;
        if (m[3] === 'experiment') { st.experiments += 1; facts.experimentOpens.push({ seq: e.seq, ref: m[1], interviewClosedInChain: st.interviewClosedInChain, phase: snapshot() }); }
        facts.cycles.push({ seq: e.seq, action: 'open', ref: m[1], kind: m[3], chainIndex: st.chainIndex });
      }
    } else if (name === 'gil_close') {
      const fields = parseReport(args.report);
      let m;
      const sc = e.result?.structuredContent;
      // A typed (visual) close answers {confirmed:true, visuals.owner} without a "닫았다" line.
      if (!said.match(CLOSE_STEP) && sc?.confirmed === true && sc?.visuals?.owner?.kind === 'step') {
        m = [null, sc.visuals.owner.ref, null, null, st.stepKind || 'verify'];
      }
      if (m || (m = said.match(CLOSE_STEP))) {
        const kind = m[4];
        facts.steps.push({ seq: e.seq, action: 'close', ref: m[1], kind, cycleKind: st.cycleKind, fields });
        if (kind === 'synthesis') facts.synthesisCloses.push({ seq: e.seq, ref: m[1], cycleRef: st.cycleRef, chainIndex: st.chainIndex, approved: fields.approved ?? null, plan_revision: fields.plan_revision ? Number(fields.plan_revision) : null, plan_digest: fields.plan_digest ?? null, chain_verdict: fields.chain_verdict ?? null });
        if (kind === 'verify') facts.verifyCloses.push({ seq: e.seq, ref: m[1], evidence: evidenceFiles(fields), plan_document: fields.plan_document ?? null, cycleRef: st.cycleRef });
        st.stepKind = null; st.stepRef = null;
      } else if ((m = said.match(CLOSE_CYCLE))) {
        facts.cycles.push({ seq: e.seq, action: 'close', ref: m[1], kind: m[3], verdict: m[4], next: fields['next_direction.action'] ?? null, chainIndex: st.chainIndex });
        if (m[3] === 'interview') st.interviewClosedInChain = true;
        if (fields['next_direction.action'] === 'close_chain') st.awaitingChainReport = true;
        st.stepKind = null; st.stepRef = null;
      } else if ((m = said.match(CLOSE_CHAIN))) {
        facts.chains.push({ seq: e.seq, action: 'close', ref: m[1], verdict: m[2], chainIndex: st.chainIndex });
        st.awaitingChainReport = false;
      }
    }
  }
  facts.final = snapshot();
  facts.unknownTools = [...facts.unknownTools];
  return facts;
}
