// One isolated temporary GIL project driven over the real `gil mcp --serve` stdio protocol.
// Reuses tools/scenarios/lib (rpc client, isolated child environment, product record path,
// redaction) read-only. Only an allowlist of tools can be called; App decisions are labelled
// synthetic harness input; the record is only ever read, never edited.
import { mkdir, writeFile, readFile } from 'node:fs/promises';
import { readFileSync, realpathSync, existsSync } from 'node:fs';
import { join, resolve, relative, isAbsolute, sep } from 'node:path';
import { CoreClient, isolatedEnv } from '../../scenarios/lib/rpc.mjs';
import { projectStatePath } from '../../scenarios/lib/state-path.mjs';

export const MODEL_TOOLS = new Set(['gil_start', 'gil_open', 'gil_close', 'gil_context', 'gil_status', 'gil_help', 'gil_plan_propose', 'gil_plan_read', 'gil_plan_request_interview']);
export const APP_TOOLS = new Set(['gil_plan_review', 'gil_plan_decide']);
export const DISPLAY_TOOLS = new Set(['show_gil_monitor']);
export const PROFILE_TOOLS = new Set(['gil_plan_propose', 'gil_plan_read', 'gil_plan_request_interview', 'gil_plan_review', 'gil_plan_decide']);
export const PROFILE_FEATURES = 'approved-plan,visual-authoring,product-safety';

export class CheckFailed extends Error {
  constructor(check, message) { super(`${check}: ${message}`); this.check = check; }
}

/** True when `child` is `parent` or inside it (after resolving symlinks where they exist). */
export function inside(child, parent) {
  // Resolve symlinks through the nearest existing ancestor, so /var and /private/var agree even
  // for a file that does not exist yet.
  const real = (p) => {
    let head = resolve(p);
    const tail = [];
    for (;;) {
      try { return join(realpathSync.native(head), ...tail.reverse()); } catch {}
      const up = resolve(head, '..');
      if (up === head) return resolve(p);
      tail.push(relative(up, head));
      head = up;
    }
  };
  const rel = relative(real(parent), real(child));
  return rel === '' || (!rel.startsWith('..') && !isAbsolute(rel));
}

export const said = (r) => {
  const s = r?.structured?.said;
  if (typeof s === 'string' && s) return s;
  return (r?.result?.content ?? []).map((b) => b.text ?? '').join('\n');
};
/** Every text block (Host guidance + Core receipt) of a tool result. */
export const blocks = (r) => (r?.result?.content ?? []).map((b) => b.text ?? '').join('\n');

export class Rig {
  constructor({ binary, workRoot, label, repo, timeoutMs, onEvent }) {
    this.label = label;
    this.base = join(workRoot, label);
    this.root = join(this.base, 'project');
    this.binary = binary;
    this.repo = repo;
    this.timeoutMs = timeoutMs;
    this.onEvent = onEvent ?? (() => {});
    this.scope = null;
    this.checks = [];
    this.vars = {};
  }

  async open() {
    if (inside(this.base, this.repo)) throw new Error(`refusing a temporary project inside the source repository: ${this.base}`);
    if (existsSync(this.base)) throw new Error(`temporary case folder already exists: ${this.base}`);
    for (const d of ['project', 'home', 'localappdata']) await mkdir(join(this.base, d), { recursive: true });
    // bindings/ is left for the Core to create with private permissions.
    this.env = isolatedEnv({ home: join(this.base, 'home'), localAppData: process.platform === 'win32' ? join(this.base, 'localappdata') : null });
    this.client = new CoreClient({ binary: this.binary, settingsDir: join(this.base, 'bindings'), isolated: this.env, cwd: this.base, requestTimeoutMs: this.timeoutMs, label: this.label }).start();
    this.initialize = await this.client.initialize({ name: 'gil-manual-example-check', version: '0' });
    // Product-profile preflight: without the approved-plan tools every case would fail for an
    // unrelated reason, so name the real cause once.
    const listed = await this.client.request('tools/list', {});
    const names = new Set((listed.result?.tools ?? []).map((t) => t.name));
    const missing = [...PROFILE_TOOLS].filter((n) => !names.has(n));
    if (missing.length) throw new CheckFailed('profile', `this Core does not offer ${missing.join(', ')}; build it with --features ${PROFILE_FEATURES} (the default profile cannot run these examples)`);
    return this;
  }

  async close() {
    if (!this.client) return null;
    const got = await this.client.close();
    this.client = null;
    return got;
  }

