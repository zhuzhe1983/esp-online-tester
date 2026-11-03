#!/usr/bin/env node
// Official remote UIFlow2 probe; remote target/hash are explicit, no firmware is published.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createDefaultScene} from '../web/platform/catalog.js';
const [url,sha256]=process.argv.slice(2);
if(!/^https:\/\//.test(url||'')||!/^[a-f0-9]{64}$/.test(sha256||''))throw Error('Supply <official-https-url> <sha256>.');
const productIndex=process.argv.indexOf('--product'),productId=productIndex<0?'m5stack-c050-b':process.argv[productIndex+1];
const binaryIndex=process.argv.indexOf('--binary'),binaryPath=binaryIndex<0?null:process.argv[binaryIndex+1];
if(binaryIndex>=0&&!binaryPath)throw Error('--binary requires an already downloaded original image.');
const originalBytes=binaryPath?await readFile(binaryPath):null;
if(originalBytes)assert.equal(createHash('sha256').update(originalBytes).digest('hex'),sha256,'The local original image must match the explicit source digest.');
const root=new URL('../',import.meta.url),baseUrl='https://stamppico-fixture.invalid/platform/';
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
assert.equal(catalog.products.find(p=>p.id===productId)?.runtime?.board,'stamppico','Select an explicitly mapped Stamp-Pico SKU.');
const fetchImpl=async (value,options)=>{if(originalBytes&&String(value)===url)return new Response(originalBytes);const target=new URL(value);if(target.origin!=='https://stamppico-fixture.invalid')return fetch(value,options);try{return new Response(await readFile(new URL('web'+target.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root);const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(message,...args)=>thread.postMessage(message.op==='create'&&process.argv.includes('--no-jit')?{...message,jit:false}:message,...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl,assetBaseUrl:'https://stamppico-fixture.invalid/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'stamppico-official-probe'})});
const seen=[];runtime.subscribe(e=>{if(e.type==='leds')seen.push(e.leds);});
const wait=async predicate=>{const end=Date.now()+120000;while(Date.now()<end){const state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(JSON.stringify(state.lastError));await new Promise(r=>setTimeout(r,30));}throw Error('Official probe timed out');};
try {
 await runtime.configure(createDefaultScene(catalog,productId));await runtime.loadFirmware({url,sha256});await runtime.start();
 const boot=await wait(s=>s.stats.time>=20);assert.equal(boot.frame,null);assert.ok(!boot.serial.includes('Guru Meditation'));
 const blue=JSON.stringify(boot.leds?.[0])==='[0,0,255]';
 // The official network/application behavior is not inferred from the blue LED.
 await runtime.input({type:'button',id:'button',pressed:true});const pressedAt=(await runtime.observe()).stats.time;await wait(s=>s.stats.time>pressedAt+1);await runtime.input({type:'button',id:'button',pressed:false});
 await runtime.pause();const state=await runtime.observe();
 console.log(JSON.stringify({checkedAt:new Date().toISOString(),command:'node tools/probe-stamppico-official.mjs <official-https-url> <sha256> --product <product-id>'+(binaryPath?' --binary <downloaded-original-image>':''),productId,url,sha256,transport:binaryPath?'caller-downloaded-original-image':'node-fetch-not-browser-cors',wasmSha256:createHash('sha256').update(await readFile(new URL('web/wasm/esp32sim.wasm',root))).digest('hex'),firmware:state.firmware,guestSeconds:state.stats.time,checks:['actual-page-worker-rom-startup',...(blue?['official-g27-rmt-blue']:[]),'no-invented-framebuffer','button-transport-only-not-official-application-acceptance'],leds:seen,serial:state.serial,status:blue?'pass':'partial',limitations:[...(!blue?['No expected blue RGB output after 20 guest seconds; full application startup is not established.']:[]),'Official application interaction and networking not verified; button electrical behavior is independently tested by source-built fixture.','Transport does not establish browser CORS access.']},null,2));
 if(!blue)process.exitCode=1;
} finally {await runtime.dispose();}
