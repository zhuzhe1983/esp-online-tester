import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {pathToFileURL} from 'node:url';
import {resolve} from 'node:path';
const repo=resolve(process.argv[2]),out=resolve(process.argv[3]);
const module=path=>pathToFileURL(resolve(repo,path)).href;
const {syncReleaseMetadata,compactReleaseHistory}=await import(module('tools/sync-firmware-release-metadata.mjs'));
const {updateFirmwareCatalog}=await import(module('tools/sync-firmware-catalog.mjs'));
const {resolveCatalogDownloads}=await import(module('tools/resolve-firmware-downloads.mjs'));
const hash=data=>createHash('sha256').update(data).digest('hex');
const base=resolve(repo,'web/platform'),cat=resolve(base,'catalog');
const read=async name=>JSON.parse(await readFile(resolve(cat,name),'utf8'));
const now=new Date().toISOString(),products=await read('products.json'),firmwares=await read('firmwares.json'),watchlist=await read('firmware-watchlist.json'),previous=await read('firmware-release-snapshot.json');
await mkdir(out,{recursive:true});
const seeded=[];
for(const entry of watchlist.entries){
  if(previous.entries.some(x=>x.id===entry.id)||!entry.metadataAssetNamePattern)continue;
  const observed=previous.entries.find(x=>x.upstreamRepo===entry.upstreamRepo&&x.sourceUrl===entry.metadataUrl&&x.release&&x.checkedAt);
  if(!observed)continue;
  const releases=compactReleaseHistory(entry,observed.releases?.length?observed.releases:[observed.release]);
  if(!releases[0]?.assets.length)continue;
  previous.entries.push({...structuredClone(observed),id:entry.id,releases,release:releases[0]});
  seeded.push({id:entry.id,sourceObservationId:observed.id,sourceUrl:observed.sourceUrl,checkedAt:observed.checkedAt,release:releases[0].tag_name,matchedAssets:releases[0].assets.length});
}
const requests=[];
const tracedFetch=async(url,options={})=>{
  const attempt={url:String(url),method:options.method||'GET',attemptedAt:new Date().toISOString()};
  try{
    const response=await fetch(url,options);attempt.status=response.status;
    const accept=new Headers(options.headers||{}).get('accept')||'';
    if(response.ok&&/json/i.test(accept)&&response.body){
      const chunks=[],reader=response.body.getReader();let size=0;
      try{for(;;){const {done,value}=await reader.read();if(done)break;size+=value.length;if(size>32*1024*1024)throw Error('Metadata body limit');chunks.push(value);}}finally{await reader.cancel();}
      const bytes=Buffer.concat(chunks),name=hash(String(url)).slice(0,16)+'.json';
      await writeFile(resolve(out,name),bytes);Object.assign(attempt,{bytes:bytes.length,sha256:hash(bytes),archiveName:name});
      requests.push(attempt);await writeFile(resolve(out,'requests.json'),JSON.stringify(requests,null,2)+'\n');
      return new Response(bytes,{status:response.status,headers:response.headers});
    }
    requests.push(attempt);await writeFile(resolve(out,'requests.json'),JSON.stringify(requests,null,2)+'\n');return response;
  }catch(error){attempt.status??=null;attempt.errorType=error.name;requests.push(attempt);await writeFile(resolve(out,'requests.json'),JSON.stringify(requests,null,2)+'\n');throw error;}
};
const snapshot=await syncReleaseMetadata(watchlist,{previous,fetchImpl:tracedFetch,now});
const manifests={};
for(const target of watchlist.entries.flatMap(x=>x.catalogTargets||[]))if(target.manifestUrl&&!manifests[target.manifestUrl])manifests[target.manifestUrl]=JSON.parse(await readFile(resolve(base,target.manifestUrl),'utf8'));
const result=updateFirmwareCatalog({products,firmwares,watchlist,snapshot,manifests,now});
await resolveCatalogDownloads(result,{fetchImpl:tracedFetch,now});
const sourceChanges=result.firmwares.items.flatMap(f=>{
  const old=firmwares.items.find(x=>x.id===f.id);
  if(JSON.stringify([old.source,old.version,old.revision])===JSON.stringify([f.source,f.version,f.revision]))return [];
  return [{id:f.id,productIds:f.productIds,before:{source:old.source,version:old.version,revision:old.revision},after:{source:f.source,version:f.version,revision:f.revision}}];
});
const manifestChanges=Object.keys(manifests).filter(p=>JSON.stringify(manifests[p])!==JSON.stringify(result.manifests[p]));
const snapshotText=JSON.stringify(snapshot)+'\n';
if(Buffer.byteLength(snapshotText)>2*1024*1024)throw Error('Snapshot exceeds 2 MiB');
for(const[name,data]of Object.entries({'proposed-firmwares.json':result.firmwares,'proposed-watchlist.json':result.watchlist,'proposed-report.json':result.report,'proposed-manifests.json':result.manifests}))await writeFile(resolve(out,name),JSON.stringify(data,null,2)+'\n');
await writeFile(resolve(out,'proposed-snapshot.json'),snapshotText);
const receipt={schemaVersion:'1',checkedAt:now,finishedAt:new Date().toISOString(),watchEntries:watchlist.entries.length,snapshotBytes:Buffer.byteLength(snapshotText),snapshotSha256:hash(snapshotText),snapshotLimitBytes:2*1024*1024,seededHistoricalObservations:seeded,requests,sourceChanges,manifestChanges,summary:result.report.summary,firmwareBodiesDownloaded:0,firmwaresExecuted:0,repositoryFilesWritten:false};
await writeFile(resolve(out,'receipt.json'),JSON.stringify(receipt,null,2)+'\n');
console.log(JSON.stringify({watchEntries:receipt.watchEntries,snapshotBytes:receipt.snapshotBytes,requests:requests.length,seeded:seeded.length,sourceChanges,manifestChanges,summary:result.report.summary},null,2));
