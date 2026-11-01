// Presentation evidence only. Polling and file edits are not Agent activity.
export function progressTracker() {
  const scopes = new Map();
  return (scope, view, now = Date.now()) => {
    // Exclude read timestamps and world dirtiness: neither means a GIL record changed.
    const key = JSON.stringify([view.current, view.chain, view.timeline, view.current_will]);
    const previous = scopes.get(scope);
    const changed = !!previous && previous.key !== key;
    const state = { key, observedAt: changed || !previous ? now : previous.observedAt,
      changed: changed || previous?.changed || false };
    scopes.set(scope, state);
    return state;
  };
}

/** Attached visuals are counted from the recorded declarations only; a Report without one adds nothing. */
export function visualCounts(view) {
  let total = 0, here = 0, declaredNone = 0, missing = 0;
  for (const cycle of view.timeline) {
    for (const step of cycle.steps) {
      const v = step.visuals;
      if (!v) continue;
      if (v.state === 'attached') { total += v.count; if (cycle.cycle_ref === view.current.cycle_ref) here += v.count; }
      else if (v.state === 'not_needed') declaredNone += 1;
      else if (v.state === 'missing') { missing += 1; total += v.count; if (cycle.cycle_ref === view.current.cycle_ref) here += v.count; }
    }
  }
  return { total, here, declaredNone, missing };
}

export function progressSummary(view) {
  const cycle = view.timeline.find(one => one.cycle_ref === view.current.cycle_ref);
  const step = cycle?.steps.find(one => one.step_ref === view.current.step_ref);
  const will = view.current_will;
  const names = { question: '질문', interpretation: '해석', synthesis: '합의 확인',
    define: '문제 정의', hypothesis: '가설', verify: '구현·검증', analysis: '결과 분석', outcome: '판정' };
  const closed = view.timeline.reduce((n, one) => n + one.steps.filter(s => s.state === 'closed').length, 0);
  const visuals = visualCounts(view);
  return {
    visuals: visuals.total === 0 && visuals.declaredNone === 0
      ? '시각 자료 기록 없음'
      : `시각 자료 ${visuals.total}개 (이 Cycle ${visuals.here}개)${visuals.declaredNone ? ` · 없음 선언 ${visuals.declaredNone}건` : ''}${visuals.missing ? ` · 미완 ${visuals.missing}건` : ''}`,
    position: view.chain?.state === 'closed' ? 'Chain 종료' : step ? `${names[step.kind] || step.kind} · ${step.state === 'open' ? '열려 있음' : '닫힘'}` : '다음 단계 경계',
    action: view.chain?.state === 'closed' ? `최종 판정: ${view.chain.report?.verdict || ''}` : will?.next_action || '열린 행동 계약이 없습니다. 다음 단계를 확인합니다.',
    done: view.chain?.state === 'closed' ? (view.chain.report?.handoff_summary || 'Chain Report가 저장되었습니다.') : will?.done_when || '새 행동을 열 때 완료 조건을 정합니다.',
    records: `${view.current.chain_ref ? `Chain ${view.current.chain_ref}${view.chain?.state === "closed" ? ` 닫힘 (${view.chain.report?.verdict || ""})` : " 열림"} · ` : ''}닫힌 단계 ${closed}개 · 현재 ${view.current.cycle_ref}${view.current.step_ref ? ` / ${view.current.step_ref}` : ''}`,
  };
}
