// Scale-controlled record generator. It walks only the Core's public MCP tools, in the order the
// specification allows; it never writes `.gil` and never forges a Report the Core would refuse.
// App-only plan decisions (`gil_plan_review` / `gil_plan_decide`) are synthetic harness input
// recorded as such — they prove a Core path, not a person's approval.
//
// After a failed Experiment the walk goes back to the branch point through whichever recovery
// path the Core under test offers (see RECOVERY). A product build (`product-safety`) refuses a
// model's bare `gil_revisit`; there the harness takes the existing App-only approval path
// `gil_revisit_review` → `gil_revisit_apply` with the digest and the private capability the Core
// itself returned. That is synthetic harness input too: no protection is switched off, no refusal
// is relaxed, and nothing here is a person's Monitor click.
//
// Every free-text Report field carries a short tag `ccbT-…`. After generation the measurement
// layer looks the tags up in each read response to see at which resolution a layer arrived
// (Context Model §2): e.g. a previous Cycle's Define problem must appear, its Verify result must not.

/**
 * A generation step that did not happen. `kind` says why, so a result never mixes them up:
 *   core_refusal — the Core answered and refused the call (a tool execution failure for this walk)
 *   harness      — the Core answered, but not with what the walk needs (no ref, no capability, …)
 */
export class GenerationError extends Error {
  constructor(step, detail, kind = 'core_refusal') { super(`${step}: ${detail}`); this.step = step; this.detail = detail; this.kind = kind; }
}
/**
 * A real condition under which this Core cannot make the transition at all — it reports the
 * recovery as unsupported on this platform, or offers no recovery path. Not a failed call.
 */
export class UnsupportedError extends GenerationError {
  constructor(step, detail) { super(step, detail, 'unsupported_condition'); }
}

/** How a failed Cycle is left for its branch point. Chosen from the tools the Core lists. */
export const RECOVERY = {
  /** `gil_revisit` called as a model would (builds without `product-safety`). */
  model_revisit: 'model_revisit',
  /** App-only `gil_revisit_review` → `gil_revisit_apply` with a synthetic harness approval. */
  app_approval: 'app_approval',
  /** The Core lists no recovery tool. */
  none: 'none',
};
export function recoveryPathFor(toolNames) {
  const has = (n) => toolNames.includes(n);
  if (has('gil_revisit_review') && has('gil_revisit_apply')) return RECOVERY.app_approval;
  return has('gil_revisit') ? RECOVERY.model_revisit : RECOVERY.none;
}

/** What each tagged layer should look like in gil_context; `bundle` = in at least one read call. */
export const LAYER_EXPECTATIONS = {
  prev_chain_report: { context: 'present', why: '이전 Chain은 Chain Report 해상도로 전달된다' },
  prev_chain_inner: { context: 'absent', why: '이전 Chain 내부 Cycle·Step은 기본 context에 펼치지 않는다' },
  prev_cycle_projection: { context: 'present', why: '이전 Cycle의 Define 목적·Outcome 교훈·Cycle Report 인계' },
  prev_cycle_inner: { context: 'absent', why: '이전 Cycle의 Hypothesis·Verify·Analysis 등 내부 Step은 펼치지 않는다' },
  failure_projection: { context: 'present', why: '누적 실패 Cycle은 Cycle 해상도로 모두 전달된다(Context Model §5 반복 실패)' },
  failure_inner: { context: 'absent', why: '실패 Cycle의 내부 Step은 펼치지 않는다' },
  current_step: { context: 'present', why: '현재 Cycle은 Step Report 해상도로 전달된다' },
  plan_current_item: { context: 'present', why: '이번 Experiment의 승인 항목' },
  plan_pending_item: { context: 'observe', bundle: 'present', why: '미실행 승인 항목 본문 — 어느 읽기 호출에서 오는지 관측' },
  plan_constraint: { context: 'observe', bundle: 'present', why: '승인 범위의 제약 — 어느 읽기 호출에서 오는지 관측' },
};

