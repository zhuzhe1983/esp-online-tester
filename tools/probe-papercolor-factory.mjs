#!/usr/bin/env node
// Caller-selected official image: bytes stay in RAM and execute in the actual page worker.
import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {basename} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createDefaultScene} from '../web/platform/catalog.js';
const args=process.argv.slice(2),arg=name=>args[args.indexOf(name)+1];
for(const key of ['--url','--sha256'])assert(args.includes(key)&&arg(key),`Required ${key}; explicitly select the C151 factory image.`);
const url=arg('--url'),expected=arg('--sha256'),root=new URL('../',import.meta.url),baseUrl='https://papercolor-factory.invalid/platform/';
assert(/^https:\/\//.test(url));assert(/^[a-f0-9]{64}$/.test(expected));
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const response=await fetch(url,{signal:AbortSignal.timeout(45000)});assert(response.ok,`Firmware HTTP ${response.status}`);
const firmware=new Uint8Array(await response.arrayBuffer());assert.equal(hash(firmware),expected,'Changed firmware must be separately reviewed.');
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const fetchImpl=async value=>{const target=new URL(value);if(String(target)===url)return new Response(firmware);assert.equal(target.origin,'https://papercolor-factory.invalid');try{return new Response(await readFile(new URL(target.pathname.endsWith('.elf')?'web/wasm/fw/'+basename(target.pathname):'web'+target.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root),thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const ledEvents=[];let guestTime=0;
const runtime=createPlatformRuntime({onEvent:event=>{if(event.type==='stats')guestTime=event.stats.time;if(event.type==='leds')ledEvents.push({guestTime,leds:event.leds});},catalog,fetchImpl,workerFactory,baseUrl,assetBaseUrl:'https://papercolor-factory.invalid/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'papercolor-factory-probe'})});
const receipt={checkedAt:new Date().toISOString(),command:'node tools/probe-papercolor-factory.mjs --url <catalog-c151-factory-url> --sha256 <catalog-sha256>',source:{url,resolvedUrl:response.url,bytes:firmware.length,sha256:hash(firmware)},backend:'actual-page-worker-wasm-jit',firmwarePatched:false,wasmSha256:hash(await readFile(new URL('web/wasm/esp32sim.wasm',root))),checks:[]};
const wait=async predicate=>{const end=Date.now()+180000;while(Date.now()<end){const state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(JSON.stringify(state.lastError));await new Promise(r=>setTimeout(r,50));}throw Error(`Factory probe timed out: ${(await runtime.observe()).serial.slice(-2000)}`);};
const advance=async seconds=>{const start=(await runtime.observe()).stats.time||0;return wait(s=>(s.stats.time||0)>=start+seconds);};
const save=async (name,frame)=>{if(args.includes('--output')){const {join}=await import('node:path');await mkdir(arg('--output'),{recursive:true});await writeFile(join(arg('--output'),`${name}.rgba`),frame.pixels);}return {width:frame.width,height:frame.height,sha256:hash(frame.pixels)};};

try {
 const scene=createDefaultScene(catalog,'m5stack-c151');scene.firmware={url,sha256:expected};await runtime.configure(scene);await runtime.loadFirmware();await runtime.start();
 console.error('phase: official first factory boot');await wait(s=>s.serial.includes('Shutdown requested'));await advance(.4);await runtime.pause();
 const first=await runtime.capture({format:'rgba'});assert.equal(first.width,400);assert.equal(first.height,600);assert(first.pixels.some((v,i)=>i%4!==3&&v===0));receipt.first=await save('first',first);receipt.checks.push('rom-boot-first-factory-shutdown-logo');
 const storage=await runtime.flushDeviceStorage();assert(!storage.pristine);assert(storage.changedPages>0);receipt.changedPages=storage.changedPages;
 console.error('phase: power cycle retaining guest NVS');await runtime.reset();await advance(15);await runtime.pause();
 let state=await runtime.observe();assert(!state.serial.includes('Show the factory boot guide image'),'Factory NVS flag must survive reset');assert(state.serial.includes('papercolor server ready'));
 const second=await runtime.capture({format:'rgba'});assert.notDeepEqual(first.pixels,second.pixels);receipt.second=await save('second',second);receipt.checks.push('retained-factory-NVS-restart-to-guest-AP-QR-ui');
 await runtime.start();const from=ledEvents.length;const down=await runtime.input({type:'button',id:'button-c',pressed:true});await advance(7);const up=await runtime.input({type:'button',id:'button-c',pressed:false});await wait(s=>s.serial.includes('AP re-enabled by long press'));await advance(3);await runtime.pause();
 assert(ledEvents.slice(from).some(e=>e.leds.length===2&&e.leds.every(c=>c[0]===0&&c[1]>10&&c[2]===0)),'Official top-key hold must produce green confirmation through RMT');
 receipt.checks.push('top-C-G1-long-hold-real-RMT-green-confirmation-and-official-AP-reenable');receipt.input={id:'button-c',down,up};receipt.longHold=await save('long-hold',await runtime.capture({format:'rgba'}));state=await runtime.observe();
 receipt.ledColors=[...new Set(ledEvents.map(e=>JSON.stringify(e.leds)))].map(JSON.parse);receipt.status='pass';receipt.guestSeconds=state.stats.time;receipt.artifacts=state.firmware.artifacts.map(({kind,sha256,bytes})=>({kind,sha256,bytes}));
 receipt.serialEvidence=[...new Set((state.serialByPort.uart0||'').split(/\r?\n/).filter(line=>/board_M5PaperColor|Software version:|AP re-enabled by long press|papercolor server ready|Refresh start|Refresh complete/.test(line)))];
 receipt.limitations=['Actual guest AP stack/QR UI starts, but no host Wi-Fi bridge, RF or portal exchange was verified.','Ideal six-color digital output and bounded BUSY, not optical refresh calibration. SHT40 environment is synthetic; RTC is register RAM.','TF reports absent; audio, optional Stamp1262 LoRa, PMIC physical power button and RTC wake are not modeled. A/B physical GPIO paths are unit-tested; this factory no-mode screen has no verified A/B action.','No physical hardware; 3D RGB chain order is not calibrated. Input durations use host delivery, not cycle-exact reproduction.'];console.log(JSON.stringify(receipt,null,2));
} finally {await runtime.dispose();}
