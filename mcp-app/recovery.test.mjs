import test from 'node:test';
import assert from 'node:assert/strict';
import {makeRecoveryReview} from './recovery.mjs';
class Element {
  constructor(tag) { this.tag=tag; this.children=[]; this.listeners={}; this.textContent=''; this.hidden=false; }
  append(...nodes) {this.children.push(...nodes);}
  replaceChildren(...nodes) {this.children=nodes;}
  addEventListener(name,fn) {this.listeners[name]=fn;}
  querySelectorAll(tag) {return this.children.flatMap(n=>[...(n.tag===tag?[n]:[]),...n.querySelectorAll(tag)]);}
  allText() {return this.textContent+this.children.map(n=>n.allText()).join(' ');}
  click(trusted) {return this.listeners.click?.({isTrusted:trusted});}
}
function rig() {
  const panel=new Element('section'), calls=[], messages=[], doc={createElement:tag=>new Element(tag)};
  let changes=0;
  const state={value:{supported:true,state:'awaiting_approval',from:'cycle:C2',target:'cycle:C1',digest:'digest',archive:'.gil/revisit-preserved/test',
    changes:[{path:'<img onerror=attack>.md',action:'preserve'},{path:'report.md',action:'replace'}]},token:'private',failRead:false,loseReply:false};
  const app={async callServerTool(call) {
    calls.push(call);
    if(call.name==='gil_revisit_review') {
      if(state.failRead) return {isError:true,structuredContent:{problem:'protected path differs'}};
      return {structuredContent:state.value,_meta:{'gil/revisitApprovalToken':state.token}};
    }
    assert.equal(call.arguments.token,'private');
    if(state.refuseApply) return {isError:true,structuredContent:{problem:'changed since review'}};
    state.value={...state.value,state:'completed'};
    if(state.loseReply) throw new Error('reply lost');
    return {structuredContent:state.value};
  }, async sendMessage(message) {
    messages.push(message);
    if (state.failMessage) throw new Error('Host cannot accept message');
    return state.messageError ? {isError:true} : {};
  }};
  const review=makeRecoveryReview({app,scope:'scope1',panel,document:doc,changed:()=>{changes++;}});
  return {review,panel,calls,messages,state,get changes(){return changes;}};
}
test('reading and synthetic clicks do not approve; explicit click sends exact capability once',async()=>{
  const r=rig();await r.review.refresh();await r.review.refresh();
  assert.equal(r.calls.filter(c=>c.name==='gil_revisit_apply').length,0);
  assert.match(r.panel.allText(),/<img onerror=attack>/);assert.equal(r.panel.querySelectorAll('img').length,0);
  assert.ok(!r.panel.allText().includes('private'));
  const b=r.panel.querySelectorAll('button')[0];await b.click(false);
  assert.equal(r.calls.length,2);await b.click(true);await b.click(true);
  assert.equal(r.calls.filter(c=>c.name==='gil_revisit_apply').length,1);
  assert.match(r.panel.allText(),/복원을 마쳤습니다/);assert.equal(r.changes,1);
  assert.equal(r.messages.length,1);
  assert.match(r.messages[0].content[0].text,/gil_context/);
  assert.doesNotMatch(JSON.stringify(r.messages),/private|revisit-preserved|report\.md|<img|digest/);
  await r.review.refresh();assert.equal(r.messages.length,1);
});
test('lost apply reply is reconciled with read, never automatically retried',async()=>{
  const r=rig();r.state.loseReply=true;await r.review.refresh();
  await r.panel.querySelectorAll('button')[0].click(true);
  assert.match(r.panel.allText(),/자동으로 다시 실행하지 않습니다/);
  await r.review.refresh();assert.match(r.panel.allText(),/복원을 마쳤습니다/);
  assert.equal(r.calls.filter(c=>c.name==='gil_revisit_apply').length,1);
  assert.equal(r.messages.length,0,'uncertain writes and polling must not send chat messages');
  const resume=r.panel.querySelectorAll('button')[0];await resume.click(false);
  assert.equal(r.messages.length,0);await resume.click(true);await resume.click(true);
  assert.equal(r.messages.length,1);assert.equal(r.calls.filter(c=>c.name==='gil_revisit_apply').length,1);
});
test('failed read clears old approval, missing token disables, stop makes detached controls inert',async()=>{
  const r=rig();await r.review.refresh();const old=r.panel.querySelectorAll('button')[0];
  r.state.failRead=true;await r.review.refresh();await old.click(true);
  assert.match(r.panel.allText(),/protected path differs/);
  assert.equal(r.calls.filter(c=>c.name==='gil_revisit_apply').length,0);
  r.state.failRead=false;r.state.token=null;await r.review.refresh();
  assert.equal(r.panel.querySelectorAll('button')[0].disabled,true);
  r.state.token='private';await r.review.refresh();const b=r.panel.querySelectorAll('button')[0];
  r.review.stop();await b.click(true);assert.equal(r.calls.filter(c=>c.name==='gil_revisit_apply').length,0);
});

test('chat delivery failure leaves completed recovery intact and retry only sends a message',async()=>{
  for (const failure of ['failMessage','messageError']) {
    const r=rig();r.state[failure]=true;await r.review.refresh();
    await r.panel.querySelectorAll('button')[0].click(true);
    assert.match(r.panel.allText(),/복원을 마쳤습니다/);
    assert.match(r.panel.allText(),/채팅 전달이 확인되지/);
    assert.equal(r.messages.length,1);
    await r.review.refresh();assert.equal(r.messages.length,1);
    r.state[failure]=false;await r.panel.querySelectorAll('button')[0].click(true);
    assert.equal(r.messages.length,2);assert.match(r.panel.allText(),/채팅에 계속 요청을 전달/);
    assert.equal(r.calls.filter(c=>c.name==='gil_revisit_apply').length,1);
  }
});

test('opening a completed recovery never sends automatically, and stopped panels cannot notify',async()=>{
  const r=rig();r.state.value.state='completed';await r.review.refresh();await r.review.refresh();
  assert.equal(r.messages.length,0);
  const b=r.panel.querySelectorAll('button')[0];r.review.stop();await b.click(true);
  assert.equal(r.messages.length,0);assert.equal(r.calls.filter(c=>c.name==='gil_revisit_apply').length,0);
});

test('a refused apply never tells the chat that recovery completed',async()=>{
  const r=rig();r.state.refuseApply=true;await r.review.refresh();
  await r.panel.querySelectorAll('button')[0].click(true);
  assert.equal(r.messages.length,0);assert.equal(r.changes,0);
  assert.doesNotMatch(r.panel.allText(),/복원을 마쳤습니다/);
});
