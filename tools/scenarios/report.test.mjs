import test from 'node:test';
import assert from 'node:assert/strict';
import { parseReport, formatReport, evidenceFiles } from './lib/report.mjs';

test('report text round-trips scalar and block fields', () => {
  const fields = { statement: '한 줄', basis_refs: 'step:C1/S1\nstep:C1/S2', 'next_direction.action': 'open_child', evidence_files: 'evidence/a.md\nevidence/b.md' };
  const text = formatReport(fields);
  assert.match(text, /^basis_refs: \|\n  step:C1\/S1\n  step:C1\/S2\n/m);
  assert.deepEqual(parseReport(text), fields);
  assert.deepEqual(evidenceFiles(parseReport(text)), ['evidence/a.md', 'evidence/b.md']);
  assert.deepEqual(evidenceFiles(parseReport('execution: x\nresult: y\nevidence_files: one.md\n')), ['one.md']);
  assert.deepEqual(evidenceFiles({}), []);
});

test('parser ignores prose lines and keeps the last block line without trailing blanks', () => {
  const parsed = parseReport('intro prose\nanswer: yes\nnotes: |\n  first\n  second\n\nafter: tail\n');
  assert.equal(parsed.answer, 'yes');
  assert.equal(parsed.notes, 'first\nsecond');
  assert.equal(parsed.after, 'tail');
  assert.deepEqual(parseReport(undefined), {});
});
