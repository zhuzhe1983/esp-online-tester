#!/usr/bin/env node
// Read a caller-selected private full image in RAM. Never publish it or its UART.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {basename} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createSdCardStorage} from '../web/platform/sd-card-storage.js';
import {createDefaultScene} from '../web/platform/catalog.js';
const args=process.argv.slice(2),arg=k=>args[args.indexOf(k)+1];assert(args.includes('--file')&&arg('--file'),'Required --file <complete PaperS3 image>');
const root=new URL('../',import.meta.url),hash=b=>createHash('sha256').update(b).digest('hex'),firmware=new Uint8Array(await readFile(arg('--file'))),catalog={};
for(const k of ['products','accessories','firmwares'])catalog[k]=JSON.parse(await readFile(new URL(`web/platform/catalog/${k}.json`,root))).items;
const fetchImpl=async value=>{const u=new URL(value);assert.equal(u.origin,'https://papers3-sd.invalid');try{return new Response(await readFile(new URL(u.pathname.endsWith('.elf')?'web/wasm/fw/'+basename(u.pathname):'web'+u.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root),thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(...a)=>thread.postMessage(...a),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl:'https://papers3-sd.invalid/platform/',assetBaseUrl:'https://papers3-sd.invalid/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'papers3-sd-flash'}),sdStorage:createSdCardStorage({indexedDB:null,BroadcastChannel:null,dbName:'papers3-sd-card'})});
// FAT32 chain inspection; receipts omit all private names and file contents.
function files(bytes){const v=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength),u16=a=>v.getUint16(a,true),u32=a=>v.getUint32(a,true),boot=u32(454)*512,sector=u16(boot+11),spc=bytes[boot+13],fat=boot+u16(boot+14)*sector,data=fat+bytes[boot+16]*u32(boot+36)*sector;assert.equal(sector,512);assert(spc>0);
 const chain=c=>{const seen=new Set(),chunks=[];while(c>=2&&c<0xffffff8){assert(!seen.has(c),'FAT loop');seen.add(c);const at=data+(c-2)*spc*sector;assert(at>=0&&at+spc*sector<=bytes.length);chunks.push(bytes.slice(at,at+spc*sector));c=u32(fat+c*4)&0xfffffff;}return Buffer.concat(chunks);};
 const scan=(c,depth=0)=>{assert(depth<16);const d=chain(c),entries=[];for(let a=0;a<d.length;a+=32){if(!d[a])break;if(d[a]===229||d[a]===46||d[a+11]&8)continue;const cl=d.readUInt16LE(a+26)+(d.readUInt16LE(a+20)<<16),n=d.readUInt32LE(a+28);if(d[a+11]&16)entries.push(...scan(cl,depth+1));else{const content=chain(cl);assert(content.length>=n);entries.push({bytes:n,clusters:content.length/(spc*sector),sha256:hash(content.subarray(0,n))});}}return entries;};return scan(u32(boot+44));}
const receipt={checkedAt:new Date().toISOString(),firmware:{bytes:firmware.length,sha256:hash(firmware),modified:false,private:true},backend:'actual-page-worker-WASM',wasmSha256:hash(await readFile(new URL('web/wasm/esp32sim.wasm',root))),checks:[]};
const wait=async()=>{const end=Date.now()+180000;while(Date.now()<end){const s=await runtime.observe();if(s.status==='error')throw Error(s.lastError?.code);if(s.serial.includes('[SD] SD card detected')&&s.frame&&s.stats.time>=8)return s;await new Promise(r=>setTimeout(r,100));}throw Error('SD mount/frame timeout; private serial intentionally omitted');};
const load=()=>runtime.loadFirmware({file:new Blob([firmware])});
try{
 await runtime.configure(createDefaultScene(catalog,'m5stack-c139'));assert((await runtime.capabilities()).sdCard.available);await load();await runtime.start();await wait();await runtime.pause();await runtime.flushSdCard();
 const first=await runtime.observe(),image=(await runtime.exportSdCard()).bytes;receipt.first={sd:first.sdCard,frame:first.frame,files:files(image),sdSha256:hash(image)};assert(first.sdCard.pristine===false);assert(receipt.first.files.some(f=>f.bytes>512&&f.clusters>1),'Actual guest must commit a file across sectors');receipt.checks.push('original ROM boot mounts FAT32 and commits multi-cluster file');
 await runtime.reset();await wait();await runtime.pause();const reset=(await runtime.exportSdCard()).bytes;assert.deepEqual(files(reset),receipt.first.files);receipt.checks.push('reset preserves complete file bytes and FAT chain');
 await load();assert((await runtime.observe()).sdCard.restored);assert.equal(hash((await runtime.exportSdCard()).bytes),hash(reset));await runtime.start();await wait();await runtime.pause();assert.deepEqual(files((await runtime.exportSdCard()).bytes),receipt.first.files);receipt.checks.push('fresh worker reload restores saved card; original guest mounts and reads existing file');
 const scope=(await runtime.observe()).storage.scope;await runtime.attachSdCard({bytes:image});assert.equal(hash((await runtime.exportSdCard()).bytes),hash(image));assert.equal((await runtime.observe()).status,'ready');assert.deepEqual((await runtime.observe()).storage.scope,scope);receipt.checks.push('image import preserves bytes and Flash identity, leaves ready');
 await runtime.clearSdCard();assert.deepEqual(files((await runtime.exportSdCard()).bytes),[]);assert((await runtime.observe()).sdCard.pristine);assert.deepEqual((await runtime.observe()).storage.scope,scope);receipt.checks.push('explicit TF clear creates empty FAT32 and keeps Flash identity');receipt.status='pass';console.log(JSON.stringify(receipt,null,2));
}finally{await runtime.dispose();}
