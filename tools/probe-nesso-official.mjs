#!/usr/bin/env node
// Official remote UIFlow2-image probe; remote target/hash are explicit, no firmware is published.
import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createDefaultScene} from '../web/platform/catalog.js';
const [url,sha256]=process.argv.slice(2);
if(!/^https:\/\//.test(url||'')||!/^[a-f0-9]{64}$/.test(sha256||''))throw Error('Supply <official-https-url> <sha256>.');
const root=new URL('../',import.meta.url),baseUrl='https://nesso-fixture.invalid/platform/';
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const fetchImpl=async (value,options)=>{const url=new URL(value);if(url.origin!=='https://nesso-fixture.invalid')return fetch(value,options);try{return new Response(await readFile(new URL('web'+url.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root);const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(message,...args)=>thread.postMessage(message.op==='create'&&process.argv.includes('--no-jit')?{...message,jit:false}:message,...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl,assetBaseUrl:'https://nesso-fixture.invalid/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'nesso-official-probe'})});
const wait=async predicate=>{const end=Date.now()+30000;while(Date.now()<end){const state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(JSON.stringify(state.lastError));await new Promise(r=>setTimeout(r,30));}throw Error('Official probe timed out: '+JSON.stringify(await runtime.observe()));};
const frames=[];
const capture=async name=>{const f=await runtime.capture({format:'rgba'});const pixels=Buffer.from(f.pixels);const rgb=Buffer.alloc(f.width*f.height*3);for(let i=0;i<f.width*f.height;i++)pixels.copy(rgb,i*3,i*4,i*4+3);await mkdir(new URL('output/',root),{recursive:true});await writeFile(new URL(`output/nesso-${name}.ppm`,root),Buffer.concat([Buffer.from(`P6\n${f.width} ${f.height}\n255\n`),rgb]));frames.push({name,width:f.width,height:f.height,frameNumber:f.frameNumber,rgbaSha256:createHash('sha256').update(pixels).digest('hex')});};
const advance=async seconds=>{const at=(await runtime.observe()).stats.time;return wait(s=>s.stats.time>=at+seconds);};
const press=async id=>{const down=await runtime.input({type:'button',id,pressed:true});await wait(s=>s.stats.time>=down.guestTime+.3);const up=await runtime.input({type:'button',id,pressed:false});await wait(s=>s.stats.time>=up.guestTime+2);return {down:down.guestTime,up:up.guestTime};};
try {
 await runtime.configure(createDefaultScene(catalog,'m5stack-dk001'));await runtime.loadFirmware({url,sha256});await runtime.start();
 await wait(s=>s.stats.time>=3&&s.frame?.frameNumber>2);await capture('wasm-cloud');
 assert.deepEqual((await runtime.capabilities()).input.buttons,['btnA','btnB']);
 const next=await press('btnB');await capture('wasm-usb');const enter=await press('btnA');await capture('wasm-ready');
 assert.equal(new Set(frames.map(f=>f.rgbaSha256)).size,3,'Official display changes after each source-routed key');
 await runtime.input({type:'serial',port:'usb',data:'\x03'});await advance(.5);
 await runtime.input({type:'serial',port:'usb',data:'import M5; print("NESSO_REPL", M5.Touch.getCount())\r'});await wait(s=>s.serial.includes('NESSO_REPL 0'));
 await runtime.input({type:'touch',x:42,y:73,down:true});await advance(.2);
 await runtime.input({type:'serial',port:'usb',data:'M5.update(); print("NESSO_TOUCH", M5.Touch.getCount(), M5.Touch.getX(), M5.Touch.getY())\r'});await wait(s=>s.serial.includes('NESSO_TOUCH 1 42 72'));
 await runtime.input({type:'serial',port:'usb',data:'from machine import I2C, Pin; i=I2C(0,sda=Pin(10),scl=Pin(8)); print("NESSO_RAW", list(i.readfrom_mem(56,2,5)))\r'});await wait(s=>s.serial.includes('NESSO_RAW [1, 128, 42, 0, 73]'));
 await runtime.input({type:'touch',x:42,y:73,down:false});await advance(.2);await runtime.input({type:'serial',port:'usb',data:'import time; M5.update(); time.sleep_ms(30); M5.update(); time.sleep_ms(30); M5.update(); print("NESSO_RELEASE", M5.Touch.getCount())\r'});await wait(s=>s.serial.includes('NESSO_RELEASE 0'));
 await runtime.pause();const state=await runtime.observe();assert.equal(state.lastError,null);assert.ok(!state.serial.includes('Guru Meditation'));
 console.log(JSON.stringify({checkedAt:new Date().toISOString(),command:'node tools/probe-nesso-official.mjs <official-https-url> <sha256>',productId:'m5stack-dk001',url,sha256,wasmSha256:createHash('sha256').update(await readFile(new URL('web/wasm/esp32sim.wasm',root))).digest('hex'),firmware:state.firmware,guestSeconds:state.stats.time,keys:{next,enter},frames,checks:['actual-production-worker-C6-ROM-boot','official-CLOUD-to-USB-to-READY-through-expander-buttons','official-MicroPython-FT6336-touch-register-read'],serialEvidence:state.serial.split(/\r?\n/).filter(x=>/ESP-IDF:|NESSO_|MicroPython v|Guru Meditation/.test(x)),status:'pass',limitations:['Charger/fuel gauge absent: battery -1% is retained. LoRa RF, network, buzzer, power key and web IMU pose are unverified. Touch checked in the official firmware REPL; launcher itself has no touch callback.']},null,2));
} finally {await runtime.dispose();}
