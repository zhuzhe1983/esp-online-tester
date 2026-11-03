#!/usr/bin/env node
// Bounded observation of caller-supplied exact official merged firmware.
// A completed observation is not a whole-application compatibility assertion.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createDefaultScene} from '../web/platform/catalog.js';
const [file,sha256,sourceUrl]=process.argv.slice(2);if(!file||!/^[a-f0-9]{64}$/.test(sha256||'')||!/^https:\/\//.test(sourceUrl||''))throw Error('Supply <merged-firmware-file> <sha256> <official-https-source-url>.');
const hash=bytes=>createHash('sha256').update(bytes).digest('hex'),bytes=await readFile(file);assert.equal(hash(bytes),sha256);
const root=new URL('../',import.meta.url),origin='https://stamplc-official.invalid',catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const fetchImpl=async value=>{const u=new URL(value);assert.equal(u.origin,origin);try{return new Response(await readFile(new URL('web'+u.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root),thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const a={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};thread.on('message',data=>a.onmessage?.({data}));thread.on('error',e=>a.onerror?.(e));return a;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'stamplc-factory-probe'})});
const receipt={checkedAt:new Date().toISOString(),command:'node tools/probe-stamplc-factory.mjs <merged-firmware-file> <sha256> <official-https-source-url>',sourceUrl,sha256,bytes:bytes.length,wasmSha256:hash(await readFile(new URL('web/wasm/esp32sim.wasm',root))),productId:'m5stack-k141',backend:'actual-wasm-worker',observations:[],limitations:['No physical hardware test.','Elapsed boot time, input transport or a frame change alone does not prove application button behavior.','Missing modeled sensor/RTC/SD/industrial interfaces remain explicit.']};
const delay=ms=>new Promise(r=>setTimeout(r,ms)),serial=s=>s.serialByPort.uart0||s.serial;
async function until(target){const end=Date.now()+120000;while(Date.now()<end){const s=await runtime.observe();if(s.stats.time>=target||s.status==='error')return s;await delay(30);}throw Error(`Guest did not reach ${target} seconds within the 120 second wall limit.`);}
async function snapshot(label){const s=await runtime.observe();const r={label,status:s.status,guestSeconds:s.stats.time,leds:s.leds,frame:s.frame};if(s.frame){const f=await runtime.capture({format:'rgba'});r.rgbaSha256=hash(f.pixels);let nonblack=0;const colors=new Set();for(let i=0;i<f.pixels.length;i+=4){if(f.pixels[i]||f.pixels[i+1]||f.pixels[i+2])nonblack++;colors.add(f.pixels[i]<<16|f.pixels[i+1]<<8|f.pixels[i+2]);}r.nonblackPixels=nonblack;r.distinctColors=colors.size;}receipt.observations.push(r);return s;}
try {
 const scene=createDefaultScene(catalog,'m5stack-k141');scene.firmware=null;await runtime.configure(scene);await runtime.loadFirmware({file:new File([bytes],'stamplc-factory.bin')});await runtime.start();await until(10);let s=await snapshot('boot-10s');
 if(s.status==='running'){for(const id of ['button-a','button-b','button-c']){const at=serial(s).length;await runtime.input({type:'button',id,pressed:true});s=await until(s.stats.time+0.25);await runtime.input({type:'button',id,pressed:false});s=await until(s.stats.time+0.75);await snapshot(id+'-transport');receipt.observations.at(-1).newSerial=serial(s).slice(at);}}
 await runtime.pause();s=await runtime.observe();receipt.serial=serial(s);receipt.lastError=s.lastError;receipt.status='observed';receipt.simulationVerified=false;console.log(JSON.stringify(receipt,null,2));
}finally{await runtime.dispose();}
