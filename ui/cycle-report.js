// Presentation of the canonical View projection, not reconstruction of a Report.
export function cycleReportDetail(cycle) {
  const report = cycle.report;
  const fields = report ? [
    {name:'판정', value:report.verdict},
    {name:'인계 요약', value:report.handoff_summary},
    ...(report.outcome_lesson == null ? [] : [{name:'판정의 교훈 (Outcome)', value:report.outcome_lesson}]),
    ...(report.next_direction ? [
      {name:'당시 다음 방향', value:report.next_direction.action},
      ...(report.next_direction.reason == null ? [] : [{name:'이유',value:report.next_direction.reason}]),
      ...(report.next_direction.target_cycle_ref == null ? [] : [{name:'대상',value:report.next_direction.target_cycle_ref}]),
    ] : []),
  ] : null;
  return {owner:{kind:'cycle',ref:cycle.cycle_ref},cycle_ref:cycle.cycle_ref,
    kind:cycle.kind,state:cycle.state,report:fields && {fields}};
}
