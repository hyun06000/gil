// Offline measurement of one recording. Reads nothing but the parsed recording it is given.
// Separate measures are kept separate: text response (what a person reads), structured response
// (structuredContent), request arguments and the whole recorded line. They are never added into
// one "amount sent to the model" because the recording does not show how the Host composed the
// model input.
import { measure, zero, add, scale, stableStringify, sha256, blockId } from './size.mjs';
import { textOf, markersOf } from './events.mjs';

export const TOOL_VERSION = '0.1.0';
export const CHARACTER_BASIS = 'code_points';
export const DEFAULT_MIN_BLOCK_CODE_POINTS = 40;

/** Help topics whose answer depends on project state; a differing repeat is expected there. */
const STATE_DEPENDENT_TOPICS = new Set(['close']);
const NO_TOPIC_KEY = '(current-state)';

export const NORMALIZATION_RULES = [
  'Unicode NFC normalization',
  'leading/trailing whitespace trimmed',
  'runs of whitespace (spaces, tabs, newlines) collapsed to one space',
  'case preserved; no stemming, no tokenizer',
];

export function normalizeBlock(s) {
  return s.normalize('NFC').replace(/\s+/gu, ' ').trim();
}

/**
 * Repetition unit: each text content part is first its own boundary, then a part is cut into
 * paragraphs at blank lines. A block never spans two content parts.
 */
export function splitBlocks(parts) {
  const list = Array.isArray(parts) ? parts : [parts];
  return list.flatMap((part) => part.split(/\n[ \t]*\n+/u)).map(normalizeBlock).filter(Boolean);
}

function measureEvent(e) {
  const { text, parts, list } = textOf(e.result);
  const sc = e.result?.structuredContent;
  const structured = sc && typeof sc === 'object' ? stableStringify(sc) : null;
  const said = typeof sc?.said === 'string' ? sc.said : null;
  const normalizedText = text === null ? null : normalizeBlock(text);
  return {
    seq: e.seq, line: e.line, tool: e.name, actor: e.actor, synthetic: e.synthetic, status: e.status,
    topic: e.name === 'gil_help' ? topicKeyOf(e.args) : undefined,
    request_arguments: measure(stableStringify(e.args)),
    text_response: measure(text),
    text_parts: parts,
    structured_response: measure(structured),
    said: measure(said),
    said_in_text: said !== null && text !== null ? text.includes(said) : null,
    record_line: measure(e.raw),
    text_id: normalizedText ? blockId(normalizedText) : null,
    ...markersOf({ request: e.args, result: e.result, error: e.error }),
    _text: text,
    _parts: list,
  };
}

export function topicKeyOf(args) {
  const t = args?.topic;
  return typeof t === 'string' && t.trim() ? t.trim() : NO_TOPIC_KEY;
}

function emptyGroup() {
  return { calls: 0, by_status: { ok: 0, error_response: 0, rpc_error: 0, missing_result: 0 }, events_with_text: 0, events_with_structured: 0, text_response: zero(), structured_response: zero(), request_arguments: zero(), record_line: zero() };
}

function accumulate(g, m) {
  g.calls += 1;
  g.by_status[m.status] += 1;
  g.request_arguments = add(g.request_arguments, m.request_arguments);
  g.record_line = add(g.record_line, m.record_line);
  if (m.text_response) { g.events_with_text += 1; g.text_response = add(g.text_response, m.text_response); }
  if (m.structured_response) { g.events_with_structured += 1; g.structured_response = add(g.structured_response, m.structured_response); }
}

function perCall(m) {
  return { seq: m.seq, status: m.status, text_response: m.text_response, structured_response: m.structured_response, said: m.said, said_in_text: m.said_in_text, record_line: m.record_line, truncated_strings: m.truncated_strings, text_id: m.text_id };
}

