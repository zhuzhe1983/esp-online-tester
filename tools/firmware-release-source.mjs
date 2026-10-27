// Build-time metadata adapters. Never request firmware payloads or retain authors.
import {cloudzaoMetadataUrl, cloudzaoReleases} from './firmware-source-cloudzao.mjs';
const safe=value=>{const url=new URL(value);if(url.protocol!=='https:'||url.username||url.password)throw Error('Unexpected release URL');return url.href;};
const digest=value=>/^sha256:[a-f0-9]{64}$/i.test(value||'')?value.toLowerCase():null;
export const releaseIsPrerelease=release=>release.prerelease===true||/(?:^|[.\d-])(?:rc|pre|alpha|beta|nightly)(?:[.\d-]|$)/i.test(release.tag_name||'');
export function releaseMatchesChannel(release,channel='stable') {
  if(release.draft)return false;
  if(channel==='rc')return /(?:^|[.\d-])rc(?:[.\d-]|$)/i.test(release.tag_name||'');
  return channel==='prerelease'?releaseIsPrerelease(release):!releaseIsPrerelease(release);
}
export function releaseRequest(entry,token) {
  if(entry.metadataProvider==='cloudzao') return {url:cloudzaoMetadataUrl(entry),headers:{Accept:'application/json'},maxBytes:2*1024*1024};
  if(entry.metadataProvider==='m5burner') {
    if(entry.metadataUrl!=='https://m5burner-api.m5stack.com/api/firmware')throw Error('Unexpected M5Burner metadata URL');
    return {url:entry.metadataUrl,headers:{Accept:'application/json'},maxBytes:16*1024*1024};
  }
  if(entry.metadataUrl!==`https://api.github.com/repos/${entry.upstreamRepo}/releases/latest`)throw Error('Unexpected GitHub metadata URL');
  const limit=entry.releaseHistoryLimit;
  if(limit!==undefined&&(!Number.isInteger(limit)||limit<1||limit>30))throw Error('Invalid release history limit');
  return {url:limit?`https://api.github.com/repos/${entry.upstreamRepo}/releases?per_page=${limit}`:entry.metadataUrl,headers:{Accept:'application/vnd.github+json','X-GitHub-Api-Version':'2022-11-28',...(token?{Authorization:`Bearer ${token}`}:{})},maxBytes:32*1024*1024};
}
export function publicReleases(entry,data) {
  if(entry.metadataProvider==='cloudzao') return cloudzaoReleases(entry,data);
  if(entry.metadataProvider==='m5burner') {
    if(!Array.isArray(data))throw Error('Invalid M5Burner catalog');
    const matches=data.filter(item=>item.fid===entry.firmwareId);if(matches.length!==1)throw Error('Firmware identity missing or ambiguous');
    return (matches[0].versions||[]).filter(v=>v.published===true&&typeof v.version==='string'&&typeof v.published_at==='string'&&Number.isFinite(Date.parse(v.published_at.replaceAll('.','-')))).map(version=>{
      if(!/^\w[\w.-]*\.bin$/.test(version.file))throw Error('Invalid published firmware filename');
      const release={tag_name:version.version,html_url:safe(entry.releaseUrl),published_at:version.published_at.replaceAll('.','-'),prerelease:false,draft:false,
        assets:[{name:version.file,browser_download_url:`https://m5burner-cdn.m5stack.com/firmware/${version.file}`,size:Number.isSafeInteger(version.size)?version.size:null,digest:digest(version.digest)||digest(`sha256:${version.sha256||''}`)}]};
      release.prerelease=releaseIsPrerelease(release);return release;
    }).sort((a,b)=>Date.parse(b.published_at)-Date.parse(a.published_at));
  }
  return (Array.isArray(data)?data:[data]).filter(item=>!item.draft).slice(0,entry.releaseHistoryLimit||30).map(item=>({tag_name:String(item.tag_name||''),html_url:safe(item.html_url||entry.releaseUrl),published_at:item.published_at||null,prerelease:item.prerelease===true,draft:false,
    assets:(item.assets||[]).slice(0,400).map(asset=>({name:String(asset.name),browser_download_url:safe(asset.browser_download_url),size:asset.size,digest:digest(asset.digest)}))})).sort((a,b)=>Date.parse(b.published_at||0)-Date.parse(a.published_at||0));
}
export function publicRelease(entry,data) {
  const latest=publicReleases(entry,data).find(release=>releaseMatchesChannel(release,entry.releaseChannel));
  if(!latest)throw Error('No valid published firmware version for this channel');return latest;
}
