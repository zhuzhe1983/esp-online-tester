#!/usr/bin/env node
// Public metadata only. Firmware bodies and publisher account data never persist.
import {execFileSync,spawnSync} from 'node:child_process';
import {pathToFileURL,fileURLToPath} from 'node:url';
import {releaseRequest,publicReleases,releaseMatchesChannel} from './firmware-release-source.mjs';
import {validateWatchlist} from '../web/platform/watchlist.js';
import {enrichMeshtasticReleases} from './meshtastic-release-metadata.mjs';
import {m5burnerV1Request,validateM5BurnerV1Project} from './firmware-source-m5burner-v1.mjs';
const DAY=24*3600*1000;
export function githubToken(args=process.argv) {
  let token=process.env.GITHUB_TOKEN||process.env.GH_TOKEN;
  if(!token&&args.includes('--use-gh-auth'))token=execFileSync('gh',['auth','token'],{encoding:'utf8',stdio:['ignore','pipe','pipe']}).trim();return token;
}
const same=(a,b)=>JSON.stringify(a)===JSON.stringify(b);
export function compactReleaseHistory(entry,releases){
  const targets=entry.catalogTargets||[];
  if(!targets.length)return releases.filter(release=>releaseMatchesChannel(release,entry.releaseChannel)).slice(0,1);
  const patterns=targets.map(target=>new RegExp(target.assetNamePattern));
  return releases.map(release=>({...release,assetNames:release.assetNames||(release.assets||[]).map(asset=>asset.name),assets:(release.assets||[]).filter(asset=>patterns.some(pattern=>pattern.test(asset.name)))}));
}

export async function syncReleaseMetadata(list,{previous={entries:[]},token,fetchImpl=fetch,now=new Date().toISOString()}={}) {
  validateWatchlist(list);const cache=new Map(),entries=[];
  const metadata=async request=>{
    const response=await fetchImpl(request.url,{headers:request.headers,redirect:'error',signal:AbortSignal.timeout(30000)});
    if(!response.ok){await response.body?.cancel();throw Object.assign(Error([403,429].includes(response.status)?'发布源 API 查询受限；保留上次结果。':response.status===404?'发布源暂无公开 Release；保留已收录固件。':`发布元信息 HTTP ${response.status}`),{status:response.status});}
    if(!response.body)throw Error('Metadata response has no stream');
    const reader=response.body.getReader(),chunks=[];let length=0;
    try{while(true){const{done,value}=await reader.read();if(done)break;length+=value.byteLength;if(length>request.maxBytes)throw Error('Release metadata exceeds limit');chunks.push(value);}}
    finally{await reader.cancel();}return JSON.parse(Buffer.concat(chunks).toString('utf8'));
  };
  for(const entry of list.entries){
    const saved=previous.entries?.find(item=>item.id===entry.id&&item.upstreamRepo===entry.upstreamRepo);let result;
    try{
      const request=releaseRequest(entry,token);if(!cache.has(request.url))cache.set(request.url,metadata(request));
      let data=await cache.get(request.url);
      if(entry.metadataProvider==='m5burner-v1'){
        validateM5BurnerV1Project(entry,data);
        const versions=m5burnerV1Request(entry,true);if(!cache.has(versions.url))cache.set(versions.url,metadata(versions));
        data={project:data,versions:await cache.get(versions.url)};
      }
      const upstream=await enrichMeshtasticReleases(entry,publicReleases(entry,data),{fetchImpl});
      const releases=compactReleaseHistory(entry,upstream);const release=releases.find(value=>releaseMatchesChannel(value,entry.releaseChannel));
      if(!release)throw Error('No published release for configured channel');
      result={id:entry.id,upstreamRepo:entry.upstreamRepo,checkedAt:now,sourceUrl:request.url,release,releases,error:null};
      if(saved&&!saved.error&&same(saved.releases||[saved.release],releases)&&same(saved.release,release)&&Date.parse(now)-Date.parse(saved.checkedAt)<DAY)result={...result,checkedAt:saved.checkedAt};
    }catch(error){
      result={id:entry.id,upstreamRepo:entry.upstreamRepo,checkedAt:saved?.checkedAt||null,sourceUrl:entry.metadataUrl,release:saved?.release||null,releases:saved?.releases||[],error:{status:error.status||null,message:error.status?error.message:'发布元信息同步失败；保留上次结果。',attemptedAt:now}};
      if(saved?.error&&saved.error.status===result.error.status&&saved.error.message===result.error.message&&Date.parse(now)-Date.parse(saved.error.attemptedAt)<DAY)result.error=saved.error;
    }
    entries.push(result);
  }
  const generatedAt=same(entries,previous.entries)&&previous.generatedAt?previous.generatedAt:now;
  return {schemaVersion:'1',generatedAt,policy:'Public release metadata only; firmware binaries stay at upstream sources. No client GitHub API token.',entries};
}
// Keep the former command as an alias: every CLI/Action updates both the
// snapshot and device definitions through the same policy, never one alone.
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href){
  const child=spawnSync(process.execPath,[fileURLToPath(new URL('./sync-firmware-catalog.mjs',import.meta.url)),...process.argv.slice(2)],{stdio:'inherit'});
  process.exitCode=child.status??1;
}
