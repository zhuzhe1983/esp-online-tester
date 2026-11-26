// Production Worker + actual compiled H2 WASM; no mocked emulator exports.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {Worker} from 'node:worker_threads';
import {waveshareH2CoreProbe} from './fixtures/waveshare-h2-core-probe.mjs';

const workerUrl=new URL('../web/wasm/worker.js',import.meta.url);
const wasmPath=process.env.ESP32SIM_WASM || new URL('../web/wasm/esp32sim.wasm',import.meta.url);
const bytes=await readFile(wasmPath);
const binary=value=>value.buffer.slice(value.byteOffset,value.byteOffset+value.byteLength);
function realWorker(){
 const script=`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(m,t)=>parentPort.postMessage(m,t);await import(${JSON.stringify(workerUrl.href)});parentPort.on('message',data=>globalThis.onmessage({data}));`;
 const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(script))),messages=[],waiters=[];
 let serial='';
 thread.on('message',message=>{
  messages.push(message);
  if(message.text){try{const text=JSON.parse(message.text);if(text.t==='serial')serial+=text.data||'';}catch{}}
  for(const waiter of [...waiters])if(messages.length>waiter.after&&waiter.predicate(message)){clearTimeout(waiter.timer);waiters.splice(waiters.indexOf(waiter),1);waiter.resolve(message);}
 });
 thread.on('error',error=>{for(const waiter of waiters.splice(0)){clearTimeout(waiter.timer);waiter.reject(error);}});
 const wait=(predicate,after=0)=>{
  const found=messages.slice(after).find(predicate);if(found)return Promise.resolve(found);
  return new Promise((resolve,reject)=>{const waiter={predicate,after,resolve,reject};waiter.timer=setTimeout(()=>{waiters.splice(waiters.indexOf(waiter),1);reject(Error('Actual H2 Worker timeout: '+JSON.stringify(messages.slice(-5))));},15000);waiters.push(waiter);});
 };
 return{messages,get serial(){return serial;},clearSerial(){serial='';},post:data=>thread.postMessage(data),wait,
  request(data,predicate){const after=messages.length;thread.postMessage(data);return wait(predicate,after);},close:()=>thread.terminate()};
}

test('NanoH2 and every H2 name retain SRAM-only Worker capabilities and execute a real H2 probe',async t=>{
 const h=realWorker(),probe=waveshareH2CoreProbe();let requestId=0;
 try{
  await h.request({op:'init',wasm:binary(bytes)},m=>m.ready===true);
  for(const board of ['nanoh2','m5stack-c149','esp32h2','waveshare-h2-zero','waveshare-h2-dev-kit-n4']){
   h.clearSerial();
   const create=()=>h.request({op:'create',board,flash_mb:4,psram_mb:0,jit:true,audioStreaming:true,appDirect:true},m=>Object.hasOwn(m,'created'));
   const created=await create();assert.equal(created.created,true,board);
   assert.equal(created.networkAvailable,false,board+' cannot advertise Wi-Fi/Ethernet');
   assert.equal(created.audioAvailable,false,board+' cannot advertise PCM audio');
   assert.equal(created.flashStorage,false,board+' cannot advertise persistent Flash in a SRAM session');
   for(const op of ['network-setup','network-relay','network-receive']){
    const id=++requestId,reply=await h.request({op,requestId:id,spec:'',enabled:true,interface:0,data:new ArrayBuffer(14)},m=>m.networkReply===id);
    assert.equal(reply.ok,false);assert.match(reply.error,/transport unavailable/i);
   }
   for(const op of ['audio-configure','audio-state','audio-push','audio-disable']){
    const id=++requestId,reply=await h.request({op,requestId:id,port:0,sampleRate:16000,channels:1,data:new ArrayBuffer(2)},m=>m.audioReply===id);
    assert.equal(reply.ok,false);assert.match(reply.error,/PCM audio is unsupported/);
   }
   for(const op of ['flash-snapshot','flash-watch','flash-restore']){
    const id=++requestId,reply=await h.request({op,requestId:id,pages:[]},m=>m.flashReply===id);
    assert.equal(reply.ok,false);assert.match(reply.error,/Flash snapshots require a ROM\/Flash session/);
   }
   const rom=await h.request({op:'load',kind:0,data:new ArrayBuffer(16)},m=>m.loaded===0);assert.equal(rom.ok,false,board+' cannot accept a ROM');
   const image=await h.request({op:'load',kind:3,data:binary(probe)},m=>m.loaded===3);assert.equal(image.ok,true);
   const romBoot=await h.request({op:'start',appDirect:false},m=>Object.hasOwn(m,'started'));assert.equal(romBoot.started,false);
   await h.request({op:'load',kind:3,data:binary(probe)},m=>m.loaded===3);
   const started=await h.request({op:'start',appDirect:true},m=>Object.hasOwn(m,'started'));assert.equal(started.started,true);
   await h.wait(()=>h.serial.includes('CORE H2 SRAM READY\n')&&h.serial.includes('BOOT UP\n'));
   const serialBefore=h.serial.length,inputId=++requestId;
   await h.request({op:'text',inputId,data:JSON.stringify({t:'btn',pin:9,v:1})},m=>m.inputQueued===inputId);
   await h.wait(()=>h.serial.slice(serialBefore).includes('BOOT DOWN\n'));
   await h.request({op:'stop'},m=>Object.hasOwn(m,'stopped'));
   t.diagnostic(JSON.stringify({board,created,guestSerial:h.serial,networkDenied:3,audioDenied:4,flashDenied:3,romLoad:rom.ok,romBoot:romBoot.started}));
   for(const [op,spec,error]of [['stub','rom_fn=0',/invalid stub/],['wifi','ap=example',/invalid WiFi/]]){
    await create();h.post({op,spec});
    const denied=await h.request({op:'start',appDirect:true},m=>Object.hasOwn(m,'started'));
    assert.equal(denied.started,false);assert.match(denied.error,error,board+' setup cannot bypass the SRAM boundary');
   }
  }
 }finally{await h.close();}
});
