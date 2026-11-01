// Redaction for saved requests/responses. Tokens, absolute paths and oversized private
// content never reach the committed JSON/Markdown results.
import { tmpdir, homedir } from 'node:os';

const TOKEN_KEYS = new Set(['token', 'gil/planApprovalToken']);
const WINDOWS_ABS = /[A-Za-z]:\\[^\s"'`<>|]+/g;

export function makeRedactor({ roots = {}, secrets = [], maxString = 4000 } = {}) {
  // Longest roots first so a nested directory is labelled by its most specific name.
  const labelled = Object.entries(roots).filter(([, p]) => typeof p === 'string' && p.length > 1).sort((a, b) => b[1].length - a[1].length);
  const secretSet = new Set(secrets.filter((s) => typeof s === 'string' && s.length >= 8));
  const generic = [tmpdir(), homedir()].filter(Boolean).sort((a, b) => b.length - a.length);
  function text(s) {
    let out = s;
    for (const secret of secretSet) out = out.split(secret).join('<redacted-token>');
    for (const [label, path] of labelled) out = out.split(path).join(`<${label}>`);
    for (const path of generic) out = out.split(path).join('<abs-path>');
    out = out.replace(/\/(?:Users|home)\/[^\s"'`<>|:]+/g, '<abs-path>').replace(WINDOWS_ABS, '<abs-path>');
    if (out.length > maxString) out = `${out.slice(0, maxString)}…<truncated ${out.length - maxString} chars>`;
    return out;
  }
  function redact(value, key = '') {
    if (typeof value === 'string') return TOKEN_KEYS.has(key) ? '<redacted-token>' : text(value);
    if (Array.isArray(value)) return value.map((v) => redact(v));
    if (value && typeof value === 'object') {
      const out = {};
      for (const [k, v] of Object.entries(value)) out[k] = TOKEN_KEYS.has(k) ? (v === undefined || v === null ? v : '<redacted-token>') : redact(v, k);
      return out;
    }
    return value;
  }
  return { redact, text, addSecret: (s) => { if (typeof s === 'string' && s.length >= 8) secretSet.add(s); } };
}

/** True when a serialized value still carries any of the given secrets or labelled roots. */
export function leaks(serialized, { secrets = [], roots = [] } = {}) {
  return [...secrets, ...roots].filter((s) => typeof s === 'string' && s.length > 1 && serialized.includes(s));
}