export function analyzeRecording(recording, { source = null, minBlockCodePoints = DEFAULT_MIN_BLOCK_CODE_POINTS } = {}) {
  const measured = recording.events.map(measureEvent);
  const byTool = new Map();
  for (const m of measured) { if (!byTool.has(m.tool)) byTool.set(m.tool, emptyGroup()); accumulate(byTool.get(m.tool), m); }

  const context = measured.filter((m) => m.tool === 'gil_context');
  const contextGroup = emptyGroup(); context.forEach((m) => accumulate(contextGroup, m));
  const help = measured.filter((m) => m.tool === 'gil_help');
  const topics = new Map();
  for (const m of help) {
    if (!topics.has(m.topic)) topics.set(m.topic, { topic: m.topic, state_dependent: m.topic === NO_TOPIC_KEY || STATE_DEPENDENT_TOPICS.has(m.topic), calls: 0, ok_calls: 0, seqs: [], text_ids: new Set(), text_response: zero(), structured_response: zero(), per_call: [] });
    const t = topics.get(m.topic);
    t.calls += 1; if (m.status === 'ok') t.ok_calls += 1; t.seqs.push(m.seq);
    if (m.text_id) t.text_ids.add(m.text_id);
    t.text_response = add(t.text_response, m.text_response); t.structured_response = add(t.structured_response, m.structured_response);
    t.per_call.push(perCall(m));
  }
  const topicRows = [...topics.values()].sort((a, b) => a.topic.localeCompare(b.topic)).map((t) => ({
    topic: t.topic, state_dependent: t.state_dependent, calls: t.calls, ok_calls: t.ok_calls,
    repeated_lookups: Math.max(0, t.calls - 1),
    distinct_text_responses: t.text_ids.size,
    identical_text_repeats: Math.max(0, t.per_call.filter((c) => c.text_id).length - t.text_ids.size),
    seqs: t.seqs, text_response: t.text_response, structured_response: t.structured_response, per_call: t.per_call,
  }));

  // Repetition candidates: identical normalized paragraphs across all text responses.
  const blocks = new Map();
  for (const m of measured) {
    if (m._text === null) continue;
    const seen = new Set();
    for (const b of splitBlocks(m._parts)) {
      const size = measure(b);
      if (size.code_points < minBlockCodePoints) continue;
      const id = blockId(b);
      if (!blocks.has(id)) blocks.set(id, { id, size, occurrences: 0, events: new Set(), seqs: [], tools: new Set() });
      const row = blocks.get(id);
      row.occurrences += 1; row.seqs.push(m.seq); row.tools.add(m.tool);
      if (!seen.has(id)) { seen.add(id); row.events.add(m.seq ?? m.line); }
    }
  }
  const blockRows = [...blocks.values()].filter((b) => b.occurrences > 1).map((b) => ({
    id: b.id, occurrences: b.occurrences, events: b.events.size, seqs: b.seqs, tools: [...b.tools].sort(), size: b.size,
    repeated_size: scale(b.size, b.occurrences - 1),
  })).sort((a, b) => b.repeated_size.utf8_bytes - a.repeated_size.utf8_bytes || a.id.localeCompare(b.id));
  const repeatedTotal = blockRows.reduce((acc, b) => add(acc, b.repeated_size), zero());

  const identicalText = new Map();
  for (const m of measured) {
    if (!m.text_id) continue;
    if (!identicalText.has(m.text_id)) identicalText.set(m.text_id, { id: m.text_id, occurrences: 0, seqs: [], tools: new Set(), size: m.text_response });
    const r = identicalText.get(m.text_id); r.occurrences += 1; r.seqs.push(m.seq); r.tools.add(m.tool);
  }
  const identicalRows = [...identicalText.values()].filter((r) => r.occurrences > 1).map((r) => ({ id: r.id, occurrences: r.occurrences, seqs: r.seqs, tools: [...r.tools].sort(), size: r.size })).sort((a, b) => b.occurrences - a.occurrences || a.id.localeCompare(b.id));

  const statusTotals = { ok: 0, error_response: 0, rpc_error: 0, missing_result: 0 };
  for (const m of measured) statusTotals[m.status] += 1;
  const truncated = measured.reduce((n, m) => n + m.truncated_strings, 0);
  const redacted = measured.reduce((n, m) => n + m.redacted_strings, 0);
  const totals = emptyGroup(); measured.forEach((m) => accumulate(totals, m));

  const limits = [
    '크기는 기록 안 문자열의 UTF-8 바이트, 유니코드 코드 포인트("문자 수"), UTF-16 단위다. 토큰 계산기를 쓰지 않았으므로 어떤 값도 토큰 수가 아니다.',
    '기록은 도구 응답을 보여 줄 뿐 Host가 모델 입력을 어떻게 구성했는지는 보여 주지 않는다. text·구조화·기록 줄 크기는 서로 다른 표현이며 "모델에 전달된 양"으로 합산하지 않는다.',
    '전체 실행 비용(입력 구성, 재시도, 도구 호출이 아닌 턴)은 도구 호출 기록에서 관측할 수 없다.',
    '반복 행은 후보다: 정규화 뒤 동일한 단락이다. 승인·경고·거절 안내처럼 필요한 반복을 낭비로 판정하지 않으며 아무것도 제거하지 않는다.',
  ];
  if (truncated) limits.push(`잘림 표식이 있는 문자열 ${truncated}개: 남아 있는 부분만 측정했고 원문을 복원하지 않았다.`);
  if (redacted) limits.push(`가림 표식이 있는 문자열 ${redacted}개: 측정한 크기는 가림이 적용된 기록의 크기이며 원래 응답의 크기가 아니다.`);
  if (recording.unsupported.length) limits.push(`기록 형식에 맞지 않는 줄 ${recording.unsupported.length}개는 모든 측정에서 제외했다.`);

  return {
    tool_version: TOOL_VERSION,
    character_basis: CHARACTER_BASIS,
    input: { file_name: source?.file_name ?? null, sha256: source?.sha256 ?? null, lines: recording.lines, events: recording.events.length, unsupported_lines: recording.unsupported },
    status_totals: statusTotals,
    markers: { truncated_strings: truncated, redacted_strings: redacted },
    totals,
    by_tool: [...byTool.entries()].sort(([a], [b]) => a.localeCompare(b)).map(([tool, g]) => ({ tool, ...g })),
    gil_context: { ...contextGroup, said_in_text_count: context.filter((m) => m.said_in_text === true).length, per_call: context.map(perCall) },
    gil_help: { calls: help.length, distinct_topics: topicRows.length, repeated_lookups: topicRows.reduce((n, t) => n + t.repeated_lookups, 0), identical_text_repeats: topicRows.reduce((n, t) => n + t.identical_text_repeats, 0), topics: topicRows },
    repetition_candidates: { unit: 'paragraph (blank-line separated) of a text response', normalization: NORMALIZATION_RULES, min_block_code_points: minBlockCodePoints, block_id: 'sha256 of the normalized block, first 16 hex', blocks: blockRows, repeated_total: repeatedTotal, identical_text_responses: identicalRows },
    limits,
  };
}

export function fingerprint(text) { return sha256(text); }
