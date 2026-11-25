// File-format and lifecycle protocol checks. Worker doubles never execute a CPU
// or establish original factory/LCD compatibility. The optional ROM is real.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {runInNewContext} from 'node:vm';
import {createPlatformRuntime,mergedImageLayout,validateMergedImage} from '../web/platform/runtime.js';
import {validateBoardRevision,validateAppImage,isC5Board,isSramOnlyBoard} from '../web/platform/app-image.js';
import {assembleLocalFirmware,inspectLocalBuild} from '../web/platform/local-firmware.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createSdCardStorage} from '../web/platform/sd-card-storage.js';
import {preparedTfKey} from '../web/platform/tf-card-capability.js';
const hash=bytes=>createHash('sha256').update(bytes).digest('hex'),MIB=1024*1024;
const ROM_HASH='49693d745eb4b4e6e6c77be4f5213765d545d8c7a1d9742a8a619bf596c4e27a';
const board='waveshare-c5-lcd273',productId='waveshare-esp32-c5-lcd-2-73';
const catalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async key=>[key,JSON.parse(await readFile(new URL(`../web/platform/catalog/${key}.json`,import.meta.url))).items])));
function image(segments=[[0x40800000,Uint8Array.of(0x6f,0,0,0)]],entry=segments[0][0]) {
 const end=24+segments.reduce((n,[,bytes])=>n+8+bytes.length,0),checksumAt=Math.floor(end/16)*16+15,d=new Uint8Array(checksumAt+33),v=new DataView(d.buffer);
 d[0]=0xe9;d[1]=segments.length;d[12]=23;d[15]=100;d[17]=199;d[23]=1;v.setUint32(4,entry,true);
 let at=24,sum=0xef;for(const [address,bytes] of segments){v.setUint32(at,address,true);v.setUint32(at+4,bytes.length,true);d.set(bytes,at+8);at+=8+bytes.length;for(const b of bytes)sum^=b;}
 d[checksumAt]=sum;d.set(createHash('sha256').update(d.subarray(0,checksumAt+1)).digest(),checksumAt+1);return d;
}
function layout(appOffset=0x20000,tableOffset=0x8000) {
 const boot=image(),app=image([[0x42000020,Uint8Array.of(1,2,3,4)],[0x408002b0,Uint8Array.of(0x6f,0,0,0)]],0x408002b0);
 const table=new Uint8Array(0xc00).fill(255),v=new DataView(table.buffer);
 for(const [i,subtype,offset] of [[0,0,appOffset],[1,0x10,appOffset+0x10000]]){const at=i*32;v.setUint16(at,0x50aa,true);table[at+2]=0;table[at+3]=subtype;v.setUint32(at+4,offset,true);v.setUint32(at+8,0x10000,true);}
 table.set([0xeb,0xeb],64);table.set(createHash('md5').update(table.subarray(0,64)).digest(),80);
 const data=new Uint8Array(appOffset+app.length).fill(255);data.set(boot,0x2000);data.set(table,tableOffset);data.set(app,appOffset);
 return {boot,app,table,data,appOffset,tableOffset};
}
test('C5 merged headers use boot0x2000, table-derived applications and full MP revision100 fields',()=>{
 for(const [at,pt] of [[0x20000,0x8000],[0x30000,0x9000]])assert.deepEqual(mergedImageLayout(layout(at,pt).data,board,32*MIB),{bootloaderOffset:0x2000,partitionTableOffset:pt});
 const good=layout().data;
 for(const at of [0x2000,0x20000]){
  for(const [field,value] of [[12,13],[15,101],[17,99]]){const d=good.slice();new DataView(d.buffer).setUint16(at+field,value,true);assert.throws(()=>mergedImageLayout(d,board,32*MIB),{code:field===12?'CHIP_MISMATCH':'CHIP_REVISION_UNSUPPORTED'});}
  for(const maximum of [0,100,199,65535]){const d=good.slice();d[at+14]=255;new DataView(d.buffer).setUint16(at+17,maximum,true);validateBoardRevision(d.subarray(at,at+24),board);}
 }
 const moved=good.slice();moved.set(moved.subarray(0x2000,0x2000+80),0);moved[0x2000]=255;assert.throws(()=>mergedImageLayout(moved,board,32*MIB),{code:'INCOMPLETE_IMAGE'});
});
test('C5 local segments and IDF build layouts preserve actual bootloader/table/app offsets',async()=>{
 const l=layout(0x30000,0x9000),files=[new File([l.boot],'boot.bin'),new File([l.table],'table.bin'),new File([l.app],'app.bin')];
 const segments=files.map((file,i)=>({file,offset:[0x2000,l.tableOffset,l.appOffset][i]}));
 const manual=await assembleLocalFirmware({format:'segments',files:segments},{board,flashBytes:32*MIB});assert.deepEqual(manual.data,l.data);
 const args=new File([JSON.stringify({extra_esptool_args:{chip:'esp32c5'},flash_settings:{flash_size:'32MB'},flash_files:{'0x2000':'boot.bin','0x9000':'table.bin','0x30000':'app.bin'}})],'flasher_args.json');
 const build=await assembleLocalFirmware({format:'build',files:[args,...files]},{board,flashBytes:32*MIB});assert.deepEqual(build.data,l.data);assert.equal(build.partitionTableOffset,0x9000);
 await assert.rejects(()=>assembleLocalFirmware({format:'segments',files:[{...segments[0],offset:0},...segments.slice(1)]},{board,flashBytes:32*MIB}),{code:'BOOTLOADER_REQUIRED'});
});
test('the local UI selector preserves C5 SRAM uploads while accepting complete ROM and IDF build inputs',async()=>{
 const source=await readFile(new URL('../web/platform/app.js',import.meta.url),'utf8'),errors=[];
 const helpers=source.slice(source.indexOf('const sramProduct='),source.indexOf('const localFileHint='));
 const selector=source.slice(source.indexOf('async function selectLocalFiles('),source.indexOf('\nfunction onRuntimeEvent('));
 const ui=runInNewContext(`let localSelectionToken=0,localFiles=[],localFile=null,localSource=null;${helpers}\n${selector}\n({selectLocalFiles,source:()=>localSource,sramOption})`,{
  product:()=>catalog.products.find(p=>p.id===productId),isC5Board,isSramOnlyBoard,validateAppImage,validateMergedImage,inspectLocalBuild,
  MAX_FIRMWARE_FILE_BYTES:256*MIB,Uint8Array,Map,Set,text(){},renderTransport(){},toast(){},errorMessages:{},report:e=>errors.push(e),
 });
 assert.equal(ui.sramOption(),true);await ui.selectLocalFiles([new File([image()],'sram.bin')],{replace:true});assert.equal(ui.source().bootMode,'app-direct');
 const l=layout();await ui.selectLocalFiles([new File([l.data],'merged.bin')],{replace:true});assert.equal(ui.source().format,'merged');
 const corrupt=image();corrupt[32]^=1;await ui.selectLocalFiles([new File([corrupt],'bad-sram.bin')],{replace:true});assert.equal(ui.source(),null);assert.equal(errors.at(-1).code,'APP_IMAGE_CHECKSUM');
 const config={extra_esptool_args:{chip:'esp32c5'},flash_files:{'0x2000':'boot.bin','0x8000':'table.bin','0x20000':'app.bin'}};
 await ui.selectLocalFiles([new File([JSON.stringify(config)],'flasher_args.json'),new File([l.boot],'boot.bin'),new File([l.table],'table.bin'),new File([l.app],'app.bin')],{replace:true});assert.equal(ui.source().format,'build');
});

