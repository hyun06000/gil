// Maintainer-only check shared by macOS and Windows artifact smoke tests.
// This is the default/read surface, not approval to ship visual-authoring.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const expected = JSON.parse(readFileSync(new URL('../../src/mcp/tools.json', import.meta.url)));
const authoring = JSON.parse(readFileSync(new URL('../../src/mcp/authoring-schema.json', import.meta.url)));
const migration = JSON.parse(readFileSync(new URL('../../src/mcp/migration-tools.json', import.meta.url)));

export function checkAuthoringTools(tools, platform = process.platform) {
  // Windows adds gil_prepare_workspace (approved local workspace preparation); macOS has 24.
  assert.equal(tools.length, platform === 'win32' ? 25 : 24);
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
  assert.equal(expected.length, 19, 'review the packaging contract when the catalog changes');
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
