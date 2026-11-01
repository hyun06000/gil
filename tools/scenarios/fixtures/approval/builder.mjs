// Synthetic event builder for the approval-link fixtures. Receipts imitate the Core's wire text;
// nothing here is a product observation.
// Builder shared by the fixtures below; receipts imitate the Core wire text.
export function builder() {
  const events = [];
  let seq = 0;
  const ok = (said) => ({ structuredContent: { ok: true, said }, isError: false, content: [{ type: 'text', text: said }] });
  const push = (name, args, result, actor = 'model', synthetic = false) => { events.push({ seq: ++seq, actor, synthetic, request: { name, arguments: args }, result }); return seq; };
  const view = (proposal, extra = {}) => ({ supported: true, proposal, active_revision: null, pending: [], reinterview_requests: [], executed_cycles: [], ...extra });
  const b = {
    events,
    start() { push('gil_start', { project_root: '/p' }, ok('started show_gil_monitor({"scope_id":"project:aa"})')); push('show_gil_monitor', { scope_id: 'project:aa' }, ok('requested')); },
    interview(c, { revision = 1, digest = 'd'.repeat(64), decision = 'approve', answered = true, closeRevision = revision, editAfterDecision = false, propose = true, extra = {} } = {}) {
      push('gil_open', { project_root: '/p', kind: 'question', contract: 'c' }, ok(`열었다: step:C${c}/S1 · question\n`));
      push('gil_close', { project_root: '/p', report: 'question: q\nchoices: c\nresponse: r\n' }, ok(`닫았다: step:C${c}/S1 · question\n`));
      push('gil_open', { project_root: '/p', kind: 'synthesis', contract: 'c' }, ok(`열었다: step:C${c}/S3 · synthesis\n`));
      const ref = `step:C${c}/S3`;
      const p = { revision, synthesis_ref: ref, digest, receipt: null };
      if (propose) push('gil_plan_propose', { project_root: '/p', draft: {} }, { structuredContent: view(p, extra), isError: false });
      if (decision) {
        const receipt = { decision, source: 'monitor_button' };
        push('gil_plan_decide', { scope_id: 'project:aa', revision, digest, token: 'x'.repeat(64), decision }, answered ? { structuredContent: view({ ...p, receipt }), isError: false } : null, 'app');
        if (editAfterDecision) push('gil_plan_propose', { project_root: '/p', draft: {} }, { structuredContent: view({ ...p, revision: revision + 1, digest: 'e'.repeat(64) }), isError: false });
      }
      const approved = decision === 'approve' ? 'yes' : 'no';
      push('gil_close', { project_root: '/p', report: `statement: s\nbasis_refs: b\napproved: ${approved}\nplan_revision: ${closeRevision}\nplan_digest: ${digest}\n${extra.chain_verdict ? `chain_verdict: ${extra.chain_verdict}\n` : ''}` }, ok(`닫았다: ${ref} · synthesis\n`));
      if (approved === 'yes') {
        push('gil_open', { project_root: '/p', kind: 'outcome', contract: 'c' }, ok(`열었다: step:C${c}/S4 · outcome\n`));
        push('gil_close', { project_root: '/p', report: 'verdict: success\nlesson: l\n' }, ok(`닫았다: step:C${c}/S4 · outcome\n`));
        push('gil_close', { project_root: '/p', report: `verdict: success\noutcome_ref: step:C${c}/S4\nhandoff_summary: h\nnext_direction.action: ${extra.chain_verdict ? 'close_chain' : 'open_child'}\nnext_direction.reason: r\n` }, ok(`닫았다: cycle:C${c} · interview · success\n`));
      }
      return p;
    },
    experiment(c, { read = true, item = 'A', active = 1, proposal = null } = {}) {
      push('gil_open', { project_root: '/p', kind: 'experiment' }, ok(`열었다: cycle:C${c} · experiment\n`));
      if (read) push('gil_plan_read', { project_root: '/p' }, { structuredContent: view(proposal, { active_revision: active, current_item: item ? { id: item } : null }), isError: false });
      push('gil_open', { project_root: '/p', kind: 'verify', contract: 'c' }, ok(`열었다: step:C${c}/S3 · verify\n`));
      push('gil_close', { project_root: '/p', report: 'execution: e\nresult: r\nevidence_files: evidence/a.md\n' }, ok(`닫았다: step:C${c}/S3 · verify\n`));
      push('gil_close', { project_root: '/p', report: 'verdict: success\noutcome_ref: x\nhandoff_summary: h\nnext_direction.action: open_child\nnext_direction.reason: r\n' }, ok(`닫았다: cycle:C${c} · experiment · success\n`));
    },
    closeChain(n, c) { push('gil_close', { project_root: '/p', report: `verdict: success\nclosing_synthesis_ref: step:C${c}/S3\nhandoff_summary: h\n` }, ok(`닫았다: chain:C${n} · success\n`)); },
    openChain(n, c) { push('gil_open', { project_root: '/p', kind: 'chain' }, ok(`열었다: chain:C${n} · Opening Interview cycle:C${c}\n`)); },
    read(proposal, extra) { push('gil_plan_read', { project_root: '/p' }, { structuredContent: view(proposal, extra), isError: false }); },
    raw(name, args, result, actor = 'model') { push(name, args, result, actor); },
  };
  return b;
}