const CONTRACT = 'objective: 합성 벤치 기록의 이 Step을 수행한다\nnext_action: 이 Step의 Report를 적는다\ndone_when: Report가 기록된다\n';
const VISUALS = { attachments: [], uses: [], declaration: { state: 'not_needed', reason: '문서 증거만 필요한 합성 벤치 기록' } };
const STEP_ORDER = ['define', 'hypothesis', 'verify', 'analysis', 'outcome'];

export class TagBook {
  constructor() { this.rows = []; this.seen = new Set(); }
  add(layer, ...parts) {
    if (!LAYER_EXPECTATIONS[layer]) throw new Error(`unknown tag layer ${layer}`);
    const tag = `ccbT-${parts.join('-')}`;
    if (this.seen.has(tag)) throw new Error(`duplicate tag ${tag}`);
    this.seen.add(tag);
    this.rows.push({ tag, layer });
    return tag;
  }
}

/** Report field: plain `k: v` for one line, block form for several (the Core's own close_report form). */
export function field(name, lines) {
  const list = Array.isArray(lines) ? lines : [lines];
  if (list.length === 1) return `${name}: ${list[0]}\n`;
  return `${name}: |\n${list.map((l) => `  ${l}`).join('\n')}\n`;
}

/** n distinct, numbered sentences; the first carries the tag. Not filler: each line is a separate record. */
export function detailLines(tag, subject, n) {
  return Array.from({ length: n }, (_, i) => (i === 0 ? `${tag} ${subject}` : `관측 ${i + 1}/${n}: ${subject}의 ${i + 1}번째 확인 기록`));
}

const said = (r) => r?.structured?.said || r?.result?.content?.map((c) => c.text || '').join('\n') || '';
const problemOf = (r) => (r?.error ? `JSON-RPC error ${JSON.stringify(r.error).slice(0, 300)}` : `${said(r)}\n${r?.structured?.problem ?? ''}`.trim().slice(0, 400));
const stepRef = (r, step) => { const m = said(r).match(/step:C\d+\/S\d+/); if (!m) throw new GenerationError(step, 'receipt names no step ref'); return m[0]; };
const cycleNo = (r, step) => { const m = said(r).match(/cycle:C(\d+)/); if (!m) throw new GenerationError(step, 'receipt names no cycle ref'); return Number(m[1]); };

/**
 * Walk the Core to `scale`. `g` supplies: root, model(name,args), app(name,args),
 * writeProjectFile(rel, text), typedVerify, locale, recovery (a RECOVERY value),
 * modelRevisitBlockedByProfile, onSecret. Returns counters, the tag book, the failed attempts
 * and how each one was recovered. No capability token is ever part of the returned value.
 */
