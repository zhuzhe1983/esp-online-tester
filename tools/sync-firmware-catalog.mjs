#!/usr/bin/env node
// Device-specific release selection; metadata only, never fetch a firmware body.
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve,dirname} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {validateWatchlist} from '../web/platform/watchlist.js';
import {releaseMatchesChannel} from './firmware-release-source.mjs';
import {syncReleaseMetadata,githubToken,compactReleaseHistory} from './sync-firmware-release-metadata.mjs';
import {createHash} from 'node:crypto';
const copy=value=>structuredClone(value), equal=(a,b)=>JSON.stringify(a)===JSON.stringify(b);
const norm=value=>String(value||'').replace(/^v/i,'').toLowerCase();
const sha=value=>/^sha256:[a-f0-9]{64}$/i.test(value||'')?value.slice(7).toLowerCase():null;
const encoded=value=>JSON.stringify(value,null,2)+'\n';
const hash=value=>createHash('sha256').update(value).digest('hex');
function older(candidate,current){
  const a=norm(candidate).match(/^(\d+)\.(\d+)(?:\.(\d+))?/),b=norm(current).match(/^(\d+)\.(\d+)(?:\.(\d+))?/);
  if(!a||!b)return false;
  for(let i=1;i<=3;i++){const difference=Number(a[i]||0)-Number(b[i]||0);if(difference)return difference<0;}return false;
}
function localManifest(target,firmware,manifests){
  if(target.manifestUrl!==firmware.source?.manifestUrl)throw Error('manifest-target-mismatch');
  const manifest=copy(manifests[target.manifestUrl]);
  if(!manifest||manifest.app_direct!==true||!manifest.files?.app||manifest.flash_at||manifest.files.flash)throw Error('manifest-layout-not-app-direct');
  return manifest;
}
function artifactSource(firmware,target,manifests){
  if(target.updateMode==='manifest-app'){
    const entry=manifests[target.manifestUrl]?.files?.app;
    return typeof entry==='string'?{url:entry,sha256:manifests[target.manifestUrl]?.artifact_sha256?.[entry]}:entry;
  }
  return firmware.source;
}
export function updateFirmwareCatalog({products,firmwares,watchlist,snapshot,manifests={},now=new Date().toISOString()}){
  validateWatchlist(watchlist);const result={firmwares:copy(firmwares),watchlist:copy(watchlist),manifests:copy(manifests),report:{schemaVersion:'1',checkedAt:now,policy:'Metadata only. Exact device/channel/format selectors; missing SHA-256 retains the pinned artifact. No firmware execution.',entries:[]}};
  const pids=new Set(products.items.map(p=>p.id)),usedTargets=new Set();
  for(const entry of result.watchlist.entries){
    const saved=snapshot.entries?.find(s=>s.id===entry.id&&s.upstreamRepo===entry.upstreamRepo);const targets=entry.catalogTargets||[];
    if(!targets.length){result.report.entries.push({watchlistId:entry.id,status:'retained',reason:'no-explicit-device-targets',metadataStatus:saved?.error?'failed':'checked'});continue;}
    const updated=[];
    for(const target of targets){
      const firmware=result.firmwares.items.find(f=>f.id===target.firmwareId);const report={watchlistId:entry.id,firmwareId:target.firmwareId,productIds:target.productIds,channel:target.channel,format:target.format,sourceType:target.sourceType,previousVersion:firmware?.revision||firmware?.version||null,status:'retained',reason:null};result.report.entries.push(report);
      if(usedTargets.has(target.firmwareId))throw Error(`Firmware target belongs to multiple watch entries: ${target.firmwareId}`);usedTargets.add(target.firmwareId);
      if(!firmware||firmware.watchlistId!==entry.id||!equal([...firmware.productIds].sort(),[...target.productIds].sort())||target.productIds.some(id=>!pids.has(id))){report.reason='target-product-or-source-mismatch';continue;}
      if(!saved||saved.error){report.reason='metadata-unavailable';continue;}
      let manifest;
      if(target.updateMode==='manifest-app'){try{manifest=localManifest(target,firmware,result.manifests);}catch(error){report.reason=error.message;continue;}}
      const releases=(saved.releases?.length?saved.releases:[saved.release]).filter(Boolean).filter(release=>releaseMatchesChannel(release,target.channel)).sort((a,b)=>Date.parse(b.published_at||0)-Date.parse(a.published_at||0));
      const pattern=new RegExp(target.assetNamePattern);let selected;
      for(const release of releases){
        const assets=(release.assets||[]).filter(asset=>asset.name.length<=240&&pattern.test(asset.name));
        if(assets.length>1){report.reason='ambiguous-device-assets';report.candidate={version:release.tag_name,assetNames:assets.map(a=>a.name)};break;}
        if(assets.length===1){selected={release,asset:assets[0]};break;}
      }
      if(report.reason)continue;
      if(!selected){report.reason='no-matching-device-asset';continue;}
      const{release,asset}=selected;const digest=sha(asset.digest),current=report.previousVersion,currentSource=artifactSource(firmware,target,result.manifests);
      report.candidate={version:release.tag_name,publishedAt:release.published_at,assetName:asset.name,url:asset.browser_download_url,sha256:digest};
      if(asset.format&&asset.format!==target.format){report.reason='artifact-format-mismatch';continue;}
      if(norm(release.tag_name)===norm(current)){
        report.reason=digest&&currentSource?.sha256&&digest!==currentSource.sha256?'same-version-different-artifact':'current-version';continue;
      }
      const currentRelease=releases.find(r=>norm(r.tag_name)===norm(current)),currentDate=currentRelease?.published_at||firmware.catalogUpdate?.publishedAt;
      if(older(release.tag_name,current)||currentDate&&Date.parse(release.published_at)<=Date.parse(currentDate)){report.reason='no-newer-matching-release';continue;}
      if(!digest){report.reason='candidate-missing-sha256';continue;}
      if(target.sourceType==='manufacturer-distribution'&&!['m5burner','cloudzao'].includes(entry.metadataProvider)){report.reason='distribution-source-mismatch';continue;}
      const url=target.downloadUrl||asset.browser_download_url;const parsed=new URL(url);if(parsed.protocol!=='https:'||parsed.username||parsed.password)throw Error('Unsafe target source URL');
      firmware.artifactHistory=[...(firmware.artifactHistory||[]),{revision:current,source:copy(firmware.source),status:firmware.status,simulationVerified:firmware.simulationVerified===true,hardwareValidated:firmware.hardwareValidated===true,limitations:copy(firmware.limitations||[]),...Object.fromEntries(['integrityNotes','sdk','verification','validation','evidence','verificationStatus','description','sources'].filter(key=>firmware[key]!==undefined).map(key=>[key,copy(firmware[key])]))}].slice(-12);
      if(target.updateMode==='manifest-app'){
        manifest.files.app={url,sha256:digest};delete manifest.artifact_sha256;
        // Pin all remaining manifest files explicitly before dropping old map.
        for(const [kind,value] of Object.entries(manifest.files))if(typeof value==='string'){
          const old=manifests[target.manifestUrl].artifact_sha256?.[value];if(!old)throw Error('Unpinned ancillary manifest artifact');manifest.files[kind]={url:value,sha256:old};
        }
        result.manifests[target.manifestUrl]=manifest;firmware.source={manifestUrl:target.manifestUrl,sha256:hash(encoded(manifest))};
      }else firmware.source=target.format==='manifest'?{manifestUrl:url,sha256:digest}:{url,sha256:digest};
      firmware.revision=release.tag_name;firmware.version=release.tag_name;firmware.name=target.nameTemplate.replaceAll('{version}',release.tag_name);firmware.status='trial';firmware.simulationVerified=false;firmware.hardwareValidated=false;firmware.compatibility='unverified';firmware.publicationUrl=release.html_url;
      for(const key of ['integrityNotes','sdk','verification','validation','evidence','verificationStatus','description','browserCheckedAt','browserNote'])delete firmware[key];
      firmware.limitations=['按明确设备资产规则更新远程发布元信息；SHA-256 来自发布源，未下载完整镜像或执行验证。','启动时仍须核对实际 hash、芯片、容量与镜像布局；来源版本更新不代表模拟器或实机兼容性通过。',...(target.format==='app'?['这是 app-only 实验直启清单，跳过 bootloader/分区初始化，不能作为完整镜像一键刷写硬件。']:[])];
      firmware.catalogUpdate={watchlistId:entry.id,channel:target.channel,format:target.format,sourceType:target.sourceType,version:release.tag_name,publishedAt:release.published_at,assetName:asset.name,upstreamUrl:asset.browser_download_url,digestSource:'release-metadata',layoutVerified:false,checkedAt:saved.checkedAt||now};
      firmware.sources=[{title:'Device asset mapping basis; not validation of this version',url:target.basis.url,checkedAt:(saved.checkedAt||now).slice(0,10)},{title:'Automatic device release metadata',url:release.html_url,checkedAt:(saved.checkedAt||now).slice(0,10)}];
      report.status='updated';report.reason='new-device-release-with-sha256';updated.push(target.firmwareId);
    }
    if(updated.length){
      entry.artifactMetadataHistory=[...(entry.artifactMetadataHistory||[]),{version:entry.version,...Object.fromEntries(['description','browserNote','browserCheckedAt','browserAuditUrl','compatibility','publishedAt','checkedAt','assets'].filter(key=>entry[key]!==undefined).map(key=>[key,copy(entry[key])]))}].slice(-12);
      entry.description='按设备、发布渠道和镜像格式同步远程版本；新构建未执行验证，旧版本验收记录仅保留在历史中。';
      for(const key of ['browserNote','browserCheckedAt','browserAuditUrl'])delete entry[key];
      const versions=targets.map(t=>result.firmwares.items.find(f=>f.id===t.firmwareId)).filter(Boolean);const names=new Set(versions.map(f=>f.revision||f.version));
      entry.deviceVersions=versions.map(f=>({firmwareId:f.id,productIds:f.productIds,version:f.revision||f.version,publishedAt:f.catalogUpdate?.publishedAt||saved.releases?.find(release=>norm(release.tag_name)===norm(f.revision||f.version))?.published_at||null}));
      if(names.size===1)entry.version=[...names][0];
      entry.publishedAt=names.size===1?entry.deviceVersions[0]?.publishedAt||null:null;
      entry.checkedAt=saved.checkedAt||now;entry.compatibility.status='reference-only';entry.compatibility.simulationVerified=false;entry.compatibility.hardwareValidated=false;entry.compatibility.note='按设备规则同步发布资产；新版本未执行验证，需在已实现板级模型中尝试。';
      entry.assets=targets.map(target=>{const f=result.firmwares.items.find(f=>f.id===target.firmwareId),source=artifactSource(f,target,result.manifests);if(!source?.url)return null;return{name:f.catalogUpdate?.assetName||source.url.split('/').at(-1),url:source.url,sha256:source.sha256,format:target.format,sourcePolicy:target.downloadUrl?'mutable-pinned':'immutable-pinned'};}).filter(Boolean).filter((asset,index,all)=>all.findIndex(a=>a.url===asset.url&&a.sha256===asset.sha256)===index);
    }
  }
  if(result.report.entries.some(entry=>entry.status==='updated'))result.firmwares.revision=now.slice(0,10)+'-device-release-sync';
  result.report.summary={watchEntries:watchlist.entries.length,upstreamRepositories:new Set(watchlist.entries.map(entry=>entry.upstreamRepo)).size,deviceTargets:usedTargets.size,updated:result.report.entries.filter(e=>e.status==='updated').length,retained:result.report.entries.filter(e=>e.status!=='updated').length,firmwareBodiesDownloaded:0,firmwaresExecuted:0};
  validateWatchlist(result.watchlist);return result;
}
export async function runCatalogSync({base=new URL('../',import.meta.url),fetchImpl=fetch,token,now=new Date().toISOString(),snapshotOnly=false,dryRun=false}={}){
  const platform=new URL('web/platform/',base),catalog=new URL('catalog/',platform);const read=async name=>JSON.parse(await readFile(new URL(name,catalog),'utf8'));
  const products=await read('products.json'),firmwares=await read('firmwares.json'),watchlist=await read('firmware-watchlist.json');let previous={entries:[]};try{previous=await read('firmware-release-snapshot.json');}catch{}
  const snapshot=snapshotOnly?copy(previous):await syncReleaseMetadata(watchlist,{previous,token,fetchImpl,now});
  if(snapshotOnly)for(const entry of snapshot.entries){const watch=watchlist.entries.find(w=>w.id===entry.id);if(watch&&entry.releases?.length){entry.releases=compactReleaseHistory(watch,entry.releases);entry.release=entry.releases.find(release=>releaseMatchesChannel(release,watch.releaseChannel))||entry.release;}}
  if(Buffer.byteLength(encoded(snapshot))>2*1024*1024)throw Error('Release snapshot exceeds the browser metadata limit');const manifests={};
  for(const target of watchlist.entries.flatMap(e=>e.catalogTargets||[]))if(target.manifestUrl&&!manifests[target.manifestUrl])manifests[target.manifestUrl]=JSON.parse(await readFile(new URL(target.manifestUrl,platform),'utf8'));
  const result=updateFirmwareCatalog({products,firmwares,watchlist,snapshot,manifests,now});
  const reportUrl=new URL('firmware-catalog-sync-report.json',catalog);let oldReport;try{oldReport=JSON.parse(await readFile(reportUrl,'utf8'));}catch{}
  let recordedReport=result.report;
  if(oldReport&&Date.parse(now)-Date.parse(oldReport.checkedAt)<24*3600*1000){
    const sameObservation=oldReport.summary?.watchEntries===result.report.summary.watchEntries&&oldReport.summary?.upstreamRepositories===result.report.summary.upstreamRepositories&&oldReport.entries.length===result.report.entries.length&&result.report.entries.every(entry=>{
      const old=oldReport.entries.find(previous=>previous.watchlistId===entry.watchlistId&&previous.firmwareId===entry.firmwareId);
      return equal(old,entry)||old?.status==='updated'&&entry.reason==='current-version'&&equal(old.candidate,entry.candidate);
    });
    if(sameObservation)recordedReport=oldReport;
  }
  const files=new Map([[new URL('firmwares.json',catalog),encoded(result.firmwares)],[new URL('firmware-watchlist.json',catalog),encoded(result.watchlist)],[new URL('firmware-release-snapshot.json',catalog),encoded(snapshot)],[reportUrl,encoded(recordedReport)]]);
  for(const[url,manifest]of Object.entries(result.manifests))files.set(new URL(url,platform),encoded(manifest));
  const changed=[];for(const[url,text]of files){let old='';try{old=await readFile(url,'utf8');}catch{}if(old===text)continue;changed.push(fileURLToPath(url).slice(fileURLToPath(base).length));if(!dryRun){await mkdir(dirname(fileURLToPath(url)),{recursive:true});await writeFile(url,text);}}
  return {report:result.report,changed};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href){
  const result=await runCatalogSync({token:githubToken(),snapshotOnly:process.argv.includes('--snapshot-only'),dryRun:process.argv.includes('--dry-run')});
  console.log(JSON.stringify({summary:result.report.summary,changed:result.changed,dryRun:process.argv.includes('--dry-run')},null,2));
}
