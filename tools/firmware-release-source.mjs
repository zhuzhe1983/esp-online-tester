// Build-time metadata adapters. Never request firmware payloads or retain authors.
export function releaseRequest(entry,token) {
  if(entry.metadataProvider==='m5burner') {
    if(entry.metadataUrl!=='https://m5burner-api.m5stack.com/api/firmware')throw Error('Unexpected M5Burner metadata URL');
    return {url:entry.metadataUrl,headers:{Accept:'application/json'},maxBytes:16*1024*1024};
  }
  if(entry.metadataUrl!==`https://api.github.com/repos/${entry.upstreamRepo}/releases/latest`)throw Error('Unexpected GitHub metadata URL');
  return {url:entry.metadataUrl,headers:{Accept:'application/vnd.github+json','X-GitHub-Api-Version':'2022-11-28',...(token?{Authorization:`Bearer ${token}`}:{})},maxBytes:2*1024*1024};
}
const safe=value=>{const url=new URL(value);if(url.protocol!=='https:'||url.username||url.password)throw Error('Unexpected release URL');return url.href;};
export function publicRelease(entry,data) {
  if(entry.metadataProvider==='m5burner') {
    if(!Array.isArray(data))throw Error('Invalid M5Burner catalog');
    const matches=data.filter(item=>item.fid===entry.firmwareId);if(matches.length!==1)throw Error('Firmware identity missing or ambiguous');
    const versions=(matches[0].versions||[]).filter(v=>v.published===true&&typeof v.version==='string'&&Number.isFinite(Date.parse(v.published_at))).sort((a,b)=>Date.parse(a.published_at)-Date.parse(b.published_at));
    const latest=versions.at(-1);if(!latest||!/^\w[\w.-]*\.bin$/.test(latest.file))throw Error('No valid published firmware version');
    return {tag_name:latest.version,html_url:safe(entry.releaseUrl),published_at:latest.published_at,assets:[{name:latest.file,browser_download_url:`https://m5burner-cdn.m5stack.com/firmware/${latest.file}`,size:null,digest:null}]};
  }
  return {tag_name:String(data.tag_name||''),html_url:safe(data.html_url||entry.releaseUrl),published_at:data.published_at||null,
    assets:(data.assets||[]).slice(0,400).map(asset=>({name:String(asset.name),browser_download_url:safe(asset.browser_download_url),size:asset.size,digest:/^sha256:[a-f0-9]{64}$/i.test(asset.digest)?asset.digest:null}))};
}
