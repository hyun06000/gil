// Offline, self-contained evidence preparation. No I/O, path following or semantic judging.
import { isDeepStrictEqual } from 'node:util';
import { sha256 } from './io.mjs';
import { extractOutputs } from '../make-skeleton.mjs';
import { CHOICE_ROLES } from './vocab.mjs';

export const IMPORT_SCHEMA = 'gil.chain-interview.import/1';
export const CONVERSATION_SCHEMA = 'gil.chain-interview.conversation/1';
export const EVIDENCE_SCHEMA = 'gil.chain-interview.prepared-evidence/1';
const object = (v) => v !== null && typeof v === 'object' && !Array.isArray(v);
const nonBlank = (v) => typeof v === 'string' && v.trim().length > 0;
// Error codes are static: never interpolate supplied text, identifiers or paths.
function need(condition, code) { if (!condition) throw new Error(code); }
function shape(v, required, optional = []) {
  need(object(v) && required.every((k) => Object.hasOwn(v, k))
    && Object.keys(v).every((k) => [...required, ...optional].includes(k)), 'invalid_shape');
}
function scope(v, runId, caseId, required = true) {
  need((!required && !Object.hasOwn(v, 'run_id') || v.run_id === runId)
    && (!required && !Object.hasOwn(v, 'case_id') || v.case_id === caseId), 'scope_mismatch');
}
function parse(text) {
  try { return JSON.parse(text); } catch { throw new Error('invalid_json'); }
}
function digest(v) { return typeof v === 'string' && /^[a-f0-9]{64}$/.test(v); }

