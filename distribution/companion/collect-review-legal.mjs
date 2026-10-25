// Maintainer-only, explicit network retrieval of immutable upstream legal originals.
// Never run during installation or normal packaging. Generates a new, reviewable snapshot.
import {execFileSync} from 'node:child_process';
import {readFile,mkdir,writeFile} from 'node:fs/promises';
import {dirname,join,resolve} from 'node:path';
import assert from 'node:assert/strict';
import {sha256} from '../compliance/notices.mjs';
const repo=resolve(import.meta.dirname,'../..');
const groups=[
  ['dropbox/rust-alloc-no-stdlib','ae42d22078b98549e987d2f03d12df7b984fde47',['alloc-stdlib@0.2.4'],['LICENSE']],
  ['servo/stylo','635e1a19d02960588a00e189bd4bd5bdb150ec3d',['selectors@0.36.1'],['selectors/lib.rs']],
  ['tauri-apps/tauri','6f6ab1207bb3923c2721fbc67d2fdb1c8deb0c7a',['tauri-plugin@2.6.3'],['LICENSE_APACHE-2.0','LICENSE_MIT']],
  ['open-i18n/rust-unic','5878605364af97a3358368a6eaef02104af2e016',['unic-char-property@0.9.0','unic-char-range@0.9.0','unic-common@0.9.0','unic-ucd-version@0.9.0'],['COPYRIGHT.md','LICENSE-APACHE','LICENSE-MIT']],
  ['open-i18n/rust-unic','8a6ce83063d90b91ae2ce59eddb803edd393fca9',['unic-ucd-ident@0.9.0'],['COPYRIGHT.md','LICENSE-APACHE','LICENSE-MIT']],
  ['wravery/webview2-rs','b74dc5e2b394044bea5191052868ce7a106c202c',['webview2-com@0.38.2','webview2-com-sys@0.38.2'],['LICENSE']],
  ['wravery/webview2-rs','dffa41a8a46d3f5565eefbff2de57d38d399f158',['webview2-com-macros@0.8.1'],['LICENSE']],
];
const metadata=JSON.parse(execFileSync('cargo',['metadata','--offline','--locked','--format-version','1','--filter-platform','x86_64-pc-windows-msvc'],{cwd:repo,maxBuffer:64e6}));
const out=join(import.meta.dirname,'legal');await mkdir(out);await mkdir(join(out,'texts'));
const entries={};const stored=new Set();
async function keep(bytes,path,url){
  assert.ok(bytes.length>20&&!bytes.includes(0),'invalid legal text');
  assert.ok(Buffer.from(bytes.toString('utf8')).equals(bytes));
  const digest=sha256(bytes);
  if(!stored.has(digest)){await writeFile(join(out,'texts',`${digest}.json`),JSON.stringify({text:bytes.toString('utf8')})+'\n',{flag:'wx'});stored.add(digest);}
  return {path,sha256:digest,url};
}
for(const [repository,commit,packages,paths] of groups){
  const files=[];
  for(const path of paths){
    const data=JSON.parse(execFileSync('gh',['api',`repos/${repository}/contents/${path}?ref=${commit}`],{maxBuffer:8e6}));
    assert.equal(data.encoding,'base64');assert.equal(data.path,path);
    files.push(await keep(Buffer.from(data.content,'base64'),path,`https://github.com/${repository}/blob/${commit}/${path}`));
  }
  if(repository==='servo/stylo'){
    const url='https://www.mozilla.org/media/MPL/2.0/index.txt';
    const bytes=execFileSync('curl',['--fail','--silent','--show-error','--location',url],{maxBuffer:1e6});
    assert.ok(bytes.toString().includes('Mozilla Public License Version 2.0'));
    files.push(await keep(bytes,'MPL-2.0.txt',url));
  }
  for(const key of packages){
    const p=metadata.packages.find(p=>`${p.name}@${p.version}`===key);assert.ok(p);
    const vcs=JSON.parse(await readFile(join(dirname(p.manifest_path),'.cargo_vcs_info.json')));
    assert.equal(vcs.git.sha1,commit);
    entries[key]={repository:`https://github.com/${repository}`,commit,crate_path:vcs.path_in_vcs??null,files,
      review:'Exact crate VCS revision; upstream root license/copyright originals retained. selectors also retains its source header and the official MPL 2.0 text. Source redistribution link is the pinned repository.'};
  }
}
await writeFile(join(out,'upstream.json'),JSON.stringify(entries,null,2)+'\n',{flag:'wx'});
console.log(`Retrieved ${Object.keys(entries).length} pinned package entries / ${stored.size} distinct originals. Review before publication.`);
