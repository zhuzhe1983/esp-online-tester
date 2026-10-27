#!/usr/bin/env node
// Caller-selected official image: bytes stay in RAM and execute in the actual page worker.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {basename} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {installAutomation} from '../web/platform/automation.js';
const args=process.argv.slice(2),arg=name=>args[args.indexOf(name)+1];
for(const key of ['--url','--sha256'])assert(args.includes(key)&&arg(key),`Required ${key}; explicitly select the original K134 factory image.`);
const url=arg('--url'),expected=arg('--sha256'),root=new URL('../',import.meta.url),baseUrl='https://dinmeter-factory.invalid/platform/';
assert(/^https:\/\//.test(url));assert(/^[a-f0-9]{64}$/.test(expected));
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const response=await fetch(url,{signal:AbortSignal.timeout(45000)});assert(response.ok,`Firmware HTTP ${response.status}`);
const firmware=new Uint8Array(await response.arrayBuffer());assert.equal(hash(firmware),expected,'Changed firmware must be separately reviewed.');
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const fetchImpl=async value=>{const target=new URL(value);if(String(target)===url)return new Response(firmware);assert.equal(target.origin,'https://dinmeter-factory.invalid');try{return new Response(await readFile(new URL(target.pathname.endsWith('.elf')?'web/wasm/fw/'+basename(target.pathname):'web'+target.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root),thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl,assetBaseUrl:'https://dinmeter-factory.invalid/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'dinmeter-factory-probe'})});
const receipt={checkedAt:new Date().toISOString(),command:'node tools/probe-dinmeter-factory.mjs --url <catalog-original-k134-factory-url> --sha256 <catalog-sha256>',source:{url,resolvedUrl:response.url,bytes:firmware.length,sha256:hash(firmware)},backend:'actual-page-worker-wasm-jit',firmwarePatched:false,wasmSha256:hash(await readFile(new URL('web/wasm/esp32sim.wasm',root))),checks:[]};
const wait=async predicate=>{const end=Date.now()+90000;while(Date.now()<end){const state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(JSON.stringify(state.lastError));await new Promise(r=>setTimeout(r,50));}throw Error(`Factory probe timed out: ${(await runtime.observe()).serial.slice(-2000)}`);};
const advance=async seconds=>{const start=(await runtime.observe()).stats.time||0;return wait(s=>(s.stats.time||0)>=start+seconds);};
try {
 const scene=createDefaultScene(catalog,'m5stack-k134');scene.firmware={url,sha256:expected};await runtime.configure(scene);await runtime.loadFirmware();await runtime.start();await advance(2);
 await runtime.pause();const menu=await runtime.capture({format:'rgba'});assert.equal(menu.width,240);assert.equal(menu.height,135);assert(new Set(menu.pixels).size>10,'Expected a real nonblank factory framebuffer');receipt.menuFrameSha256=hash(menu.pixels);receipt.checks.push('rom-boot-nonblank-240x135-menu');await runtime.start();
 // Fixed factory source view.cpp: DISPLAY, BRIGHTNESS, RTC, WIFI, ENCODER.
 // Each complete positive phase cycle is sampled separately by the guest menu.
 for(let i=0;i<4;i++){await runtime.input({type:'encoder',id:'encoder',steps:1});await advance(.8);}
 const api=installAutomation(runtime,{target:{location:{href:baseUrl}}}),before=(await runtime.observe()).serial.length;
 await api.press('encoder-button',{durationMs:150});await wait(s=>s.serial.slice(before).includes('encoder test'));await advance(.4);await runtime.pause();const zero=await runtime.capture({format:'rgba'});receipt.zeroFrameSha256=hash(zero.pixels);await runtime.start();
 await runtime.input({type:'encoder',id:'encoder',steps:1});await advance(.4);await runtime.pause();const changed=await runtime.capture({format:'rgba'});assert.notDeepEqual(changed.pixels,zero.pixels);receipt.plusOneFrameSha256=hash(changed.pixels);await runtime.start();
 await runtime.input({type:'encoder',id:'encoder',steps:-1});await advance(.4);await runtime.pause();const restored=await runtime.capture({format:'rgba'});assert.deepEqual(restored.pixels,zero.pixels,'Reverse step must restore the factory encoder display');receipt.checks.push('four-real-pcnt-detents-and-gpio42-enter-encoder-test','encoder-plus1-changes-frame-minus1-restores-every-pixel');await runtime.start();
 const exit=(await runtime.observe()).serial.length;await api.press('encoder-button',{durationMs:150});await wait(s=>s.serial.slice(exit).includes('quit encoder test'));await runtime.pause();const state=await runtime.observe();receipt.checks.push('gpio42-exits-encoder-test');receipt.status='pass';receipt.guestSeconds=state.stats.time;receipt.artifacts=state.firmware.artifacts.map(({kind,sha256,bytes})=>({kind,sha256,bytes}));receipt.serialEvidence=[...new Set((state.serialByPort.uart0||'').split(/\r?\n/).filter(line=>/encoder test|not long pressed|i2cWriteReadNonStop/.test(line)))];receipt.limitations=['RTC NACK is retained. Brightness PWM, battery, audio, power/sleep, Grove and networking are not validated.','Frame text was separately reviewed in browser screenshots; this script asserts guest menu serial and reversible pixel changes, not OCR or physical hardware behavior.'];console.log(JSON.stringify(receipt,null,2));
} finally {await runtime.dispose();}
