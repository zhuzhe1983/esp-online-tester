import {firmwareCache,MAX_CACHED_FIRMWARE_BYTES} from './firmware-cache.js';
import {sha256} from './runtime.js';

const problem=(code,message)=>Object.assign(new Error(message),{code});
function pinnedSource(source,baseUrl) {
  if(!source?.url||source.manifestUrl||source.file||source.files||!/^[a-f0-9]{64}$/i.test(source.sha256||''))return null;
  try {
    const url=new URL(source.url,baseUrl);
    if(!['https:','http:'].includes(url.protocol)||url.username||url.password)return null;
    return {url:url.href,sha256:source.sha256.toLowerCase()};
  } catch {return null;}
}

// Only replace the exact failed, pinned remote BIN. Never recover a missing ROM,
// manifest, hash mismatch or HTTP 404 by putting a different file in its cache.
export function firmwareRecoverySource(error,source,baseUrl=globalThis.location?.href) {
  const pinned=pinnedSource(source,baseUrl);
  if(!['FETCH_FAILED','FIRMWARE_RELAY_FAILED'].includes(error?.code)||!pinned)return null;
  try {
    if(new URL(error.details?.url,baseUrl).href!==pinned.url||new URL(baseUrl).origin===new URL(pinned.url).origin)return null;
  } catch {return null;}
  return pinned;
}

export async function importFirmwareDownload(file,source,{cache=firmwareCache,signal}={}) {
  const pinned=pinnedSource(source);
  if(!pinned)throw problem('INVALID_SOURCE','恢复下载需要原始固件地址和固定的 SHA-256。');
  const active=()=>{if(signal?.aborted)throw problem('FETCH_ABORTED','已取消文件校验。');};
  active();
  if(!file||typeof file.arrayBuffer!=='function'||!Number.isSafeInteger(file.size)||file.size<=0||file.size>MAX_CACHED_FIRMWARE_BYTES)throw problem('INVALID_FILE','请选择非空、大小不超过 64 MiB 的原始 BIN 文件。');
  const data=new Uint8Array(await file.arrayBuffer());active();
  if(data.length!==file.size)throw problem('INVALID_FILE','文件读取不完整，请重新选择。');
  const actual=await sha256(data);active();
  if(actual!==pinned.sha256)throw problem('HASH_MISMATCH','文件与当前固件版本的 SHA-256 不一致。请下载下方链接对应的 BIN，不要选择 ZIP、网页或其他版本。');
  const persistent=await cache.putVerified(pinned.url,pinned.sha256,data);active();
  // Storage may be blocked or the cache cleared while the file was being read.
  const saved=await cache.get(pinned.url,pinned.sha256);active();
  if(!saved||await sha256(saved.data)!==pinned.sha256)throw problem('CACHE_UNAVAILABLE','无法保存已校验的固件，请重新选择文件。');
  active();return {bytes:data.length,persistent};
}

// Human UI only; automation continues to receive the original structured error.
// Browser navigation can download an upstream file even when fetch cannot read it.
export function recoverFirmwareDownload({error,source,firmware,issueUrl,baseUrl=globalThis.location?.href}) {
  const pinned=firmwareRecoverySource(error,source,baseUrl);
  if(!pinned)return Promise.resolve(false);
  const node=(tag,text,className)=>{const el=document.createElement(tag);if(text)el.textContent=text;if(className)el.className=className;return el;};
  const dialog=node('dialog',null,'firmware-run-confirm firmware-download-recovery');dialog.setAttribute('aria-labelledby','firmware-download-recovery-title');
  const title=node('h2','无法直接下载固件');title.id='firmware-download-recovery-title';
  const description=node('p','网络连接或上游跨域限制阻止了下载。可先从原地址下载文件，再选回此处；校验通过后继续加载当前版本。');
  const identity=node('p',firmware?.name||new URL(pinned.url).pathname.split('/').pop(),'helper');
  const link=node('a','1. 打开原始下载地址 ↗','button');link.href=pinned.url;link.target='_blank';link.rel='noopener noreferrer';
  const label=node('label','2. 选择下载好的 BIN 文件','firmware-recovery-file');
  const input=node('input');input.type='file';input.accept='.bin,application/octet-stream';label.append(input);
  const hash=node('p',`SHA-256：${pinned.sha256}`,'helper firmware-recovery-hash');
  const note=node('p','校验后的文件只缓存于当前浏览器。原地址、版本和校验值保持不变，后续仍会检查芯片、容量和镜像布局。','helper');
  const feedback=node('p',null,'form-error');feedback.setAttribute('role','status');feedback.setAttribute('aria-live','polite');
  const actions=node('div',null,'dialog-actions'),cancel=node('button','取消','button'),proceed=node('button','校验并继续','button primary');
  cancel.type=proceed.type='button';proceed.disabled=true;actions.append(cancel,proceed);
  if(issueUrl){const report=node('a','报告问题 ↗','button');report.href=issueUrl;report.target='_blank';report.rel='noopener noreferrer';actions.prepend(report);}
  dialog.append(title,identity,description,link,label,hash,note,feedback,actions);document.body.append(dialog);
  return new Promise(resolve=>{
    let accepted=false,checking=false;const controller=new AbortController();
    input.addEventListener('change',()=>{proceed.disabled=!input.files.length;feedback.textContent='';});
    cancel.addEventListener('click',()=>dialog.close());
    proceed.addEventListener('click',async()=>{
      if(checking)return;checking=true;input.disabled=proceed.disabled=true;feedback.textContent='正在核对文件 SHA-256…';
      try {await importFirmwareDownload(input.files[0],pinned,{signal:controller.signal});if(dialog.open){accepted=true;dialog.close();}}
      catch(e){if(dialog.open)feedback.textContent=e.message;}
      finally{checking=false;input.disabled=false;proceed.disabled=!input.files.length;}
    });
    dialog.addEventListener('close',()=>{controller.abort();dialog.remove();resolve(accepted);},{once:true});
    dialog.showModal();link.focus();
  });
}
