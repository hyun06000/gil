// Maintainer-only check shared by macOS and Windows artifact smoke tests.
// This is the default/read surface, not approval to ship visual-authoring.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const expected = JSON.parse(readFileSync(new URL('../../src/mcp/tools.json', import.meta.url)));
expected.push(JSON.parse(readFileSync(new URL('../../src/mcp/handoff-tool.json', import.meta.url))));
const authoring = JSON.parse(readFileSync(new URL('../../src/mcp/authoring-schema.json', import.meta.url)));
const migration = JSON.parse(readFileSync(new URL('../../src/mcp/migration-tools.json', import.meta.url)));

// Explicit product-safety surface: verify the private consent boundary before normalizing.
export function withoutRecoveryTools(tools) {
  const names = ['gil_revisit_review', 'gil_revisit_apply'];
  for (const name of names) {
    const found = tools.filter(t => t.name === name);
    assert.equal(found.length, 1, `missing/duplicate ${name}`);
    const tool = found[0], apply = name.endsWith('_apply');
    assert.deepEqual(tool._meta?.ui?.visibility, ['app']);
    assert.deepEqual(tool.annotations, {readOnlyHint: !apply, destructiveHint: apply, openWorldHint: false});
    assert.equal(tool.inputSchema.additionalProperties, false);
    const keys = apply ? ['scope_id', 'digest', 'token'] : ['scope_id'];
    assert.deepEqual([...tool.inputSchema.required].sort(), [...keys].sort());
    assert.deepEqual(Object.keys(tool.inputSchema.properties).sort(), [...keys].sort());
    for (const key of keys) assert.deepEqual(tool.inputSchema.properties[key], {type:'string'});
  }
  return tools.filter(t => !names.includes(t.name));
}

export function checkAuthoringTools(tools, platform = process.platform) {
  // Windows adds gil_prepare_workspace (approved local workspace preparation); macOS has 25.
  assert.equal(tools.length, platform === 'win32' ? 27 : 26);
  const workspace = tools.find(t => t.name === 'gil_prepare_workspace');
  if (platform === 'win32') {
    assert.ok(workspace, 'Windows authoring surface carries gil_prepare_workspace');
    assert.deepEqual(workspace._meta?.ui?.visibility, ['model']);
    assert.equal(workspace.inputSchema.additionalProperties, false);
    assert.deepEqual(workspace.inputSchema.required, ['name', 'confirm']);
    assert.equal(workspace.annotations.readOnlyHint, false);
  } else assert.equal(workspace, undefined, 'macOS authoring surface is unchanged');
  for (const contract of migration) {
    const actual=tools.find(t=>t.name===contract.name);
    assert.ok(actual);
    assert.deepEqual(actual.inputSchema,contract.inputSchema);
    assert.deepEqual(actual.annotations,contract.annotations);
    assert.deepEqual(actual._meta,contract._meta);
  }
  const read = tools.find(t => t.name === 'gil_report_visuals');
  assert.ok(read);
  assert.deepEqual(read.inputSchema, authoring.read);
  assert.deepEqual(read._meta?.ui?.visibility, ['model']);
  assert.deepEqual(read.annotations, {readOnlyHint:true, destructiveHint:false, openWorldHint:false});
  const close = tools.find(t => t.name === 'gil_close');
  assert.deepEqual(close.inputSchema, authoring.close);
  assert.equal(close.outputSchema, undefined);
  const normalized = structuredClone(tools.filter(t => t.name !== 'gil_report_visuals' && t.name !== 'gil_prepare_workspace' && !migration.some(m=>m.name===t.name)));
  normalized.find(t => t.name === 'gil_close').inputSchema = expected.find(t => t.name === 'gil_close').inputSchema;
  checkDefaultTools(normalized);
}

