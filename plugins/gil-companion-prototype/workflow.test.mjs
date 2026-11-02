import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {sync, render, begin, end} from './sync-workflow.mjs';

const workflow = await readFile(new URL('../../src/mcp/workflow.md', import.meta.url), 'utf8');

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

// Wave 1 / D: the shared text must give every recorded state exactly one next call and must
// not offer the shortcuts that light models took in the field. This is a guidance regression,
// not a record of any real model run.
const rows = workflow.split('\n').filter(line => line.trim().startsWith('|')).map(line =>
  line.split('|').slice(1, -1).map(cell => cell.trim()));
const next = Object.fromEntries(rows.filter(([state, call]) => state && call && !state.startsWith('---') && state !== 'Current state' && state !== 'Situation').map(([state, call]) => [state, call]));

test('each Interview state maps to one next call and approval is never derived', () => {
  assert.match(next['Question is open'], /end the turn.*gil_close/);
  assert.match(next['Synthesis is open and the user has not answered'], /gil_plan_propose.*end the turn and wait; no project file changes/);
  assert.match(next['Synthesis is open and the user approved its statement'], /gil_close.*approved: yes/);
  assert.match(next['Synthesis is open and the user declined'], /approved: no.*gil_open. Question/);
  assert.match(next['Synthesis is closed and Outcome has not been opened'], /^`gil_open` Outcome$/);
  assert.match(next['Outcome is closed but the Interview Cycle is still open'], /Cycle Report; there is no extra Report step/);
  assert.match(next['Interview Cycle is closed and the agreed execution work remains'], /gil_open. Experiment; never .gil_start/);
  assert.match(workflow, /silence, earlier requests and the agent's own judgment are not approval/);
});

test('each Experiment state names the next call and Verify is opened before project file changes', () => {
  assert.match(next['Experiment Cycle just opened'], /Define.*no project file changes yet/);
  assert.match(next['Hypothesis is closed'], /`gil_open` Verify\. \*\*Wait for its success\*\* before changing project files/);
  assert.match(next['Verify is open'], /save the observation as project files.*evidence_files/);
  assert.match(next['Verify is closed'], /Analysis.*interpretation/);
  assert.match(next['Analysis is closed'], /Outcome.*lesson/);
  assert.match(next['Outcome is closed but the Experiment Cycle is still open'], /Cycle Report/);
  assert.match(next['Cycle closed `failure` · `revisit`'], /open Monitor and wait.*button.*do not call App-only tools or invent consent.*confirmed recovery.*gil_open/);
  assert.match(workflow, /Chat alone cannot complete\s+Verify/);
  assert.match(workflow, /Refused: an empty file, a file unchanged since the baseline, or the chat summary alone/);
});

test('file boundary covers Closing and imports without restricting outside work or claiming an OS lock', () => {
  assert.match(workflow, /Creating, modifying, deleting, renaming or moving/);
  assert.match(workflow, /importing files made elsewhere, DOCX\/PDF production, chart PNGs, rendering/);
  assert.match(workflow, /Closing.*Define, Hypothesis, Analysis, Outcome, Cycle\/Chain Reports, approval waits, boundaries/s);
  assert.match(workflow, /Work outside the project and web research that leaves project files unchanged are outside/);
  assert.match(workflow, /not an OS write lock/);
  assert.match(workflow, /do not claim it was created there/);
  assert.doesNotMatch(workflow, /no research|before reading sources|never researches/);
});

test('approved plans are detected from the record and schema, never required on legacy projects', () => {
  const section = workflow.slice(workflow.indexOf('**Approved plans'), workflow.indexOf('**Recover and keep'));
  assert.match(section, /`supported: true`/);
  assert.match(section, /`tools\/list` offers `gil_plan_propose`/);
  assert.match(section, /never require `gil_plan_\*`/);
  assert.match(section, /Do not infer this from the OS or the install kind/);
  for (const field of ['goal', 'in_scope', 'out_of_scope', 'constraints', 'success_criteria', '`id`, `question`, `done_when`'])
    assert.ok(section.includes(field), field);
  assert.match(section, /`exploration_checklist` lists the item IDs in the same\s+order, one per line/);
  assert.match(section, /Never write `approved`, `plan_revision` or `plan_digest`/);
  assert.match(section, /end the turn and wait for the Monitor button\.\*\* Chat agreement is not approval/);
  assert.match(section, /`close_report` \*\*unchanged\*\*/);
  assert.match(section, /then `gil_close` the Interview Cycle Report, then `gil_open`\s+Experiment/);
  assert.match(section, /\*\*byte for byte\*\* \(UTF-8\)/);
  assert.match(section, /The plan file is not execution evidence/);
  assert.match(section, /an earlier approval is never reused/);
  assert.match(section, /closes nothing\. Finish the current Step and Cycle truthfully/);
  assert.match(section, /the next call is\s+still the one the latest receipt names/);
  assert.match(section, /never an early Cycle\s+close/);
  assert.match(section, /there is no cancel or interrupt call/);
  assert.match(section, /not finished by a chat summary:\s+`gil_open` Interview as the Closing Interview.*`chain_verdict`.*Chain Report/s);
  assert.match(next['Cycle closed `success`'], /Closing Interview and Chain Report follow; a chat summary alone never completes the project/);
});

test('recovery rows connect each failure to the current fact and forbid loops and guarantees', () => {
  assert.match(next['`gil_start` refused because a project exists'], /gil_context. once.*never start again/);
  assert.match(next['Close response lost or uncertain'], /gil_status.*close again only if the step is still open/);
  assert.match(next['Plan decision or save result uncertain'], /gil_plan_read.*never assume approval/);
  assert.match(next['Approved earlier, proposal edited since'], /old approval is gone/);
  // Wave 2 / C (R4): a preserved request never replaces the legal next call or invites an early Cycle close.
  assert.match(next['Asked to re-interview during an Experiment'], /preserved and closes nothing.*each receipt's next call.*only when its receipt offers the Cycle Report.*then `gil_open` Interview/);
  assert.doesNotMatch(next['Asked to re-interview during an Experiment'], /cancel|interrupt|abort/i);
  assert.match(next['`gil_revisit` refused'], /show Monitor and wait.*Never edit records, fake success or loop restore/);
  assert.match(workflow, /Do not loop a tool to confirm a fact that a receipt already states/);
  assert.match(workflow, /never promise that a\s+screen, button or save "always works"/);
});

test('forbidden shortcuts never appear as instructions', () => {
  assert.doesNotMatch(workflow, /gil_cycle[^.]*action: close/);
  assert.doesNotMatch(workflow, /approved: yes` (when|if) (the )?(scope|answer) (is|seems) obvious/i);
  assert.match(workflow, /`gil_cycle` cannot accept a Report/);
  assert.match(workflow, /Never repeat a call that already succeeded/);
  assert.match(workflow, /^## GIL shared workflow\n/);
});


test('project deliverables default to tracked paths; necessary external outputs disclose the tracking gap before writing', () => {
  assert.match(workflow, /Project deliverables belong in the project by default/);
  assert.match(workflow, /intermediate renders and production scripts under `project_root` during an open Verify/);
  assert.match(workflow, /Before invoking a file-producing tool, choose its output path inside this project explicitly/);
  assert.match(workflow, /If external creation is genuinely necessary/);
  assert.match(workflow, /first tell the user the concrete reason and destination/);
  assert.match(workflow, /not tracked by GIL/);
  assert.match(workflow, /Do not create an external file just to evade a gate/);
  assert.match(workflow, /Unrelated external work remains allowed/);
});
