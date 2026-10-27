// Core embeds the source directly; Plugin packages need a self-contained copy in SKILL.md.
import {readFile, writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {resolve} from 'node:path';
export const begin = '<!-- BEGIN GENERATED SHARED WORKFLOW -->';
export const end = '<!-- END GENERATED SHARED WORKFLOW -->';
export function render(skill, workflow) {
  const start = skill.indexOf(begin), stop = skill.indexOf(end);
  if (start < 0 || stop < start || skill.indexOf(begin, start + 1) !== -1 || skill.indexOf(end, stop + 1) !== -1)
    throw new Error('Expected exactly one shared workflow block');
  return skill.slice(0, start) + `${begin}\n${workflow}${end}` + skill.slice(stop + end.length);
}
export async function sync(check = true) {
  const path = new URL('./skills/gil-companion/SKILL.md', import.meta.url);
  const [skill, workflow] = await Promise.all([
    readFile(path, 'utf8'), readFile(new URL('../../src/mcp/workflow.md', import.meta.url), 'utf8'),
  ]);
  const next = render(skill, workflow);
  if (check && next !== skill) throw new Error('Shared workflow is stale: run node plugins/gil-companion-prototype/sync-workflow.mjs --write');
  if (!check && next !== skill) await writeFile(path, next);
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  if (process.argv.slice(2).some(arg => arg !== '--write')) throw new Error('Usage: sync-workflow.mjs [--write]');
  await sync(!process.argv.includes('--write'));
}
