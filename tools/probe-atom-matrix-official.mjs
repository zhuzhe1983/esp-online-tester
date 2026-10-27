#!/usr/bin/env node
// Official remote factory-image probe; remote target/hash are explicit, no firmware is published.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createDefaultScene} from '../web/platform/catalog.js';
const [url,sha256]=process.argv.slice(2);
if(!/^https:\/\//.test(url||'')||!/^[a-f0-9]{64}$/.test(sha256||''))throw Error('Supply <official-https-url> <sha256>.');
const root=new URL('../',import.meta.url),baseUrl='https://atom-matrix-fixture.invalid/platform/';
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const fetchImpl=async (value,options)=>{const url=new URL(value);if(url.origin!=='https://atom-matrix-fixture.invalid')return fetch(value,options);try{return new Response(await readFile(new URL('web'+url.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root);const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(message,...args)=>thread.postMessage(message.op==='create'&&process.argv.includes('--no-jit')?{...message,jit:false}:message,...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl,assetBaseUrl:'https://atom-matrix-fixture.invalid/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'atom-matrix-official-probe'})});
const seen=[];runtime.subscribe(e=>{if(e.type==='leds')seen.push(e.leds);});
const wait=async predicate=>{const end=Date.now()+120000;while(Date.now()<end){const state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(JSON.stringify(state.lastError));await new Promise(r=>setTimeout(r,30));}throw Error('Official probe timed out');};
try {
 await runtime.configure(createDefaultScene(catalog,'m5stack-c008-b'));await runtime.loadFirmware({url,sha256});await runtime.start();
 const boot=await wait(s=>s.stats.time>=5&&s.serial.includes('Atom Matrix Factory test'));assert.equal(boot.frame,null);assert.ok(!boot.serial.includes('Guru Meditation'));assert.equal(boot.leds.length,25);assert.ok(boot.leds.some(p=>p.some(v=>v>0)));
 await wait(s=>s.stats.time>=10);
 await runtime.pause();const state=await runtime.observe();
 console.log(JSON.stringify({checkedAt:new Date().toISOString(),command:'node tools/probe-atom-matrix-official.mjs <official-https-url> <sha256>',productId:'m5stack-c008-b',url,sha256,wasmSha256:createHash('sha256').update(await readFile(new URL('web/wasm/esp32sim.wasm',root))).digest('hex'),firmware:state.firmware,bootSerial:boot.serial.split(/\r?\n/).filter(x=>/Atom Matrix Factory|Wifi scan/.test(x)),guestSeconds:state.stats.time,checks:['actual-page-worker-full-rom-boot','official-g27-rmt-25-pixel-animation','no-invented-framebuffer'],leds:seen.filter((x,i)=>i===0||i===seen.length-1||i%20===0),serialEvidence:[...new Set(state.serial.split(/\r?\n/))].filter(x=>/Atom Matrix Factory|Wifi scan|Guru Meditation/.test(x)),status:'pass',limitations:['Official startup and RMT animation only; remains in Wi-Fi scan through native 60 guest seconds. Official button/IMU/full flow not verified; source fixture covers input path separately.']},null,2));
} finally {await runtime.dispose();}
