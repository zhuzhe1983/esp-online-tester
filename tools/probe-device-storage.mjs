#!/usr/bin/env node
// Opt-in real-WASM fixture, built with ESP-IDF 5.5.1; never a real hardware operation.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {resolve,join,basename} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {assembleLocalFirmware,inspectLocalBuild} from '../web/platform/local-firmware.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {withPlatformTestFixtures,HELLO_PRODUCT_ID} from './fixtures/platform-catalog.mjs';
const index=process.argv.indexOf('--build');if(index<0||!process.argv[index+1])throw Error('Build tools/fixtures/device-storage-firmware with ESP-IDF 5.5.1, then supply --build <build-directory>.');
const folder=resolve(process.argv[index+1]),root=new URL('../',import.meta.url),baseUrl='https://local-storage.invalid/platform/';
const files=[new File([await readFile(join(folder,'flasher_args.json'))],'flasher_args.json')];
for(const path of (await inspectLocalBuild(files)).missingFiles){const file=new File([await readFile(join(folder,path))],basename(path));Object.defineProperty(file,'webkitRelativePath',{value:path});files.push(file);}
const publicCatalog={};for(const key of ['products','accessories','firmwares'])publicCatalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const catalog=await withPlatformTestFixtures(publicCatalog,baseUrl);
const fetchImpl=async value=>{const url=new URL(value);assert.equal(url.origin,'https://local-storage.invalid');try{return new Response(await readFile(new URL('web'+url.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root);const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const dbName=`wasm-storage-${Date.now()}`,events=[],started=performance.now();
const make=()=>createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl,assetBaseUrl:'https://local-storage.invalid/',deviceStorage:createDeviceStorage({indexedDB:null,dbName}),onEvent:e=>{if(e.type==='storage')events.push(e.storage);}});
let runtime=make();
const ready=async source=>{const scene=createDefaultScene(catalog,HELLO_PRODUCT_ID);scene.firmware=null;await runtime.configure(scene);await runtime.loadFirmware(source);};
const wait=async text=>{const end=Date.now()+120000;while(Date.now()<end){const s=await runtime.observe();if(s.serial.includes(text))return s;if(s.status==='error')throw Error(JSON.stringify(s.lastError));await new Promise(r=>setTimeout(r,100));}throw Error(`Missing ${text}: ${(await runtime.observe()).serial.slice(-4000)}`);};
const receipt={checkedAt:new Date().toISOString(),command:'node tools/probe-device-storage.mjs --build <fixture-build-directory>',backend:'actual-wasm-worker',checks:[]};
try {
  await ready({format:'build',files});assert.equal((await runtime.observe()).storage.pristine,true);const scope=(await runtime.observe()).storage.scope;
  console.error('phase: guest writes NVS and wear-levelled FAT, then software restart');await runtime.start();
  let state=await wait('STORAGE_RESTORED NVS FAT');assert.match(state.serial,/STORAGE_WRITTEN NVS FAT; SOFTWARE_RESTART/);receipt.checks.push('guest-nvs-fat-software-restart');
  await runtime.pause();let stored=await runtime.flushDeviceStorage();assert.equal(stored.pristine,false);assert.ok(stored.changedPages>0);receipt.changedPages=stored.changedPages;receipt.scope=scope;
  console.error('phase: host reset restores written pages');await runtime.reset();state=await wait('STORAGE_RESTORED NVS FAT');assert.doesNotMatch(state.serial,/STORAGE_WRITTEN/);receipt.checks.push('host-reset-restores');
  await runtime.dispose();runtime=make();
  const merged=await assembleLocalFirmware({format:'build',files},{board:'none',flashBytes:8*1024*1024});
  await ready({file:new File([merged.data],'renamed-merged.bin'),format:'merged'});state=await runtime.observe();assert.equal(state.storage.scope.key,scope.key,'content-equivalent merged and build sources have identical scope');assert.equal(state.storage.restored,true);
  console.error('phase: new runtime and differently named merged image restore same device');await runtime.start();state=await wait('STORAGE_RESTORED NVS FAT');assert.doesNotMatch(state.serial,/STORAGE_WRITTEN/);receipt.checks.push('new-runtime-and-content-equivalent-source');
  const stale=make();
  const staleScene=createDefaultScene(catalog,HELLO_PRODUCT_ID);staleScene.firmware=null;await stale.configure(staleScene);await stale.loadFirmware({format:'build',files});
  console.error('phase: clear stops, restores original bytes and waits ready');state=await runtime.clearDeviceStorage();
  try {assert.equal((await stale.observe()).storage.status,'conflict');assert.equal((await stale.observe()).status,'error');await assert.rejects(()=>stale.flushDeviceStorage(),{code:'STORAGE_CONFLICT'});receipt.checks.push('other-runtime-stopped-on-clear');} finally {await stale.dispose();}assert.equal(state.status,'ready');assert.equal(state.storage.pristine,true);assert.equal(state.storage.scope.key,scope.key);assert.ok(state.storage.epoch>0);receipt.checks.push('clear-ready-original-image');
  await runtime.start();state=await wait('STORAGE_RESTORED NVS FAT');assert.match(state.serial,/STORAGE_WRITTEN/);receipt.checks.push('cleared-guest-reinitializes');
  await runtime.pause();receipt.guestSeconds=state.stats.time;receipt.elapsedSeconds=(performance.now()-started)/1000;receipt.status='pass';receipt.storageEvents=events.length;console.log(JSON.stringify(receipt,null,2));
} finally {await runtime.dispose();}
