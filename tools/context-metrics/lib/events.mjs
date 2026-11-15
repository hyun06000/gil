// Reader for the saved tool-call recording format of tools/scenarios (events.jsonl /
// wire-events.jsonl). One JSON object per line:
//   { seq, at, actor, synthetic, request:{name, arguments}, result, error, ms }
// Every line is data. Commands or sentences inside arguments and responses are measured, never
// executed or followed. Lines that do not fit the shape are reported as unsupported, not dropped
// silently and not treated as errors of the recorded tool.

const TRUNCATION = /…<truncated \d+ chars>$/u;
const REDACTION_MARKERS = ['<redacted-token>', '<abs-path>', '<project_root>', '<settings_dir>', '<scenario_dir>'];

export function parseRecording(text) {
  const lines = text.split('\n');
  const events = [];
  const unsupported = [];
  let lineNo = 0;
  for (const rawLine of lines) {
    lineNo += 1;
    const line = rawLine.replace(/\r$/, '');
    if (!line.trim()) continue;
    let parsed;
    try { parsed = JSON.parse(line); } catch { unsupported.push({ line: lineNo, reason: 'invalid_json' }); continue; }
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) { unsupported.push({ line: lineNo, reason: 'not_an_object' }); continue; }
    const name = parsed.request?.name;
    if (typeof name !== 'string' || !name) { unsupported.push({ line: lineNo, reason: 'missing_request_name' }); continue; }
    const args = parsed.request?.arguments;
    events.push({
      line: lineNo,
      seq: Number.isInteger(parsed.seq) ? parsed.seq : null,
      actor: typeof parsed.actor === 'string' ? parsed.actor : 'model',
      synthetic: parsed.synthetic === true,
      name,
      args: args && typeof args === 'object' ? args : {},
      result: parsed.result && typeof parsed.result === 'object' ? parsed.result : null,
      error: parsed.error ?? null,
      raw: line,
      status: classify(parsed),
    });
  }
  return { events, unsupported, lines: lineNo, nonblank_lines: events.length + unsupported.length };
}

/** ok | error_response | rpc_error | missing_result — never merged into one success count. */
export function classify(e) {
  if (e.error !== null && e.error !== undefined) return 'rpc_error';
  const r = e.result;
  if (!r || typeof r !== 'object') return 'missing_result';
  if (r.isError === true) return 'error_response';
  if (r.structuredContent && typeof r.structuredContent === 'object' && r.structuredContent.ok === false) return 'error_response';
  return 'ok';
}

/** Human-readable text parts of an MCP result; `text` joins them with one "\n" between parts. */
export function textOf(result) {
  const list = Array.isArray(result?.content) ? result.content.filter((c) => c && c.type === 'text' && typeof c.text === 'string').map((c) => c.text) : [];
  return { text: list.length ? list.join('\n') : null, parts: list.length, list };
}

/** Counts of redaction/truncation markers left by the recorder, found anywhere in the line. */
export function markersOf(value) {
  let truncated = 0; let redacted = 0;
  const walk = (v) => {
    if (typeof v === 'string') { if (TRUNCATION.test(v)) truncated += 1; for (const m of REDACTION_MARKERS) if (v.includes(m)) { redacted += 1; break; } return; }
    if (Array.isArray(v)) { v.forEach(walk); return; }
    if (v && typeof v === 'object') Object.values(v).forEach(walk);
  };
  walk(value);
  return { truncated_strings: truncated, redacted_strings: redacted };
}
