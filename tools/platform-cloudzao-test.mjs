import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {releaseRequest,publicReleases,publicRelease} from './firmware-release-source.mjs';
import {syncReleaseMetadata} from './sync-firmware-release-metadata.mjs';
import {updateFirmwareCatalog} from './sync-firmware-catalog.mjs';
import {validateWatchlist} from '../web/platform/watchlist.js';
const read=async name=>JSON.parse(await readFile(new URL(`../web/platform/catalog/${name}.json`,import.meta.url),'utf8'));
const catalog=await read('firmware-watchlist'),entries=catalog.entries.filter(e=>e.metadataProvider==='cloudzao');
const list={...catalog,entries},entry=entries[0],url='https://cloudzaoai.oss-rg-china-mainland.aliyuncs.com/OTA/xiaozhi/00000000-0000-4000-8000-000000000001.bin';
function fixture(e=entry){return {code:0,data:{total:1,list:[{id:e.firmwareId,hardwareModelId:e.hardwareModelId,chipName:e.targetChip,sourceType:'official',status:1,ownerUserId:'omit-me',authorAvatar:'omit-me',versions:[{version:'3.0.0',versionStatus:'test',status:1,deleteStatus:1,createDate:'2026-10-27T20:00:00+08:00',firmwareUrl:url,size:'102400',fileSuffix:'bin',flashStartAddress:null},{version:'2.9.0',versionStatus:'latest',status:1,deleteStatus:1,createDate:'2026-10-26T20:00:00+08:00',firmwareUrl:url,size:'102400',fileSuffix:'bin',flashStartAddress:null}]}]}};}
test('CloudZao stable/test numeric versions remain separated and no owner fields escape',()=>{
 validateWatchlist(list);const d=fixture(),releases=publicReleases(entry,d);assert.equal(publicRelease(entry,d).tag_name,'2.9.0');assert.equal(publicRelease({...entry,releaseChannel:'prerelease'},d).tag_name,'3.0.0');assert.equal(releases[0].assets[0].digest,null);assert.equal(JSON.stringify(releases).includes('omit-me'),false);assert.equal(releases[0].published_at,'2026-10-27T12:00:00.000Z');
});
test('CloudZao requires exact hardware, chip, firmware identity and official source',()=>{
 for(const [key,value]of [['id','other'],['hardwareModelId','44'],['chipName','esp32s31'],['sourceType','community'],['status',0]]){const d=fixture();d.data.list[0][key]=value;assert.throws(()=>publicReleases(entry,d));}
 const duplicate=fixture();duplicate.data.list.push(duplicate.data.list[0]);assert.throws(()=>publicReleases(entry,duplicate));
 for(const key of ['metadataUrl','hardwareModelId','targetChip'])assert.throws(()=>releaseRequest({...entry,[key]:'invalid'},'secret'));
});
test('CloudZao rejects foreign artifact hosts, nonzero flash offsets and duplicate versions',()=>{
 for(const change of [{firmwareUrl:'https://evil.example/image.bin'},{firmwareUrl:url+'?private=secret'},{flashStartAddress:'0x10000'},{fileSuffix:'zip'},{size:'999999999999'},{createDate:'bad'}]){const d=fixture();Object.assign(d.data.list[0].versions[0],change);assert.throws(()=>publicReleases(entry,d));}
 const d=fixture();d.data.list[0].versions.push(d.data.list[0].versions[0]);assert.throws(()=>publicReleases(entry,d));
});
test('disabled/deleted/unknown-channel versions are not published; no GitHub token is sent',async()=>{
 const d=fixture();for(const change of [{status:0},{deleteStatus:0},{versionStatus:'draft'}])d.data.list[0].versions.push({...d.data.list[0].versions[0],...change,version:'99.0.0'});
 assert.equal(publicReleases(entry,d).length,2);const single={...list,entries:[entry]};let calls=0;
 const result=await syncReleaseMetadata(single,{token:'secret',fetchImpl:async(u,o)=>{calls++;assert.equal(u,entry.metadataUrl);assert.equal(o.headers.Authorization,undefined);return Response.json(d);}});assert.equal(calls,1);assert.equal(result.entries[0].error,null);assert.equal(JSON.stringify(result).includes('omit-me'),false);
});
test('new CloudZao version without SHA retains audited image, including unsupported boards',async()=>{
 const snapshot={entries:entries.map(e=>({id:e.id,upstreamRepo:e.upstreamRepo,releases:publicReleases(e,fixture(e)),error:null}))},firmwares=await read('firmwares');
 const result=updateFirmwareCatalog({products:await read('products'),firmwares,watchlist:list,snapshot});assert.deepEqual(result.firmwares,firmwares);assert.ok(result.report.entries.every(e=>e.reason==='candidate-missing-sha256'));
 const products=(await read('products')).items;for(const e of entries){const p=products.find(p=>e.compatibility.productIds.includes(p.id));assert.equal(p.runtime.board,p.id);assert.equal(p.runtime.status,'partial');assert.equal(p.chip,e.targetChip);assert.equal(firmwares.items.find(f=>f.id===p.defaultFirmwareId).publisherGroup,'官方');}
});

