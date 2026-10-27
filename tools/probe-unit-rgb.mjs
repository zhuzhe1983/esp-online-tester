#!/usr/bin/env node
// Run the source-built internal Unit RGB U003 fixture on the actual page worker/WASM path.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {resolve,join,basename} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {installAutomation} from '../web/platform/automation.js';
const index=process.argv.indexOf('--build');if(index<0||!process.argv[index+1])throw Error('Build tools/fixtures/unit-rgb-firmware with ESP-IDF 5.5.1 and supply --build <build-directory>.');
const folder=resolve(process.argv[index+1]),root=new URL('../',import.meta.url),baseUrl='https://unit-rgb-fixture.invalid/platform/';
const artifacts=new Map(),inputHashes={};
const layout=JSON.parse(await readFile(join(folder,'flasher_args.json')));
const manifest={board:'stopwatch',flash_mb:16,psram_mb:8,files:{rom:'/wasm/fw/esp32s3_rev0_rom.elf'},artifact_sha256:{}};
for(const [kind,key] of [['bootloader','bootloader'],['ptable','partition-table'],['app','app']]){
  const path=layout[key].file,bytes=await readFile(join(folder,path)),url='/fixture/'+basename(path);artifacts.set(url,bytes);manifest.files[kind]=url;inputHashes[path]=createHash('sha256').update(bytes).digest('hex');manifest.artifact_sha256[url]=inputHashes[path];
}
const manifestBytes=Buffer.from(JSON.stringify(manifest));artifacts.set('/fixture/manifest.json',manifestBytes);
const source={manifestUrl:'https://unit-rgb-fixture.invalid/fixture/manifest.json',sha256:createHash('sha256').update(manifestBytes).digest('hex')};
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const fetchImpl=async value=>{const url=new URL(value);assert.equal(url.origin,'https://unit-rgb-fixture.invalid');try{if(artifacts.has(url.pathname))return new Response(artifacts.get(url.pathname));return new Response(await readFile(new URL('web'+url.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root);const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl,assetBaseUrl:'https://unit-rgb-fixture.invalid/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'unit-rgb-probe'})});
const sourceSha256=Object.fromEntries(await Promise.all(['esp-soc/src/board.rs','esp-soc/src/machine/web.rs','esp32s3/src/board/stopwatch.rs','esp-periph/src/i2c/m5.rs','web/platform/runtime.js','web/platform/recording.js','tools/probe-unit-rgb.mjs','tools/fixtures/unit-rgb-firmware/main/main.c'].map(async path=>[path,createHash('sha256').update(await readFile(new URL(path,root))).digest('hex')])));
const receipt={checkedAt:new Date().toISOString().replace(/\.\d{3}Z$/,'Z'),sourceSha256,wasmSha256:createHash('sha256').update(await readFile(new URL('web/wasm/esp32sim.wasm',root))).digest('hex'),romSha256:createHash('sha256').update(await readFile(new URL('web/wasm/fw/esp32s3_rev0_rom.elf',root))).digest('hex'),command:'node tools/probe-unit-rgb.mjs --build <fixture-build-directory>',backend:'actual-wasm-worker',firmware:'internal ESP-IDF 5.5.1 protocol fixture',inputHashes,checks:[]};
const wait=async predicate=>{const end=Date.now()+90000;while(Date.now()<end){const state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(JSON.stringify(state.lastError));await new Promise(r=>setTimeout(r,50));}throw Error(`Fixture timed out: ${(await runtime.observe()).serial.slice(-4000)}`);};
const check=message=>{receipt.checks.push(message);console.error('ok '+message);};
const api=installAutomation(runtime,{target:{}});
const black=[[0,0,0],[0,0,0],[0,0,0]],a=[[255,0,0],[0,128,0],[0,0,64]],b=[[0,128,0],[0,0,64],[255,0,0]];
const colors=s=>s.accessoryOutputs[0]?.leds;
const equal=(x,y)=>JSON.stringify(x)===JSON.stringify(y);
const command=async(data,marker,expected)=>{const at=(await runtime.observe()).serial.length;await runtime.input({type:'serial',port:'uart0',data});return wait(s=>s.serial.slice(at).includes(marker)&&equal(colors(s),expected));};
const connect=connected=>({type:'accessory-connection',accessoryId:'rgb-unit',connected});
try {
  const scene=createDefaultScene(catalog,'m5stack-stopwatch');scene.firmware=source;
  scene.accessories=[{id:'rgb-unit',catalogId:'m5stack-unit-rgb-u003',parameters:{},wiring:{mode:'physical',interfaceId:'grove'}}];
  await runtime.configure(scene);await runtime.loadFirmware(source);await runtime.start();
  await wait(s=>s.serial.includes('U003_BOOT_READY')&&equal(colors(s),a));check('ROM/bootloader/IDF RMT delivers three exact GRB->RGB colours through G10');
  await command('b','U003_TX pin=10 offset=1',b);await command('w','U003_TX pin=11 offset=2',b);check('different guest payload changes colours; wrong white G11 route leaves colours unchanged');
  await command('o','U003_POWER_OFF',black);await command('a','U003_TX pin=10 offset=0',black);await command('i','U003_POWER_ON',black);check('PWR_CFG poweroff clears output; unpowered transmit is discarded and re-enable stays dark');
  await command('a','U003_TX pin=10 offset=0',a);await runtime.input(connect(false));await wait(s=>equal(colors(s),black));
  await command('b','U003_TX pin=10 offset=1',black);await runtime.input(connect(true));await command('w','U003_TX pin=11 offset=2',black);await command('b','U003_TX pin=10 offset=1',b);check('unplug clears light; detached frames discarded; reconnect needs new correct-pin guest data');
  await assert.rejects(()=>runtime.input({type:'accessory-button',accessoryId:'rgb-unit',id:'button',pressed:true}),{code:'INVALID_INPUT'});
  await runtime.pause();await runtime.input(connect(false));await runtime.start();await wait(s=>equal(colors(s),black));check('paused disconnect delivered on resume; RGB unit rejects button injection');
  await runtime.input(connect(true));await command('a','U003_TX pin=10 offset=0',a);await command('h','U003_HOLD',a);await command('s','U003_SHUTDOWN',a);await command('n','U003_NO_HOLD',black);check('HOLD_CFG bit6 retains Grove output during PMIC shutdown and clearing hold blacks it');
  await runtime.input(connect(false));await command('r','U003_BOOT_READY',black);assert.equal((await runtime.observe()).accessoryInputs[0].connected,false);check('guest reboot retains physical disconnect even when firmware re-enables power and transmits');
  await runtime.reset();await wait(s=>s.serial.includes('U003_BOOT_READY')&&equal(colors(s),a));assert.equal((await runtime.observe()).accessoryInputs[0].connected,true);check('platform reset restores scene connection, fresh guest RMT lights output');
  await api.recordingStart();await wait(s=>s.serial.includes('U003_BOOT_READY')&&equal(colors(s),a));await runtime.input(connect(false));await wait(s=>equal(colors(s),black));await runtime.input(connect(true));
  const recording=await api.recordingStop();assert.equal(recording.events.length,2);const replay=await api.recordingPlay(recording);assert.equal(replay.result,'completed');await wait(s=>equal(colors(s),black));assert.equal((await runtime.observe()).accessoryInputs[0].connected,true);receipt.recording={events:recording.events,result:replay.result,bitExact:replay.bitExact};check('record/replay starts fresh guest output then removes it on detach; reconnect preserves dark state');
  await runtime.pause();const state=await runtime.observe();receipt.status='pass';receipt.serialEvidence=state.serial.split(/\r?\n/).filter(l=>l.startsWith('U003_'));receipt.outputs=state.accessoryOutputs;receipt.guestSeconds=state.stats.time;console.log(JSON.stringify(receipt,null,2));
} catch(error) {console.error(JSON.stringify((await runtime.observe()).accessoryOutputs));throw error;} finally {await api.dispose();}
