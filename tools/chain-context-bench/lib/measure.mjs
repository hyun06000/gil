// Measurement helpers on top of tools/context-metrics. Sizes come from context-metrics'
// `measure` (UTF-8 bytes, code points, UTF-16 units); none of them is a token count.
import { createHash } from 'node:crypto';
import { readdirSync, readFileSync, lstatSync, readlinkSync } from 'node:fs';
import { join, relative, sep } from 'node:path';
import { measure, add, zero, sha256 } from '../../context-metrics/lib/size.mjs';
import { splitBlocks, normalizeBlock } from '../../context-metrics/lib/analyze.mjs';
import { blockId } from '../../context-metrics/lib/size.mjs';
import { LAYER_EXPECTATIONS } from './scenario.mjs';

// ── Read-only check ────────────────────────────────────────────────────────────────────────

/** Content digest of every entry below `dir` (files, symlinks, directories). Nothing is followed. */
export function treeDigest(dir) {
  const entries = [];
  const walk = (abs) => {
    for (const name of readdirSync(abs).sort()) {
      const p = join(abs, name);
      const rel = relative(dir, p).split(sep).join('/');
      const st = lstatSync(p);
      if (st.isSymbolicLink()) entries.push([rel, 'link', sha256(readlinkSync(p))]);
      else if (st.isDirectory()) { entries.push([rel, 'dir', '']); walk(p); }
      else if (st.isFile()) entries.push([rel, 'file', createHash('sha256').update(readFileSync(p)).digest('hex'), st.size]);
      else entries.push([rel, 'other', '']);
    }
  };
  walk(dir);
  const bytes = entries.reduce((n, e) => n + (e[3] ?? 0), 0);
  return { entries, files: entries.filter((e) => e[1] === 'file').length, bytes, digest: sha256(JSON.stringify(entries)) };
}

/** Relative names that differ between two digests (content, kind, added or removed). */
export function diffTrees(before, after) {
  const a = new Map(before.entries.map((e) => [e[0], JSON.stringify(e)]));
  const b = new Map(after.entries.map((e) => [e[0], JSON.stringify(e)]));
  const changed = [];
  for (const [k, v] of a) if (!b.has(k)) changed.push({ path: k, change: 'removed' }); else if (b.get(k) !== v) changed.push({ path: k, change: 'modified' });
  for (const k of b.keys()) if (!a.has(k)) changed.push({ path: k, change: 'added' });
  return changed.sort((x, y) => x.path.localeCompare(y.path));
}

// ── gil_context sections ───────────────────────────────────────────────────────────────────

export function snapshotReadRoots(paths) {
  return Object.fromEntries(Object.entries(paths).map(([key, path]) => [key, treeDigest(path)]));
}

export function compareReadRoots(before, after) {
  return Object.fromEntries(Object.keys(before).map(key => {
    const changes = diffTrees(before[key], after[key]);
    return [key, { unchanged: changes.length === 0, changes }];
  }));
}

export function readOnlyUnchanged(readonly) {
  const states = ['project', 'settings', 'record'].map(key => readonly?.[key]?.unchanged);
  return states.includes(false) ? false : states.every(x => x === true) ? true : null;
}

/** Locale keys that open a gil_context section, and how much of their first line is static. */
export const SECTION_KEYS = [
  ['preamble', 'ctx.preamble', 'line'],
  ['next_call', 'context.next_call_heading', 'line'],
  ['file_scope', 'ctx.file_scope', 'line'],
  ['plan', 'ctx.plan_profile', 'colon'],
  ['chain_current', 'ctx.chain_current', 'brace'],
  ['chain_previous', 'ctx.chain_previous', 'brace'],
  ['previous_cycles', 'ctx.previous_head', 'line'],
  ['failed_cycles', 'ctx.failed_head', 'line'],
  ['current_cycle', 'ctx.current_head', 'line'],
  ['here', 'ctx.here_head', 'line'],
  ['will', 'ctx.will_head', 'line'],
  ['next_moves', 'ctx.next_moves_head', 'line'],
  ['rules', 'ctx.rules_head', 'line'],
  ['report_principles', 'ctx.report_principles', 'line'],
  ['resume', 'resume.ready', 'colon'],
];

