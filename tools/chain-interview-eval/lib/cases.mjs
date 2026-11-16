// Case-set loading and fixture integrity checks for scenarios/chain-interview.
// Reads only the files it is pointed at. Nothing here judges wording.
import { existsSync, statSync } from 'node:fs';
import { join, basename, resolve, dirname, isAbsolute, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

// The repository this tool ships in: spec_refs of a case set copied elsewhere still resolve here.
const TOOL_REPO = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..', '..');
import { readJson, listJsonFiles, caseSetDigest } from './io.mjs';
import { checkContent } from './structural.mjs';
import {
  CASE_SCHEMA, COMPARISON_SCHEMA, RUBRIC_SCHEMA, CASE_CATEGORIES, STAGES, EXAMPLE_KINDS, NECESSITY,
} from './vocab.mjs';

const nonBlank = (v) => typeof v === 'string' && v.trim().length > 0;
const arr = (v) => (Array.isArray(v) ? v : null);

export function loadRubric(path) {
  const r = readJson(path);
  if (r.error) return { error: `rubric ${path}: ${r.error}` };
  const v = r.value;
  if (v?.schema !== RUBRIC_SCHEMA) return { error: `rubric ${path}: schema must be ${RUBRIC_SCHEMA}` };
  const automatic = new Set((arr(v.automatic) || []).map((x) => x?.id).filter(nonBlank));
  const semantic = new Set((arr(v.semantic) || []).map((x) => x?.id).filter(nonBlank));
  const failureModes = new Set((arr(v.failure_modes) || []).filter(nonBlank));
  return { rubric: v, automatic, semantic, failureModes };
}

/**
 * Load every case under <dir>/cases plus rubric.json and comparisons/*.json.
 * @returns {{cases, rubric, comparisons, problems, digest}}
 */
export function loadCaseSet(dir, { repoRoot = null } = {}) {
  const problems = [];
  // Problems name files relative to the case directory: no absolute personal paths in reports.
  const push = (check, file, detail) => problems.push({ check, file: isAbsolute(file) ? (relative(dir, file) || basename(file)) : file, detail });
  const rubricPath = join(dir, 'rubric.json');
  const rub = loadRubric(rubricPath);
  if (rub.error) { push('case_schema', rubricPath, rub.error); return { cases: [], rubric: null, comparisons: new Map(), problems, digest: null }; }

  const listed = listJsonFiles(join(dir, 'cases'));
  if (listed.error) { push('case_schema', join(dir, 'cases'), listed.error); return { cases: [], rubric: rub.rubric, comparisons: new Map(), problems, digest: null }; }
  const comparisons = new Map();
  const compList = listJsonFiles(join(dir, 'comparisons'));
  for (const f of compList.files || []) {
    const r = readJson(f);
    if (r.error) { push('comparison_shape', f, r.error); continue; }
    comparisons.set(`comparisons/${basename(f)}`, r.value);
    checkComparison(r.value, f, push);
  }

  const cases = [];
  const ids = new Map();
  for (const f of listed.files) {
    const r = readJson(f);
    if (r.error) { push('case_schema', f, r.error); continue; }
    const c = r.value;
    validateCase(c, f, { rub, comparisons, dir, repoRoot, push });
    if (nonBlank(c?.id)) {
      if (ids.has(c.id)) push('case_ids_unique', f, `id ${c.id} already used by ${ids.get(c.id)}`);
      else ids.set(c.id, basename(f));
      if (basename(f, '.json') !== c.id) push('case_id_matches_filename', f, `file ${basename(f)} holds id ${c.id}`);
    }
    cases.push({ file: f, value: c });
  }
  const digest = listed.files.length ? caseSetDigest([...listed.files, rubricPath, ...(compList.files || [])]) : null;
  return { cases, rubric: rub.rubric, comparisons, problems, digest, rubricIndex: rub };
}

function checkComparison(v, file, push) {
  if (v?.schema !== COMPARISON_SCHEMA) push('comparison_shape', file, `schema must be ${COMPARISON_SCHEMA}`);
  if (!nonBlank(v?.goal)) push('comparison_shape', file, 'goal blank');
  for (const variant of ['bundled', 'separable']) {
    const p = v?.[variant]?.proposal;
    if (!p) { push('comparison_shape', file, `${variant}.proposal missing`); continue; }
    const bad = checkContent('proposal', p).filter((x) => x.status === 'fail');
    for (const b of bad) push('comparison_shape', file, `${variant}: ${b.id}: ${b.detail}`);
    const n = Array.isArray(p.items) ? p.items.length : 0;
    if (variant === 'bundled' && n !== 1) push('comparison_shape', file, `bundled must hold exactly one item, has ${n}`);
    if (variant === 'separable' && n < 2) push('comparison_shape', file, `separable must hold at least two items, has ${n}`);
    if (!nonBlank(v?.[variant]?.assessment)) push('comparison_shape', file, `${variant}.assessment blank`);
  }
}

function resolvePlanFor(c) {
  // The plan a Define may bind to: the case's prior approved plan, else the first good proposal.
  if (c?.input?.prior_state?.approved_plan?.items) return c.input.prior_state.approved_plan;
  const g = (arr(c?.good_examples) || []).find((e) => e?.kind === 'proposal' && e?.content?.items);
  return g ? g.content : null;
}

function validateCase(c, file, { rub, comparisons, dir, repoRoot, push }) {
  if (!c || typeof c !== 'object') { push('case_schema', file, 'not an object'); return; }
  if (c.schema !== CASE_SCHEMA) push('case_schema', file, `schema must be ${CASE_SCHEMA}`);
  if (!/^CI-\d{2}$/.test(c.id || '')) push('case_schema', file, 'id must look like CI-NN');
  if (!nonBlank(c.title)) push('case_schema', file, 'title blank');
  if (!CASE_CATEGORIES.includes(c.category)) push('case_schema', file, `category "${c.category}" not in ${CASE_CATEGORIES.join('|')}`);
  const stages = arr(c.stages_under_test);
  if (!stages || !stages.length || stages.some((s) => !STAGES.includes(s))) push('case_schema', file, `stages_under_test must be a nonempty subset of ${STAGES.join('|')}`);

  // Inputs.
  const input = c.input && typeof c.input === 'object' ? c.input : null;
  if (!input) push('required_inputs_present', file, 'input missing');
  else {
    if (!nonBlank(input.user_request)) push('required_inputs_present', file, 'input.user_request blank');
    for (const [i, m] of (arr(input.materials) || []).entries()) {
      if (!nonBlank(m?.path) || !nonBlank(m?.summary) || typeof m?.exists !== 'boolean') push('required_inputs_present', file, `input.materials[${i}] needs path, summary, exists`);
    }
    for (const [i, a] of (arr(input.simulated_answers) || []).entries()) {
      if (!nonBlank(a?.topic) || !nonBlank(a?.answer)) push('required_inputs_present', file, `input.simulated_answers[${i}] needs topic and answer`);
      if (a?.source !== 'synthetic') push('required_inputs_present', file, `input.simulated_answers[${i}].source must be "synthetic"`);
    }
    const plan = input.prior_state?.approved_plan;
    if (plan) {
      const bad = checkContent('proposal', plan).filter((x) => x.status === 'fail' && x.id === 'proposal_shape');
      for (const b of bad) push('reference_integrity', file, `prior_state.approved_plan: ${b.detail}`);
      if (input.prior_state.approved_plan.approval?.kind && input.prior_state.approved_plan.approval.kind !== 'synthetic') push('reference_integrity', file, 'prior_state.approved_plan.approval.kind must be "synthetic" in a fixture');
    }
  }

  // Clarifications and their scripted answers.
  const topics = new Set((arr(input?.simulated_answers) || []).map((a) => a?.topic));
  const clar = arr(c.clarifications) || [];
  const clarIds = new Set();
  for (const [i, q] of clar.entries()) {
    if (!nonBlank(q?.id)) push('case_schema', file, `clarifications[${i}].id blank`);
    else if (clarIds.has(q.id)) push('case_schema', file, `clarifications[${i}].id ${q.id} duplicated`);
    else clarIds.add(q.id);
    if (!NECESSITY.includes(q?.necessity)) push('case_schema', file, `clarifications[${i}].necessity must be ${NECESSITY.join('|')}`);
    if (!nonBlank(q?.why)) push('case_schema', file, `clarifications[${i}].why blank`);
    if (q?.necessity === 'required' && !topics.has(q?.topic)) push('required_inputs_present', file, `clarification ${q?.id} (${q?.topic}) is required but input.simulated_answers has no answer for that topic`);
  }

  // Examples.
  const good = arr(c.good_examples) || [];
  const bad = arr(c.bad_examples) || [];
  if (!good.length) push('examples_present', file, 'good_examples empty');
  if (!bad.length) push('examples_present', file, 'bad_examples empty');
  const exIds = new Set();
  const plan = resolvePlanFor(c);
  const closed = input?.prior_state?.closed_experiments ?? null;
  for (const [label, list] of [['good', good], ['bad', bad]]) {
    for (const [i, e] of list.entries()) {
      const where = `${label}_examples[${i}]`;
      if (!nonBlank(e?.id)) push('examples_present', file, `${where}.id blank`);
      else if (exIds.has(e.id)) push('examples_present', file, `${where}.id ${e.id} duplicated`);
      else exIds.add(e.id);
      if (!EXAMPLE_KINDS.includes(e?.kind)) { push('case_schema', file, `${where}.kind must be ${EXAMPLE_KINDS.join('|')}`); continue; }
      if (!e.content || typeof e.content !== 'object') { push('case_schema', file, `${where}.content missing`); continue; }
      const found = checkContent(e.kind, e.content, { plan, closedExperiments: closed }).filter((x) => x.status === 'fail').map((x) => x.id).sort();
      if (label === 'good') {
        if (!nonBlank(e.note)) push('case_schema', file, `${where}.note blank (say what makes it good)`);
        const meets = arr(e.meets) || [];
        if (!meets.length) push('reference_integrity', file, `${where}.meets must name at least one semantic criterion`);
        for (const m of meets) if (!rub.semantic.has(m)) push('reference_integrity', file, `${where}.meets names unknown criterion ${m}`);
        if (found.length) push('good_example_has_no_violation', file, `${where} (${e.id}) fails structural check(s) ${found.join(',')}`);
      } else {
        const modes = arr(e.failure_modes) || [];
        if (!modes.length) push('reference_integrity', file, `${where}.failure_modes empty`);
        for (const m of modes) if (!rub.failureModes.has(m)) push('reference_integrity', file, `${where}.failure_modes names unknown mode ${m}`);
        const declared = [...(arr(e.declared_structural_violations) || [])].sort();
        if (declared.join(',') !== found.join(',')) push('bad_example_violations_declared', file, `${where} (${e.id}) declared [${declared.join(',')}] but structural checks found [${found.join(',')}]`);
        for (const d of declared) if (!rub.automatic.has(d)) push('reference_integrity', file, `${where}.declared_structural_violations names unknown check ${d}`);
        if (!nonBlank(e.note)) push('case_schema', file, `${where}.note blank (say why it fails)`);
      }
    }
  }

  // Judgement wiring.
  const jd = c.judgement && typeof c.judgement === 'object' ? c.judgement : null;
  if (!jd) push('case_schema', file, 'judgement missing');
  else {
    for (const a of arr(jd.automatic) || []) if (!rub.automatic.has(a)) push('reference_integrity', file, `judgement.automatic names unknown check ${a}`);
    const sem = arr(jd.semantic) || [];
    if (!sem.length) push('case_schema', file, 'judgement.semantic empty');
    for (const s of sem) if (!rub.semantic.has(s)) push('reference_integrity', file, `judgement.semantic names unknown criterion ${s}`);
    const ny = arr(jd.not_yet_judgeable);
    if (!ny || !ny.length || ny.some((s) => !nonBlank(s))) push('case_schema', file, 'judgement.not_yet_judgeable must list what cannot be judged yet (nonempty strings)');
  }

  // References to other files.
  if (c.comparison_ref != null) {
    if (!comparisons.has(c.comparison_ref)) push('reference_integrity', file, `comparison_ref ${c.comparison_ref} not found under ${join(dir, 'comparisons')}`);
  }
  const refs = arr(c.spec_refs) || [];
  if (!refs.length) push('reference_integrity', file, 'spec_refs empty');
  for (const ref of refs) {
    if (!nonBlank(ref)) { push('reference_integrity', file, 'spec_refs holds a blank entry'); continue; }
    let filePart = ref.split('#')[0];
    try { filePart = decodeURIComponent(filePart); } catch { push('reference_integrity', file, `spec_refs ${ref} has invalid percent-encoding`); continue; }
    if (isAbsolute(filePart) || filePart.split('/').includes('..')) { push('reference_integrity', file, `spec_refs ${ref} must be repo-relative`); continue; }
    const root = repoRoot || findRepoRoot(dir) || TOOL_REPO;
    if (!existsSync(join(root, filePart))) push('reference_integrity', file, `spec_refs ${ref}: ${filePart} does not exist in the repository`);
  }
}

function findRepoRoot(start) {
  let d = resolve(start);
  for (let i = 0; i < 12; i += 1) {
    if (existsSync(join(d, 'AGENTS.md')) && existsSync(join(d, 'spec'))) return d;
    const up = dirname(d); if (up === d) break; d = up;
  }
  return null;
}

/** Group problems by check id for reporting. */
export function summarizeProblems(problems) {
  const byCheck = {};
  for (const p of problems) { byCheck[p.check] = byCheck[p.check] || []; byCheck[p.check].push(p); }
  return byCheck;
}
