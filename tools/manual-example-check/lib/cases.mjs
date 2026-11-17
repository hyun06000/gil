// Reviewed cases. Each case is an explicit, fixed list of tool calls over one fresh temporary
// project; document excerpts enter only as parsed payload values (lib/extract.mjs), never as code.
// A case records every passed check; the first failed check stops the case.
import { fillTemplate } from './extract.mjs';
import { Rig, said, blocks } from './rig.mjs';

const PLAN_STATE = '승인 계획:';
/** Key-order-independent JSON, for comparing what Core returns with what was sent. */
const canon = (v) => JSON.stringify(v, (k, x) => (x && typeof x === 'object' && !Array.isArray(x) ? Object.fromEntries(Object.keys(x).sort().map((key) => [key, x[key]])) : x));
const ids = (list) => (list ?? []).map((i) => i.id ?? i.item_id);

/** The documented wrapper with only its declared placeholders replaced by issued values. */
function wrapper(rig, ctx, basis) {
  const { input, verbatimPaths } = fillTemplate(ctx.ex['WF-PROPOSE-WRAPPER'].payload, { project_root: rig.root, issued_basis_refs: basis });
  return { input, verbatimPaths };
}

/** A derived proposal: item text verbatim from the document, everything else harness-supplied. */
function derivedDraft(ctx, items, basis) {
  return { scope: { ...ctx.inputs.scope }, items: items.map((i) => ({ ...i })), synthesis: { statement: ctx.inputs.statement, basis_refs: basis, exploration_checklist: items.map((i) => i.id).join('\n') } };
}

async function approvedPlan(rig, ctx, items) {
  const iv = await rig.interviewToSynthesis(ctx.inputs);
  const p = rig.expectOk(await rig.model('gil_plan_propose', { draft: derivedDraft(ctx, items, iv.basis) }), 'propose derived plan');
  await rig.approveSynthetic(p.structured.proposal.revision);
  await rig.closeApprovedInterview(ctx.inputs, iv.synthesis);
  return iv;
}

async function runItem(rig, ctx, defineReport, contract = ctx.inputs.contract) {
  await rig.openExperiment();
  const item = (await rig.plan()).current_item;
  await rig.openStep('define', contract);
  await rig.closeStep(defineReport, 'close define');
  await rig.finishExperiment(ctx.inputs, item.id, contract);
  return item;
}

