import assert from 'node:assert/strict';
import {test} from 'node:test';
import {compatibility,checkCore} from './compatibility.mjs';
import {checkTransition,validateRecord} from './update-policy.mjs';
import {compatible as legacyCompatible} from '../../plugins/gil-companion-prototype/core.mjs';
const core=profile=>({schema_version:1,product:'gil_core',os:'macos',arch:'aarch64',...compatibility(profile)});
const record=(profile,version)=>({schema:1,plugin:'gil-companion-prototype',platform:'darwin-arm64',version,
 source_commit:'a'.repeat(40),source_snapshot_sha256:'b'.repeat(64),archive_sha256:'c'.repeat(64),
 core_sha256:'d'.repeat(64),ui_sha256:'e'.repeat(64),compatibility:compatibility(profile),
 migration:profile==='default'?'none':'requires_review'});
test('default remains action 1/format 4; image profile declares action 2/format 4..5',()=>{
 assert.deepEqual(compatibility().action_surface,{min:1,max:1});
 assert.deepEqual(compatibility('visual-authoring').storage_format,{min:4,max:5});
 for(const profile of ['default','visual-authoring'])assert.deepEqual(checkCore(core(profile),profile),compatibility(profile));
 assert.throws(()=>compatibility('unknown'));
});
test('fresh probe rejects old, wrong-profile and unsupported-platform claims',()=>{
 assert.throws(()=>checkCore(core('default'),'visual-authoring'));
 assert.throws(()=>checkCore(core('visual-authoring'),'default'));
 for(const change of [{os:'windows'},{arch:'x86_64'},{schema_version:2},{storage_format:{min:5,max:5}},{action_surface:{min:1,max:2}}])
  assert.throws(()=>checkCore({...core('visual-authoring'),...change},'visual-authoring'));
});
test('legacy action-1 consumer refuses the image Core instead of silently accepting it',()=>{
 const machine={os:'macos',arch:'aarch64'};
 assert.equal(legacyCompatible({...core('default'),core_version:'fixture'},machine),true);
 assert.equal(legacyCompatible({...core('visual-authoring'),core_version:'fixture'},machine),false);
});
test('image update and rollback never authorize implicit record conversion or installation',()=>{
 const old=record('default','0.2.1-preview.3'),next=record('visual-authoring','0.3.0-preview.1');
 for(const [a,b,d] of [[old,next,'update'],[next,old,'rollback'],[next,record('visual-authoring','0.3.0-preview.2'),'update']]){
  const r=checkTransition(a,b,d);assert.equal(r.status,'blocked');assert.equal(r.install_authorized,false);assert.equal(r.publishable,false);
 }
});
test('image release record cannot claim no migration or omit legacy format support',()=>{
 const r=record('visual-authoring','0.3.0-preview.1');
 assert.throws(()=>validateRecord({...r,migration:'none'}));
 assert.throws(()=>validateRecord({...r,compatibility:{...r.compatibility,storage_format:{min:5,max:5}}}));
});
