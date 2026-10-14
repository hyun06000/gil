import test from 'node:test';
import assert from 'node:assert/strict';
import {cycleReportDetail} from '../ui/cycle-report.js';
test('cycle projection preserves identity, literal text and source without mutation',()=>{
  const c={cycle_ref:'cycle:C3',kind:'experiment',state:'closed',report:{verdict:'success',handoff_summary:'<img src=x onerror=alert(1)>',outcome_lesson:'stored lesson',next_direction:{action:'open_child',reason:'next',target_cycle_ref:null}}};
  const before=structuredClone(c),d=cycleReportDetail(c);
  assert.deepEqual(d.owner,{kind:'cycle',ref:'cycle:C3'});
  assert.equal(d.step_ref,undefined);
  assert.equal(d.report.fields[1].value,c.report.handoff_summary);
  assert.equal(d.report.fields[2].name,'판정의 교훈 (Outcome)');
  assert.equal(d.report.fields.length,5);assert.deepEqual(c,before);
});
test('open cycle has no invented report or optional fields',()=>{
  assert.equal(cycleReportDetail({cycle_ref:'cycle:C1',kind:'interview',state:'open',report:null}).report,null);
  assert.equal(cycleReportDetail({cycle_ref:'cycle:C1',kind:'interview',state:'closed',report:{verdict:'success',handoff_summary:'handoff'}}).report.fields.length,2);
});