let sequence=0;
function fixture({rom=Uint8Array.of(0),rejectFlash=false}={}) {
 const id=++sequence,base=`https://c5-rom-${id}.invalid/repository/`,requests=[],messages=[],workers=[],resources=new Map();let resource=0;
 const options={indexedDB:null,BroadcastChannel:null,dbName:`c5-runtime-${id}`},sd=createSdCardStorage(options),storage=createDeviceStorage(options);
 resources.set(new URL('wasm/fw/esp32c5_rev100_rom.elf',base).href,rom);
 const workerFactory=()=>{const w={flash:null,sd:null,flashWatch:null,sdWatch:null,dirty:new Set(),terminate(){w.flash=null;w.sd=null;},postMessage(m){messages.push(m);let r;
  if(m.op==='init')r={ready:true};
  else if(m.op==='create'){w.flash=new Uint8Array(m.flash_mb*MIB).fill(255);r={created:true,flashStorage:true,networkAvailable:true,audioAvailable:true};}
  else if(m.op==='load'){if(m.kind===5&&!rejectFlash)w.flash.set(new Uint8Array(m.data));if(m.kind===3)w.flash.set(new Uint8Array(m.data),0x10000);r={loaded:m.kind,ok:!(m.kind===5&&rejectFlash)};}
  else if(m.op==='flash-snapshot')r={flashReply:m.requestId,data:w.flash.slice().buffer};
  else if(m.op==='flash-restore'){for(const p of m.pages)w.flash.set(new Uint8Array(p.data),p.offset);r={flashReply:m.requestId,ok:true};}
  else if(m.op==='flash-watch'){w.flashWatch=m.requestId;r={flashReply:m.requestId,ok:true};}
  else if(m.op==='flash-flush'){if(w.dirty.size){const pages=[...w.dirty].map(offset=>({offset,data:w.flash.slice(offset,offset+4096).buffer}));w.dirty.clear();queueMicrotask(()=>w.onmessage?.({data:{flashDelta:{watchId:w.flashWatch,pages}}}));}r={flashReply:m.requestId,ok:true};}
  else if(m.op==='sd-attach'){w.sd=new Uint8Array(m.data).slice();r={sdReply:m.requestId,ok:true};}
  else if(m.op==='sd-export')r={sdReply:m.requestId,data:w.sd.slice().buffer};
  else if(m.op==='sd-watch'||m.op==='sd-flush')r={sdReply:m.requestId,ok:true};
  else if(m.op==='start')r={started:true};else if(m.op==='stop')r={stopped:0};
  else throw Error('unexpected structural Worker message '+m.op);
  queueMicrotask(()=>w.onmessage?.({data:r}));
 }};workers.push(w);return w;};
 const runtime=createPlatformRuntime({catalog,sdStorage:sd,deviceStorage:storage,workerFactory,baseUrl:new URL('platform/',base).href,assetBaseUrl:base,fetchImpl:async url=>{requests.push(url);if(url.endsWith('esp32sim.wasm'))return new Response(Uint8Array.of(0));assert(resources.has(url),'unexpected fetch '+url);return new Response(resources.get(url));}});
 const remote=bytes=>{const url=new URL(`input-${++resource}.bin`,base).href;resources.set(url,bytes);return {url,sha256:hash(bytes)};};
 const manifest=data=>{const url=new URL(`plan-${++resource}.json`,base).href;resources.set(url,JSON.stringify(data));return {manifestUrl:url};};
 return {runtime,requests,messages,workers,sd,storage,remote,manifest};
}
test('default and manifest C5 ROM hashes reject corruption before creating any Worker',async()=>{
 const f=fixture();try{
  await f.runtime.configure(createDefaultScene(catalog,productId));
  await assert.rejects(()=>f.runtime.loadFirmware({...f.remote(layout().data),bootMode:'rom'}),e=>e.code==='HASH_MISMATCH'&&e.details.expected===ROM_HASH);
  assert.equal(f.workers.length,0);assert(f.requests.some(url=>url.endsWith('/repository/wasm/fw/esp32c5_rev100_rom.elf')));
  await assert.rejects(()=>f.runtime.loadFirmware(f.manifest({board,files:{flash:f.remote(layout().data),rom:f.remote(Uint8Array.of(0))}})),{code:'HASH_MISMATCH'});
  for(const sha256 of ['',0,false])await assert.rejects(()=>f.runtime.loadFirmware(f.manifest({board,files:{flash:f.remote(layout().data),rom:{url:f.remote(Uint8Array.of(0)).url,sha256}}})),{code:'HASH_MISMATCH'});
  assert.equal(f.workers.length,0);
 }finally{await f.runtime.dispose();}
});
test('offsetless C5 ROM apps are rejected instead of staging at the generic0x10000 address',async()=>{
 const f=fixture(),l=layout();try{
  await f.runtime.configure(createDefaultScene(catalog,productId));
  await assert.rejects(()=>f.runtime.loadFirmware(f.manifest({board,files:{bootloader:f.remote(l.boot),ptable:f.remote(l.table),app:f.remote(l.app)}})),{code:'C5_FLASH_LAYOUT_REQUIRED'});
  assert.equal(f.workers.length,0);assert(!f.requests.some(url=>url.endsWith('rom.elf')));
 }finally{await f.runtime.dispose();}
});
test('C5 SRAM TF media retains its original application-hash namespace and modified imported pages',async()=>{
 const f=fixture(),app=image(),key=`v1:${productId}:${board}:${hash(app)}`,media=new Uint8Array(MIB);media[42]=83;
 const saved=await f.sd.replace(key,{image:media.buffer,media:{format:'image',bytes:media.length,sha256:hash(media)}}),page=new Uint8Array(4096);page[71]=19;
 await f.sd.save(key,{epoch:saved.row.epoch,revision:saved.row.revision,pages:[{offset:4096,data:page.buffer}]});
 try{
  await f.runtime.configure(createDefaultScene(catalog,productId));assert.equal((await f.runtime.observe()).sdCard.bytes,64*MIB);
  await f.runtime.loadFirmware({file:new File([app],'sram.bin')});const state=await f.runtime.observe(),caps=await f.runtime.capabilities(),w=f.workers.at(-1);
  assert.equal(state.storage.status,'unsupported');assert.equal(state.sdCard.restored,true);assert.equal(w.sd[42],83);assert.equal(w.sd[4096+71],19);
  assert.equal(caps.sdCard.scope,'product-board-application-sha256');assert.equal(caps.execution.resetMode,'reload-same-sram-image');
  assert.equal(f.messages.find(m=>m.op==='create').appDirect,true);assert(f.messages.filter(m=>m.op==='load').every(m=>m.kind===3));assert(f.requests.every(url=>url.endsWith('esp32sim.wasm')));
  await f.runtime.loadFirmware({file:new File([app],'sram.bin')});assert.equal(f.workers.at(-1).sd[4096+71],19);assert.equal((await f.sd.read(key)).row.pages.length,1);
 }finally{await f.runtime.dispose();}
});

