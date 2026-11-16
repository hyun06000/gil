// Terminal summaries. Short, Korean, numbers only — never a response body, token or path.
import { UNITS } from './size.mjs';

const UNIT_LABEL = { utf8_bytes: 'UTF-8 B', code_points: '문자(코드 포인트)', utf16_units: 'UTF-16' };

function size(s) { return s ? UNITS.map((u) => `${s[u]} ${UNIT_LABEL[u]}`).join(' · ') : '없음'; }
function sizeShort(s) { return s ? `${s.utf8_bytes} B / ${s.code_points} 자` : '없음'; }
function seqs(list, max = 12) { return list.length > max ? `${list.slice(0, max).join(',')} …(+${list.length - max})` : list.join(','); }

export function formatAnalysis(a) {
  const st = a.status_totals;
  const lines = [
    `# 문맥 전달량 측정 — ${a.input.file_name ?? '(stdin)'}`,
    `입력: ${a.input.lines}줄 · 기록 ${a.input.events}건 · 지원하지 않는 줄 ${a.input.unsupported_lines.length}`,
    `응답 상태: ok ${st.ok} · 오류 응답 ${st.error_response} · RPC 오류 ${st.rpc_error} · 결과 누락 ${st.missing_result}`,
    `가림/잘림 표식: 잘림 ${a.markers.truncated_strings} · 가림 ${a.markers.redacted_strings}`,
    '',
    '## 도구별 (text 응답 / 구조화 응답은 따로 읽는다)',
    ...a.by_tool.map((t) => `- ${t.tool}: ${t.calls}회 (ok ${t.by_status.ok}) · text ${sizeShort(t.text_response)} [${t.events_with_text}건] · 구조화 ${sizeShort(t.structured_response)} [${t.events_with_structured}건]`),
    '',
    `## gil_context: ${a.gil_context.calls}회`,
    `- text 합계 ${size(a.gil_context.text_response)}`,
    `- 구조화 합계 ${size(a.gil_context.structured_response)}`,
    `- said가 text에 그대로 포함된 호출: ${a.gil_context.said_in_text_count}/${a.gil_context.calls} (같은 내용의 두 표현; 합산하지 않는다)`,
    '',
    `## gil_help: ${a.gil_help.calls}회 · 주제 ${a.gil_help.distinct_topics}개 · 반복 조회 ${a.gil_help.repeated_lookups}회 · 그중 응답까지 동일 ${a.gil_help.identical_text_repeats}회`,
    ...a.gil_help.topics.map((t) => `- ${t.topic}${t.state_dependent ? ' (상태 의존)' : ''}: ${t.calls}회 (ok ${t.ok_calls}) · 반복 ${t.repeated_lookups} · 서로 다른 text 응답 ${t.distinct_text_responses} · text ${sizeShort(t.text_response)} · seq ${seqs(t.seqs)}`),
    '',
    `## 반복 후보 (단위: 빈 줄로 나눈 단락, ${a.repetition_candidates.min_block_code_points}자 이상, 정규화 후 동일)`,
    `- 후보 블록 ${a.repetition_candidates.blocks.length}개 · 첫 등장을 제외한 반복분 합계 ${size(a.repetition_candidates.repeated_total)}`,
    ...a.repetition_candidates.blocks.slice(0, 10).map((b) => `- ${b.id}: ${b.occurrences}회 / ${b.events}건 · 블록 ${sizeShort(b.size)} · 반복분 ${sizeShort(b.repeated_size)} · 도구 ${b.tools.join(',')} · seq ${seqs(b.seqs)}`),
    ...(a.repetition_candidates.blocks.length > 10 ? [`- … ${a.repetition_candidates.blocks.length - 10}개 더 (JSON 참조)`] : []),
    `- 전체 text 응답이 동일한 묶음 ${a.repetition_candidates.identical_text_responses.length}개`,
    '',
    '## 한계',
    ...a.limits.map((l) => `- ${l}`),
  ];
  return lines.join('\n');
}

function d(x) { return `${x.before} → ${x.after} (${x.delta >= 0 ? '+' : ''}${x.delta}${x.ratio !== null && x.ratio !== undefined ? `, ×${x.ratio}` : ''})`; }

export function formatComparison(c) {
  const t = c.totals;
  const lines = [
    `# 개선 전후 비교 — ${c.before.file_name ?? '?'} (${c.before.events}건) ↔ ${c.after.file_name ?? '?'} (${c.after.events}건)`,
    `호출 수: ${d(t.calls)}${t.call_count_differs ? ' — 호출 수가 다르므로 동일 작업 비교로 단정하지 않는다' : ''}`,
    `text 응답 합계: ${d(t.text_response.utf8_bytes)} B · ${d(t.text_response.code_points)} 자`,
    `구조화 응답 합계: ${d(t.structured_response.utf8_bytes)} B`,
    `기록 줄 합계: ${d(t.record_line.utf8_bytes)} B`,
    `gil_help 호출: ${t.help_calls.before} → ${t.help_calls.after} · 반복 조회 ${t.help_repeated_lookups.before} → ${t.help_repeated_lookups.after}`,
    `gil_context 호출: ${t.context_calls.before} → ${t.context_calls.after}`,
    `반복 블록 반복분: ${d(t.repeated_block_total.utf8_bytes)} B`,
    '',
    '## 도구별',
    ...c.by_tool.map((r) => `- ${r.tool}: 호출 ${d(r.calls)}${r.call_count_differs ? ' ⚠호출 수 다름' : ''} · text ${d(r.text_response.utf8_bytes)} B · 구조화 ${d(r.structured_response.utf8_bytes)} B`),
    '',
    '## gil_help 주제별',
    ...(c.gil_help_topics.length ? c.gil_help_topics.map((r) => `- ${r.topic}${r.state_dependent ? ' (상태 의존)' : ''}: 호출 ${d(r.calls)} · 반복 ${r.repeated_lookups.before} → ${r.repeated_lookups.after} · text ${d(r.text_response.utf8_bytes)} B`) : ['- (양쪽 모두 gil_help 없음)']),
    '',
    '## 주의',
    ...c.caveats.map((l) => `- ${l}`),
  ];
  return lines.join('\n');
}
