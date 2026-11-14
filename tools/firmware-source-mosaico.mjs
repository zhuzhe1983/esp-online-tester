// Official recovery metadata only; no BIN requests or publisher/account fields.
// Schema observed at https://mosaico.ai/api/v1/firmware on 2026-11-14.
export const MOSAICO_METADATA_URL='https://mosaico.ai/api/v1/firmware';
export function mosaicoRequest(entry) {
  if(entry.metadataUrl!==MOSAICO_METADATA_URL)throw Error('Unexpected Mosaico metadata URL');
  return {url:MOSAICO_METADATA_URL,headers:{Accept:'application/json'},maxBytes:64*1024};
}
export function mosaicoReleases(entry,data) {
  mosaicoRequest(entry);
  if(!data||data.target!=='ESP32-S31'||!Number.isSafeInteger(data.id)||data.id<1
    ||typeof data.title!=='string'||!data.title.startsWith('Mosaico ')||data.title.length>200
    ||!/^\d{4}-\d{2}-\d{2}$/.test(data.build_date)||!Number.isFinite(Date.parse(data.build_date))
    ||typeof data.file_name!=='string'||!/^hello-mosaico-complete_[\w.-]+\.bin$/.test(data.file_name)
    ||!Number.isSafeInteger(data.file_size)||data.file_size<1||data.file_size>128*1024*1024
    ||data.file_url!==`${MOSAICO_METADATA_URL}/${data.id}/download`)throw Error('Invalid Mosaico recovery metadata');
  return [{tag_name:data.title,html_url:'https://mosaico.ai/en/firmware',published_at:data.build_date,prerelease:false,draft:false,
    assets:[{name:data.file_name,browser_download_url:data.file_url,size:data.file_size,digest:null}]}];
}
