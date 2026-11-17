// Combine source evaluation and case results into per-example statuses and a readable report.
// Report text is data for people; nothing in it is an instruction for the checker.
import { RUNNABLE } from './registry.mjs';

/**
 * Per-example status. Only a runnable kind with a current excerpt, a passing served-text check and
 * all its reviewed cases passing is `pass`. semantic_only / not_executed are reported as such and
 * never counted as pass; a changed, missing or unmapped document never becomes a pass.
 */
export function exampleStatuses(sources, caseResults) {
  const byCase = new Map(caseResults.map((c) => [c.id, c]));
  const served = byCase.get('CX-00');
  return sources.examples.map((e) => {
    const row = { id: e.id, kind: e.kind, path: e.path, lines: e.lines ?? null, sha256: e.sha256 ?? null, cases: e.cases, claim: e.claim };
    if (e.source_status !== 'ok') return { ...row, status: e.source_status, detail: e.detail };
    const servedFail = served?.status === 'fail' && served.failed_check === `served ${e.id}`;
    if (!RUNNABLE.has(e.kind)) {
      const obs = e.cases.map((id) => byCase.get(id)).filter(Boolean).map((c) => `${c.id}:${c.status}`);
      return { ...row, status: e.kind, detail: obs.length ? `observation ${obs.join(', ')}` : 'not executed' };
    }
    const linked = e.cases.filter((id) => id !== 'CX-00').map((id) => byCase.get(id) ?? { id, status: 'not_run' });
    if (servedFail) return { ...row, status: 'fail', detail: served.detail };
    if (linked.some((c) => c.status === 'fail')) return { ...row, status: 'fail', detail: linked.filter((c) => c.status === 'fail').map((c) => `${c.id}: ${c.detail}`).join(' | ') };
    if (linked.length && linked.every((c) => c.status === 'pass') && (!served || served.status === 'pass' || !e.served)) return { ...row, status: 'pass' };
    return { ...row, status: 'not_run', detail: linked.filter((c) => c.status !== 'pass').map((c) => `${c.id}:${c.status}`).join(', ') };
  });
}

export function verdict({ examples, unmapped, consistency, cases, isolation }) {
  const problems = [];
  for (const e of examples) if (!['pass', 'semantic_only', 'not_executed'].includes(e.status)) problems.push(`${e.id}: ${e.status}`);
  for (const u of unmapped) problems.push(`unmapped ${u.path}:${u.startLine}-${u.endLine}`);
  for (const c of consistency) if (!['consistent', 'known_mismatch'].includes(c.status)) problems.push(`consistency ${c.id}: ${c.status}`);
  for (const c of cases) if (c.status !== 'pass') problems.push(`${c.id}: ${c.status}`);
  for (const i of isolation ?? []) if (!i.ok) problems.push(`isolation: ${i.check}`);
  return { ok: problems.length === 0, problems };
}

const cell = (s) => String(s ?? '').replace(/\|/g, '\\|').replace(/\n/g, ' ');
const table = (head, rows) => [`| ${head.join(' | ')} |`, `| ${head.map(() => '---').join(' | ')} |`, ...rows.map((r) => `| ${r.map(cell).join(' | ')} |`)].join('\n');

export function markdown(report) {
  const out = [];
  out.push('# Manual·workflow Chain example contract check', '');
  out.push(`- mode: ${report.mode}${report.binary ? ` · binary sha256 ${report.binary.sha256.slice(0, 16)}… · features ${report.binary.features}` : ''}`);
  out.push(`- source HEAD: ${report.head} · documents: ${report.docs_root}${report.dirty_sources ? ' (watched documents have local changes)' : report.dirty_sources === null ? ' (not compared with HEAD)' : ''}`);
  out.push(`- verdict: **${report.verdict.ok ? 'ok' : 'problems'}**${report.verdict.problems.length ? ` — ${report.verdict.problems.join('; ')}` : ''}`);
  out.push('', '## Scope of the claims', '');
  out.push(table(['claim', 'status'], Object.entries(report.claims)));
  out.push('', '## Examples', '');
  out.push(table(['id', 'kind', 'source', 'cases', 'status', 'detail'], report.examples.map((e) => [e.id, e.kind, e.lines ? `${e.path}:${e.lines[0]}-${e.lines[1]}` : e.path, e.cases.join(', '), e.status, e.detail ?? ''])));
  if (report.unmapped.length) out.push('', '## Unmapped example regions', '', table(['source', 'kind', 'detail'], report.unmapped.map((u) => [`${u.path}:${u.startLine}-${u.endLine}`, u.kind, u.detail])));
  out.push('', '## Document consistency', '');
  out.push(table(['id', 'status', 'left', 'right', 'note'], report.consistency.map((c) => [c.id, c.status, c.left ?? '', c.right ?? '', c.note ?? c.detail ?? ''])));
  if (report.cases.length) {
    out.push('', '## Cases', '');
    for (const c of report.cases) {
      out.push(`### ${c.id} — ${c.title}`, '', `status: **${c.status}**${c.observation ? ' (observation, never a pass for its example)' : ''}${c.detail ? ` — ${c.detail}` : ''}`, '');
      if (c.checks.length) out.push(table(['check', 'status', 'detail'], c.checks.map((k) => [k.check, k.status, k.detail])), '');
    }
  }
  if (report.isolation?.length) out.push('## Isolation', '', table(['check', 'ok', 'detail'], report.isolation.map((i) => [i.check, i.ok ? 'yes' : 'NO', i.detail ?? ''])), '');
  return `${out.join('\n')}\n`;
}
