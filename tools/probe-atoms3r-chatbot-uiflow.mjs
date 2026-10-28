#!/usr/bin/env node
// Official remote UIFlow2-image probe; remote target/hash are explicit, no firmware is published.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createDefaultScene} from '../web/platform/catalog.js';
const [url,sha256]=process.argv.slice(2);
if(!/^https:\/\//.test(url||'')||!/^[a-f0-9]{64}$/.test(sha256||''))throw Error('Supply <official-https-url> <sha256>.');
const root=new URL('../',import.meta.url),baseUrl='https://chatbot-fixture.invalid/platform/';
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const fetchImpl=async (value,options)=>{const url=new URL(value);if(url.origin!=='https://chatbot-fixture.invalid')return fetch(value,options);try{return new Response(await readFile(new URL('web'+url.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root);const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(message,...args)=>thread.postMessage(message.op==='create'&&process.argv.includes('--no-jit')?{...message,jit:false}:message,...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl,assetBaseUrl:'https://chatbot-fixture.invalid/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'chatbot-official-probe'})});
const wait=async predicate=>{const end=Date.now()+90000;while(Date.now()<end){const state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(JSON.stringify(state.lastError));await new Promise(r=>setTimeout(r,30));}throw Error(`Guest condition timed out: ${(await runtime.observe()).serial.slice(-2000)}`);};
const hash=b=>createHash('sha256').update(b).digest('hex');
const labelBand=frame=>hash(frame.pixels.subarray(40*128*4,56*128*4));
try {
 const scene=createDefaultScene(catalog,'m5stack-k147');scene.firmware={url,sha256};await runtime.configure(scene);await runtime.loadFirmware();await runtime.start();await wait(s=>s.stats.time>=4);await runtime.pause();const before=await runtime.capture({format:'rgba'});assert.equal(before.width,128);assert.equal(before.height,128);
 await runtime.start();const t=(await runtime.observe()).stats.time;const press=await runtime.input({type:'button',id:'button-a',pressed:true});await wait(s=>s.stats.time>t+.2);const release=await runtime.input({type:'button',id:'button-a',pressed:false});await wait(s=>s.stats.time>t+.5);await runtime.pause();const after=await runtime.capture({format:'rgba'});
 assert.equal(labelBand(before),'69d15874d5bdd100e9480a52c928c3e107930aa9e00f1eaab2b9037550b95a69','Expected original UIFlow Access Code label band');assert.equal(labelBand(after),'15c15934b982597e026b397ebd6aa2b3aa57789311829e40336a62cb47f6fc5e','Expected original UIFlow IP Address label band after G41 press/release');
 const state=await runtime.observe();console.log(JSON.stringify({checkedAt:new Date().toISOString(),productId:state.product.id,source:{url,sha256},wasmSha256:hash(await readFile(new URL('web/wasm/esp32sim.wasm',root))),backend:'actual-page-worker-wasm',firmwarePatched:false,inputs:{press,release},guestSeconds:state.stats.time,frame:{width:after.width,height:after.height,beforeSha256:hash(before.pixels),afterSha256:hash(after.pixels),beforeLabelBandSha256:labelBand(before),afterLabelBandSha256:labelBand(after)},checks:['original-official-uiflow-ui','guest-gc9107-framebuffer-128x128','guest-g41-page-change-access-code-to-ip-address'],status:'pass',privacy:'Only image hashes retained for IP/MAC information page; raw identifying page is not published. Label bands cover y40..55 only, above the identity value.',limitations:['Wi-Fi/cloud remain NG; no ES8311/audio/IR/network/BMM150 support.','No physical hardware; BMI270 input and REPL not part of this acceptance.']},null,2));
}finally{await runtime.dispose();}
