import { readFileSync, readdirSync, existsSync } from 'node:fs';
import { resolve, join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { isDeepStrictEqual } from 'node:util';
import { caseSetDigest } from '../../chain-interview-eval/lib/io.mjs';
import { inspect, CHECKS, SEMANTIC, MODES, nonblank } from './evidence.mjs';

export const REPO = resolve(dirname(fileURLToPath(import.meta.url)), '../../..');
export const DEFAULT_CASES = join(REPO, 'scenarios/chain-closing');
export const json = file => JSON.parse(readFileSync(file, 'utf8'));
const same = isDeepStrictEqual;
const sorted = a => [...a].sort();
const list = x => Array.isArray(x) ? x : [];

export function loadCases(root = DEFAULT_CASES) {
  const rubric = json(join(root, 'rubric.json'));
  const files = readdirSync(join(root, 'cases')).filter(f => f.endsWith('.json')).sort();
  const cases = files.map(f => json(join(root, 'cases', f)));
  const problems = [];
  const issue = (where, detail) => problems.push({ where, detail });
  if (rubric.schema !== 'gil.chain-closing.rubric/1'
    || !same(rubric.automatic.map(x => x.id), CHECKS)
    || !same(rubric.semantic.map(x => x.id), SEMANTIC)
    || !same(rubric.failure_modes, MODES)) issue('rubric', 'Vocabulary mismatch.');
  if (cases.length !== 10 || new Set(cases.map(c => c.id)).size !== 10) issue('cases', 'Exactly ten distinct required cases expected.');
  cases.forEach((c, i) => {
    const where = files[i];
    if (c.schema !== 'gil.chain-closing.case/1' || c.id !== `CC-${String(i + 1).padStart(2, '0')}`
      || files[i] !== `${c.id}.json` || !nonblank(c.title) || !nonblank(c.input?.request)
      || !nonblank(c.input?.setup) || !nonblank(c.judgement?.basis)
      || !list(c.judgement?.unknown_when).length || c.judgement.unknown_when.some(x => !nonblank(x))) issue(where, 'Case schema/setup/judgement incomplete.');
    if (!list(c.spec_refs).length || c.spec_refs.some(f => !/^spec\/[^/]+\.md$/.test(f) || !existsSync(join(REPO, f)))) issue(where, 'Invalid spec file reference.');
    for (const kind of ['good', 'bad']) {
      if (!list(c[kind]).length) issue(where, `Missing ${kind} examples.`);
      const seen = new Set();
      for (const ex of list(c[kind])) {
        if (!nonblank(ex.id) || seen.has(ex.id) || !nonblank(ex.reason) || !nonblank(ex.acceptable_variants)) issue(where, 'Example id/rationale/variants missing or duplicate.');
        seen.add(ex.id);
        const actual = inspect(ex.bundle);
        const failures = actual.filter(x => x.status === 'fail').map(x => x.check);
        if (!same(sorted(failures), sorted(list(ex.expected_failures)))) issue(`${where}/${ex.id}`, `Declared automatic failures differ: ${failures.join(',')}.`);
        if (kind === 'good' && failures.length) issue(`${where}/${ex.id}`, 'Good example has an automatic failure.');
        if (kind === 'bad' && (!list(ex.failure_modes).length || ex.failure_modes.some(x => !MODES.includes(x)))) issue(where, 'Unknown/missing failure mode.');
        if (ex.bundle?.events?.some(e => e.synthetic !== true) || ex.bundle?.messages?.some(m => m.synthetic !== true)) issue(where, 'Fixtures must explicitly be synthetic.');
      }
    }
  });
  return { cases, rubric, problems, digest: caseSetDigest([join(root, 'rubric.json'), ...files.map(f => join(root, 'cases', f))]) };
}

export function skeleton(set) {
  return {
    schema: 'gil.chain-closing.result/1',
    run: { id: 'not-run-example', example: true, kind: 'synthetic', model_executed: false, host_executed: false,
      recorded_at: null, model: null, host: null, baseline_commit: 'bcefbc42ab6909c9e5c59587d79aa37f4e9fbb10',
      guidance_sha256: null, profile: 'approved-plan,visual-authoring,product-safety', case_set_sha256: set.digest },
    judges: [],
    cases: set.cases.map(c => ({ case_id: c.id, status: 'not_run', reason: 'No model or Host evaluation performed.',
      bundle: null, automatic: [], semantic: SEMANTIC.map(criterion => ({ criterion, status: 'not_run', rationale: 'Not evaluated.', basis: [], judge: null })) })),
  };
}

export function checkResult(r, set) {
  const problems = [];
  const add = (where, detail) => problems.push({ where, detail });
  const run = r?.run ?? {};
  if (r?.schema !== 'gil.chain-closing.result/1' || !nonblank(run.id) || typeof run.example !== 'boolean'
    || !['synthetic', 'model'].includes(run.kind) || typeof run.model_executed !== 'boolean'
    || typeof run.host_executed !== 'boolean' || run.case_set_sha256 !== set.digest
    || !/^[a-f0-9]{40}$/.test(run.baseline_commit ?? '') || !nonblank(run.profile)) add('run', 'Invalid metadata or case-set digest.');
  if (run.kind === 'synthetic' && (run.model_executed || run.host_executed || run.model || run.host)) add('run', 'Synthetic evidence is not a model/Host run.');
  if (run.kind === 'model' && (!run.model_executed || !nonblank(run.model?.name) || !nonblank(run.model?.version)
    || !/^[a-f0-9]{64}$/.test(run.guidance_sha256 ?? '') || !nonblank(run.recorded_at))) add('run', 'Real model run needs name/version/time/guidance hash.');
  if (run.host_executed && (!nonblank(run.host?.name) || !nonblank(run.host?.version))) add('run', 'Host run needs name/version.');
  const judges = new Map();
  for (const j of list(r?.judges)) {
    if (!nonblank(j?.id) || judges.has(j.id) || !['human', 'model'].includes(j.kind)
      || (j.kind === 'model' && (!nonblank(j.model?.name) || !nonblank(j.model?.version)))) add('judges', 'Invalid/duplicate judge.');
    judges.set(j?.id, j);
  }
  const records = list(r?.cases);
  if (!same(sorted(records.map(c => c?.case_id)), sorted(set.cases.map(c => c.id)))) add('cases', 'Every case must occur exactly once.');
  const counts = { pass: 0, fail: 0, unknown: 0, not_run: 0 };
  for (const c of records) {
    const at = c?.case_id ?? '?';
    if (!['not_run', 'unknown', 'evaluated'].includes(c?.status) || !nonblank(c.reason)) add(at, 'Invalid status or missing reason.');
    if (!Array.isArray(c.automatic) || !Array.isArray(c.semantic)) add(at, 'Automatic/semantic arrays required.');
    if (c.status === 'not_run' && (c.bundle !== null || list(c.automatic).length)) add(at, 'not_run cannot carry observations or checks.');
    if (c.status !== 'not_run' && !c.bundle) add(at, 'Observed case needs an evidence bundle (empty arrays allowed for unknown).');
    if (c.bundle) {
      const actual = inspect(c.bundle);
      if (!same(actual, c.automatic)) add(at, 'Automatic checks must exactly match recomputation.');
      // Malformed references are a result-format error, unlike a valid observed behavior failure.
      if (actual.find(x => x.check === 'references')?.status === 'fail') add(at, 'Evidence references are malformed.');
      if (run.kind === 'synthetic' && (list(c.bundle.events).some(e => e.synthetic !== true)
        || list(c.bundle.messages).some(m => m.synthetic !== true))) add(at, 'Synthetic run contains unlabelled observations.');
    }
    if (!same(sorted(list(c.semantic).map(s => s.criterion)), sorted(SEMANTIC))) add(at, 'Semantic criteria must each occur once.');
    for (const s of list(c.semantic)) {
      if (!Object.hasOwn(counts, s.status)) { add(at, 'Unknown judgement status.'); continue; }
      counts[s.status]++;
      if (!nonblank(s.rationale)) add(at, 'Every judgement needs a reason, including unknown/not_run.');
      if (c.status === 'not_run' && s.status !== 'not_run') add(at, 'not_run was promoted.');
      if (c.status === 'unknown' && ['pass', 'fail'].includes(s.status)) add(at, 'Unknown case cannot carry semantic verdicts.');
      if (!['pass', 'fail'].includes(s.status)) continue;
      const judge = judges.get(s.judge);
      if (!judge || !list(s.basis).length) add(at, 'Semantic verdict requires a registered judge and exact public evidence.');
      for (const quote of list(s.basis)) {
        const m = c.bundle?.messages?.find(m => m.id === quote.message);
        if (!m || !nonblank(quote.quote) || !Number.isInteger(quote.start) || quote.start < 0
          || m.text.slice(quote.start, quote.start + quote.quote.length) !== quote.quote) add(at, 'Semantic quote does not resolve.');
      }
      if (['host_display', 'user_opened_result', 'human_approval_real'].includes(s.criterion)) {
        if (!run.host_executed || run.kind !== 'model' || judge?.kind !== 'human'
          || !list(s.basis).some(q => c.bundle?.messages?.some(m => m.id === q.message && m.role === 'observer' && m.synthetic === false)))
          add(at, 'Host/access/human judgement requires actual Host and human observation; tool success is insufficient.');
        if (s.criterion === 'human_approval_real' && s.status === 'pass') {
          const decisions = list(c.bundle?.events).filter(e => e.request?.name === 'gil_plan_decide');
          if (!decisions.length || decisions.some(e => e.synthetic || e.actor !== 'app')) add(at, 'Synthetic/absent App input cannot establish real human approval.');
        }
      }
    }
  }
  return { problems, semantic_counts: counts, model_task: run.model_executed ? 'recorded; inspect per-criterion results' : 'not_run',
    host_task: run.host_executed ? 'recorded; inspect human observations' : 'not_run' };
}
