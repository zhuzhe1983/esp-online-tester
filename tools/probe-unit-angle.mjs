#!/usr/bin/env node
// Run the source-built internal Unit Angle U005 fixture on the actual page worker/WASM path.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {resolve,join,basename} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {installAutomation} from '../web/platform/automation.js';
const index=process.argv.indexOf('--build');if(index<0||!process.argv[index+1])throw Error('Build tools/fixtures/unit-angle-firmware with ESP-IDF 5.5.1 and supply --build <build-directory>.');
const folder=resolve(process.argv[index+1]),root=new URL('../',import.meta.url),baseUrl='https://unit-angle-fixture.invalid/platform/';
const artifacts=new Map(),inputHashes={};
const layout=JSON.parse(await readFile(join(folder,'flasher_args.json')));
const manifest={board:'stopwatch',flash_mb:16,psram_mb:8,files:{rom:'/wasm/fw/esp32s3_rev0_rom.elf'},artifact_sha256:{}};
for(const [kind,key] of [['bootloader','bootloader'],['ptable','partition-table'],['app','app']]){
  const path=layout[key].file,bytes=await readFile(join(folder,path)),url='/fixture/'+basename(path);artifacts.set(url,bytes);manifest.files[kind]=url;inputHashes[path]=createHash('sha256').update(bytes).digest('hex');manifest.artifact_sha256[url]=inputHashes[path];
}
const manifestBytes=Buffer.from(JSON.stringify(manifest));artifacts.set('/fixture/manifest.json',manifestBytes);
const source={manifestUrl:'https://unit-angle-fixture.invalid/fixture/manifest.json',sha256:createHash('sha256').update(manifestBytes).digest('hex')};
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const fetchImpl=async value=>{const url=new URL(value);assert.equal(url.origin,'https://unit-angle-fixture.invalid');try{if(artifacts.has(url.pathname))return new Response(artifacts.get(url.pathname));return new Response(await readFile(new URL('web'+url.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root);const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl,assetBaseUrl:'https://unit-angle-fixture.invalid/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'unit-angle-probe'})});
const sourceSha256=Object.fromEntries(await Promise.all(['esp-soc/src/board.rs','esp-soc/src/machine/web.rs','esp32s3/src/board/stopwatch.rs','esp-periph/src/i2c/m5.rs','esp-periph/src/analog/stream.rs','web/platform/runtime.js','web/platform/recording.js','tools/probe-unit-angle.mjs','tools/fixtures/unit-angle-firmware/main/main.c'].map(async path=>[path,createHash('sha256').update(await readFile(new URL(path,root))).digest('hex')])));
const receipt={checkedAt:new Date().toISOString().replace(/\.\d{3}Z$/,'Z'),sourceSha256,wasmSha256:createHash('sha256').update(await readFile(new URL('web/wasm/esp32sim.wasm',root))).digest('hex'),romSha256:createHash('sha256').update(await readFile(new URL('web/wasm/fw/esp32s3_rev0_rom.elf',root))).digest('hex'),command:'node tools/probe-unit-angle.mjs --build <fixture-build-directory>',backend:'actual-wasm-worker',firmware:'internal ESP-IDF 5.5.1 protocol fixture',inputHashes,checks:[]};
const wait=async predicate=>{const end=Date.now()+90000;while(Date.now()<end){const state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(JSON.stringify(state.lastError));await new Promise(r=>setTimeout(r,50));}throw Error(`Fixture timed out: ${(await runtime.observe()).serial.slice(-4000)}`);};
const check=message=>{receipt.checks.push(message);console.error('ok '+message);};
const api=installAutomation(runtime,{target:{}});
const connect=connected=>({type:'accessory-connection',accessoryId:'angle-unit',connected});
const turn=value=>({type:'accessory-angle',accessoryId:'angle-unit',id:'knob',value});
const readings=serial=>[...serial.matchAll(/U005_ADC white11=(\d+) mv=(-?\d+) other12=(\d+) yellow10=(\d+)/g)].map(m=>({raw:+m[1],mv:+m[2],other12:+m[3],yellow10:+m[4]}));
const query=async(key='q')=>{const at=(await runtime.observe()).serial.length;await runtime.input({type:'serial',port:'uart0',data:key});const state=await wait(s=>readings(s.serial.slice(at)).length>0);return readings(state.serial.slice(at)).at(-1);};
const verify=(reading,volts)=>{assert.equal(reading.other12,0,'G12 must not follow the knob');assert.equal(reading.yellow10,0,'yellow NC/G10 must not follow the knob');if(volts===0)assert.equal(reading.raw,0);else assert.ok(Math.abs(reading.mv-1000*volts)<=30,JSON.stringify({reading,volts}));return reading;};
receipt.samples=[];
try {
 const scene=createDefaultScene(catalog,'m5stack-stopwatch');scene.firmware=source;scene.accessories=[{id:'angle-unit',catalogId:'m5stack-unit-angle-u005',parameters:{position:.25},wiring:{mode:'physical',interfaceId:'grove'}}];
 await runtime.configure(scene);await runtime.loadFirmware(source);await runtime.start();await wait(s=>s.serial.includes('U005_BOOT_READY')&&readings(s.serial).length>=2);verify(await query(),1);check('ROM/IDF ADC2_CH0 reads U005 initial G11 voltage; wrong G12 and yellow G10 remain undriven');
 for(const value of [0,.25,.5,.75,1]){await runtime.input(turn(value));const sample=verify(await query(),5*value/(1+value));receipt.samples.push({value,...sample});}assert.ok(receipt.samples.every((v,i,a)=>!i||v.raw>a[i-1].raw));check('five normalized positions reach real ADC driver conversions and calibrated mV');
 for(const value of [-1,2,NaN,'0.5'])await assert.rejects(()=>runtime.input(turn(value)),{code:'INVALID_INPUT'});await assert.rejects(()=>runtime.input({...turn(.5),id:'button'}),{code:'INVALID_INPUT'});await assert.rejects(()=>runtime.input({type:'accessory-button',accessoryId:'angle-unit',id:'button',pressed:true}),{code:'INVALID_INPUT'});verify(await query(),2.5);check('malformed knob values and fake button injection reject without changing the ADC source');
 verify(await query('o'),0);await runtime.input(turn(.75));verify(await query(),0);verify(await query('i'),5*.75/1.75);check('M5PM1 Grove5V gate wins over knob input and restores current position on powerup');
 await runtime.input(connect(false));verify(await query(),0);await runtime.input(turn(.5));verify(await query(),0);await runtime.input(connect(true));verify(await query(),5/3);check('unplug clears analog drive; rotation while unplugged is retained but cannot energize the pad');
 await runtime.pause();await assert.rejects(()=>runtime.input(turn(1)),{code:'NOT_RUNNING'});await runtime.input(connect(false));await runtime.start();verify(await query(),0);await runtime.input(connect(true));verify(await query(),5/3);await runtime.input(turn(1));verify(await query(),2.5);check('paused rotation rejects; paused disconnect delivers on resume and fresh conversion follows reconnect');
 verify(await query('h'),2.5);verify(await query('s'),2.5);verify(await query('n'),0);check('PMIC HOLD5V preserves power during shutdown; clearing hold removes the voltage');
 await runtime.input(connect(false));const at=(await runtime.observe()).serial.length;await runtime.input({type:'serial',port:'uart0',data:'r'});await wait(s=>s.serial.slice(at).includes('U005_BOOT_READY'));verify(await query(),0);assert.equal((await runtime.observe()).accessoryInputs[0].connected,false);await runtime.input(connect(true));verify(await query(),2.5);check('guest reboot preserves physical knob/connection state while resetting PMIC');
 await runtime.reset();await wait(s=>s.serial.includes('U005_BOOT_READY'));verify(await query(),1);assert.equal((await runtime.observe()).accessoryInputs[0].value,.25);check('platform reset restores the scene initial position and connection');
 await api.recordingStart();await wait(s=>s.serial.includes('U005_BOOT_READY'));await runtime.input(turn(.75));await runtime.input(connect(false));await runtime.input(turn(1));await runtime.input(connect(true));const recording=await api.recordingStop();assert.equal(recording.events.length,4);const replay=await api.recordingPlay(recording);assert.equal(replay.result,'completed');verify(await query(),2.5);receipt.recording={events:recording.events,result:replay.result,bitExact:replay.bitExact};check('record/replay reproduces semantic positions and connection; final ADC conversion matches');
 await runtime.pause();receipt.status='pass';receipt.serialEvidence=(await runtime.observe()).serial.split(/\r?\n/).filter(l=>l.startsWith('U005_'));console.log(JSON.stringify(receipt,null,2));
} finally {await api.dispose();}
