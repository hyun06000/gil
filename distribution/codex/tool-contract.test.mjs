import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {checkDefaultTools, checkAuthoringTools, withoutRecoveryTools} from './tool-contract.mjs';

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

test('product recovery requires both strict App-only tools and never model consent', () => {
  const recovery = (apply) => ({ name: `gil_revisit_${apply ? 'apply' : 'review'}`,
    _meta:{ui:{visibility:['app']}}, annotations:{readOnlyHint:!apply,destructiveHint:apply,openWorldHint:false},
    inputSchema:{type:'object', additionalProperties:false, required:apply ? ['scope_id','digest','token'] : ['scope_id'],
      properties:Object.fromEntries((apply ? ['scope_id','digest','token'] : ['scope_id']).map(k=>[k,{type:'string'}]))} });
  const tools = [...catalog, recovery(false), recovery(true)];
  checkDefaultTools(withoutRecoveryTools(tools));
  for (const mutate of [ t=>t._meta.ui.visibility.push('model'), t=>t.annotations.destructiveHint=false,
    t=>t.inputSchema.required.pop(), t=>t.inputSchema.properties.path={type:'string'} ]) {
    const bad=structuredClone(tools); mutate(bad.at(-1)); assert.throws(()=>withoutRecoveryTools(bad));
  }
  assert.throws(()=>withoutRecoveryTools(catalog));
  assert.throws(()=>withoutRecoveryTools([...tools, recovery(true)]));
});
