#!/usr/bin/env node
// Official remote factory-image probe; remote target/hash are explicit, no firmware is published.
import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createDefaultScene} from '../web/platform/catalog.js';
const [url,sha256,productId='m5stack-k131']=process.argv.slice(2);
if(!['m5stack-k131','m5stack-k131-v11'].includes(productId))throw Error('Unsupported Air Quality product id.');
if(!/^https:\/\//.test(url||'')||!/^[a-f0-9]{64}$/.test(sha256||''))throw Error('Supply <official-https-url> <sha256>.');
const root=new URL('../',import.meta.url),baseUrl='https://airquality-fixture.invalid/platform/';
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const fetchImpl=async (value,options)=>{const url=new URL(value);if(url.origin!=='https://airquality-fixture.invalid')return fetch(value,options);try{return new Response(await readFile(new URL('web'+url.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root);const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(message,...args)=>thread.postMessage(message.op==='create'&&process.argv.includes('--no-jit')?{...message,jit:false}:message,...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl,assetBaseUrl:'https://airquality-fixture.invalid/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'airquality-official-probe'})});
const wait=async predicate=>{const end=Date.now()+90000;while(Date.now()<end){const state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(JSON.stringify(state.lastError));await new Promise(r=>setTimeout(r,30));}throw Error('Official probe timed out: '+JSON.stringify(await runtime.observe()));};
const frames=[];
const capture=async name=>{const f=await runtime.capture({format:'rgba'});const pixels=Buffer.from(f.pixels);const rgb=Buffer.alloc(f.width*f.height*3);for(let i=0;i<f.width*f.height;i++)pixels.copy(rgb,i*3,i*4,i*4+3);await mkdir(new URL('output/',root),{recursive:true});await writeFile(new URL(`output/airquality-${name}.ppm`,root),Buffer.concat([Buffer.from(`P6\n${f.width} ${f.height}\n255\n`),rgb]));frames.push({name,width:f.width,height:f.height,frameNumber:f.frameNumber,rgbaSha256:createHash('sha256').update(pixels).digest('hex')});};
const advance=async seconds=>{const at=(await runtime.observe()).stats.time;return wait(s=>s.stats.time>=at+seconds);};
const press=async id=>{const down=await runtime.input({type:'button',id,pressed:true});await wait(s=>s.stats.time>=down.guestTime+.3);const up=await runtime.input({type:'button',id,pressed:false});await wait(s=>s.stats.time>=up.guestTime+2);return {down:down.guestTime,up:up.guestTime};};
try {
 await runtime.configure(createDefaultScene(catalog,productId));await runtime.loadFirmware({url,sha256});await runtime.start();
 await wait(s=>s.stats.time>=16&&s.frame?.frameNumber>20);await capture('wasm-samples');
 assert.deepEqual((await runtime.capabilities()).input.buttons,['button-a','button-b']);
 const settings=await press('button-b');await capture('wasm-settings');const next=await press('button-b');await capture('wasm-setting-next');
 assert.equal(new Set(frames.map(f=>f.rgbaSha256)).size,3,'Official display changes after source-routed B presses');
 await runtime.pause();const state=await runtime.observe();assert.equal(state.lastError,null);assert.ok(!state.serial.includes('Guru Meditation'));
 console.log(JSON.stringify({checkedAt:new Date().toISOString(),command:'node tools/probe-airquality-official.mjs <official-https-url> <sha256> [product-id]',productId,url,sha256,wasmSha256:createHash('sha256').update(await readFile(new URL('web/wasm/esp32sim.wasm',root))).digest('hex'),firmware:state.firmware,guestSeconds:state.stats.time,keys:{settings,next},frames,checks:['actual-production-worker-S3-ROM-boot','official-SCD40-SEN55-samples-page','B-settings-and-selection-change'],serialEvidence:state.serial.split(/\r?\n/).filter(x=>/ESP-IDF:|Waiting for first measurement|Temperature Offset set|Guru Meditation/.test(x)),status:'pass',limitations:['RTC/battery/audio/network/power-sleep are not modeled. Samples are explicitly virtual constants; conversion/warmup timing is not physically calibrated.']},null,2));
} finally {await runtime.dispose();}
