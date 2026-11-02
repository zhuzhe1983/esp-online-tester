#!/usr/bin/env node
// Explicit local inputs; run the actual platform Worker/WASM, never physical USB.
// Cases: [{id,mergedPath|buildPath|source,seconds?,expectCodes?,absentCodes?,noWarnings?}].
// Only hashes, counters and bounded hardware observations go into the receipt.
import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve,dirname,join} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';

const [casesPath,output]=process.argv.slice(2);
if(!casesPath||!output)throw Error('Usage: node tools/probe-hardware-diagnostics.mjs <cases.json> <receipt.json>');
const cases=JSON.parse(await readFile(casesPath,'utf8'));
assert.ok(Array.isArray(cases)&&cases.length>0&&cases.length<=12);
const root=new URL('../',import.meta.url),origin='https://hardware-check.invalid',catalog={},assets=new Map();
const romPath=process.env.ESP32SIM_ROM_DIR?join(process.env.ESP32SIM_ROM_DIR,'esp32s3_rev0_rom.elf'):new URL('web/wasm/fw/esp32s3_rev0_rom.elf',root);
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
for(const name of ['products','accessories','firmwares'])catalog[name]=JSON.parse(await readFile(new URL(`web/platform/catalog/${name}.json`,root))).items;
const fetchImpl=async(value,options)=>{
  const url=new URL(value);
  if(url.origin!==origin)return fetch(value,options);
  if(url.pathname==='/wasm/fw/esp32s3_rev0_rom.elf')return new Response(await readFile(romPath));
  try{return new Response(assets.get(url.pathname)||await readFile(new URL(`web${url.pathname}`,root)));}
  catch{return new Response('',{status:404});}
};
const workerFactory=url=>{
  const path=new URL('web'+url.pathname,root);
  const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(path))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));
  const adapter={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};
  thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',event=>adapter.onerror?.(event));return adapter;
};
let frame=null,events=0;
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',deviceStorage:createDeviceStorage({indexedDB:null}),onEvent(event){
  if(event.type==='frame')frame=event;
  if(event.type==='hardware-diagnostics')events++;
}});
const receipt={backend:'actual-platform-worker-wasm',wasmSha256:hash(await readFile(new URL('web/wasm/esp32sim.wasm',root))),romSha256:hash(await readFile(romPath)),cases:[]};
const wait=async predicate=>{
  const until=Date.now()+180000;
  while(Date.now()<until){const state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(JSON.stringify(state.lastError));await new Promise(r=>setTimeout(r,40));}
  throw Error('Timed out waiting for guest execution or hardware checks');
};
try {
  for(const spec of cases){
    assert.match(spec.id,/^[a-z0-9-]{1,64}$/);
    const seconds=spec.seconds??8;assert.ok(seconds>=1&&seconds<=60);
    assets.clear();let source=spec.source,inputSha256=source?.sha256?{source:source.sha256}:{};
    if(spec.mergedPath){
      const bytes=await readFile(resolve(spec.mergedPath));assets.set('/fixture/image.bin',bytes);
      source={url:origin+'/fixture/image.bin',sha256:hash(bytes)};inputSha256.merged=hash(bytes);
    }else if(spec.buildPath){
      const folder=resolve(spec.buildPath),layout=JSON.parse(await readFile(join(folder,'flasher_args.json'),'utf8'));
      const manifest={board:'stopwatch',flash_mb:16,psram_mb:8,files:{rom:'/wasm/fw/esp32s3_rev0_rom.elf'},artifact_sha256:{}};
      for(const [kind,key] of [['bootloader','bootloader'],['ptable','partition-table'],['app','app']]){
        const bytes=await readFile(join(folder,layout[key].file)),url=`/fixture/${kind}.bin`;
        assets.set(url,bytes);manifest.files[kind]=url;manifest.artifact_sha256[url]=hash(bytes);inputSha256[kind]=hash(bytes);
      }
      const bytes=Buffer.from(JSON.stringify(manifest));assets.set('/fixture/manifest.json',bytes);
      source={manifestUrl:origin+'/fixture/manifest.json',sha256:hash(bytes)};
    }
    assert.ok(source);
    if(spec.source&&!inputSha256.source){
      const response=await fetchImpl(source.manifestUrl||source.url);assert.ok(response.ok);
      inputSha256.source=hash(Buffer.from(await response.arrayBuffer()));
    }
    const scene=createDefaultScene(catalog,'m5stack-stopwatch');scene.firmware=source;
    await runtime.configure(scene);assert.equal((await runtime.observe()).hardwareDiagnostics,null,'new scene clears previous findings');
    frame=null;events=0;await runtime.loadFirmware(source);await runtime.start();
    await wait(state=>Number(state.stats.time)>=seconds&&state.hardwareDiagnostics);
    await runtime.pause();const state=await runtime.observe(),checks=state.hardwareDiagnostics;
    assert.ok(checks,'real WASM must supply a hardware snapshot');
    const codes=checks.findings.map(f=>f.code);
    for(const code of spec.expectCodes||[])assert.ok(codes.includes(code),`${spec.id}: missing ${code}`);
    for(const code of spec.absentCodes||[])assert.ok(!codes.includes(code),`${spec.id}: unexpected ${code}`);
    if(spec.noWarnings)assert.equal(checks.findings.filter(f=>f.severity==='warning').length,0,`${spec.id}: ${JSON.stringify(checks.findings)}`);
    assert.ok(frame&&frame.pixels.some((v,i)=>i%4!==3&&v!==0),'firmware must produce actual visible pixels');
    const isolated=await runtime.observe();isolated.hardwareDiagnostics.findings.length=0;
    assert.deepEqual((await runtime.observe()).hardwareDiagnostics,checks,'returned snapshots cannot mutate runtime checks');
    receipt.cases.push({id:spec.id,inputSha256,guestSeconds:state.stats.time,insns:state.stats.insns,diagnosticEvents:events,checks,
      framebuffer:{width:frame.width,height:frame.height,sha256:hash(frame.pixels)},
      panicSeen:/Guru Meditation Error|abort\(\) was called/.test(state.serial),applicationReady:state.serial.includes('PLATFORM_READY')});
    assert.equal(receipt.cases.at(-1).panicSeen,false);
    await mkdir(dirname(resolve(output)),{recursive:true});await writeFile(output,JSON.stringify(receipt,null,2)+'\n');
    console.log(JSON.stringify({case:spec.id,codes,guestSeconds:state.stats.time}));
  }
} finally {await runtime.dispose();}
