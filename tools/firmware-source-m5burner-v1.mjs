// Public metadata only. Device categories are not product identities: each
// watch entry pins a reviewed project/device pair and, where needed, edition.
const BASE='https://burner.m5stack.com/api/v1/firmwares/';
const id=value=>typeof value==='string'&&/^[1-9][0-9]{0,29}$/.test(value);
const supports=(value,deviceId)=>Array.isArray(value)&&value.some(device=>device?.deviceId===deviceId);
export function m5burnerV1Request(entry,versions=false){
  if(!id(entry.firmwareId)||!id(entry.deviceId)||entry.metadataUrl!==BASE+entry.firmwareId)throw Error('Invalid M5Burner v1 identity');
  return {url:entry.metadataUrl+(versions?'/versions?scope=PUBLIC':''),headers:{Accept:'application/json'},maxBytes:2*1024*1024};
}
export function validateM5BurnerV1Project(entry,response){
  m5burnerV1Request(entry);
  const project=response?.data;
  if(response?.code!==200||project?.firmwareId!==entry.firmwareId||project.sourceType!=='OFFICIAL'||!supports(project.supportedDevices,entry.deviceId))throw Error('Official M5Burner project/device identity changed');
}
export function m5burnerV1Releases(entry,{project,versions}={}){
  validateM5BurnerV1Project(entry,project);
  if(versions?.code!==200||!Array.isArray(versions.data)||versions.data.length>1000)throw Error('Invalid M5Burner versions');
  const pattern=entry.versionNamePattern?new RegExp(entry.versionNamePattern):null,seen=new Set();
  return versions.data.filter(v=>v.visibility==='PUBLIC'&&v.status==='PUBLISHED'&&supports(v.supportedDevices,entry.deviceId)).map(v=>{
    if(!id(v.versionId)||seen.has(v.versionId)||typeof v.versionName!=='string'||v.versionName.length>160||!Number.isFinite(Date.parse(v.publishTime)))throw Error('Invalid or duplicate M5Burner version identity');
    seen.add(v.versionId);
    if(typeof v.binFileName!=='string'||v.binFileName.length>240||!/^[-\w.]+$/.test(v.binFileName)||v.binFileName==='.'||v.binFileName==='..')throw Error('Invalid M5Burner firmware filename');
    const digest=/^[a-f0-9]{64}$/i.test(v.sha256||'')?`sha256:${v.sha256.toLowerCase()}`:/^sha256:[a-f0-9]{64}$/i.test(v.digest||'')?v.digest.toLowerCase():null;
    return {tag_name:v.versionName,html_url:entry.releaseUrl,published_at:new Date(v.publishTime).toISOString().replace('.000Z','Z'),prerelease:false,draft:false,
      assets:[{name:v.binFileName,browser_download_url:BASE+entry.firmwareId+'/versions/'+v.versionId+'/download',size:null,digest,
        publisherIdentity:{provider:'m5burner-v1',sourceType:'OFFICIAL',firmwareId:entry.firmwareId,deviceId:entry.deviceId,versionId:v.versionId,visibility:'PUBLIC',status:'PUBLISHED'}}]};
  }).filter(release=>!pattern||pattern.test(release.tag_name)).sort((a,b)=>Date.parse(b.published_at)-Date.parse(a.published_at)).slice(0,entry.releaseHistoryLimit||30);
}
// This exception is intentionally not available to GitHub, legacy Burner,
// community projects, mutable URLs, or a manually substituted asset URL.
export function isOfficialM5BurnerVersion(entry,asset){
  const p=asset?.publisherIdentity;
  return entry.metadataProvider==='m5burner-v1'&&p?.provider==='m5burner-v1'&&p.sourceType==='OFFICIAL'&&p.visibility==='PUBLIC'&&p.status==='PUBLISHED'&&p.firmwareId===entry.firmwareId&&p.deviceId===entry.deviceId&&id(p.versionId)&&asset.browser_download_url===BASE+entry.firmwareId+'/versions/'+p.versionId+'/download';
}
