// Offline transcription only. Never open paths or dispatch requests found in evidence.
import { isDeepStrictEqual } from 'node:util';
import { inspect, nonblank, pointer } from './evidence.mjs';
import { skeleton, checkResult } from './package.mjs';
import { sha256 } from '../../chain-interview-eval/lib/io.mjs';
import { okOf } from '../../scenarios/lib/trace.mjs';

export class ImportError extends Error {
  constructor(code) { super(code); this.code = code; }
}
const requireThat = (ok, code) => { if (!ok) throw new ImportError(code); };
const object = x => x !== null && typeof x === 'object' && !Array.isArray(x);
const keys = (x, allowed) => object(x) && Object.keys(x).every(k => allowed.includes(k));
const annotationKeys = ['presentation', 'interview_open_event', 'capture_complete',
  'artifact_write_events', 'closing', 'resume', 'handoff'];

function source(c, name, required = true) {
  const s = c[name];
  if (s == null && !required) return undefined;
  requireThat(keys(s, ['execution_id', 'project_root', 'case_id', 'data', 'first_seq', 'last_seq']), 'source_shape');
  requireThat(s.execution_id === c.execution_id && s.project_root === c.project_root
    && s.case_id === c.case_id, 'source_identity');
  requireThat(Object.hasOwn(s, 'data'), 'source_data');
  return structuredClone(s.data);
}

// Validate references even when there is no cross-stream timeline. Do not manufacture one
// just to satisfy inspect: incomplete input is retained separately as preparation material.
function validateReferences(b, c) {
  requireThat(Array.isArray(b.events) && Array.isArray(b.messages) && Array.isArray(b.files), 'source_arrays');
  const seen = new Set();
  for (const [i, e] of b.events.entries()) {
    requireThat(object(e) && Number.isSafeInteger(e.seq) && e.seq > 0 && !seen.has(e.seq)
      && nonblank(e.request?.name) && ['model', 'app', 'harness'].includes(e.actor)
      && typeof e.synthetic === 'boolean', 'event_shape_or_duplicate');
    requireThat(i === 0 || e.seq === b.events[i - 1].seq + 1, 'event_gap_or_order');
    seen.add(e.seq);
    // Optional identity on individual records must agree with the explicit source envelope.
    for (const key of ['execution_id', 'project_root', 'case_id']) {
      requireThat(!Object.hasOwn(e, key) || e[key] === c[key], 'record_identity');
    }
    const root = e.request.arguments?.project_root;
    requireThat(root === undefined || root === c.project_root, 'event_project');
  }
  requireThat(c.events.first_seq === (b.events[0]?.seq ?? null)
    && c.events.last_seq === (b.events.at(-1)?.seq ?? null), 'event_range');
  const mids = new Set();
  for (const m of b.messages) {
    requireThat(object(m) && /^m[1-9]\d*$/.test(m.id) && !mids.has(m.id)
      && nonblank(m.text) && ['assistant', 'user', 'observer'].includes(m.role)
      && typeof m.synthetic === 'boolean', 'message_shape_or_duplicate');
    mids.add(m.id);
    for (const key of ['execution_id', 'project_root', 'case_id']) {
      requireThat(!Object.hasOwn(m, key) || m[key] === c[key], 'record_identity');
    }
  }
  requireThat(b.presentation === undefined || Array.isArray(b.presentation), 'annotation_shape');
  requireThat(b.artifact_write_events === undefined || Array.isArray(b.artifact_write_events), 'annotation_shape');
  requireThat(b.capture_complete === undefined || typeof b.capture_complete === 'boolean', 'annotation_shape');
  for (const key of ['closing', 'resume', 'handoff']) requireThat(b[key] == null || object(b[key]), 'annotation_shape');
  requireThat(b.files.every(object), 'file_shape');
  const spans = [...(b.presentation ?? []), ...b.files.map(f => f.span), b.closing?.span,
    b.resume?.span, b.handoff?.span].filter(s => s != null);
  for (const s of spans) {
    const m = b.messages.find(m => m.id === s.message);
    requireThat(m && nonblank(s.quote) && Number.isSafeInteger(s.start) && s.start >= 0
      && m.text.slice(s.start, s.start + s.quote.length) === s.quote, 'broken_quote');
  }
  const refs = [b.interview_open_event, b.closing?.proposal_event, b.closing?.receipt_event,
    b.closing?.report_event, b.closing?.scope_event, b.resume?.read_event, b.handoff?.open_event,
    b.handoff?.read_event, ...(b.artifact_write_events ?? []),
    ...b.files.flatMap(f => [f.verify_event, f.verified?.event, f.current?.event])];
  requireThat(refs.every(id => id == null || seen.has(id)), 'missing_event_reference');
  for (const f of b.files) for (const observation of [f.verified, f.current]) {
    if (observation == null) continue;
    requireThat(object(observation) && typeof observation.pointer === 'string'
      && observation.pointer.startsWith('/result/') && !/~(?![01])/u.test(observation.pointer)
      && pointer(b.events.find(e => e.seq === observation.event), observation.pointer) !== undefined,
    'broken_file_pointer');
  }
  // Scope-based App calls must resolve to this project's recorded prepare. Never resolve
  // a scope via a live service, filesystem canonicalization, or a different input run.
  const scopes = new Map();
  for (const e of b.events) {
    const args = e.request.arguments ?? {};
    if (e.request.name === 'gil_monitor_prepare' && okOf(e)) {
      const id = e.result?.structuredContent?.scope_id;
      if (nonblank(id)) { requireThat(args.project_root === c.project_root, 'scope_project'); scopes.set(id, c.project_root); }
    }
    if (args.scope_id !== undefined) requireThat(scopes.get(args.scope_id) === c.project_root, 'unresolved_scope');
  }
}

