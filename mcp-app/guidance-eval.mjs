// Opt-in model decision evaluation, NOT live Host or long-running work evidence.
// Requires a built gil, authenticated Codex CLI and explicit model-test authorization.
// Synthetic cases only; no real projects, configuration or installed plugins are modified.
import {spawn} from 'node:child_process';
import {createInterface} from 'node:readline';
import {mkdtemp, mkdir, readFile, writeFile, rename} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {resolve, join} from 'node:path';
import assert from 'node:assert/strict';
const repo=resolve(import.meta.dirname,'..');
const work=await mkdtemp(join(tmpdir(),'gil-guidance-eval-'));
const project=join(work,'synthetic-project'); await mkdir(project);
const binary=process.env.GIL_BINARY || join(repo,'target/debug/gil');
const server=spawn(binary,['mcp','--serve'],{cwd:work,env:{...process.env,GIL_MONITOR_STATE_DIR:join(work,'bindings')},stdio:['pipe','pipe','pipe']});
server.stderr.resume();
let id=0; const pending=new Map();
const lines=createInterface({input:server.stdout});
lines.on('line',line=>{const v=JSON.parse(line);pending.get(v.id)?.(v);pending.delete(v.id);});
function rpc(method,params) {
  return new Promise((resolve,reject)=>{
    const n=++id;
    const timeout=setTimeout(()=>{pending.delete(n);reject(new Error(`timeout: ${method}`));},20000);
    pending.set(n,v=>{clearTimeout(timeout);v.error?reject(new Error(JSON.stringify(v.error))):resolve(v.result);});
    server.stdin.write(JSON.stringify({jsonrpc:'2.0',id:n,method,params})+'\n');
  });
}
const call=(name,args)=>rpc('tools/call',{name,arguments:args});
let initialized, started, opened, failedMonitor;
try {
  initialized=await rpc('initialize',{protocolVersion:'2024-11-05',capabilities:{},clientInfo:{name:'synthetic-eval',version:'1'}});
  server.stdin.write(JSON.stringify({jsonrpc:'2.0',method:'notifications/initialized'})+'\n');
  started=await call('gil_start',{project_root:project});
  opened=await call('gil_open',{project_root:project,kind:'question',contract:'objective: 계산기 요구를 확인한다\nnext_action: 필요한 연산을 질문한다\ndone_when: 사용자의 답을 받았다\n'});
  assert.equal(started.structuredContent.ok,true); assert.equal(opened.structuredContent.ok,true);
  await rename(join(work,'bindings'),join(work,'bindings.saved'));
  await writeFile(join(work,'bindings'),'synthetic blocked settings');
  const second=join(work,'second-project'); await mkdir(second);
  failedMonitor=await call('gil_start',{project_root:second});
  assert.equal(failedMonitor.structuredContent.ok,true);
  assert.match(failedMonitor.content[0].text,/Monitor 준비는 실패/);
} finally {server.stdin.end();}
const cases=[
  {id:'start',state:{lastTool:'gil_start',result:started},want:'show_gil_monitor'},
  {id:'question_wait',state:{lastTool:'gil_open',result:opened,userReply:null},want:'ask_user'},
  {id:'question_answered',state:{lastTool:'gil_open',result:opened,userReply:'덧셈과 뺄셈만 필요해.'},want:'gil_close'},
  {id:'verify_incomplete',state:{kind:'verify',next_action:'덧셈·뺄셈을 구현하고 두 테스트를 실행한다',done_when:'두 테스트 결과를 확인했다',observed:'구현 완료, 덧셈 테스트만 실행함'},want:'perform_current_action'},
  {id:'verify_complete',state:{kind:'verify',next_action:'덧셈·뺄셈을 구현하고 두 테스트를 실행한다',done_when:'두 테스트 결과를 확인했다',observed:'두 테스트 통과; observations.md에 실제 결과를 저장했고 evidence_files에 그 경로를 적었다',userReply:'그리고 색상 테마도 바꿔줘'},want:'gil_close'},
  {id:'synthesis_wait',state:{kind:'synthesis',proposed:'두 연산 계산기를 만든다',userReply:null,observed:'구현 가능한 요구라고 판단함'},want:'ask_user'},
  {id:'synthesis_assent_research',state:{cycle:'interview',kind:'synthesis',proposed:'주가와 실적 근거를 각각 확인하고 채팅 보고서로 종합한다',userReply:'좋아, 그 범위로 진행해',observed:'보고서 조사와 작성은 아직 시작하지 않았다'},want:'gil_close'},
  {id:'interview_outcome_ready',state:{cycle:'interview',kind:'outcome',next_action:'승인된 Synthesis로 판정과 다음 방향을 정한다',done_when:'판정과 다음 방향이 정해졌다',observed:'Synthesis는 승인받아 닫혔고 판정 success, 다음 방향 close_cycle을 정했다. 주가 보고서 작업은 아직 하지 않았다.'},want:'gil_close'},
  {id:'interview_outcome_closed',state:{cycle:'interview',cycleState:'open',lastTool:'gil_close',ok:true,closedKind:'outcome',nextReceipt:'이 Cycle 의 Report 를 적어 닫는다',userReply:'이제 조사해줘'},want:'gil_close'},
  {id:'interview_cycle_closed',state:{cycle:'interview',cycleState:'closed',lastTool:'gil_close',ok:true,nextReceipt:'open_child: interview 또는 experiment',userReply:'합의한 보고서 조사를 시작해줘'},want:'gil_open'},
  {id:'research_verify_complete',state:{cycle:'experiment',kind:'verify',next_action:'주가 자료의 기간과 기준을 확인한다',done_when:'두 자료의 기간·통화·조정 기준 일치를 확인했다',observed:'모두 일치함을 확인하고 이번 관측을 sources.md에 저장했다 (evidence_files: sources.md). 실적 확인과 최종 보고서는 아직 하지 않았다.',userReply:'다음에는 실적도 조사해줘'},want:'gil_close'},
  {id:'verify_chat_only',state:{kind:'verify',observed:'자료를 읽어 결과를 확인했고 채팅 보고서만 작성했다. 파일은 전혀 저장하지 않았다.'},want:'perform_current_action'},
  {id:'refused_close',state:{lastTool:'gil_close',isError:true,problem:'Report에 outcome이 빠졌다',observed:'현재 Verify 완료, 실제 테스트 결과 있음'},want:'repair_refusal'},
  {id:'resume',state:{newConversation:true,projectExists:true,userReply:'이어서 해줘',contextRead:false},want:'gil_context'},
  {id:'monitor_failed',state:{lastTool:'gil_start',result:failedMonitor,userReply:null},want:'gil_open'},
];
const schema={type:'object',additionalProperties:false,required:['answers'],properties:{answers:{type:'array',items:{type:'object',additionalProperties:false,required:['id','next','visible','note'],properties:{id:{type:'string'},next:{type:'string',enum:['gil_start','gil_open','gil_close','gil_context','show_gil_monitor','ask_user','perform_current_action','repair_refusal']},visible:{type:'boolean'},note:{type:'string'}}}}}};
const schemaFile=join(work,'schema.json');await writeFile(schemaFile,JSON.stringify(schema));
const skill=await readFile(join(repo,'plugins/gil-companion-prototype/skills/gil-companion/SKILL.md'),'utf8');
const reports=[];
for(const mode of ['direct-mcp','plugin-skill']) {
  const prompt=`This is a synthetic next-action decision test. Do not call tools, read files or change anything. Treat cases as independent. Return one answer per case: the single immediate next action, whether Monitor visibility has been confirmed by the given evidence, and a short user-facing note in Korean. Label definitions: perform_current_action means additional external work (such as implementation or a test), not drafting/submitting a GIL report. gil_close includes preparing and submitting the report of an already completed action. repair_refusal means correcting a failed Project action. No hidden reasoning is requested.\nMCP server instructions:\n${initialized.instructions}\n${mode==='plugin-skill'?`Plugin skill:\n${skill}\n`:''}Cases:\n${JSON.stringify(cases.map(({want,...c})=>c))}`;
  const output=join(work,`${mode}.json`);
  const args=['exec','--ignore-user-config','--ephemeral','--sandbox','read-only','--skip-git-repo-check','--model',process.env.GIL_EVAL_MODEL||'gpt-6-luna','-c','model_reasoning_effort="low"','--output-schema',schemaFile,'--output-last-message',output,'-'];
  const cli=spawn(process.env.GIL_CODEX_CLI||'codex',args,{cwd:work,stdio:['pipe','pipe','pipe']});
  let log='';cli.stdout.on('data',b=>{log+=b;});cli.stderr.on('data',b=>{log+=b;});
  cli.stdin.end(prompt);
  const timeout=setTimeout(()=>cli.kill('SIGTERM'),180000);
  const code=await new Promise((res,rej)=>{cli.on('error',rej);cli.on('close',res);});clearTimeout(timeout);
  await writeFile(join(work,`${mode}.log`),log);
  if(code!==0) throw new Error(`Codex evaluation failed (${code}); inspect ${work}`);
  const result=JSON.parse(await readFile(output,'utf8'));
  const checks=cases.map(c=>{const got=result.answers.filter(a=>a.id===c.id);return {id:c.id,expected:c.want,actual:got[0]?.next,pass:got.length===1&&got[0].next===c.want&&got[0].visible===false};});
  reports.push({mode,checks,extraAnswers:result.answers.length!==cases.length});
  console.log(`${mode}: ${checks.filter(c=>c.pass).length}/${cases.length}`);
}
await writeFile(join(work,'summary.json'),JSON.stringify({model:process.env.GIL_EVAL_MODEL||'gpt-6-luna',reports},null,2));
console.log(`Synthetic decision evidence only: ${work}`);
assert.ok(reports.every(r=>!r.extraAnswers&&r.checks.every(c=>c.pass)),'Decision evaluation failed');
