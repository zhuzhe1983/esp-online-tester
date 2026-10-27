#!/usr/bin/env node
// Run the source-built internal Atom-Matrix fixture on the actual page worker/WASM path.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {resolve,join,basename} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {inspectLocalBuild} from '../web/platform/local-firmware.js';
import {createDefaultScene} from '../web/platform/catalog.js';
const index=process.argv.indexOf('--build');if(index<0||!process.argv[index+1])throw Error('Build tools/fixtures/atom-matrix-firmware with ESP-IDF 5.5.1 and supply --build <build-directory>.');
const folder=resolve(process.argv[index+1]),root=new URL('../',import.meta.url),baseUrl='https://atom-matrix-fixture.invalid/platform/';
const files=[new File([await readFile(join(folder,'flasher_args.json'))],'flasher_args.json')],inputHashes={};
for(const path of (await inspectLocalBuild(files)).missingFiles){const bytes=await readFile(join(folder,path)),file=new File([bytes],basename(path));inputHashes[path]=createHash('sha256').update(bytes).digest('hex');Object.defineProperty(file,'webkitRelativePath',{value:path});files.push(file);}
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const fetchImpl=async value=>{const url=new URL(value);assert.equal(url.origin,'https://atom-matrix-fixture.invalid');try{return new Response(await readFile(new URL('web'+url.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root);const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(message,...args)=>thread.postMessage(message.op==='create'&&process.argv.includes('--no-jit')?{...message,jit:false}:message,...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl,assetBaseUrl:'https://atom-matrix-fixture.invalid/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'atom-matrix-probe'})});
const receipt={checkedAt:new Date().toISOString(),command:'node tools/probe-atom-matrix.mjs --build <fixture-build-directory>',backend:'actual-wasm-worker',firmware:'internal ESP-IDF 5.5.1 protocol fixture',inputHashes,checks:[]};
const wait=async predicate=>{const end=Date.now()+90000;while(Date.now()<end){const state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(JSON.stringify(state.lastError));await new Promise(r=>setTimeout(r,50));}throw Error(`Fixture timed out LEDs=${JSON.stringify(seen)}: ${(await runtime.observe()).serial.slice(-1000)}`);};
const seen=[];runtime.subscribe(e=>{if(e.type==='leds')seen.push(e.leds);});
const color=s=>s.leds?.[0];
try {
 const scene=createDefaultScene(catalog,'m5stack-c008-b');scene.firmware=null;await runtime.configure(scene);await runtime.loadFirmware({format:'build',files});await runtime.start();
 await wait(s=>s.serial.includes('ATOM_MATRIX_IMU_ID 19')&&s.serial.includes('ATOM_MATRIX_BUTTON 1')&&JSON.stringify(color(s))==='[255,0,0]');
 assert.deepEqual((await runtime.observe()).leds[24],[0,0,255]);receipt.checks.push('classic-rom-boot-rmt27-first-red-last-blue-mpu-id19');
 const at=(await runtime.observe()).serial.length;await runtime.input({type:'button',id:'button',pressed:true});await wait(s=>s.serial.slice(at).includes('ATOM_MATRIX_BUTTON 0')&&JSON.stringify(color(s))==='[0,255,0]');
 await runtime.input({type:'button',id:'button',pressed:false});await wait(s=>/ATOM_MATRIX_BUTTON 0[\s\S]*ATOM_MATRIX_BUTTON 1/.test(s.serial.slice(at))&&JSON.stringify(color(s))==='[255,0,0]');receipt.checks.push('guest-g39-press-release-first-green-red');
 await runtime.setImu({quaternion:[0,0,0,1],source:'agent'});await wait(s=>s.serial.includes('ATOM_MATRIX_ACCEL 0 16384 0'));
 await runtime.setImu({quaternion:[Math.SQRT1_2,0,0,Math.SQRT1_2],source:'scene'});await wait(s=>s.serial.includes('ATOM_MATRIX_ACCEL 0 0 -16384'));receipt.checks.push('real-i2c-sda25-scl21-imu-tracks-quaternion');
 await assert.rejects(runtime.input({type:'button',id:'unknown',pressed:true}));assert.equal((await runtime.observe()).frame,null);
 await runtime.input({type:'button',id:'button',pressed:true});await wait(s=>JSON.stringify(color(s))==='[0,255,0]');await runtime.reset();await runtime.start();
 const state=await wait(s=>s.serial.includes('ATOM_MATRIX_BUTTON 1')&&JSON.stringify(color(s))==='[255,0,0]');await runtime.pause();receipt.checks.push('platform-reset-releases-button-and-fresh-guest-red');
 receipt.status='pass';receipt.guestSeconds=state.stats.time;receipt.leds=seen;receipt.serialEvidence=state.serial.split(/\r?\n/).filter(l=>/^ATOM_MATRIX_/.test(l));console.log(JSON.stringify(receipt,null,2));
} finally {await runtime.dispose();}