export function importEvidence(input, set) {
  requireThat(keys(input, ['schema', 'run', 'cases']) && input.schema === 'gil.chain-closing.import/1'
    && object(input.run) && Array.isArray(input.cases) && input.cases.length > 0, 'input_shape');
  requireThat(!set.problems.length, 'invalid_case_set');
  const result = skeleton(set);
  result.run = structuredClone(input.run); // explicit declarations, never inferred from actors
  result.import_evidence = {
    schema: 'gil.chain-closing.import-provenance/1',
    source_sha256: sha256(JSON.stringify(input)),
    source: structuredClone(input), cases: [],
  };
  const seen = new Set(), executions = new Set();
  for (const [index, c] of input.cases.entries()) {
    requireThat(keys(c, ['case_id', 'execution_id', 'project_root', 'events', 'messages', 'timeline', 'annotations', 'files'])
      && nonblank(c.execution_id) && nonblank(c.project_root), 'case_shape');
    const target = result.cases.find(x => x.case_id === c.case_id);
    requireThat(target && !seen.has(c.case_id) && !executions.has(c.execution_id), 'case_or_execution_duplicate');
    seen.add(c.case_id); executions.add(c.execution_id);
    const annotations = source(c, 'annotations');
    requireThat(keys(annotations, annotationKeys), 'annotation_keys');
    const b = { ...annotations, events: source(c, 'events'), messages: source(c, 'messages'),
      files: source(c, 'files'), timeline: source(c, 'timeline', false) };
    validateReferences(b, c);
    if (b.timeline !== undefined) {
      requireThat(Array.isArray(b.timeline), 'timeline_shape');
      const ids = [...b.events.map(e => `e${e.seq}`), ...b.messages.map(m => m.id)];
      requireThat(isDeepStrictEqual([...ids].sort(), [...b.timeline].sort())
        && new Set(b.timeline).size === b.timeline.length, 'timeline_coverage');
      requireThat(b.events.every((e, i) => i === 0
        || b.timeline.indexOf(`e${e.seq}`) > b.timeline.indexOf(`e${b.events[i - 1].seq}`)), 'timeline_order');
    }
    // All original data remains available at this pointer, including unordered streams.
    // An empty bundle is the existing result/1 representation of unjudgeable evidence.
    const ordered = b.timeline !== undefined;
    target.status = 'unknown';
    target.reason = ordered ? 'Imported observations; semantic evaluation not performed.'
      : 'Preparation only: no explicit cross-stream order; source preserved in import_evidence.';
    target.bundle = ordered ? b : { events: [], messages: [], timeline: [], capture_complete: false };
    target.automatic = inspect(target.bundle);
    requireThat(target.automatic[0].status !== 'fail', 'invalid_references');
    target.semantic.forEach(s => { s.status = 'unknown'; s.rationale = 'Import does not perform semantic evaluation.'; });
    result.import_evidence.cases.push({ case_id: c.case_id, state: ordered ? 'imported' : 'preparation',
      source_pointer: `/import_evidence/source/cases/${index}` });
  }
  // Also validates explicit run metadata and synthetic labels; automatic failures describe
  // observed behavior and are not rewritten to pass by the importer.
  requireThat(checkResult(result, set).problems.length === 0, 'result_metadata_or_consistency');
  // Unordered records are outside bundle; enforce the same synthetic declaration there too.
  if (result.run.kind === 'synthetic') for (const c of input.cases) {
    requireThat([...c.events.data, ...c.messages.data].every(x => x.synthetic === true), 'synthetic_declaration');
  }
  return result;
}
