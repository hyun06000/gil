// The reviewed mapping (examples.json): which document excerpt is which example, what kind of
// example it is, and which reviewed case exercises it. This module checks the mapping against the
// current documents without running anything: drift, missing/ambiguous anchors, unmapped regions
// and cross-document consistency. A changed document is never silently accepted.
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { locate, inventory, readSource, LocatorError } from './sources.mjs';
import { parsePayload, getPath, PayloadError } from './extract.mjs';

/** Kinds a mapping may declare. Only the first three can ever be reported as `pass`. */
export const KINDS = {
  executable_template: 'complete tool input with declared placeholders; placeholders are replaced by values the temporary project issued',
  executable_fragment: 'exact field values (Report lines, contract lines, refs) inserted verbatim into a reviewed call',
  illustrative_partial: 'explanatory partial example; only the declared fields are extracted, the rest is harness-supplied and labelled',
  rule_statement: 'normative prose; the reviewed case derives the accepted and refused inputs it states',
  semantic_only: 'quality guidance Core does not judge; never executed as a pass (an observation may record what Core accepts)',
  not_executed: 'shell/CLI or out-of-scope block; the checker never executes document code',
};
export const RUNNABLE = new Set(['executable_template', 'executable_fragment', 'illustrative_partial', 'rule_statement']);

export class RegistryError extends Error {}

export function loadRegistry(path) {
  let reg;
  try { reg = JSON.parse(readFileSync(path, 'utf8')); } catch (err) { throw new RegistryError(`cannot read mapping ${path}: ${err.message}`); }
  validateRegistry(reg);
  return reg;
}

export function validateRegistry(reg) {
  const fail = (m) => { throw new RegistryError(m); };
  if (reg.format !== 1) fail('mapping format must be 1');
  if (!Array.isArray(reg.sources) || !Array.isArray(reg.examples)) fail('mapping needs sources[] and examples[]');
  const paths = new Set(reg.sources.map((s) => s.path));
  const ids = new Set();
  for (const ex of reg.examples) {
    if (!/^[A-Z][A-Z0-9-]+$/.test(ex.id ?? '')) fail(`bad example id ${JSON.stringify(ex.id)}`);
    if (ids.has(ex.id)) fail(`duplicate example id ${ex.id}`);
    ids.add(ex.id);
    if (!KINDS[ex.kind]) fail(`${ex.id}: unknown kind ${ex.kind}`);
    if (!paths.has(ex.source?.path)) fail(`${ex.id}: source ${ex.source?.path} is not a declared source`);
    if (typeof ex.source.locator !== 'object') fail(`${ex.id}: locator missing`);
    if (!/^[0-9a-f]{64}$/.test(ex.sha256 ?? '') && ex.sha256 !== 'unreviewed') fail(`${ex.id}: sha256 must be the reviewed 64-hex fingerprint (or "unreviewed" for a new entry)`);
    if (!Array.isArray(ex.cases)) fail(`${ex.id}: cases[] required (may be empty only for semantic_only/not_executed)`);
    if (RUNNABLE.has(ex.kind) && ex.cases.length === 0) fail(`${ex.id}: a ${ex.kind} example needs at least one reviewed case`);
    if (!RUNNABLE.has(ex.kind) && !ex.reason) fail(`${ex.id}: a ${ex.kind} example needs a reason`);
    if (typeof ex.claim !== 'string' || !ex.claim) fail(`${ex.id}: claim required`);
  }
  for (const c of reg.consistency ?? []) {
    for (const side of ['left', 'right']) {
      if (!ids.has(c[side]?.example)) fail(`consistency ${c.id}: unknown example ${c[side]?.example}`);
      if (!Array.isArray(c[side].path) || c[side].path.length === 0) fail(`consistency ${c.id}: ${side}.path must name the compared value`);
    }
    // Two documents that quote the same approved item must agree. A recorded exception would let
    // an example redefine an approved item, so the mapping cannot carry one.
    if ('acknowledged' in c) fail(`consistency ${c.id}: an acknowledged mismatch is not accepted; make the documents agree`);
  }
  return reg;
}

