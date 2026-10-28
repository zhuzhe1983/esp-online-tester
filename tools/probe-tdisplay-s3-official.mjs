#!/usr/bin/env node
// Caller-selected unmodified official image, actual platform worker, no mirrored firmware.
import assert from 'node:assert/strict';
import {readFile,mkdir,writeFile} from 'node:fs/promises';
import {basename,join} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
const args=process.argv.slice(2),arg=name=>args[args.indexOf(name)+1];
for(const flag of ['--url','--sha256','--expect'])assert(args.includes(flag)&&arg(flag),`Required ${flag}`);
const url=arg('--url'),expected=arg('--sha256'),expect=arg('--expect');
assert(/^https:\/\//.test(url));assert(/^[a-f0-9]{64}$/.test(expected));assert(['tft-demo','factory-blocked'].includes(expect));
const hash=b=>createHash('sha256').update(b).digest('hex'),root=new URL('../',import.meta.url),baseUrl='https://tdisplay-s3.invalid/platform/';
const response=await fetch(url,{signal:AbortSignal.timeout(45000)});assert(response.ok,`Firmware HTTP ${response.status}`);
const firmware=new Uint8Array(await response.arrayBuffer());assert.equal(hash(firmware),expected);
const catalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async key=>[key,JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items])));
const product=catalog.products.find(p=>p.id==='lilygo-t-display-s3');assert.equal(product.runtime.board,'tdisplay-s3');assert.equal(product.defaultFirmwareId,'lilygo-t-display-s3-factory');
const fetchImpl=async value=>{const target=new URL(value);if(String(target)===url)return new Response(firmware);assert.equal(target.origin,'https://tdisplay-s3.invalid');try{return new Response(await readFile(target.pathname.endsWith('_rom.elf')&&process.env.ESP32SIM_ROM_DIR?join(process.env.ESP32SIM_ROM_DIR,basename(target.pathname)):new URL('web'+target.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root),thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',error=>adapter.onerror?.(error));return adapter;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl,assetBaseUrl:'https://tdisplay-s3.invalid/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'tdisplay-s3-probe'})});
const receipt={checkedAt:new Date().toISOString(),productId:product.id,source:{url,resolvedUrl:response.url,mediaType:response.headers.get('content-type'),bytes:firmware.length,sha256:hash(firmware)},backend:'actual-platform-worker-wasm-jit',wasmSha256:hash(await readFile(new URL('web/wasm/esp32sim.wasm',root))),firmwarePatched:false,expectedOutcome:expect,frames:[],checks:[]};
const waitTime=async time=>{const deadline=Date.now()+90000;while(Date.now()<deadline){const s=await runtime.observe();if(s.status==='error')throw Error(JSON.stringify(s.lastError));if(s.stats.time>=time)return s;await new Promise(r=>setTimeout(r,10));}throw Error('Guest did not reach requested time');};
const capture=async label=>{await runtime.pause();const state=await runtime.observe(),frame=await runtime.capture({format:'rgba'});assert.deepEqual([frame.width,frame.height],[170,320]);let nonblack=0;for(let i=0;i<frame.pixels.length;i+=4)nonblack+=frame.pixels[i]!==0||frame.pixels[i+1]!==0||frame.pixels[i+2]!==0;receipt.frames.push({label,guestSeconds:state.stats.time,insns:state.stats.insns,nonblackPixels:nonblack,rgbaSha256:hash(frame.pixels)});await mkdir('output/tdisplay-s3-official',{recursive:true});await writeFile(`output/tdisplay-s3-official/${expect}-${label}.rgba`,frame.pixels);return frame;};
try {
 const scene=createDefaultScene(catalog,product.id);scene.firmware={url,sha256:expected};await runtime.configure(scene);await runtime.loadFirmware();await runtime.start();
 if(expect==='tft-demo') {
  await waitTime(3.9);const first=await capture('font-page-1');assert(receipt.frames.at(-1).nonblackPixels>1000);
  await runtime.start();await waitTime(5.1);const second=await capture('font-page-2');assert(receipt.frames.at(-1).nonblackPixels>1000);assert.notDeepEqual(first.pixels,second.pixels);
  receipt.checks.push('ROM boot and two nonblank changing 170x320 guest font pages','GPIO i8080 writes and latched LEDC backlight reach actual worker framebuffer');
 } else {
  await waitTime(1);await capture('blocked');assert.equal(receipt.frames.at(-1).nonblackPixels,0);receipt.checks.push('ROM execution reaches one second without visible framebuffer; native descriptor diagnosis retained separately');
 }
 receipt.status='pass';receipt.limitations=['TFT demo has no button handler; independent Rust guest-instruction tests cover GPIO0/14 input.','Factory and LVGL DMA compatibility remain unresolved; no physical hardware test.'];
 console.log(JSON.stringify(receipt,null,2));
} finally {await runtime.dispose();}