/** Static line prefixes from the Core's locale table (data, read from the repository). */
export function sectionMarkers(table) {
  const markers = [];
  const missing = [];
  for (const [name, key, cut] of SECTION_KEYS) {
    const value = table?.[key];
    if (typeof value !== 'string') { missing.push(key); continue; }
    let line = value.split('\n').find((l) => l.trim()) ?? '';
    if (cut === 'brace' || cut === 'line') line = line.split('{')[0];
    if (cut === 'colon') line = line.slice(0, line.indexOf(':') + 1 || line.length);
    line = line.trim();
    if (line) markers.push({ name, prefix: line }); else missing.push(key);
  }
  return { markers, missing };
}

/**
 * Split a context response into named sections. Each MCP text part starts as `text_part_N` (the
 * receipt guidance before the context body); a locale heading switches to its section name.
 * Repeated sections (one per previous Chain) add up.
 */
export function segmentContext(parts, markers) {
  const list = (Array.isArray(parts) ? parts : [parts]).filter((p) => typeof p === 'string');
  const sections = new Map();
  let current = null;
  let buf = [];
  const row = (name) => { if (!sections.has(name)) sections.set(name, { name, occurrences: 0, size: zero() }); return sections.get(name); };
  const flush = () => {
    if (current && buf.length) { const r = row(current); r.size = add(r.size, measure(buf.join('\n'))); }
    buf = [];
  };
  list.forEach((part, i) => {
    flush();
    current = `text_part_${i + 1}`;
    let headed = false;
    for (const line of part.split('\n')) {
      const hit = markers.find((m) => line.trim().startsWith(m.prefix));
      if (hit) { flush(); current = hit.name; row(current).occurrences += 1; headed = true; }
      buf.push(line);
    }
    if (!headed) row(current).occurrences += 1;
  });
  flush();
  return [...sections.values()].filter((s) => s.size.utf8_bytes > 0 || s.occurrences > 0);
}

/** Section sizes across cases: constant sections are fixed text, changing ones follow state. */
export function sectionVariability(sectionLists) {
  const names = new Map();
  const present = sectionLists.filter(Boolean);
  for (const list of present) for (const s of list) {
    const r = names.get(s.name) ?? { name: s.name, cases: 0, min: Infinity, max: -Infinity };
    r.cases += 1; r.min = Math.min(r.min, s.size.utf8_bytes); r.max = Math.max(r.max, s.size.utf8_bytes);
    names.set(s.name, r);
  }
  return [...names.values()].map((r) => ({ ...r, in_every_case: r.cases === present.length, constant: r.cases === present.length && r.min === r.max }));
}

/** Content identity without the per-run Monitor scope id (a hash of the temporary project path). */
export function normalizedSha(text) {
  return typeof text === 'string' ? sha256(text.replace(/project:[0-9a-f]{64}/g, 'project:<scope>')) : null;
}

// ── Resolution / coverage by tags ─────────────────────────────────────────────────────────

export function tagsIn(text, tags) {
  if (typeof text !== 'string') return [];
  return tags.filter((t) => text.includes(t.tag)).map((t) => t.tag);
}

/**
 * Compare where each generated tag arrived with the layer's expectation.
 * `reads` = [{tool, status, text, structured}] of one run.
 */
