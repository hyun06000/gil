// Terminal summaries. Korean prose, identifiers and counts only; never the content of an
// example, a proposal or a message.
import { table } from '../../scenarios/lib/markdown.mjs';

export function formatFixtureReport(r) {
  const lines = [];
  lines.push(`사례 집합: ${r.case_count}건 · 비교 파일 ${r.comparison_count}건 · digest ${r.digest ? r.digest.slice(0, 16) : '(없음)'}`);
  if (!r.problems.length) lines.push('문제 없음. 구조·참조·예시 선언이 모두 일치한다. (의미 품질은 이 검사의 대상이 아니다)');
  else {
    lines.push(`문제 ${r.problems.length}건:`);
    for (const p of r.problems) lines.push(`- [${p.check}] ${p.file}: ${p.detail}`);
  }
  if (r.cases.length) {
    lines.push('');
    lines.push(table(['id', 'category', 'stages', 'good', 'bad', 'semantic'], r.cases.map((c) => [c.id, c.category, c.stages.join('+'), c.good, c.bad, c.semantic.join(' ')])));
  }
  return lines.join('\n');
}

export function formatResultReport(r) {
  const lines = [];
  const run = r.run;
  if (run.example) lines.push('※ 예시 결과 파일이다. 실제 평가 증거가 아니다.');
  lines.push(`실행: ${run.executed ? '모델 실행 기록 있음' : '모델 미실행'} · model ${run.model ? `${run.model.name}@${run.model.version}` : '(없음)'} · guidance ${run.guidance?.workflow_md_sha256 ? run.guidance.workflow_md_sha256.slice(0, 12) : '(없음)'} · commit ${run.guidance?.repo_commit || '(없음)'}`);
  if (r.case_set_match === false) lines.push(`주의: 결과의 case_set 해시가 현재 사례 집합과 다르다 (결과 ${String(run.case_set?.sha256 || '').slice(0, 12)} / 현재 ${String(r.current_case_set_sha256 || '').slice(0, 12)})`);
  lines.push(`사례 상태: evaluated ${r.counts.cases.evaluated} · unknown ${r.counts.cases.unknown} · not_run ${r.counts.cases.not_run} · 결과에 없음 ${r.counts.cases.missing}`);
  lines.push(`판정 수(자동): pass ${r.counts.automatic.pass} · fail ${r.counts.automatic.fail} · unknown ${r.counts.automatic.unknown} · not_run ${r.counts.automatic.not_run}`);
  lines.push(`판정 수(의미): pass ${r.counts.semantic.pass} · fail ${r.counts.semantic.fail} · unknown ${r.counts.semantic.unknown} · not_run ${r.counts.semantic.not_run}`);
  if (!run.executed) lines.push('모델을 실행하지 않았으므로 어떤 사례도 통과로 세지 않는다.');
  if (!r.problems.length) lines.push('일관성 문제 없음. (pass 개수는 품질 판정이 아니라 기록된 판정의 수다)');
  else {
    lines.push(`일관성 문제 ${r.problems.length}건:`);
    for (const p of r.problems) lines.push(`- [${p.check}] ${p.where}: ${p.detail}`);
  }
  if (r.cases.length) {
    lines.push('');
    lines.push(table(['case', 'status', 'approval', 'auto pass/fail/unk/nr', 'sem pass/fail/unk/nr'], r.cases.map((c) => [c.case_id, c.status, c.approval, `${c.automatic.pass}/${c.automatic.fail}/${c.automatic.unknown}/${c.automatic.not_run}`, `${c.semantic.pass}/${c.semantic.fail}/${c.semantic.unknown}/${c.semantic.not_run}`])));
  }
  return lines.join('\n');
}
