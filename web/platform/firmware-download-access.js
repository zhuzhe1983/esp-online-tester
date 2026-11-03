import {parseBurnerDownload} from './m5burner.js';

export const M5BURNER_VERIFICATION_MESSAGE = 'M5Burner 要求验证码或登录验证，当前未下载到固件。请到官方页面完成验证并下载，再通过“本地文件”导入刷写。';
export function burnerProjectUrl(url) {
  const ids=parseBurnerDownload(url);
  return ids?`https://burner.m5stack.com/firmware/${ids.firmwareId}`:null;
}

// Download endpoints can return JSON errors even with HTTP 200. Never pass
// those bytes to an image parser or cache; retain no request IDs or tokens.
export async function checkFirmwareDownloadResponse(response,url,{signal}={}) {
  const officialUrl=burnerProjectUrl(url);
  if(!officialUrl||!/\bjson\b/i.test(response.headers.get('content-type')||''))return;
  const details={url,officialUrl,provider:'m5burner',status:response.status};
  const fail=(code,message)=>Object.assign(new Error(message),{code,details});
  const limit=16*1024,reader=response.body?.getReader();
  let data;
  const aborted=()=>{if(signal?.aborted)throw fail('FETCH_ABORTED','固件下载已取消或超时。');};
  if(Number(response.headers.get('content-length'))>limit){await reader?.cancel();reader?.releaseLock();throw fail('FIRMWARE_DOWNLOAD_RESPONSE','M5Burner 返回了过大的 JSON 响应，并非固件文件。');}
  if(reader) {
    const cancel=()=>{void reader.cancel().catch(()=>{});};signal?.addEventListener('abort',cancel,{once:true});
    const chunks=[];let size=0;
    try {
      aborted();
      for(;;){const {done,value}=await reader.read();aborted();if(done)break;size+=value.byteLength;if(size>limit)throw fail('FIRMWARE_DOWNLOAD_RESPONSE','M5Burner 返回了过大的 JSON 响应，并非固件文件。');chunks.push(value);}
      data=new Uint8Array(size);let at=0;for(const chunk of chunks){data.set(chunk,at);at+=chunk.byteLength;}
    } finally {await reader.cancel().catch(()=>{});reader.releaseLock();signal?.removeEventListener('abort',cancel);}
  } else {data=new Uint8Array(await response.arrayBuffer());aborted();if(data.length>limit)throw fail('FIRMWARE_DOWNLOAD_RESPONSE','M5Burner 返回了过大的 JSON 响应，并非固件文件。');}
  let body;try{body=JSON.parse(new TextDecoder().decode(data));}catch{}
  const code=body?.data?.errorCode;
  if(typeof code==='string'&&/^[A-Z][A-Z_]{0,79}$/.test(code))details.providerCode=code;
  if(code==='VERSION_NOT_FOUND')throw fail('FIRMWARE_VERSION_NOT_FOUND','M5Burner 上的这个固件版本已不存在，发布者可能已重新发布或撤下。请刷新版本列表，确认后选择其他版本。');
  if(code==='BURN_CAPTCHA_INVALID'||/^(?:BURN_)?CAPTCHA_/.test(code||'')||[401,403].includes(response.status))throw fail('FIRMWARE_AUTH_REQUIRED',M5BURNER_VERIFICATION_MESSAGE);
  throw fail('FIRMWARE_DOWNLOAD_RESPONSE','M5Burner 返回了 JSON 响应，并非固件文件。请前往官方页面检查下载状态。');
}
