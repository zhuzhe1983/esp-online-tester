// Lifecycle/transport doubles only: these tests do not execute factory firmware.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {runInNewContext} from 'node:vm';
import {resolve} from 'node:path';
import {createPacing} from '../web/wasm/pacing.mjs';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createSdCardStorage} from '../web/platform/sd-card-storage.js';
import {C5_WIFI_DRIVER_COMPAT_PROFILE as PROFILE} from '../web/platform/c5-wifi-driver-compat.js';

const catalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async name=>[name,JSON.parse(await readFile(new URL(`../web/platform/catalog/${name}.json`,import.meta.url))).items])));
const rom=new Uint8Array(await readFile(process.env.ESP32SIM_ROM_DIR?resolve(process.env.ESP32SIM_ROM_DIR,'esp32c5_rev100_rom.elf'):new URL('../web/wasm/fw/esp32c5_rev100_rom.elf',import.meta.url)));
const hash=b=>createHash('sha256').update(b).digest();
function image() {
  const b=new Uint8Array(81),v=new DataView(b.buffer);b[0]=0xe9;b[1]=1;b[12]=23;b[15]=100;b[17]=199;b[23]=1;v.setUint32(4,0x40800000,true);v.setUint32(24,0x40800000,true);v.setUint32(28,4,true);b[32]=0x6f;b[47]=0xef^0x6f;b.set(hash(b.subarray(0,48)),48);return b;
}
function merged() {
  const b=new Uint8Array(0x20000+81).fill(255),v=new DataView(b.buffer);b.set(image(),0x2000);b.set(image(),0x20000);
  v.setUint16(0x8000,0x50aa,true);b[0x8002]=0;b[0x8003]=0;v.setUint32(0x8004,0x20000,true);v.setUint32(0x8008,0x100000,true);b.set([0xeb,0xeb],0x8020);b.set(createHash('md5').update(b.subarray(0x8000,0x8020)).digest(),0x8030);return new File([b],'protocol-fixture.bin');
}
const state=()=>({enabled:true,profile:PROFILE,nativeWifi:false,associated:false,hasIp:false,storage:'session-RAM',hookCalls:'0',virtualSlots:'0'});
let sequence=0;
function fixture() {
  const workers=[],options={indexedDB:null,BroadcastChannel:null,dbName:`c5-compat-protocol-${++sequence}`};let nextCreated,holdNext=false;
  const workerFactory=()=>{
    const w={terminated:false,flash:new Uint8Array(32<<20).fill(255),heldCreate:null,terminate(){w.terminated=true;w.flash=null;},postMessage(m){
      const send=value=>queueMicrotask(()=>{if(!w.terminated)w.onmessage?.({data:value});});
      if(m.op==='init')send({ready:true});
      else if(m.op==='create'){
        const created=()=>send({created:true,clockHz:240000000,flashStorage:true});
        if(holdNext){holdNext=false;w.heldCreate=created;nextCreated?.();}else created();
      }else if(m.op==='load'){if(m.kind===5)w.flash.set(new Uint8Array(m.data));send({loaded:m.kind,ok:true});}
      else if(m.op==='flash-snapshot')send({flashReply:m.requestId,ok:true,data:w.flash.slice().buffer});
      else if(m.op.startsWith('flash-'))send({flashReply:m.requestId,ok:true});
      else if(m.op.startsWith('sd-'))send({sdReply:m.requestId,ok:true});
      else if(m.op==='c5-driver-compat')send({c5DriverCompatReply:m.requestId,ok:true,driverCompatibility:state()});
      else if(m.op==='stop')send({stopped:0});
      else if(m.op==='start')send({started:true});
    }};workers.push(w);return w;
  };
  const runtime=createPlatformRuntime({catalog,workerFactory,deviceStorage:createDeviceStorage(options),sdStorage:createSdCardStorage(options),baseUrl:'https://c5-compat.invalid/platform/',assetBaseUrl:'https://c5-compat.invalid/',requestTimeout:5000,fetchImpl:async value=>{
    const path=new URL(value).pathname;if(path.endsWith('esp32c5_rev100_rom.elf'))return new Response(rom);assert(path.endsWith('esp32sim.wasm'));return new Response(Uint8Array.of(0));
  }});
  return {runtime,workers,holdCreate(){holdNext=true;return new Promise(done=>{nextCreated=done;});},async configure(){const scene=createDefaultScene(catalog,'waveshare-esp32-c5-lcd-2-73');scene.c5WifiDriverCompat=PROFILE;await runtime.configure(scene);assert.equal((await runtime.capabilities()).execution.counterSemantics,'interpreter-slots-including-wfi-idle-and-driver-compat');}};
}

