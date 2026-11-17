// Unmodified Monitor wire payloads, produced by tests/chain_monitor_fixtures.rs.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { chainModel, chainLine, chainContrast, chainListEntries, chainDetail, chainOfCycle } from '../ui/chain.mjs';
import { setLocale } from '../ui/locale.js';
setLocale('ko');
const folder = new URL('../ui/fixtures/chain-core/', import.meta.url);
const read = name => JSON.parse(readFileSync(new URL(`${name}.json`, folder), 'utf8'));
const fields = detail => Object.fromEntries((detail.report?.fields ?? []).map(f => [f.name, f.value]));
const freeze = value => {
  if (value && typeof value === 'object') { Object.values(value).forEach(freeze); Object.freeze(value); }
  return value;
};
const phases = ['first-interview', 'experiment-closed', 'closing-ready', 'chain-closed', 'next-chain'];

for (const verdict of ['success', 'failure']) {
  for (const phase of phases) test(`${verdict}/${phase}: real current, ownership, state and literal reports`, () => {
    const { view, details } = freeze(read(`${verdict}-${phase}`));
    const before = JSON.stringify(view), model = chainModel(view);
    assert.equal(model.current.ref, view.current.chain_ref);
    assert.equal(model.current.ownsCurrentCycle, true);
    assert.deepEqual(model.all.map(c => c.ref), view.chains.map(c => c.chain_ref));
    assert.equal(chainListEntries(view).filter(c => c.isCurrent).length, 1);
    const owned = [];
    for (const raw of view.chains) {
      const projected = model.all.find(c => c.ref === raw.chain_ref);
      assert.equal(projected.state, raw.state);
      assert.equal(projected.verdict, raw.report?.verdict ?? '');
      assert.equal(projected.handoff, raw.report?.handoff_summary ?? '');
      const detail = chainDetail(view, raw.chain_ref);
      assert.deepEqual(detail.cycles.map(c => c.ref), raw.cycle_refs);
      for (const cycle of detail.cycles) {
        const source = view.timeline.find(c => c.cycle_ref === cycle.ref);
        assert.ok(source && cycle.known);
        assert.equal(cycle.kind, source.kind);
        assert.equal(cycle.state, source.state);
        assert.equal(cycle.isHere, cycle.ref === view.current.cycle_ref);
        assert.equal(chainOfCycle(view, cycle.ref).ref, raw.chain_ref);
        for (const step of source.steps) {
          assert.equal(details[step.step_ref].cycle_ref, cycle.ref);
          assert.equal(details[step.step_ref].state, step.state);
        }
        owned.push(cycle.ref);
      }
      if (raw.report) {
        assert.equal(detail.report.fields.length, Object.keys(raw.report).length);
        assert.equal(fields(detail)['인계 요약'], raw.report.handoff_summary);
        assert.equal(fields(detail)['종료 근거 (승인된 합의 확인)'], raw.report.closing_synthesis_ref);
        assert.ok(fields(detail)['판정'].endsWith(`(${raw.report.verdict})`));
      } else assert.equal(detail.report, null);
    }
    assert.equal(new Set(owned).size, owned.length);
    assert.deepEqual(owned, view.timeline.map(c => c.cycle_ref));
    assert.equal(JSON.stringify(view), before, 'projection cannot mutate Current or stored facts');
  });

  test(`${verdict}: closed Experiment and closing-ready are not a closed Chain`, () => {
    const done = read(`${verdict}-experiment-closed`);
    assert.equal(done.view.timeline.at(-1).report.verdict, 'success');
    assert.equal(chainModel(done.view).current.state, 'open');
    assert.equal(chainModel(done.view).current.verdict, '');
    assert.match(chainContrast(done.view), /사이클 결과는 Chain의 결과가 아닙니다/);
    assert.equal(done.plan.current_item.id, 'A');
    assert.equal(done.plan.current_cycle.state, 'closed');
    const ready = read(`${verdict}-closing-ready`).view;
    assert.equal(ready.chains[0].cycle_refs.length, 3);
    assert.equal(ready.timeline.at(-1).report.verdict, 'success', 'Closing Interview succeeds even when Chain verdict will be failure');
    assert.match(chainContrast(ready), /아직 닫히지 않았습니다/);
    assert.ok(ready.next_actions.some(a => a.kind === 'close_chain'));
    const closed = read(`${verdict}-chain-closed`).view;
    assert.equal(chainModel(closed).current.verdict, verdict);
    assert.match(chainContrast(closed), /다음 Chain은 새 인터뷰/);
  });

  test(`${verdict}: selecting previous Chain/Cycles leaves Current and fresh approval unchanged`, () => {
    const packet = freeze(read(`${verdict}-next-chain`));
    const before = JSON.stringify(packet);
    const old = chainDetail(packet.view, 'chain:C1');
    assert.equal(old.isCurrent, false);
    assert.equal(old.cycles.length, 3);
    assert.ok(old.cycles.every(c => !c.isHere));
    for (const cycle of old.cycles) assert.equal(chainOfCycle(packet.view, cycle.ref).ref, 'chain:C1');
    assert.equal(packet.view.current.chain_ref, 'chain:C2');
    assert.equal(packet.view.timeline.at(-1).cycle_ref, 'cycle:C4');
    assert.equal(packet.view.timeline.at(-1).parent_cycle_ref, null, 'Chain parent is not a Cycle parent');
    assert.equal(packet.view.timeline.at(-1).kind, 'interview');
    assert.equal(packet.view.timeline.at(-1).state, 'open');
    assert.equal(packet.plan.active_plan, null);
    assert.equal(packet.plan.active_revision, null);
    assert.deepEqual(packet.plan.executed_cycles, []);
    const previousApproval = fields(packet.details['step:C3/S3']);
    assert.equal(previousApproval.approved, 'yes');
    assert.equal(previousApproval.chain_verdict, verdict);
    assert.equal(previousApproval.plan_revision, '2');
    assert.equal(packet.view.chains[0].report.closing_synthesis_ref, 'step:C3/S3');
    assert.equal(JSON.stringify(packet), before);
    assert.match(chainLine(packet.view), /이전 Chain 1개/);
  });

  test(`${verdict}: stored evidence joins old Verify and retains bytes, digest and provenance`, () => {
    const packet = read(`${verdict}-next-chain`), { visuals, image } = read(`${verdict}-historical-evidence`);
    assert.equal(visuals.owner.ref, 'step:C2/S3');
    assert.equal(packet.details[visuals.owner.ref].kind, 'verify');
    assert.equal(chainOfCycle(packet.view, packet.details[visuals.owner.ref].cycle_ref).ref, 'chain:C1');
    assert.equal(visuals.uses[0].origin_step_ref, visuals.owner.ref);
    assert.equal(visuals.uses[0].asset_id, image.asset_id);
    assert.equal(visuals.uses[0].sha256, image.sha256);
    const bytes = Buffer.from(image.data_base64, 'base64');
    assert.equal(bytes.length, image.total_bytes);
    assert.equal(createHash('sha256').update(bytes).digest('hex'), image.sha256);
    assert.equal(visuals.uses[0].source_kind, 'generated');
    assert.equal(visuals.uses[0].source_description, 'Synthetic pixel, not a screenshot');
    assert.equal(image.next_offset, null);
    assert.equal(image.offset, 0);
  });
}

