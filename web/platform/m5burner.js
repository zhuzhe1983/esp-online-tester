// Public API used by burner.m5stack.com app 131; see the dated CORS audit.
export const M5BURNER_API = 'https://burner.m5stack.com/api/v1';
const label = value => typeof value === 'string' ? value.trim().slice(0,180) : '';
const id = value => typeof value === 'string' && /^\d{1,30}$/.test(value) ? value : null;
const key = value => label(value).normalize('NFKC').toLowerCase().replace(/[^a-z0-9]/g,'');
export const isM5Stack = product => product?.manufacturer?.toLowerCase() === 'm5stack';
function docsKey(value) {
  try { const url=new URL(value); return url.hostname==='docs.m5stack.com' ? decodeURIComponent(url.pathname).toLowerCase().replace(/^\/(en|zh_cn|zh|ja)\//,'/').replace(/\/$/,'') : ''; } catch { return ''; }
}
export function matchBurnerDevice(product, devices) {
  if (!isM5Stack(product)) return null;
  const urls=new Set((product.sources||[]).flatMap(source=>[docsKey(source.url),docsKey(source.resolvedUrl)]).filter(Boolean));
  const byUrl=devices.filter(device=>urls.has(docsKey(device.productUrl)));
  if(byUrl.length===1)return byUrl[0];
  const names=new Set([product.name?.replace(/^M5Stack\s+/i,'').split(' · ')[0],...(product.aliases||[])].map(key).filter(Boolean));
  const byName=devices.filter(device=>names.has(key(device.deviceName)));
  return byName.length===1 ? byName[0] : null;
}
export function burnerDownloadUrl(firmwareId,versionId) {
  if(!id(firmwareId)||!id(versionId))throw Error('M5Burner 固件或版本 ID 无效。');
  return `${M5BURNER_API}/firmwares/${firmwareId}/versions/${versionId}/download`;
}
export function parseBurnerDownload(value) {
  try { const url=new URL(value);if(url.origin!=='https://burner.m5stack.com'||url.username||url.password||url.search||url.hash)return null;const match=/^\/api\/v1\/firmwares\/(\d{1,30})\/versions\/(\d{1,30})\/download$/.exec(url.pathname);return match?{firmwareId:match[1],versionId:match[2]}:null;} catch {return null;}
}
const supports = (item,deviceId) => Array.isArray(item.supportedDevices)&&item.supportedDevices.some(device=>device.deviceId===deviceId);
export function burnerFirmware(project,version,deviceId) {
  return {id:`m5burner:${project.firmwareId}:${version.versionId}`,name:project.firmwareName,revision:version.versionName,version:version.versionName,
    simulationVerified:false,source:{url:burnerDownloadUrl(project.firmwareId,version.versionId)},provider:'m5burner',deviceId,
    firmwareId:project.firmwareId,versionId:version.versionId,publisherGroup:project.sourceType==='OFFICIAL'?'官方':'社区',
    detailsUrl:`https://burner.m5stack.com/firmware/${project.firmwareId}`};
}
export function burnerHistoryFirmware(entry) {
  const ids=parseBurnerDownload(entry?.url);if(entry?.provider!=='m5burner'||!ids)return null;
  return {...burnerFirmware({firmwareId:ids.firmwareId,firmwareName:entry.name||'M5Burner 固件'}, {versionId:ids.versionId,versionName:entry.version||'版本未知'},entry.deviceId),source:{url:entry.url,...(entry.sha256?{sha256:entry.sha256}:{}),...(['rom','app-direct'].includes(entry.bootMode)?{bootMode:entry.bootMode}:{})}};
}
export function createM5BurnerClient(fetchImpl=globalThis.fetch) {
  let devicesCache=null;
  async function request(path,signal) {
    const timeout=new AbortController(),timer=setTimeout(()=>timeout.abort(),20000),abort=()=>timeout.abort();
    signal?.addEventListener('abort',abort,{once:true});if(signal?.aborted)timeout.abort();
    try {
      let response;
      try {response=await fetchImpl(M5BURNER_API+path,{mode:'cors',credentials:'omit',headers:{Accept:'application/json'},signal:timeout.signal});}
      catch(e){if(signal?.aborted)throw e;throw Error(timeout.signal.aborted?'M5Burner 请求超时，请重试。':'暂时无法连接 M5Burner，请检查网络或稍后重试。');}
      if(!response.ok)throw Error(response.status===429?'M5Burner 请求过于频繁，请稍后重试。':`M5Burner 返回 HTTP ${response.status}。`);
      // Metadata only. Enforce the bound while streaming, including chunked replies.
      const reader=response.body.getReader(),chunks=[];let size=0;
      for(;;){const {done,value}=await reader.read();if(done)break;size+=value.length;if(size>2*1024*1024){await reader.cancel();throw Error('M5Burner 元信息过大。');}chunks.push(value);}
      const bytes=new Uint8Array(size);let offset=0;for(const chunk of chunks){bytes.set(chunk,offset);offset+=chunk.length;}
      let data;try{data=JSON.parse(new TextDecoder().decode(bytes));}catch{throw Error('M5Burner 返回了无效的 JSON，请稍后重试。');}
      if(!data||data.code!==200)throw Error(label(data?.msg)||'M5Burner 未返回有效结果。');return data;
    } finally {clearTimeout(timer);signal?.removeEventListener('abort',abort);}
  }
  return {
    async devices(signal) {
      if(devicesCache)return devicesCache;
      const result=await request('/devices',signal);if(!Array.isArray(result.data))throw Error('M5Burner 型号列表格式有变。');
      devicesCache=result.data.filter(item=>id(item.deviceId)&&label(item.deviceName)).map(item=>({deviceId:item.deviceId,deviceName:label(item.deviceName),deviceSlug:label(item.deviceSlug),productUrl:label(item.productUrl)}));return devicesCache;
    },
    async search(deviceId,query='',page=1,signal) {
      if(!id(deviceId)||!Number.isSafeInteger(page)||page<1)throw Error('M5Burner 查询参数无效。');
      const params=new URLSearchParams({deviceId,pageNum:String(page),pageSize:'50',sort:'LATEST'});if(query.trim())params.set('keyword',query.trim().slice(0,120));
      const data=await request(`/firmwares?${params}`,signal);if(!Array.isArray(data.rows)||!Number.isSafeInteger(data.total)||data.total<0)throw Error('M5Burner 搜索结果格式有变。');
      return {total:data.total,page,received:data.rows.length,rows:data.rows.filter(item=>id(item.firmwareId)&&label(item.firmwareName)&&supports(item,deviceId)).map(item=>({firmwareId:item.firmwareId,firmwareName:label(item.firmwareName),sourceType:item.sourceType,currentVersionId:id(item.currentVersion?.versionId)}))};
    },
    async versions(firmwareId,deviceId,signal) {
      if(!id(firmwareId)||!id(deviceId))throw Error('M5Burner 查询参数无效。');
      const data=await request(`/firmwares/${firmwareId}/versions?scope=PUBLIC`,signal);if(!Array.isArray(data.data))throw Error('M5Burner 版本列表格式有变。');
      return data.data.filter(item=>id(item.versionId)&&label(item.versionName)&&item.visibility==='PUBLIC'&&item.status==='PUBLISHED'&&supports(item,deviceId)).map(item=>({versionId:item.versionId,versionName:label(item.versionName),publishTime:label(item.publishTime),current:item.current===true}));
    },
  };
}