test('cancelling from driver confirmation cannot commit a cancelled bundle or return a successful load',async()=>{
  const f=fixture();let cancelled;
  try {
    await f.configure();const unsubscribe=f.runtime.subscribe(event=>{if(event.type==='driver-compatibility'&&event.driverCompatibility?.enabled)cancelled=f.runtime.cancelPendingLoad();});
    try {await assert.rejects(f.runtime.loadFirmware({file:merged()}),{code:'FETCH_ABORTED'});}finally{unsubscribe();}
    await cancelled;const s=await f.runtime.observe();assert.equal(s.status,'configured');assert.equal(s.firmware,null);assert.equal(s.driverCompatibility,null);assert(f.workers.every(w=>w.terminated));
  }finally{await f.runtime.dispose();}
});

test('an old worker error cannot reject a new worker pending creation',async()=>{
  const f=fixture();
  try {
    await f.configure();await f.runtime.loadFirmware({file:merged()});const old=f.workers[0],created=f.holdCreate(),loading=f.runtime.loadFirmware({file:merged()});
    await created;old.onerror({message:'late error from a terminated worker'});f.workers.at(-1).heldCreate();
    await loading;const s=await f.runtime.observe();assert.equal(s.status,'ready');assert.equal(s.lastError,null);assert.equal(s.driverCompatibility.enabled,true);
  }finally{await f.runtime.dispose();}
});

test('a current worker error terminates it and clears the active driver observation',async()=>{
  const f=fixture();
  try {
    await f.configure();await f.runtime.loadFirmware({file:merged()});const current=f.workers.at(-1);assert.equal((await f.runtime.observe()).driverCompatibility.enabled,true);
    current.onerror({message:'snapshot transport failed'});const s=await f.runtime.observe();assert.equal(s.status,'error');assert.equal(s.driverCompatibility,null);assert.equal(current.terminated,true);assert.equal(s.lastError.code,'WORKER_ERROR');
  }finally{await f.runtime.dispose();}
});

test('a confirmed low-level profile seals loading and Flash restore before boot',async()=>{
  const source=(await readFile(new URL('../web/wasm/worker.js',import.meta.url),'utf8')).replace(/^import .*;\n/gm,'');
  let loads=0,writes=0,boots=0,present=true;const messages=[],memory=new WebAssembly.Memory({initial:1});
  const wasm={memory,esp32sim_alloc:()=>128,esp32sim_free(){},esp32sim_new:()=>1,esp32sim_delete(){},esp32sim_set_jit(){},esp32sim_cpu_hz:()=>240000000,esp32sim_cycles:()=>0,esp32sim_insns:()=>0,esp32sim_boot:()=>{boots++;return 0;},esp32sim_out_take:()=>0,esp32sim_load:()=>{loads++;return 0;},esp32sim_flash_ptr:()=>4096,esp32sim_flash_len:()=>8192,esp32sim_flash_write:()=>{writes++;return 0;},esp32sim_c5_wifi_driver_compat:()=>0,esp32sim_c5_wifi_driver_compat_state:(_e,p)=>{if(!present)return 0;const b=new TextEncoder().encode(JSON.stringify(state()));new Uint8Array(memory.buffer).set(b,p);return b.length;}};
  class Channel {port1={};port2={postMessage(){}};}
  const context={TextEncoder,TextDecoder,MessageChannel:Channel,ArrayBuffer,performance:{now:()=>0,timeOrigin:0},Date,setTimeout(){},postMessage:m=>messages.push(m),createPacing,createJitHost:()=>({imports:{}}),applyExperiments(){},WebAssembly:{instantiate:async()=>({instance:{exports:wasm}})}};
  runInNewContext(source,context);const send=data=>context.onmessage({data});
  await send({op:'init'});await send({op:'create',board:'waveshare-c5-lcd273',flash_mb:32,psram_mb:8,appDirect:false,c5WifiDriverCompat:PROFILE});
  await send({op:'c5-driver-compat',profile:PROFILE,requestId:1});assert.equal(messages.at(-1).ok,true);
  await send({op:'load',kind:5,data:new ArrayBuffer(0)});assert.equal(messages.at(-1).ok,false);assert.equal(loads,0);
  await send({op:'flash-restore',requestId:2,pages:[]});assert.equal(messages.at(-1).ok,false);assert.equal(writes,0);
  await send({op:'start',appDirect:false});assert.equal(boots,1);assert.equal(messages.at(-1).started,true);
  await send({op:'stop'});present=false;await send({op:'start',appDirect:false});assert.equal(messages.at(-1).started,false);assert.equal(boots,1,'a lost profile must not reach boot');assert(messages.some(m=>m.driverCompatibility===null));
});
