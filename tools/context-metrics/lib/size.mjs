// Size measures for one string. Three units are always reported together and never summed
// across units: UTF-8 bytes (what a file or wire carries), Unicode code points (what this tool
// calls "문자 수"; a family emoji with ZWJ sequences counts several code points, not one glyph)
// and UTF-16 code units (JavaScript `string.length`). No tokenizer is involved anywhere, so no
// field here is a token count.
import { createHash } from 'node:crypto';

export const UNITS = ['utf8_bytes', 'code_points', 'utf16_units'];

export function measure(str) {
  if (typeof str !== 'string') return null;
  let codePoints = 0;
  for (const _ of str) codePoints += 1;
  return { utf8_bytes: Buffer.byteLength(str, 'utf8'), code_points: codePoints, utf16_units: str.length };
}

export function zero() { return { utf8_bytes: 0, code_points: 0, utf16_units: 0 }; }

export function add(a, b) {
  const out = zero();
  for (const u of UNITS) out[u] = (a?.[u] ?? 0) + (b?.[u] ?? 0);
  return out;
}

export function scale(a, n) {
  const out = zero();
  for (const u of UNITS) out[u] = (a?.[u] ?? 0) * n;
  return out;
}

/** JSON with object keys sorted, so the same value always serializes to the same bytes. */
export function stableStringify(value) {
  if (value === undefined) return undefined;
  if (value === null || typeof value !== 'object') return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map((v) => stableStringify(v) ?? 'null').join(',')}]`;
  const keys = Object.keys(value).filter((k) => value[k] !== undefined).sort();
  return `{${keys.map((k) => `${JSON.stringify(k)}:${stableStringify(value[k])}`).join(',')}}`;
}

export function sha256(str) { return createHash('sha256').update(str, 'utf8').digest('hex'); }

/** Short stable identifier for a block of text; the report carries this instead of the text. */
export function blockId(normalized) { return sha256(normalized).slice(0, 16); }
