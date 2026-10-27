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
const root=new URL('../',import.meta.url),baseUrl='https://atom-lite-fixture.invalid/platform/';
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const fetchImpl=async (value,options)=>{const url=new URL(value);if(url.origin!=='https://atom-lite-fixture.invalid')return fetch(value,options);try{return new Response(await readFile(new URL('web'+url.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root);const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(message,...args)=>thread.postMessage(message.op==='create'&&process.argv.includes('--no-jit')?{...message,jit:false}:message,...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl,assetBaseUrl:'https://atom-lite-fixture.invalid/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'atom-lite-official-probe'})});
const seen=[];runtime.subscribe(e=>{if(e.type==='leds')seen.push(e.leds);});
const wait=async predicate=>{const end=Date.now()+120000;while(Date.now()<end){const state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(JSON.stringify(state.lastError));await new Promise(r=>setTimeout(r,30));}throw Error('Official probe timed out');};
try {
 await runtime.configure(createDefaultScene(catalog,'m5stack-c008'));await runtime.loadFirmware({url,sha256});await runtime.start();
 const boot=await wait(s=>s.stats.time>=2&&s.serial.includes('M5Atom initializing...OK'));assert.equal(boot.frame,null);assert.ok(!boot.serial.includes('Guru Meditation'));assert.deepEqual(boot.leds?.[0],[0,0,0]);
 await runtime.input({type:'button',id:'button',pressed:true});await wait(s=>s.serial.includes('Testing grove gpio32:')&&JSON.stringify(s.leds?.[0])==='[20,0,0]');
 await runtime.input({type:'button',id:'button',pressed:false});const at=(await runtime.observe()).stats.time;await wait(s=>s.stats.time>at+.2);
 await runtime.pause();const state=await runtime.observe();
 console.log(JSON.stringify({checkedAt:new Date().toISOString(),command:'node tools/probe-atom-lite-official.mjs <official-https-url> <sha256>',productId:'m5stack-c008',url,sha256,wasmSha256:createHash('sha256').update(await readFile(new URL('web/wasm/esp32sim.wasm',root))).digest('hex'),firmware:state.firmware,bootSerial:boot.serial.split(/\r?\n/).filter(x=>x.includes('M5Atom initializing')),guestSeconds:state.stats.time,checks:['actual-page-worker-full-rom-boot','official-g27-rmt-red-after-g39-press','no-invented-framebuffer','official-g39-enters-grove-test'],leds:seen,serialEvidence:[...new Set(state.serial.split(/\r?\n/))].filter(x=>/M5Atom initializing|Testing grove|Guru Meditation/.test(x)),status:'pass',limitations:['Official startup/G39/RMT subset only; Grove fixture inputs, IR and physical network unverified. This binary differs from public rainbow source.']},null,2));
} finally {await runtime.dispose();}