const romPath=process.env.ESP32SIM_C5_ROM;
test('external_pinned_C5_ROM_protocol_keeps_Flash_and_TF_scopes_reset_and_clear_semantics',{skip:!romPath?'set ESP32SIM_C5_ROM to the unchanged pinned revision100 ROM ELF':false},async()=>{
 const rom=new Uint8Array(await readFile(romPath));assert.equal(hash(rom),ROM_HASH);const f=fixture({rom}),l=layout(),sram=image(),oldKey=`v1:${productId}:${board}:${hash(sram)}`;
 try{
  await f.runtime.configure(createDefaultScene(catalog,productId));
  await f.runtime.loadFirmware(f.manifest({board,files:{bootloader:f.remote(l.boot),ptable:f.remote(l.table)},flash_at:{'0x20000':f.remote(l.app)}}));
  const first=f.workers.at(-1),firstState=await f.runtime.observe();assert.equal(first.sd.length,64*MIB);assert.equal(firstState.sdCard.format,'fat32');assert.equal(first.sd[450],0x0c);assert.equal(new DataView(first.sd.buffer).getUint16(510,true),0xaa55);
  assert.deepEqual(first.flash.subarray(0,l.data.length),l.data);assert(f.messages.filter(m=>m.op==='load').every(m=>m.kind===0||m.kind===5));
  await f.runtime.loadFirmware({file:new File([sram],'sram.bin')});
  const oldMedia=new Uint8Array(MIB);oldMedia[42]=91;await f.runtime.attachSdCard({bytes:oldMedia});
  await f.runtime.configure(createDefaultScene(catalog,productId));const prepared=new Uint8Array(MIB);prepared[42]=37;await f.runtime.attachSdCard({bytes:prepared});
  await f.runtime.loadFirmware({file:new File([l.data],'merged.bin'),bootMode:'rom'});let state=await f.runtime.observe(),w=f.workers.at(-1);const scope=state.storage.scope,romKey=`v1:${productId}:${board}:${scope.firmwareSha256}`;
  const initial=new Uint8Array(32*MIB).fill(255);initial.set(l.data);assert.equal(scope.firmwareSha256,hash(initial));assert.notEqual(romKey,oldKey);assert.equal(w.sd[42],37);
  const caps=await f.runtime.capabilities();assert.equal(caps.execution.mode,'wasm-interpreter');assert.equal(caps.execution.resetMode,'rom-reset');assert.equal(caps.network.available,false);assert.equal(caps.audio.input.available,false);assert.equal(caps.audio.output.available,false);assert.deepEqual(caps.input.serialPorts,['uart0']);
  assert.equal(f.messages.filter(m=>m.op==='load').at(-2).kind,0);assert.equal(f.messages.filter(m=>m.op==='load').at(-1).kind,5);
  const offset=32*MIB-4096;w.flash[offset+17]=0x5a;w.dirty.add(offset);await f.runtime.flushDeviceStorage();assert.equal((await f.storage.read(scope)).row.pages.length,1);
  const imported=new Uint8Array(MIB);imported[42]=79;await f.runtime.attachSdCard({bytes:imported});w=f.workers.at(-1);assert.equal(w.flash[offset+17],0x5a);assert.equal(w.sd[42],79);
  await f.runtime.reset();assert.equal(f.messages.filter(m=>m.op==='start').at(-1).appDirect,false);assert.equal(f.workers.at(-1).sd[42],79);assert.equal(f.workers.at(-1).flash[offset+17],0x5a);await f.runtime.pause();
  await f.runtime.clearDeviceStorage();assert.equal(f.workers.at(-1).flash[offset+17],255);assert.equal(f.workers.at(-1).sd[42],79);assert.equal((await f.storage.read(scope)).row.pages.length,0);
  assert.equal(new Uint8Array((await f.sd.read(oldKey)).image)[42],91);assert.equal(new Uint8Array((await f.sd.read(romKey)).image)[42],79);assert.equal(new Uint8Array((await f.sd.read(preparedTfKey(catalog.products.find(p=>p.id===productId)))).image)[42],37);
  await f.runtime.loadFirmware({file:new File([sram],'sram.bin')});assert.equal(f.workers.at(-1).sd[42],91);assert.equal((await f.runtime.observe()).storage.status,'unsupported');
 }finally{await f.runtime.dispose();}
});
test('external_pinned_ROM_C5_backend_rejection_is_not_retried_as_SRAM_or_raw_flash',{skip:!romPath?'set ESP32SIM_C5_ROM to the unchanged pinned revision100 ROM ELF':false},async()=>{
 const f=fixture({rom:new Uint8Array(await readFile(romPath)),rejectFlash:true});try{
  await f.runtime.configure(createDefaultScene(catalog,productId));await assert.rejects(()=>f.runtime.loadFirmware({file:new File([layout().data],'merged.bin'),bootMode:'rom'}),{code:'LOAD_FAILED'});
  assert.deepEqual(f.messages.filter(m=>m.op==='load').map(m=>m.kind),[0,5]);assert(!f.messages.some(m=>['start','flash-restore','flash-watch','sd-attach'].includes(m.op)));
 }finally{await f.runtime.dispose();}
});
const factoryPath=process.env.ESP32SIM_C5_FACTORY;
test('external_original_LCD273_bytes_remain_merged_Flash_in_the_frontend_protocol',{skip:!romPath||!factoryPath?'set ESP32SIM_C5_ROM and ESP32SIM_C5_FACTORY to the unchanged pinned ROM/LCD273 BIN':false},async()=>{
 const bytes=new Uint8Array(await readFile(factoryPath)),digest='cf10a4358d03898458c16390f3f16ceb0306befe2260160de0919d755b920942';assert.equal(hash(bytes),digest);
 assert.deepEqual(mergedImageLayout(bytes,board,32*MIB),{bootloaderOffset:0x2000,partitionTableOffset:0x8000});
 const f=fixture({rom:new Uint8Array(await readFile(romPath))});try{
  await f.runtime.configure(createDefaultScene(catalog,productId));await f.runtime.loadFirmware({file:new File([bytes],'original-lcd273.bin'),sha256:digest,bootMode:'rom'});
  assert.deepEqual(f.messages.filter(m=>m.op==='load').map(m=>m.kind),[0,5]);assert.equal(hash(f.workers.at(-1).flash.subarray(0,bytes.length)),digest);
  const state=await f.runtime.observe();assert.equal(state.firmware.appDirect,false);assert.equal(state.firmware.artifacts.find(a=>a.kind==='flash').sha256,digest);assert.equal((await f.runtime.capabilities()).framebuffer.available,false);
  assert(!f.messages.some(m=>m.op==='start'),'protocol preparation does not start original firmware');
 }finally{await f.runtime.dispose();}
});
