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
import {installAutomation} from '../web/platform/automation.js';
const args=process.argv.slice(2),arg=name=>args[args.indexOf(name)+1];
for(const key of ['--url','--sha256'])assert(args.includes(key)&&arg(key),`Required ${key}; explicitly select the K159-B factory image.`);
const url=arg('--url'),expected=arg('--sha256'),root=new URL('../',import.meta.url),baseUrl='https://paperdiy-factory.invalid/platform/';
assert(/^https:\/\//.test(url));assert(/^[a-f0-9]{64}$/.test(expected));
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const response=await fetch(url,{signal:AbortSignal.timeout(45000)});assert(response.ok,`Firmware HTTP ${response.status}`);
const firmware=new Uint8Array(await response.arrayBuffer());assert.equal(hash(firmware),expected,'Changed firmware must be separately reviewed.');
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const fetchImpl=async value=>{const target=new URL(value);if(String(target)===url)return new Response(firmware);assert.equal(target.origin,'https://paperdiy-factory.invalid');try{return new Response(await readFile(new URL(target.pathname.endsWith('.elf')?'web/wasm/fw/'+basename(target.pathname):'web'+target.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root),thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl,assetBaseUrl:'https://paperdiy-factory.invalid/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'paperdiy-factory-probe'})});
const receipt={checkedAt:new Date().toISOString(),command:'node tools/probe-paperdiy-factory.mjs --url <catalog-k159-b-factory-url> --sha256 <catalog-sha256>',source:{url,resolvedUrl:response.url,bytes:firmware.length,sha256:hash(firmware)},backend:'actual-page-worker-wasm-jit',firmwarePatched:false,wasmSha256:hash(await readFile(new URL('web/wasm/esp32sim.wasm',root))),checks:[]};
const wait=async predicate=>{const end=Date.now()+180000;while(Date.now()<end){const state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(JSON.stringify(state.lastError));await new Promise(r=>setTimeout(r,50));}throw Error(`Factory probe timed out: ${(await runtime.observe()).serial.slice(-2000)}`);};
const advance=async seconds=>{const start=(await runtime.observe()).stats.time||0;return wait(s=>(s.stats.time||0)>=start+seconds);};
const save=async (name,frame)=>{if(args.includes('--output')){const {join}=await import('node:path');await mkdir(arg('--output'),{recursive:true});await writeFile(join(arg('--output'),`${name}.rgba`),frame.pixels);}return {width:frame.width,height:frame.height,sha256:hash(frame.pixels)};};
const pixel=(frame,x,y)=>Array.from(frame.pixels.slice((y*frame.width+x)*4,(y*frame.width+x)*4+4));
try {
 const scene=createDefaultScene(catalog,'m5stack-k159-b');scene.firmware={url,sha256:expected};await runtime.configure(scene);await runtime.loadFirmware();await runtime.start();
 console.error('phase: official first factory boot');await wait(s=>s.serial.includes('Sending M5PM1 shutdown command'));await advance(.4);await runtime.pause();
 const first=await runtime.capture({format:'rgba'});assert.equal(first.width,540);assert.equal(first.height,960);assert(first.pixels.some((v,i)=>i%4!==3&&v===0));receipt.first=await save('first',first);receipt.checks.push('rom-boot-first-factory-shutdown-logo');
 const storage=await runtime.flushDeviceStorage();assert(!storage.pristine);assert(storage.changedPages>0);receipt.changedPages=storage.changedPages;
 console.error('phase: power cycle retaining guest NVS');await runtime.reset();await wait(s=>s.serial.includes('Display ready: 540x960')&&s.stats.time>3);await runtime.pause();
 let state=await runtime.observe();assert(!state.serial.includes('First factory boot'),'NVS factory_done must survive reset');const main=await runtime.capture({format:'rgba'});assert.notDeepEqual(main.pixels,first.pixels);receipt.main=await save('main',main);receipt.checks.push('factory-nvs-retained-after-host-restart');await runtime.start();
 for(const [id,y] of [['button-a',620],['button-b',731]]) {
  console.error(`phase: ${id} held border`);await runtime.input({type:'button',id,pressed:true});await advance(.9);await runtime.pause();const held=await runtime.capture({format:'rgba'});
  assert.deepEqual(pixel(held,30,y),[0,0,0,255]);receipt[id]=await save(id,held);await runtime.start();await runtime.input({type:'button',id,pressed:false});await advance(.9);await runtime.pause();const released=await runtime.capture({format:'rgba'});
  assert.deepEqual(pixel(released,30,y),[255,255,255,255]);receipt.checks.push(`${id}-physical-gpio-held-black-released-white-border`);await runtime.start();
 }
 await runtime.pause();state=await runtime.observe();receipt.status='pass';receipt.guestSeconds=state.stats.time;receipt.artifacts=state.firmware.artifacts.map(({kind,sha256,bytes})=>({kind,sha256,bytes}));receipt.serialEvidence=[...new Set((state.serialByPort.uart0||'').split(/\r?\n/).filter(line=>/PaperDIY|Display ready|TF card|TF Card/.test(line)))];receipt.limitations=['Black/white digital terminal response only, not 16-gray optical calibration.','TF SPI, FPC, true power cycling/sleep and wireless networking are not validated. Host restart retains flash but is not a physical PMIC power button.','No physical hardware used. Host input duration is not a cycle-exact timing guarantee.'];console.log(JSON.stringify(receipt,null,2));
} finally {await runtime.dispose();}
