import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {checkDefaultTools, checkAuthoringTools, withoutRecoveryTools} from './tool-contract.mjs';

const catalog = JSON.parse(readFileSync(new URL('../../src/mcp/tools.json', import.meta.url)));
catalog.push(JSON.parse(readFileSync(new URL('../../src/mcp/handoff-tool.json', import.meta.url))));
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

import {checkExplorationTools} from './tool-contract.mjs';
test('exploration keeps approval boundaries and requires exactly two read-only original-record tools', () => {
  const schemas=JSON.parse(readFileSync(new URL('../../src/mcp/authoring-schema.json', import.meta.url)));
  const tools=structuredClone(catalog);
  tools.find(t=>t.name==='gil_close').inputSchema=schemas.close;
  delete tools.find(t=>t.name==='gil_close').outputSchema;
  tools.push({name:'gil_report_visuals',inputSchema:schemas.read,annotations:{readOnlyHint:true,destructiveHint:false,openWorldHint:false},_meta:{ui:{visibility:['model']}}});
  tools.push(...JSON.parse(readFileSync(new URL('../../src/mcp/migration-tools.json', import.meta.url))));
  for(const name of ['gil_plan_propose','gil_plan_request_interview','gil_plan_read','gil_plan_review','gil_plan_decide']) {
    const app=['gil_plan_review','gil_plan_decide'].includes(name);
    tools.push({name,_meta:{ui:{visibility:[app?'app':'model']}},annotations:{readOnlyHint:['gil_plan_read','gil_plan_review'].includes(name),destructiveHint:false},inputSchema:{additionalProperties:false,required:name==='gil_plan_decide'?['scope_id','revision','digest','token','decision']:[app?'scope_id':'project_root']}});
  }
  for(const name of ['gil_report_read','gil_evidence_read']) {
    const evidence=name==='gil_evidence_read';
    const properties={project_root:{type:'string'},owner:{additionalProperties:false,required:['kind','ref'],properties:{kind:{enum:evidence?['step']:['step','cycle','chain']},ref:{type:'string'}}},offset:{type:'integer',minimum:0},...(evidence?{path:{type:'string'}}:{list:{enum:['cycles','steps','fields','files']},from:{type:'integer',minimum:0},field:{type:'integer',minimum:0},part:{enum:['value','name']}})};
    tools.push({name,_meta:{ui:{visibility:['model']}},annotations:{readOnlyHint:true,destructiveHint:false,openWorldHint:false},inputSchema:{additionalProperties:false,required:evidence?['project_root','owner','path']:['project_root','owner'],properties}});
  }
  checkExplorationTools(tools);
  assert.throws(()=>checkExplorationTools(tools.slice(0,-1)));
  assert.throws(()=>checkExplorationTools([...tools,tools.at(-1)]));
  for(const mutate of [t=>t.annotations.readOnlyHint=false,t=>t._meta.ui.visibility=['app'],t=>t.inputSchema.properties.offset.minimum=-1,t=>t.inputSchema.properties.extra={type:'string'}]) {
    const bad=structuredClone(tools); mutate(bad.at(-1)); assert.throws(()=>checkExplorationTools(bad));
  }
  const bad=structuredClone(tools);bad.find(t=>t.name==='gil_plan_decide')._meta.ui.visibility=['model'];assert.throws(()=>checkExplorationTools(bad));
});
