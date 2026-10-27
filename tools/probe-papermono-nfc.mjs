#!/usr/bin/env node
// Run the source-built internal PaperMono NFC fixture on the actual page worker/WASM path.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {resolve,join,basename} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {inspectLocalBuild} from '../web/platform/local-firmware.js';
import {createDefaultScene} from '../web/platform/catalog.js';
const index=process.argv.indexOf('--build');if(index<0||!process.argv[index+1])throw Error('Build tools/fixtures/papermono-nfc-firmware with ESP-IDF 5.5.1 and supply --build <build-directory>.');
const folder=resolve(process.argv[index+1]),root=new URL('../',import.meta.url),baseUrl='https://papermono-nfc-fixture.invalid/platform/';
const files=[new File([await readFile(join(folder,'flasher_args.json'))],'flasher_args.json')],inputHashes={};
for(const path of (await inspectLocalBuild(files)).missingFiles){const bytes=await readFile(join(folder,path)),file=new File([bytes],basename(path));inputHashes[path]=createHash('sha256').update(bytes).digest('hex');Object.defineProperty(file,'webkitRelativePath',{value:path});files.push(file);}
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const fetchImpl=async value=>{const url=new URL(value);assert.equal(url.origin,'https://papermono-nfc-fixture.invalid');try{return new Response(await readFile(new URL('web'+url.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root);const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl,assetBaseUrl:'https://papermono-nfc-fixture.invalid/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'papermono-nfc-probe'})});
const receipt={checkedAt:new Date().toISOString(),command:'node tools/probe-papermono-nfc.mjs --build <fixture-build-directory>',backend:'actual-wasm-worker',firmware:'internal ESP-IDF 5.5.1 protocol fixture',inputHashes,wasmSha256:createHash('sha256').update(await readFile(new URL('web/wasm/esp32sim.wasm',root))).digest('hex'),checks:[]};
const wait=async predicate=>{const end=Date.now()+90000;while(Date.now()<end){const state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(JSON.stringify(state.lastError));await new Promise(r=>setTimeout(r,50));}throw Error(`Fixture timed out: ${(await runtime.observe()).serial.slice(-4000)}`);};
try {
  for(const [id,expected] of [['m5stack-papermono','PASS_FULL'],['m5stack-c153-lite','LITE_NACK']]) {
    const scene=createDefaultScene(catalog,id);scene.firmware=null;await runtime.configure(scene);await runtime.loadFirmware({format:'build',files});await runtime.start();
    const state=await wait(s=>s.serial.includes(`NFC_FIXTURE ${expected}`));await runtime.pause();
    const lines=(state.serialByPort.uart0||'').split(/\r?\n/).filter(l=>l.startsWith('NFC_FIXTURE '));
    if(id==='m5stack-papermono')for(const marker of ['IDENTITY 2a','OSC_IRQ_READ_CLEAR','REQA_TX_NRT_MASK_NO_RX','STOP_CANCELS_NRT','SPI3_QUEUED_COMPLETED'])assert.ok(lines.includes(`NFC_FIXTURE ${marker}`));
    else assert.deepEqual(lines,['NFC_FIXTURE LITE_NACK']);
    receipt.checks.push({productId:id,board:catalog.products.find(p=>p.id===id).runtime.board,serial:lines,guestSeconds:state.stats.time});
  }
  receipt.status='pass';console.log(JSON.stringify(receipt,null,2));
} finally {await runtime.dispose();}
