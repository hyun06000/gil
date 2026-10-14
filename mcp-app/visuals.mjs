// App-only visual transport and lifecycle. No filesystem URLs or model messages.
import {imageZoom} from './image-zoom.mjs';
const CHUNK = 256 * 1024, MAX = 16 * 1024 * 1024;
const sameOwner = (a, b) => a?.kind === b.kind && a?.ref === b.ref;
const invalid = () => Object.assign(new Error('시각 자료 응답을 검증하지 못했습니다.'), { code: 'invalid_visual' });
const check = (data, scope, owner) => {
  if (data?.schema_version !== 1 || data.scope_id !== scope || !sameOwner(data.owner, owner)) throw invalid();
};
// Serialize assemblies across remounted panels; a superseded request cannot retain
// one full allocation per rapid click while the Host is still answering it.
let assemblies = Promise.resolve();
export function readImage(...args) {
  const job = assemblies.then(() => assembleImage(...args));
  assemblies = job.catch(() => {});
  return job;
}
async function assembleImage(call, scope, owner, item, alive = () => true, stage = () => {}) {
  stage('metadata');
  if (!alive()) throw invalid();
  const size = item.byte_length;
  if (!Number.isSafeInteger(size) || size < 1 || size > MAX || item.mime_type !== 'image/png'
      || !/^[0-9a-f]{64}$/.test(item.sha256) || !/^[1-9][0-9]*$/.test(item.asset_id)
      || !Number.isInteger(item.width) || !Number.isInteger(item.height) || item.width < 1 || item.height < 1
      || item.width > 8192 || item.height > 8192 || item.width * item.height > 16777216) throw invalid();
  const bytes = new Uint8Array(size);
  for (let offset = 0; offset < size;) {
    if (!alive()) throw invalid();
    stage('image_tool');
    const data = await call('gil_monitor_report_image', { owner, asset_id: item.asset_id, offset });
    stage('chunk_validation');
    if (!alive()) throw invalid();
    check(data, scope, owner);
    const end = Math.min(offset + CHUNK, size);
    if (data.asset_id !== item.asset_id || data.sha256 !== item.sha256 || data.mime_type !== 'image/png'
        || data.total_bytes !== size || data.offset !== offset || data.next_offset !== (end < size ? end : null)
        || typeof data.data_base64 !== 'string' || data.data_base64.length !== 4 * Math.ceil((end-offset)/3)) throw invalid();
    let decoded; try { decoded = atob(data.data_base64); } catch { throw invalid(); }
    if (decoded.length !== end-offset || btoa(decoded) !== data.data_base64) throw invalid();
    for (let i = 0; i < decoded.length; i++) bytes[offset+i] = decoded.charCodeAt(i);
    offset = end;
  }
  stage('digest');
  if (!globalThis.crypto?.subtle) throw Object.assign(new Error('SHA-256 unavailable'), {code:'digest_unavailable'});
  const digest = [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))].map(b => b.toString(16).padStart(2,'0')).join('');
  if (!alive() || digest !== item.sha256) throw invalid();
  return new Blob([bytes], { type: 'image/png' });
}

// Show only fixed stage/code vocabulary, never Host error messages, paths or bytes.
export function visualFailure(stage, error) {
  const stages = ['list','metadata','image_tool','chunk_validation','digest','blob_url','png_decode','data_png','dimensions'];
  const codes = ['invalid_visual','digest_unavailable','visual_unavailable','response_too_large','busy','connection_closed'];
  return `이미지를 읽지 못했습니다. 보고서 본문은 보존되어 있습니다. [${stages.includes(stage) ? stage : 'unknown'}/${codes.includes(error?.code) ? error.code : 'failed'}]`;
}

