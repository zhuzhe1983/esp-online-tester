import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {createFat32Image,validateSdImage} from '../web/platform/sd-card-image.js';
import {createSdCardStorage} from '../web/platform/sd-card-storage.js';
import {createDeviceStorage,deviceStorageScope} from '../web/platform/device-storage.js';
import {persistentBoard,firmwareBoardMatches,hasSdCard} from '../web/platform/board-compatibility.js';
test('FAT32 default image has consistent partition, backup boot, FSInfo, FAT copies and free-root accounting',()=>{
 const data=createFat32Image(),v=new DataView(data.buffer),start=v.getUint32(454,true),size=v.getUint32(458,true),at=start*512;
 assert.equal(start+size,data.length/512);assert.equal(v.getUint16(510,true),0xaa55);assert.equal(data[450],0x0c);
 const reserved=v.getUint16(at+14,true),fat=v.getUint32(at+36,true),clusters=(size-reserved-2*fat)/data[at+13];
 assert.ok(clusters>=65525);assert.ok(fat*512>=(clusters+2)*4);assert.equal(v.getUint32(at+512+488,true),clusters-1);
 assert.deepEqual(data.subarray(at,at+512),data.subarray(at+3072,at+3584));
 const f=(start+reserved)*512;assert.deepEqual(data.subarray(f,f+fat*512),data.subarray(f+fat*512,f+fat*1024));assert.equal(v.getUint32(f+8,true),0x0fffffff);
 assert.equal(validateSdImage(data),data);assert.throws(()=>validateSdImage(new Uint8Array(513)),{code:'INVALID_SD_IMAGE'});
});
test('SD base images and dirty pages survive reload, isolate product/firmware, and reject stale writers after clear',async()=>{
 const options={indexedDB:null,BroadcastChannel:null,dbName:'sd-test-'+Date.now()},a=createSdCardStorage(options),b=createSdCardStorage(options),hash='a'.repeat(64),key='v1:papermono:papermono:'+hash;
 try{
  const initial=await a.read(key);assert.equal(initial.image,null);
  const image=new Uint8Array(1024*1024);image[123]=42;
  await a.replace(key,{image:image.buffer,media:{format:'image',bytes:image.length,sha256:hash}});
  const opened=await b.read(key);assert.equal(new Uint8Array(opened.image)[123],42);
  const page=new Uint8Array(4096);page[0]=97;
  const saved=await b.save(key,{epoch:opened.row.epoch,revision:opened.row.revision,pages:[{offset:4096,data:page.buffer}]});
  const loaded=await a.read(key);assert.equal(new Uint8Array(loaded.row.pages[0].data)[0],97);assert.equal(new Uint8Array(loaded.image)[123],42);
  assert.equal((await a.read('v1:papermono-lite:papermono:'+hash)).image,null);assert.equal((await a.read('v1:papermono:papermono:'+'b'.repeat(64))).image,null);
  await a.replace(key);await assert.rejects(()=>b.save(key,{epoch:saved.row.epoch,revision:saved.row.revision,pages:[]}),{code:'STORAGE_CONFLICT'});
  const blank=await b.read(key);assert.equal(blank.image,null);assert.deepEqual(blank.row.pages,[]);
 }finally{a.close();b.close();}
});
test('disposing while IndexedDB open is pending closes the late connection and creates no channel',async()=>{
 let request,closes=0,channels=0;
 const indexedDB={open(){request={};return request;}},BroadcastChannel=class{constructor(){channels++;}close(){}};
 const storage=createSdCardStorage({indexedDB,BroadcastChannel,dbName:'sd-close-race-'+Date.now()});
 const pending=storage.read('v1:papermono:papermono:'+'c'.repeat(64));storage.close();
 request.result={close(){closes++;}};request.onsuccess();await pending;
 assert.equal(closes,1);assert.equal(channels,0);
});

const catalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async type=>[type,JSON.parse(await readFile(new URL(`../web/platform/catalog/${type}.json`,import.meta.url))).items])));
const productId='m5stack-papermono',digest=bytes=>createHash('sha256').update(bytes).digest('hex');
const defer=()=>{let resolve;const promise=new Promise(r=>resolve=r);return {promise,resolve};};
// Structural worker: no CPU or firmware is executed by these lifecycle tests.
function structuralRuntime(sdStorage,deviceStorage){
 const counts={created:0,terminated:0,attached:0};
 const workerFactory=()=>{counts.created++;const worker={terminate(){counts.terminated++;},postMessage(m){let response;
  if(m.op==='init')response={ready:true};if(m.op==='create')response={created:true,flashStorage:true};
  if(m.op==='imu')response={imuApplied:m.inputId,ok:true,time:0};if(m.op==='load')response={loaded:m.at===undefined?m.kind:`at${m.at}`,ok:true};
  if(m.op==='flash-snapshot')response={flashReply:m.requestId,data:new ArrayBuffer(16*1024*1024)};
  if(m.op==='flash-watch'||m.op==='flash-flush'||m.op==='flash-restore')response={flashReply:m.requestId,ok:true};
  if(m.op.startsWith('sd-'))response={sdReply:m.requestId,ok:true};if(m.op==='sd-attach')counts.attached++;
  if(m.op==='stop')response={stopped:0};if(response)queueMicrotask(()=>worker.onmessage?.({data:response}));
 }};return worker;};
 const runtime=createPlatformRuntime({catalog,workerFactory,sdStorage,deviceStorage,fetchImpl:async()=>new Response(new Uint8Array([1,2,3])),baseUrl:'https://example.invalid/platform/',assetBaseUrl:'https://example.invalid/'});
 const header=new Uint8Array(0x10100),v=new DataView(header.buffer);
 for(const at of [0,0x10000]){header[at]=0xe9;header[at+1]=1;header[at+12]=9;v.setUint32(at+28,4,true);}
 header[0x8000]=0xaa;header[0x8001]=0x50;v.setUint32(0x8004,0x10000,true);v.setUint32(0x8008,0x100000,true);
 return {runtime,counts,load:()=>runtime.loadFirmware({file:new Blob([header])})};
}
test('Lite execution split retains legacy Flash and SD scopes, product isolation and manual clear',async()=>{
 const options={indexedDB:null,BroadcastChannel:null,dbName:'paper-variant-'+Date.now()},sd=createSdCardStorage(options),flash=createDeviceStorage(options),hash=digest(new Uint8Array(16*1024*1024));
 const lite='m5stack-c153-lite',old=deviceStorageScope({productId:lite,board:'papermono',chip:'esp32s3',flashBytes:16*1024*1024,firmwareSha256:hash}),sdKey=`v1:${lite}:papermono:${hash}`;
 const page=new Uint8Array(4096);page[42]=71;await flash.save(old,{epoch:0,revision:0,pages:[{offset:0x200000,data:page.buffer}]});
 const image=new Uint8Array(1024*1024);image[42]=83;await sd.replace(sdKey,{image:image.buffer,media:{format:'image',bytes:image.length,sha256:digest(image)}});
 const {runtime,load}=structuralRuntime(sd,flash);
 try{
  assert.equal(catalog.products.find(p=>p.id===lite).runtime.board,'papermono-lite');
  await runtime.configure(createDefaultScene(catalog,lite));assert.equal((await runtime.capabilities()).sdCard.available,true);await load();
  const state=await runtime.observe();assert.equal(state.storage.scope.key,old.key);assert.equal(state.storage.restored,true);assert.equal(state.sdCard.restored,true);assert.equal(new Uint8Array((await sd.read(sdKey)).image)[42],83);
  await runtime.clearSdCard();assert.equal((await sd.read(sdKey)).image,null);
  // A Flash clear uses the same compatibility scope and must fence stale writers.
  await runtime.clearDeviceStorage();assert.equal((await flash.read(old)).row.pages.length,0);await assert.rejects(()=>flash.save(old,{epoch:0,revision:1,pages:[]}),{code:'STORAGE_CONFLICT'});
  await runtime.configure(createDefaultScene(catalog,productId));await load();const full=await runtime.observe();assert.notEqual(full.storage.scope.key,old.key);assert.equal(full.storage.restored,false);assert.equal(full.sdCard.restored,false);
  assert.equal(persistentBoard('another-product','papermono-lite'),'papermono-lite');assert.equal(firmwareBoardMatches('papermono','papermono-lite'),true);assert.equal(firmwareBoardMatches('stopwatch','papermono-lite'),false);
 }finally{await runtime.dispose();}
});
test('pending SD replacement cannot resurrect a disposed runtime',async()=>{
 const sd=createSdCardStorage({indexedDB:null,BroadcastChannel:null,dbName:'sd-replace-dispose-'+Date.now()}),entered=defer(),release=defer(),replace=sd.replace;
 sd.replace=async(...args)=>{entered.resolve();await release.promise;return replace(...args);};
 const {runtime,counts,load}=structuralRuntime(sd);
 try{
  await runtime.configure(createDefaultScene(catalog,productId));await load();
  const pending=runtime.clearSdCard();await entered.promise;await runtime.dispose();release.resolve();
  await assert.rejects(pending,{code:'DISPOSED'});
  assert.equal((await runtime.observe()).status,'disposed');assert.deepEqual(counts,{created:1,terminated:1,attached:1});
 }finally{release.resolve();await runtime.dispose();}
});
for(const window of ['read','checksum'])test(`another tab clearing SD during ${window} rejects the stale pending mount`,async()=>{
 const options={indexedDB:null,BroadcastChannel:null,dbName:'sd-mount-clear-'+window+'-'+Date.now()},sd=createSdCardStorage(options),other=createSdCardStorage(options);
 const image=new Uint8Array(1024*1024);image[123]=42;
 const key=`v1:${productId}:papermono:${digest(new Uint8Array(16*1024*1024))}`;
 await sd.replace(key,{image:image.buffer,media:{format:'image',bytes:image.length,sha256:digest(image)}});
 const entered=defer(),release=defer(),originalRead=sd.read,originalDigest=crypto.subtle.digest;
 if(window==='read')sd.read=async(...args)=>{const result=await originalRead(...args);if(args[0]===key){entered.resolve();await release.promise;}return result;};
 else crypto.subtle.digest=async function(...args){if(args[1].byteLength===image.length){entered.resolve();await release.promise;}return originalDigest.apply(this,args);};
 const {runtime,counts,load}=structuralRuntime(sd);
 try{
  await runtime.configure(createDefaultScene(catalog,productId));const pending=load();await entered.promise;await other.replace(key);release.resolve();
  await assert.rejects(pending,{code:'STORAGE_CONFLICT'});
  const state=await runtime.observe();assert.equal(state.status,'error');assert.equal(state.sdCard.status,'conflict');assert.equal(counts.attached,0);
  assert.equal((await other.read(key)).row.media,null);
 }finally{release.resolve();crypto.subtle.digest=originalDigest;await runtime.dispose();other.close();}
});
for (const [device,board] of [['m5stack-c139','papers3'],['m5stack-k159-b','paperdiy'],['metalio-ink4','metalio-ink4']]) test(`${board} TF shares controls but isolates media from PaperMono and preserves Flash on explicit clear`,async()=>{
 const options={indexedDB:null,BroadcastChannel:null,dbName:board+'-tf-'+Date.now()},sd=createSdCardStorage(options),flash=createDeviceStorage(options);
 const {runtime,counts,load}=structuralRuntime(sd,flash);
 try {
  await runtime.configure(createDefaultScene(catalog,device));assert.equal((await runtime.capabilities()).sdCard.available,true);await load();
  const state=await runtime.observe();assert.equal(state.sdCard.supported,true);assert.equal(counts.attached,1);
  const hash=state.storage.scope.firmwareSha256,key=`v1:${device}:${board}:${hash}`;
  const image=new Uint8Array(1024*1024);image[123]=42;await runtime.attachSdCard({bytes:image});assert.equal((await sd.read(key)).image.byteLength,image.length);
  await runtime.configure(createDefaultScene(catalog,productId));await load();assert.equal((await runtime.observe()).sdCard.restored,false);
  await runtime.configure(createDefaultScene(catalog,device));await load();assert.equal((await runtime.observe()).sdCard.restored,true);
  const before=(await flash.read(state.storage.scope)).row;await runtime.clearSdCard();assert.deepEqual((await flash.read(state.storage.scope)).row,before);assert.equal((await sd.read(key)).image,null);assert.equal((await runtime.observe()).status,'ready');
  assert.equal((await sd.read(`v1:${device}:${board}:${'b'.repeat(64)}`)).image,null);
  await runtime.configure(createDefaultScene(catalog,'m5stack-stopwatch'));assert.equal((await runtime.capabilities()).sdCard.available,false);
 } finally {await runtime.dispose();}
});
test('INK4 and INK4 Plus enable only their individually implemented SD board routes',()=>{
 assert.equal(hasSdCard('metalio-ink4'),true);assert.equal(hasSdCard('metalio-ink4-plus'),true);assert.equal(hasSdCard('metalio-ink4-s31'),false);
});

