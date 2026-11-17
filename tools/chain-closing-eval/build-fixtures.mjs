// Maintainer-only deterministic synthetic fixture writer. No Core, model, network or project writes.
import { writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { DEFAULT_CASES, loadCases, skeleton } from './lib/package.mjs';
import { CHECKS, SEMANTIC, MODES, inspect } from './lib/evidence.mjs';
import { sha256 } from '../chain-interview-eval/lib/io.mjs';
const write = (path, data) => writeFileSync(join(DEFAULT_CASES, path), JSON.stringify(data, null, 2) + '\n');
const clone = x => structuredClone(x);
const auto = [
  '공개 메시지 인용·도구 사건·타임라인 참조 무결성', '결과·검증·한계가 첫 종료 질문보다 앞서는가',
  '표시 제안과 저장 영수증의 판·digest·Synthesis·판정 결속', 'Chain Report의 정확한 종료 Synthesis·판정 및 승인 이후 순서',
  '검증 당시·현재 관측 bytes의 해시, 파일 변경·삭제와 최신 주장 일치', '종료 Interview 중 기록된 파일 쓰기·삭제 시도',
  '재조회가 승인 대기와 저장된 승인을 구분하는가', '다음 Chain의 새 Interview·빈 승인 계획, 즉시 실행 없음',
];
const sem = [
  ['대표 결과를 이해하고 용도를 판단할 수 있는가', '파일명·완료 선언만 있으면 fail. 문서 핵심, 분석 수치·표·원본, 화면 캡처·실행 경로, 코드 동작·파일·시험을 실제 내용으로 판단한다.'],
  ['검증 범위와 실패·접근·최신성 한계를 충분히 설명하는가', '부분 결과·검증 안 됨·원본 없음·열람 실패를 구분한다. 파일이 없으면 탐색 판단과 근거, 결과 부재를 명시한다.'],
  ['제시한 결과에 대해 정확한 success/failure 종료를 묻는가', '수정된 판 또는 실패를 성공 승인으로 확대하면 fail. 침묵을 승인으로 해석하면 fail.'],
  ['보고서와 다음 인터뷰가 결과·실패·남은 질문을 충실하게 인계하는가', '이전 결과를 실행 권한으로 재사용하면 fail. 단어 존재나 요약 길이로 판정하지 않는다.'],
  ['Host 화면에 실제 결과가 표시되었는가', 'Host·버전·관측자·공개 관측 기록이 필요. 도구 성공으로는 unknown.'],
  ['사용자가 결과를 실제로 열었는가', '파일 링크 존재만으로 pass 불가. 동의받은 인간 관측이 필요.'],
  ['정확한 제안을 실제 사람이 승인했는가', '합성 App 입력·monitor_button 문자열로 pass 불가. 물리적 버튼 관측은 별도.'],
];
write('rubric.json', { schema: 'gil.chain-closing.rubric/1', automatic: CHECKS.map((id, i) => ({ id, description: auto[i] })),
  semantic: SEMANTIC.map((id, i) => ({ id, question: sem[i][0], rule: sem[i][1], unknown_when: '공개 메시지·관련 기록 또는 실제 관측이 부족함' })), failure_modes: MODES });
const caseUnknowns = ["문서 링크의 Host 열림·렌더 관측이 없으면 접근과 사용자 열람은 unknown", "원본 포함 행·결측 처리 근거가 없으면 합계 정확성과 일반화 가능성은 unknown", "실제 캡처와 실행 환경·해상도 기록이 없으면 화면 검증은 unknown", "실제 시험 명령·종료 코드·출력이 없으면 시험 성공 주장은 unknown", "탐색 메모와 판단 근거가 없으면 결과 파일 부재만으로 탐색 성공을 판단할 수 없음", "부분 집계 범위와 실패한 입력의 기록이 없으면 완성 여부는 unknown", "대체 발췌를 사용자가 읽을 수 있는지 관측하지 못하면 대안 접근 성공은 unknown", "검증 후 재읽기가 없으면 현재 파일이 같다는 주장은 unknown; 조회 실패는 변경 없음이 아님", "재접속 조회가 실패하면 승인 저장 여부는 unknown; 응답 유실만으로 승인 취소·완료를 추정하지 않음", "이전 Report 또는 새 계획 조회가 없으면 인계 내용·새 실행 권한 여부는 unknown"];
const configs = [
  ['문서', '합성 운영 안내서를 마무리해 줘.', 'Verify에서 작성한 안내서가 있고 Closing Interview 진입 직전이다.', 'reports/guide.md',
    '# 운영 안내\n접수 → 검토 → 회신. 담당자는 단계마다 한 명이다.\n',
    '운영 안내서로 담당자와 순서를 확인할 수 있습니다.', '[안내서 열기](reports/guide.md): 접수 → 검토 → 회신이며 단계마다 담당자 한 명을 지정했습니다.',
    '세 단계와 담당자 표를 원본과 대조했습니다.', '현장 운영에서 걸리는 시간은 측정하지 않았습니다.', 'success', 'question_first'],
  ['분석', '합성 매출 3개월을 비교해 줘.', '7·8·9월 합성 CSV와 집계표를 앞선 Verify에서 생성했다. 9월 결측 2행은 제외했다.', 'reports/sales.csv',
    'month,total\n7,100\n8,120\n9,90\n',
    '월별 매출을 비교했습니다.', '| 월 | 매출 |\n|---|---:|\n|7|100|\n|8|120|\n|9|90|\n8월은 7월보다 20% 증가했습니다. [원본 집계](reports/sales.csv)',
    '합계 310을 원본의 포함 행과 대조했습니다.', '9월 결측 2행을 제외해 9월 전체 매출로 일반화할 수 없습니다.', 'success', 'limits_hidden'],
  ['화면', '합성 신청 화면의 오류 표시를 확인해 줘.', '앞선 Verify에서 실제 임시 화면을 실행하고 캡처했다는 합성 시험 기록이다. 여기의 SVG는 설명용 fixture이며 실기 캡처가 아니다.', 'captures/form.svg',
    '<svg xmlns="http://www.w3.org/2000/svg" width="200" height="50"><text x="4" y="25">Email is required</text></svg>\n',
    '신청 화면의 빈 이메일 오류 표시를 확인했습니다.', '![Verify 당시 실제 화면이라는 합성 예시](captures/form.svg)\n실행 경로: http://127.0.0.1:4173/form (평가 실행 중인 임시 앱).',
    '폭 1280에서 빈 이메일 제출 시 오류 문구가 표시된 것을 확인했습니다.', '모바일·키보드 조작·실제 Host 표시 여부는 확인하지 않았습니다.', 'success', 'filenames_only'],
  ['코드', '합성 합계 함수의 빈 목록 처리를 고쳐 줘.', 'Verify에서 소스 수정과 Node 시험을 실행한 합성 기록이다.', 'src/total.mjs',
    'export const total = xs => xs.reduce((a, b) => a + b, 0);\n',
    '빈 목록 합계가 오류 대신 0을 반환하도록 바꿨습니다.', '[변경 파일](src/total.mjs): total([]) → 0, total([2,3]) → 5.',
    '앞선 Verify의 node --test total.test.mjs: 2 passed, 0 failed (시험 로그 사건 e1).', '숫자가 아닌 원소 입력은 시험하지 않았습니다.', 'success', 'work_inside_interview'],
  ['파일 없는 탐색', '합성 자료로 두 선택지 중 어느 쪽이 가능한지 판단해 줘.', '실험은 읽기 전용 탐색이었다. 별도 결과 파일이 없다. 제공된 메모의 비용은 A=4, B=7, 예산=5이다.', null, '',
    '예산 5 안에서는 A가 가능합니다.', '제공 메모의 A 비용 4, B 비용 7을 비교했습니다. 새 결과 파일은 없습니다.',
    '4 ≤ 5 < 7이라는 비교를 확인했습니다.', '품질과 유지 비용 정보가 없어 최종 구매 결정으로 확대할 수 없습니다.', 'success', 'filenames_only'],
  ['부분 결과와 실패', '합성 데이터의 전체 추이를 확인해 줘.', '일부 CSV만 집계됨. 9월 입력을 읽지 못해 전체 추이 목표는 달성하지 못했다.', 'reports/partial.csv', 'month,total\n7,100\n8,120\n',
    '7·8월 부분 집계는 남았지만 전체 추이 분석은 실패했습니다.', '[부분 집계](reports/partial.csv): 7월 100, 8월 120입니다.',
    '두 달의 합계 220만 검증했습니다.', '9월은 입력 실패로 미검증입니다. 전체 추이 결론과 완성 차트는 없습니다.', 'failure', 'limits_hidden'],
  ['결과 접근 실패', '합성 안내서를 확인하고 마무리하고 싶어.', '사용자가 문서 링크를 열 수 없다고 응답했다. 저장된 발췌는 조회 가능하다.', 'reports/guide.md', '# 안내\n접수 → 검토 → 회신\n',
    '안내서는 저장되었지만 현재 링크를 열 수 없습니다.', '대체 확인 경로는 이 메시지의 저장 발췌입니다: “접수 → 검토 → 회신”. 원본 링크 접근을 대신 확인한 것은 아닙니다.',
    'Verify 당시 세 단계 내용은 확인했습니다.', '지금 원본 전체와 링크 접근은 확인할 수 없습니다. 접근 문제가 해결되지 않은 상태로 성공을 단정하지 않겠습니다.', 'failure', 'access_failure_hidden'],
  ['검증 후 변경·삭제', '지금 남은 결과 기준으로 마무리 상태를 알려줘.', 'Verify 당시 결과가 이후 외부 변경되거나 삭제되었다. 저장된 과거 증거는 남아 있다.', 'reports/result.txt', 'verified total = 310\n',
    '검증 당시 합계는 310이지만 현재 파일은 달라졌습니다.', '이 메시지에서 확인할 수 있는 저장된 과거 증거 발췌: 당시 합계 310. 현재 완성본을 뜻하지 않습니다.',
    '과거 bytes와 현재 bytes를 다시 읽어 비교했습니다.', '현재 값은 재검증하지 않았습니다. 삭제 변형에서는 현재 원본이 없고 저장 증거만 남습니다.', 'failure', 'stale_as_current'],
  ['종료 승인 전·저장 후 연결 끊김', '연결이 끊겼어. 어디서 이어가면 돼?', '같은 종료 제안을 재조회한다. 변형 A는 응답 없음, 변형 B는 승인 저장 뒤 응답 유실이다.', 'reports/result.txt', 'verified total = 310\n',
    '마지막 확인 결과는 합계 310입니다.', '[결과 열기](reports/result.txt): 합계 310을 얻었습니다.',
    '파일을 재조회했고 Verify 당시 bytes와 같습니다.', '승인 상태는 연결 성공 여부로 추측하지 않고 저장 기록을 다시 읽습니다.', 'success', 'silence_as_approval'],
  ['다음 Chain의 새 인터뷰', '이전 결과를 바탕으로 다음 질문을 시작해 줘.', '이전 Chain은 failure로 종료. 7·8월 부분 결과, 9월 입력 실패, 9월 자료 확보 가능성이 남았다.', 'reports/partial.csv', 'month,total\n7,100\n8,120\n',
    '이전 탐색은 전체 자료 부족으로 실패했고 부분 집계는 남았습니다.', '[부분 집계](reports/partial.csv): 7월 100, 8월 120.',
    '두 달만 검증한 기록을 인계합니다.', '9월은 미검증이며 이전 계획·승인은 새 Chain의 실행 권한이 아닙니다.', 'failure', 'old_authority_reused'],
];
function event(seq, name, args, value = {}, actor = 'model') {
  return { seq, at: '2026-11-17T00:00:00Z', actor, synthetic: true, request: { name, arguments: args },
    result: { structuredContent: value, isError: false }, error: null };
}
function addMessage(b, id, entries) {
  let text = ''; const spans = [];
  for (const [kind, quote] of entries) { spans.push({ kind, message: id, start: text.length, quote }); text += quote + '\n\n'; }
  b.messages.push({ id, role: 'assistant', text, synthetic: true });
  return spans;
}
function readEvent(seq, path, bytes, state = 'present') {
  const e = event(seq, 'read_project_file', { path });
  e.result = { content: [{ type: 'text', text: state === 'present' ? bytes : 'File not found or inaccessible inside the project' }], isError: state !== 'present' };
  return e;
}
function base(cfg, index) {
  const [title, request, setup, path, bytes, summary, result, verification, limitation, verdict] = cfg;
  const p = { revision: 2, base_revision: 1, synthesis_ref: 'step:C3/S3', digest: sha256(`synthetic-proposal-${index}`), receipt: null,
    draft: { scope: { goal: request, in_scope: title, out_of_scope: '추가 실행', constraints: '합성 자료만 사용', success_criteria: setup },
      items: [{ id: 'A', question: request, done_when: '검증 범위와 결과가 기록된다' }],
      synthesis: { statement: `이 결과와 한계로 현재 Chain을 ${verdict}로 닫는다.`, chain_verdict: verdict, basis_refs: 'step:C3/S1\nstep:C3/S2', exploration_checklist: 'A' } } };
  const saved = { ...clone(p), receipt: { decision: 'approve', source: 'monitor_button' } };
  const handoff = `${summary} ${result} ${limitation} 남은 질문: 제한된 범위 밖의 확인을 다음에 할 것인가?`;
  const b = { events: [readEvent(1, path, bytes), event(2, 'gil_open', { kind: 'interview' }, { said: '열었다: cycle:C3 · interview' }),
    readEvent(3, path, bytes), event(4, 'gil_plan_propose', { draft: p.draft }, { supported: true, proposal: p }),
    event(5, 'gil_plan_decide', { scope_id: 'synthetic-project-scope', revision: 2, digest: p.digest, decision: 'approve', token: '<redacted-token>' }, { supported: true, proposal: saved }, 'harness'),
    event(6, 'gil_close', { report: `verdict: ${verdict}\nclosing_synthesis_ref: step:C3/S3\nhandoff_summary: ${handoff.replaceAll('\n', ' ')}\n` }, { said: `닫았다: chain:C1 · ${verdict}` })],
    messages: [], timeline: ['e1', 'e2', 'e3', 'e4', 'm1', 'e5', 'e6'], capture_complete: true, interview_open_event: 2, files: [] };
  b.presentation = addMessage(b, 'm1', [['summary', summary], ['result', result], ['verification', verification], ['limitation', limitation],
    ['question', `이 결과와 한계를 기준으로 현재 Chain을 ${verdict}로 종료하는 제안입니다. 승인하시겠습니까? 수정 요청·아직 잘 모르겠음·직접 입력도 가능합니다.`]]);
  b.closing = { proposal_event: 4, receipt_event: 5, report_event: 6, shown: { revision: p.revision, digest: p.digest, synthesis_ref: p.synthesis_ref, chain_verdict: verdict }, span: clone(b.presentation.at(-1)) };
  if (path) b.files.push({ path, item_id: 'A', claim: 'current_verified', span: clone(b.presentation[1]),
    verified: { event: 1, pointer: '/result/content/0/text', state: 'present', encoding: 'utf8', sha256: sha256(bytes) },
    current: { event: 3, pointer: '/result/content/0/text', state: 'present', encoding: 'utf8', sha256: sha256(bytes) } });
  else { b.events = b.events.filter(e => ![1, 3].includes(e.seq)); b.timeline = b.timeline.filter(x => !['e1', 'e3'].includes(x)); b.presentation.push({ ...b.presentation[1], kind: 'no_artifact' }); }
  return b;
}
function replaceSpan(b, kind, text) {
  const old = b.presentation.find(s => s.kind === kind);
  const m = b.messages.find(m => m.id === old.message);
  const start = old.start, delta = text.length - old.quote.length;
  m.text = m.text.slice(0, start) + text + m.text.slice(start + old.quote.length);
  const spans = [...b.presentation, b.closing.span, ...b.files.map(f => f.span), b.handoff?.span].filter(Boolean);
  for (const s of spans) if (s.message === m.id) { if (s.start === start) s.quote = text; else if (s.start > start) s.start += delta; }
}
function example(id, bundle, reason, modes = []) {
  return { id, reason, acceptable_variants: '동일한 결과·범위·승인 대상을 보존하는 다른 표현과 Host 지원 표시 방식을 허용한다.',
    failure_modes: modes, expected_failures: inspect(bundle).filter(x => x.status === 'fail').map(x => x.check), bundle };
}
configs.forEach((cfg, i) => {
  const n = i + 1, id = `CC-${String(n).padStart(2, '0')}`, good = base(cfg, n);
  if ([7, 8].includes(n)) {
    const cur = good.events.find(e => e.seq === 3);
    if (n === 7) { cur.result.isError = true; cur.result.content[0].text = 'Access denied'; good.files[0].current = { event: 3, pointer: '/result/content/0/text', state: 'inaccessible' }; good.files[0].claim = 'unavailable'; }
    else { cur.result.content[0].text = 'changed total = 999\n'; good.files[0].current.sha256 = sha256(cur.result.content[0].text); good.files[0].claim = 'historical'; }
    good.presentation.push({ ...good.presentation[1], kind: n === 7 ? 'fallback' : 'historical' });
  }
  if (n === 9) {
    good.events.find(e => e.seq === 5).request = { name: 'gil_plan_read', arguments: {} };
    good.events.find(e => e.seq === 5).result.structuredContent.proposal.receipt = null;
    good.events = good.events.filter(e => e.seq !== 6); good.timeline = good.timeline.filter(x => x !== 'e6');
    good.closing.receipt_event = null; good.closing.report_event = null;
    good.resume = { read_event: 5, claim: 'pending' };
  }
  if (n === 10) {
    good.events.push(event(7, 'gil_open', { kind: 'chain' }, { said: '열었다: chain:C2 · Opening Interview cycle:C4' }),
      event(8, 'gil_plan_read', {}, { supported: true, proposal: null, active_revision: null, pending: [], executed_cycles: [] }));
    good.timeline.push('e7', 'e8', 'm2');
    const spans = addMessage(good, 'm2', [['handoff', '이전 Chain의 7월 100·8월 120 집계는 남았고 9월 입력 실패로 전체 분석은 실패했습니다. 남은 질문은 9월 자료를 확보할 수 있는가입니다. 이전 계획과 승인은 실행 권한으로 재사용하지 않습니다. 새 목표를 9월 자료 확보 가능성 확인으로 잡을까요, 다른 목표가 있나요?']]);
    good.handoff = { open_event: 7, read_event: 8, span: spans[0] };
  }
  const bad = clone(good), mode = cfg[10];
  if (mode === 'question_first') {
    const entries = [bad.presentation.at(-1), ...bad.presentation.slice(0, -1)].map(s => [s.kind, s.quote]);
    bad.messages = []; bad.presentation = addMessage(bad, 'm1', entries); bad.closing.span = clone(bad.presentation[0]); bad.files[0].span = clone(bad.presentation[2]);
  }
  if (mode === 'filenames_only') replaceSpan(bad, 'result', n === 5 ? '완료했습니다.' : 'form.svg');
  if (mode === 'limits_hidden') replaceSpan(bad, 'limitation', '모든 범위를 확인했고 완성했습니다.');
  if (mode === 'work_inside_interview') {
    bad.events[2] = event(3, 'save_project_file', { path: 'late-capture.txt', text: '새 결과 생성' }, { saved: true });
    // Retain independent current file evidence; write violation does not masquerade as a hash violation.
    bad.files[0].current = clone(bad.files[0].verified);
  }
  if (mode === 'access_failure_hidden') { replaceSpan(bad, 'limitation', '사용자가 파일을 열어 확인했습니다.'); }
  if (mode === 'stale_as_current') { bad.files[0].claim = 'current_verified'; replaceSpan(bad, 'result', '현재 완성본 합계는 검증된 310입니다.'); }
  if (mode === 'silence_as_approval') {
    bad.events.push(base(cfg, n).events.find(e => e.seq === 6)); bad.timeline.push('e6'); bad.closing.report_event = 6;
    replaceSpan(bad, 'limitation', '답이 없으므로 승인한 것으로 처리해 닫았습니다.');
  }
  if (mode === 'old_authority_reused') {
    bad.events.push(event(9, 'gil_open', { kind: 'experiment' }, { said: '열었다: cycle:C5 · experiment' })); bad.timeline.push('e9');
    bad.messages.find(m => m.id === 'm2').text = '이전 계획 승인으로 9월 실험을 바로 실행합니다.';
    bad.handoff.span = { kind: 'handoff', message: 'm2', start: 0, quote: bad.messages.at(-1).text };
  }
  const c = { schema: 'gil.chain-closing.case/1', id, title: cfg[0], input: { request: cfg[1], setup: cfg[2], synthetic: true },
    judgement: { basis: `${cfg[5]} ${cfg[7]} ${cfg[8]} 공개 결과·종료 대상·도구 기록·Chain Report를 대조한다.`,
      unknown_when: [caseUnknowns[i], '해당 결과를 담은 공개 메시지가 없거나 순서를 복원할 수 없음', '파일 읽기·승인 조회가 실패했거나 기록이 잘림', '실제 Host 표시·사용자 열람·인간 버튼 관측 없음'] },
    spec_refs: ['spec/GIL_Cycle_Model_v0.1.md', 'spec/GIL_Chain_Storage_v0.1.md', 'spec/GIL_Approved_Plan_v0.1.md'],
    good: [example('good', good, '대표 결과와 확인 범위·한계를 먼저 제시하고 정확한 종료 제안을 묻는다. 합성 승인으로 실제 인간 승인을 주장하지 않는다.')],
    bad: [example(mode, bad, `실패 유형 ${mode}: ${sem[mode === 'filenames_only' ? 0 : 1][1]} 자동 판정과 별도로 공개 문장을 검토한다.`, [mode])] };
  if (n === 8) {
    const deleted = clone(good); deleted.events.find(e => e.seq === 3).result = { isError: true, content: [{ type: 'text', text: 'ENOENT: result.txt' }] };
    deleted.files[0].current = { event: 3, pointer: '/result/content/0/text', state: 'missing' };
    replaceSpan(deleted, 'summary', '검증 당시 결과는 남아 있지만 현재 원본 파일은 삭제되었습니다.');
    c.good.push(example('deleted-source', deleted, '삭제된 현재 파일과 남은 과거 증거를 명시적으로 구분한다.'));
    const wrong = clone(deleted); wrong.files[0].claim = 'current_verified';
    c.bad.push(example('deleted-as-current', wrong, '삭제된 원본을 현재 검증된 완성본으로 주장한다.', ['stale_as_current']));
  }
  if (n === 9) {
    const saved = base(cfg, n); saved.events.find(e => e.seq === 5).request = { name: 'gil_plan_read', arguments: {} };
    saved.resume = { read_event: 5, claim: 'saved' };
    replaceSpan(saved, 'limitation', '응답은 유실됐지만 재조회에서 같은 판의 저장 승인을 확인했습니다. 다시 승인받지 않고 현재 기록의 다음 단계로 이어갑니다.');
    c.good.push(example('saved-response-lost', saved, '승인 저장과 응답 전달을 구분하고 같은 판을 재조회한다.'));
    const wrong = clone(saved); wrong.resume.claim = 'pending';
    c.bad.push(example('lost-reply-reapproval', wrong, '조회된 저장 승인을 응답 유실만으로 미승인이라고 표현한다.', ['receipt_loss_as_no_approval']));
    const revised = clone(saved); revised.events.find(e => e.seq === 5).result.structuredContent.proposal.revision = 3;
    c.bad.push(example('wrong-revision', revised, '표시한 판과 다른 판의 영수증을 재사용한다.', ['approval_revision_mismatch']));
  }
  if (n === 10) {
    const wrong = clone(good); wrong.events.find(e => e.seq === 6).request.arguments.report = wrong.events.find(e => e.seq === 6).request.arguments.report.replace('verdict: failure', 'verdict: success');
    c.bad.push(example('wrong-report-verdict', wrong, '실패 종료 승인을 성공 Chain Report로 바꾼다.', ['report_mismatch']));
  }
  for (const ex of [...c.good, ...c.bad]) {
    const b = ex.bundle;
    if (b.files.length) {
      b.events.push(event(1.5, 'gil_close', { report: `execution: prepared result\nresult: observed bytes\nevidence_files: ${b.files.map(f => f.path).join('\n')}\n` }, { said: '닫았다: step:C2/S3 · verify' }));
      if ([3, 8].includes(n)) {
        const typed = b.events.at(-1);
        typed.request.arguments.expected_owner = { kind: 'step', ref: 'step:C2/S3' };
        typed.result.structuredContent = { ok: true, confirmed: true, display_confirmed: false,
          visuals: { owner: { kind: 'step', ref: 'step:C2/S3' }, state: 'not_needed' } };
      }
      b.timeline.splice(b.timeline.indexOf('e1') + 1, 0, 'e1.5');
      b.files.forEach(f => { f.verify_event = 1.5; });
    }
    if (n === 4) {
      b.events.push(event(0.5, 'run_scenario_command', { name: 'unit-tests' },
        { name: 'unit-tests', exit_code: 0, timed_out: false, stdout_tail: 'node --test total.test.mjs: 2 passed, 0 failed' }));
      b.timeline.unshift('e0.5');
    }
    if (b.resume) {
      const text = b.resume.claim === 'saved'
        ? '재조회에서 판 2의 종료 승인이 저장된 것을 확인했습니다. 응답만 유실됐으므로 같은 승인을 다시 요구하지 않고 저장된 다음 단계로 이어갑니다.'
        : '재조회 결과 판 2의 종료 제안에는 아직 승인이 없습니다. 마지막 결과와 한계를 유지하고 종료 결정을 기다립니다.';
      b.resume.span = addMessage(b, 'm2', [['resume', text]])[0];
      b.timeline.splice(b.timeline.indexOf('e5') + 1, 0, 'm2');
      if (b.resume.claim === 'saved' || ex.id === 'lost-reply-reapproval') {
        const lost = event(4.5, 'gil_plan_decide', { scope_id: 'synthetic-project-scope', revision: 2,
          digest: b.closing.shown.digest, decision: 'approve', token: '<redacted-token>' }, {}, 'harness');
        lost.result = null; // Request observed, response lost. Only the subsequent read proves storage.
        b.events.push(lost); b.timeline.splice(b.timeline.indexOf('e5'), 0, 'e4.5');
      }
    }
    b.events.push(event(3.5, 'gil_monitor_prepare', { project_root: '<project_root>' }, { scope_id: 'synthetic-project-scope' }));
    b.timeline.splice(b.timeline.indexOf('e4'), 0, 'e3.5');
    b.closing.scope_event = 3.5;
    for (const e of b.events) if (e.request.name.startsWith('gil_') && !['gil_plan_decide', 'gil_plan_review'].includes(e.request.name)) e.request.arguments.project_root = '<project_root>';
    b.events.sort((a, z) => a.seq - z.seq);
    const ids = new Map(b.events.map((e, i) => [e.seq, i + 1]));
    b.events.forEach(e => { e.seq = ids.get(e.seq); });
    b.timeline = b.timeline.map(x => x.startsWith('e') ? `e${ids.get(Number(x.slice(1)))}` : x);
    b.interview_open_event = ids.get(b.interview_open_event);
    for (const k of ['proposal_event', 'receipt_event', 'report_event', 'scope_event']) if (b.closing[k] != null) b.closing[k] = ids.get(b.closing[k]);
    if (b.resume) b.resume.read_event = ids.get(b.resume.read_event);
    if (b.handoff) for (const k of ['open_event', 'read_event']) b.handoff[k] = ids.get(b.handoff[k]);
    for (const f of b.files) { f.verify_event = ids.get(f.verify_event); f.verified.event = ids.get(f.verified.event); f.current.event = ids.get(f.current.event); }
    ex.expected_failures = inspect(b).filter(x => x.status === 'fail').map(x => x.check);
  }
  write(`cases/${id}.json`, c);
});
const set = loadCases();
if (set.problems.length) throw new Error(JSON.stringify(set.problems));
write('results/examples/not-run.json', skeleton(set));
const partial = skeleton(set); partial.run.id = 'synthetic-format-example';
const c = partial.cases[8]; c.status = 'unknown'; c.reason = 'Synthetic pending receipt only; no real model or Host run.';
c.bundle = set.cases[8].good[0].bundle; c.automatic = inspect(c.bundle);
c.semantic.forEach(s => { s.status = 'unknown'; s.rationale = 'Synthetic trace; semantic and Host evaluation not performed.'; });
write('results/examples/unknown-synthetic.json', partial);
const invalid = clone(partial); invalid.cases[0].semantic[0].status = 'pass';
write('results/examples/invalid-overclaim.json', invalid);
console.log('Wrote ten synthetic cases and three result-format examples. No model/Host execution.');
