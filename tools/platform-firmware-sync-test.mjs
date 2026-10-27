import test from 'node:test';
import assert from 'node:assert/strict';
import {updateFirmwareCatalog} from './sync-firmware-catalog.mjs';
import {syncReleaseMetadata} from './sync-firmware-release-metadata.mjs';
import {publicReleases,releaseMatchesChannel} from './firmware-release-source.mjs';
const H='a'.repeat(64),NEXT='b'.repeat(64),NOW='2026-10-27T02:00:00Z';
const asset=(device,version,hash=NEXT)=>({name:`fw-${version}-${device}.bin`,browser_download_url:`https://example.com/${version}/${device}.bin`,size:100,digest:hash?`sha256:${hash}`:null});
const release=(version,devices,day,{prerelease=false}={})=>({tag_name:version,html_url:`https://github.com/example/firmware/releases/tag/${version}`,published_at:`2026-10-${day}T00:00:00Z`,prerelease,assets:devices.map(device=>asset(device,version))});
const target=(device,channel='stable')=>({firmwareId:`firmware-${device}`,productIds:[device],channel,format:'merged',sourceType:'upstream-release',updateMode:'source',assetNamePattern:`^fw-[0-9A-Za-z.-]+-${device}\\.bin$`,nameTemplate:`Device ${device} · {version}`,basis:{url:'https://example.com/docs',note:'Explicit device filename'}});
function fixture(devices=['one','two']){
 const entry={id:'project',name:'Example',description:'Version 1.0 verified',browserNote:'Old browser verification passed',publishedAt:'2026-10-20',category:'factory',upstreamRepo:'example/firmware',metadataUrl:'https://api.github.com/repos/example/firmware/releases/latest',releaseUrl:'https://github.com/example/firmware/releases',releaseHistoryLimit:20,version:'1.0.0',checkedAt:'2026-10-20T00:00:00Z',assets:[],compatibility:{status:'reference-only',productIds:devices},catalogTargets:devices.map(d=>target(d)),storage:'remote-links-only'};
 return{products:{items:devices.map(id=>({id,runtime:{board:null}}))},firmwares:{schemaVersion:'1',revision:'old',items:devices.map(d=>({id:`firmware-${d}`,productIds:[d],watchlistId:'project',revision:'1.0.0',name:`Device ${d} · 1.0.0`,status:'available',source:{url:`https://cdn.example.com/old-${d}.bin`,sha256:H},sources:[{title:'Original evidence',url:'https://example.com/source',checkedAt:'2026-10-20'}],limitations:['Old version tested'],simulationVerified:true,hardwareValidated:true,integrityNotes:{oldWarning:true},sdk:'old SDK',evidence:{old:'old release pass'},verification:{passed:true},compatibility:'verified'}))},watchlist:{schemaVersion:'1',categories:[{id:'factory'}],entries:[entry]},snapshot:{entries:[{id:'project',upstreamRepo:'example/firmware',checkedAt:NOW,releases:[],error:null}]},now:NOW};
}
test('one project resolves newest device assets independently, even without runtime boards',()=>{
 const f=fixture();f.snapshot.entries[0].releases=[release('2.0.0',['one'],'26'),release('1.4.0',['two'],'24'),release('1.0.0',['one','two'],'20')];const r=updateFirmwareCatalog(f);
 assert.deepEqual(r.firmwares.items.map(x=>x.revision),['2.0.0','1.4.0']);assert.equal(r.report.summary.updated,2);assert.equal(r.watchlist.entries[0].deviceVersions[1].version,'1.4.0');assert.equal(r.watchlist.entries[0].browserNote,undefined);assert.equal(r.watchlist.entries[0].publishedAt,null);assert.equal(r.watchlist.entries[0].description.includes('1.0 verified'),false);assert.equal(r.watchlist.entries[0].artifactMetadataHistory[0].description,'Version 1.0 verified');assert.equal(r.watchlist.entries[0].artifactMetadataHistory[0].browserNote,'Old browser verification passed');assert.equal(r.watchlist.entries[0].artifactMetadataHistory[0].publishedAt,'2026-10-20');
 for(const x of r.firmwares.items){assert.equal(x.simulationVerified,false);assert.equal(x.hardwareValidated,false);assert.equal(x.status,'trial');assert.equal(x.integrityNotes,undefined);assert.equal(x.sdk,undefined);assert.equal(x.evidence,undefined);assert.equal(x.verification,undefined);assert.equal(x.compatibility,'unverified');assert.deepEqual(x.artifactHistory[0].evidence,{old:'old release pass'});assert.equal(x.artifactHistory[0].integrityNotes.oldWarning,true);assert.equal(x.source.sha256,NEXT);}
 assert.equal(f.firmwares.items[0].revision,'1.0.0','inputs never mutated');
});
test('a device missing from new releases retains its previous pinned artifact',()=>{
 const f=fixture();f.snapshot.entries[0].releases=[release('2.0.0',['one'],'26')];const r=updateFirmwareCatalog(f);assert.deepEqual(r.firmwares.items[1],f.firmwares.items[1]);assert.equal(r.report.entries[1].reason,'no-matching-device-asset');
});
test('new version without provider SHA never inherits the old hash',()=>{
 const f=fixture(['one']);const v=release('2.0.0',['one'],'26');v.assets[0].digest=null;f.snapshot.entries[0].releases=[v];const r=updateFirmwareCatalog(f);assert.deepEqual(r.firmwares,f.firmwares);assert.equal(r.report.entries[0].reason,'candidate-missing-sha256');assert.equal(r.report.entries[0].candidate.sha256,null);
});
test('stable skips RC, RC skips stable and beta releases',()=>{
 for(const[channel,expected]of [['stable','1.4.0'],['rc','2.0.0rc']]){const f=fixture(['one']);f.watchlist.entries[0].catalogTargets[0].channel=channel;f.snapshot.entries[0].releases=[release('3.0.0beta',['one'],'27',{prerelease:true}),release('2.0.0rc',['one'],'26',{prerelease:true}),release('1.4.0',['one'],'24')];const r=updateFirmwareCatalog(f);assert.equal(r.firmwares.items[0].revision,expected);}
 assert.equal(releaseMatchesChannel({tag_name:'2.0.0rc',prerelease:false},'stable'),false);
});
test('ambiguous assets and changed formats fail closed without choosing the first file',()=>{
 for(const format of [false,true]){const f=fixture(['one']),v=release('2.0.0',['one'],'26');if(format)v.assets[0].format='app';else v.assets.push({...v.assets[0],browser_download_url:'https://example.com/duplicate.bin'});f.snapshot.entries[0].releases=[v];const r=updateFirmwareCatalog(f);assert.deepEqual(r.firmwares,f.firmwares);assert.equal(r.report.entries[0].reason,format?'artifact-format-mismatch':'ambiguous-device-assets');}
});
test('same version preserves a working CDN; changed artifact stays a reported candidate',()=>{
 const f=fixture(['one']);f.snapshot.entries[0].releases=[release('1.0.0',['one'],'26')];let r=updateFirmwareCatalog(f);assert.equal(r.report.entries[0].reason,'same-version-different-artifact');assert.deepEqual(r.firmwares,f.firmwares);
 f.snapshot.entries[0].releases[0].assets[0].digest=`sha256:${H}`;r=updateFirmwareCatalog(f);assert.equal(r.report.entries[0].reason,'current-version');assert.equal(r.firmwares.items[0].source.url,'https://cdn.example.com/old-one.bin');
});
test('missing latest device asset cannot downgrade an already newer firmware',()=>{
 const f=fixture(['one']);f.firmwares.items[0].revision='3.0.0';f.snapshot.entries[0].releases=[release('2.0.0',['one'],'26')];const r=updateFirmwareCatalog(f);assert.equal(r.report.entries[0].reason,'no-newer-matching-release');assert.equal(r.firmwares.items[0].revision,'3.0.0');
});
test('app manifest update pins just its explicit remote app and preserves its ROM',()=>{
 const f=fixture(['one']);const path='./firmware-plans/example.json',rom={url:'../../wasm/fw/rom.elf',sha256:H};f.manifests={[path]:{board:'papermono',app_direct:true,files:{rom,app:{url:'https://example.com/old-app.bin',sha256:H}}}};
 f.firmwares.items[0].source={manifestUrl:path,sha256:H};Object.assign(f.watchlist.entries[0].catalogTargets[0],{format:'app',updateMode:'manifest-app',manifestUrl:path,downloadUrl:'https://example.com/api/app?device=one'});f.snapshot.entries[0].releases=[release('2.0.0',['one'],'26')];const r=updateFirmwareCatalog(f);
 assert.deepEqual(r.manifests[path].files.rom,rom);assert.equal(r.manifests[path].app_direct,true);assert.equal(r.manifests[path].files.app.sha256,NEXT);assert.equal(r.manifests[path].files.app.url,'https://example.com/api/app?device=one');assert.notEqual(r.firmwares.items[0].source.sha256,H);assert.equal(r.firmwares.items[0].source.url,undefined);
});
test('manufacturer distribution is not replaced by another project sharing its repository',()=>{
 const f=fixture(['one']);f.watchlist.entries[0].catalogTargets[0].sourceType='manufacturer-distribution';f.snapshot.entries[0].releases=[release('2.0.0',['one'],'26')];assert.equal(updateFirmwareCatalog(f).report.entries[0].reason,'distribution-source-mismatch');
});
test('target products must match the full firmware product set; metadata errors retain it',()=>{
 const f=fixture(['one']);f.firmwares.items[0].productIds.push('unmapped');f.snapshot.entries[0].releases=[release('2.0.0',['one'],'26')];assert.equal(updateFirmwareCatalog(f).report.entries[0].reason,'target-product-or-source-mismatch');f.firmwares.items[0].productIds=['one'];f.snapshot.entries[0].error={status:403};assert.equal(updateFirmwareCatalog(f).report.entries[0].reason,'metadata-unavailable');
});
test('history fetch is cached, strips publisher account data, and excludes drafts',async()=>{
 const f=fixture(['one']);const entry=f.watchlist.entries[0];f.watchlist.entries.push({...entry,id:'second',catalogTargets:[]});let calls=0;
 const fake=async(url,options)=>{calls++;assert.equal(url,'https://api.github.com/repos/example/firmware/releases?per_page=20');assert.equal(options.headers.Authorization,'Bearer test-token');return Response.json([{...release('2.0.0',['one'],'26'),author:{login:'private'},assets:release('2.0.0',['one'],'26').assets},{...release('99.0.0',['one'],'27'),draft:true}]);};
 const result=await syncReleaseMetadata(f.watchlist,{fetchImpl:fake,token:'test-token',now:NOW});assert.equal(calls,1);assert.equal(result.entries[0].release.tag_name,'2.0.0');assert.equal(JSON.stringify(result).includes('private'),false);assert.equal(JSON.stringify(result).includes('test-token'),false);
 const again=await syncReleaseMetadata(f.watchlist,{previous:result,fetchImpl:fake,token:'test-token',now:'2026-10-27T03:00:00Z'});assert.deepEqual(again,result,'same-day check does not churn timestamps');
});
test('M5Burner failure is cached and never receives a GitHub token',async()=>{
 const f=fixture(['one']);const entry=f.watchlist.entries[0];Object.assign(entry,{metadataProvider:'m5burner',firmwareId:'1234567890123456',metadataUrl:'https://m5burner-api.m5stack.com/api/firmware'});f.watchlist.entries.push({...entry,id:'second',catalogTargets:[]});let calls=0;
 const r=await syncReleaseMetadata(f.watchlist,{token:'test-token',now:NOW,fetchImpl:async(url,options)=>{calls++;assert.equal(options.headers.Authorization,undefined);return new Response(null,{status:403});}});assert.equal(calls,1);assert.equal(r.entries.length,2);assert.ok(r.entries.every(x=>x.error.status===403));
});
test('manufacturer history retains old versions and rejects unsafe files',()=>{
 const e={metadataProvider:'m5burner',firmwareId:'1234567890123456',releaseUrl:'https://example.com/source'};
 const versions=[{version:'1.0',published_at:'2026.10.01',published:true,file:'old.bin'},{version:'2.0',published_at:'2026-10-20',published:true,file:'new.bin',sha256:NEXT}];const releases=publicReleases(e,[{fid:e.firmwareId,author:'do not retain',versions}]);assert.equal(releases.length,2);assert.equal(releases[0].assets[0].digest,`sha256:${NEXT}`);assert.equal(releases[1].published_at,'2026-10-01');versions.push({version:'3.0',published_at:'2026-10-22',published:true,file:'../unsafe.bin'});assert.throws(()=>publicReleases(e,[{fid:e.firmwareId,versions}]));
});

