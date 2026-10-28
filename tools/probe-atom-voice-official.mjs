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
const root=new URL('../',import.meta.url),baseUrl='https://atom-voice-fixture.invalid/platform/';
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const fetchImpl=async (value,options)=>{const url=new URL(value);if(url.origin!=='https://atom-voice-fixture.invalid')return fetch(value,options);try{return new Response(await readFile(new URL('web'+url.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root);const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(message,...args)=>thread.postMessage(message.op==='create'&&process.argv.includes('--no-jit')?{...message,jit:false}:message,...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl,assetBaseUrl:'https://atom-voice-fixture.invalid/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'atom-voice-official-probe'})});
const seen=[];runtime.subscribe(e=>{if(e.type==='leds')seen.push(e.leds);});
const wait=async predicate=>{const end=Date.now()+30000;while(Date.now()<end){const state=await runtime.observe();if(process.argv.includes('--trace'))console.error(JSON.stringify({t:state.stats.time,leds:state.leds,serial:state.serial.slice(-200)}));if(predicate(state))return state;if(state.status==='error')throw Error(JSON.stringify(state.lastError));await new Promise(r=>setTimeout(r,30));}const last=await runtime.observe();throw Error('Official probe timed out '+JSON.stringify({time:last.stats.time,leds:last.leds,serial:last.serial.slice(-4000)}));};
try {
 await runtime.configure(createDefaultScene(catalog,'m5stack-c008-c'));await runtime.loadFirmware({url,sha256});await runtime.start();
 const state=await wait(s=>s.serial.includes('StoreProhibited')&&seen.some(leds=>JSON.stringify(leds[0])==='[5,0,0]'));
 await runtime.pause();assert.equal(state.frame,null);assert.ok(seen.some(leds=>JSON.stringify(leds[0])==='[5,0,0]'));
 assert.ok(state.serial.includes('BTDM_INIT'));assert.ok(state.serial.includes('0x0000019c'));
 console.log(JSON.stringify({checkedAt:new Date().toISOString(),command:'node tools/probe-atom-voice-official.mjs <official-https-url> <sha256>',productId:'m5stack-c008-c',url,sha256,wasmSha256:createHash('sha256').update(await readFile(new URL('web/wasm/esp32sim.wasm',root))).digest('hex'),firmware:state.firmware,guestSeconds:state.stats.time,leds:state.leds,ledUpdateEvents:seen.length,checks:['page-worker-full-rom-boot','actual-g27-rmt-red-5-0-0','no-invented-framebuffer','known-classic-bt-startup-panic-retained'],serialEvidence:[...new Set(state.serial.split(/\r?\n/))].filter(x=>/BT controller compile version|StoreProhibited|EXCVADDR:|Rebooting/.test(x)),status:'partial-known-failure',limitations:['The unmodified factory image initializes the real LED then faults at classic Bluetooth startup; A2DP/audio/microphone are not supported. G39 is not consumed by this factory source.']},null,2));

} finally {await runtime.dispose();}
