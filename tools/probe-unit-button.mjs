#!/usr/bin/env node
// Run the source-built internal Unit Button U027 fixture on the actual page worker/WASM path.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {resolve,join,basename} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {installAutomation} from '../web/platform/automation.js';
const index=process.argv.indexOf('--build');if(index<0||!process.argv[index+1])throw Error('Build tools/fixtures/unit-button-firmware with ESP-IDF 5.5.1 and supply --build <build-directory>.');
const folder=resolve(process.argv[index+1]),root=new URL('../',import.meta.url),baseUrl='https://unit-button-fixture.invalid/platform/';
const artifacts=new Map(),inputHashes={};
const layout=JSON.parse(await readFile(join(folder,'flasher_args.json')));
const manifest={board:'stopwatch',flash_mb:16,psram_mb:8,files:{rom:'/wasm/fw/esp32s3_rev0_rom.elf'},artifact_sha256:{}};
for(const [kind,key] of [['bootloader','bootloader'],['ptable','partition-table'],['app','app']]){
  const path=layout[key].file,bytes=await readFile(join(folder,path)),url='/fixture/'+basename(path);artifacts.set(url,bytes);manifest.files[kind]=url;inputHashes[path]=createHash('sha256').update(bytes).digest('hex');manifest.artifact_sha256[url]=inputHashes[path];
}
const manifestBytes=Buffer.from(JSON.stringify(manifest));artifacts.set('/fixture/manifest.json',manifestBytes);
const source={manifestUrl:'https://unit-button-fixture.invalid/fixture/manifest.json',sha256:createHash('sha256').update(manifestBytes).digest('hex')};
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const fetchImpl=async value=>{const url=new URL(value);assert.equal(url.origin,'https://unit-button-fixture.invalid');try{if(artifacts.has(url.pathname))return new Response(artifacts.get(url.pathname));return new Response(await readFile(new URL('web'+url.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root);const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl,assetBaseUrl:'https://unit-button-fixture.invalid/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'unit-button-probe'})});
const sourceSha256=Object.fromEntries(await Promise.all(['esp-soc/src/board.rs','esp-soc/src/machine/web.rs','esp32s3/src/board/stopwatch.rs','esp-periph/src/i2c/m5.rs','web/platform/runtime.js','web/platform/recording.js','tools/probe-unit-button.mjs','tools/fixtures/unit-button-firmware/main/main.c'].map(async path=>[path,createHash('sha256').update(await readFile(new URL(path,root))).digest('hex')])));
const receipt={checkedAt:new Date().toISOString().replace(/\.\d{3}Z$/,'Z'),sourceSha256,wasmSha256:createHash('sha256').update(await readFile(new URL('web/wasm/esp32sim.wasm',root))).digest('hex'),romSha256:createHash('sha256').update(await readFile(new URL('web/wasm/fw/esp32s3_rev0_rom.elf',root))).digest('hex'),command:'node tools/probe-unit-button.mjs --build <fixture-build-directory>',backend:'actual-wasm-worker',firmware:'internal ESP-IDF 5.5.1 protocol fixture',inputHashes,checks:[]};
const wait=async predicate=>{const end=Date.now()+90000;while(Date.now()<end){const state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(JSON.stringify(state.lastError));await new Promise(r=>setTimeout(r,50));}throw Error(`Fixture timed out: ${(await runtime.observe()).serial.slice(-4000)}`);};
const check=message=>{receipt.checks.push(message);console.error('ok '+message);};
const api=installAutomation(runtime,{target:{}});
const line=(s,value)=>s.serial.includes(value);
const change=async(event,expected)=>{const at=(await runtime.observe()).serial.length;await runtime.input(event);return wait(s=>s.serial.slice(at).includes(expected));};
const command=async(data,expected)=>change({type:'serial',port:'uart0',data},expected);
const press=pressed=>({type:'accessory-button',accessoryId:'button-unit',id:'button',pressed});
const connect=connected=>({type:'accessory-connection',accessoryId:'button-unit',connected});
const read=value=>`U027_READ G11=${value} G10=1`;
try {
  const scene=createDefaultScene(catalog,'m5stack-stopwatch');scene.firmware=source;
  scene.accessories=[{id:'button-unit',catalogId:'m5stack-unit-button-u027',parameters:{},wiring:{mode:'physical',interfaceId:'grove'}}];
  await runtime.configure(scene);await runtime.loadFirmware(source);await runtime.start();
  await wait(s=>line(s,'U027_POWER_OFF G11=0 G10=1')&&line(s,read(1)));check('ROM boot; Grove power off reads low, enabled reads high; yellow G10 unaffected');
  await change(press(true),read(0));await change(press(false),read(1));check('button press/release observed as actual G11 0/1');
  await command('o',read(0));await runtime.input(press(true));await runtime.input(press(false));await command('i',read(1));check('PWR_CFG bit3 gates released switch voltage');
  await change(connect(false),read(0));await command('u',read(1));await command('d',read(0));
  await assert.rejects(()=>runtime.input(press(true)),{code:'ACCESSORY_DISCONNECTED'});
  await change(connect(true),read(1));check('detach releases external input: guest pull-down=0, pull-up=1; reconnect=1');
  await change(press(true),read(0));await runtime.pause();const pausedAt=(await runtime.observe()).serial.length;await runtime.input(press(false));await runtime.start();await wait(s=>s.serial.slice(pausedAt).includes(read(1)));check('release accepted while paused and consumed after resume');
  await change(press(true),read(0));const restartAt=(await runtime.observe()).serial.length;await command('r','U027_POWER_OFF G11=0 G10=1');await wait(s=>s.serial.slice(restartAt).includes('U027_POWER_ON'));
  await change(press(false),read(1));check('guest esp_restart retains physically held switch');
  await runtime.input(connect(false));await runtime.reset();await wait(s=>line(s,read(1)));assert.equal((await runtime.observe()).accessoryInputs[0].connected,true);assert.equal((await runtime.observe()).accessoryInputs[0].pressed,false);check('host reset restores scene connection and releases switch');
  await command('h',read(1));await command('s',read(1));await command('n',read(0));check('HOLD_CFG bit6 retains Grove5V after shutdown, clearing hold removes it');
  await api.recordingStart();await wait(s=>line(s,read(1)));
  await change(press(true),read(0));await change(press(false),read(1));await change(connect(false),read(0));await change(connect(true),read(1));
  const recording=await api.recordingStop();assert.equal(recording.events.length,4);assert.deepEqual(recording.events.map(e=>e.type),['accessory-button','accessory-button','accessory-connection','accessory-connection']);
  const replay=await api.recordingPlay(recording);assert.equal(replay.result,'completed');await wait(s=>/U027_READ G11=1 G10=1[\s\S]*U027_READ G11=0 G10=1[\s\S]*U027_READ G11=1 G10=1[\s\S]*U027_READ G11=0 G10=1[\s\S]*U027_READ G11=1 G10=1/.test(s.serial));receipt.recording={eventCount:recording.events.length,result:replay.result,bitExact:replay.bitExact,events:replay.events};check('record/replay reproduces guest-observed press, release, disconnect and reconnect');
  await runtime.pause();const state=await runtime.observe();receipt.status='pass';receipt.serialEvidence=state.serial.split(/\r?\n/).filter(l=>l.startsWith('U027_'));receipt.guestSeconds=state.stats.time;console.log(JSON.stringify(receipt,null,2));
} finally {await api.dispose();}