/**
 * Evaluate every example against the current documents. `readText(path)` lets tests substitute
 * documents; by default the repository files are read.
 */
export function evaluateSources(reg, { repo, readText = (p) => readSource(repo, p) } = {}) {
  const texts = new Map();
  const text = (p) => { if (!texts.has(p)) texts.set(p, readText(p)); return texts.get(p); };
  const examples = [];
  for (const ex of reg.examples) {
    const row = { id: ex.id, kind: ex.kind, path: ex.source.path, cases: ex.cases, claim: ex.claim, served: !!ex.served };
    try {
      const got = locate(text(ex.source.path), ex.source.locator);
      Object.assign(row, { lines: [got.startLine, got.endLine], sha256: got.sha256, excerpt: got.excerpt });
      if (got.sha256 !== ex.sha256) {
        row.source_status = 'source_changed';
        row.detail = ex.sha256 === 'unreviewed' ? `new mapping entry is not reviewed yet (document ${got.sha256}); review ${ex.source.path}:${got.startLine}-${got.endLine} and record the fingerprint in examples.json → ${ex.id}.sha256` : `excerpt fingerprint changed (mapping ${ex.sha256.slice(0, 12)}…, document ${got.sha256.slice(0, 12)}…). Re-review the excerpt at ${ex.source.path}:${got.startLine}-${got.endLine}, adjust the case(s) ${ex.cases.join(', ') || '(none)'} if the contract moved, then update examples.json → ${ex.id}.sha256`;
      } else {
        row.source_status = 'ok';
        try { row.payload = parsePayload(got.excerpt, ex.payload); } catch (err) {
          if (!(err instanceof PayloadError)) throw err;
          row.source_status = 'payload_invalid';
          row.detail = `${err.message}; update examples.json → ${ex.id}.payload`;
        }
      }
    } catch (err) {
      if (!(err instanceof LocatorError)) throw err;
      row.source_status = err.code === 'ambiguous' ? 'ambiguous' : 'missing';
      row.detail = `${err.message} in ${ex.source.path}; the example may have been removed, moved or duplicated. Update examples.json → ${ex.id}.source.locator after review`;
    }
    examples.push(row);
  }
  const unmapped = [];
  for (const src of reg.sources) {
    if (!src.inventory) continue;
    const covered = examples.filter((e) => e.path === src.path && e.lines);
    for (const region of inventory(text(src.path), src.inventory)) {
      const hit = covered.some((e) => e.lines[0] <= region.endLine && e.lines[1] >= region.startLine);
      if (!hit) unmapped.push({ path: src.path, ...region, detail: `example-shaped region ${region.kind} at ${src.path}:${region.startLine}-${region.endLine} has no mapping; add an examples.json entry (an executable case, or semantic_only/not_executed with a reason)` });
    }
  }
  const byId = new Map(examples.map((e) => [e.id, e]));
  const consistency = [];
  for (const c of reg.consistency ?? []) {
    const l = byId.get(c.left.example);
    const r = byId.get(c.right.example);
    if (!l.payload || !r.payload) { consistency.push({ id: c.id, status: 'not_checked', detail: 'an excerpt is not current; fix the source status first' }); continue; }
    const left = getPath(l.payload, c.left.path);
    const right = getPath(r.payload, c.right.path);
    const row = { id: c.id, left, right, note: c.note, examples: [c.left.example, c.right.example] };
    if (typeof left === 'string' && left !== '' && left === right) row.status = 'consistent';
    else { row.status = 'mismatch'; row.detail = `${c.left.example}${fmt(c.left.path)} ≠ ${c.right.example}${fmt(c.right.path)}; make both documents quote the same text (a mismatch cannot be acknowledged in examples.json)`; }
    consistency.push(row);
  }
  return { examples, unmapped, consistency };
}

const fmt = (path) => path.map((k) => (typeof k === 'number' ? `[${k}]` : `.${k}`)).join('');

/** True when nothing in the source evaluation blocks execution. */
export function sourcesClean(result) {
  return result.examples.every((e) => e.source_status === 'ok') && result.unmapped.length === 0 && result.consistency.every((c) => c.status === 'consistent');
}

export function registryPath(here) { return join(here, 'examples.json'); }
