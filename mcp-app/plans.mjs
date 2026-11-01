// Only this MCP App adapter writes approval metadata. The shared/native renderer stays read-only.
// A token remains in this closure, never GIL_HOST, DOM, host-evidence or model-visible content.
//
// The panel speaks only about observable facts: a recorded receipt, an open proposal, a bound
// item, a reply this screen did or did not get. It never claims that the agent is working.

export const PLAN_STATE = {
  idle: '승인 계획 없음',
  requested: '승인 요청',
  reapproval: '새 판 재승인 필요',
  no_token: '승인 버튼 연결 없음',
  approved: '승인됨',
  changes_requested: '수정 요청됨',
  executing: '승인된 계획',
  uncertain: '저장 결과 불확실',
  unreadable: '읽기 실패',
  unsupported: '',
};
const ITEM_STATUS = { executed: '실행함', current: '지금 열린 Cycle', pending: '남음' };

/** One state for the whole screen, derived from the wire value and this screen's own facts. */
export function planSummary(value, { hasToken = false, uncertain = null } = {}) {
  if (uncertain) return { state: 'uncertain', revision: uncertain.revision, decision: uncertain.decision };
  if (!value || value.code === 'unreadable') return { state: 'unreadable', said: value?.said ?? null };
  if (!value.supported) return { state: 'unsupported' };
  const p = value.proposal;
  if (!p) {
    return value.active_revision == null ? { state: 'idle' } : { state: 'executing', revision: value.active_revision };
  }
  const base = { revision: p.revision, base_revision: p.base_revision ?? null };
  if (p.receipt) return { ...base, state: p.receipt.decision === 'approve' ? 'approved' : 'changes_requested' };
  if (!hasToken) return { ...base, state: 'no_token' };
  return { ...base, state: p.base_revision != null || value.active_revision != null ? 'reapproval' : 'requested' };
}

/** Items with their recorded status. The agent's intent is not a status. */
export function planItems(value) {
  const executed = value?.executed_cycles || [];
  const current = value?.current_item?.id ?? null;
  const status = (id) => id === current ? 'current' : executed.some(e => e.item_id === id) ? 'executed' : 'pending';
  const cycle = (id) => executed.find(e => e.item_id === id)?.cycle_ref ?? null;
  const p = value?.proposal;
  if (p) return p.draft.items.map(i => ({ ...i, status: status(i.id), cycle_ref: cycle(i.id) }));
  const seen = new Set();
  const out = [];
  for (const e of executed) {
    if (seen.has(e.item_id)) continue;
    seen.add(e.item_id);
    const item = e.item_id === current ? value.current_item : null;
    out.push({ id: e.item_id, question: item?.question ?? null, done_when: item?.done_when ?? null,
      status: status(e.item_id), cycle_ref: e.cycle_ref });
  }
  for (const i of value?.pending || []) {
    if (seen.has(i.id)) continue;
    seen.add(i.id);
    out.push({ ...i, status: 'pending', cycle_ref: null });
  }
  return out;
}

/** What approving this Synthesis means, in one plain sentence. Unknown verdicts are quoted, not interpreted. */
export function verdictSentence(synthesis, itemCount) {
  const verdict = synthesis?.chain_verdict;
  if (verdict === undefined || verdict === null || verdict === '') {
    return `이 제안은 실행할 항목 ${itemCount}개를 담은 계획입니다. 전체 결과(Chain)를 끝내자는 제안은 아닙니다.`;
  }
  if (verdict === 'success') return '이 제안은 전체 결과(Chain)를 성공으로 마치자는 제안입니다. 승인하면 성공으로 끝내는 데 동의합니다.';
  if (verdict === 'failure') return '이 제안은 전체 결과(Chain)를 실패로 마치자는 제안입니다. 승인하면 실패로 끝내는 데 동의합니다.';
  return `이 제안의 chain_verdict 값은 "${verdict}" 입니다. 성공이나 실패로 해석하지 않습니다. 아래 원문을 확인하세요.`;
}

