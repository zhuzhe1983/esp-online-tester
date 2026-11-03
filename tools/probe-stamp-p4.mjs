#!/usr/bin/env node
// Real source-built P4 firmware, platform API and worker/WASM. No output mocks.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {resolve,join,basename} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {inspectLocalBuild} from '../web/platform/local-firmware.js';
import {createDefaultScene} from '../web/platform/catalog.js';
const argument=name=>{const i=process.argv.indexOf(name);return i<0?null:process.argv[i+1];};
const build=argument('--build');if(!build)throw Error('Supply --build <ESP-IDF stamp-p4-firmware build directory>.');
const folder=resolve(build),root=new URL('../',import.meta.url),origin='https://stamp-p4-fixture.invalid',hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const files=[new File([await readFile(join(folder,'flasher_args.json'))],'flasher_args.json')],inputHashes={};
for(const path of (await inspectLocalBuild(files)).missingFiles){const bytes=await readFile(join(folder,path)),file=new File([bytes],basename(path));inputHashes[path]=hash(bytes);Object.defineProperty(file,'webkitRelativePath',{value:path});files.push(file);}
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const fetchImpl=async value=>{const url=new URL(value);assert.equal(url.origin,origin);try{return new Response(await readFile(new URL('web'+url.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{
 const real=new URL('web'+url.pathname,root),thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));
 const adapter={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;
};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'stamp-p4-probe'})});
const receipt={checkedAt:new Date().toISOString(),command:'node tools/probe-stamp-p4.mjs --build <fixture-build-directory>',backend:'actual-wasm-worker',firmware:'internal source-built execution fixture',sdk:JSON.parse(await readFile(join(folder,'config.env'))).IDF_VERSION,productId:'m5stack-s013',wasmSha256:hash(await readFile(new URL('web/wasm/esp32sim.wasm',root))),inputHashes,checks:[],outputs:[],limitations:['Not official UIFlow compatibility or physical hardware validation.','Rev < 3 virtual profile selected from official source configuration; physical module revision is unknown.','UART0 input is explicit logical injection, not USB signaling or GPIO-routed serial.','No display, mapped user buttons, external C6, MIPI/SDIO/USB peripherals or analog power model.']};
const delay=ms=>new Promise(r=>setTimeout(r,ms)),evidence=new Set();
const observe=async()=>{const state=await runtime.observe(),serial=state.serialByPort.uart0??state.serial;for(const line of serial.split(/\r?\n/).slice(0,-1))if(/^P4_/.test(line))evidence.add(line);return {...state,serial};};
let traced=0;
const wait=async(label,predicate)=>{const end=Date.now()+Number(argument('--timeout-ms')||120000);while(Date.now()<end){const state=await observe();if(process.argv.includes('--verbose')){if(state.serial.length<traced)traced=0;process.stderr.write(state.serial.slice(traced));traced=state.serial.length;}if(predicate(state))return state;if(state.status==='error')throw Error(JSON.stringify(state.lastError));await delay(20);}const state=await observe();throw Error(`${label} timed out; stats=${JSON.stringify(state.stats)}\n${state.serial.slice(-6000)}`);};
const challenge=async(value,count)=>{let x=value>>>0,sum=0;for(let i=0;i<4096;i++){x^=x<<13;x^=x>>>17;x^=x<<5;sum=(sum+(x>>>0))>>>0;}const marker=`P4_RESULT count=${count} input=${value.toString(16).padStart(8,'0')} sum=${sum.toString(16).padStart(8,'0')}`,at=(await observe()).serial.length;await runtime.input({type:'serial',port:'uart0',data:`C:${value.toString(16)}\n`});await wait(marker,s=>s.serial.slice(at).includes(marker));receipt.outputs.push({input:value,sum,count});};
try {
 const scene=createDefaultScene(catalog,'m5stack-s013');scene.firmware=null;await runtime.configure(scene);await runtime.loadFirmware({format:'build',files});await runtime.start();
 let state=await wait('P4 boot and PSRAM exercise',s=>s.serial.includes('P4_READY'));
 assert(state.serial.includes('P4_PSRAM bytes=33554432'));assert(state.serial.includes('P4_MEMORY_PASS bytes=2097152 stride=4096'));assert(state.serial.includes('cores=2 revision=100'));assert.equal(state.frame,null);
 const cap=await runtime.capabilities();assert.equal(cap.network.available,false);assert.equal(cap.audio.input.available,false);assert.equal(cap.lora.available,false);
 receipt.checks.push('actual-rev-lt3-P4-boot-two-cores-32MiB-PSRAM-and-2MiB-spanning-memory-exercise','no-inherited-display-network-audio-input-or-LoRa');
 await challenge(0x12345678,1);await challenge(0x9abcdef0,2);receipt.checks.push('two-fresh-UART0-challenges-execute-4096-word-PSRAM-write-read-and-independent-xorshift-checksum');
 let at=(await observe()).serial.length;await runtime.input({type:'serial',port:'uart0',data:'invalid\n'});await wait('malformed command rejected by guest',s=>s.serial.slice(at).includes('P4_REJECT'));receipt.checks.push('malformed-input-rejected-by-guest');
 await runtime.pause();const paused=await observe();await delay(100);assert.equal((await observe()).stats.time,paused.stats.time);assert.equal((await observe()).serial,paused.serial);receipt.checks.push('pause-freezes-guest-clock-and-output');
 await runtime.start();await challenge(0x87654321,3);
 at=(await observe()).serial.length;await runtime.input({type:'serial',port:'uart0',data:'B\n'});await wait('guest software restart',s=>{const fresh=s.serial.slice(at);return fresh.includes('P4_RESTART')&&fresh.includes('P4_READY');});await challenge(0x12345678,1);receipt.checks.push('guest-software-restart-reboots-and-resets-guest-counter');
 await runtime.reset();await wait('platform reset',s=>s.serial.includes('P4_READY'));await challenge(0x9abcdef0,1);receipt.checks.push('platform-reset-recreates-machine-and-reboots-fixture');
 await runtime.pause();receipt.serialEvidence=[...evidence];receipt.status='pass';console.log(JSON.stringify(receipt,null,2));
} catch(error) {receipt.status='failed';receipt.error=String(error.message).split('\n')[0];receipt.serialEvidence=[...evidence];console.log(JSON.stringify(receipt,null,2));throw error;} finally {await runtime.dispose();}