test('real legacy Core response and existing fixtures retain their Chain-free UI', () => {
  const views = [read('legacy').view, ...['reading-one','first-interview','dense'].map(n =>
    JSON.parse(readFileSync(new URL(`../ui/fixtures/${n}/view.json`, import.meta.url), 'utf8')))];
  for (const view of views) {
    freeze(view);
    assert.equal(chainModel(view), null);
    assert.equal(chainLine(view), '');
    assert.equal(chainContrast(view), '');
    assert.deepEqual(chainListEntries(view), []);
    assert.equal(chainDetail(view, 'chain:C1'), null);
    assert.equal(chainOfCycle(view, view.current.cycle_ref), null);
  }
});

test('fixture inventory and normalization are explicit; private transport values are absent', () => {
  const names = readdirSync(folder).filter(n => n.endsWith('.json')).sort();
  assert.deepEqual(names, ['legacy.json', ...['success','failure'].flatMap(v => [...phases, 'historical-evidence'].map(p => `${v}-${p}.json`))].sort());
  for (const name of names) {
    const text = readFileSync(new URL(name, folder), 'utf8'), packet = JSON.parse(text);
    assert.doesNotMatch(text, /gil\/planApprovalToken|approval_token|\/Users\/|\/private\/|[A-Z]:\\\\Users\\\\/);
    if (packet.view) assert.equal(packet.view.captured_at_unix_ms, 0);
    if (packet.image) assert.equal(packet.image.scope_id, `project:${'0'.repeat(64)}`);
  }
});