export function evaluateResolution(tags, reads) {
  const ctx = reads.find((r) => r.tool === 'gil_context');
  const rows = [];
  const violations = [];
  for (const [layer, spec] of Object.entries(LAYER_EXPECTATIONS)) {
    const group = tags.filter((t) => t.layer === layer);
    if (!group.length) continue;
    const perTool = {};
    for (const r of reads) {
      perTool[r.tool] = { text: tagsIn(r.text, group).length, structured: tagsIn(r.structured, group).length };
    }
    const inContext = ctx && ctx.status === 'ok' ? tagsIn(ctx.text, group) : null;
    const inBundle = new Set(reads.filter((r) => r.status === 'ok').flatMap((r) => [...tagsIn(r.text, group), ...tagsIn(r.structured, group)]));
    const row = { layer, expectation: spec.context, bundle_expectation: spec.bundle ?? null, why: spec.why, tags: group.length, in_context_text: inContext === null ? null : inContext.length, in_any_read: inBundle.size, per_tool: perTool, verdict: 'ok' };
    if (inContext === null) { row.verdict = 'not_measured'; violations.push({ layer, kind: 'context_not_measured' }); }
    else if (spec.context === 'present' && inContext.length !== group.length) {
      row.verdict = 'missing';
      violations.push({ layer, kind: 'missing_in_context', missing: group.map((t) => t.tag).filter((t) => !inContext.includes(t)) });
    } else if (spec.context === 'absent' && inContext.length) {
      row.verdict = 'expanded';
      violations.push({ layer, kind: 'expanded_in_context', found: inContext });
    }
    if (spec.bundle === 'present' && inBundle.size !== group.length) {
      row.verdict = row.verdict === 'ok' ? 'missing_in_bundle' : row.verdict;
      violations.push({ layer, kind: 'missing_in_reads', missing: group.map((t) => t.tag).filter((t) => !inBundle.has(t)) });
    }
    rows.push(row);
  }
  return { rows, violations };
}

// ── Repetition evidence ───────────────────────────────────────────────────────────────────

/** Replace generated tags, 64-hex digests and digits so per-entry scaffolding can be compared. */
export function templateOf(block) {
  return normalizeBlock(block).replace(/ccbT-[A-Za-z0-9-]+/g, '<tag>').replace(/\b[0-9a-f]{64}\b/g, '<hex>').replace(/\d+/g, '#');
}

/**
 * Blocks (context-metrics unit: blank-line paragraphs) of one response, with repeats inside the
 * same response: exact repeats and repeats of the same template (labels/scaffold per entry).
 */
export function blockProfile(parts, minCodePoints = 1) {
  const list = (Array.isArray(parts) ? parts : [parts]).filter((p) => typeof p === 'string');
  const blocks = splitBlocks(list).filter((b) => measure(b).code_points >= minCodePoints);
  const exact = new Map();
  const template = new Map();
  for (const b of blocks) {
    const id = blockId(b);
    const e = exact.get(id) ?? { id, size: measure(b), n: 0 }; e.n += 1; exact.set(id, e);
    const tid = blockId(templateOf(b));
    const t = template.get(tid) ?? { id: tid, n: 0, size: zero() }; t.n += 1; t.size = add(t.size, measure(b)); template.set(tid, t);
  }
  const exactRepeats = [...exact.values()].filter((e) => e.n > 1);
  const templateRepeats = [...template.values()].filter((t) => t.n > 1);
  return {
    blocks: blocks.length,
    block_ids: [...exact.keys()].sort(),
    block_sizes: Object.fromEntries([...exact.values()].map((e) => [e.id, e.size.utf8_bytes])),
    exact_repeats: { distinct: exactRepeats.length, extra_occurrences: exactRepeats.reduce((n, e) => n + e.n - 1, 0), extra_utf8_bytes: exactRepeats.reduce((n, e) => n + e.size.utf8_bytes * (e.n - 1), 0) },
    template_repeats: { distinct: templateRepeats.length, occurrences: templateRepeats.reduce((n, t) => n + t.n, 0), utf8_bytes: templateRepeats.reduce((n, t) => n + t.size.utf8_bytes, 0) },
  };
}

/**
 * Blocks shared by every level of one axis (same normalized paragraph at every scale) versus
 * blocks that appear or change with scale. Shared blocks are fixed guidance paid on each read;
 * changing blocks are state-dependent. Neither is judged as waste here.
 */
