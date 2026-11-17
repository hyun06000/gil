import test from 'node:test';
import assert from 'node:assert/strict';
import { summary, normalize, report } from './report.mjs';
test('exact boundaries include details but exclude next call',()=>{
  assert.equal(summary('pre\nSTART\nbody\nEND\nnext','\nSTART\n','END\n'),'\nSTART\nbody\nEND\n');
  assert.throws(()=>summary('none','START','END'));
});
test('path normalization preserves non-path values and input',()=>{
 const e=[{result:{content:[{text:'/tmp/test/한글'}]},n:2}];
 assert.equal(normalize(e,'/tmp/test')[0].result.content[0].text,'<project_root>/한글');
 assert.equal(normalize([{text:'project:abc /tmp/test'}],'/tmp/test','project:abc')[0].text,'<scope_id> <project_root>');
 assert.equal(e[0].result.content[0].text,'/tmp/test/한글');
});
test('actual text is measured independently from structured representation',()=>{
 const e={seq:1,case:'unit-only',pass:0,request:{name:'gil_context',arguments:{}},result:{content:[{type:'text',text:'S한글Erest'}],structuredContent:{said:'different'}}};
 const r=report([e],{'ctx.resume.heading':'S','ctx.resume.details':'E'});
 assert.equal(r.contexts[0].summary.utf8_bytes,8);
 assert.equal(r.contexts[0].summary.code_points,4);
 assert.equal(r.contexts[0].text.utf8_bytes,12);
 assert.throws(()=>report([{...e,result:{isError:true}}],{}));
});