export const CASES = [
  {
    id: 'CX-00',
    title: 'The Core under test serves the same example text as the reviewed documents',
    examples: 'served',
    async run(rig, ctx) {
      const instructions = rig.initialize?.instructions ?? '';
      for (const row of ctx.served) {
        if (row.via === 'instructions') {
          rig.need(instructions.includes(row.excerpt), `served ${row.id}`, `initialize.instructions does not contain the excerpt (stale binary, or the MCP surface changed the text) — rebuild or re-review ${row.id}`);
        } else {
          const topic = row.via.slice('topic:'.length);
          const r = rig.expectOk(await rig.call('model', 'gil_help', { topic }), `gil_help ${topic}`);
          rig.need(said(r).includes(row.excerpt.trimEnd()), `served ${row.id}`, `gil_help(${topic}) does not contain the excerpt (stale binary or a projection rewrote it)`);
        }
        rig.pass(`served ${row.id}`, row.via);
      }
    },
  },
  {
    id: 'CX-01',
    title: 'Question·Interpretation, then the documented wrapper with the issued basis refs',
    examples: ['WF-PROPOSE-WRAPPER', 'WF-BASIS-REPLACE', 'WF-BASIS-RULE'],
    async run(rig, ctx) {
      await rig.bootstrap();
      const iv = await rig.interviewToSynthesis(ctx.inputs);
      const { input, verbatimPaths } = wrapper(rig, ctx, iv.basis);
      rig.pass('template filled', `placeholders: project_root, draft.synthesis.basis_refs; sent verbatim: ${verbatimPaths.join(', ')}`);
      const r = rig.expectOk(await rig.call('model', 'gil_plan_propose', input), 'documented wrapper accepted');
      const p = r.structured.proposal;
      rig.need(p?.revision === 1 && p.receipt === null, 'revision 1 waits', `proposal ${JSON.stringify({ revision: p?.revision, receipt: p?.receipt })}`);
      rig.pass('documented wrapper accepted', `revision 1, waiting for a decision; issued refs ${iv.basis.replace('\n', ' + ')}`);
      const literal = ctx.ex['WF-PROPOSE-WRAPPER'].payload.placeholders.find((x) => x.with === 'issued_basis_refs').literal;
      rig.pass('observation: literal refs coincide', literal === iv.basis ? 'the document literal equals the refs a first Interview issues; substitution is only visible from the second Interview on (CX-05)' : 'the document literal differs from the issued refs');
      ctx.observations.literalRefsCoincide = literal === iv.basis;
      const plan = await rig.plan();
      rig.need(plan.close_report === null, 'no close_report before a decision', 'close_report offered before any decision');
      rig.pass('no close_report before a decision');
    },
  },
  {
    id: 'CX-02',
    title: 'Schema and basis_refs refusals leave the record and the position intact',
    examples: ['WF-PROPOSE-WRAPPER', 'WF-PROPOSE-SHAPE', 'WF-PROPOSE-FIELDS', 'WF-BASIS-RULE'],
    async run(rig, ctx) {
      await rig.bootstrap();
      const iv = await rig.interviewToSynthesis(ctx.inputs);
      const base = () => structuredClone(wrapper(rig, ctx, iv.basis).input);
      const mutate = (fn) => { const v = base(); fn(v); return v; };
      const schema = [
        ['draft fields at the top level', mutate((v) => { Object.assign(v, v.draft); delete v.draft; }), ["'draft' is missing"]],
        ['extra top-level field', mutate((v) => { v.note = 'x'; }), []],
        ['extra field in draft', mutate((v) => { v.draft.note = 'x'; }), []],
        ['extra field in scope', mutate((v) => { v.draft.scope.note = 'x'; }), []],
        ['extra field in an item', mutate((v) => { v.draft.items[0].note = 'x'; }), []],
        ['missing scope field', mutate((v) => { delete v.draft.scope.constraints; }), []],
        ['non-string synthesis value', mutate((v) => { v.draft.synthesis.statement = ['x']; }), []],
      ];
      const core = [
        ['empty scope field', mutate((v) => { v.draft.scope.constraints = ''; }), [PLAN_STATE, '범위의 모든 칸']],
        ['zero items', mutate((v) => { v.draft.items = []; v.draft.synthesis.exploration_checklist = ''; }), [PLAN_STATE, '1~64']],
        ['65 items', mutate((v) => { v.draft.items = Array.from({ length: 65 }, (_, i) => ({ id: `I${i + 1}`, question: '…', done_when: '…' })); v.draft.synthesis.exploration_checklist = v.draft.items.map((i) => i.id).join('\n'); }), [PLAN_STATE, '1~64']],
        ['checklist out of item order', mutate((v) => { v.draft.items.push({ id: 'B', question: '…', done_when: '…' }); v.draft.synthesis.exploration_checklist = 'B\nA'; }), [PLAN_STATE]],
        ['basis: comma list', mutate((v) => { v.draft.synthesis.basis_refs = iv.basis.replace('\n', ', '); }), ['$.draft.synthesis.basis_refs', 'line 1']],
        ['basis: prose line', mutate((v) => { v.draft.synthesis.basis_refs = `${iv.question}\nQuestion에서 확인함`; }), ['$.draft.synthesis.basis_refs', 'line 2']],
        ['basis: Cycle ref', mutate((v) => { v.draft.synthesis.basis_refs = `${iv.question}\ncycle:C1`; }), ['$.draft.synthesis.basis_refs', 'line 2']],
        ['basis: open step (the Synthesis itself)', mutate((v) => { v.draft.synthesis.basis_refs = `${iv.question}\n${iv.synthesis}`; }), ['$.draft.synthesis.basis_refs', 'line 2']],
        ['basis: duplicate', mutate((v) => { v.draft.synthesis.basis_refs = `${iv.question}\n${iv.question}`; }), ['$.draft.synthesis.basis_refs', 'line 2']],
        ['basis: step of a Cycle that is not this Interview', mutate((v) => { v.draft.synthesis.basis_refs = `${iv.question}\nstep:C9/S1`; }), ['$.draft.synthesis.basis_refs', 'line 2']],
        ['basis: empty', mutate((v) => { v.draft.synthesis.basis_refs = ''; }), ['$.draft.synthesis.basis_refs']],
        ['approved written by the agent', mutate((v) => { v.draft.synthesis.approved = 'yes'; }), [PLAN_STATE, 'approved']],
      ];
      await rig.unchanged('every refusal leaves record·context·plan intact', async () => {
        for (const [name, input, markers] of schema) {
          rig.expectSchemaError(await rig.call('model', 'gil_plan_propose', input), `schema: ${name}`, markers);
          rig.still(`schema: ${name}`);
          rig.pass(`schema: ${name}`, 'JSON-RPC -32602');
        }
        for (const [name, input, markers] of core) {
          rig.expectRefused(await rig.call('model', 'gil_plan_propose', input), `core: ${name}`, markers);
          rig.still(`core: ${name}`);
          rig.pass(`core: ${name}`, `refused with ${markers.join(' + ') || 'a Core reason'}`);
        }
      });
      rig.pass('every refusal leaves record·context·plan intact', 'record bytes, gil_context text and gil_plan_read identical');
      rig.need((await rig.plan()).proposal === null, 'no proposal stored', 'a refused proposal was stored');
      rig.expectOk(await rig.call('model', 'gil_plan_propose', base()), 'valid input still accepted');
      rig.pass('valid input still accepted', 'refusals did not damage the open Synthesis');
    },
  },
  {
    id: 'CX-03',
    title: 'No Synthesis close before the decision; after a synthetic approval only the exact close_report',
    examples: ['WF-PROPOSE-WRAPPER', 'WF-TABLE-SYNTH-WAIT', 'WF-TABLE-SYNTH-APPROVED', 'WF-APPROVE-CLOSE', 'WF-CLOSE-REPORT-OWNER', 'WF-RECOVER-PLAN-STATE'],
    async run(rig, ctx) {
      await rig.bootstrap();
      const iv = await rig.interviewToSynthesis(ctx.inputs);
      const { input } = wrapper(rig, ctx, iv.basis);
      rig.expectOk(await rig.call('model', 'gil_plan_propose', input), 'propose');
      const s = input.draft.synthesis;
      const handWritten = `statement: ${s.statement}\nbasis_refs: |\n  ${s.basis_refs.split('\n').join('\n  ')}\nexploration_checklist: ${s.exploration_checklist}\n`;
      await rig.unchanged('waiting close refused, nothing moved', async () => {
        const waiting = rig.expectRefused(await rig.model('gil_close', { report: handWritten }), 'close before decision', [PLAN_STATE, 'gil_plan_read']);
        rig.need(!blocks(waiting).includes('입력 오류를 고쳐'), 'close before decision', 'a state refusal must not read as an input error');
        rig.still('close before decision');
        rig.pass('close before decision', 'refused as an approval state that names gil_plan_read');
        rig.expectRefused(await rig.model('gil_close', { report: `${handWritten}approved: yes\n` }), 'bare approved: yes', [PLAN_STATE]);
        rig.pass('bare approved: yes', 'refused');
      });
      rig.pass('waiting close refused, nothing moved');
      const receipt = await rig.approveSynthetic(1);
      ctx.observations.receiptSource = receipt.source;
      rig.pass('synthetic approval recorded', `receipt.source=${receipt.source}: synthetic harness input through the App-only path; no person pressed a Monitor button, so this is protocol evidence, not human approval`);
      const report = (await rig.plan()).close_report;
      rig.need(typeof report === 'string' && report.includes('plan_revision') && report.includes('plan_digest'), 'close_report bound', 'close_report lacks revision/digest');
      await rig.unchanged('tampered close_report refused', async () => {
        rig.expectRefused(await rig.model('gil_close', { report: report.replace(s.statement, `${s.statement} (수정)`) }), 'tampered close_report');
      });
      rig.pass('tampered close_report refused', 'record and position unchanged');
      rig.expectOk(await rig.model('gil_close', { report }), 'exact close_report');
      rig.pass('exact close_report closes the Synthesis');
      const outcome = await rig.openStep('outcome', ctx.inputs.contract);
      await rig.unchanged('close_report is not the Outcome Report', async () => {
        rig.expectRefused(await rig.model('gil_close', { report }), 'close_report reused for Outcome');
      });
      rig.pass('close_report is not the Outcome Report', 'refused; Outcome still open');
      await rig.closeStep(rig.fill(ctx.inputs.interview_outcome, { synthesis: iv.synthesis }), 'close outcome');
      await rig.closeStep(rig.fill(ctx.inputs.cycle_report, { outcome }), 'close interview cycle');
      const plan = await rig.plan();
      rig.need(plan.active_revision === 1 && plan.pending?.length === 1, 'executable after Cycle close', `active_revision ${plan.active_revision}, pending ${JSON.stringify(ids(plan.pending))}`);
      rig.pass('plan executable after the Interview Cycle closed', 'active revision 1, pending [A]');
    },
  },
  {
    id: 'CX-04',
    title: 'Manual items A/B/C → Experiment bound to A → Manual Define and contract → documented Verify paths',
    examples: ['MAN-CLARIFY-SHAPE', 'MAN-CLARIFY-GOOD', 'WF-APPROVE-CLOSE', 'WF-ONE-ITEM-BINDING', 'WF-DEFINE-SMALLEST', 'MAN-DEFINE-GOOD', 'MAN-OPEN-GOOD', 'WF-DOCUMENT-VERIFY'],
    async run(rig, ctx) {
      await rig.bootstrap();
      const items = ctx.ex['MAN-CLARIFY-GOOD'].payload.items;
      const iv = await rig.interviewToSynthesis(ctx.inputs);
      const draft = derivedDraft(ctx, items, iv.basis);
      rig.need(Object.keys(draft).join() === 'scope,items,synthesis', 'no new field', 'derived draft grew a field');
      const p = rig.expectOk(await rig.model('gil_plan_propose', { draft }), 'manual items accepted');
      rig.pass('manual items accepted', `items ${ids(items).join(',')} verbatim; scope/statement/answers harness-supplied`);
      await rig.approveSynthetic(p.structured.proposal.revision);
      await rig.closeApprovedInterview(ctx.inputs, iv.synthesis);
      rig.need((await rig.plan()).document === null, 'no document before binding', 'document exists before an Experiment opened');
      await rig.openExperiment();
      const bound = await rig.plan();
      rig.need(bound.current_item?.id === 'A' && bound.current_item.question === items[0].question, 'binds first pending item', `current_item ${JSON.stringify(bound.current_item)}`);
      rig.need(typeof bound.document === 'string', 'document after binding', 'no document after the Experiment opened');
      rig.pass('Experiment binds item A', 'current_item.question is the Manual text verbatim; document available');
      const contract = ctx.ex['MAN-OPEN-GOOD'].payload.text;
      await rig.openStep('define', contract);
      rig.pass('Manual contract opens Define', 'objective/next_action/done_when verbatim as gil_open.contract');
      await rig.closeStep(ctx.ex['MAN-DEFINE-GOOD'].payload.text, 'Manual Define closes');
      rig.need((await rig.plan()).current_item?.id === 'A', 'binding kept through Define', 'current_item moved');
      rig.pass('Manual Define closes while bound to A', 'problem/success_condition verbatim; semantic fit to item A is not judged by Core');
      await rig.openStep('hypothesis', contract);
      await rig.closeStep(ctx.inputs.hypothesis, 'close hypothesis');
      const verify = await rig.openStep('verify', contract);
      const doc = (await rig.plan()).document;
      const spans = ctx.ex['WF-DOCUMENT-VERIFY'].payload;
      const planPath = spans.plan_document.slice('plan_document: '.length);
      const evidencePath = spans.evidence_files.slice('evidence_files: '.length);
      await rig.writeProjectFile(planPath, doc);
      await rig.writeProjectFile(evidencePath, ctx.inputs.evidence);
      await rig.unchanged('documented refusals', async () => {
        rig.expectRefused(await rig.closeVerify(verify, `${ctx.inputs.verify_report}${spans.plan_document}\nevidence_files: ${planPath}\n`), 'plan file alone');
        rig.pass('plan file alone refused');
        await rig.writeProjectFile(planPath, `${doc}\n(변경)`);
        rig.expectRefused(await rig.closeVerify(verify, `${ctx.inputs.verify_report}${spans.plan_document}\n${spans.evidence_files}\n`), 'plan bytes differ');
        rig.pass('plan bytes that differ refused');
        await rig.writeProjectFile(planPath, doc);
      });
      rig.pass('documented refusals', 'record and position unchanged');
      rig.expectOk(await rig.closeVerify(verify, `${ctx.inputs.verify_report}${spans.plan_document}\n${spans.evidence_files}\n`), 'documented Verify accepted');
      rig.pass('documented Verify accepted', `${spans.plan_document} + ${spans.evidence_files}`);
      await rig.openStep('analysis', contract);
      await rig.closeStep(ctx.inputs.analysis, 'close analysis');
      const outcome = await rig.openStep('outcome', contract);
      await rig.closeStep(ctx.inputs.experiment_outcome, 'close outcome');
      await rig.closeStep(rig.fill(ctx.inputs.cycle_report, { outcome }), 'close experiment cycle');
      const after = await rig.plan();
      const executed = (after.executed_cycles ?? []).map((e) => `${e.item_id}@${e.revision}`);
      rig.need(executed.join() === 'A@1' && ids(after.pending).join() === 'B,C', 'ledger', `executed ${executed}, pending ${ids(after.pending)}`);
      rig.pass('ledger after the Cycle', 'executed [A@1], pending [B,C]');
    },
  },
  {
    id: 'CX-05',
    title: '[A,B,C] → A executed → [A,D,B,C] with copied items; omission, redefinition and literal refs refused',
    examples: ['SPEC-REORDER', 'WF-EXTEND', 'SPEC-EXTEND-REFUSALS', 'WF-ONE-ITEM-BINDING', 'MAN-CLARIFY-GOOD', 'WF-BASIS-REPLACE', 'WF-BASIS-RULE', 'WF-PROPOSE-WRAPPER', 'SPEC-BASIS-JSON'],
    async run(rig, ctx) {
      await rig.bootstrap();
      const spec = ctx.ex['SPEC-REORDER'].payload;
      const text = Object.fromEntries(ctx.ex['MAN-CLARIFY-GOOD'].payload.items.map((i) => [i.id, i]));
      text.D = ctx.inputs.item_D;
      const first = spec.first_approved.map((id) => text[id]);
      rig.need(first.every(Boolean), 'item texts', `no text for ${spec.first_approved}`);
      await approvedPlan(rig, ctx, first);
      const executedId = spec.first_approved[0];
      const ran = await runItem(rig, ctx, ctx.ex['MAN-DEFINE-GOOD'].payload.text);
      rig.need(ran.id === executedId, 'first item executed', `ran ${ran.id}`);
      rig.pass(`[${spec.first_approved}] approved, ${executedId} executed`);
      rig.expectOk(await rig.model('gil_open', { kind: 'interview' }), 'open extension interview');
      const iv = await rig.interviewToSynthesis(ctx.inputs);
      const active = (await rig.plan()).active_plan;
      rig.need(canon(active?.items) === canon(first), 'active_plan verbatim', `active_plan.items ${canon(active?.items).slice(0, 400)} differ from the approved items ${canon(first).slice(0, 400)}`);
      rig.pass('active_plan returns every approved item verbatim', `revision ${active.revision}`);
      const proposal = (order, mutateItems = (x) => x) => {
        const items = mutateItems(order.map((id) => structuredClone(id === 'D' ? ctx.inputs.item_D : active.items.find((i) => i.id === id))));
        return { draft: { scope: structuredClone(active.scope), items, synthesis: { statement: ctx.inputs.statement, basis_refs: iv.basis, exploration_checklist: items.map((i) => i.id).join('\n') } } };
      };
      const literalWorkflow = ctx.ex['WF-PROPOSE-WRAPPER'].payload.placeholders.find((x) => x.with === 'issued_basis_refs').literal;
      const literalSpec = ctx.ex['SPEC-BASIS-JSON'].payload.value;
      await rig.unchanged('extension refusals leave everything intact', async () => {
        for (const [name, literal] of [['workflow literal refs', literalWorkflow], ['spec literal refs', literalSpec]]) {
          const input = proposal(spec.second_approved);
          input.draft.synthesis.basis_refs = literal;
          rig.expectRefused(await rig.model('gil_plan_propose', input), name, ['$.draft.synthesis.basis_refs', 'line 1', 'line 2']);
          rig.still(name);
          rig.pass(`${name} refused in a later Interview`, `${literal.replace('\n', ' + ')} vs issued ${iv.basis.replace('\n', ' + ')}`);
        }
        const dropped = spec.second_approved.filter((id) => id !== 'B');
        rig.expectRefused(await rig.model('gil_plan_propose', proposal(dropped)), 'omission', [PLAN_STATE, '누락', 'B']);
        rig.still('omission');
        rig.pass('omitting an approved item refused', `[${dropped}] names B`);
        rig.expectRefused(await rig.model('gil_plan_propose', proposal(['D'])), 'only new items', [PLAN_STATE, '누락']);
        rig.still('only new items');
        rig.pass('sending only new items refused');
        for (const [field, value] of [['question', ctx.inputs.changed_question], ['done_when', ctx.inputs.changed_done_when]]) {
          const changed = proposal(spec.second_approved, (items) => { items.find((i) => i.id === 'B')[field] = value; return items; });
          rig.expectRefused(await rig.model('gil_plan_propose', changed), `redefine ${field}`, [`draft.items[id="B"].${field}`]);
          rig.still(`redefine ${field}`);
          rig.pass(`changing B.${field} refused`, `field path draft.items[id="B"].${field}`);
        }
      });
      rig.pass('extension refusals leave everything intact', 'record, context and plan view unchanged; no proposal stored');
      const ok = rig.expectOk(await rig.model('gil_plan_propose', proposal(spec.second_approved)), 'copied items + D accepted');
      rig.need(ok.structured.proposal.base_revision === active.revision, 'base revision', `base ${ok.structured.proposal.base_revision}`);
      rig.pass(`[${spec.second_approved}] accepted`, `revision ${ok.structured.proposal.revision} on base ${active.revision}`);
      await rig.approveSynthetic(ok.structured.proposal.revision);
      await rig.closeApprovedInterview(ctx.inputs, iv.synthesis);
      const after = await rig.plan();
      rig.need(ids(after.pending).join() === spec.expected_pending.join(), 'pending', `pending [${ids(after.pending)}], document says [${spec.expected_pending}]`);
      rig.pass(`pending is [${spec.expected_pending}]`, `executed ${(after.executed_cycles ?? []).map((e) => `${e.item_id}@${e.revision}`)}`);
      await rig.openExperiment();
      rig.need((await rig.plan()).current_item?.id === spec.expected_pending[0], 'next binding', 'next Experiment did not bind the first pending item');
      rig.pass(`next Experiment binds ${spec.expected_pending[0]}`);
    },
  },
  {
    id: 'CX-06',
    title: 'Re-interview request inside an open Experiment is preserved, closes nothing, and opens no new Cycle',
    examples: ['WF-REINTERVIEW', 'WF-TABLE-REINTERVIEW', 'WF-RECOVER-PLAN-STATE', 'MAN-CLARIFY-GOOD', 'MAN-DEFINE-GOOD'],
    async run(rig, ctx) {
      await rig.bootstrap();
      const items = ctx.ex['MAN-CLARIFY-GOOD'].payload.items.slice(0, 2);
      await approvedPlan(rig, ctx, items);
      await rig.openExperiment();
      await rig.openStep('define', ctx.inputs.contract);
      const before = await rig.plan();
      rig.need(before.current_cycle?.state === 'open', 'cycle open', JSON.stringify(before.current_cycle));
      rig.expectOk(await rig.model('gil_plan_request_interview', { reason: ctx.inputs.reinterview_reason }), 'request interview');
      const after = await rig.plan();
      rig.need(after.reinterview_requests?.length === 1, 'request preserved', `requests ${JSON.stringify(after.reinterview_requests)}`);
      rig.need(JSON.stringify(after.current_cycle) === JSON.stringify(before.current_cycle) && after.current_item?.id === 'A', 'closes nothing', `cycle ${JSON.stringify(after.current_cycle)}`);
      rig.pass('request preserved, Cycle still open on A', `${after.current_cycle.ref} ${after.current_cycle.state}`);
      await rig.unchanged('no new Cycle while the Step is open', async () => {
        rig.expectRefused(await rig.model('gil_open', { kind: 'interview' }), 'interview while Define open');
        rig.expectRefused(await rig.model('gil_open', { kind: 'experiment' }), 'experiment while Define open');
      });
      rig.pass('no new Cycle while the Step is open', 'Interview and Experiment opens refused; record and position unchanged');
      const listed = await rig.client.request('tools/list', {});
      const names = (listed.result?.tools ?? []).map((t) => t.name);
      rig.need(!names.some((n) => /cancel|interrupt|abort/.test(n)), 'no cancel tool', `tools ${names.filter((n) => /cancel|interrupt|abort/.test(n))}`);
      rig.pass('no cancel or interrupt tool is offered', 'as the workflow states');
      await rig.closeStep(ctx.ex['MAN-DEFINE-GOOD'].payload.text, 'close define');
      await rig.finishExperiment(ctx.inputs, 'A', ctx.inputs.contract);
      rig.need((await rig.plan()).reinterview_requests?.length === 1, 'still preserved', 'request lost after the Cycle closed');
      rig.pass('request preserved through the truthful Cycle close');
      await rig.unchanged('further Experiment refused', async () => {
        rig.expectRefused(await rig.model('gil_open', { kind: 'experiment' }), 'experiment after the Cycle', [PLAN_STATE, 'gil_plan_read', '재인터뷰']);
      });
      rig.pass('further Experiment refused as an approval state', 'pending B stays unexecuted until a new Interview');
      rig.expectOk(await rig.model('gil_open', { kind: 'interview' }), 'interview opens');
      rig.pass('gil_open Interview is the next legal move');
    },
  },
  {
    id: 'CX-07',
    title: 'Observation only: Manual failure examples Core does not judge',
    examples: ['MAN-CLARIFY-FAIL', 'MAN-OPEN-FAIL'],
    observation: true,
    async run(rig, ctx) {
      await rig.bootstrap();
      const bad = ctx.ex['MAN-OPEN-FAIL'].payload.fields;
      const contract = `${ctx.inputs.observation_objective}\nnext_action: ${bad.next_action}\ndone_when: ${bad.done_when}\n`;
      const opened = await rig.model('gil_open', { kind: 'question', contract });
      ctx.observations.badContractAccepted = opened.ok;
      rig.pass('observed: bad contract', opened.ok ? 'Core opened the Step (it only refuses missing/blank fields); quality is not checked' : `Core refused: ${said(opened).slice(0, 120)}`);
      const question = opened.ok ? Rig.stepRef(opened, 'question') : await rig.openStep('question', ctx.inputs.contract);
      await rig.closeStep(ctx.inputs.question, 'close question');
      const interpretation = await rig.openStep('interpretation', ctx.inputs.contract);
      await rig.closeStep(ctx.inputs.interpretation, 'close interpretation');
      await rig.openStep('synthesis', ctx.inputs.contract);
      const label = ctx.ex['MAN-CLARIFY-FAIL'].payload.label;
      const draft = { scope: { ...ctx.inputs.scope }, items: [{ id: 'A', question: label, done_when: ctx.inputs.bundled_item_done_when }], synthesis: { statement: ctx.inputs.statement, basis_refs: `${question}\n${interpretation}`, exploration_checklist: 'A' } };
      const r = await rig.model('gil_plan_propose', { draft });
      ctx.observations.bundledItemAccepted = r.ok;
      rig.pass('observed: bundled item', r.ok ? 'Core accepted the bundled item; structural validity is not proposal quality' : `Core refused: ${said(r).slice(0, 120)}`);
    },
  },
];
