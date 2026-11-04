// Parser/transport negative tests only. Execution is separately proved with a
// GCC/esptool-built image on the real worker/WASM, never this worker double.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {validateAppImage,chipIdForBoard} from '../web/platform/app-image.js';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
const hash=b=>createHash('sha256').update(b).digest('hex'),flash=4*1024*1024;
function image({segments=[{address:0x40800000,length:4}],entry=segments[0].address,chip=23,min=100,max=199,legacy=0,digest=true,descriptor=false}={}) {
 const end=24+segments.reduce((n,s)=>n+8+s.length,0),checksumAt=Math.floor(end/16)*16+15,b=new Uint8Array(checksumAt+1+(digest?32:0)),v=new DataView(b.buffer);
 b[0]=0xe9;b[1]=segments.length;v.setUint32(4,entry,true);v.setUint16(12,chip,true);b[14]=legacy;v.setUint16(15,min,true);v.setUint16(17,max,true);b[23]=digest?1:0;
 let at=24,x=0xef;for(const s of segments){v.setUint32(at,s.address,true);v.setUint32(at+4,s.length,true);at+=8;for(let i=0;i<s.length;i++)b[at+i]=(i*17+3)&255;if(descriptor&&at===32&&s.length>=4)v.setUint32(at,0xabcd5432,true);for(const n of b.subarray(at,at+s.length))x^=n;at+=s.length;}
 b[checksumAt]=x;if(digest)b.set(createHash('sha256').update(b.subarray(0,checksumAt+1)).digest(),checksumAt+1);return b;
}
const valid=(b,board='stamp-c5')=>validateAppImage(b,board,flash),reject=(b,code)=>assert.rejects(()=>valid(b),{code});
test('C5 bare-metal HP/LP SRAM images accept exact limits and optional descriptor without changing S3 rules',async()=>{
 for(const board of ['stamp-c5','stamp-c5-dip','esp32c5']){assert.equal(chipIdForBoard(board),23);const r=await valid(image(),board);assert.equal(r.applicationDescriptor,false);assert.equal(r.entry,0x40800000);}
 await valid(image({segments:[{address:0x4085fffc,length:4},{address:0x50003ffc,length:4}],entry:0x50003ffe,legacy:255,max:0}));
 await valid(image({segments:[{address:0x40800000,length:0x60000},{address:0x50000000,length:0x4000}],max:65535,digest:false}));
 assert.equal((await valid(image({segments:[{address:0x40800000,length:256}],descriptor:true}))).applicationDescriptor,true);
 await reject(image({descriptor:true}),'APP_IMAGE_INVALID');
 await assert.rejects(()=>validateAppImage(image({chip:9}),'stopwatch',flash),{code:'APP_IMAGE_INVALID'});
});
test('C5 rejects flash/ROM/MMIO, range overflow, empty/unaligned/overlapping segments and unloaded entries',async()=>{
 for(const address of [0x42000000,0x40000000,0x60000000,0x407ffffc,0x40860000,0x50004000,0xfffffffc])await reject(image({segments:[{address,length:4}]}),'C5_SRAM_REQUIRED');
 await reject(image({segments:[{address:0x4085fffc,length:8}]}),'C5_SRAM_REQUIRED');
 for(const s of [{address:0x40800001,length:4},{address:0x40800000,length:3},{address:0x40800000,length:0}])await reject(image({segments:[s]}),'C5_SRAM_SEGMENT_INVALID');
 await reject(image({segments:[{address:0x40800000,length:8},{address:0x40800004,length:4}]}),'C5_SRAM_OVERLAP');
 for(const entry of [0x40800001,0x40800004,0x50000000,0x42000000])await reject(image({entry}),'C5_SRAM_ENTRY_INVALID');
 for(const options of [{min:101},{max:99},{min:200,max:99}])await reject(image(options),'CHIP_REVISION_UNSUPPORTED');
 await reject(image({chip:13}),'CHIP_MISMATCH');
});
test('C5 verifies exact image framing, checksum, optional digest and erased-only staging trailer',async()=>{
 const good=image(),bad=good.slice();bad[32]^=1;await reject(bad,'APP_IMAGE_CHECKSUM');
 const digest=good.slice();digest[digest.length-1]^=1;await reject(digest,'APP_IMAGE_DIGEST');
 const padding=good.slice();padding[36]=1;await reject(padding,'APP_IMAGE_INVALID');
 for(const [offset,value] of [[0,0],[1,0],[1,17],[23,2]]){const b=good.slice();b[offset]=value;await reject(b,'APP_IMAGE_INVALID');}
 await reject(good.subarray(0,34),'APP_IMAGE_INVALID');await reject(good.subarray(0,good.length-1),'APP_IMAGE_INVALID');
 const trailing=new Uint8Array(good.length+16).fill(255);trailing.set(good);await valid(trailing);trailing[good.length]=0;await reject(trailing,'APP_IMAGE_INVALID');
 await assert.rejects(()=>validateAppImage(good,'stamp-c5',0x10000),{code:'FLASH_OVERFLOW'});
});
const catalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async key=>[key,JSON.parse(await readFile(new URL(`../web/platform/catalog/${key}.json`,import.meta.url))).items])));
for(const [id,board] of [['m5stack-s016','stamp-c5'],['m5stack-s016-dip','stamp-c5-dip']]){const p=catalog.products.find(p=>p.id===id);assert.ok(p);p.runtime={board,flashMb:4,psramMb:0,status:'partial'};}
function fixture(){
 const requests=[],messages=[],workers=[],resources=new Map();let sequence=0;
 const workerFactory=()=>{const w={terminate(){},postMessage(m){messages.push(m);let r;
  if(m.op==='init')r={ready:true};else if(m.op==='create')r={created:true,flashStorage:true,networkAvailable:true,audioAvailable:true};
  else if(m.op==='load')r={loaded:m.kind,ok:true};else if(m.op==='start')r={started:true};else if(m.op==='resume')r={resumed:true};else if(m.op==='stop')r={stopped:0};
  else throw Error('Unexpected C5 transport '+m.op);queueMicrotask(()=>w.onmessage?.({data:r}));}};workers.push(w);return w;};
 const runtime=createPlatformRuntime({catalog,workerFactory,baseUrl:'https://c5.invalid/platform/',assetBaseUrl:'https://c5.invalid/',fetchImpl:async url=>{requests.push(url);if(url.endsWith('esp32sim.wasm'))return new Response(new Uint8Array([0]));assert(resources.has(url),'unexpected fetch '+url);return new Response(resources.get(url));}});
 const remote=bytes=>{const url=`https://c5.invalid/app-${++sequence}.bin`;resources.set(url,bytes);return {url,sha256:hash(bytes)};};
 const manifest=m=>{const url=`https://c5.invalid/manifest-${++sequence}.json`;resources.set(url,JSON.stringify(m));return {manifestUrl:url};};
 return {runtime,requests,messages,workers,remote,manifest};
}
test('both C5 local upload paths use loadkind3 and app-direct reset with no ROM, storage, audio or network requests',async()=>{
 const f=fixture();try{for(const id of ['m5stack-s016','m5stack-s016-dip']){
  await f.runtime.configure(createDefaultScene(catalog,id));const caps=await f.runtime.capabilities();assert.equal(caps.execution.mode,'wasm-interpreter');assert.deepEqual(caps.execution.bootModes,['app-direct']);assert.deepEqual(caps.input.serialPorts,['uart0']);
  for(const x of [caps.network.available,caps.audio.input.available,caps.audio.output.available,caps.sdCard.available,caps.deviceStorage.available,caps.firmware.ota,caps.firmware.mergedImage,caps.firmware.localBuild])assert.equal(x,false);
  const loaded=await f.runtime.loadFirmware({file:new File([image()],'bare.bin'),...(id.endsWith('dip')?{bootMode:'app-direct'}:{})});assert.equal(loaded.firmware.source,'local');assert.equal(loaded.firmware.resetMode,'reload-same-sram-image');assert.equal(loaded.storage.status,'unsupported');assert.equal(loaded.audio.available,false);assert.equal(loaded.network.connection,'unsupported');
  await f.runtime.start();await f.runtime.pause();await f.runtime.start();await f.runtime.reset();await f.runtime.pause();
 }
 assert(f.messages.filter(x=>x.op==='load').every(x=>x.kind===3));assert(f.messages.filter(x=>x.op==='start').every(x=>x.appDirect));assert(f.messages.filter(x=>x.op==='create').every(x=>x.jit===false&&x.psram_mb===0&&x.flash_mb===4));assert(f.requests.every(x=>x.endsWith('esp32sim.wasm')));
 }finally{await f.runtime.dispose();}
});
test('C5 remote bare metal requires explicit mode and manifests cannot smuggle ROM/stubs/network or a second image',async()=>{
 const f=fixture();try{
  await f.runtime.configure(createDefaultScene(catalog,'m5stack-s016'));const source=f.remote(image());
  await assert.rejects(()=>f.runtime.loadFirmware(source),{code:'C5_APP_DIRECT_REQUIRED'});assert.equal(f.workers.length,0);
  await f.runtime.loadFirmware({...source,bootMode:'app-direct'});assert.equal((await f.runtime.observe()).scene.firmware.bootMode,'app-direct');
  await assert.rejects(()=>f.runtime.loadFirmware({...source,bootMode:'rom'}),{code:'C5_SRAM_REQUIRED'});
  for(const format of ['merged','segments','build'])await assert.rejects(()=>f.runtime.loadFirmware({format,file:new File([image()],'bare.bin')}),{code:'C5_SRAM_REQUIRED'});
  await assert.rejects(()=>f.runtime.loadFirmware({...source,bootMode:'app-direct',sha256:'0'.repeat(64)}),{code:'HASH_MISMATCH'});
  const plan={board:'stamp-c5',app_direct:true,files:{app:source}};await f.runtime.loadFirmware(f.manifest(plan));
  for(const patch of [{app_direct:false},{files:{...plan.files,rom:'rom.elf'}},{files:{app:[source,source]}},{flash_at:{'0x0':source}}])await assert.rejects(()=>f.runtime.loadFirmware(f.manifest({...plan,...patch})),{code:'C5_SRAM_REQUIRED'});
  for(const patch of [{wifi:'ssid,pw'},{stubs:['fake=return0']}])await assert.rejects(()=>f.runtime.loadFirmware(f.manifest({...plan,...patch})),{code:'C5_FEATURE_UNSUPPORTED'});
  const stock=f.remote(image({segments:[{address:0x42000000,length:4}]}));await assert.rejects(()=>f.runtime.loadFirmware({...stock,bootMode:'app-direct'}),{code:'C5_SRAM_REQUIRED'});
  assert(!f.requests.some(x=>/rom\.elf|bootloader|ptable/.test(x)));
 }finally{await f.runtime.dispose();}
});
