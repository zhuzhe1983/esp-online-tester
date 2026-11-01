// Resolve public redirects before Pages deployment. Never fetch BIN bodies,
// send credentials, persist signed URLs, or replace an existing pinned CDN.
import {parseBurnerDownload} from '../web/platform/m5burner.js';
import {burnerProjectUrl,checkFirmwareDownloadResponse} from '../web/platform/firmware-download-access.js';

export function stableBurnerCdnUrl(value) {
  try {
    const url=new URL(value);
    return url.origin==='https://m5burner-cdn.m5stack.com'&&!url.username&&!url.password&&!url.search&&!url.hash&&/^\/(?:firmware|community|official)\/[A-Za-z0-9/_.-]+\.bin$/.test(url.pathname)?url.href:null;
  }catch{return null;}
}
export async function resolveBurnerDownload(sourceUrl,{fetchImpl=fetch,now=new Date().toISOString()}={}) {
  if(!parseBurnerDownload(sourceUrl))throw Error('Expected a fixed-version M5Burner download endpoint');
  const result={sourceUrl,officialUrl:burnerProjectUrl(sourceUrl),checkedAt:now,status:'unresolved'};
  const controller=new AbortController(),timer=setTimeout(()=>controller.abort(),15000);
  const options={redirect:'manual',credentials:'omit',cache:'no-store',signal:controller.signal};
  let response;
  try {
    response=await fetchImpl(sourceUrl,options);
    await checkFirmwareDownloadResponse(response,sourceUrl,{signal:controller.signal});
    if(![301,302,303,307,308].includes(response.status)){result.status=response.ok?'no-public-redirect':'http-error';result.httpStatus=response.status;return result;}
    const directUrl=stableBurnerCdnUrl(response.headers.get('location'));
    if(!directUrl){result.status='unstable-or-untrusted-redirect';return result;}
    await response.body?.cancel();response=null;
    response=await fetchImpl(directUrl,{...options,method:'HEAD'});
    if(response.status!==200||/\b(?:json|html)\b/i.test(response.headers.get('content-type')||'')){result.status='cdn-unavailable';result.httpStatus=response.status;return result;}
    result.status='resolved';result.directUrl=directUrl;
    return result;
  } catch(error) {
    result.status=error.code==='FIRMWARE_AUTH_REQUIRED'?'verification-required':error.code==='FIRMWARE_DOWNLOAD_RESPONSE'?'invalid-download-response':'check-failed';
    if(error.details?.providerCode)result.providerCode=error.details.providerCode;
    if(error.details?.status)result.httpStatus=error.details.status;
    return result;
  } finally {clearTimeout(timer);if(response?.body&&!response.body.locked)await response.body.cancel().catch(()=>{});}
}

export async function resolveCatalogDownloads(result,{fetchImpl=fetch,now=new Date().toISOString()}={}) {
  const entries=[],requests=new Map();
  const current=firmware=>{
    const old=firmware.downloadAccess,age=Date.parse(now)-Date.parse(old?.checkedAt);
    return old?.sourceUrl===firmware.source?.url&&age>=0&&age<24*3600*1000;
  };
  const pending=[...new Set(result.firmwares.items.filter(f=>parseBurnerDownload(f.source?.url)&&!current(f)).map(f=>f.source.url))];
  let next=0;
  await Promise.all(Array.from({length:Math.min(4,pending.length)},async()=>{
    while(next<pending.length){const url=pending[next++];requests.set(url,await resolveBurnerDownload(url,{fetchImpl,now}));}
  }));
  for(const firmware of result.firmwares.items) {
    const url=firmware.source?.url;
    if(!parseBurnerDownload(url)){
      const old=firmware.downloadAccess;
      if(old?.status==='resolved'&&old.directUrl===url&&parseBurnerDownload(old.sourceUrl))entries.push({firmwareId:firmware.id,...old});
      continue;
    }
    // Repeat predeploy checks on the same day should not create bot commits.
    const access=current(firmware)?firmware.downloadAccess:requests.get(url);firmware.downloadAccess=structuredClone(access);
    entries.push({firmwareId:firmware.id,...access});
    if(access.status!=='resolved')continue;
    firmware.source={...firmware.source,url:access.directUrl};
    for(const watch of result.watchlist.entries)for(const asset of watch.assets||[])if(asset.url===url){asset.upstreamUrl=url;asset.url=access.directUrl;}
  }
  result.report.downloads={policy:'Public M5Burner redirect plus CDN HEAD only; no credentials, firmware bodies or execution. Existing direct URLs and hashes are retained.',entries};
  result.report.summary.downloadsResolved=entries.filter(e=>e.status==='resolved').length;
  result.report.summary.downloadsNeedVerification=entries.filter(e=>e.status==='verification-required').length;
  result.report.summary.downloadChecksFailed=entries.filter(e=>!['resolved','verification-required'].includes(e.status)).length;
  return result;
}
