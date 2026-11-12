// App-only visual transport and lifecycle. No filesystem URLs or model messages.
import {imageZoom} from './image-zoom.mjs';
import { t } from '../ui/locale.js';
const CHUNK = 256 * 1024, MAX = 16 * 1024 * 1024;
const sameOwner = (a, b) => a?.kind === b.kind && a?.ref === b.ref;
const invalid = () => Object.assign(new Error(t('visual.invalid')), { code: 'invalid_visual' });
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
  return t('visual.failure', { stage: stages.includes(stage) ? stage : 'unknown', code: codes.includes(error?.code) ? error.code : 'failed' });
}

// One sentence per recorded state. Only the recorded declaration speaks; nothing is inferred.
export function visualSummary(data) {
  const n = Array.isArray(data?.uses) ? data.uses.length : 0;
  switch (data?.state) {
    case 'attached': return { title: t('visual.attached_title', { n }), status: t('visual.attached_status'), tone: 'attached' };
    case 'references': return { title: t('visual.references_title', { n }), status: t('visual.references_status'), tone: 'attached' };
    case 'not_needed': return { title: t('visual.none_title'), status: t('visual.reason', { reason: data.reason }), tone: 'none' };
    case 'missing': return { title: n ? t('visual.missing_title_n', { n }) : t('visual.missing_title'), status: t('visual.missing_status', { reason: data.reason, follow_up: data.follow_up }), tone: 'missing' };
    case 'open_report': return { title: t('visual.title'), status: t('visual.open_report'), tone: 'pending' };
    case 'legacy_unspecified': return { title: t('visual.title'), status: t('visual.legacy'), tone: 'pending' };
    default: return { title: t('visual.title'), status: '', tone: 'pending' };
  }
}

export function mountVisuals(container, scope, initialOwner, call) {
  let disposed = false, epoch = 0;
  const releases = new Set();
  const node = (tag, text) => { const el = document.createElement(tag); if (text !== undefined) el.textContent = text; return el; };
  const section = node('section'); section.className = 'report-visuals';
  section.setAttribute('aria-label', t('visual.section_aria'));
  const title = node('h3', t('visual.title')), status = node('p'), gallery = node('div');
  gallery.className = 'visual-gallery';
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
    const serial = ++epoch; release(); status.textContent = t('visual.loading_list');
    const alive = () => !disposed && serial === epoch;
    try {
      const data = await invoke('gil_monitor_report_visuals',{owner});
      if (!alive()) return;
      check(data,scope,owner);
      const labels = {open_report:1,legacy_unspecified:1,not_needed:1,missing:1,attached:1,references:1};
      if (!Object.hasOwn(labels,data.state) || !Array.isArray(data.uses) || data.uses.length > 8) throw invalid();
      if (['open_report','legacy_unspecified','not_needed'].includes(data.state) && data.uses.length) throw invalid();
      if (data.state === 'attached' && !data.uses.length) throw invalid();
      if (['not_needed','missing'].includes(data.state) && typeof data.reason !== 'string') throw invalid();
      if (data.state === 'missing' && typeof data.follow_up !== 'string') throw invalid();
      const summary = visualSummary(data);
      title.textContent = summary.title;
      status.textContent = summary.status;
      section.dataset.state = data.state;
      section.dataset.tone = summary.tone;
      async function choose(item) {
        if (!alive()) return;
        const frame = node('figure'), transport = node('p'), loading = node('p', t('visual.loading_image'));
        transport.setAttribute('aria-label', t('visual.transport_aria'));
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
            if (current()) transport.textContent = t('visual.blob_ok');
          } catch {
            if (!current()) return;
            const blobResult=()=>blobBlocked ? t('visual.blob_blocked') : t('visual.blob_decode_failed');
            URL.revokeObjectURL(url); url=null; img.removeAttribute('src');
            stage='data_png';
            const data = await new Promise((resolve,reject)=>{
              const reader=new FileReader(); reader.onload=()=>resolve(reader.result); reader.onerror=()=>reject(new Error('data PNG conversion failed')); reader.readAsDataURL(blob);
            });
            if (!current()) return;
            if (typeof data !== 'string' || !data.startsWith('data:image/png;base64,')) throw invalid();
            img.src=data;
            try { await img.decode(); }
            catch (error) { if(current()) transport.textContent = t('visual.data_failed', { blob: blobResult() }); throw error; }
            if(current()) transport.textContent = t('visual.data_ok', { blob: blobResult() });
          } finally { document.removeEventListener('securitypolicyviolation',violation); }
          if (!current()) return;
          stage='dimensions';
          if (img.naturalWidth !== item.width || img.naturalHeight !== item.height) throw invalid();
          const caption=node('figcaption',item.caption), purpose=node('p', t('visual.purpose', { purpose: item.purpose }));
          const kindKey = `visual.kind.${item.source_kind}`, kindText = t(kindKey);
          const source=node('p', t('visual.source', { kind: kindText === kindKey ? item.source_kind : kindText, description: item.source_description, origin: item.origin_step_ref }));
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
  const dispose = () => { disposed=true; epoch++; release(); status.textContent = t('visual.disposed'); };
  window.addEventListener('pagehide',dispose,{once:true});
  return () => { dispose(); window.removeEventListener('pagehide',dispose); };
}
