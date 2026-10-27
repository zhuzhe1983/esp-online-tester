#!/usr/bin/env node
// Opt-in remote-firmware probe. Firmware bytes stay in RAM; only logs/frame PNGs
// and a hash receipt are written. Supply the upstream URL and SHA-256 explicitly.
import assert from 'node:assert/strict';
import {readFile, mkdir, writeFile} from 'node:fs/promises';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {deflateSync} from 'node:zlib';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';

const [url, expectedHash, output = 'output/official-ink4', duration = '8'] = process.argv.slice(2);
if (!url || !/^[a-f0-9]{64}$/.test(expectedHash || '') || !/^https:\/\//.test(url)) throw new Error('Usage: node tools/probe-official-ink4.mjs <https-url> <sha256> [output-dir] [guest-seconds]');
const seconds = Number(duration); if (!(seconds > 0 && seconds <= 300)) throw new Error('Guest seconds must be 0…300');
const clockTimeout = Number(process.env.ESP32SIM_PROBE_CLOCK_TIMEOUT_MS || 10000);
if (!(clockTimeout >= 1000 && clockTimeout <= 120000)) throw new Error('Clock timeout must be 1000…120000 ms');
const root = new URL('../', import.meta.url), catalog = {};
for (const name of ['products','accessories','firmwares']) catalog[name] = JSON.parse(await readFile(new URL(`web/platform/catalog/${name}.json`,root))).items;
const productId='metalio-ink4';
const product=catalog.products.find(p=>p.id===productId);assert.ok(product.runtime.board,'Catalog must declare the real board');
await mkdir(output,{recursive:true});
const fetchImpl = async (value,options) => new URL(value).hostname === 'platform.example' ? new Response(await readFile(new URL(`web${new URL(value).pathname}`,root))) : fetch(value,options);
let queryClock;
const workerFactory = path => {
  path = new URL(`web${new URL(path).pathname}`,root);
  const script = `import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(m,t)=>parentPort.postMessage(m,t);await import(${JSON.stringify(String(path))});parentPort.on('message',data=>globalThis.onmessage({data}));`;
  const thread = new Worker(new URL(`data:text/javascript,${encodeURIComponent(script)}`));
  const adapter = {postMessage:(message,...args)=>thread.postMessage(message.op==='create'&&(process.env.ESP32SIM_NO_WASM_JIT||process.env.ESP32SIM_FORCE_WASM_JIT)?{...message,jit:!!process.env.ESP32SIM_FORCE_WASM_JIT}:message,...args),terminate:()=>thread.terminate()};
  let clockSequence=0;const clocks=new Map();
  // '{}' is an ignored protocol message: it adds no bytes/events to any guest
  // device. Reuse its queue receipt to sample actual cycles between stat pushes.
  queryClock=()=>new Promise((resolve,reject)=>{const inputId=`probe-clock-${++clockSequence}`;const timer=setTimeout(()=>{clocks.delete(inputId);reject(new Error(`Probe clock query timeout (${clockTimeout} ms)`));},clockTimeout);clocks.set(inputId,time=>{clearTimeout(timer);resolve(time);});adapter.postMessage({op:'text',data:'{}',inputId});});
  thread.on('message',data=>{if(clocks.has(data.inputQueued)){clocks.get(data.inputQueued)(data.time);clocks.delete(data.inputQueued);}else adapter.onmessage?.({data});});thread.on('error',event=>adapter.onerror?.(event));return adapter;
};
const crc32 = bytes => {let n=0xffffffff;for(const value of bytes){n^=value;for(let i=0;i<8;i++)n=(n>>>1)^((n&1)?0xedb88320:0);}return (n^0xffffffff)>>>0;};
const png = frame => {
  const chunk=(type,bytes)=>{const name=Buffer.from(type),size=Buffer.alloc(4),crc=Buffer.alloc(4);size.writeUInt32BE(bytes.length);crc.writeUInt32BE(crc32(Buffer.concat([name,bytes])));return Buffer.concat([size,name,bytes,crc]);};
  const header=Buffer.alloc(13);header.writeUInt32BE(frame.width);header.writeUInt32BE(frame.height,4);header[8]=8;header[9]=6;
  const rows=Buffer.alloc((frame.width*4+1)*frame.height);for(let y=0;y<frame.height;y++)rows.set(frame.pixels.subarray(y*frame.width*4,(y+1)*frame.width*4),y*(frame.width*4+1)+1);
  return Buffer.concat([Buffer.from([137,80,78,71,13,10,26,10]),chunk('IHDR',header),chunk('IDAT',deflateSync(rows)),chunk('IEND',Buffer.alloc(0))]);
};
let serial='',emulator='', latestFrame=null, lastSecond=-1;const inputs=[];
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl:'https://platform.example/platform/index.html',assetBaseUrl:'https://platform.example/',onEvent(event){
  if(event.type==='serial'&&event.source!=='emulator'){serial+=event.data;process.stdout.write(event.data);}
  if(event.type==='serial'&&event.source==='emulator')emulator+=event.data;
  if(event.type==='frame')latestFrame=event;
  if(event.type==='input')inputs.push(event);
  if(event.type==='stats'){const second=Math.floor(Number(event.stats.time));if(second!==lastSecond){lastSecond=second;console.log(JSON.stringify({guestSeconds:second,insns:event.stats.insns}));}}
}});
const started=Date.now();let failure=null;const stages=[];
const checkpoint=async name=>{
  assert.ok(latestFrame,`No frame at ${name}`);assert.equal(latestFrame.width,480);assert.equal(latestFrame.height,800);
  const state=await runtime.observe(),sha256=createHash('sha256').update(latestFrame.pixels).digest('hex');
  const black=latestFrame.pixels.reduce((n,v,i)=>n+(i%4===0&&v===0?1:0),0);
  assert.ok(black>1000&&black<480*800-1000,'Real UI must have both black and white pixels');
  await writeFile(`${output}/${name}.png`,png(latestFrame));
  stages.push({name,guestSeconds:await queryClock(),frameNumber:latestFrame.frameNumber,blackPixels:black,sha256,status:state.status});
};
const waitGuest=async target=>{
  const wall=Date.now();
  while(true){const state=await runtime.observe();if(state.status!=='running')throw new Error(`Runtime entered ${state.status}: ${JSON.stringify(state.lastError)}`);if(serial.includes('Guru Meditation Error'))throw new Error('Official firmware panicked; see serial.log');if(await queryClock()>=target)return;if(Date.now()-wall>240000)throw new Error(`Wall timeout waiting for guest ${target}s`);await new Promise(resolve=>setTimeout(resolve,10));}
};
const button=async(ids,seconds)=>{
  let receipt;
  try{for(const id of ids)receipt=await runtime.input({type:'button',id,pressed:true});await waitGuest(receipt.guestTime+seconds);}
  finally{for(const id of ids){const released=await runtime.input({type:'button',id,pressed:false});if(seconds<0.5&&receipt&&released.guestTime-receipt.guestTime>0.5)throw new Error(`SHORT_PRESS_OVERRUN: short press lasted ${(released.guestTime-receipt.guestTime).toFixed(3)} guest seconds`);}}
  await waitGuest((await queryClock())+1.5);
};
const tap=async(x,y)=>{
  try{const receipt=await runtime.input({type:'touch',x,y,down:true});await waitGuest(receipt.guestTime+0.15);}
  finally{await runtime.input({type:'touch',x,y,down:false});}
  await waitGuest((await queryClock())+1);
};
try {
  const scene=createDefaultScene(catalog,productId);await runtime.configure(scene);await runtime.loadFirmware({url,sha256:expectedHash});await runtime.start();
  await waitGuest(seconds);await checkpoint('home');await tap(395,670);await checkpoint('settings');
  assert.notEqual(stages[0].sha256,stages[1].sha256,'Touch must change actual guest output');
  await button(['home'],0.7);await waitGuest((await queryClock())+3);await checkpoint('cover-home');
  assert.notEqual(stages[1].sha256,stages[2].sha256,'Home hold must change actual guest output');
  await button(['volume-up'],0.15);await checkpoint('volume-up');
  assert.ok(inputs.some(e=>e.event?.id==='home'&&e.receipt?.delivery==='queued'),'Semantic board button must reach the worker');
  await runtime.pause();
}catch(e){failure={message:e.message,code:e.code};process.exitCode=1;}
finally {
  const state=await runtime.observe();
  await writeFile(`${output}/serial.log`,serial);await writeFile(`${output}/emulator.log`,emulator);
  await writeFile(`${output}/receipt.json`,JSON.stringify({productId,board:product.runtime.board,url,expectedHash,
    wasmSha256:createHash('sha256').update(await readFile(new URL('web/wasm/esp32sim.wasm',root))).digest('hex'),
    firmware:state.firmware,stats:state.stats,stages,inputs,failure,elapsedMs:Date.now()-started},null,2)+'\n');
  await runtime.dispose();
}
