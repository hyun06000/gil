// 사람이 읽는 결과와 JSON 결과 — **같은 findings 에서 나온다.** 둘이 어긋나면 시험이 잡는다.

import { ALL_STATUSES, PROBLEM_STATUSES } from './check.mjs';

export const EXIT_CLEAN = 0;
export const EXIT_PROBLEMS = 1;
export const EXIT_TOOL_ERROR = 2;

const NOT_CHECKED = [
  '#절 주소: 파일 존재만 검사했고 절(heading) 존재는 검사하지 않았다.',
  '외부 웹 주소(http/https)와 이메일(mailto)은 네트워크를 쓰지 않으며 유효성을 확인하지 않았다.',
  '4칸 들여쓰기 코드 블록은 코드로 인식하지 않는다(목록 안 문단과 구분하지 않음). 울타리 코드·인라인 코드·HTML 주석만 가린다.',
  'HTML 속성(href/src) 연결은 unsupported 로 보고만 한다.',
  '존재 판정의 기준은 Git index 다. 추적되지 않은 파일만 가리키는 연결은 untracked 로 보고한다.',
];

export function summarize(findings) {
  const counts = Object.fromEntries(ALL_STATUSES.map((s) => [s, 0]));
  for (const f of findings) counts[f.status] = (counts[f.status] || 0) + 1;
  const problems = findings.filter((f) => PROBLEM_STATUSES.includes(f.status));
  return { counts, problems, exit_code: problems.length > 0 ? EXIT_PROBLEMS : EXIT_CLEAN };
}

export function toJson({ root, scope, findings }) {
  const { counts, problems, exit_code } = summarize(findings);
  return {
    tool: 'gil-docs-check',
    schema: 1,
    root,
    scope,
    counts,
    problem_statuses: PROBLEM_STATUSES,
    problems,
    findings,
    not_checked: NOT_CHECKED,
    exit_code,
  };
}

const problemLine = (f) => `${f.file}:${f.line}:${f.column}  [${f.status}]  ${f.target}  — ${f.reason}`;

export function toText({ scope, findings }, { verbose = false } = {}) {
  const { counts, problems, exit_code } = summarize(findings);
  const out = [];
  out.push(`검사 범위: 문서 ${scope.documents.length}개 (${scope.mode === 'tracked' ? 'Git 이 추적하는 Markdown 전체' : '지정한 문서'}), 연결 ${findings.length}개`);
  if (problems.length === 0) out.push('문제 없음.');
  else {
    out.push(`문제 ${problems.length}개:`);
    for (const f of problems) out.push(`  ${problemLine(f)}`);
  }
  out.push('');
  out.push('분류:');
  for (const s of ALL_STATUSES) if (counts[s]) out.push(`  ${s.padEnd(20)} ${counts[s]}`);
  if (verbose) {
    const rest = findings.filter((f) => !PROBLEM_STATUSES.includes(f.status));
    if (rest.length) {
      out.push('');
      out.push('문제 아님(참고):');
      for (const f of rest) out.push(`  ${problemLine(f)}`);
    }
  }
  out.push('');
  out.push('검사하지 않은 것:');
  for (const n of NOT_CHECKED) out.push(`  - ${n}`);
  out.push('');
  out.push(`종료 코드 ${exit_code} (0 문제 없음 · 1 문제 있음 · 2 도구 오류)`);
  return out.join('\n');
}
