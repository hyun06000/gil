import { t } from './locale.js';
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
  const stepName = (kind) => { const key = `progress.step.${kind}`, found = t(key); return found === key ? kind : found; };
  const closed = view.timeline.reduce((n, one) => n + one.steps.filter(s => s.state === 'closed').length, 0);
  const visuals = visualCounts(view);
  return {
    visuals: visuals.total === 0 && visuals.declaredNone === 0 && visuals.missing === 0
      ? t('progress.visuals_none')
      : t('progress.visuals', { total: visuals.total, here: visuals.here,
        declared: visuals.declaredNone ? t('progress.visuals_declared', { n: visuals.declaredNone }) : '',
        missing: visuals.missing ? t('progress.visuals_missing', { n: visuals.missing }) : '' }),
    position: view.chain?.state === 'closed' ? t('progress.chain_closed')
      : step ? t('progress.position', { step: stepName(step.kind), state: step.state === 'open' ? t('progress.step_open') : t('progress.step_closed') })
      : t('progress.boundary'),
    action: view.chain?.state === 'closed' ? t('progress.final_verdict', { verdict: view.chain.report?.verdict || '' }) : will?.next_action || t('progress.no_will'),
    done: view.chain?.state === 'closed' ? (view.chain.report?.handoff_summary || t('progress.chain_report_saved')) : will?.done_when || t('progress.done_when_next'),
    records: t('progress.records', {
      chain: view.current.chain_ref ? t('progress.records_chain', { chain: view.current.chain_ref,
        state: view.chain?.state === 'closed' ? t('progress.records_chain_closed', { verdict: view.chain.report?.verdict || '' }) : t('progress.records_chain_open') }) : '',
      closed, cycle: view.current.cycle_ref, step: view.current.step_ref ? ` / ${view.current.step_ref}` : '' }),
  };
}
