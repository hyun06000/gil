// Opt-in live tool-sequence evaluation. Requires authorized model usage and a built Core.
// Only synthetic projects/data; no installation, real markets, user records or configuration edits.
import {spawn} from 'node:child_process';
import {createInterface} from 'node:readline';
import {mkdtemp, mkdir, readFile, writeFile, copyFile, chmod} from 'node:fs/promises';
import {appendFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {resolve, join} from 'node:path';
import assert from 'node:assert/strict';
const repo=resolve(import.meta.dirname,'..');
// Pass the binary explicitly: Host MCP subprocess environments need not inherit GIL_BINARY.
let binary=(process.argv[2]==='--proxy'?process.argv[5]:process.env.GIL_BINARY) || join(repo,'target/debug/gil');
const sourceTool={name:'read_synthetic_source',description:'Read one fictional source for this evaluation. IDs: price or earnings. No live market data.',
  annotations:{readOnlyHint:true,destructiveHint:false,idempotentHint:true,openWorldHint:false},
  inputSchema:{type:'object',properties:{id:{type:'string',enum:['price','earnings']}},required:['id'],additionalProperties:false}};
const writeTool={name:'save_synthetic_evidence',description:'Save your actual observations or report as a durable UTF-8 document in this isolated evaluation project. Returns the project-relative evidence_files path. Use factual content, source IDs, method, results and limitations.',
  annotations:{readOnlyHint:false,destructiveHint:false,idempotentHint:false,openWorldHint:false},
  inputSchema:{type:'object',properties:{text:{type:'string',minLength:1,maxLength:65536}},required:['text'],additionalProperties:false}};
const sources={price:'Fictional company SAMPLE, source PRICE-1: adjusted closing price 100 on 2025-01-02 and 120 on 2025-12-30, same currency and split-adjusted basis.',
  earnings:'Fictional company SAMPLE, source EARNINGS-1: annual revenue 200 in 2024 and 300 in 2025, same currency and accounting scope.'};
function connect(settings) {
  const child=spawn(binary,['mcp','--serve'],{cwd:tmpdir(),env:{...process.env,GIL_MONITOR_STATE_DIR:settings},stdio:['pipe','pipe','pipe']});
  child.stderr.resume(); let id=0; const waiting=new Map();
  createInterface({input:child.stdout}).on('line',line=>{const v=JSON.parse(line);waiting.get(v.id)?.(v);waiting.delete(v.id);});
  child.on('exit',()=>{for(const done of waiting.values())done({error:{message:'Core exited'}});waiting.clear();});
  const send=v=>child.stdin.write(JSON.stringify(v)+'\n');
  const rpc=(method,params)=>new Promise((res,rej)=>{
    const n=++id, timer=setTimeout(()=>{waiting.delete(n);rej(new Error(`Timeout: ${method}`));},20000);
    waiting.set(n,v=>{clearTimeout(timer);v.error?rej(new Error(JSON.stringify(v.error))):res(v.result);});
    send({jsonrpc:'2.0',id:n,method,params});
  });
  return {child,rpc,send};
}
if(process.argv[2]==='--proxy') {
  const root=process.argv[3], events=process.argv[4];
  const core=connect(join(root,'..','bindings')); let written=0;
  // Log observations, never reject research because of GIL phase: that would make the test pass
  // through a gate the shipped product does not possess.
  for await(const line of createInterface({input:process.stdin})) {
    const req=JSON.parse(line);
    if(req.id===undefined){core.send(req);continue;}
    let result;
    try {
      if(req.method==='tools/call' && req.params.name===sourceTool.name) {
        const text=sources[req.params.arguments?.id];
        result={content:[{type:'text',text:text || 'Unknown fictional source'}],isError:!text};
      } else if(req.method==='tools/call' && req.params.name===writeTool.name) {
        const text=req.params.arguments?.text;
        if(typeof text!=='string'||!text.trim()||Buffer.byteLength(text)>65536)throw new Error('Nonempty document up to 64 KiB required');
        const path=`evidence/observation-${++written}.md`;
        await mkdir(join(root,'evidence'),{recursive:true});
        await writeFile(join(root,path),text,{flag:'wx'});
        result={content:[{type:'text',text:JSON.stringify({saved:true,path})}],isError:false};
      } else if(req.method==='tools/call' && req.params.arguments?.project_root && req.params.arguments.project_root!==root) {
        throw new Error('Evaluation only permits the selected temporary project');
      } else {
        result=await core.rpc(req.method,req.params);
        if(req.method==='tools/list') result.tools.push(sourceTool,writeTool);
      }
      if(req.method==='tools/call') appendFileSync(events,JSON.stringify({request:{name:req.params.name,arguments:req.params.arguments},result})+'\n');
      process.stdout.write(JSON.stringify({jsonrpc:'2.0',id:req.id,result})+'\n');
    } catch(e) {
      if(req.method==='tools/call')appendFileSync(events,JSON.stringify({request:{name:req.params.name,arguments:req.params.arguments},result:{isError:true},error:e.message})+'\n');
      process.stdout.write(JSON.stringify({jsonrpc:'2.0',id:req.id,error:{code:-32602,message:e.message}})+'\n');
    }
  }
  core.child.stdin.end();
} else {
  const work=await mkdtemp(join(tmpdir(),'gil-interview-workflow-'));
  const pinned=join(work,process.platform==='win32'?'gil-core.exe':'gil-core');
  await copyFile(binary,pinned); await chmod(pinned,0o700); binary=pinned;
  console.log(`Synthetic workflow evidence: ${work}`);
  const summary=[];
  const selected=(process.env.GIL_EVAL_MODES||'direct-mcp,plugin-skill').split(',');
  assert.ok(selected.every(mode=>['direct-mcp','plugin-skill'].includes(mode)),'Unknown evaluation mode');
  for(const [mode,position] of [['direct-mcp','synthesis'],['plugin-skill','outcome']].filter(([mode])=>selected.includes(mode))) {
    const dir=join(work,mode), root=join(dir,'project');await mkdir(root,{recursive:true});
    const core=connect(join(dir,'bindings'));
    const call=async(name,args={})=>{
      const r=await core.rpc('tools/call',{name,arguments:{project_root:root,...args}});
      assert.equal(r.structuredContent?.ok,true,JSON.stringify(r));return r;
    };
    try {
      await core.rpc('initialize',{protocolVersion:'2024-11-05',capabilities:{},clientInfo:{name:'workflow-fixture',version:'1'}});
      core.send({jsonrpc:'2.0',method:'notifications/initialized'});
      await call('gil_start');
      for(const [kind,report] of [
        ['question','question: 어떤 보고서인가\nchoices: 주가 / 매출\nresponse: SAMPLE 주가 상승률과 별도로 매출 성장률도 확인하고 채팅 보고서로 정리해줘\n'],
        ['interpretation','interpretation: 주가 상승률과 매출 성장률을 각각 확인하고 보고서로 종합한다\nunresolved: 없음\n'],
      ]) {
        await call('gil_open',{kind,contract:'objective: 보고서 목표 합의\nnext_action: 범위를 확인한다\ndone_when: 답을 확인했다\n'});
        await call('gil_close',{report});
      }
      await call('gil_open',{kind:'synthesis',contract:'objective: 보고서 범위 승인\nnext_action: 주가·매출 성장률을 각각 확인하고 종합한다는 제안을 묻는다\ndone_when: 이 제안에 대한 사용자 답을 받았다\n'});
      if(position==='outcome') {
        await call('gil_close',{report:'statement: SAMPLE 주가 상승률과 매출 성장률을 각각 확인하고 채팅 보고서로 종합한다\nbasis_refs: step:C1/S2\napproved: yes\n'});
        await call('gil_open',{kind:'outcome',contract:'objective: 승인된 범위를 판정한다\nnext_action: 승인된 제안을 근거로 판정과 다음 방향을 정한다\ndone_when: 판정과 다음 방향이 정해졌다\n'});
      }
    } finally {core.child.stdin.end();}
    const events=join(dir,'events.jsonl'), output=join(dir,'final.txt');
    await writeFile(events,'');
    const skill=mode==='plugin-skill'?await readFile(join(repo,'plugins/gil-companion-prototype/skills/gil-companion/SKILL.md'),'utf8'):'';
    const prompt=`This is an authorized isolated evaluation using fictional data only. Use the connected GIL tools and read_synthetic_source (price and earnings), plus save_synthetic_evidence for authorized documents inside the temporary project; no shell, web, real financial data or other projects. The user-selected project is ${root}. It already exists; Monitor was previously requested (visibility is not established). The preceding conversation asked approval for: "SAMPLE 주가 상승률과 매출 성장률을 각각 확인하고 채팅 보고서로 종합한다." User reply: "좋아, 그 범위에 동의해. 진행해줘." Continue the actual task from its stored position and deliver the short Korean report in chat. Do not describe a hypothetical plan instead of doing it. ${skill?`Installed Plugin skill for this evaluation:\n${skill}`:''}`;
    const args=['exec','--ignore-user-config','--ephemeral','--sandbox','read-only','--skip-git-repo-check','--model',process.env.GIL_EVAL_MODEL||'gpt-6-luna',
      '-c','model_reasoning_effort="low"','-c','web_search="disabled"','-c','features.shell_tool=false',
      '-c',`mcp_servers.gil.command=${JSON.stringify(process.execPath)}`,
      '-c',`mcp_servers.gil.args=${JSON.stringify([resolve(import.meta.filename),'--proxy',root,events,binary])}`,
      '--json','--output-last-message',output,'-'];
    const cli=spawn(process.env.GIL_CODEX_CLI||'codex',args,{cwd:dir,stdio:['pipe','pipe','pipe']});
    let log='';cli.stdout.on('data',b=>{log+=b;});cli.stderr.on('data',b=>{log+=b;});cli.stdin.end(prompt);
    const timer=setTimeout(()=>cli.kill('SIGTERM'),300000);
    const code=await new Promise((res,rej)=>{cli.on('error',rej);cli.on('close',res);});clearTimeout(timer);
    await writeFile(join(dir,'cli.jsonl'),log);
    const trace=(await readFile(events,'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse);
    let kind=position, interviewClosed=false, cycleClosed=false, experiments=0;
    const reads=[],badReads=[],writes=[],badWrites=[],verifyReports=[],firstSourceCycles=new Map();
    for(const {request,result} of trace) {
      if(request.name===sourceTool.name) {
        const id=request.arguments.id;reads.push(id);
        if(!firstSourceCycles.has(id))firstSourceCycles.set(id,experiments);
        if(!interviewClosed||kind!=='verify')badReads.push({id,kind,interviewClosed});
      }
      if(request.name===writeTool.name && !result.isError) {
        writes.push(JSON.parse(result.content[0].text).path);
        if(!interviewClosed||kind!=='verify')badWrites.push({kind,interviewClosed});
      }
      if(result.structuredContent?.ok!==true)continue;
      if(request.name==='gil_close' && kind==='verify')verifyReports.push(request.arguments.report);
      const said=result.structuredContent.said||'';
      const opened=said.match(/^열었다: .*? · (\w+)/m);
      if(request.name==='gil_open' && opened) {
        kind=opened[1];cycleClosed=false;if(kind==='experiment')experiments++;
      }
      if(request.name==='gil_close') {
        if(/^닫았다: .*? · interview · /m.test(said)){interviewClosed=true;cycleClosed=true;}
        if(/^닫았다: .*? · experiment · /m.test(said))cycleClosed=true;
        kind='boundary';
      }
    }
    const final=await readFile(output,'utf8').catch(()=>'');
    const savedFiles=await Promise.all(writes.map(async path=>({path,content:await readFile(join(root,path),'utf8')})));
    const checks={cliCompleted:code===0,interviewClosed,allResearchInVerify:reads.length>0&&!badReads.length,
      separateExperiments:experiments>=2&&firstSourceCycles.get('price')!==firstSourceCycles.get('earnings'),
      bothSources:reads.includes('price')&&reads.includes('earnings'),
      finalCycleClosed:cycleClosed,
      evidenceSavedInVerify:writes.length>=2&&!badWrites.length,
      everyVerifyCitesSavedEvidence:verifyReports.length>=2&&verifyReports.every(report=>savedFiles.some(f=>f.content.trim()&&report.includes(f.path)&&report.includes('evidence_files:'))),
      finalLinksEvidence:writes.some(path=>final.includes(path)),
      reportHasResults:/\b20(?:\.0+)?\s*%/.test(final)&&/\b50(?:\.0+)?\s*%/.test(final)};
    summary.push({mode,position,checks,experiments,reads,badReads,writes,badWrites,verifyReports,calls:trace.length});
    console.log(JSON.stringify(summary.at(-1)));
  }
  await writeFile(join(work,'summary.json'),JSON.stringify(summary,null,2));
  assert.ok(summary.every(s=>Object.values(s.checks).every(Boolean)),'Workflow failed; inspect synthetic evidence directory');
}
