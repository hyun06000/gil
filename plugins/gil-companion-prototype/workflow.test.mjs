import test from 'node:test';
import assert from 'node:assert/strict';
import {sync, render, begin, end} from './sync-workflow.mjs';

test('packaged Plugin skill contains the exact Core workflow', async () => {
  await sync();
});
test('workflow synchronization preserves host-specific instructions and is idempotent', () => {
  const skill = `frontmatter\n${begin}\nstale\n${end}\nhost-specific instructions\n`;
  const expected = `frontmatter\n${begin}\nnew workflow\n${end}\nhost-specific instructions\n`;
  assert.equal(render(skill, 'new workflow\n'), expected);
  assert.equal(render(expected, 'new workflow\n'), expected);
  assert.ok(render(skill, 'new workflow\n\n').includes(`new workflow\n\n${end}`));
  for (const malformed of ['no markers', `${end}${begin}`, `${begin}${begin}${end}`, `${begin}${end}${end}`])
    assert.throws(() => render(malformed, 'text'), /exactly one/);
});