export function checkDefaultTools(tools) {
  assert.equal(expected.length, 21, 'review the packaging contract when the catalog changes');
  assert.equal(tools.length, expected.length, 'default artifact must not expose development tools');
  assert.deepEqual(tools.map(t => t.name).sort(), expected.map(t => t.name).sort(),
    'missing, duplicate or unexpected tool');
  for (const contract of expected) {
    const actual = tools.find(t => t.name === contract.name);
    assert.deepEqual(actual.inputSchema, contract.inputSchema, `${contract.name}: input contract`);
    assert.deepEqual(actual._meta?.ui?.visibility, contract._meta.ui.visibility,
      `${contract.name}: Host visibility boundary`);
    assert.deepEqual(actual.annotations, contract.annotations, `${contract.name}: annotations`);
  }
  for (const name of ['gil_monitor_report_visuals', 'gil_monitor_report_image']) {
    const tool = tools.find(t => t.name === name);
    assert.deepEqual(tool._meta.ui.visibility, ['app']);
    assert.equal(tool.annotations.readOnlyHint, true);
    assert.equal(tool.inputSchema.additionalProperties, false);
    assert.ok(tool.inputSchema.required.includes('scope_id'));
    assert.ok(tool.inputSchema.required.includes('owner'));
  }
  const close = tools.find(t => t.name === 'gil_close');
  assert.deepEqual(Object.keys(close.inputSchema.properties).sort(), ['project_root', 'report']);
  assert.ok(!tools.some(t => t.name === 'gil_report_visuals'));
}

// Approved-plan is an explicit test profile, never inferred from an arbitrary extra tool.
export function checkApprovedPlanTools(tools, platform = process.platform) {
  const names = ['gil_plan_propose', 'gil_plan_request_interview', 'gil_plan_read', 'gil_plan_review', 'gil_plan_decide'];
  assert.equal(tools.length, (platform === 'win32' ? 27 : 26) + names.length);
  checkAuthoringTools(tools.filter(t => !names.includes(t.name)), platform);
  for (const name of names) {
    const matches = tools.filter(t => t.name === name);
    assert.equal(matches.length, 1, `missing/duplicate ${name}`);
    const t = matches[0], app = ['gil_plan_review', 'gil_plan_decide'].includes(name);
    assert.deepEqual(t._meta?.ui?.visibility, [app ? 'app' : 'model']);
    assert.equal(t.inputSchema.additionalProperties, false);
    assert.ok(t.inputSchema.required.includes(app ? 'scope_id' : 'project_root'));
    assert.equal(t.annotations.readOnlyHint, ['gil_plan_read', 'gil_plan_review'].includes(name));
    assert.equal(t.annotations.destructiveHint, false);
  }
  const decide = tools.find(t => t.name === 'gil_plan_decide');
  assert.deepEqual([...decide.inputSchema.required].sort(), ['scope_id', 'revision', 'digest', 'token', 'decision'].sort());
}

// Explicit exploration surface: retain all approval/recovery boundaries and add only original reads.
export function checkExplorationTools(tools, platform = process.platform) {
  const names = ['gil_report_read', 'gil_evidence_read'];
  checkApprovedPlanTools(tools.filter(t => !names.includes(t.name)), platform);
  for (const name of names) {
    const matches = tools.filter(t => t.name === name);
    assert.equal(matches.length, 1, `missing/duplicate ${name}`);
    const t = matches[0], s = t.inputSchema;
    assert.deepEqual(t._meta?.ui?.visibility, ['model']);
    assert.deepEqual(t.annotations, {readOnlyHint:true, destructiveHint:false, openWorldHint:false});
    assert.equal(s.additionalProperties, false);
    const evidence = name === 'gil_evidence_read';
    assert.deepEqual([...s.required].sort(), (evidence ? ['project_root','owner','path'] : ['project_root','owner']).sort());
    assert.deepEqual(Object.keys(s.properties).sort(), (evidence ? ['project_root','owner','path','offset'] : ['project_root','owner','list','from','field','part','offset']).sort());
    assert.equal(s.properties.project_root.type, 'string');
    const owner = s.properties.owner;
    assert.equal(owner.additionalProperties, false);
    assert.deepEqual([...owner.required].sort(), ['kind','ref']);
    assert.deepEqual([...owner.properties.kind.enum].sort(), evidence ? ['step'] : ['chain','cycle','step']);
    assert.equal(owner.properties.ref.type, 'string');
    assert.equal(s.properties.offset.type, 'integer');
    assert.equal(s.properties.offset.minimum, 0);
    if (evidence) assert.equal(s.properties.path.type, 'string');
    else {
      assert.deepEqual(s.properties.list.enum, ['cycles','steps','fields','files']);
      assert.deepEqual(s.properties.part.enum, ['value','name']);
      for (const key of ['field','from']) {
        assert.equal(s.properties[key].type, 'integer');
        assert.equal(s.properties[key].minimum, 0);
      }
    }
  }
}

