#!/usr/bin/env node
// Run the source-built internal Dial fixture on the actual page worker/WASM path.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {resolve,join,basename} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {inspectLocalBuild} from '../web/platform/local-firmware.js';
import {installAutomation} from '../web/platform/automation.js';
import {createDefaultScene} from '../web/platform/catalog.js';
const index=process.argv.indexOf('--build');if(index<0||!process.argv[index+1])throw Error('Build tools/fixtures/dial-firmware with ESP-IDF 5.5.1 and supply --build <build-directory>.');
const folder=resolve(process.argv[index+1]),root=new URL('../',import.meta.url),baseUrl='https://dial-fixture.invalid/platform/';
const files=[new File([await readFile(join(folder,'flasher_args.json'))],'flasher_args.json')],inputHashes={};
for(const path of (await inspectLocalBuild(files)).missingFiles){const bytes=await readFile(join(folder,path)),file=new File([bytes],basename(path));inputHashes[path]=createHash('sha256').update(bytes).digest('hex');Object.defineProperty(file,'webkitRelativePath',{value:path});files.push(file);}
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const manifest=JSON.stringify({board:'dial',flash_mb:8,psram_mb:0,files:{rom:'esp32s3_rev0_rom.elf',bootloader:'bootloader/bootloader.bin',ptable:'partition_table/partition-table.bin',app:'dial_fixture.bin'},artifact_sha256:inputHashes});
const fetchImpl=async value=>{const url=new URL(value);assert.equal(url.origin,'https://dial-fixture.invalid');try{if(url.pathname==='/fixture/manifest.json')return new Response(manifest);if(url.pathname.startsWith('/fixture/')&&!url.pathname.endsWith('.elf'))return new Response(await readFile(join(folder,url.pathname.slice('/fixture/'.length))));if(url.pathname.endsWith('.elf'))return new Response(await readFile(new URL('web/wasm/fw/'+basename(url.pathname),root)));return new Response(await readFile(new URL('web'+url.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root);const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl,assetBaseUrl:'https://dial-fixture.invalid/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'dial-probe'})});
const receipt={checkedAt:new Date().toISOString(),command:'node tools/probe-dial.mjs --build <fixture-build-directory>',backend:'actual-wasm-worker',firmware:'internal ESP-IDF 5.5.1 protocol fixture',inputHashes,wasmSha256:createHash('sha256').update(await readFile(new URL('web/wasm/esp32sim.wasm',root))).digest('hex'),checks:[]};
const wait=async predicate=>{const end=Date.now()+90000;while(Date.now()<end){const state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(JSON.stringify(state.lastError));await new Promise(r=>setTimeout(r,50));}throw Error(`Fixture timed out: ${(await runtime.observe()).serial.slice(-4000)}`);};
try {
  const scene=createDefaultScene(catalog,'m5stack-k130');scene.firmware=null;await runtime.configure(scene);await runtime.loadFirmware({format:'build',files});await runtime.start();
  await wait(s=>s.serial.includes('DIAL_TOUCH_VENDOR 11')&&s.serial.includes('DIAL_BUTTON 1')&&s.frame?.width===240);receipt.checks.push('rom-boot-gpio42-idle-ft3267-vendor');
  await runtime.pause();const frame=await runtime.capture({format:'rgba'});assert.equal(frame.width,240);assert.equal(frame.height,240);
  for(let y=0;y<240;y++)for(let x=0;x<240;x++)assert.deepEqual([...frame.pixels.subarray((y*240+x)*4,(y*240+x)*4+4)],x<128?[255,0,0,255]:[0,255,0,255]);
  receipt.frameSha256=createHash('sha256').update(frame.pixels).digest('hex');receipt.checks.push('every-guest-spi-pixel-red128-green112');
  await runtime.start();
  async function action(event,text){const start=((await runtime.observe()).serialByPort.uart0||'').length;const ack=await runtime.input(event);await wait(s=>(s.serialByPort.uart0||'').slice(start).includes(text));return ack;}
  await action({type:'button',id:'button',pressed:true},'DIAL_BUTTON 0');await action({type:'button',id:'button',pressed:false},'DIAL_BUTTON 1');receipt.checks.push('gpio42-press-release-observed-by-guest');
  await action({type:'touch',x:100,y:200,down:true},'DIAL_TOUCH 1 100 200 IRQ=0');await action({type:'touch',x:100,y:200,down:false},'DIAL_TOUCH 0 100 200 IRQ=1');receipt.checks.push('ft3267-registers-and-gpio14-touch-down-up');
  await action({type:'encoder',id:'encoder',steps:1},'DIAL_ENCODER -4');await action({type:'encoder',id:'encoder',steps:-2},'DIAL_ENCODER 4');await action({type:'encoder',id:'encoder',steps:1},'DIAL_ENCODER 0');receipt.checks.push('real-gpio-interrupt-quadrature-plus1-minus2-plus1');
  for(const steps of [0,65,-65,1.5,NaN])await assert.rejects(runtime.input({type:'encoder',id:'encoder',steps}),{code:'INVALID_INPUT'});
  await assert.rejects(runtime.input({type:'encoder',id:'absent',steps:1}),{code:'UNSUPPORTED_CONTROL'});receipt.checks.push('encoder-boundary-rejection');
  const initial=await runtime.observe();receipt.serialEvidence=(initial.serialByPort.uart0||'').split(/\r?\n/).filter(l=>/^DIAL_/.test(l)).filter((l,i,a)=>a.indexOf(l)===i);
  const api=installAutomation(runtime,{target:{location:{href:baseUrl}}});
  const remote=createDefaultScene(catalog,'m5stack-k130');remote.firmware={manifestUrl:'https://dial-fixture.invalid/fixture/manifest.json',sha256:createHash('sha256').update(manifest).digest('hex')};await runtime.configure(remote);await runtime.loadFirmware();
  await api.recordingStart();await wait(s=>s.serial.includes('DIAL_TOUCH_VENDOR 11')&&s.serial.includes('DIAL_ENCODER 0'));
  await action({type:'encoder',id:'encoder',steps:2},'DIAL_ENCODER -8');await action({type:'encoder',id:'encoder',steps:-1},'DIAL_ENCODER -4');
  const recorded=await api.recordingStop();const played=await api.recordingPlay(recorded,{timeoutMs:30000});assert.equal(played.result,'completed');
  const state=await wait(s=>s.serial.includes('DIAL_ENCODER -8')&&s.serial.includes('DIAL_ENCODER -4'));receipt.checks.push('real-wasm-record-replay-encoder-detents');receipt.replay={events:recorded.events,result:played.result,frameComparison:played.frameComparison,bitExact:false,guestSerial:(state.serialByPort.uart0||'').split(/\r?\n/).filter(l=>/^DIAL_ENCODER/.test(l))};
  await runtime.pause();receipt.status='pass';receipt.guestSeconds=state.stats.time;receipt.serialPort='uart0';console.log(JSON.stringify(receipt,null,2));
} finally {await runtime.dispose();}
