// Same App-only consent on every Host. A tool receipt is not a user click.
export function makeRecoveryReview({app, scope, panel, document, changed = () => {}}) {
  let stopped = false, busy = false, reading = false, value = null, token = null;
  const text = (tag, content) => { const e = document.createElement(tag); e.textContent = content; return e; };
  function message(content) { panel.hidden = false; panel.replaceChildren(text('h2', '실패한 시도에서 되돌아가기'), text('p', content)); }
  function render() {
    if (value?.state === 'not_needed') { panel.hidden = true; panel.replaceChildren(); return; }
    message(value?.notice || value?.reason || '복원 상태를 확인합니다.');
    if (!value?.supported) return;
    panel.append(text('p', `${value.from} → ${value.target}`));
    const list = document.createElement('ul');
    for (const change of value.changes || []) list.append(text('li', `${({preserve:'보존 후 작업 폴더에서 제외',replace:'원본 보존 후 교체',create:'복원 생성'})[change.action] || change.action}: ${change.path}`));
    panel.append(list, text('p', `보존 위치: ${value.archive}`));
    if (value.state === 'completed') { panel.append(text('p', '복원을 마쳤습니다. 실패 기록과 증거는 남아 있습니다. 채팅에서 계속 진행해 달라고 요청하세요.')); return; }
    const button = text('button', value.state === 'resumable' ? '승인한 복원 재개' : '보존 후 되돌아가기');
    button.type = 'button'; button.disabled = busy || !token;
    button.addEventListener('click', async event => {
      if (!event.isTrusted || stopped || busy || !token) return;
      busy = true; button.disabled = true;
      const request = { scope_id: scope, digest: value.digest, token };
      token = null;
      try {
        const result = await app.callServerTool({name:'gil_revisit_apply', arguments:request}, {timeout:120000});
        if (stopped) return;
        if (result.isError || !result.structuredContent) throw new Error(result.structuredContent?.problem || result.structuredContent?.said || '복원을 확인하지 못했습니다.');
        value = result.structuredContent; render(); changed();
      } catch (e) {
        if (!stopped) message(`결과를 확인하지 못했습니다. 자동으로 다시 실행하지 않습니다. 새로 확인해 주세요. ${e.message || ''}`);
      } finally { busy = false; }
    });
    panel.append(button, text('p', '승인 전에는 작업 파일을 바꾸지 않습니다. 보호 대상이 달라지거나 계획이 오래되면 거절합니다.'));
  }
  return {
    async refresh() {
      if (stopped || busy || reading) return;
      reading = true;
      try {
        const result = await app.callServerTool({name:'gil_revisit_review', arguments:{scope_id:scope}}, {timeout:10000});
        if (stopped) return;
        if (result.isError || !result.structuredContent) throw new Error(result.structuredContent?.problem || result.structuredContent?.said || '변경 목록을 읽지 못했습니다.');
        value = result.structuredContent; token = result._meta?.['gil/revisitApprovalToken'] || null; render();
      } catch (e) { if (!stopped) { token = null; message(e.message || '변경 목록을 읽지 못했습니다.'); } }
      finally { reading = false; }
    },
    stop() { stopped = true; token = null; panel.querySelectorAll('button').forEach(b => { b.disabled = true; }); },
  };
}
