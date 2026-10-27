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
const root=new URL('../',import.meta.url),baseUrl='https://nanoc6-fixture.invalid/platform/';
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const fetchImpl=async (value,options)=>{const url=new URL(value);if(url.origin!=='https://nanoc6-fixture.invalid')return fetch(value,options);try{return new Response(await readFile(new URL('web'+url.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root);const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(message,...args)=>thread.postMessage(message.op==='create'&&process.argv.includes('--no-jit')?{...message,jit:false}:message,...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl,assetBaseUrl:'https://nanoc6-fixture.invalid/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'nanoc6-official-probe'})});
const seen=[];runtime.subscribe(e=>{if(e.type==='leds')seen.push(e.leds);});
const wait=async predicate=>{const end=Date.now()+120000;while(Date.now()<end){const state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(JSON.stringify(state.lastError));await new Promise(r=>setTimeout(r,30));}throw Error('Official probe timed out: '+JSON.stringify(await runtime.observe()));};
try {
 await runtime.configure(createDefaultScene(catalog,'m5stack-c125'));await runtime.loadFirmware({url,sha256});await runtime.start();
 const boot=await wait(s=>s.stats.time>=3&&s.serial.includes('nanoc6_factorytest'));assert.equal(boot.frame,null);assert.ok(!boot.serial.includes('Guru Meditation'));assert.equal(boot.leds.length,2);
 const at=boot.serial.length;await runtime.input({type:'button',id:'button',pressed:true});await wait(s=>s.stats.time>=boot.stats.time+.35);await runtime.input({type:'button',id:'button',pressed:false});const first=await wait(s=>s.serial.slice(at).includes('example: status: 1'));
 await runtime.input({type:'button',id:'button',pressed:false});await wait(s=>s.stats.time>=first.stats.time+.4);
 const next=(await runtime.observe()).serial.length;const secondAt=(await runtime.observe()).stats.time;await runtime.input({type:'button',id:'button',pressed:true});await wait(s=>s.stats.time>=secondAt+.35);await runtime.input({type:'button',id:'button',pressed:false});const second=await wait(s=>s.serial.slice(next).includes('example: status: 0'));await runtime.input({type:'button',id:'button',pressed:false});
 await wait(s=>s.stats.time>=second.stats.time+.5);await runtime.pause();const state=await runtime.observe();
 assert.ok(seen.some(x=>JSON.stringify(x[0])==='[255,0,0]'));assert.ok(seen.some(x=>JSON.stringify(x[0])==='[0,255,0]'));assert.ok(seen.some(x=>JSON.stringify(x[0])==='[0,0,255]'));assert.ok(seen.some(x=>x[1]?.[2]===255));assert.ok(seen.some(x=>x[1]?.[2]===0));
 console.log(JSON.stringify({checkedAt:new Date().toISOString(),command:'node tools/probe-nanoc6-official.mjs <official-https-url> <sha256>',productId:'m5stack-c125',url,sha256,wasmSha256:createHash('sha256').update(await readFile(new URL('web/wasm/esp32sim.wasm',root))).digest('hex'),firmware:state.firmware,guestSeconds:state.stats.time,inputGuestSeconds:[first.stats.time,second.stats.time],checks:['actual-page-worker-full-c6-rom-boot','official-g20-rmt-rgb-cycle-g19-power','independent-g7-blue-led','g9-two-press-release-cycles-toggle-official-status-1-0','no-invented-framebuffer'],leds:seen,serialEvidence:[...new Set(state.serial.split(/\r?\n/))].filter(x=>/Project name|App version|ESP-IDF:|example: status|SH1107:|esp_timer: Task|Guru Meditation/.test(x)),status:'pass',limitations:['IR optical output, wireless stacks, physical Grove routing and complete factory checks unverified.']},null,2));
} finally {await runtime.dispose();}
