// Public CloudZao burner metadata only. No firmware bodies, credentials or account fields.
export function cloudzaoMetadataUrl(entry) {
  if(!/^[1-9][0-9]{0,8}$/.test(entry.hardwareModelId||'')||!/^\w{28,64}$/.test(entry.firmwareId||'')||!['esp32s3','esp32s31'].includes(entry.targetChip))throw Error('Invalid CloudZao identity');
  const expected=`https://admin.metalio.world/xiaozhi/api/firmwares?page=1&limit=1&hardwareModelId=${entry.hardwareModelId}&sourceType=official`;
  if(entry.metadataUrl!==expected)throw Error('Unexpected CloudZao metadata URL');return expected;
}
export function cloudzaoReleases(entry,data) {
  cloudzaoMetadataUrl(entry);
  if(data?.code!==0||data.data?.total!==1||!Array.isArray(data.data.list)||data.data.list.length!==1)throw Error('CloudZao firmware identity missing or ambiguous');
  const item=data.data.list[0];
  if(item.id!==entry.firmwareId||String(item.hardwareModelId)!==entry.hardwareModelId||item.chipName!==entry.targetChip||item.sourceType!=='official'||item.status!==1)throw Error('CloudZao hardware, chip or official publisher mismatch');
  if(!Array.isArray(item.versions)||item.versions.length>300)throw Error('Invalid CloudZao versions');
  const seen=new Set(),page=new URL(entry.releaseUrl);if(page.origin!=='https://burner.cloudzao.com'||page.username||page.password)throw Error('Invalid CloudZao release page');
  return item.versions.filter(v=>v.status===1&&v.deleteStatus===1&&['latest','test'].includes(v.versionStatus)).map(v=>{
    if(!/^[0-9][0-9A-Za-z.-]{0,63}$/.test(v.version)||seen.has(v.version))throw Error('Invalid or duplicate CloudZao version');seen.add(v.version);
    const url=new URL(v.firmwareUrl),size=Number(v.size);
    if(url.origin!=='https://cloudzaoai.oss-rg-china-mainland.aliyuncs.com'||!/^\/OTA\/xiaozhi\/[a-f0-9-]+\.bin$/.test(url.pathname)||url.username||url.password||url.search||url.hash||v.fileSuffix!=='bin'||!Number.isSafeInteger(size)||size<1||size>256*1024*1024||!Number.isFinite(Date.parse(v.createDate)))throw Error('Invalid CloudZao artifact');
    if(v.flashStartAddress!==null&&v.flashStartAddress!==undefined&&Number(v.flashStartAddress)!==0)throw Error('Unexpected CloudZao flash address');
    // The API marks numeric versions as "test"; never infer stability from semver alone.
    // Published data currently has no SHA-256. Do not promote MD5 or invent a digest.
    return {tag_name:v.version,html_url:page.href,published_at:new Date(v.createDate).toISOString(),prerelease:v.versionStatus==='test',draft:false,
      assets:[{name:url.pathname.split('/').at(-1),browser_download_url:url.href,size,digest:null}]};
  }).sort((a,b)=>Date.parse(b.published_at)-Date.parse(a.published_at));
}
