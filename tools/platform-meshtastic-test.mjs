import test from 'node:test';
import assert from 'node:assert/strict';
import {enrichMeshtasticReleases} from './meshtastic-release-metadata.mjs';
import {updateFirmwareCatalog} from './sync-firmware-catalog.mjs';
import {syncReleaseMetadata} from './sync-firmware-release-metadata.mjs';
import {readFile} from 'node:fs/promises';
import {firmwareRunPolicy} from '../web/platform/firmware-policy.js';
const target=(board,chip,channel='stable')=>({boardTarget:board,targetChip:chip,format:'merged',channel});
const entry={assetMetadata:'meshtastic-targets',upstreamRepo:'meshtastic/firmware',catalogTargets:[target('m5stack-unitc6l','esp32c6'),target('m5stack-cardputer-adv','esp32s3')]};
const releases=[{tag_name:'v2.1.0.abcdef0',published_at:'2026-10-27',assets:[]},{tag_name:'v2.0.0.abcdef0',published_at:'2026-10-01',assets:[]}];
function manifest(board,version='2.1.0.abcdef0',chip='esp32c6') {return {version,repo:'meshtastic/firmware',platformioTarget:board,mcu:chip,files:[{name:`firmware-${board}-${version}.factory.bin`,bytes:200,md5:'a'.repeat(32)}]};}
test('Meshtastic resolves each exact board independently and reads metadata only',async()=>{
 const urls=[];const r=await enrichMeshtasticReleases(entry,releases,{fetchImpl:async(url,options)=>{
  urls.push(url);assert.equal(options.headers.Authorization,undefined);assert.ok(url.endsWith('.mt.json'));
  const mesh=url.includes('cardputer');if(mesh&&url.includes('2.1.0'))return new Response(null,{status:404});
  return Response.json(manifest(mesh?'m5stack-cardputer-adv':'m5stack-unitc6l',mesh?'2.0.0.abcdef0':'2.1.0.abcdef0',mesh?'esp32s3':'esp32c6'));
 }});
 assert.equal(urls.length,3);assert.equal(r[0].assets[0].boardTarget,'m5stack-unitc6l');assert.equal(r[1].assets[0].boardTarget,'m5stack-cardputer-adv');assert.equal(r[0].assets[0].digest,null);assert.equal(r[0].assets[0].publisherMd5,'a'.repeat(32));assert.match(r[0].assets[0].metadata.sha256,/^[a-f0-9]{64}$/);assert.equal(releases[0].assets.length,0);
});
test('Meshtastic fails on mismatched identity, duplicate images and oversized metadata',async()=>{
 const single={...entry,catalogTargets:[entry.catalogTargets[0]]};
 for(const mutate of [d=>d.version='wrong',d=>d.mcu='esp32s3',d=>d.platformioTarget='other',d=>d.files.push(d.files[0]),d=>d.files[0].name='../other.factory.bin']){
  const d=manifest('m5stack-unitc6l');mutate(d);await assert.rejects(enrichMeshtasticReleases(single,releases,{fetchImpl:async()=>Response.json(d)}));
 }
 await assert.rejects(enrichMeshtasticReleases(single,releases,{fetchImpl:async()=>new Response(' '.repeat(256*1024+1))}),/exceeds/);
});
test('classic ESP32 T-Beam targets retain the chip and exact board identity',async()=>{
 const single={...entry,catalogTargets:[target('tbeam','esp32')]};
 const good=manifest('tbeam','2.1.0.abcdef0','esp32');
 const result=await enrichMeshtasticReleases(single,releases,{fetchImpl:async()=>Response.json(good)});
 assert.equal(result[0].assets[0].chip,'esp32');assert.equal(result[0].assets[0].boardTarget,'tbeam');
 for(const patch of [{mcu:'esp32s3'},{platformioTarget:'tbeam-s3-core'}])await assert.rejects(enrichMeshtasticReleases(single,releases,{fetchImpl:async()=>Response.json({...good,...patch})}),/identity mismatch/);
});
test('Meshtastic sync is stable within a day and a failed target fetch preserves the previous release',async()=>{
 const list=JSON.parse(await readFile(new URL('../web/platform/catalog/firmware-watchlist.json',import.meta.url),'utf8'));
 list.entries=list.entries.filter(x=>x.id==='meshtastic');list.entries[0].catalogTargets=list.entries[0].catalogTargets.slice(0,1);
 const fake=async(url,options)=>{
  if(url.startsWith('https://api.github.com/')){assert.equal(options.headers.Authorization,'Bearer test-only');return Response.json(releases);}
  assert.equal(options.headers.Authorization,undefined);return Response.json(manifest('m5stack-unitc6l'));
 };
 const first=await syncReleaseMetadata(list,{fetchImpl:fake,token:'test-only',now:'2026-10-27T01:00:00Z'});
 assert.equal(first.entries[0].error,null);assert.equal(first.entries[0].release.assets[0].boardTarget,'m5stack-unitc6l');
 const second=await syncReleaseMetadata(list,{previous:first,fetchImpl:fake,token:'test-only',now:'2026-10-27T02:00:00Z'});assert.deepEqual(second,first);
 const failed=await syncReleaseMetadata(list,{previous:first,token:'test-only',now:'2026-10-27T03:00:00Z',fetchImpl:async(url,options)=>url.startsWith('https://api.github.com/')?fake(url,options):new Response(null,{status:503})});
 assert.ok(failed.entries[0].error);assert.deepEqual(failed.entries[0].releases,first.entries[0].releases);
});
test('actual dual-role catalog, stable/alpha targets and missing-SHA update policy remain distinct',async()=>{
 const read=async name=>JSON.parse(await readFile(new URL(`../web/platform/catalog/${name}.json`,import.meta.url),'utf8'));
 const [products,accessories,firmwares,watchlist,snapshot]=await Promise.all(['products','accessories','firmwares','firmware-watchlist','firmware-release-snapshot'].map(read));
 const product=products.items.find(x=>x.id==='m5stack-u202'),accessory=accessories.items.find(x=>x.id==='m5stack-unit-c6l-u202');
 assert.equal(product.knowledgeUrl,accessory.knowledgeUrl);assert.equal(product.model.url,accessory.visual.modelUrl);assert.equal(product.catalogRoles.accessoryId,accessory.id);assert.equal(accessory.catalogRoles.productId,product.id);
 const mesh=watchlist.entries.find(x=>x.id==='meshtastic');assert.equal(mesh.catalogTargets.filter(t=>t.productIds.every(id=>id.startsWith('m5stack-'))).length,4);
 assert.deepEqual(mesh.catalogTargets.filter(t=>t.productIds.some(id=>id.startsWith('lilygo-'))).map(t=>[t.boardTarget,t.targetChip]),[['t-deck','esp32s3'],['tbeam','esp32'],['t-watch-s3','esp32s3']]);
 const runnableTrials=new Set(['lilygo-t-deck','lilygo-t-beam-v1-2']);
 for(const t of mesh.catalogTargets){
  const fw=firmwares.items.find(x=>x.id===t.firmwareId),product=products.items.find(p=>p.id===t.productIds[0]);
  assert.match(fw.source.sha256,/^[a-f0-9]{64}$/);assert.ok(fw.source.url.includes(t.boardTarget));
  // Board adaptation permits an attempt; factory checks do not verify Meshtastic.
  assert.equal(fw.status,'trial');assert.equal(fw.simulationVerified,false);
  const policy=firmwareRunPolicy(product,fw),canRun=runnableTrials.has(product.id);
  assert.equal(policy.canRun,canRun,product.id);assert.equal(policy.trial,true);
  assert.equal(policy.reason,canRun?null:'board-unavailable');
 }
 const saved=snapshot.entries.find(x=>x.id==='meshtastic'),old=firmwares.items.find(x=>x.id==='m5stack-u202-meshtastic-stable');
 const newAsset={...saved.releases.flatMap(r=>r.assets).find(a=>a.boardTarget==='m5stack-unitc6l'),name:'firmware-m5stack-unitc6l-99.0.0.abcdef0.factory.bin',digest:null};
 saved.releases.unshift({...releases[0],tag_name:'v99.0.0.abcdef0',prerelease:false,published_at:'2099-01-01',assets:[newAsset]});
 const result=updateFirmwareCatalog({products,firmwares,watchlist,snapshot});assert.deepEqual(result.firmwares.items.find(x=>x.id===old.id),old);assert.equal(result.report.entries.find(x=>x.firmwareId===old.id).reason,'candidate-missing-sha256');
});
