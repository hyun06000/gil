import { readFileSync, readdirSync, statSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { join } from 'node:path';

export const sha256 = (s) => createHash('sha256').update(s).digest('hex');

/** Parse a JSON file; returns {value} or {error}. Never throws on bad JSON. */
export function readJson(path) {
  let text;
  try { text = readFileSync(path, 'utf8'); } catch (err) { return { error: `cannot read: ${err.message}` }; }
  try { return { value: JSON.parse(text), text }; } catch (err) { return { error: `invalid JSON: ${err.message}`, text }; }
}

export function listJsonFiles(dir) {
  let names;
  try { names = readdirSync(dir); } catch (err) { return { error: `cannot list ${dir}: ${err.message}` }; }
  return { files: names.filter((n) => n.endsWith('.json')).sort().map((n) => join(dir, n)).filter((p) => { try { return statSync(p).isFile(); } catch { return false; } }) };
}

/** Deterministic digest of a case set: sorted file names and their bytes. */
export function caseSetDigest(files) {
  const h = createHash('sha256');
  for (const f of [...files].sort()) { h.update(f.split(/[\\/]/).pop()); h.update('\0'); h.update(readFileSync(f)); h.update('\0'); }
  return h.digest('hex');
}
