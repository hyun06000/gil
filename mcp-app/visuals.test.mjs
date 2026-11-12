import test from 'node:test';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {readImage,visualFailure,visualSummary} from './visuals.mjs';
// These assertions read the Korean table; the language-specific cases live in locale.test.mjs.
import { setLocale } from '../ui/locale.js';
setLocale('ko');
const scope='project:test', owner={kind:'step',ref:'step:C2/S3'};
function fixture(size=600000) {
  const bytes=Buffer.alloc(size,37);
  const item={asset_id:'9007199254740993',byte_length:size,width:1,height:1,mime_type:'image/png',sha256:createHash('sha256').update(bytes).digest('hex')};
  let calls=0;
  const call=async (_,args)=>{calls++;const end=Math.min(args.offset+262144,size);return {schema_version:1,scope_id:scope,owner,asset_id:item.asset_id,total_bytes:size,offset:args.offset,next_offset:end<size?end:null,mime_type:'image/png',sha256:item.sha256,data_base64:bytes.subarray(args.offset,end).toString('base64')};};
  return {bytes,item,call,count:()=>calls};
}
test('bounded chunk assembly verifies exact bytes and retains string IDs',async()=>{
  const f=fixture();const blob=await readImage(f.call,scope,owner,f.item);
  assert.deepEqual(Buffer.from(await blob.arrayBuffer()),f.bytes);assert.equal(f.count(),3);
});
test('scope, owner, hash, offsets, sizes and chunk contents cannot be mixed',async()=>{
  for(const edit of [d=>d.scope_id='other',d=>d.owner={kind:'cycle',ref:'cycle:C2'},d=>d.offset++,d=>d.next_offset=0,d=>d.total_bytes++,d=>d.asset_id='2',d=>d.sha256='0'.repeat(64),d=>d.data_base64='AAAA']){
    const f=fixture();await assert.rejects(readImage(async(...a)=>{const d=await f.call(...a);edit(d);return d;},scope,owner,f.item));
  }
});
test('a complete but wrong digest is refused; bounds are checked before requesting',async()=>{
  const f=fixture();f.item.sha256='0'.repeat(64);await assert.rejects(readImage(f.call,scope,owner,f.item));
  for(const change of [{byte_length:16777217},{width:8193},{mime_type:'image/svg+xml'},{asset_id:'01'}]){
    const g=fixture();await assert.rejects(readImage(g.call,scope,owner,{...g.item,...change}));assert.equal(g.count(),0);
  }
});
test('stale selections stop before another chunk request',async()=>{
  const f=fixture();let alive=true;
  await assert.rejects(readImage(async(...a)=>{const d=await f.call(...a);alive=false;return d;},scope,owner,f.item,()=>alive));assert.equal(f.count(),1);
});
test('Host failure diagnostics expose only fixed stage/code, never raw errors',()=>{
  assert.match(visualFailure('digest',{code:'digest_unavailable'}),/\[digest\/digest_unavailable\]/);
  const message=visualFailure('secret path',{code:'private data',message:'secret path'});
  assert.match(message,/\[unknown\/failed\]/);assert.doesNotMatch(message,/secret|private/);
});
test('the panel headline states count or the recorded reason, never an inferred absence',()=>{
  assert.deepEqual(visualSummary({state:'attached',uses:[{},{}]}).title,'시각 자료 2개');
  assert.equal(visualSummary({state:'references',uses:[{}]}).title,'인용한 시각 자료 1개');
  const none=visualSummary({state:'not_needed',reason:'두 파일의 sha256 일치만 비교했다',uses:[]});
  assert.equal(none.title,'시각 자료 없음');assert.match(none.status,/사유: 두 파일의 sha256/);assert.equal(none.tone,'none');
  const missing=visualSummary({state:'missing',reason:'렌더러 없음',follow_up:'다음 Cycle에서 캡처',uses:[{}]});
  assert.equal(missing.title,'시각 자료 미완 · 1개 첨부');assert.match(missing.status,/후속 계획: 다음 Cycle/);
  assert.equal(visualSummary({state:'legacy_unspecified',uses:[]}).status,'당시 시각 자료 선언이 기록되지 않았습니다.');
  assert.equal(visualSummary({state:'open_report',uses:[]}).tone,'pending');
});
