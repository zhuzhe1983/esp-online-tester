#!/usr/bin/env node
// Run the source-built internal Unit Hub U006 fixture on the actual page worker/WASM path.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {resolve,join,basename} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {installAutomation} from '../web/platform/automation.js';
const index=process.argv.indexOf('--build');if(index<0||!process.argv[index+1])throw Error('Build tools/fixtures/unit-hub-firmware with ESP-IDF 5.5.1 and supply --build <build-directory>.');
const folder=resolve(process.argv[index+1]),root=new URL('../',import.meta.url),baseUrl='https://unit-hub-fixture.invalid/platform/';
const artifacts=new Map(),inputHashes={};
const layout=JSON.parse(await readFile(join(folder,'flasher_args.json')));
const manifest={board:'stopwatch',flash_mb:16,psram_mb:8,files:{rom:'/wasm/fw/esp32s3_rev0_rom.elf'},artifact_sha256:{}};
for(const [kind,key] of [['bootloader','bootloader'],['ptable','partition-table'],['app','app']]){
  const path=layout[key].file,bytes=await readFile(join(folder,path)),url='/fixture/'+basename(path);artifacts.set(url,bytes);manifest.files[kind]=url;inputHashes[path]=createHash('sha256').update(bytes).digest('hex');manifest.artifact_sha256[url]=inputHashes[path];
}
const manifestBytes=Buffer.from(JSON.stringify(manifest));artifacts.set('/fixture/manifest.json',manifestBytes);
const source={manifestUrl:'https://unit-hub-fixture.invalid/fixture/manifest.json',sha256:createHash('sha256').update(manifestBytes).digest('hex')};
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const fetchImpl=async value=>{const url=new URL(value);assert.equal(url.origin,'https://unit-hub-fixture.invalid');try{if(artifacts.has(url.pathname))return new Response(artifacts.get(url.pathname));return new Response(await readFile(new URL('web'+url.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root);const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl,assetBaseUrl:'https://unit-hub-fixture.invalid/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'unit-hub-probe'})});
const sourceSha256=Object.fromEntries(await Promise.all(['web/wasm/worker.js','web/platform/accessory-routing.js','web/platform/catalog.js','web/platform/runtime.js','web/platform/recording.js','tools/probe-unit-hub.mjs','tools/fixtures/unit-hub-firmware/main/main.c'].map(async path=>[path,createHash('sha256').update(await readFile(new URL(path,root))).digest('hex')])));
const receipt={checkedAt:new Date().toISOString().replace(/\.\d{3}Z$/,'Z'),sourceSha256,wasmSha256:createHash('sha256').update(await readFile(new URL('web/wasm/esp32sim.wasm',root))).digest('hex'),romSha256:createHash('sha256').update(await readFile(new URL('web/wasm/fw/esp32s3_rev0_rom.elf',root))).digest('hex'),command:'node tools/probe-unit-hub.mjs --build <fixture-build-directory>',backend:'actual-wasm-worker',firmware:'internal ESP-IDF 5.5.1 protocol fixture',inputHashes,checks:[]};
const wait=async predicate=>{const end=Date.now()+90000;while(Date.now()<end){const state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(JSON.stringify(state.lastError));await new Promise(r=>setTimeout(r,50));}throw Error(`Fixture timed out: ${(await runtime.observe()).serial.slice(-4000)}`);};
const check=message=>{receipt.checks.push(message);console.error('ok '+message);};
const api=installAutomation(runtime,{target:{}});
const black=[[0,0,0],[0,0,0],[0,0,0]],a=[[255,0,0],[0,128,0],[0,0,64]],b=[[0,128,0],[0,0,64],[255,0,0]];
const equal=(x,y)=>JSON.stringify(x)===JSON.stringify(y),colors=s=>s.accessoryOutputs.find(o=>o.accessoryId==='rgb-unit')?.leds;
const send=async(data,marker)=>{const at=(await runtime.observe()).serial.length;await runtime.input({type:'serial',port:'uart0',data});return wait(s=>s.serial.slice(at).includes(marker));};
const sample=async expected=>{const state=await send('q','U006_INPUT ');const matches=[...state.serial.matchAll(/U006_INPUT white11=(\d+) other12=(\d+)/g)];const result=matches.at(-1).slice(1).map(Number);assert.deepEqual(result,[expected,0]);return result;};
const connect=(id,connected)=>runtime.input({type:'accessory-connection',accessoryId:id,connected});
const press=pressed=>runtime.input({type:'accessory-button',accessoryId:'button-unit',id:'button',pressed});
const scene=createDefaultScene(catalog,'m5stack-stopwatch');scene.firmware=source;
scene.accessories=[{id:'hub',catalogId:'m5stack-unit-hub-u006',parameters:{},wiring:{mode:'physical',interfaceId:'grove'}},{id:'button-unit',catalogId:'m5stack-unit-button-u027',parameters:{},wiring:{mode:'physical',viaAccessoryId:'hub',portId:'port-1'}},{id:'rgb-unit',catalogId:'m5stack-unit-rgb-u003',parameters:{},wiring:{mode:'physical',viaAccessoryId:'hub',portId:'port-3'}}];
try {
 await runtime.configure(scene);await runtime.loadFirmware(source);await runtime.start();await wait(s=>s.serial.includes('U006_BOOT_READY')&&equal(colors(s),a));await sample(1);check('actual ROM/IDF fixture drives U003 G10 RMT and reads U027 G11 simultaneously through U006');
 await press(true);await sample(0);await press(false);await sample(1);await send('b','U006_TX pin=10 offset=1');await wait(s=>equal(colors(s),b));check('button low/high and fresh RGB payload change independently on the shared two-wire host port');
 await connect('button-unit',false);await sample(0);assert.ok(equal(colors(await runtime.observe()),b));await connect('button-unit',true);await sample(1);check('child unplug affects its signal only; sibling output preserved');
 await press(true);const hubReceipt=await connect('hub',false);await sample(0);await wait(s=>equal(colors(s),black));assert.ok((await runtime.observe()).accessoryInputs.filter(i=>i.viaAccessoryId).every(i=>!i.effectiveConnected&&i.connected));await assert.rejects(()=>press(true),{code:'ACCESSORY_DISCONNECTED'});await send('b','U006_TX pin=10 offset=1');assert.ok(equal(colors(await runtime.observe()),black));check('hub batch disconnect clears both actual routes, releases button, and discards guest LED data');
 await connect('hub',true);await sample(1);assert.ok(equal(colors(await runtime.observe()),black));await send('a','U006_TX pin=10 offset=0');await wait(s=>equal(colors(s),a));check('hub reconnect restores released button but LEDs require fresh real RMT data');
 await send('o','U006_POWER_OFF');await sample(0);await wait(s=>equal(colors(s),black));await send('a','U006_TX pin=10 offset=0');await send('i','U006_POWER_ON');await sample(1);assert.ok(equal(colors(await runtime.observe()),black));check('shared Grove supply gate applies to both children; no fabricated retained light');
 await runtime.pause();await connect('hub',false);await runtime.start();await sample(0);await send('r','U006_BOOT_READY');await sample(0);assert.ok(equal(colors(await runtime.observe()),black));check('paused parent disconnect and guest reset preserve the physical open connection');
 await runtime.reset();await wait(s=>s.serial.includes('U006_BOOT_READY')&&equal(colors(s),a));await sample(1);assert.ok((await runtime.observe()).accessoryInputs.every(i=>i.connected));check('platform reset restores initial scene topology');
 await api.recordingStart();await wait(s=>s.serial.includes('U006_BOOT_READY')&&equal(colors(s),a));await press(true);await sample(0);await connect('hub',false);await wait(s=>equal(colors(s),black));const recording=await api.recordingStop();assert.equal(recording.events.filter(e=>e.type==='accessory-connection').length,1);const replay=await api.recordingPlay(recording);assert.equal(replay.result,'completed');await sample(0);await wait(s=>equal(colors(s),black));receipt.recording={events:recording.events,result:replay.result,bitExact:replay.bitExact};check('semantic recording replays button plus upstream disconnect through real guest outputs');
 await runtime.pause();scene.accessories[1]={id:'angle-unit',catalogId:'m5stack-unit-angle-u005',parameters:{position:1},wiring:{mode:'physical',viaAccessoryId:'hub',portId:'port-2'}};await runtime.configure(scene);await runtime.loadFirmware(source);await runtime.start();await wait(s=>s.serial.includes('U006_BOOT_READY')&&equal(colors(s),a));const analog=async expected=>{const s=await send('v','U006_ADC ');const raw=Number([...s.serial.matchAll(/U006_ADC white11=(\d+)/g)].at(-1)[1]);assert.equal(raw,expected);};
 await analog(2786);await connect('hub',false);await analog(0);await runtime.input({type:'accessory-angle',accessoryId:'angle-unit',id:'knob',value:.25});await analog(0);await connect('hub',true);await analog(1087);assert.ok(equal(colors(await runtime.observe()),black));check('U005 through another outlet reaches real ADC2_CH0; detached knob changes remain unpowered and restore on reconnect');
 await runtime.pause();receipt.status='pass';receipt.serialEvidence=(await runtime.observe()).serial.split(/\r?\n/).filter(l=>l.startsWith('U006_'));receipt.accessoryInputs=(await runtime.observe()).accessoryInputs;console.log(JSON.stringify(receipt,null,2));
} finally {await api.dispose();}
