// Locate reviewed example excerpts in the repository's documents, fingerprint them and inventory
// the example-shaped regions (fenced blocks, Manual example sections) that no mapping covers yet.
// Read-only: this module never writes, never executes a code block and never follows a link.
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

export const sha256 = (text) => createHash('sha256').update(text, 'utf8').digest('hex');

export class LocatorError extends Error {
  constructor(code, message) { super(message); this.code = code; }
}

/** 1-based line number of a character offset. */
function lineAt(text, offset) {
  let line = 1;
  for (let i = 0; i < offset && i < text.length; i += 1) if (text.charCodeAt(i) === 10) line += 1;
  return line;
}

function uniqueIndex(text, needle, from, label) {
  const at = text.indexOf(needle, from);
  if (at < 0) throw new LocatorError('missing', `${label} ${JSON.stringify(needle)} not found`);
  if (text.indexOf(needle, at + 1) >= 0) throw new LocatorError('ambiguous', `${label} ${JSON.stringify(needle)} occurs more than once`);
  return at;
}

/** Bounds of the text the locator searches in: after `after` (exclusive) and before `before`. */
function window(text, locator) {
  let start = 0;
  let end = text.length;
  if (locator.after !== undefined) start = uniqueIndex(text, locator.after, 0, 'anchor `after`') + locator.after.length;
  if (locator.before !== undefined) {
    const at = text.indexOf(locator.before, start);
    if (at < 0) throw new LocatorError('missing', `anchor \`before\` ${JSON.stringify(locator.before)} not found after the start anchor`);
    end = at;
  }
  return { start, end };
}

function onlyOnce(body, needle, label) {
  const at = body.indexOf(needle);
  if (at < 0) throw new LocatorError('missing', `${label} ${JSON.stringify(needle)} not found`);
  if (body.indexOf(needle, at + 1) >= 0) throw new LocatorError('ambiguous', `${label} ${JSON.stringify(needle)} occurs more than once in its window`);
  return at;
}

/**
 * Extract one excerpt. Locator forms (all exact, case-sensitive, never regular expressions):
 *   {fence: "json", after?, before?}   the only ```json fenced block in the window (body only)
 *   {section: "[올바른 예]"}            a Manual body section up to the next `[...]` header line
 *   {from, to, after?, before?}         the substring from `from` through `to`, both unique in the window
 *   {line: "...", after?, before?}      the whole line containing the unique substring
 */
export function locate(text, locator) {
  const { start, end } = window(text, locator);
  const body = text.slice(start, end);
  let a;
  let b;
  if (locator.fence !== undefined) {
    const open = new RegExp(`^[ \\t]*\`\`\`${locator.fence}[ \\t]*$`, 'gm');
    const hits = [...body.matchAll(open)];
    if (hits.length === 0) throw new LocatorError('missing', `no \`\`\`${locator.fence} fenced block in the window`);
    if (hits.length > 1) throw new LocatorError('ambiguous', `${hits.length} \`\`\`${locator.fence} fenced blocks in the window`);
    a = hits[0].index + hits[0][0].length + 1;
    const close = /^[ \t]*```[ \t]*$/gm;
    close.lastIndex = a;
    const done = close.exec(body);
    if (!done) throw new LocatorError('missing', 'fenced block is not closed');
    b = done.index;
  } else if (locator.section !== undefined) {
    const header = new RegExp(`^${locator.section.replace(/[[\]\\^$.|?*+(){}]/g, '\\$&')}[ \\t]*$`, 'gm');
    const hits = [...body.matchAll(header)];
    if (hits.length === 0) throw new LocatorError('missing', `section ${locator.section} not found`);
    if (hits.length > 1) throw new LocatorError('ambiguous', `section ${locator.section} occurs ${hits.length} times`);
    a = hits[0].index;
    const next = /^\[[^\]\n]+\][ \t]*$/gm;
    next.lastIndex = a + hits[0][0].length;
    const n = next.exec(body);
    b = n ? n.index : body.length;
  } else if (locator.from !== undefined) {
    a = onlyOnce(body, locator.from, 'start text `from`');
    const tail = body.slice(a);
    const t = tail.indexOf(locator.to);
    if (t < 0) throw new LocatorError('missing', `end text \`to\` ${JSON.stringify(locator.to)} not found after \`from\``);
    b = a + t + locator.to.length;
  } else if (locator.line !== undefined) {
    const at = onlyOnce(body, locator.line, 'line text');
    a = body.lastIndexOf('\n', at) + 1;
    const nl = body.indexOf('\n', at);
    b = nl < 0 ? body.length : nl;
  } else {
    throw new LocatorError('invalid', `unknown locator ${JSON.stringify(locator)}`);
  }
  const excerpt = body.slice(a, b);
  return { excerpt, startLine: lineAt(text, start + a), endLine: lineAt(text, start + Math.max(a, b - 1)), sha256: sha256(excerpt) };
}

/** Example-shaped regions: fenced blocks of any language, and Manual example sections. */
export function inventory(text, { sections = [] } = {}) {
  const regions = [];
  const lines = text.split('\n');
  let fence = null;
  for (let i = 0; i < lines.length; i += 1) {
    const m = /^[ \t]*```(\S*)[ \t]*$/.exec(lines[i]);
    if (!m) continue;
    if (!fence) fence = { start: i + 1, lang: m[1] || 'plain' };
    else { regions.push({ kind: `fence:${fence.lang}`, startLine: fence.start, endLine: i + 1 }); fence = null; }
  }
  for (const name of sections) {
    const at = lines.findIndex((l) => l.trimEnd() === name);
    if (at < 0) continue;
    let end = lines.length;
    for (let j = at + 1; j < lines.length; j += 1) if (/^\[[^\]]+\]\s*$/.test(lines[j])) { end = j; break; }
    regions.push({ kind: `section:${name}`, startLine: at + 1, endLine: end });
  }
  return regions;
}

export function readSource(repo, path) {
  return readFileSync(join(repo, path), 'utf8');
}
