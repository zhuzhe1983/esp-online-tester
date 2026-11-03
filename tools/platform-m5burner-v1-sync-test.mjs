import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {publicReleases,releaseRequest} from './firmware-release-source.mjs';
import {syncReleaseMetadata} from './sync-firmware-release-metadata.mjs';
import {updateFirmwareCatalog} from './sync-firmware-catalog.mjs';
import {validateWatchlist} from '../web/platform/watchlist.js';
import {firstFirmware} from '../web/platform/catalog-selection.js';
const P='1897200000000002172',D='1897100000000000022',V='2113079049317601281',HASH='a'.repeat(64),NOW='2026-10-27T13:00:00Z';
const target={firmwareId:'test-uiflow',productIds:['test'],channel:'stable',format:'merged',sourceType:'manufacturer-distribution',updateMode:'source',assetNamePattern:'^[-\\w.]+\\.bin$',nameTemplate:'UIFlow · {version}',basis:{url:'https://docs.m5stack.com/en/core/StopWatch',note:'Exact model and project'}};
function fixture(){
 const entry={id:'official-uiflow',category:'development',upstreamRepo:'m5stack/uiflow-micropython',metadataProvider:'m5burner-v1',firmwareId:P,deviceId:D,metadataUrl:`https://burner.m5stack.com/api/v1/firmwares/${P}`,releaseUrl:`https://burner.m5stack.com/firmware/${P}`,compatibility:{status:'reference-only',productIds:['test']},assets:[],checkedAt:NOW,catalogTargets:[structuredClone(target)]};
 const metadata={project:{code:200,data:{firmwareId:P,sourceType:'OFFICIAL',supportedDevices:[{deviceId:D}],developer:{nickname:'unnecessary-account'}}},versions:{code:200,data:[{versionId:V,versionName:'v2.5.3',publishTime:'2026-10-22T09:24:41+08:00',visibility:'PUBLIC',status:'PUBLISHED',binFileName:'uiflow-stopwatch.bin',supportedDevices:[{deviceId:D}]}]}};
 return {entry,metadata};
}
function catalog(entry,releases){return {products:{items:[{id:'test'}]},firmwares:{items:[{id:'test-uiflow',productIds:['test'],revision:'2.5.2',source:{url:'https://example.com/old.bin',sha256:HASH},watchlistId:entry.id,simulationVerified:true,status:'available',limitations:['old version verified']}]},watchlist:{schemaVersion:'1',categories:[{id:'development'}],entries:[entry]},snapshot:{entries:[{id:entry.id,upstreamRepo:entry.upstreamRepo,checkedAt:NOW,releases}]},now:NOW};}
test('official v1 reads project and PUBLIC versions only; string IDs survive and tokens/accounts do not',async()=>{
 const {entry,metadata}=fixture();const list={schemaVersion:'1',categories:[{id:'development'}],entries:[entry,{...entry,id:'other-watch',catalogTargets:[]}]},calls=[];
 const fetchImpl=async(url,options)=>{calls.push(url);assert.equal(options.headers.Authorization,undefined);assert.equal(options.redirect,'error');assert.ok(!url.endsWith('/download'));return Response.json(url===entry.metadataUrl?metadata.project:metadata.versions);};
 const r=await syncReleaseMetadata(list,{fetchImpl,token:'private-token',now:NOW});assert.equal(calls.length,2);assert.equal(r.entries[0].error,null);assert.equal(r.entries[0].release.assets[0].publisherIdentity.versionId,V);assert.equal(r.entries[0].release.assets[0].digest,null);assert.ok(!JSON.stringify(r).includes('unnecessary-account'));assert.ok(!JSON.stringify(r).includes('private-token'));
});
test('OFFICIAL identity and exact project/device membership fail closed; unpublished versions cannot leak',()=>{
 for(const mutate of [d=>d.project.data.sourceType='COMMUNITY',d=>d.project.data.firmwareId='9',d=>d.project.data.supportedDevices=[{deviceId:'8'}],d=>d.project.code=403]){const f=fixture();mutate(f.metadata);assert.throws(()=>publicReleases(f.entry,f.metadata));}
 for(const mutate of [v=>v.visibility='PRIVATE',v=>v.status='DRAFT',v=>v.supportedDevices=[{deviceId:'8'}]]){const f=fixture();mutate(f.metadata.versions.data[0]);assert.equal(publicReleases(f.entry,f.metadata).length,0);}
 const f=fixture();f.entry.firmwareId=Number(P);assert.throws(()=>releaseRequest(f.entry));assert.throws(()=>validateWatchlist({schemaVersion:'1',categories:[{id:'development'}],entries:[f.entry]}));
});
test('specific edition filtering cannot choose current AtomS3 build for AtomS3R or TOUGH for Core2',()=>{
 const {entry,metadata}=fixture();entry.versionNamePattern='^v[0-9.]+-NiHaoXiaoZhi$';metadata.versions.data[0].versionName='v2.2.6-AtomS3';metadata.versions.data.push({...metadata.versions.data[0],versionId:'2113079049317601282',versionName:'v2.2.6-NiHaoXiaoZhi'});assert.deepEqual(publicReleases(entry,metadata).map(x=>x.tag_name),['v2.2.6-NiHaoXiaoZhi']);
});
test('new official immutable version may advance without publisher hash, archives evidence and clears verification',()=>{
 const {entry,metadata}=fixture(),r=updateFirmwareCatalog(catalog(entry,publicReleases(entry,metadata)));const f=r.firmwares.items[0];assert.equal(f.revision,'v2.5.3');assert.equal(f.source.sha256,undefined);assert.equal(f.status,'trial');assert.equal(f.simulationVerified,false);assert.equal(f.hardwareValidated,false);assert.equal(f.artifactHistory[0].source.sha256,HASH);assert.equal(f.artifactHistory[0].simulationVerified,true);assert.equal(f.catalogUpdate.digestSource,'publisher-hash-unavailable');assert.equal(r.report.summary.firmwareBodiesDownloaded,0);
});
test('hashless exception cannot accept substituted URL, community identity, other device or other provider',()=>{
 for(const mutate of [a=>a.browser_download_url='https://example.com/file.bin',a=>a.publisherIdentity.sourceType='COMMUNITY',a=>a.publisherIdentity.deviceId='9',a=>a.publisherIdentity.versionId=Number(V),a=>delete a.publisherIdentity]){
  const {entry,metadata}=fixture(),releases=publicReleases(entry,metadata);mutate(releases[0].assets[0]);const f=catalog(entry,releases),r=updateFirmwareCatalog(f);assert.deepEqual(r.firmwares,f.firmwares);assert.equal(r.report.entries[0].reason,'official-version-identity-mismatch');
 }
});
test('same-version pinned SHA/URL are never overwritten, with or without a publisher digest',()=>{
 for(const sha256 of [undefined,HASH,'b'.repeat(64)]){const {entry,metadata}=fixture();if(sha256)metadata.versions.data[0].sha256=sha256;const f=catalog(entry,publicReleases(entry,metadata));f.firmwares.items[0].revision='2.5.3';const r=updateFirmwareCatalog(f);assert.deepEqual(r.firmwares,f.firmwares);assert.equal(r.report.entries[0].reason,sha256&&sha256!==HASH?'same-version-different-artifact':'current-version');}
});
async function republishedFixture(mutate=()=>{}){
 const {entry,metadata}=fixture(),oldId='2113079049317601280',oldDate='2026-10-21T00:00:00Z';entry.releaseHistoryLimit=1;
 mutate(metadata,oldId);const f=catalog(entry,[]),firmware=f.firmwares.items[0];
 firmware.revision='v2.5.3';firmware.source={url:`${entry.metadataUrl}/versions/${oldId}/download`};
 firmware.catalogUpdate={publishedAt:oldDate,publisherIdentity:{provider:'m5burner-v1',sourceType:'OFFICIAL',firmwareId:P,deviceId:D,versionId:oldId,visibility:'PUBLIC',status:'PUBLISHED'}};
 firmware.downloadAccess={url:firmware.source.url,status:'verification-required'};firmware.verification={passed:true};
 f.snapshot=await syncReleaseMetadata(f.watchlist,{now:NOW,fetchImpl:async url=>Response.json(url===entry.metadataUrl?metadata.project:metadata.versions)});
 return f;
}
test('a fresh complete version inventory permits a same-label official republish, preserving old evidence and clearing validation',async()=>{
 const f=await republishedFixture(),r=updateFirmwareCatalog(f),firmware=r.firmwares.items[0];
 assert.equal(r.report.entries[0].reason,'republished-official-version-without-publisher-sha256');assert.equal(firmware.revision,'v2.5.3');assert.equal(firmware.source.url,`${f.watchlist.entries[0].metadataUrl}/versions/${V}/download`);
 assert.equal(firmware.status,'trial');assert.equal(firmware.simulationVerified,false);assert.equal(firmware.hardwareValidated,false);assert.equal(firmware.verification,undefined);assert.equal(firmware.downloadAccess,undefined);
 assert.deepEqual(firmware.artifactHistory[0].source,f.firmwares.items[0].source);assert.deepEqual(firmware.artifactHistory[0].downloadAccess,f.firmwares.items[0].downloadAccess);assert.equal(firmware.artifactHistory[0].verification.passed,true);
 assert.deepEqual(f.snapshot.entries[0].versionInventory.versionIds,[V]);assert.equal(r.report.summary.firmwareBodiesDownloaded,0);
});
test('old IDs retained outside compact history or excluded by status/device filters do not authorize replacement',async()=>{
 for(const override of [{},{status:'DRAFT'},{visibility:'PRIVATE'},{supportedDevices:[{deviceId:'9'}]}]){
  const f=await republishedFixture((metadata,oldId)=>metadata.versions.data.push({...metadata.versions.data[0],versionId:oldId,publishTime:'2026-10-21T00:00:00Z',...override}));
  assert.equal(f.snapshot.entries[0].releases.length,1);assert.equal(f.snapshot.entries[0].versionInventory.versionIds.length,2);
  assert.deepEqual(updateFirmwareCatalog(f).firmwares,f.firmwares);
 }
});
test('same-label republishing needs fresh successful complete metadata, exact identities and an unpinned current source',async()=>{
 for(const mutate of [
  f=>f.firmwares.items[0].source.sha256=HASH,
  f=>f.firmwares.items[0].source.url='https://example.com/old.bin',
  f=>f.firmwares.items[0].catalogUpdate.publisherIdentity.deviceId='9',
  f=>f.firmwares.items[0].catalogUpdate.publisherIdentity.provider='m5burner',
  f=>delete f.firmwares.items[0].catalogUpdate.publishedAt,
  f=>f.firmwares.items[0].catalogUpdate.publishedAt='2026-10-28T00:00:00Z',
  f=>f.snapshot.entries[0].releases[0].assets[0].publisherIdentity.provider='m5burner',
  f=>f.snapshot.entries[0].releases[0].assets[0].publisherIdentity.firmwareId='9',
  f=>delete f.snapshot.entries[0].versionInventory,
  f=>f.snapshot.entries[0].versionInventory.complete=false,
  f=>f.snapshot.entries[0].versionInventory.versionIds=[],
  f=>f.snapshot.entries[0].versionInventory.firmwareId='9',
  f=>f.snapshot.entries[0].versionInventory.deviceId='9',
  f=>f.snapshot.entries[0].versionInventory.sourceUrl+='&page=1',
  f=>f.snapshot.entries[0].sourceUrl='https://example.com/metadata',
  f=>f.snapshot.entries[0].checkedAt='2026-10-26T12:59:59Z',
  f=>f.snapshot.entries[0].checkedAt='2026-10-28T00:00:00Z',
  f=>f.snapshot.entries[0].error={status:503},
 ]){const f=await republishedFixture();mutate(f);assert.deepEqual(updateFirmwareCatalog(f).firmwares,f.firmwares);}
});
test('complete inventory changes outside compact history refresh metadata and failed version fetches cannot reuse inventory',async()=>{
 const f=await republishedFixture(),{entry,metadata}=fixture();entry.releaseHistoryLimit=1;
 metadata.versions.data.push({...metadata.versions.data[0],versionId:'7',publishTime:'2020-01-01T00:00:00Z'});
 const list={...f.watchlist,entries:[entry]},later='2026-10-27T14:00:00Z';
 const next=await syncReleaseMetadata(list,{previous:f.snapshot,now:later,fetchImpl:async url=>Response.json(url===entry.metadataUrl?metadata.project:metadata.versions)});
 assert.equal(next.entries[0].checkedAt,later);assert.deepEqual(next.entries[0].versionInventory.versionIds,[V,'7']);
 const failed=await syncReleaseMetadata(list,{previous:next,now:later,fetchImpl:async url=>url===entry.metadataUrl?Response.json(metadata.project):new Response(null,{status:503})});
 assert.ok(failed.entries[0].error);assert.equal(failed.entries[0].versionInventory,undefined);
 metadata.versions={code:200,data:{records:metadata.versions.data,total:2}};assert.throws(()=>publicReleases(entry,metadata));
});
test('bad project response retains prior snapshot and does not fetch version list',async()=>{
 const {entry}=fixture(),list={schemaVersion:'1',categories:[{id:'development'}],entries:[entry]};let calls=0;const previous={entries:[{id:entry.id,upstreamRepo:entry.upstreamRepo,release:{tag_name:'old'},releases:[],checkedAt:NOW}]};const r=await syncReleaseMetadata(list,{previous,fetchImpl:async()=>{calls++;return Response.json({code:200,data:{sourceType:'COMMUNITY'}});},now:NOW});assert.equal(calls,1);assert.equal(r.entries[0].release.tag_name,'old');assert.ok(r.entries[0].error);
});
test('checked-in mapping replays exact metadata; factory remains first and every watched device target is scoped',async()=>{
 const read=async p=>JSON.parse(await readFile(new URL(p,import.meta.url),'utf8'));
 const [list,fw,products,data,mapping]=await Promise.all(['../web/platform/catalog/firmware-watchlist.json','../web/platform/catalog/firmwares.json','../web/platform/catalog/products.json','../docs/platform/evidence/m5-official-firmware-catalog/metadata.json','../docs/platform/evidence/m5-official-firmware-catalog/mapping.json'].map(read));validateWatchlist(list);
 for(const m of mapping.mappings){const entry=list.entries.find(x=>x.id===m.watchlistId);assert.deepEqual(entry.compatibility.productIds,m.productIds);const releases=publicReleases(entry,data.find(x=>x.projectId===m.projectId));assert.ok(releases.length);assert.equal(releases[0].assets[0].publisherIdentity.versionId,m.initialVersionId);for(const t of entry.catalogTargets)assert.deepEqual(fw.items.find(f=>f.id===t.firmwareId).productIds,t.productIds);}
 const stopwatch=products.items.find(x=>x.id==='m5stack-stopwatch');assert.equal(firstFirmware(fw.items,stopwatch).id,stopwatch.defaultFirmwareId);for(const family of ['uiflow2','xiaozhi'])assert.ok(fw.items.find(f=>f.id===`m5stack-stopwatch-${family}`)?.source.url);
 const mono=fw.items.find(f=>f.id==='papermono-uiflow2');const pinnedSha='f3da049b65b5a3899d3484594e3bede92a0928e5217fa260925951a777447b05';assert.ok(mono?.source?.url);const retained=[mono.source.sha256,...((mono.artifactHistory||[]).map(h=>h.source?.sha256))].filter(Boolean);assert.ok(retained.includes(pinnedSha),'PaperMono UIFlow2 must keep the audited CDN digest on the current source or in artifactHistory');if(!mono.source.sha256)assert.equal(mono.catalogUpdate?.digestSource,'publisher-hash-unavailable');assert.ok(!list.entries.find(e=>e.id==='m5-official-xiaozhi-k146').compatibility.productIds.includes('m5stack-k049-b'),'Paper category must not route Xiaozhi Card Kit to M5Paper');
});

test('legacy non-bin metadata does not poison valid modern releases, while paths and duplicate version IDs fail closed',()=>{
 const {entry,metadata}=fixture();metadata.versions.data.push({...metadata.versions.data[0],versionId:'1897300000000005234',versionName:'v2.0.4',publishTime:'2024-01-01T00:00:00Z',binFileName:'2ada62a2b146816415a92fa3d37844affile'});assert.equal(publicReleases(entry,metadata)[0].tag_name,'v2.5.3');
 metadata.versions.data[1].binFileName='../payload.bin';assert.throws(()=>publicReleases(entry,metadata));metadata.versions.data[1].binFileName='payload.bin';metadata.versions.data[1].versionId=V;assert.throws(()=>publicReleases(entry,metadata));
 entry.upstreamRepo=null;assert.doesNotThrow(()=>validateWatchlist({schemaVersion:'1',categories:[{id:'development'}],entries:[entry]}));entry.metadataProvider='github';assert.throws(()=>validateWatchlist({schemaVersion:'1',categories:[{id:'development'}],entries:[entry]}));
});
