// Only this MCP App adapter writes approval metadata. The shared/native renderer stays read-only.
// A token remains in this closure, never GIL_HOST, DOM, host-evidence or model-visible content.
export function makePlanReview({ app, scope, panel, document, changed = () => {} }) {
  let current = null, token = null, busy = false, dead = false, rendered = null;
  const text = (tag, content) => { const n = document.createElement(tag); n.textContent = content; return n; };
  function clear() { panel.replaceChildren(); }
  function status(message) { clear(); panel.hidden = false; panel.append(text('p', message)); }
  function render(value) {
    const p = value.proposal;
    panel.hidden = !value.supported;
    if (panel.hidden) { clear(); return; }
    if (!p) {
      status(`승인 계획 ${value.active_revision ? `v${value.active_revision}` : '대기 중'} · 남은 순서: ${(value.pending || []).map(i => i.id).join(' → ') || '없음'}`);
      if (value.current_item) panel.append(text('p', `현재 실험 ${value.current_item.id}: ${value.current_item.question}\n확인 조건: ${value.current_item.done_when}`));
      for (const request of value.reinterview_requests || []) panel.append(text('p', `재인터뷰 대기: ${request.reason} — 현재 Cycle을 닫은 뒤 확인합니다.`));
      return;
    }
    clear(); panel.hidden = false;
    panel.append(text('h2', `계획 v${p.revision} — ${p.receipt ? '응답 기록됨' : '사용자 확인이 필요합니다'}`));
    const labels = { goal: '목표', in_scope: '포함 범위', out_of_scope: '제외 범위', constraints: '제약', success_criteria: '성공 조건' };
    const dl = document.createElement('dl');
    for (const [key, label] of Object.entries(labels)) { dl.append(text('dt', label), text('dd', p.draft.scope[key])); }
    panel.append(dl, text('h3', '실행할 항목과 확인 조건'));
    const list = document.createElement('ol');
    for (const i of p.draft.items) list.append(text('li', `${i.id}: ${i.question}${(value.executed_cycles || []).some(e => e.item_id === i.id) ? ' (이미 실행한 항목)' : ''}\n확인: ${i.done_when}`));
    panel.append(list, text('h3', '승인할 Synthesis 전체'), text('pre', JSON.stringify(p.draft.synthesis, null, 2)));
    const trace = document.createElement('details');
    trace.append(text('summary', '승인 기록 정보'), text('p', `기준 승인 판: ${p.base_revision ?? '없음'} · 제안: ${p.synthesis_ref} · SHA-256: ${p.digest}`));
    panel.append(trace);
    if (p.receipt) {
      panel.append(text('p', p.receipt.decision === 'approve'
        ? '승인을 기록했습니다. 에이전트가 이어서 진행하지 않으면 채팅에서 승인 결과를 확인해 달라고 요청하세요.'
        : '수정 요청을 기록했습니다. 에이전트가 다시 질문하고 새 제안을 준비합니다.'));
      return;
    }
    panel.append(text('p', '이 내용에 동의하는 경우 승인하세요. 내용을 바꾸려면 수정 요청 후 채팅으로 알려 주세요.'));
    for (const [decision, label] of [['approve', '이 계획 승인하고 계속'], ['request_changes', '수정 요청']]) {
      const button = text('button', label); button.type = 'button'; button.disabled = busy || !token;
      button.addEventListener('click', async (event) => {
        if (!event.isTrusted || busy || dead || !token || current?.digest !== p.digest) return;
        busy = true; for (const b of panel.querySelectorAll('button')) b.disabled = true;
        const request = { scope_id: scope, revision: p.revision, digest: p.digest, token, decision };
        try {
          const result = await app.callServerTool({ name: 'gil_plan_decide', arguments: request }, { timeout:10000 });
          if (dead) return;
          if (result.isError) throw new Error(result.structuredContent?.said || '승인 결과를 확인하지 못했습니다.');
          token = null; current = result.structuredContent?.proposal; rendered = null;
          render(result.structuredContent); changed();
          // This message is a consequence of this explicit button click, never of polling/rendering.
          // The persisted receipt remains authoritative; message failure must not repeat the write.
          try {
            const message = await app.sendMessage({ role:'user', content:[{type:'text', text:
              `GIL Monitor에서 계획 v${p.revision} (${p.synthesis_ref})에 ${decision === 'approve' ? '승인' : '수정 요청'}을 기록했습니다. gil_plan_read로 실제 기록을 확인하고 close_report로 Synthesis를 닫아 다음 단계를 진행해 주세요. 승인했다면 Outcome와 Interview Cycle도 닫은 뒤 Experiment로 진행하세요. 수정 요청이라면 다음 Question에서 변경 내용을 물어보세요.`}] }, {timeout:10000});
            if (message?.isError) throw new Error('message refused');
            if (!dead) panel.append(text('p', '채팅에 계속 요청을 전달했습니다.'));
          } catch {
            if (!dead) panel.append(text('p', '응답은 저장됐지만 채팅에 전달하지 못했습니다. 채팅에서 “GIL 승인 결과를 확인하고 계속해 줘”라고 입력해 주세요.'));
          }
        } catch (err) {
          if (!dead) { token = null; rendered = null; status(`${err.message} 다시 읽어서 기록 여부를 확인합니다.`); }
        } finally { busy = false; }
      });
      panel.append(button);
    }
  }
  return {
    async refresh() {
      if (dead || busy) return;
      busy = true;
      try {
        const result = await app.callServerTool({ name: 'gil_plan_review', arguments: { scope_id: scope } }, { timeout:10000 });
        if (dead) return;
        if (result.isError || !result.structuredContent) throw new Error('계획 승인 상태를 읽지 못했습니다. 연결되면 다시 확인합니다.');
        const value = result.structuredContent;
        current = value.proposal;
        token = result._meta?.['gil/planApprovalToken'] ?? null;
        const key = JSON.stringify(value);
        busy = false;
        // Preserve focus while polling. A renewed token does not change the visible proposal.
        if (key !== rendered) { render(value); rendered = key; }
      } catch (err) {
        if (!dead) { token = null; current = null; rendered = null; status(err.message); }
      } finally { busy = false; }
    },
    stop() { dead = true; token = null; current = null; clear(); panel.hidden = true; },
  };
}
