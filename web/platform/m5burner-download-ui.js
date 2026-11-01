import {parseBurnerDownload} from './m5burner.js';
import {fetchBytes,sha256} from './runtime.js';
import {firmwareCache,MAX_CACHED_FIRMWARE_BYTES} from './firmware-cache.js';
import {requestBurnerVerification} from './m5burner-captcha.js';

// Run only from human load/flash actions. Agent runtime remains noninteractive.
// Keep the canonical URL in receipts, cache, scene and history; never a token URL.
export async function prepareBurnerSource(source,{signal,fetchImpl=globalThis.fetch,cache=firmwareCache,verify=requestBurnerVerification}={}) {
  if(!parseBurnerDownload(source?.url)||source.manifestUrl||source.file||source.files||source.archive)return source;
  const fail=(code,message)=>Object.assign(new Error(message),{code});
  const active=()=>{if(signal?.aborted)throw fail('FETCH_ABORTED','已取消固件下载。');};active();
  if(source.sha256&&!/^[a-f0-9]{64}$/i.test(source.sha256))throw fail('INVALID_HASH','SHA-256 格式不正确。');
  const cached=await cache.get(source.url,source.sha256);active();
  if(cached){if(await sha256(cached.data)===source.sha256.toLowerCase())return source;await cache.remove(source.url,source.sha256);}
  let data;
  const options={fetchImpl,signal,maxBytes:MAX_CACHED_FIRMWARE_BYTES};
  try{data=await fetchBytes(source.url,options);}
  catch(error){
    if(error.code!=='FIRMWARE_AUTH_REQUIRED')throw error;
    let token=await verify(source.url,{signal});active();
    if(typeof token!=='string'||!/^[A-Za-z0-9_-]{16,256}$/.test(token))throw fail('CAPTCHA_RESPONSE','验证码响应格式有变。');
    try{data=await fetchBytes(source.url,{...options,fetchImpl:(url,init)=>{if(url!==source.url)throw fail('INVALID_SOURCE','验证令牌只能用于当前固件。');return fetchImpl(`${url}?captchaToken=${encodeURIComponent(token)}`,init);}});}
    finally{token=null;}
  }
  active();const hash=await sha256(data);active();
  if(source.sha256&&hash!==source.sha256.toLowerCase())throw fail('HASH_MISMATCH','下载内容与当前版本的 SHA-256 不一致。');
  await cache.putVerified(source.url,hash,data);active();
  const saved=await cache.get(source.url,hash);active();
  if(!saved||await sha256(saved.data)!==hash)throw fail('CACHE_UNAVAILABLE','无法缓存已下载的固件，请重试。');
  return {...source,sha256:hash};
}