  async call(actor, name, args, { synthetic = false } = {}) {
    const allowed = actor === 'app' ? APP_TOOLS : actor === 'display' ? DISPLAY_TOOLS : MODEL_TOOLS;
    if (!allowed.has(name)) throw new Error(`${name} is not in the ${actor} allowlist`);
    const started = Date.now();
    const r = await this.client.call(name, args);
    this.onEvent({ case: this.label, actor: synthetic ? 'harness' : actor, synthetic, tool: name, args, ok: r.ok, error: r.error ? { code: r.error.code, message: r.error.message } : null, said: said(r).slice(0, 600), ms: Date.now() - started });
    return r;
  }
  model(name, args = {}) { return this.call('model', name, { project_root: this.root, ...args }); }
  /** App-only path. Always synthetic here: a harness stands in for a Monitor button press. */
  app(name, args = {}) { return this.call('app', name, { scope_id: this.scope, ...args }, { synthetic: true }); }

  recordPath() { return projectStatePath(this.root, { env: this.env }).path; }
  recordBytes() {
    const p = this.recordPath();
    try { return readFileSync(p); } catch (err) { throw new CheckFailed('record', `project record missing at the product layout path (${err.code}); an unchanged comparison would be vacuous`); }
  }
  async plan() { return this.expectOk(await this.model('gil_plan_read'), 'gil_plan_read').structured; }
  /** Read-only projection of "where the project is": the record bytes, Context text and plan view. */
  async position() {
    const bytes = this.recordBytes();
    const context = said(this.expectOk(await this.model('gil_context'), 'gil_context'));
    const plan = await this.plan();
    if (!this.recordBytes().equals(bytes)) throw new CheckFailed('position', 'reading Context/plan changed the record');
    return { bytes, context, plan: JSON.stringify(plan) };
  }

  // ── checks ────────────────────────────────────────────────────────────────
  pass(check, detail = '') { this.checks.push({ check, status: 'pass', detail }); }
  need(cond, check, message) { if (!cond) throw new CheckFailed(check, message); }
  expectOk(r, check) {
    this.need(r.ok, check, `expected success, got ${r.error ? `JSON-RPC ${r.error.code} ${r.error.message}` : said(r).slice(0, 500)}`);
    return r;
  }
  /** A Core refusal (tool result with ok:false), never a JSON-RPC schema error. */
  expectRefused(r, check, markers = []) {
    this.need(!r.error, check, `expected a Core refusal, got JSON-RPC error ${r.error?.code} ${r.error?.message}`);
    this.need(!r.ok, check, `expected a refusal but the call succeeded: ${said(r).slice(0, 300)}`);
    const text = blocks(r);
    for (const m of markers) this.need(text.includes(m), check, `refusal lacks ${JSON.stringify(m)}: ${text.slice(0, 600)}`);
    return r;
  }
  /** A JSON-RPC input-schema rejection (-32602). */
  expectSchemaError(r, check, markers = []) {
    this.need(!!r.error, check, `expected a JSON-RPC schema error, got ${r.ok ? 'success' : `Core refusal: ${said(r).slice(0, 300)}`}`);
    this.need(r.error.code === -32602, check, `expected -32602, got ${r.error.code}`);
    for (const m of markers) this.need(String(r.error.message).includes(m), check, `schema error lacks ${JSON.stringify(m)}: ${r.error.message}`);
    return r;
  }
  /** Run a rejected call and prove the record and the read projection did not move. */
  async unchanged(check, fn) {
    const before = await this.position();
    this.mark = before.bytes;
    const out = await fn();
    this.mark = null;
    const after = await this.position();
    this.need(after.bytes.equals(before.bytes), check, 'record bytes changed after a rejection');
    this.need(after.context === before.context, check, 'gil_context (current position) changed after a rejection');
    this.need(after.plan === before.plan, check, 'gil_plan_read changed after a rejection');
    return out;
  }

  /** Inside `unchanged`: the record is still byte-identical right after this one rejected call. */
  still(check) {
    this.need(this.mark && this.recordBytes().equals(this.mark), check, 'record bytes changed right after this rejection');
  }

