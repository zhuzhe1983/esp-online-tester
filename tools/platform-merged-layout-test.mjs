// Layout fixtures exercise validation/loading only; they are not boot evidence.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {join,basename} from 'node:path';
import {Worker} from 'node:worker_threads';
import {mergedImageLayout,validateMergedImage,createPlatformRuntime,sha256} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {prepareFlashPlan} from '../web/platform/flasher.js';
const root=new URL('../',import.meta.url),MIB=1024*1024;
function merged({table=0x9000,chip=9,boot=0,revision=0}={}){
  const data=new Uint8Array(0x21000).fill(255),view=new DataView(data.buffer);
  for(const at of [boot,0x20000]){data.fill(0,at,at+36);data[at]=0xe9;data[at+1]=1;view.setUint16(at+12,chip,true);view.setUint16(at+15,revision,true);view.setUint32(at+28,4,true);}
  view.setUint16(table,0x50aa,true);data[table+2]=0;data[table+3]=0;view.setUint32(table+4,0x20000,true);view.setUint32(table+8,0x1000,true);
  return data;
}
test('merged BINs detect actual partition sectors across chips while retaining boot offsets',()=>{
  for(const [board,chip,boot,revision]of [['stopwatch',9,0,0],['stickc',0,0x1000,0],['esp32c3',5,0,0],['waveshare-p4-lcd35-v13',18,0x2000,100]])for(const table of [0x8000,0x9000,0x15000]){
    const data=merged({table,chip,boot,revision});assert.equal(validateMergedImage(data,board,16*MIB),true);
    assert.deepEqual(mergedImageLayout(data,board,16*MIB),{bootloaderOffset:boot,partitionTableOffset:table});
  }
});
test('detection rejects ambiguity, malformed tables, overflow, chip and wafer mismatches',()=>{
  const ambiguous=merged(),other=merged({table:0x8000});ambiguous.set(other.subarray(0x8000,0x8020),0x8000);
  assert.throws(()=>mergedImageLayout(ambiguous,'stopwatch',16*MIB),{code:'AMBIGUOUS_FLASH_LAYOUT'});
  assert.equal(validateMergedImage(ambiguous,'stopwatch',16*MIB,{partitionTableOffset:0x9000}),true);
  for(const mutate of [view=>view.setUint32(0x9004,0x20001,true),view=>view.setUint32(0x9008,32*MIB,true),view=>{view.setUint16(0x9020,0x50aa,true);view.setUint32(0x9024,0x20000,true);view.setUint32(0x9028,0x1000,true);}]){
    const data=merged();mutate(new DataView(data.buffer));assert.throws(()=>validateMergedImage(data,'stopwatch',16*MIB));
  }
  assert.throws(()=>validateMergedImage(merged(),'esp32c3',16*MIB),{code:'CHIP_MISMATCH'});
  assert.throws(()=>validateMergedImage(merged({chip:18,boot:0x2000,revision:301}),'waveshare-p4-lcd35-v13',16*MIB),{code:'CHIP_REVISION_UNSUPPORTED'});
  assert.throws(()=>validateMergedImage(merged(),'stopwatch',0x20000),{code:'FLASH_OVERFLOW'});
});
const catalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async name=>[name,JSON.parse(await readFile(new URL(`web/platform/catalog/${name}.json`,root))).items])));
test('raw URL and local BIN loading retain ROM boot and report the detected table address',async()=>{
  const data=merged(),origin='https://merged-layout.invalid',source={url:origin+'/complete.bin',sha256:await sha256(data),bootMode:'rom'};
  const workerFactory=url=>{
    const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(new URL('web'+url.pathname,root)))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));
    const adapter={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',error=>adapter.onerror?.(error));return adapter;
  };
  const runtime=createPlatformRuntime({catalog,workerFactory,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',fetchImpl:async value=>{
    const url=new URL(value);assert.equal(url.origin,origin);if(url.pathname==='/complete.bin')return new Response(data);
    if(url.pathname==='/complete.json')return Response.json({board:'stopwatch',flash_mb:16,psram_mb:8,files:{flash:{url:source.url,sha256:source.sha256}}});
    if(process.env.ESP32SIM_ROM_DIR&&url.pathname.endsWith('_rom.elf'))return new Response(await readFile(join(process.env.ESP32SIM_ROM_DIR,basename(url.pathname))));
    return new Response(await readFile(new URL('web'+url.pathname,root)));
  }});
  try{for(const selected of [source,{file:new File([data],'complete.bin')},{manifestUrl:origin+'/complete.json'}]){
    await runtime.configure(createDefaultScene(catalog,'m5stack-stopwatch'));await runtime.loadFirmware(selected);const state=await runtime.observe();
    assert.equal(state.status,'ready');assert.equal(state.firmware.appDirect,false);assert.deepEqual(state.firmware.flashLayout,{bootloaderOffset:0,partitionTableOffset:0x9000});
  }}finally{await runtime.dispose();}
});
test('hardware preparation preserves original BIN bytes and the detected table sector without a port',async()=>{
  const data=merged(),scene=createDefaultScene(catalog,'m5stack-stopwatch');
  const plan=await prepareFlashPlan({scene,catalog,source:{file:new File([data],'complete.bin')},baseUrl:'https://merged-layout.invalid/platform/'});
  assert.equal(plan.receipt.partitionTableOffset,0x9000);assert.deepEqual(plan.files.map(file=>file.address),[0]);assert.deepEqual(plan.files[0].data,data);
  const image=new File([data],'complete.bin'),build=(chip,flash_size='16MB')=>({format:'build',files:[image,new File([JSON.stringify({flash_files:{'0x0':'complete.bin'},extra_esptool_args:{chip},flash_settings:{flash_size}})],'flasher_args.json')]});
  const fromBuild=await prepareFlashPlan({scene,catalog,source:build('esp32s3')});assert.equal(fromBuild.receipt.partitionTableOffset,0x9000);assert.deepEqual(fromBuild.files[0].data,data);
  await assert.rejects(()=>prepareFlashPlan({scene,catalog,source:build('esp32c3')}),{code:'CHIP_MISMATCH'});
  await assert.rejects(()=>prepareFlashPlan({scene,catalog,source:build('esp32s3','32MB')}),{code:'FLASH_OVERFLOW'});
});
