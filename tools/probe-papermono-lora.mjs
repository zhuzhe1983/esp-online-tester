#!/usr/bin/env node
// Run the source-built internal PaperMono LoRa fixture on the actual page worker/WASM path.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {resolve,join,basename} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {inspectLocalBuild} from '../web/platform/local-firmware.js';
import {createDefaultScene} from '../web/platform/catalog.js';
const index=process.argv.indexOf('--build');if(index<0||!process.argv[index+1])throw Error('Build tools/fixtures/papermono-lora-firmware with ESP-IDF 5.5.1 and supply --build <build-directory>.');
const folder=resolve(process.argv[index+1]),root=new URL('../',import.meta.url),baseUrl='https://papermono-lora-fixture.invalid/platform/';
const files=[new File([await readFile(join(folder,'flasher_args.json'))],'flasher_args.json')],inputHashes={};
for(const path of (await inspectLocalBuild(files)).missingFiles){const bytes=await readFile(join(folder,path)),file=new File([bytes],basename(path));inputHashes[path]=createHash('sha256').update(bytes).digest('hex');Object.defineProperty(file,'webkitRelativePath',{value:path});files.push(file);}
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const fetchImpl=async value=>{const url=new URL(value);assert.equal(url.origin,'https://papermono-lora-fixture.invalid');try{return new Response(await readFile(new URL('web'+url.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root);const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl,assetBaseUrl:'https://papermono-lora-fixture.invalid/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'papermono-lora-probe'})});
const receipt={checkedAt:new Date().toISOString(),command:'node tools/probe-papermono-lora.mjs --build <fixture-build-directory>',backend:'actual-wasm-worker',firmware:'internal ESP-IDF 5.5.1 protocol fixture',inputHashes,wasmSha256:createHash('sha256').update(await readFile(new URL('web/wasm/esp32sim.wasm',root))).digest('hex'),checks:[]};
const wait=async predicate=>{const end=Date.now()+90000;while(Date.now()<end){const state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(JSON.stringify(state.lastError));if(/assert failed:|Guru Meditation Error/.test(state.serial))throw Error('Guest fixture assertion/panic: '+state.serial.slice(-4000));await new Promise(r=>setTimeout(r,50));}throw Error(`Fixture timed out: ${(await runtime.observe()).serial.slice(-4000)}`);};
try {
  for(const [id,expected] of [['m5stack-papermono','PASS_FULL'],['m5stack-c153-lite','LITE_ABSENT']]) {
    const scene=createDefaultScene(catalog,id);scene.firmware=null;await runtime.configure(scene);await runtime.loadFirmware({format:'build',files});await runtime.start();
    if(id==='m5stack-papermono') {
      await wait(s=>s.serial.includes('LORA_FIXTURE WAIT_RX'));
      const radio=await runtime.readLoRa();assert.equal(radio.txCount,1);assert.equal(radio.rxCount,0);assert.equal(radio.transmitted[0].payloadHex,'48454c4c4f');
      const packet={config:radio.config,payloadHex:'50454552',rssiDbm:-70,snrQuarterDb:24,crcError:false};
      await assert.rejects(runtime.receiveLoRa({...packet,config:{...packet.config,frequency:packet.config.frequency+1}}),{code:'LORA_PACKET_REJECTED'});
      const input=await runtime.receiveLoRa(packet);assert.equal(input.delivery,'scheduled-digital-packet');
      receipt.peerInput={...packet,receipt:input};receipt.beforeReceive=radio;
    }
    const state=await wait(s=>s.serial.includes(`LORA_FIXTURE ${expected}`));await runtime.pause();
    const lines=(state.serialByPort.uart0||'').split(/\r?\n/).filter(l=>l.startsWith('LORA_FIXTURE '));
    if(id==='m5stack-papermono')for(const marker of ['POWER_OFF_NO_RESPONSE','SPI3_DMA_VERSION','TX_DONE_IRQ','NO_PEER_TIMEOUT','WAIT_RX','RX_BUFFER PEER RSSI=-70 SNR=6','RESET_POWER_GATES'])assert.ok(lines.includes(`LORA_FIXTURE ${marker}`));
    else assert.deepEqual(lines,['LORA_FIXTURE POWER_OFF_NO_RESPONSE','LORA_FIXTURE LITE_ABSENT']);
    receipt.checks.push({productId:id,board:catalog.products.find(p=>p.id===id).runtime.board,serial:lines,guestSeconds:state.stats.time});
  }
  receipt.status='pass';console.log(JSON.stringify(receipt,null,2));
} finally {await runtime.dispose();}
