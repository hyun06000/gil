import { parseRecording, textOf } from '../context-metrics/lib/events.mjs';
import { analyzeRecording } from '../context-metrics/lib/analyze.mjs';
import { measure, sha256 } from '../context-metrics/lib/size.mjs';
export function summary(text, heading, end) {
  const start = text.indexOf(heading);
  const stop = text.indexOf(end, start);
  if (start < 0 || stop < start) throw new Error('resume summary boundaries missing');
  return text.slice(start, stop + end.length);
}
export function normalize(events, root, scope) {
  const walk = value => {
    if (typeof value === 'string') return (scope ? value.split(scope).join('<scope_id>') : value).split(root).join('<project_root>');
    if (Array.isArray(value)) return value.map(walk);
    if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([k,v]) => [k, walk(v)]));
    return value;
  };
  return events.map(walk);
}
export function report(events, locale) {
  const text = events.map(e => JSON.stringify(e)).join('\n')+'\n';
  const a = analyzeRecording(parseRecording(text));
  if (a.status_totals.ok !== events.length) throw new Error('not all measured responses succeeded');
  return {
    calls: events.length, totals: a.totals, by_tool: a.by_tool,
    help: {calls:a.gil_help.calls, repeated_lookups:a.gil_help.repeated_lookups, identical_text_repeats:a.gil_help.identical_text_repeats},
    repetition: {normalization:a.repetition_candidates.normalization, min_block_code_points:40, repeated_total:a.repetition_candidates.repeated_total, distinct_repeated_blocks:a.repetition_candidates.blocks.length, top_five_blocks:a.repetition_candidates.blocks.slice(0,5)},
    contexts: events.filter(e => e.request.name === 'gil_context').map(e => {
      const t=textOf(e.result).text;
      return {case:e.case,pass:e.pass,text:measure(t),text_sha256:sha256(t),summary:measure(summary(t,locale['ctx.resume.heading'],locale['ctx.resume.details']))};
    }), limits:a.limits,
  };
}