  // ── reviewed building blocks (explicit calls only) ─────────────────────────
  fill(template, extra = {}) {
    const vars = { ...this.vars, ...extra };
    return template.replace(/\$\{([a-z_]+)\}/g, (_, k) => { if (vars[k] === undefined) throw new Error(`fixture variable ${k} is not bound`); return vars[k]; });
  }
  static stepRef(r, kind) {
    const line = said(r).split('\n').find((l) => l.startsWith('열었다: step:') && l.includes(` · ${kind}`));
    if (!line) throw new CheckFailed(`open ${kind}`, `no open receipt line: ${said(r).slice(0, 300)}`);
    return line.slice('열었다: '.length).split(' · ')[0];
  }
  static cycleRef(r) {
    const line = said(r).split('\n').find((l) => l.startsWith('열었다: cycle:C'));
    if (!line) throw new CheckFailed('open cycle', `no cycle receipt line: ${said(r).slice(0, 300)}`);
    return line.slice('열었다: '.length).split(' ')[0];
  }
  async bootstrap() {
    const started = this.expectOk(await this.model('gil_start'), 'gil_start');
    const next = started.structured?.next_call;
    this.need(next?.name === 'show_gil_monitor' && next.arguments?.scope_id, 'gil_start', 'start did not name show_gil_monitor(scope_id)');
    this.scope = next.arguments.scope_id;
    this.expectOk(await this.call('display', 'show_gil_monitor', next.arguments), 'show_gil_monitor');
    this.need(started.structured.display_confirmed !== true, 'display', 'a protocol success must not claim a visible Monitor');
  }
  async openStep(kind, contract) {
    const r = this.expectOk(await this.model('gil_open', { kind, contract }), `open ${kind}`);
    return Rig.stepRef(r, kind);
  }
  async closeStep(report, check) { return this.expectOk(await this.model('gil_close', { report }), check); }
  /** Question + Interpretation closed, Synthesis open; returns the issued refs. */
  async interviewToSynthesis(inputs) {
    const question = await this.openStep('question', inputs.contract);
    await this.closeStep(inputs.question, 'close question');
    const interpretation = await this.openStep('interpretation', inputs.contract);
    await this.closeStep(inputs.interpretation, 'close interpretation');
    const synthesis = await this.openStep('synthesis', inputs.contract);
    return { question, interpretation, synthesis, basis: `${question}\n${interpretation}` };
  }
  /** Synthetic stand-in for the Monitor button: review → capability from _meta → decide. */
  async approveSynthetic(revision, decision = 'approve') {
    const review = this.expectOk(await this.app('gil_plan_review'), 'gil_plan_review');
    const token = review.meta?.['gil/planApprovalToken'];
    this.need(typeof token === 'string' && token.length === 64, 'gil_plan_review', 'no App capability in _meta');
    const p = review.structured?.proposal;
    this.need(p?.revision === revision, 'gil_plan_review', `reviewing revision ${p?.revision}, expected ${revision}`);
    this.expectOk(await this.app('gil_plan_decide', { revision, digest: p.digest, token, decision }), 'gil_plan_decide');
    const receipt = (await this.plan()).proposal?.receipt;
    this.need(receipt?.decision === decision, 'receipt', `receipt decision ${receipt?.decision}`);
    return receipt;
  }
  /** close_report verbatim → Outcome → Interview Cycle Report. */
  async closeApprovedInterview(inputs, synthesis) {
    const plan = await this.plan();
    this.need(typeof plan.close_report === 'string', 'close_report', 'no close_report after approval');
    await this.closeStep(plan.close_report, 'close synthesis with close_report');
    const outcome = await this.openStep('outcome', inputs.contract);
    await this.closeStep(this.fill(inputs.interview_outcome, { synthesis }), 'close interview outcome');
    await this.closeStep(this.fill(inputs.cycle_report, { outcome }), 'close interview cycle');
  }
  async openExperiment() {
    const r = this.expectOk(await this.model('gil_open', { kind: 'experiment' }), 'open experiment');
    return Rig.cycleRef(r);
  }
  async writeProjectFile(rel, content) {
    const target = resolve(this.root, rel);
    if (!inside(target, this.root) || target === this.root) throw new Error(`refusing to write outside the temporary project: ${rel}`);
    if (relative(this.root, target).split(sep)[0] === '.gil') throw new Error('refusing to write GIL records');
    await mkdir(resolve(target, '..'), { recursive: true });
    await writeFile(target, content);
  }
  readProjectFile(rel) { return readFile(resolve(this.root, rel)); }
  /** Hypothesis → Verify (plan + evidence) → Analysis → Outcome → Cycle Report, after Define closed. */
  async finishExperiment(inputs, item, contract) {
    await this.openStep('hypothesis', contract);
    await this.closeStep(inputs.hypothesis, 'close hypothesis');
    const verify = await this.openStep('verify', contract);
    const plan = await this.plan();
    this.need(typeof plan.document === 'string', 'document', 'no bound plan document in Verify');
    await this.writeProjectFile(`plan/${item}.md`, plan.document);
    await this.writeProjectFile(`evidence/${item}-result.md`, inputs.evidence);
    this.expectOk(await this.closeVerify(verify, `${inputs.verify_report}plan_document: plan/${item}.md\nevidence_files: evidence/${item}-result.md\n`), 'close verify');
    await this.openStep('analysis', contract);
    await this.closeStep(inputs.analysis, 'close analysis');
    const outcome = await this.openStep('outcome', contract);
    await this.closeStep(inputs.experiment_outcome, 'close experiment outcome');
    await this.closeStep(this.fill(inputs.cycle_report, { outcome }), 'close experiment cycle');
  }
  /** Typed Verify close for visual-authoring builds (the product feature set checked here). */
  closeVerify(step, report) {
    return this.model('gil_close', { report, expected_owner: { kind: 'step', ref: step }, visuals: { attachments: [], uses: [], declaration: { state: 'not_needed', reason: '문서 증거만 필요한 합성 예시 계약 시험' } } });
  }
}
