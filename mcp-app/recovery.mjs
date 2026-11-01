// Same App-only consent on every Host. A tool receipt is not a user click.
export function makeRecoveryReview({app, scope, panel, document, changed = () => {}}) {
  let stopped = false, busy = false, reading = false, value = null, token = null, delivery = null;
  const text = (tag, content) => { const e = document.createElement(tag); e.textContent = content; return e; };
  function message(content) { panel.hidden = false; panel.replaceChildren(text('h2', '실패한 시도에서 되돌아가기'), text('p', content)); }
  // Only a trusted click sends a continuation. Polling never starts a model turn.
  // A message is a hint to re-read Core state, not a substitute for its receipt.
  async function notify(completed) {
    try {
      const reply = await app.sendMessage({role:'user', content:[{type:'text', text:
        'GIL Monitor에서 승인한 복원이 완료됐습니다. 기존에 선택한 같은 project_root로 gil_context를 한 번 읽고 현재 기록의 다음 호출을 따르세요. 과거의 승인 대기 안내를 반복하거나 버튼을 다시 누르라고 요구하지 마세요. 실패 기록·증거·배움을 유지하고, 이미 다음 Cycle이 열렸다면 그 자리에서 이어가세요.'}]}, {timeout:10000});
      if (reply?.isError) throw new Error('message refused');
      delivery = {digest:completed.digest, state:'delivered'};
    } catch { delivery = {digest:completed.digest, state:'undelivered'}; }
  }
  function completedNotice() {
    panel.append(text('p', '복원을 마쳤습니다. 실패 기록과 증거는 남아 있습니다.'));
    const delivered = delivery?.digest === value.digest && delivery?.state === 'delivered';
    panel.append(text('p', delivered
      ? '채팅에 계속 요청을 전달했습니다. 이어지지 않으면 채팅에서 현재 기록을 다시 읽어 달라고 요청하세요.'
      : '채팅 전달이 확인되지 않았습니다. 아래 버튼으로 계속 요청을 보내거나, 채팅에서 “gil_context로 현재 상태를 다시 읽고 이어가”라고 요청하세요. 복원 버튼을 다시 누를 필요는 없습니다.'));
    if (delivered) return;
    const completed = value, button = text('button', '채팅에 계속 요청 보내기');
    button.type = 'button'; button.disabled = busy;
    button.addEventListener('click', async event => {
      if (!event.isTrusted || stopped || busy || value?.state !== 'completed' || value.digest !== completed.digest ||
          (delivery?.digest === completed.digest && delivery?.state === 'delivered')) return;
      busy = true; button.disabled = true;
      try { await notify(completed); } finally { busy = false; }
      if (!stopped) render();
    });
    panel.append(button);
  }
  function render() {
    if (value?.state === 'not_needed') { panel.hidden = true; panel.replaceChildren(); return; }
    message(value?.notice || value?.reason || '복원 상태를 확인합니다.');
    if (!value?.supported) return;
    panel.append(text('p', `${value.from} → ${value.target}`));
    const list = document.createElement('ul');
    for (const change of value.changes || []) list.append(text('li', `${({preserve:'보존 후 작업 폴더에서 제외',replace:'원본 보존 후 교체',create:'복원 생성'})[change.action] || change.action}: ${change.path}`));
    panel.append(list, text('p', `보존 위치: ${value.archive}`));
    if (value.state === 'completed') { completedNotice(); return; }
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
        if (value.state === 'completed') await notify(value);
      } catch (e) {
        if (!stopped) message(`결과를 확인하지 못했습니다. 자동으로 다시 실행하지 않습니다. 새로 확인해 주세요. ${e.message || ''}`);
      } finally { busy = false; if (!stopped && value?.state === 'completed') render(); }
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