test('all declared TF slots prepare a card before firmware load without claiming an unimplemented transport',async()=>{
 const {hasTfSlot}=await import('../web/platform/tf-card-capability.js');
 const sd=createSdCardStorage({indexedDB:null,BroadcastChannel:null,dbName:'all-tf-slots-'+Date.now()}),{runtime,counts,load}=structuralRuntime(sd);
 try{
  const devices=catalog.products.filter(hasTfSlot);assert.ok(devices.length>=33);
  for(const product of devices){await runtime.configure(createDefaultScene(catalog,product.id));const state=await runtime.observe(),cap=(await runtime.capabilities()).sdCard;assert.equal(state.sdCard.slot,true);assert.equal(state.sdCard.present,true);assert.equal(state.sdCard.mounted,false);assert.equal(state.sdCard.bytes,64*1024*1024);assert.equal(state.sdCard.format,'fat32');assert.equal(state.sdCard.supported,hasSdCard(product.runtime?.board));assert.equal(cap.defaultInserted,true);assert.deepEqual(cap.formats,['fat32','exfat','raw-image']);}
  assert.equal(counts.created,0,'Preparing local cards does not start any firmware');
  await runtime.configure(createDefaultScene(catalog,'m5stack-k128'));await assert.rejects(load,{code:'UNSUPPORTED_PRODUCT'});assert.equal((await runtime.observe()).sdCard.present,true,'A failed firmware load does not hide the prepared card');assert.equal((await runtime.exportSdCard()).bytes.length,64*1024*1024);
  await runtime.configure(createDefaultScene(catalog,'m5stack-stopwatch'));assert.equal((await runtime.observe()).sdCard.present,false);await assert.rejects(()=>runtime.attachSdCard(),{code:'SD_UNSUPPORTED'});
 }finally{await runtime.dispose();}
});
test('prepared imported media is product-isolated, seeds only fresh firmware cards and never reverses an explicit clear',async()=>{
 const {createExfatImage,sdImageFormat}=await import('../web/platform/sd-card-image.js');
 const sd=createSdCardStorage({indexedDB:null,BroadcastChannel:null,dbName:'prepared-tf-'+Date.now()}),{runtime,load}=structuralRuntime(sd);
 try{
  await runtime.configure(createDefaultScene(catalog,productId));const image=createExfatImage();await runtime.attachSdCard({bytes:image});assert.equal((await runtime.observe()).sdCard.format,'exfat');assert.equal(digest((await runtime.exportSdCard()).bytes),digest(image));
  await runtime.configure(createDefaultScene(catalog,'m5stack-k128'));let state=await runtime.observe();assert.equal(state.sdCard.supported,false);assert.equal(state.sdCard.slot,true);assert.equal(state.sdCard.format,'fat32');
  await runtime.attachSdCard({format:'exfat',byteLength:128*1024*1024});state=await runtime.observe();assert.equal(state.sdCard.bytes,128*1024*1024);assert.equal(state.sdCard.mounted,false);assert.equal(sdImageFormat((await runtime.exportSdCard()).bytes),'exfat');
  await runtime.configure(createDefaultScene(catalog,productId));assert.equal((await runtime.observe()).sdCard.bytes,image.length);await load();state=await runtime.observe();assert.equal(state.sdCard.mounted,true);assert.equal(state.sdCard.format,'exfat');
  const key=`v1:${productId}:papermono:${state.storage.scope.firmwareSha256}`;assert.equal(digest(new Uint8Array((await sd.read(key)).image)),digest(image));
  await runtime.clearSdCard();assert.equal((await runtime.observe()).sdCard.format,'fat32');await runtime.configure(createDefaultScene(catalog,productId));assert.equal((await runtime.observe()).sdCard.format,'exfat','Prepared image remains available to genuinely new firmware');await load();assert.equal((await runtime.observe()).sdCard.format,'fat32','Existing explicit empty-card choice wins over the prepared image');
 }finally{await runtime.dispose();}
});
test('prepared card changes in another tab fence export until the device is selected again',async()=>{
 const {preparedTfKey}=await import('../web/platform/tf-card-capability.js'),options={indexedDB:null,BroadcastChannel:null,dbName:'prepared-conflict-'+Date.now()},sd=createSdCardStorage(options),other=createSdCardStorage(options),{runtime}=structuralRuntime(sd);
 try{
  await runtime.configure(createDefaultScene(catalog,'m5stack-k128'));await other.replace(preparedTfKey(catalog.products.find(p=>p.id==='m5stack-k128')));assert.equal((await runtime.observe()).sdCard.status,'conflict');await assert.rejects(()=>runtime.exportSdCard(),{code:'SD_NOT_LOADED'});
  await runtime.configure(createDefaultScene(catalog,'m5stack-k128'));assert.equal((await runtime.observe()).sdCard.present,true);assert.equal((await runtime.exportSdCard()).bytes.length,64*1024*1024);
 }finally{await runtime.dispose();other.close();}
});
test('initial card seeding checks epoch/revision instead of overwriting another tab',async()=>{
 const sd=createSdCardStorage({indexedDB:null,BroadcastChannel:null,dbName:'sd-seed-'+Date.now()}),key='v1:paper:board:'+'a'.repeat(64);
 try{await sd.replace(key);await assert.rejects(()=>sd.replace(key,{expected:{epoch:0,revision:0},image:new ArrayBuffer(1024*1024),media:{format:'image'}}),{code:'STORAGE_CONFLICT'});assert.equal((await sd.read(key)).image,null);}finally{sd.close();}
});

