import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { normalize, report } from './report.mjs';
const root=resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const [profile,outArg,targetArg]=process.argv.slice(2);
if (!['default','product'].includes(profile) || !outArg || !targetArg) throw new Error('Usage: node tools/chain-resume-benchmark/run.mjs default|product OUTPUT_DIR TARGET_DIR');
const out=resolve(outArg), target=resolve(targetArg);
mkdirSync(out); // Never overwrite a prior run.
const temp=mkdtempSync(join(tmpdir(),'gil-resume-capture-'));
const run=(bin,args,env={})=>execFileSync(bin,args,{cwd:root,encoding:'utf8',env:{...process.env,...env},maxBuffer:16*1024*1024});
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
try {
  const features=profile==='product'?['approved-plan','visual-authoring','product-safety']:[];
  const args=['test','--locked','-p','gil','--no-default-features','--test','chain_resume_benchmark','--target-dir',target,...(features.length?['--features',features.join(',')]:[])];
  const stdout=run('cargo',args,{GIL_RESUME_BENCH_OUTPUT:temp});
  const events=readFileSync(join(temp,'events.jsonl'),'utf8').trim().split('\n').map(JSON.parse);
  const capture=JSON.parse(readFileSync(join(temp,'capture.json'),'utf8'));
  const normalized=normalize(events,capture.root,capture.scope);
  const locale=JSON.parse(readFileSync(join(root,'src/i18n/ko.json')));
  const inputs=['tests/chain_resume_benchmark.rs','tests/support_wave1/mod.rs','src/mcp/workflow.md','src/i18n/ko.json','spec/gil-spec.yaml','Cargo.lock','tools/chain-resume-benchmark/run.mjs','tools/chain-resume-benchmark/report.mjs',...run('git',['ls-files','src/manual','tools/context-metrics/lib']).trim().split('\n').filter(Boolean)];
  const result={case_version:1,profile,features,locale:'ko',source_commit:run('git',['rev-parse','HEAD']).trim(),source_dirty:!!run('git',['status','--porcelain']).trim(),binary_sha256:capture.binary_sha256,input_sha256:Object.fromEntries(inputs.map(p=>[p,hash(readFileSync(join(root,p)))])),environment:{platform:process.platform,arch:process.arch,node:process.version,rust:run('rustc',['--version']).trim(),cargo:run('cargo',['--version']).trim()},command:['cargo',...args.map(x=>x===target?'<dedicated-target>':x)],tool_calls:capture.tool_calls,checks:capture.checks,unsupported:profile==='default'?['plan waiting','request changes','saved approval','item binding','Chain report','closed Chain','next Chain']:[],normalization:['Replace exact project root with <project_root> and exact transport scope ID with <scope_id> in string values; no other replacement, time masking or truncation.','Raw: actual response text/structuredContent and serialized event sizes before replacement. Normalized: same measures after replacement. JSON-RPC framing and initialize instructions excluded; instructions identified by hashes.','Two fresh processes per checkpoint, each gil_context then gil_help(topic omitted); gil_plan_read once per product pass is an assertion-only read, excluded from measured events. Setup and App decisions counted separately in tool_calls.'],raw:report(events,locale),normalized:report(normalized,locale),unmeasured:['actual model tokens','model cost','Host model input composition','task quality','human consent','Host UI'],test_passed:stdout.includes('1 passed')};
  writeFileSync(join(out,'baseline.json'),JSON.stringify(result,null,2)+'\n');
  writeFileSync(join(out,'events.jsonl'),normalized.map(e=>JSON.stringify(e)).join('\n')+'\n');
  console.log(JSON.stringify({profile,calls:result.raw.calls,text:result.raw.totals.text_response,checkpoints:result.checks.length}));
} finally { rmSync(temp,{recursive:true,force:true}); }