/** Rebuild projections from embedded originals; also used by the existing result checker. */
export function projectEvidence(sources, runId, caseId, priorPlan = null) {
  shape(sources, ['manifest', 'events', 'conversation']);
  const ids = new Set();
  const register = (id) => {
    need(nonBlank(id) && !ids.has(id), 'duplicate_or_blank_id');
    ids.add(id);
  };
  for (const source of Object.values(sources).filter((s) => s !== null)) {
    shape(source, ['id', 'ref', 'sha256', 'text']);
    register(source.id);
    need(nonBlank(source.ref) && typeof source.text === 'string', 'invalid_source');
    need(digest(source.sha256) && sha256(source.text) === source.sha256, 'source_hash_mismatch');
  }
  need(sources.manifest && sources.events, 'missing_source');
  const manifest = parse(sources.manifest.text);
  shape(manifest, ['schema', 'id', 'ref', 'run_id', 'case_id', 'events', 'conversation', 'questions', 'claims', 'links']);
  need(manifest.schema === IMPORT_SCHEMA && manifest.id === sources.manifest.id && manifest.ref === sources.manifest.ref, 'manifest_mismatch');
  scope(manifest, runId, caseId);
  for (const key of ['events', 'conversation']) {
    const pin = manifest[key]; const source = sources[key];
    if (pin === null) { need(source === null && key === 'conversation', 'missing_source'); continue; }
    shape(pin, ['id', 'ref', 'sha256']);
    need(source && pin.id === source.id && pin.ref === source.ref && pin.sha256 === source.sha256, 'source_pin_mismatch');
  }
  for (const key of ['questions', 'claims', 'links']) need(Array.isArray(manifest[key]), 'invalid_annotations');

  const messages = new Map();
  if (sources.conversation) {
    const conversation = parse(sources.conversation.text);
    shape(conversation, ['schema', 'run_id', 'case_id', 'messages']);
    need(conversation.schema === CONVERSATION_SCHEMA && Array.isArray(conversation.messages), 'invalid_conversation');
    scope(conversation, runId, caseId);
    for (const m of conversation.messages) {
      shape(m, ['id', 'role', 'text', 'source'], ['run_id', 'case_id']);
      register(m.id); scope(m, runId, caseId, false);
      need(['assistant', 'user'].includes(m.role) && nonBlank(m.text), 'invalid_message');
      shape(m.source, ['ref', 'origin']);
      need(nonBlank(m.source.ref) && ['synthetic', 'human', 'model', 'unknown'].includes(m.source.origin), 'invalid_message_source');
      messages.set(m.id, { ...m, sha256: sha256(m.text) });
    }
  }
  const cite = (c, role) => {
    shape(c, ['message_id', 'start', 'end', 'quote']);
    const m = messages.get(c.message_id);
    need(m && (!role || m.role === role), 'broken_message_ref');
    need(Number.isSafeInteger(c.start) && Number.isSafeInteger(c.end) && c.start >= 0 && c.end > c.start
      && c.end <= m.text.length && m.text.slice(c.start, c.end) === c.quote, 'quote_mismatch');
    return c.quote;
  };

  const events = new Map(); const eventIndex = []; let lastSeq = 0;
  for (const [i, line] of sources.events.text.split('\n').entries()) {
    if (!line.trim()) continue;
    const e = parse(line);
    need(object(e) && object(e.request) && nonBlank(e.request.name), 'invalid_event');
    scope(e, runId, caseId, false);
    need(Number.isSafeInteger(e.seq) && e.seq > lastSeq, 'duplicate_or_unordered_event_id');
    lastSeq = e.seq;
    // Existing proxy/wire records may also carry a provider call id. Preserve and check it.
    if (Object.hasOwn(e, 'id')) register(e.id);
    if (Object.hasOwn(e, 'call_id')) register(e.call_id);
    need(e.synthetic === undefined || typeof e.synthetic === 'boolean', 'invalid_event');
    const successful = Boolean(e.result && !e.error && e.result.isError !== true && e.result.structuredContent?.ok !== false);
    events.set(e.seq, e);
    eventIndex.push({ seq: e.seq, line: i + 1, sha256: sha256(line), successful });
  }
  const eventQuote = (c) => {
    shape(c, ['seq', 'pointer', 'start', 'end', 'quote']);
    const e = events.get(c.seq);
    need(e && typeof c.pointer === 'string' && c.pointer.startsWith('/'), 'broken_event_ref');
    let value = e;
    for (const raw of c.pointer.slice(1).split('/')) {
      need(!/~(?![01])/.test(raw), 'broken_event_pointer');
      const key = raw.replace(/~1/g, '/').replace(/~0/g, '~');
      need(value !== null && typeof value === 'object' && Object.hasOwn(value, key), 'broken_event_pointer');
      value = value[key];
    }
    need(typeof value === 'string' && Number.isSafeInteger(c.start) && Number.isSafeInteger(c.end)
      && c.start >= 0 && c.end > c.start && c.end <= value.length && value.slice(c.start, c.end) === c.quote, 'event_quote_mismatch');
    return c.quote;
  };

  const extracted = extractOutputs(sources.events.text);
  const missing = [];
  if (!eventIndex.length) missing.push('tool_events');
  if (!sources.conversation || messages.size === 0) missing.push('visible_conversation');
  if (!manifest.questions.length) missing.push('question_annotations');
  if (!manifest.links.length) missing.push('message_event_links');
  const questions = manifest.questions.map((q) => {
    shape(q, ['id', 'cite', 'choices', 'multi_select', 'response']); register(q.id);
    const question = cite(q.cite, 'assistant');
    need(Array.isArray(q.choices), 'invalid_choices');
    const choices = q.choices.map((c) => {
      shape(c, ['cite', 'role']);
      need(c.cite.message_id === q.cite.message_id && CHOICE_ROLES.includes(c.role), 'invalid_choice_annotation');
      return { label: cite(c.cite, 'assistant'), role: c.role };
    });
    if (q.multi_select !== null) {
      shape(q.multi_select, ['value', 'cite']);
      need(typeof q.multi_select.value === 'boolean' && q.multi_select.cite.message_id === q.cite.message_id, 'invalid_multi_select');
      cite(q.multi_select.cite, 'assistant');
    } else missing.push('multi_select_annotation');
    if (q.response !== null) cite(q.response, 'user');
    else missing.push('question_response');
    return { question, choices, multi_select: q.multi_select?.value ?? null,
      response: q.response === null ? null : q.response.quote, annotation_id: q.id };
  });
  const claims = [];
  const knownItems = new Set([...(extracted.outputs.proposal?.items ?? []), ...(priorPlan?.items ?? [])].map((it) => it.id));
  for (const c of manifest.claims) {
    shape(c, ['id', 'cite', 'items']); register(c.id); cite(c.cite, 'assistant');
    need(Array.isArray(c.items) && c.items.length > 0, 'invalid_claim');
    const seen = new Set();
    for (const item of c.items) {
      need(nonBlank(item) && !seen.has(item), 'duplicate_or_blank_item'); seen.add(item);
      need(knownItems.has(item), 'broken_item_ref');
      // IDs are an explicit annotator mapping, never a claim of an execution/approval link.
      if (!claims.includes(item)) claims.push(item);
    }
  }
  for (const link of manifest.links) {
    shape(link, ['id', 'message', 'event']); register(link.id);
    need(cite(link.message) === eventQuote(link.event), 'link_quote_mismatch');
  }
  return {
    purpose: 'preparation_only',
    visible_messages: [...messages.values()], event_index: eventIndex,
    outputs: { ...extracted.outputs, questions, claimed_completed_items: claims },
    tool_question_pointers: extracted.outputs.questions.map((q) => q._pointer),
    tool_log_summary: extracted.summary,
    // The absence of annotations never means no questions/claims occurred.
    missing: [...new Set(missing)],
    annotation_coverage: 'unspecified', approval: { kind: 'unknown', evidence: '' },
  };
}

export function prepareEvidence(sources, runId, caseId, priorPlan = null) {
  return { schema: EVIDENCE_SCHEMA, run_id: runId, case_id: caseId, sources,
    projection: projectEvidence(sources, runId, caseId, priorPlan) };
}

export function validatePreparedEvidence(evidence, runId, caseId, priorPlan = null) {
  shape(evidence, ['schema', 'run_id', 'case_id', 'sources', 'projection']);
  need(evidence.schema === EVIDENCE_SCHEMA, 'invalid_evidence_schema');
  scope(evidence, runId, caseId);
  need(isDeepStrictEqual(evidence.projection, projectEvidence(evidence.sources, runId, caseId, priorPlan)), 'projection_mismatch');
}