/** The one line at the top of the Monitor. Empty when nothing waits for the person. */
export function bannerLine(summary, delivery = null) {
  const v = summary.revision != null ? `계획 v${summary.revision}` : '계획';
  switch (summary.state) {
    case 'requested': return `승인 대기 · ${v} — 내용을 확인하고 승인 또는 수정 요청을 눌러 주세요.`;
    case 'reapproval': return `새 판 재승인 필요 · ${v} — ${summary.base_revision != null ? `이전 승인(v${summary.base_revision})은 ` : '이전 승인은 '}이 판에 적용되지 않습니다. 다시 확인해 주세요.`;
    case 'no_token': return `승인 대기 · ${v} — 승인 버튼 연결을 아직 받지 못했습니다. 자동으로 다시 읽습니다.`;
    case 'approved': return delivery === 'undelivered'
      ? `승인 기록됨 · ${v} — 채팅에 전달하지 못했습니다. 아래 안내대로 채팅에서 이어가 주세요.`
      : `승인 기록됨 · ${v} — 제안 단계가 아직 열려 있습니다. 에이전트가 이어가지 않으면 채팅에서 확인을 요청하세요.`;
    case 'changes_requested': return `수정 요청 기록됨 · ${v} — 바꿀 내용을 채팅으로 알려 주세요.`;
    case 'uncertain': return `저장 결과 불확실 · ${v} — 응답을 받지 못했습니다. 다시 읽어 기록 여부를 확인합니다. 응답을 다시 보내지 않습니다.`;
    case 'unreadable': return '승인 상태를 읽지 못했습니다 — 연결되면 자동으로 다시 확인합니다.';
    default: return '';
  }
}

const continuation = (p, decision) =>
  `GIL Monitor에서 계획 v${p.revision} (${p.synthesis_ref})에 ${decision === 'approve' ? '승인' : '수정 요청'}을 기록했습니다. gil_plan_read로 실제 기록을 확인하고 close_report로 Synthesis를 닫아 다음 단계를 진행해 주세요. 승인했다면 Outcome와 Interview Cycle도 닫은 뒤 Experiment로 진행하세요. 수정 요청이라면 다음 Question에서 변경 내용을 물어보세요.`;
export const MANUAL_CONTINUE = '채팅에서 “GIL 승인 결과를 확인하고 계속해 줘”라고 입력해 주세요.';