test('manufacturer aliases are searchable and official stable is selected first',async()=>{
 const {searchProducts,firstFirmware}=await import('../web/platform/catalog-selection.js');const products=(await read('products')).items,firmwares=(await read('firmwares')).items;
 const ids=['cloudzao-claw-154','metalio-ink4-plus','metalio-ink4','cloudzao-claw4'];
 for(const brand of ['CloudZao','Metalio','无名科技'])assert.deepEqual(searchProducts(products,brand).map(p=>p.id),ids);
 for(const id of ids){const p=products.find(p=>p.id===id);assert.equal(p.manufacturer,'CloudZao');assert.equal(firstFirmware(firmwares,p).id,p.defaultFirmwareId);assert.equal(firstFirmware(firmwares,p).role,'official');}
});

test('legacy INK scenes retain exact product IDs and audited official sources after rebranding',async()=>{
 const {createDefaultScene,encodeScene,decodeScene,validateScene}=await import('../web/platform/catalog.js');
 const catalog={products:(await read('products')).items,accessories:(await read('accessories')).items,firmwares:(await read('firmwares')).items};
 const hashes={'metalio-ink4':'1d02cba522fe1aec5b2c160973c6a15d89cbf0e13a5794a42faf6e3f15587919','metalio-ink4-plus':'56786c7407405993fe1b049446d8535f4cc7e72c99953fff5f2662c7aa2cfbc0'};
 for(const [id,sha256]of Object.entries(hashes)){
  const product=catalog.products.find(p=>p.id===id),firmware=catalog.firmwares.find(f=>f.id===product.defaultFirmwareId);
  assert.equal(product.runtime.board,id);assert.equal(firmware.source.sha256,sha256);
  assert.equal(product.model.url,`./assets/${id}/model.json`);
  const scene=createDefaultScene(catalog,id);scene.firmware=firmware.source;
  const restored=decodeScene(encodeScene(scene));assert.deepEqual(restored,scene);assert.equal(validateScene(restored,catalog).valid,true);
 }
});

test('CloudZao chronology is a labelled repository proxy and does not mutate source order',async()=>{
 const {productsForManufacturer,productChronologyLabel}=await import('../web/platform/catalog-selection.js');
 const products=(await read('products')).items,original=products.map(p=>p.id);
 const queue=productsForManufacturer(products,'CloudZao');
 assert.deepEqual(queue.map(p=>p.id),['cloudzao-claw-154','metalio-ink4-plus','metalio-ink4','cloudzao-claw4']);
 assert.deepEqual(queue.map(p=>p.chronology.date),['2026-10-10','2026-09-30','2026-09-14','2026-07-01']);
 for(const p of queue){assert.equal(p.chronology.basis,'repository-first-seen');assert.equal(p.chronology.confidence,'low');assert.match(productChronologyLabel(p),/代用/);assert(p.sources.some(s=>s.id===p.chronology.sourceId));}
 assert.deepEqual(products.map(p=>p.id),original);
});

test('Claw releases are discovery-only and never replace independently audited defaults',async()=>{
 const watch=await read('firmware-watchlist'),firmwares=await read('firmwares'),products=await read('products'),snapshot=await read('firmware-release-snapshot');
 const selected=watch.entries.filter(e=>e.id.startsWith('cloudzao-claw'));
 assert.equal(selected.length,2);validateWatchlist({...watch,entries:selected});
 for(const e of selected){assert.deepEqual(e.catalogTargets,[]);assert(e.assets.every(a=>a.sourcePolicy==='mutable-pinned'&&a.sha256));}
 const beforeProducts=structuredClone(products);
 const result=updateFirmwareCatalog({products,firmwares,watchlist:{...watch,entries:selected},snapshot});
 assert.deepEqual(result.firmwares,firmwares);assert.deepEqual(products,beforeProducts);
 const audit=JSON.parse(await readFile(new URL('../docs/platform/evidence/cloudzao-20261123/firmware-memory-audit.json',import.meta.url),'utf8'));
 for(const report of audit.reports){
  const f=firmwares.items.find(f=>f.id===report.firmwareId);
  assert.equal(f.source.sha256,report.actualSha256);assert.equal(f.sourceIdentity.bytes,report.actualBytes);assert.equal(f.catalogUpdate.layoutVerified,true);
  assert.equal(f.simulationVerified,false);assert.equal(f.hardwareValidated,false);assert.equal(f.role,'official');assert.equal(f.storage,'remote-links-only');
 }
 const claw4=firmwares.items.find(f=>f.id==='cloudzao-claw4-official');
 assert.equal(claw4.version,'v2.0.5.1');assert.equal(claw4.sourceIdentity.imageAppVersion,'2.0.5');
});
