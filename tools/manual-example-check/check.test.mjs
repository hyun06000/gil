// Self-tests for the example contract checker. Core-free by default: documents are copied into a
// temporary folder and edited there; the repository is never modified. One opt-in test runs a
// real Core when GIL_EXAMPLE_CHECK_GIL names a binary built with the product feature set.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, readFile, cp } from 'node:fs/promises';
import { existsSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { loadRegistry, validateRegistry, evaluateSources, sourcesClean, RegistryError } from './lib/registry.mjs';
import { locate, inventory, sha256 } from './lib/sources.mjs';
import { parsePayload, fillTemplate, PayloadError } from './lib/extract.mjs';
import { exampleStatuses, verdict } from './lib/report.mjs';
import { inside } from './lib/rig.mjs';
import { CASES } from './lib/cases.mjs';
import { parseArgs, prepareOut } from './check.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, '..', '..');
const MAPPING = join(here, 'examples.json');
const reg = () => loadRegistry(MAPPING);
const CLI = join(here, 'check.mjs');

/** A documents-only copy of the watched sources, optionally edited. */
async function docsCopy(edit = {}) {
  const root = await mkdtemp(join(tmpdir(), 'gil-example-check-docs-'));
  for (const s of reg().sources) {
    await mkdir(dirname(join(root, s.path)), { recursive: true });
    let text = readFileSync(join(repo, s.path), 'utf8');
    if (edit[s.path]) text = edit[s.path](text);
    await writeFile(join(root, s.path), text);
  }
  return root;
}
const evalAt = (root, registry = reg()) => evaluateSources(registry, { repo: root });
const row = (result, id) => result.examples.find((e) => e.id === id);
const cli = (args, opts = {}) => spawnSync(process.execPath, [CLI, ...args], { encoding: 'utf8', ...opts });

test('the reviewed mapping matches the current documents', () => {
  const result = evalAt(repo);
  const bad = result.examples.filter((e) => e.source_status !== 'ok').map((e) => `${e.id}: ${e.detail}`);
  assert.deepEqual(bad, []);
  assert.deepEqual(result.unmapped, []);
  assert.ok(sourcesClean(result));
});

test('every case example exists, and every runnable example has a reviewed case', () => {
  const r = reg();
  const ids = new Set(r.examples.map((e) => e.id));
  const caseIds = new Set(CASES.map((c) => c.id));
  for (const c of CASES) if (Array.isArray(c.examples)) for (const id of c.examples) assert.ok(ids.has(id), `${c.id} names unknown ${id}`);
  for (const e of r.examples) {
    for (const id of e.cases) assert.ok(caseIds.has(id), `${e.id} names unknown case ${id}`);
    for (const id of e.cases.filter((x) => x !== 'CX-00')) assert.ok(CASES.find((c) => c.id === id).examples.includes(e.id), `${e.id} → ${id} is not linked back`);
  }
});

test('a changed excerpt is source_changed with the file, lines and the mapping entry to update', async () => {
  const root = await docsCopy({ 'src/manual/topics/cycle/interview/clarify.md': (t) => t.replace('필요한 열·형식·누락 여부', '필요한 열·형식 여부') });
  const result = evalAt(root);
  const e = row(result, 'MAN-CLARIFY-GOOD');
  assert.equal(e.source_status, 'source_changed');
  assert.match(e.detail, /src\/manual\/topics\/cycle\/interview\/clarify\.md:\d+-\d+/);
  assert.match(e.detail, /examples\.json → MAN-CLARIFY-GOOD\.sha256/);
  assert.match(e.detail, /CX-04, CX-05, CX-06/);
  assert.equal(row(result, 'MAN-DEFINE-GOOD').source_status, 'ok');
  assert.equal(sourcesClean(result), false);
  assert.equal(result.consistency[0].status, 'not_checked', 'consistency is not judged on a stale excerpt');
});

test('a removed or duplicated example is missing / ambiguous, never ok', async () => {
  const gone = evalAt(await docsCopy({ 'src/manual/topics/action/open-contract.md': (t) => t.replace('[올바른 예]\n', '[예]\n') }));
  assert.equal(row(gone, 'MAN-OPEN-GOOD').source_status, 'missing');
  assert.match(row(gone, 'MAN-OPEN-GOOD').detail, /MAN-OPEN-GOOD\.source\.locator/);
  const twice = evalAt(await docsCopy({ 'src/mcp/workflow.md': (t) => t.replace('8. **Recover and keep the user informed.**', '     ```json\n     {}\n     ```\n8. **Recover and keep the user informed.**') }));
  assert.equal(row(twice, 'WF-PROPOSE-WRAPPER').source_status, 'ambiguous');
});

test('a new example-shaped block without a mapping is reported as unmapped', async () => {
  // Inside a mapped section the block changes that excerpt: it is source_changed, not hidden.
  const within = evalAt(await docsCopy({ 'src/manual/topics/step/define/smallest.md': (t) => t.replace('[관련 주제]', '```json\n{"problem": "x"}\n```\n\n[관련 주제]') }));
  assert.equal(row(within, 'MAN-DEFINE-FAIL').source_status, 'source_changed');
  // Outside every mapped excerpt it is an unmapped region.
  const result = evalAt(await docsCopy({ 'src/manual/topics/step/define/smallest.md': (t) => `${t}\n\`\`\`json\n{"problem": "x"}\n\`\`\`\n` }));
  assert.equal(result.unmapped.length, 1);
  assert.equal(result.unmapped[0].kind, 'fence:json');
  assert.match(result.unmapped[0].detail, /add an examples\.json entry/);
  assert.equal(sourcesClean(result), false);
});

test('cross-document wording: acknowledged mismatch is reported, a new one fails', () => {
  const r = reg();
  assert.equal(evalAt(repo, r).consistency[0].status, 'known_mismatch');
  const fresh = structuredClone(r);
  fresh.consistency[0].acknowledged = null;
  const c = evalAt(repo, validateRegistry(fresh)).consistency[0];
  assert.equal(c.status, 'mismatch');
  assert.match(c.detail, /MAN-DEFINE-GOOD\.label ≠ MAN-CLARIFY-GOOD\.items\[0\]\.question/);
});

test('a re-fingerprinted excerpt whose payload no longer parses is payload_invalid', async () => {
  const root = await docsCopy({ 'src/manual/topics/cycle/interview/clarify.md': (t) => t.replace(/      항목 C .*\n/, '') });
  const r = structuredClone(reg());
  const entry = r.examples.find((e) => e.id === 'MAN-CLARIFY-GOOD');
  entry.sha256 = locate(readFileSync(join(root, entry.source.path), 'utf8'), entry.source.locator).sha256;
  const e = row(evalAt(root, validateRegistry(r)), 'MAN-CLARIFY-GOOD');
  assert.equal(e.source_status, 'payload_invalid');
  assert.match(e.detail, /found 2 item lines, mapping expects 3/);
});

test('templates: only declared placeholders are replaced; others and changed literals fail', () => {
  const excerpt = row(evalAt(repo), 'WF-PROPOSE-WRAPPER').excerpt;
  const spec = reg().examples.find((e) => e.id === 'WF-PROPOSE-WRAPPER').payload;
  const payload = parsePayload(excerpt, spec);
  const { input } = fillTemplate(payload, { project_root: '/tmp/p', issued_basis_refs: 'step:C3/S1\nstep:C3/S2' });
  assert.equal(input.project_root, '/tmp/p');
  assert.equal(input.draft.synthesis.basis_refs, 'step:C3/S1\nstep:C3/S2');
  assert.equal(input.draft.scope.goal, '…', 'content placeholders are sent verbatim and reported');
  assert.throws(() => fillTemplate(parsePayload(excerpt.replace('"…", "in_scope"', '"<goal>", "in_scope"'), spec), { project_root: '/p', issued_basis_refs: 'x' }), /undeclared placeholder\(s\): draft\.scope\.goal/);
  assert.throws(() => parsePayload(excerpt.replace('step:C1/S1', 'step:C2/S1'), spec), PayloadError);
  assert.throws(() => fillTemplate(payload, { project_root: '/p' }), /no issued value/);
});

test('the other payload parsers read exactly the reviewed fields', () => {
  const r = evalAt(repo);
  assert.deepEqual(row(r, 'MAN-CLARIFY-GOOD').payload.items.map((i) => i.id), ['A', 'B', 'C']);
  assert.deepEqual(Object.keys(row(r, 'MAN-DEFINE-GOOD').payload.fields), ['problem', 'success_condition']);
  assert.equal(row(r, 'MAN-OPEN-GOOD').payload.text.split('\n').filter(Boolean).length, 3);
  assert.deepEqual(row(r, 'SPEC-REORDER').payload, { first_approved: ['A', 'B', 'C'], second_approved: ['A', 'D', 'B', 'C'], expected_pending: ['D', 'B', 'C'] });
  assert.equal(row(r, 'SPEC-BASIS-JSON').payload.value, 'step:C5/S1\nstep:C5/S2');
  assert.deepEqual(row(r, 'WF-DOCUMENT-VERIFY').payload, { plan_document: 'plan_document: plan/A.md', evidence_files: 'evidence_files: evidence/A-result.md' });
});

test('locators and inventory never use regular expressions from the mapping', () => {
  const text = 'a\n[올바른 예]\n  x.*\n[흔한 실패]\n  y\n```json\n{}\n```\n';
  assert.equal(locate(text, { section: '[올바른 예]' }).excerpt, '[올바른 예]\n  x.*\n');
  assert.equal(locate(text, { from: 'x.*', to: '.*' }).excerpt, 'x.*');
  assert.deepEqual(inventory(text, { sections: ['[올바른 예]'] }).map((r) => r.kind), ['fence:json', 'section:[올바른 예]']);
  assert.equal(locate(text, { fence: 'json' }).sha256, sha256('{}\n'));
});

test('statuses: semantic_only / not_executed never pass; stale or failing examples never pass', () => {
  const sources = evalAt(repo);
  const all = (status) => CASES.map((c) => ({ id: c.id, status, checks: [] }));
  const passing = exampleStatuses(sources, all('pass'));
  assert.ok(passing.filter((e) => ['semantic_only', 'not_executed'].includes(e.kind)).every((e) => e.status === e.kind));
  assert.equal(passing.filter((e) => e.status === 'pass').length, sources.examples.filter((e) => !['semantic_only', 'not_executed'].includes(e.kind)).length);
  const oneFail = all('pass').map((c) => (c.id === 'CX-05' ? { ...c, status: 'fail', detail: 'x' } : c));
  const st = Object.fromEntries(exampleStatuses(sources, oneFail).map((e) => [e.id, e.status]));
  assert.equal(st['SPEC-REORDER'], 'fail');
  assert.equal(st['WF-DOCUMENT-VERIFY'], 'pass');
  const notRun = exampleStatuses(sources, all('pass').filter((c) => c.id !== 'CX-06'));
  assert.equal(notRun.find((e) => e.id === 'WF-REINTERVIEW').status, 'not_run');
  const report = { examples: notRun, unmapped: [], consistency: sources.consistency, cases: all('pass').filter((c) => c.id !== 'CX-06'), isolation: [] };
  assert.equal(verdict(report).ok, false);
  const stale = structuredClone(sources);
  stale.examples.find((e) => e.id === 'MAN-OPEN-GOOD').source_status = 'source_changed';
  assert.equal(exampleStatuses(stale, all('pass')).find((e) => e.id === 'MAN-OPEN-GOOD').status, 'source_changed');
});

test('registry validation refuses unreviewed shapes', () => {
  const r = reg();
  const bad = (fn) => { const x = structuredClone(r); fn(x); return () => validateRegistry(x); };
  assert.throws(bad((x) => { x.examples[0].cases = []; }), RegistryError);
  assert.throws(bad((x) => { x.examples[0].kind = 'runnable'; }), RegistryError);
  assert.throws(bad((x) => { x.examples.find((e) => e.kind === 'semantic_only').reason = ''; }), RegistryError);
  assert.throws(bad((x) => { x.examples[1].id = x.examples[0].id; }), RegistryError);
  assert.throws(bad((x) => { x.examples[0].sha256 = 'abc'; }), RegistryError);
});

test('served examples cannot pass when the running Core text check is missing, skipped or fails', () => {
  const sources = evalAt(repo);
  const passing = CASES.filter((c) => c.id !== 'CX-00').map((c) => ({ id: c.id, status: 'pass' }));
  for (const check of [null, { id: 'CX-00', status: 'not_run' }, { id: 'CX-00', status: 'fail', failed_check: 'served WF-PROPOSE-WRAPPER', detail: 'stale binary' }]) {
    const rows = exampleStatuses(sources, check ? [check, ...passing] : passing);
    for (const e of sources.examples.filter((e) => e.served)) {
      assert.notEqual(rows.find((r) => r.id === e.id).status, 'pass', `${e.id} with ${check?.status ?? 'missing'} served-text check`);
    }
  }
});

test('isolation: output and projects stay outside the repository; out must be new or empty', async () => {
  await assert.rejects(prepareOut(join(repo, 'tools', 'manual-example-check', 'out'), repo), /outside the source repository/);
  const full = await mkdtemp(join(tmpdir(), 'gil-example-check-out-'));
  await writeFile(join(full, 'keep.txt'), 'x');
  await assert.rejects(prepareOut(full, repo), /not empty/);
  assert.equal(existsSync(join(repo, 'tools', 'manual-example-check', 'out')), false);
  assert.ok(inside(join(tmpdir(), 'x', 'not-yet'), tmpdir()), 'symlinked temp roots compare through the nearest existing parent');
  assert.equal(inside(tmpdir(), repo), false);
  assert.throws(() => parseArgs(['run']), /exactly one of --build or --gil/);
  assert.throws(() => parseArgs(['sources', '--gil', 'x']), /sources takes only/);
  assert.throws(() => parseArgs(['exec']), /first argument/);
});

test('CLI: a changed document fails `sources` and `run` executes no case against it', async () => {
  const root = await docsCopy({ 'src/mcp/workflow.md': (t) => t.replace('"items": [{"id": "A"', '"items": [{"id": "X"') });
  const s = cli(['sources', '--docs-root', root]);
  assert.equal(s.status, 1, s.stdout + s.stderr);
  assert.match(s.stdout, /PROBLEM WF-PROPOSE-WRAPPER/);
  assert.match(s.stdout, /nothing was executed/);
  const out = join(await mkdtemp(join(tmpdir(), 'gil-example-check-run-')), 'out');
  // The "binary" is never started: with a stale mapping no case runs.
  const r = cli(['run', '--gil', process.execPath, '--docs-root', root, '--out', out]);
  assert.equal(r.status, 1, r.stdout + r.stderr);
  const report = JSON.parse(await readFile(join(out, 'report.json'), 'utf8'));
  assert.ok(report.cases.every((c) => c.status === 'not_run'));
  assert.equal(report.examples.find((e) => e.id === 'WF-PROPOSE-WRAPPER').status, 'source_changed');
  assert.ok(report.examples.every((e) => e.status !== 'pass'));
  assert.match(report.claims.human_approval, /not_observed/);
  assert.equal(cli(['sources']).status, 0);
});

const GIL = process.env.GIL_EXAMPLE_CHECK_GIL;
test('real Core: an edited, re-fingerprinted example that breaks the contract fails its case', { skip: !GIL && 'set GIL_EXAMPLE_CHECK_GIL to a gil built with approved-plan,visual-authoring,product-safety' }, async () => {
  // The wrapper loses its `draft` level: a reviewer who only refreshes the fingerprint must still
  // see the case fail at the real Core, and CX-00 must report that the binary serves other text.
  const root = await docsCopy({ 'src/mcp/workflow.md': (t) => t.replace('{"project_root": "<absolute path>",\n      "draft": {\n', '{"project_root": "<absolute path>",\n      "flat": {\n') });
  const r = structuredClone(reg());
  for (const e of r.examples) e.sha256 = evalAt(root, validateRegistry(structuredClone(r))).examples.find((x) => x.id === e.id).sha256;
  const wrapper = r.examples.find((e) => e.id === 'WF-PROPOSE-WRAPPER');
  wrapper.payload.placeholders[1].path = ['flat', 'synthesis', 'basis_refs'];
  const dir = await mkdtemp(join(tmpdir(), 'gil-example-check-real-'));
  await writeFile(join(dir, 'examples.json'), JSON.stringify(r));
  const out = join(dir, 'out');
  const run = cli(['run', '--gil', resolve(GIL), '--docs-root', root, '--mapping', join(dir, 'examples.json'), '--out', out, '--case', 'CX-00,CX-01']);
  assert.equal(run.status, 1, run.stdout + run.stderr);
  const report = JSON.parse(await readFile(join(out, 'report.json'), 'utf8'));
  const c = Object.fromEntries(report.cases.map((x) => [x.id, x]));
  assert.equal(c['CX-00'].status, 'fail');
  assert.match(c['CX-00'].detail, /served WF-PROPOSE-WRAPPER/);
  assert.equal(c['CX-01'].status, 'fail');
  assert.match(c['CX-01'].detail, /documented wrapper accepted: expected success, got JSON-RPC -32602/);
  assert.equal(report.examples.find((e) => e.id === 'WF-PROPOSE-WRAPPER').status, 'fail');
});
