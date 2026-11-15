// Before/after comparison of two analyses. Deltas are reported per tool, per help topic and in
// total, always next to the call counts, because two recordings with different call counts are
// two different runs, not two measurements of the same work.
import { UNITS } from './size.mjs';

function delta(before, after) {
  const out = {};
  for (const u of UNITS) {
    const b = before?.[u] ?? 0; const a = after?.[u] ?? 0;
    out[u] = { before: b, after: a, delta: a - b, ratio: b > 0 ? Number((a / b).toFixed(4)) : null };
  }
  return out;
}

function perCall(size, calls) {
  if (!calls) return null;
  const out = {};
  for (const u of UNITS) out[u] = Number(((size?.[u] ?? 0) / calls).toFixed(2));
  return out;
}

function row(key, b, a) {
  const bc = b?.calls ?? 0; const ac = a?.calls ?? 0;
  return {
    ...key,
    calls: { before: bc, after: ac, delta: ac - bc },
    call_count_differs: bc !== ac,
    text_response: delta(b?.text_response, a?.text_response),
    structured_response: delta(b?.structured_response, a?.structured_response),
    per_call_text_response: { before: perCall(b?.text_response, bc), after: perCall(a?.text_response, ac) },
  };
}

export function compareAnalyses(before, after) {
  const tools = new Set([...before.by_tool.map((t) => t.tool), ...after.by_tool.map((t) => t.tool)]);
  const byTool = [...tools].sort().map((tool) => row({ tool }, before.by_tool.find((t) => t.tool === tool), after.by_tool.find((t) => t.tool === tool)));
  const topics = new Set([...before.gil_help.topics.map((t) => t.topic), ...after.gil_help.topics.map((t) => t.topic)]);
  const helpTopics = [...topics].sort().map((topic) => {
    const b = before.gil_help.topics.find((t) => t.topic === topic); const a = after.gil_help.topics.find((t) => t.topic === topic);
    return { ...row({ topic }, b, a), repeated_lookups: { before: b?.repeated_lookups ?? 0, after: a?.repeated_lookups ?? 0 }, state_dependent: (b ?? a)?.state_dependent ?? false };
  });
  const totals = {
    ...row({}, before.totals, after.totals),
    record_line: delta(before.totals.record_line, after.totals.record_line),
    status_totals: { before: before.status_totals, after: after.status_totals },
    repeated_block_total: delta(before.repetition_candidates.repeated_total, after.repetition_candidates.repeated_total),
    help_calls: { before: before.gil_help.calls, after: after.gil_help.calls },
    help_repeated_lookups: { before: before.gil_help.repeated_lookups, after: after.gil_help.repeated_lookups },
    context_calls: { before: before.gil_context.calls, after: after.gil_context.calls },
  };
  const caveats = [
    '차이는 두 기록의 차이이며 제품의 판정이 아니다. 기록이 작아졌다는 것은 모델 비용이 줄었다거나 같은 작업을 더 싸게 했다는 증거가 아니다.',
    'text와 구조화 크기는 서로 다른 표현이며 그 차이를 합산하지 않는다.',
  ];
  if (totals.call_count_differs) caveats.push(`전체 호출 수가 다르다(${totals.calls.before} ↔ ${totals.calls.after}). 두 기록을 같은 작업으로 보지 않으며 호출당 평균은 참고용이다.`);
  const errB = before.status_totals.error_response + before.status_totals.rpc_error + before.status_totals.missing_result;
  const errA = after.status_totals.error_response + after.status_totals.rpc_error + after.status_totals.missing_result;
  if (errB !== errA) caveats.push(`ok가 아닌 응답 수가 다르다(${errB} ↔ ${errA}). 크기 차이는 짧아진 안내가 아니라 거절이나 응답 누락에서 올 수 있다.`);
  if (before.markers.truncated_strings || after.markers.truncated_strings || before.markers.redacted_strings || after.markers.redacted_strings) caveats.push('한쪽 이상의 기록에 잘림·가림 표식이 있다. 비교한 크기는 남아 있는 가림된 기록의 크기다.');
  if (before.input.unsupported_lines.length || after.input.unsupported_lines.length) caveats.push('한쪽 이상의 기록에 지원하지 않는 줄이 있었고 양쪽 모두에서 제외했다.');
  return {
    tool_version: before.tool_version,
    character_basis: before.character_basis,
    before: { file_name: before.input.file_name, sha256: before.input.sha256, events: before.input.events },
    after: { file_name: after.input.file_name, sha256: after.input.sha256, events: after.input.events },
    totals, by_tool: byTool, gil_help_topics: helpTopics, caveats,
  };
}
