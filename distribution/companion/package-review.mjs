// Maintainer-only portable review cohort. No install, process launch, network or publication.
import assert from 'node:assert/strict';
import {readFile, writeFile, mkdir, copyFile, lstat} from 'node:fs/promises';
import {resolve, join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {execFileSync} from 'node:child_process';
import {sha256, regular, legalFiles, lockChecksums, loadBundle} from '../compliance/notices.mjs';

export function peMachine(bytes, arch) {
  assert.ok(bytes.length>=256 && bytes.toString('ascii',0,2)==='MZ','PE required');
  const at=bytes.readUInt32LE(0x3c);
  assert.ok(at>=64 && at+26<=bytes.length,'truncated PE');
  assert.equal(bytes.readUInt32LE(at),0x4550,'PE signature');
  assert.equal(bytes.readUInt16LE(at+4),{x86_64:0x8664,aarch64:0xaa64}[arch],'wrong native CPU');
  assert.equal(bytes.readUInt16LE(at+24),0x20b,'PE32+ required');
}
export function receipt(companion) {
  return {schema:'gil-companion-install/1',protocol:1,companion_file:'gil-companion.exe',companion_sha256:sha256(companion)};
}
export async function notices(repo, target, out) {
  const run=(args)=>execFileSync('cargo',args,{cwd:repo,encoding:'utf8',maxBuffer:64e6});
  const metadata=JSON.parse(run(['metadata','--locked','--offline','--format-version','1','--filter-platform',target]));
  const used=new Set(run(['tree','--locked','--offline','--target',target,'-p','gil','-p','gil-companion','--edges','normal,build','--prefix','none','--format','{p}']).split('\n').map(s=>s.match(/^(\S+) v([^\s]+)/)).filter(Boolean).map(m=>`${m[1]}@${m[2]}`));
  const checks=lockChecksums(await readFile(join(repo,'Cargo.lock'),'utf8'));
  const fallback=JSON.parse(await readFile(join(repo,'distribution/compliance/upstream.json'),'utf8'));
  const companionFallback=JSON.parse(await readFile(join(repo,'distribution/companion/legal/upstream.json'),'utf8'));
  const rows=[];const texts=[];const gaps=[];
  for(const p of metadata.packages.filter(p=>used.has(`${p.name}@${p.version}`)&&!metadata.workspace_members.includes(p.id))) {
    const dir=resolve(p.manifest_path,'..');const key=`${p.name}@${p.version}`;
    assert.equal(p.source,'registry+https://github.com/rust-lang/crates.io-index');
    assert.ok(checks.get(key),`unlocked ${key}`);
    let files=await legalFiles(dir);let stored=false;let textRoot=join(repo,'distribution/compliance/texts');let upstream;
    if(!files.length) {
      const f=fallback[key]??companionFallback[key];if(!f){gaps.push(key);continue;}
      if(!fallback[key])textRoot=join(repo,'distribution/companion/legal/texts');
      const vcs=JSON.parse(await readFile(join(dir,'.cargo_vcs_info.json'),'utf8'));
      assert.equal(vcs.git.sha1,f.commit);assert.equal(vcs.path_in_vcs??null,f.crate_path??null);
      files=f.files; stored=true;upstream=f;
    }
    for(const file of files){const data=stored?Buffer.from(JSON.parse(await regular(join(textRoot,`${file.sha256}.json`))).text):await regular(join(dir,file.path));assert.equal(sha256(data),file.sha256);texts.push(`\n=== ${key} / ${file.path} ===\n${data.toString('utf8')}`);}
    rows.push({name:p.name,version:p.version,license:p.license,checksum:checks.get(key),files,...(upstream?{upstream}:{})});
  }
  if(gaps.length){
    await writeFile(join(out,'LEGAL-GAPS.json'),JSON.stringify({publishable:false,missing_original_license_text:gaps},null,2));
    assert.fail(`legal text missing (no archive may be published): ${gaps.join(', ')}`);
  }
  await writeFile(join(out,'THIRD-PARTY-NOTICES.txt'),texts.join('\n'));
  await writeFile(join(out,'THIRD-PARTY-NOTICES.json'),JSON.stringify({scope:'Core + Companion, including build dependencies; legal review still required before publication',packages:rows},null,2));
  const core=await loadBundle(repo,{directory:'distribution/windows/compliance',target,policyName:target.startsWith('aarch64')?'notice-policy-arm64.json':'notice-policy.json'});
  for(const [name,bytes] of core) await writeFile(join(out,name==='RUST-STDLIB-NOTICES.html'?name:`CORE-${name}`),bytes);
}
export async function packageReview({repo,bins,out,arch,source}) {
  assert.ok(['x86_64','aarch64'].includes(arch));assert.match(source,/^[a-f0-9]{40}$/);
  // The workflow builds the alternate identifier; refuse to label an ordinary GUI as isolated.
  const build=JSON.parse(await regular(join(bins,'review-build.json')));
  assert.equal(build.identifier,'dev.ariadne.gil.companion.review');assert.equal(build.source,source);
  const names=['gil.exe','gil-companion.exe','gil-runtime-testbed.exe'];const images={};
  for(const name of names){images[name]=await regular(join(bins,name));peMachine(images[name],arch);assert.equal(build.files[name],sha256(images[name]),`build digest: ${name}`);}
  await mkdir(out); // exclusive final tree; never merge into an earlier bundle
  const files={};
  for(const name of names){await writeFile(join(out,name),images[name],{flag:'wx'});files[name]=sha256(images[name]);}
  const install=JSON.stringify(receipt(images['gil-companion.exe']),null,2)+'\n';
  await writeFile(join(out,'companion-install.json'),install,{flag:'wx'});files['companion-install.json']=sha256(install);
  await notices(repo,`${arch}-pc-windows-msvc`,out);
  await copyFile(join(repo,'LICENSE'),join(out,'LICENSE'));
  await copyFile(join(repo,'distribution/companion/INSTALL-review.md'),join(out,'INSTALL.md'));
  await writeFile(join(out,'START-REVIEW.cmd'),'@echo off\r\n"%~dp0gil-runtime-testbed.exe" companion-review\r\nset "gilReviewExit=%ERRORLEVEL%"\r\necho Review exit code: %gilReviewExit%\r\npause\r\nexit /b %gilReviewExit%\r\n');
  await writeFile(join(out,'review-bundle.json'),JSON.stringify({schema:'gil-companion-review/1',source,arch,files,publishable:false,channel:'unsigned-portable-review',identifier:build.identifier},null,2));
  const {readdir}=await import('node:fs/promises');const sums=[];
  for(const name of (await readdir(out)).sort()){assert.ok((await lstat(join(out,name))).isFile());sums.push(`${sha256(await readFile(join(out,name)))}  ${name}`);}
  await writeFile(join(out,'SHA256SUMS'),sums.join('\n')+'\n');
  return {out,files};
}
if(process.argv[1]&&resolve(process.argv[1])===fileURLToPath(import.meta.url)) {
  const [bins,out,arch,source]=process.argv.slice(2);assert.ok(source,'usage: package-review.mjs BINS NEW_OUT x86_64|aarch64 SOURCE_SHA');
  await packageReview({repo:resolve(import.meta.dirname,'../..'),bins:resolve(bins),out:resolve(out),arch,source});
}
