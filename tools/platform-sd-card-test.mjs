import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {createFat32Image,validateSdImage} from '../web/platform/sd-card-image.js';
import {createSdCardStorage} from '../web/platform/sd-card-storage.js';
import {createDeviceStorage,deviceStorageScope} from '../web/platform/device-storage.js';
import {persistentBoard,firmwareBoardMatches} from '../web/platform/board-compatibility.js';
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
  if(m.op==='load')response={loaded:m.at===undefined?m.kind:`at${m.at}`,ok:true};
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
 if(window==='read')sd.read=async(...args)=>{const result=await originalRead(...args);entered.resolve();await release.promise;return result;};
 else crypto.subtle.digest=async function(...args){if(args[1].byteLength===image.length){entered.resolve();await release.promise;}return originalDigest.apply(this,args);};
 const {runtime,counts,load}=structuralRuntime(sd);
 try{
  await runtime.configure(createDefaultScene(catalog,productId));const pending=load();await entered.promise;await other.replace(key);release.resolve();
  await assert.rejects(pending,{code:'STORAGE_CONFLICT'});
  const state=await runtime.observe();assert.equal(state.status,'error');assert.equal(state.sdCard.status,'conflict');assert.equal(counts.attached,0);
  assert.equal((await other.read(key)).row.media,null);
 }finally{release.resolve();crypto.subtle.digest=originalDigest;await runtime.dispose();other.close();}
});