test('a second same-day predeploy check does not produce a timestamp/report-only commit',async()=>{
 const {mkdtemp,mkdir,writeFile,rm}=await import('node:fs/promises'),{tmpdir}=await import('node:os'),{join}=await import('node:path'),{pathToFileURL}=await import('node:url');
 const {runCatalogSync}=await import('./sync-firmware-catalog.mjs');const directory=await mkdtemp(join(tmpdir(),'firmware-sync-fixture-'));
 try{
  const catalog=join(directory,'web/platform/catalog');await mkdir(catalog,{recursive:true});const f=fixture(['one']);f.snapshot={schemaVersion:'1',generatedAt:NOW,entries:[{id:'project',upstreamRepo:'example/firmware',checkedAt:NOW,releases:[release('2.0.0',['one'],'26')],error:null}]};
  for(const[name,value]of Object.entries({'products.json':f.products,'firmwares.json':f.firmwares,'firmware-watchlist.json':f.watchlist,'firmware-release-snapshot.json':f.snapshot}))await writeFile(join(catalog,name),JSON.stringify(value,null,2)+'\n');
  const first=await runCatalogSync({base:pathToFileURL(directory+'/'),snapshotOnly:true,now:NOW});assert.equal(first.report.summary.updated,1);assert.ok(first.changed.length);
  const second=await runCatalogSync({base:pathToFileURL(directory+'/'),snapshotOnly:true,now:'2026-10-27T03:00:00Z'});assert.equal(second.report.summary.updated,0);assert.deepEqual(second.changed,[]);
 }finally{await rm(directory,{recursive:true,force:true});}
});
