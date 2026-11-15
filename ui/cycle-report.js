import { t } from './locale.js';
// Presentation of the canonical View projection, not reconstruction of a Report.
// The verdict is spoken in the person's words with the stored value beside it: `failure` is
// "success not confirmed", never a refutation on its own (Monitor Model §1.2).
const verdictText = (verdict) => {
  if (verdict == null || verdict === '') return verdict;
  const key = `ui.verdict.${verdict}`, found = t(key);
  return found === key ? String(verdict) : `${found} (${verdict})`;
};
export function cycleReportDetail(cycle) {
  const report = cycle.report;
  const fields = report ? [
    {name:t('report.verdict'), value:verdictText(report.verdict)},
    {name:t('report.handoff'), value:report.handoff_summary},
    ...(report.outcome_lesson == null ? [] : [{name:t('report.outcome_lesson'), value:report.outcome_lesson}]),
    ...(report.next_direction ? [
      {name:t('report.next_direction'), value:report.next_direction.action},
      ...(report.next_direction.reason == null ? [] : [{name:t('report.reason'),value:report.next_direction.reason}]),
      ...(report.next_direction.target_cycle_ref == null ? [] : [{name:t('report.target'),value:report.next_direction.target_cycle_ref}]),
    ] : []),
  ] : null;
  return {owner:{kind:'cycle',ref:cycle.cycle_ref},cycle_ref:cycle.cycle_ref,
    kind:cycle.kind,state:cycle.state,report:fields && {fields}};
}
