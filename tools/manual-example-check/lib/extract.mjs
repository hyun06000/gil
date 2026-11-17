// Turn a located excerpt into the reviewed payload a case may use. Each parser is fixed code chosen
// by the mapping's `payload.parse`; excerpt text is data and is never evaluated or executed.
// A parser returns plain values only; anything it cannot read exactly is an error, never a guess.

export class PayloadError extends Error {}

const need = (cond, msg) => { if (!cond) throw new PayloadError(msg); };

/** Every string leaf with its path. */
function leaves(value, path = []) {
  if (typeof value === 'string') return [{ path, value }];
  if (Array.isArray(value)) return value.flatMap((v, i) => leaves(v, [...path, i]));
  if (value && typeof value === 'object') return Object.entries(value).flatMap(([k, v]) => leaves(v, [...path, k]));
  return [];
}

export function getPath(value, path) {
  return path.reduce((v, k) => (v === undefined || v === null ? undefined : v[k]), value);
}

function setPath(value, path, next) {
  const parent = getPath(value, path.slice(0, -1));
  need(parent && typeof parent === 'object', `placeholder path ${path.join('.')} has no parent`);
  parent[path.at(-1)] = next;
}

const PARSERS = {
  /** A complete JSON tool input. Placeholders are declared in the mapping, nothing else is replaced. */
  json(excerpt, spec) {
    let value;
    try { value = JSON.parse(excerpt); } catch (err) { throw new PayloadError(`fenced JSON does not parse: ${err.message}`); }
    for (const p of spec.placeholders ?? []) {
      need(getPath(value, p.path) === p.literal, `placeholder at ${p.path.join('.')} is ${JSON.stringify(getPath(value, p.path))}, mapping expects ${JSON.stringify(p.literal)}`);
    }
    return { input: value, placeholders: spec.placeholders ?? [], verbatim: spec.verbatim_content ?? [] };
  },
  /** `항목 X  <question> — done_when: <done_when>` lines of an illustrative proposal. */
  item_lines(excerpt, spec) {
    const items = [];
    for (const line of excerpt.split('\n')) {
      const m = /^\s*항목 ([A-Z][A-Za-z0-9_-]*)\s{2,}(.+?) — done_when: (.+?)\s*$/.exec(line);
      if (m) items.push({ id: m[1], question: m[2], done_when: m[3] });
    }
    need(items.length === spec.count, `found ${items.length} item lines, mapping expects ${spec.count}`);
    return { items };
  },
  /** `name: value` lines (Define fields, action contract fields). Values are taken verbatim. */
  field_lines(excerpt, spec) {
    const fields = {};
    for (const line of excerpt.split('\n')) {
      const m = /^\s*([a-z_]+): (.+?)\s*$/.exec(line);
      if (m && spec.fields.includes(m[1])) {
        need(fields[m[1]] === undefined, `field ${m[1]} appears twice`);
        fields[m[1]] = m[2];
      }
    }
    for (const f of spec.fields) need(fields[f] !== undefined, `field ${f} not found`);
    const out = { fields, text: spec.fields.map((f) => `${f}: ${fields[f]}`).join('\n') + '\n' };
    if (spec.label) {
      const m = new RegExp(spec.label.pattern).exec(excerpt);
      need(m, `label pattern ${spec.label.pattern} not found`);
      out.label = m[1];
    }
    return out;
  },
  /** Inline code spans such as `plan_document: plan/A.md` that are Report lines as written. */
  code_spans(excerpt, spec) {
    const spans = [...excerpt.matchAll(/`([^`]+)`/g)].map((m) => m[1]);
    const out = {};
    for (const [name, prefix] of Object.entries(spec.take)) {
      const hits = spans.filter((s) => s.startsWith(prefix));
      need(hits.length === 1, `expected one code span starting ${JSON.stringify(prefix)}, found ${hits.length}`);
      out[name] = hits[0];
    }
    return out;
  },
  /** Bracketed ID lists such as `[A,B,C]`, in document order. */
  id_lists(excerpt, spec) {
    const lists = [...excerpt.matchAll(/`\[([A-Za-z0-9_-]+(?:,[A-Za-z0-9_-]+)*)\]`/g)].map((m) => m[1].split(','));
    need(lists.length === spec.names.length, `found ${lists.length} ID lists, mapping names ${spec.names.length}`);
    return Object.fromEntries(spec.names.map((n, i) => [n, lists[i]]));
  },
  /** A JSON string literal shown inline, e.g. `"step:C5/S1\nstep:C5/S2"`. */
  json_string(excerpt, spec) {
    const m = new RegExp(spec.pattern).exec(excerpt);
    need(m, `pattern ${spec.pattern} not found`);
    let value;
    try { value = JSON.parse(m[1]); } catch (err) { throw new PayloadError(`inline JSON string does not parse: ${err.message}`); }
    need(typeof value === 'string', 'inline JSON is not a string');
    return { value };
  },
  /** First 「…」 label of the excerpt. */
  bracket_label(excerpt) {
    const m = /「(.+?)」/.exec(excerpt);
    need(m, 'no 「…」 label');
    return { label: m[1] };
  },
  none() { return {}; },
};

export function parsePayload(excerpt, spec = { parse: 'none' }) {
  const parser = PARSERS[spec.parse];
  if (!parser) throw new PayloadError(`unknown payload parser ${spec.parse}`);
  return parser(excerpt, spec);
}

/**
 * Fill a parsed JSON template. Only the mapping's declared placeholders are replaced, with values
 * the temporary project issued. Any remaining `<…>` string is an undeclared placeholder and fails:
 * running it would turn an explanation into a fake command.
 */
export function fillTemplate(payload, values) {
  const input = structuredClone(payload.input);
  for (const p of payload.placeholders) {
    need(Object.hasOwn(values, p.with), `no issued value for placeholder ${p.with}`);
    setPath(input, p.path, values[p.with]);
  }
  const left = leaves(input).filter((l) => /^<[^>]+>$/.test(l.value));
  need(left.length === 0, `undeclared placeholder(s): ${left.map((l) => l.path.join('.')).join(', ')}`);
  const kept = leaves(input).filter((l) => payload.verbatim.includes(l.value)).map((l) => l.path.join('.'));
  return { input, verbatimPaths: kept };
}
