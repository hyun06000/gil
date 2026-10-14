import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {checkDefaultTools, checkAuthoringTools} from './tool-contract.mjs';

const catalog = JSON.parse(readFileSync(new URL('../../src/mcp/tools.json', import.meta.url)));
test('authoring profile is explicit, model-only and does not relax App reads', () => {
  const schemas=JSON.parse(readFileSync(new URL('../../src/mcp/authoring-schema.json', import.meta.url)));
  const tools=structuredClone(catalog), close=tools.find(t=>t.name==='gil_close');
  close.inputSchema=schemas.close; delete close.outputSchema;
  tools.push({name:'gil_report_visuals',inputSchema:schemas.read,
    annotations:{readOnlyHint:true,destructiveHint:false,openWorldHint:false},_meta:{ui:{visibility:['model']}}});
  tools.push(...JSON.parse(readFileSync(new URL('../../src/mcp/migration-tools.json', import.meta.url))));
  checkAuthoringTools(tools);
  assert.throws(()=>checkDefaultTools(tools));
  assert.throws(()=>checkAuthoringTools(catalog));
  tools.at(-1)._meta.ui.visibility.push('app');
  assert.throws(()=>checkAuthoringTools(tools));
});
test('default image-read catalog passes in any order', () => {
  checkDefaultTools(catalog);
  checkDefaultTools([...catalog].reverse());
});
test('legacy, duplicate, unknown and authoring tool sets fail closed', () => {
  for (const tools of [catalog.slice(2), [...catalog, {name:'gil_report_visuals'}],
    [...catalog.slice(1), catalog[1]], [...catalog.slice(1), {name:'unknown'}]]) {
    assert.throws(() => checkDefaultTools(tools));
  }
});
test('image reads cannot gain model visibility, writes or path inputs', () => {
  for (const mutate of [t => t._meta.ui.visibility.push('model'),
    t => t.annotations.readOnlyHint = false,
    t => t.inputSchema.properties.path = {type:'string'},
    t => t.inputSchema.required = ['scope_id'],
    t => t.inputSchema.additionalProperties = true]) {
    const tools = structuredClone(catalog);
    mutate(tools.find(t => t.name === 'gil_monitor_report_image'));
    assert.throws(() => checkDefaultTools(tools));
  }
});
test('default close cannot silently become the experimental authoring surface', () => {
  const tools = structuredClone(catalog);
  tools.find(t => t.name === 'gil_close').inputSchema.properties.visuals = {type:'object'};
  assert.throws(() => checkDefaultTools(tools));
});