test('prepared-card read races reject stale media instead of hiding another tab replacement',async()=>{
 const {preparedTfKey}=await import('../web/platform/tf-card-capability.js'),options={indexedDB:null,BroadcastChannel:null,dbName:'prepared-read-race-'+Date.now()},sd=createSdCardStorage(options),other=createSdCardStorage(options),{runtime}=structuralRuntime(sd),entered=defer(),release=defer();
 const key=preparedTfKey(catalog.products.find(p=>p.id==='m5stack-k128')),read=sd.read;
 sd.read=async value=>{const result=await read(value);if(value===key){entered.resolve();await release.promise;}return result;};
 try{const pending=runtime.configure(createDefaultScene(catalog,'m5stack-k128'));await entered.promise;await other.replace(key);release.resolve();await assert.rejects(pending,{code:'STORAGE_CONFLICT'});assert.equal((await runtime.observe()).sdCard.status,'conflict');}finally{release.resolve();await runtime.dispose();other.close();}
});

test('stored base-image corruption is rejected before copying or formatting it',async()=>{
 const sd=createSdCardStorage({indexedDB:null,BroadcastChannel:null,dbName:'sd-corrupt-image-'+Date.now()}),key='v1:paper:board:'+'b'.repeat(64);
 try{await sd.replace(key,{image:new ArrayBuffer(513),media:{format:'image',bytes:513,sha256:'b'.repeat(64)}});await assert.rejects(()=>sd.read(key),{code:'STORAGE_CORRUPT'});}finally{sd.close();}
});
