import { t } from './locale.js';
// Presentation of the canonical View projection, not reconstruction of a Report.
export function cycleReportDetail(cycle) {
  const report = cycle.report;
  const fields = report ? [
    {name:t('report.verdict'), value:report.verdict},
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