export function makePlanReview({ app, scope, panel, banner = null, document, changed = () => {} }) {
  let current = null, token = null, busy = false, dead = false, rendered = null, heading = null;
  let last = null, lastState = null; // last value read successfully; named as text when a later read fails
  let uncertain = null;   // a decide call whose reply never arrived: { revision, decision, said }
  let delivery = null;    // { digest, state: 'delivered' | 'undelivered' } for a decision made on this screen
  let shown = null;       // digest of the proposal last shown without a receipt
  let replaced = false;   // that proposal was replaced before anyone answered it
  const text = (tag, content, className) => {
    const n = document.createElement(tag);
    n.textContent = content;
    if (className) n.className = className;
    return n;
  };
  const button = (label, extra = {}) => {
    const b = text('button', label, extra.className);
    b.type = 'button';
    if (extra.decision) b.dataset.decision = extra.decision;
    if (extra.action) b.dataset.action = extra.action;
    return b;
  };
  const buttons = () => [...panel.querySelectorAll('button')];
  const key = (value) => JSON.stringify([value, !!token, delivery, uncertain]);
  function clear() { panel.replaceChildren(); heading = null; }

  // Focus survives a re-render: the same decision button, else the heading. Nothing scrolls here.
  function standing() {
    const active = document.activeElement;
    if (!active) return null;
    if (banner?.contains?.(active)) return { where: 'banner' };
    if (panel.contains?.(active)) return { where: 'panel', decision: active.dataset?.decision ?? null, action: active.dataset?.action ?? null };
    return null;
  }
  function restore(stood) {
    if (!stood) return;
    let target = null;
    if (stood.where === 'banner') target = banner && !banner.hidden ? [...banner.querySelectorAll('button')][0] : heading;
    else target = buttons().find(b => (stood.decision && b.dataset.decision === stood.decision) || (stood.action && b.dataset.action === stood.action)) || heading;
    target?.focus?.({ preventScroll: true });
  }
  // The only scroll this adapter ever performs, and only on the person's own click: the panel's
  // top comes into view, then the first decision button takes focus (the browser scrolls further
  // only if that button is still outside the viewport).
  function jump() {
    if (panel.hidden) return;
    panel.scrollIntoView?.({ block: 'start' });
    const target = buttons().find(b => b.dataset.decision) || heading;
    target?.focus?.();
  }
  function showBanner(summary) {
    if (!banner) return;
    const line = bannerLine(summary, delivery?.state ?? null);
    banner.replaceChildren();
    banner.hidden = !line;
    banner.dataset.state = summary.state;
    if (!line) return;
    banner.append(text('span', line, 'plan-banner-text'));
    if (summary.state !== 'unreadable') {
      const go = button('승인 화면으로', { action: 'jump', className: 'plan-jump' });
      go.addEventListener('click', jump);
      banner.append(go);
    }
  }
  function head(summary, title) {
    heading = text('h2', '');
    heading.tabIndex = -1;
    const badge = text('span', PLAN_STATE[summary.state], 'plan-state');
    badge.dataset.state = summary.state;
    heading.append(badge, text('span', title));
    panel.append(heading);
  }
  function itemList(value) {
    const list = document.createElement('ol');
    list.className = 'plan-items';
    for (const item of planItems(value)) {
      const where = item.status === 'executed' && item.cycle_ref ? ` (${item.cycle_ref})` : '';
      const li = text('li', `${ITEM_STATUS[item.status]}${where} · ${item.id}${item.question ? `: ${item.question}` : ''}${item.done_when ? `\n확인: ${item.done_when}` : ''}`);
      li.dataset.status = item.status;
      list.append(li);
    }
    return list;
  }
  function counts(value) {
    const items = planItems(value);
    const n = (status) => items.filter(i => i.status === status).length;
    return `실행함 ${n('executed')}개 · 지금 열린 Cycle ${n('current')}개 · 남음 ${n('pending')}개`;
  }
  function reinterview(value) {
    for (const request of value.reinterview_requests || []) {
      panel.append(text('p', `재인터뷰 요청 기록 (${request.cycle_ref}): ${request.reason} — 현재 Cycle을 닫은 뒤 다음 Interview에서 확인합니다.`, 'plan-reinterview'));
    }
  }
  // This message is a consequence of an explicit button click, never of polling/rendering.
  // The persisted receipt remains authoritative; message failure must not repeat the write.
  async function notify(p, decision) {
    try {
      const message = await app.sendMessage({ role: 'user', content: [{ type: 'text', text: continuation(p, decision) }] }, { timeout: 10000 });
      if (message?.isError) throw new Error('message refused');
      delivery = { digest: p.digest, state: 'delivered' };
    } catch {
      delivery = { digest: p.digest, state: 'undelivered' };
    }
  }
  function deliveryNote(p, decision) {
    const state = delivery?.digest === p.digest ? delivery.state : null;
    if (state === 'delivered') { panel.append(text('p', '채팅에 계속 요청을 전달했습니다.', 'plan-delivered')); return; }
    panel.append(text('p', state === 'undelivered'
      ? `응답은 저장됐지만 채팅에 전달하지 못했습니다. 저장된 응답은 다시 보내지 않습니다. ${MANUAL_CONTINUE}`
      : `이 화면에서 채팅 전달 여부를 확인하지 못했습니다. 에이전트가 이어가지 않으면 ${MANUAL_CONTINUE}`, 'plan-delivery'));
    const retry = button('채팅에 계속 요청 보내기', { action: 'notify' });
    retry.disabled = busy;
    retry.addEventListener('click', async (event) => {
      if (!event.isTrusted || busy || dead || current?.digest !== p.digest) return;
      busy = true; for (const b of buttons()) b.disabled = true;
      try { await notify(p, decision); } finally { busy = false; }
      if (!dead && last) { render(last); rendered = key(last); }
    });
    panel.append(retry);
  }
  function decisionButtons(p) {
    for (const [decision, label] of [['approve', '이 계획 승인하고 계속'], ['request_changes', '수정 요청']]) {
      const b = button(label, { decision });
      b.disabled = busy || !token;
      b.addEventListener('click', async (event) => {
        if (!event.isTrusted || busy || dead || !token || current?.digest !== p.digest) return;
        busy = true; for (const other of buttons()) other.disabled = true;
        const request = { scope_id: scope, revision: p.revision, digest: p.digest, token, decision };
        try {
          const result = await app.callServerTool({ name: 'gil_plan_decide', arguments: request }, { timeout: 10000 });
          if (dead) return;
          if (result.isError) throw new Error(result.structuredContent?.said || '승인 결과를 확인하지 못했습니다.');
          token = null; current = result.structuredContent?.proposal; last = result.structuredContent;
          busy = false;
          render(last); rendered = key(last); changed();
          busy = true;
          await notify(p, decision);
          if (dead) return;
          busy = false;
          render(last); rendered = key(last);
        } catch (err) {
          if (!dead) {
            token = null; current = null; rendered = null;
            uncertain = { revision: p.revision, decision, said: err?.message || '응답을 받지 못했습니다.' };
            render(last);
          }
        } finally { busy = false; }
      });
      panel.append(b);
    }
  }
  function proposal(value, summary) {
    const p = value.proposal;
    head(summary, ` · 계획 v${p.revision}${summary.base_revision != null ? ` (이전 승인 판 v${summary.base_revision})` : ''}`);
    if (replaced) panel.append(text('p', '보고 있던 제안이 새 판으로 바뀌었습니다. 이전 판에 대한 응답은 이 판에 적용되지 않습니다.', 'plan-replaced'));
    // What the decision means, then what it covers, then the controls. Full texts stay below as
    // collapsible, verifiable reference: nothing is dropped, it only comes after the decision.
    const synthesis = p.draft.synthesis || {};
    panel.append(text('p', verdictSentence(synthesis, p.draft.items.length), 'plan-verdict'));
    if (synthesis.statement) panel.append(text('p', synthesis.statement, 'plan-statement'));
    const labels = { goal: '목표', in_scope: '포함 범위', out_of_scope: '제외 범위', constraints: '제약', success_criteria: '성공 조건' };
    const dl = document.createElement('dl');
    for (const [k, label] of Object.entries(labels)) dl.append(text('dt', label), text('dd', p.draft.scope[k]));
    panel.append(dl, text('h3', '실행할 항목과 확인 조건'), text('p', counts(value), 'plan-counts'), itemList(value));
    reinterview(value);
    if (p.receipt) {
      const approve = p.receipt.decision === 'approve';
      panel.append(text('p', approve
        ? '승인을 기록했습니다. 제안 단계가 아직 열려 있습니다. 에이전트가 이어서 진행하지 않으면 채팅에서 승인 결과를 확인해 달라고 요청하세요.'
        : '수정 요청을 기록했습니다. 바꿀 내용을 채팅으로 알려 주면 에이전트가 다음 질문에서 확인합니다.', 'plan-receipt'));
      deliveryNote(p, p.receipt.decision);
    } else {
      panel.append(text('p', summary.state === 'no_token'
        ? '승인 버튼 연결을 아직 받지 못해 버튼을 비활성화했습니다. 자동으로 다시 읽습니다. 그대로라면 Monitor를 다시 열어 주세요.'
        : '이 내용에 동의하는 경우 승인하세요. 내용을 바꾸려면 수정 요청 후 채팅으로 알려 주세요.', 'plan-hint'));
      decisionButtons(p);
    }
    const full = document.createElement('details');
    full.append(text('summary', '승인할 제안문 전체 (참조·점검표 포함)'));
    const rest = document.createElement('dl');
    for (const k of Object.keys(synthesis)) rest.append(text('dt', k), text('dd', synthesis[k]));
    full.append(rest, text('pre', JSON.stringify(synthesis, null, 2)));
    const trace = document.createElement('details');
    trace.append(text('summary', '승인 기록 정보 (판·제안 주소·digest)'), text('p', `기준 승인 판: ${p.base_revision ?? '없음'} · 제안: ${p.synthesis_ref} · SHA-256: ${p.digest}`));
    panel.append(full, trace);
  }
  function executing(value, summary) {
    head(summary, ` · 계획 v${value.active_revision}`);
    const item = value.current_item;
    panel.append(text('p', item
      ? `지금 열린 Cycle에 묶인 항목: ${item.id} — ${item.question}\n확인 조건: ${item.done_when}`
      : '지금 열린 Cycle에 묶인 항목이 없습니다.', 'plan-current'));
    panel.append(text('p', counts(value), 'plan-counts'), itemList(value));
    reinterview(value);
    panel.append(text('small', 'GIL에 기록된 사실입니다. 에이전트가 지금 작업 중인지는 여기서 알 수 없습니다.'));
  }
  function render(value) {
    const stood = standing();
    const summary = planSummary(value, { hasToken: !!token, uncertain });
    const p = summary.state === 'unreadable' ? null : value?.proposal;
    if (p && !p.receipt) { replaced = shown !== null && shown !== p.digest; shown = p.digest; }
    else { replaced = false; if (p?.receipt) shown = null; }
    clear();
    panel.hidden = summary.state === 'unsupported';
    panel.dataset.state = summary.state;
    showBanner(summary);
    if (panel.hidden) return;
    switch (summary.state) {
      case 'uncertain': {
        head(summary, ` · 계획 v${uncertain.revision}`);
        panel.append(text('p', `${uncertain.decision === 'approve' ? '승인' : '수정 요청'} 응답을 보냈지만 결과를 받지 못했습니다: ${uncertain.said}`, 'plan-uncertain'));
        panel.append(text('p', '응답은 다시 보내지 않습니다. 다시 읽어 실제 기록을 확인합니다. 기록이 없으면 새 버튼이 나타납니다.', 'plan-hint'));
        const again = button('다시 읽기', { action: 'reread' });
        again.addEventListener('click', () => { void api.refresh(); });
        panel.append(again);
        break;
      }
      case 'unreadable': {
        head(summary, summary.said ? ` · ${summary.said}` : '');
        panel.append(text('p', '계획 승인 상태를 읽지 못했습니다. 연결되면 자동으로 다시 확인합니다. 버튼은 읽기에 성공한 뒤에 나타납니다.', 'plan-hint'));
        if (lastState) {
          panel.append(text('p', `마지막으로 읽은 상태: ${PLAN_STATE[lastState.state] || lastState.state}${lastState.revision != null ? ` · 계획 v${lastState.revision}` : ''}`, 'plan-last'));
        }
        break;
      }
      case 'idle':
        head(summary, '');
        panel.append(text('p', '에이전트가 Interview의 제안 단계에서 계획을 제안하면 여기서 승인하거나 수정을 요청합니다.', 'plan-hint'));
        reinterview(value);
        break;
      case 'executing':
        executing(value, summary);
        break;
      default:
        proposal(value, summary);
    }
    restore(stood);
  }
  const api = {
    async refresh() {
      if (dead || busy) return;
      busy = true;
      try {
        const result = await app.callServerTool({ name: 'gil_plan_review', arguments: { scope_id: scope } }, { timeout: 10000 });
        if (dead) return;
        if (result.isError || !result.structuredContent) throw new Error(result.structuredContent?.said || '계획 승인 상태를 읽지 못했습니다.');
        const value = result.structuredContent;
        current = value.proposal; last = value; uncertain = null;
        token = result._meta?.['gil/planApprovalToken'] ?? null;
        lastState = planSummary(value, { hasToken: !!token });
        const k = key(value);
        busy = false;
        // Preserve focus while polling. A renewed token does not change the visible proposal.
        if (k !== rendered) { render(value); rendered = k; }
      } catch (err) {
        if (!dead) { token = null; current = null; rendered = null; render({ code: 'unreadable', said: err?.message }); }
      } finally { busy = false; }
    },
    stop() {
      dead = true; token = null; current = null; clear(); panel.hidden = true;
      if (banner) { banner.replaceChildren(); banner.hidden = true; }
    },
  };
  return api;
}
