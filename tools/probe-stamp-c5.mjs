#!/usr/bin/env node
// Real worker/WASM proof with a caller-supplied GCC/esptool SRAM application.
// The repository fixture is diagnostic firmware, never a public factory image.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
const arg=name=>{const i=process.argv.indexOf(name);return i<0?null:process.argv[i+1];};
const file=arg('--firmware');assert(file,'Supply --firmware <C5 SRAM probe.bin>.');
const root=arg('--assets-root')?pathToFileURL(resolve(arg('--assets-root'))+'/'):new URL('../',import.meta.url),origin='https://c5-probe.invalid',hash=b=>createHash('sha256').update(b).digest('hex');
const bytes=new Uint8Array(await readFile(file)),catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const requests=[],fetchImpl=async value=>{const u=new URL(value);assert.equal(u.origin,origin);requests.push(u.pathname);assert(!u.pathname.includes('rom.elf'),'C5 must never request any ROM');if(u.pathname==='/probe.bin')return new Response(bytes);try{return new Response(await readFile(new URL('web'+u.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root),thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const w={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};thread.on('message',data=>w.onmessage?.({data}));thread.on('error',e=>w.onerror?.(e));return w;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/'}),receipt={checkedAt:new Date().toISOString(),backend:'actual-wasm-worker',command:'node tools/probe-stamp-c5.mjs --firmware <probe.bin> [--assets-root <repository>]',firmwareSha256:hash(bytes),firmwareBytes:bytes.length,wasmSha256:hash(await readFile(new URL('web/wasm/esp32sim.wasm',root))),products:[],limitations:['Independent GCC/esptool SRAM diagnostic, not factory/UIFlow compatibility.','Remote-source mode uses a local-file fetch adapter; external HTTPS/CORS is not tested.','No ROM/cache/IDF startup, WiFi, audio, SD, OTA or device Flash persistence.','Functional clock and queued UART0 stimulus, not calibrated hardware timing or physical serial routing.']};
const delay=ms=>new Promise(r=>setTimeout(r,ms)),serial=s=>s.serialByPort.uart0||'';
async function wait(label,fn){const end=Date.now()+30000;while(Date.now()<end){const s=await runtime.observe();if(fn(s))return s;if(s.status==='error')throw Error(JSON.stringify(s.lastError));await delay(20);}throw Error(label+' timed out: '+JSON.stringify(await runtime.observe()));}
try {
 for(const productId of ['m5stack-s016','m5stack-s016-dip']) {
  const scene=createDefaultScene(catalog,productId);scene.firmware=null;await runtime.configure(scene);
  const loaded=await runtime.loadFirmware({file:new File([bytes],'probe.bin')});assert.equal(loaded.firmware.resetMode,'reload-same-sram-image');assert.equal(loaded.firmware.artifacts.length,1);assert.equal(loaded.firmware.artifacts[0].kind,'app');await runtime.start();
  let s=await wait('ready',s=>serial(s).includes('C5 UART IRQ READY\n')&&JSON.stringify(s.leds)==='[[0,0,0]]');const item={productId,checks:['local-single-file-SRAM-app-direct'],input:[]};receipt.products.push(item);
  async function command(data,color){const before=serial(await runtime.observe());await runtime.input({type:'serial',port:'uart0',data});s=await wait('fresh '+data,s=>serial(s).slice(before.length).includes(data)&&JSON.stringify(s.leds)===JSON.stringify([color]));item.input.push({byte:data,echo:serial(s).slice(before.length),led:s.leds[0],guestSeconds:s.stats.time});}
  await command('1',[0,0,255]);await command('0',[0,0,0]);await command('W',[0,0,0]);item.checks.push('UART0-ISR-on-off-and-wrong-G27-does-not-drive-G28');
  await assert.rejects(()=>runtime.input({type:'serial',port:'usb',data:'1'}),{code:'UNSUPPORTED_PORT'});await assert.rejects(()=>runtime.input({type:'serial',port:'uart1',data:'1'}),{code:'UNSUPPORTED_PORT'});
  await runtime.pause();const a=await runtime.observe();await delay(100);const b=await runtime.observe();assert.equal(b.stats.time,a.stats.time);assert.equal(serial(b),serial(a));await assert.rejects(()=>runtime.input({type:'serial',port:'uart0',data:'1'}),{code:'NOT_RUNNING'});await runtime.start();await command('1',[0,0,255]);item.checks.push('pause-stops-guest-and-rejects-input');
  await runtime.reset();s=await wait('app-direct reset',s=>serial(s)==='C5 UART IRQ READY\n'&&JSON.stringify(s.leds)==='[[0,0,0]]');await command('1',[0,0,255]);item.checks.push('reset-reloads-same-RAM-app-and-clears-lit-LED');assert.equal(s.frame,null);assert.equal(s.storage.status,'unsupported');assert.equal(s.audio.available,false);assert.equal(s.network.supported,false);
  await runtime.loadFirmware({url:origin+'/probe.bin',sha256:hash(bytes),bootMode:'app-direct'});await runtime.start();s=await wait('explicit remote',s=>serial(s).includes('C5 UART IRQ READY\n'));await command('1',[0,0,255]);item.checks.push('explicit-remote-same-image-and-pinned-mode');await runtime.pause();
 }
 receipt.romRequests=requests.filter(p=>p.includes('rom')).length;receipt.status='pass';console.log(JSON.stringify(receipt,null,2));
}finally{await runtime.dispose();}
