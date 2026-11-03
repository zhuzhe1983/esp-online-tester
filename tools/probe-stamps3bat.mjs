#!/usr/bin/env node
// Real worker/WASM + independently source-built IDF fixture; never factory proof.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {resolve,join,basename} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {inspectLocalBuild} from '../web/platform/local-firmware.js';
import {createDefaultScene} from '../web/platform/catalog.js';
const arg=name=>{const i=process.argv.indexOf(name);return i<0?null:process.argv[i+1];};
const build=arg('--build');if(!build)throw Error('Supply --build <ESP-IDF 5.5.1 stamps3bat-firmware build directory>.');
const productId=arg('--product')||'m5stack-s015';assert(['m5stack-s015','m5stack-s015-dip'].includes(productId));
const folder=resolve(build),root=new URL('../',import.meta.url),origin='https://stamps3bat-fixture.invalid',hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const files=[new File([await readFile(join(folder,'flasher_args.json'))],'flasher_args.json')],inputHashes={};
for(const path of (await inspectLocalBuild(files)).missingFiles){const bytes=await readFile(join(folder,path)),file=new File([bytes],basename(path));inputHashes[path]=hash(bytes);Object.defineProperty(file,'webkitRelativePath',{value:path});files.push(file);}
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const fetchImpl=async value=>{const url=new URL(value);assert.equal(url.origin,origin);try{return new Response(await readFile(new URL('web'+url.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root),thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(m,...args)=>thread.postMessage(m,...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'stamps3bat-probe'})});
const receipt={checkedAt:new Date().toISOString(),command:'node tools/probe-stamps3bat.mjs --build <fixture-build-directory> --product '+productId,backend:'actual-wasm-worker',productId,firmware:'internal ESP-IDF 5.5.1 protocol fixture',wasmSha256:hash(await readFile(new URL('web/wasm/esp32sim.wasm',root))),inputHashes,checks:[],stages:[],outputs:[],limitations:['This is not manufacturer firmware or a claim of official UIFlow compatibility.','PM1 physical-button polling requires guest disabling automatic reset/off. Autonomous PMIC reset, shutdown, download, IRQ/click timing, charging, optical response and networking are not modeled.','RGB565 is presented using full-range 8-bit channel expansion; this is not measured LED brightness.']};
const delay=ms=>new Promise(r=>setTimeout(r,ms)),same=(a,b)=>JSON.stringify(a)===JSON.stringify(b);
const wait=async(label,predicate)=>{const end=Date.now()+180000;while(Date.now()<end){const s=await runtime.observe();if(predicate(s))return s;if(s.status==='error')throw Error(JSON.stringify(s.lastError));await delay(20);}const s=await runtime.observe();throw Error(`${label} timed out ${JSON.stringify(s.leds)}\n${s.serial.slice(-4000)}`);};
try {
 const scene=createDefaultScene(catalog,productId);scene.firmware=null;await runtime.configure(scene);await runtime.loadFirmware({format:'build',files});await runtime.start();
 const stages=[['POWER_OFF',[0,0,0]],['POWER_ON_NO_REFRESH',[0,0,0]],['RED',[255,0,0]],['BUFFER_GREEN_RMT21_IGNORED',[255,0,0]],['POWER_CUT',[0,0,0]],['RESTORED_NO_REFRESH',[0,0,0]],['GREEN_REFRESH',[0,255,0]]];
 for(let i=0;i<stages.length;i++){const [name,color]=stages[i],s=await wait(name,s=>s.serial.includes('S3BAT_STAGE '+name)&&same(s.leds,[color]));if(i+1<stages.length)assert(!s.serial.includes('S3BAT_STAGE '+stages[i+1][0]),'Missed observation window '+name);receipt.stages.push({name,leds:s.leds});}
 let s=await wait('polling ready',s=>s.serial.includes('S3BAT_BUTTON 0 FIRST 00 SECOND 00')&&same(s.leds,[[255,0,0]]));
 for(const line of ['S3BAT_IDENT 50 20 05 06 POWER 07','S3BAT_WRONG_ADDRESS NACK','S3BAT_WRONG_PINS NACK'])assert(s.serial.includes(line),line);
 assert(s.serial.includes('SPI SRAM memory test OK')&&s.serial.includes('S3BAT_PSRAM 8388608'),'Real IDF Octal PSRAM initialization/memory test\n'+s.serial);
 receipt.checks.push('real-ROM-boot-with-8MB-Octal-PSRAM-memory-test','M5PM1-identity-default-rails-and-wrong-address-and-wrong-pins-NACK','RGB565-buffer-needs-refresh-gated-by-PM1-supply-and-GPIO0-mode','RMT21-does-not-drive-PM1-LED-and-restored-power-needs-fresh-refresh');
 for(const [step,color] of [[1,[0,255,0]],[2,[0,0,255]],[3,[0,0,0]],[0,[255,0,0]]]) {
  let at=s.serial.length;await runtime.input({type:'button',id:'power',pressed:true});s=await wait('fresh PWR press',s=>s.serial.slice(at).includes('S3BAT_BUTTON 1 FIRST 81 SECOND 01')&&s.serial.slice(at).includes('S3BAT_OUTPUT '+step)&&same(s.leds,[color]));const output={step,leds:s.leds,press:s.serial.slice(at).split(/\r?\n/).filter(l=>/^S3BAT_(BUTTON|OUTPUT)/.test(l))};receipt.outputs.push(output);
  at=s.serial.length;await runtime.input({type:'button',id:'power',pressed:false});s=await wait('fresh PWR release',s=>s.serial.slice(at).includes('S3BAT_BUTTON 0 FIRST 00 SECOND 00'));output.release=s.serial.slice(at).split(/\r?\n/).filter(l=>/^S3BAT_BUTTON/.test(l));
 }
 receipt.checks.push('fresh-physical-PM1-button-status-and-read-clear-flag-drive-guest-RGB');await assert.rejects(runtime.input({type:'button',id:'button',pressed:true}));assert.equal(s.frame,null);receipt.checks.push('no-invented-G0-user-button-or-framebuffer');
 await runtime.pause();const paused=await runtime.observe();await assert.rejects(runtime.input({type:'button',id:'power',pressed:true}));await delay(100);const still=await runtime.observe();assert.equal(still.serial,paused.serial);assert.equal(still.stats.time,paused.stats.time);assert.deepEqual(still.leds,paused.leds);await runtime.start();receipt.checks.push('pause-halts-guest-time-output-and-rejects-new-press');
 let at=(await runtime.observe()).serial.length;await runtime.input({type:'button',id:'power',pressed:true});await wait('hold before reset',s=>s.serial.slice(at).includes('S3BAT_BUTTON 1 FIRST 81 SECOND 01'));
 await runtime.reset();s=await wait('fresh reset and released PM1 key',s=>s.serial.includes('S3BAT_READY')&&s.serial.includes('S3BAT_BUTTON 0 FIRST 00 SECOND 00')&&same(s.leds,[[255,0,0]]));assert(!s.serial.includes('S3BAT_BUTTON 1'));assert.equal(s.frame,null);receipt.checks.push('platform-reset-clears-held-PM1-button-buffer-and-power-state');
 await runtime.pause();receipt.guestSeconds=s.stats.time;receipt.serialEvidence=s.serial.split(/\r?\n/).filter(l=>/^S3BAT_|SPI SRAM memory test OK/.test(l));receipt.status='pass';console.log(JSON.stringify(receipt,null,2));
} finally {await runtime.dispose();}