// The product build: every approved feature in one executable, with two kinds of new project.
// Checked here: the selection input of gil_start, the tracked-scope preparation and scope tools
// with their App-only approval boundary, the explicit format-transition pair, and — after those
// additions are verified and set aside — the unchanged exploration/approved-plan/authoring surface.
export function checkProductTools(tools, platform = process.platform) {
  const model = {
    gil_start_prepare: false, gil_start_status: true, gil_start_cancel: false,
    gil_scope_propose: false, gil_scope_status: true,
    gil_format_transition_check: true, gil_format_transition_apply: false,
  };
  const app = {
    gil_start_review: true, gil_start_decide: false, gil_start_apply: false, gil_start_list: true,
    gil_scope_review: null, gil_scope_decide: false, gil_scope_retry: false, gil_scope_halt: false, gil_scope_release: false,
  };
  for (const [name, read] of [...Object.entries(model), ...Object.entries(app)]) {
    const found = tools.filter(t => t.name === name);
    assert.equal(found.length, 1, `missing/duplicate ${name}`);
    const tool = found[0];
    assert.deepEqual(tool._meta?.ui?.visibility, [Object.hasOwn(app, name) ? 'app' : 'model'], `${name}: approval boundary`);
    assert.equal(tool.inputSchema.additionalProperties, false, `${name}: closed input`);
    assert.ok(tool.inputSchema.required.includes(Object.hasOwn(app, name) ? 'scope_id' : 'project_root'), `${name}: addressed by ${Object.hasOwn(app, name) ? 'scope' : 'root'}`);
    if (read !== null && !name.startsWith('gil_format_transition_')) assert.equal(tool.annotations.readOnlyHint, read, `${name}: read-only hint`);
    // No model tool carries an approval value: the capability exists only in App-only inputs.
    if (!Object.hasOwn(app, name)) assert.ok(!Object.keys(tool.inputSchema.properties).includes('token'), `${name}: model tool with a capability input`);
  }
  for (const name of ['gil_start_decide', 'gil_start_apply', 'gil_scope_decide', 'gil_scope_retry', 'gil_scope_release']) {
    assert.ok(tools.find(t => t.name === name).inputSchema.required.includes('token'), `${name}: requires the App capability`);
  }
  const rest = structuredClone(tools.filter(t => !Object.hasOwn(model, t.name) && !Object.hasOwn(app, t.name)));
  const start = rest.find(t => t.name === 'gil_start');
  assert.ok(start, 'gil_start');
  const kind = start.inputSchema.properties.start_kind;
  assert.deepEqual(kind?.enum, ['tracked_scope', 'image_reports'], 'both kinds of new project are selectable');
  assert.equal(kind.type, 'string');
  assert.deepEqual(start.inputSchema.required, ['project_root'], 'the selection is explicit input, not a schema default');
  assert.equal(start.outputSchema, undefined, 'one tool answers a preparation or a receipt');
  for (const hidden of ['format 9', 'format 10', 'format 11', 'tracking-policy', 'visual-authoring', 'exploration-flow']) {
    assert.ok(!start.description.includes(hidden), `gil_start description exposes ${hidden}`);
  }
  delete start.inputSchema.properties.start_kind;
  const close = rest.find(t => t.name === 'gil_close');
  const unachieved = close.inputSchema.properties.unachieved_will;
  assert.deepEqual({type: unachieved?.type, pattern: unachieved?.pattern}, {type: 'string', pattern: '^will:W[1-9][0-9]*$'},
    'the unachieved end names one canonical Will');
  if (!Object.hasOwn(authoring.close.properties ?? {}, 'unachieved_will')) delete close.inputSchema.properties.unachieved_will;
  checkExplorationTools(rest, platform);
}