export function invariantAcross(profiles) {
  const present = profiles.filter(Boolean);
  if (present.length < 2) return null;
  const sets = present.map((p) => new Set(p.block_ids));
  const shared = [...sets[0]].filter((id) => sets.every((s) => s.has(id)));
  const base = present[0];
  const sharedBytes = shared.reduce((n, id) => n + (base.block_sizes[id] ?? 0), 0);
  return {
    levels: present.length,
    shared_blocks: shared.length,
    shared_utf8_bytes_at_first_level: sharedBytes,
    per_level: present.map((p) => {
      const total = Object.values(p.block_sizes).reduce((n, b) => n + b, 0);
      const sharedHere = shared.reduce((n, id) => n + (p.block_sizes[id] ?? 0), 0);
      return { blocks: p.blocks, distinct_block_utf8_bytes: total, shared_utf8_bytes: sharedHere, varying_utf8_bytes: total - sharedHere };
    }),
  };
}

/**
 * Line-level repeats across the read calls of one run. Paragraph blocks (context-metrics) miss a
 * sentence that is joined to different neighbours by single newlines; lines catch it. Lines are
 * normalized like blocks and kept only from `minCodePoints`. Report ids and sizes, never text.
 */
export function lineRepeats(reads, minCodePoints = 40) {
  const lines = new Map();
  for (const r of reads) {
    (r.parts ?? []).forEach((part, pi) => {
      for (const raw of part.split('\n')) {
        const line = normalizeBlock(raw);
        if (measure(line).code_points < minCodePoints) continue;
        const id = blockId(line);
        const row = lines.get(id) ?? { id, utf8_bytes: measure(line).utf8_bytes, places: [] };
        row.places.push(`${r.tool}#${pi + 1}`);
        lines.set(id, row);
      }
    });
  }
  const repeated = [...lines.values()].filter((l) => l.places.length > 1)
    .map((l) => ({ id: l.id, utf8_bytes: l.utf8_bytes, occurrences: l.places.length, places: l.places, across_tools: new Set(l.places.map((p) => p.split('#')[0])).size > 1, extra_utf8_bytes: l.utf8_bytes * (l.places.length - 1) }))
    .sort((a, b) => b.extra_utf8_bytes - a.extra_utf8_bytes || a.id.localeCompare(b.id));
  return {
    unit: 'normalized line', min_code_points: minCodePoints,
    repeated_lines: repeated.length,
    extra_utf8_bytes: repeated.reduce((n, l) => n + l.extra_utf8_bytes, 0),
    across_tools_extra_utf8_bytes: repeated.filter((l) => l.across_tools).reduce((n, l) => n + l.extra_utf8_bytes, 0),
    lines: repeated.slice(0, 10),
  };
}

// ── Reproducibility ───────────────────────────────────────────────────────────────────────

/** Per read tool: are sizes and redacted contents of repeated runs identical? */
export function compareRepeats(runs) {
  const usable = runs.filter((r) => r.status === 'complete');
  if (usable.length < 2) return { compared: usable.length, identical: null, rows: [] };
  const tools = [...new Set(usable.flatMap((r) => r.reads.map((x) => x.tool)))];
  const rows = tools.map((tool) => {
    const vals = usable.map((r) => r.reads.find((x) => x.tool === tool));
    const textBytes = vals.map((v) => v?.text_response?.utf8_bytes ?? null);
    const structBytes = vals.map((v) => v?.structured_response?.utf8_bytes ?? null);
    const textSha = vals.map((v) => v?.text_sha ?? null);
    const same = (xs) => xs.every((x) => x === xs[0]);
    return {
      tool,
      text_utf8_bytes: textBytes,
      structured_utf8_bytes: structBytes,
      text_size_identical: same(textBytes),
      structured_size_identical: same(structBytes),
      text_content_identical: same(textSha),
      max_text_delta_utf8_bytes: Math.max(...textBytes.map((b) => b ?? 0)) - Math.min(...textBytes.map((b) => b ?? 0)),
    };
  });
  return { compared: usable.length, identical: rows.every((r) => r.text_size_identical && r.structured_size_identical && r.text_content_identical), rows };
}

export { measure, sha256 };