export function mountVisuals(container, scope, initialOwner, call) {
  let disposed = false, epoch = 0;
  const releases = new Set();
  const node = (tag, text) => { const el = document.createElement(tag); if (text !== undefined) el.textContent = text; return el; };
  const section = node('section'); section.className = 'report-visuals';
  section.setAttribute('aria-label', '보고서 시각 자료');
  const title = node('h3','시각 자료'), status = node('p'), gallery = node('div');
  status.setAttribute('role','status');
  section.append(title, status, gallery); container.append(section);
  const release = () => { for (const cleanup of releases) cleanup(); releases.clear(); gallery.replaceChildren(); };
  const failure = (stage, error) => { status.textContent = visualFailure(stage, error); };
  const invoke = async (name, args) => {
    try { return await call(name,args); } catch (e) {
      if (!disposed && ['busy','connection_closed'].includes(e?.code)) return call(name,args);
      throw e;
    }
  };
  async function load(owner) {
    if (disposed) return;
    const serial = ++epoch; release(); status.textContent = '시각 자료 목록을 읽는 중…';
    const alive = () => !disposed && serial === epoch;
    try {
      const data = await invoke('gil_monitor_report_visuals',{owner});
      if (!alive()) return;
      check(data,scope,owner);
      const labels = {open_report:'아직 확정되지 않은 보고서입니다.',legacy_unspecified:'당시 시각 자료 선언이 기록되지 않았습니다.',not_needed:'시각 자료 없음',missing:'시각 자료 미완',attached:'확정된 시각 자료',references:'이 보고서에서 인용한 시각 자료'};
      if (!Object.hasOwn(labels,data.state) || !Array.isArray(data.uses) || data.uses.length > 8) throw invalid();
      if (['open_report','legacy_unspecified','not_needed'].includes(data.state) && data.uses.length) throw invalid();
      if (data.state === 'attached' && !data.uses.length) throw invalid();
      if (['not_needed','missing'].includes(data.state) && typeof data.reason !== 'string') throw invalid();
      if (data.state === 'missing' && typeof data.follow_up !== 'string') throw invalid();
      const summary = labels[data.state] + (typeof data.reason==='string' ? ` — ${data.reason}`:'') + (typeof data.follow_up==='string' ? ` · 후속 계획: ${data.follow_up}`:'');
      status.textContent = summary;
      async function choose(item) {
        if (!alive()) return;
        const frame = node('figure'), transport = node('p'), loading = node('p','이미지 읽는 중…');
        transport.setAttribute('aria-label','이미지 표시 진단');
        loading.setAttribute('role','status'); frame.append(loading); gallery.append(frame,transport);
        let image=null, url=null;
        const cleanup = () => { if(image) image.removeAttribute('src'); image=null; if(url) URL.revokeObjectURL(url); url=null; };
        releases.add(cleanup);
        const current = alive;
        let stage = 'metadata';
        transport.textContent='';
        try {
          for (const key of ['caption','alt','purpose','origin_step_ref','source_kind','source_description']) if (typeof item[key]!=='string') throw invalid();
          const blob = await readImage(invoke,scope,owner,item,current, value => { stage = value; });
          if (!current()) return;
          const img = node('img'); img.alt=item.alt;
          stage='blob_url'; url=URL.createObjectURL(blob); image=img; img.src=url;
          stage='png_decode';
          // Compare only local representations of the SAME verified PNG.
          // Never relax CSP or fetch from a network/filesystem URL.
          let blobBlocked = false;
          const violation = e => {
            if (e.effectiveDirective === 'img-src' && (e.blockedURI === 'blob' || e.blockedURI?.startsWith('blob:'))) blobBlocked=true;
          };
          document.addEventListener('securitypolicyviolation',violation);
          try {
            await img.decode();
            if (current()) transport.textContent='Blob PNG: 표시 성공';
          } catch {
            if (!current()) return;
            const blobResult=()=>blobBlocked ? 'CSP img-src 차단 확인' : 'decode 실패 (차단 여부 미확인)';
            URL.revokeObjectURL(url); url=null; img.removeAttribute('src');
            stage='data_png';
            const data = await new Promise((resolve,reject)=>{
              const reader=new FileReader(); reader.onload=()=>resolve(reader.result); reader.onerror=()=>reject(new Error('data PNG conversion failed')); reader.readAsDataURL(blob);
            });
            if (!current()) return;
            if (typeof data !== 'string' || !data.startsWith('data:image/png;base64,')) throw invalid();
            img.src=data;
            try { await img.decode(); }
            catch (error) { if(current()) transport.textContent=`Blob PNG: ${blobResult()} · data PNG: decode 실패`; throw error; }
            if(current()) transport.textContent=`Blob PNG: ${blobResult()} · data PNG: 표시 성공 (동일 검증 bytes)`;
          } finally { document.removeEventListener('securitypolicyviolation',violation); }
          if (!current()) return;
          stage='dimensions';
          if (img.naturalWidth !== item.width || img.naturalHeight !== item.height) throw invalid();
          const caption=node('figcaption',item.caption), purpose=node('p',`용도: ${item.purpose}`);
          const kinds={captured:'캡처',generated:'생성',provided:'제공'};
          const source=node('p',`출처: ${kinds[item.source_kind] || item.source_kind} (작성자 기재) · ${item.source_description} · 원래 Verify: ${item.origin_step_ref}`);
          const zoom = imageZoom(img);
          frame.replaceChildren(zoom.hint,zoom.viewport,caption,purpose,source);
        } catch (error) { if (current()) { cleanup(); loading.textContent=visualFailure(stage,error); frame.replaceChildren(loading); } }
      }
      // Keep report order and independent captions; decode serially, never select a different owner.
      for (const item of data.uses) { if (!alive()) break; await choose(item); }
    } catch (error) { if (alive()) failure('list',error); }
  }
  // Display only the selected report's references. Reuse belongs to storage,
  // not an implicit switch to a different report owner inside node details.
  void load(initialOwner);
  const dispose = () => { disposed=true; epoch++; release(); status.textContent='이미지 연결이 종료되었습니다. 보고서 본문은 유지합니다.'; };
  window.addEventListener('pagehide',dispose,{once:true});
  return () => { dispose(); window.removeEventListener('pagehide',dispose); };
}