export async function generate(g, scale) {
  const tags = new TagBook();
  const counts = { chains_closed: 0, cycles_closed: 0, failures: 0, revisits: 0, plan_approvals: 0, recovery_reviews: 0, recovery_approvals: 0 };
  const ok = (r, step) => { if (!r?.ok) throw new GenerationError(step, problemOf(r)); return r; };
  const call = async (step, name, args) => ok(await g.model(name, { project_root: g.root, ...args }), step);
  let scope = null;

  async function step(kind, report, label) {
    const opened = await call(`${label}: open ${kind}`, 'gil_open', { kind, contract: CONTRACT });
    const ref = stepRef(opened, `${label}: open ${kind}`);
    if (report !== null) await call(`${label}: close ${kind}`, 'gil_close', { report });
    return ref;
  }

  async function approve(label) {
    const review = ok(await g.app('gil_plan_review', { scope_id: scope }), `${label}: plan review`);
    const token = review.meta?.['gil/planApprovalToken'];
    const p = review.structured?.proposal;
    if (typeof token !== 'string' || !p) throw new GenerationError(`${label}: plan review`, 'no proposal or approval capability');
    g.onSecret?.(token);
    ok(await g.app('gil_plan_decide', { scope_id: scope, revision: p.revision, digest: p.digest, token, decision: 'approve' }), `${label}: plan decide`);
    counts.plan_approvals += 1;
  }

  /** Question → Interpretation → Synthesis (proposal, synthetic approval) → Outcome → Cycle close. */
  async function interview(label, { items, constraintTag, chainVerdict = null, innerLayer, projLayer, prefix }) {
    const basis = [];
    basis.push(await step('question', `question: ${tags.add(innerLayer, prefix, 'question')} 어떤 합성 작업인가\nchoices: 항목별 확인 / 비교\nresponse: 합성 자료로 항목별 확인 기록을 남긴다\n`, label));
    basis.push(await step('interpretation', `interpretation: ${tags.add(innerLayer, prefix, 'interpretation')} 항목마다 파일 증거를 남긴다\nunresolved: 없음\n`, label));
    const synRef = await step('synthesis', null, label);
    const draft = {
      scope: { goal: '합성 벤치 기록의 항목별 확인', in_scope: '제공된 합성 자료', out_of_scope: '실제 데이터·개인정보', constraints: `${constraintTag} 외부 네트워크를 쓰지 않는다`, success_criteria: '항목마다 파일 증거가 남는다' },
      items,
      synthesis: { statement: `${tags.add(innerLayer, prefix, 'statement')} 합성 항목을 순서대로 확인한다`, basis_refs: basis.join('\n'), exploration_checklist: items.map((i) => i.id).join('\n'), ...(chainVerdict ? { chain_verdict: chainVerdict } : {}) },
    };
    await call(`${label}: plan propose`, 'gil_plan_propose', { draft });
    await approve(label);
    const read = await call(`${label}: plan read`, 'gil_plan_read', {});
    if (typeof read.structured?.close_report !== 'string') throw new GenerationError(`${label}: plan read`, 'no close_report after approval');
    await call(`${label}: close synthesis`, 'gil_close', { report: read.structured.close_report });
    const action = chainVerdict ? 'close_chain' : 'open_child';
    const outcomeRef = await step('outcome', `verdict: success\nlesson: ${tags.add(projLayer, prefix, 'lesson')} 합의한 범위를 기록했다\nsynthesis_ref: ${synRef}\nnext_direction.action: close_cycle\nnext_direction.reason: 승인된 범위로 진행한다\n`, label);
    await call(`${label}: close cycle`, 'gil_close', { report: `verdict: success\noutcome_ref: ${outcomeRef}\n${field('handoff_summary', `${tags.add(projLayer, prefix, 'handoff')} 합성 범위를 승인받았다`)}next_direction.action: ${action}\nnext_direction.reason: ${chainVerdict ? '승인된 종료 제안으로 Chain을 닫는다' : '승인 항목을 순서대로 실행한다'}\n` });
    counts.cycles_closed += 1;
    return { synRef, cycle: Number(basis[0].match(/step:C(\d+)\//)[1]) };
  }

  /**
   * One Experiment. `closeSteps` < 5 leaves the Cycle open with the next Step open (current Cycle);
   * otherwise the Cycle closes with `verdict` (failure → revisit `target`).
   */
  async function experiment(label, { prefix, projLayer, innerLayer, verdict = 'success', target = null, closeSteps = 5, lines = 1 }) {
    const opened = await call(`${label}: open experiment`, 'gil_open', { kind: 'experiment' });
    const n = cycleNo(opened, `${label}: open experiment`);
    const view = await call(`${label}: plan read`, 'gil_plan_read', {});
    const item = view.structured?.current_item?.id ?? 'unbound';
    const document = view.structured?.document;
    const current = closeSteps < 5;
    const L = current ? lines : 1;
    const t = (layer, what) => tags.add(layer, prefix, what);
    const reports = {
      define: () => field('problem', detailLines(t(projLayer, 'define'), `항목 ${item}을 합성 자료로 확인한다`, L)) + field('success_condition', `evidence 파일에 항목 ${item} 결과가 저장된다`),
      hypothesis: () => field('hypothesis', detailLines(t(innerLayer, 'hypothesis'), `합성 자료만으로 항목 ${item}을 확인할 수 있다`, L)) + field('rationale', '자료가 함께 제공됐다') + field('guardrail', `${t(innerLayer, 'guardrail')} 외부 데이터를 쓰지 않는다`),
      verify: async (ref) => {
        if (typeof document !== 'string') throw new GenerationError(`${label}: verify`, 'plan_read returned no document for the bound item');
        await g.writeProjectFile('plan/approved-plan.md', document);
        const evidence = `evidence/C${n}-${item}.md`;
        await g.writeProjectFile(evidence, `# 합성 관측 C${n}\n\n항목 ${item}: 합성 벤치 기록 (시험 전용)\n`);
        const report = field('execution', `합성 자료를 확인하고 결과를 저장했다`) + field('result', detailLines(t(innerLayer, 'verify'), `항목 ${item} 결과 파일을 남겼다`, L)) + 'plan_document: plan/approved-plan.md\n' + `evidence_files: ${evidence}\n`;
        return g.typedVerify ? { report, expected_owner: { kind: 'step', ref }, visuals: VISUALS } : { report };
      },
      analysis: () => {
        const fit = verdict === 'success' ? 'yes' : 'no';
        return `hypothesis_fit: ${fit}\nproblem_solved: ${fit}\nsuccess_condition_met: ${fit}\nguardrail_triggered: no\n` + field('interpretation', detailLines(t(innerLayer, 'analysis'), verdict === 'success' ? '합성 결과가 가설과 일치했다' : '합성 결과가 성공 조건에 닿지 못했다', L));
      },
      outcome: () => `verdict: ${verdict}\nlesson: ${t(projLayer, 'lesson')} ${verdict === 'success' ? `항목 ${item}을 파일 증거로 확인했다` : `항목 ${item}의 이 접근은 성공 조건에 닿지 못했다`}\nnext_direction.action: close_cycle\nnext_direction.reason: 이 Cycle을 판정한다\n`,
    };
    let outcomeRef = null;
    for (let i = 0; i < STEP_ORDER.length; i += 1) {
      const kind = STEP_ORDER[i];
      if (i === closeSteps) { await step(kind, null, label); return { cycle: n, item, open_step: kind }; }
      if (kind === 'verify') {
        const vopen = await call(`${label}: open verify`, 'gil_open', { kind, contract: CONTRACT });
        const args = await reports.verify(stepRef(vopen, `${label}: open verify`));
        await call(`${label}: close verify`, 'gil_close', args);
      } else {
        const ref = await step(kind, reports[kind](), label);
        if (kind === 'outcome') outcomeRef = ref;
      }
    }
    const direction = verdict === 'success'
      ? 'next_direction.action: open_child\nnext_direction.reason: 남은 항목을 이어간다\n'
      : `next_direction.action: revisit\nnext_direction.target_cycle_ref: ${target}\nnext_direction.reason: ${t(projLayer, 'reason')} 이 접근을 버리고 분기점에서 다시 시도한다\n`;
    await call(`${label}: close cycle`, 'gil_close', { report: `verdict: ${verdict}\noutcome_ref: ${outcomeRef}\n${field('handoff_summary', `${t(projLayer, 'handoff')} 항목 ${item} ${verdict === 'success' ? '완료' : '시도 실패'}`)}${direction}` });
    counts.cycles_closed += 1;
    if (verdict === 'failure') counts.failures += 1;
    return { cycle: n, item };
  }

  /**
   * Leave the failed Cycle `from` for the branch point `target`. The App path uses only what the
   * Core's own review returned: its digest and its private capability. The capability is handed to
   * `onSecret` before anything else can fail, and is never stored or returned.
   */
  async function recover(label, from, target) {
    const path = g.recovery ?? RECOVERY.model_revisit;
    const expected = { from: `cycle:C${from}`, target: `cycle:C${target}` };
    if (path === RECOVERY.app_approval) {
      const at = `${label}: revisit review`;
      const review = ok(await g.app('gil_revisit_review', { scope_id: scope }), at);
      const token = review.meta?.['gil/revisitApprovalToken'];
      if (typeof token === 'string') g.onSecret?.(token);
      const view = review.structured ?? {};
      if (view.supported === false) throw new UnsupportedError(at, String(view.reason ?? 'the Core reports approved recovery as unsupported here'));
      if (view.state !== 'awaiting_approval' || typeof view.digest !== 'string' || typeof token !== 'string') {
        throw new GenerationError(at, `review is not an approvable plan (state ${JSON.stringify(view.state ?? null)}, digest ${typeof view.digest}, capability ${typeof token === 'string' ? 'present' : 'absent'})`, 'harness');
      }
      if (view.from !== expected.from || view.target !== expected.target) {
        throw new GenerationError(at, `review plans ${view.from} → ${view.target}, the walk expected ${expected.from} → ${expected.target}`, 'harness');
      }
      counts.recovery_reviews += 1;
      const applied = ok(await g.app('gil_revisit_apply', { scope_id: scope, digest: view.digest, token }), `${label}: revisit apply`);
      if (applied.structured?.state !== 'completed') throw new GenerationError(`${label}: revisit apply`, `recovery state is ${JSON.stringify(applied.structured?.state ?? null)}, not completed`, 'harness');
      counts.recovery_approvals += 1;
      const changes = { create: 0, replace: 0, preserve: 0 };
      for (const c of Array.isArray(view.changes) ? view.changes : []) if (c?.action in changes) changes[c.action] += 1;
      return {
        path, ...expected, changes, policy: view.policy ?? null, preserved_key: typeof view.pending_state_sha256 === 'string' ? view.pending_state_sha256 : null,
        // What the Core wrote down as the source of this approval. In this bench the App input came
        // from the harness, so `monitor_button` here is not evidence of a person's click.
        core_approval_source: applied.structured.approval_source ?? null,
      };
    }
    if (path === RECOVERY.model_revisit) {
      const at = `${label}: gil_revisit`;
      const r = await g.model('gil_revisit', { project_root: g.root });
      if (!r?.ok) {
        // Only a profile that blocks a model's revisit by design, on a Core without the approval
        // tools, is an unsupported condition. Anywhere else the refusal is a failed call.
        if (g.modelRevisitBlockedByProfile) throw new UnsupportedError(at, problemOf(r));
        throw new GenerationError(at, problemOf(r));
      }
      return { path, ...expected, changes: null, policy: null, preserved_key: null, core_approval_source: null };
    }
    throw new UnsupportedError(`${label}: recovery`, 'this Core lists neither gil_revisit nor gil_revisit_review/gil_revisit_apply');
  }

  // ── 1. Project start: the first Chain's Opening Interview is open. ─────────────────────────
  const started = await call('start', 'gil_start', g.locale ? { locale: g.locale } : {});
  scope = (said(started).match(/project:[0-9a-f]{64}/) ?? JSON.stringify(started.structured ?? {}).match(/project:[0-9a-f]{64}/))?.[0] ?? null;
  if (!scope) throw new GenerationError('start', 'start receipt names no Monitor scope_id');

  // ── 2. Previous Chains: each one Opening Interview, one Experiment, Closing Interview, Chain Report.
  for (let p = 1; p <= scale.chains; p += 1) {
    const label = `previous chain ${p}`;
    const pre = `pc${p}`;
    const items = [{ id: `P${p}`, question: `이전 Chain ${p}의 합성 항목을 확인한다`, done_when: `evidence 파일에 이전 Chain ${p} 결과가 저장된다` }];
    const constraint = tags.add('prev_chain_inner', pre, 'constraint');
    await interview(`${label} opening`, { items, constraintTag: constraint, innerLayer: 'prev_chain_inner', projLayer: 'prev_chain_inner', prefix: `${pre}-open` });
    await experiment(`${label} experiment`, { prefix: `${pre}-e1`, projLayer: 'prev_chain_inner', innerLayer: 'prev_chain_inner' });
    const closingOpen = await call(`${label}: open closing interview`, 'gil_open', { kind: 'interview' });
    cycleNo(closingOpen, `${label}: open closing interview`);
    const { synRef } = await interview(`${label} closing`, { items, constraintTag: tags.add('prev_chain_inner', pre, 'closing-constraint'), chainVerdict: 'success', innerLayer: 'prev_chain_inner', projLayer: 'prev_chain_inner', prefix: `${pre}-close` });
    await call(`${label}: close chain`, 'gil_close', { report: `verdict: success\nclosing_synthesis_ref: ${synRef}\nhandoff_summary: ${tags.add('prev_chain_report', pre, 'chain-handoff')} 이전 Chain ${p}의 합성 항목을 파일 증거와 함께 완료했다\n` });
    counts.chains_closed += 1;
    await call(`${label}: open next chain`, 'gil_open', { kind: 'chain' });
  }

  // ── 3. Current Chain Opening Interview approving cycles + plan_items items. ───────────────
  const total = scale.cycles + scale.plan_items;
  const items = Array.from({ length: total }, (_, i) => {
    const id = `I${i + 1}`;
    const isCurrent = i === scale.cycles;
    const layer = i < scale.cycles ? 'prev_cycle_inner' : isCurrent ? 'plan_current_item' : 'plan_pending_item';
    return { id, question: `${tags.add(layer, 'item', id)} 합성 항목 ${id}을 확인한다`, done_when: `evidence 파일에 항목 ${id} 결과가 저장된다` };
  });
  const opening = await interview('current chain opening', { items, constraintTag: tags.add('plan_constraint', 'plan', 'constraint'), innerLayer: 'prev_cycle_inner', projLayer: 'prev_cycle_projection', prefix: 'cur-open' });

  // ── 4. Closed successful Experiments of the current Chain. ───────────────────────────────
  let branchPoint = null;
  for (let k = 1; k <= scale.cycles; k += 1) {
    const done = await experiment(`closed experiment ${k}`, { prefix: `k${k}`, projLayer: 'prev_cycle_projection', innerLayer: 'prev_cycle_inner' });
    branchPoint = done.cycle;
  }
  // The Opening Interview is the branch point when no Experiment has closed yet.
  if (branchPoint === null) branchPoint = opening.cycle;

  // ── 5. Accumulated failures on the same lineage: fail → revisit the branch point → retry. ──
  const failed = [];
  const recoveries = [];
  for (let f = 1; f <= scale.failures; f += 1) {
    const done = await experiment(`failed experiment ${f}`, { prefix: `f${f}`, projLayer: 'failure_projection', innerLayer: 'failure_inner', verdict: 'failure', target: `cycle:C${branchPoint}` });
    failed.push({ attempt: f, cycle: done.cycle, item: done.item, prefix: `f${f}` });
    recoveries.push(await recover(`failed experiment ${f}`, done.cycle, branchPoint));
    counts.revisits += 1;
  }

  // ── 6. The current Experiment: `steps` closed Steps, the next one open. ─────────────────────
  const current = await experiment('current experiment', { prefix: 'cur', projLayer: 'current_step', innerLayer: 'current_step', closeSteps: scale.steps, lines: scale.report_lines });
  return { scope, counts, tags: tags.rows, current, branch_point: branchPoint, failed, recoveries };
}
