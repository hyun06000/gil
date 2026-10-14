import test from 'node:test';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {readImage,visualFailure} from './visuals.mjs';
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
