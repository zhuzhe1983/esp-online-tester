#!/usr/bin/env node
// Real platform worker and unchanged official Mosaico 0.1.3 image. Local input
// does not certify the publisher's browser CORS policy; test that separately.
import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {join} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {deflateSync} from 'node:zlib';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
const [firmwarePath,out]=process.argv.slice(2);
if(!out)throw Error('Usage: node tools/probe-mosaico-factory.mjs OFFICIAL_0.1.3_BIN OUTPUT_DIRECTORY');
const root=new URL('../',import.meta.url),hash=b=>createHash('sha256').update(b).digest('hex');
const originalSha='7562ff67cad8280fe8fdd06f3be7d6c91f44e5a8cc63fe596721f48da428af9b';
const image=await readFile(firmwarePath);assert.equal(hash(image),originalSha,'Only the fixed unmodified official recovery image is accepted');
await mkdir(out,{recursive:true});
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const origin='https://mosaico-probe.invalid';
const fetchImpl=async value=>{const u=new URL(value);assert.equal(u.origin,origin);try{return new Response(await readFile(new URL('web'+u.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root),thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',error=>adapter.onerror?.(error));return adapter;};
const api=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'mosaico-factory-isolated'})});
const receipt={checkedAt:new Date().toISOString(),productId:'espressif-esp-mosaico',firmwareSha256:originalSha,firmwareBytes:image.length,firmwarePatched:false,input:'caller-local-file',wasmSha256:hash(await readFile(new URL('web/wasm/esp32sim.wasm',root))),romSha256:hash(await readFile(new URL('web/wasm/fw/esp32s31_rev0_rom.elf',root))),checks:[],actions:[],frames:[]};
const wait=async predicate=>{const end=Date.now()+240000;while(Date.now()<end){const s=await api.observe();assert.equal(s.status,'running',JSON.stringify(s.lastError));assert(!s.serial.includes('Guru Meditation Error'));if(predicate(s))return s;await new Promise(r=>setTimeout(r,20));}throw Error('Mosaico guest timeout');};
const advance=async seconds=>{const start=(await api.observe()).stats.time;return wait(s=>s.stats.time>=start+seconds);};
const crc32=b=>{let n=0xffffffff;for(const v of b){n^=v;for(let i=0;i<8;i++)n=(n>>>1)^((n&1)?0xedb88320:0);}return(n^0xffffffff)>>>0;};
const png=f=>{const chunk=(s,b)=>{const t=Buffer.from(s),len=Buffer.alloc(4),crc=Buffer.alloc(4);len.writeUInt32BE(b.length);crc.writeUInt32BE(crc32(Buffer.concat([t,b])));return Buffer.concat([len,t,b,crc]);};const h=Buffer.alloc(13);h.writeUInt32BE(f.width);h.writeUInt32BE(f.height,4);h[8]=8;h[9]=6;const rows=Buffer.alloc((f.width*4+1)*f.height);for(let y=0;y<f.height;y++)rows.set(f.pixels.subarray(y*f.width*4,(y+1)*f.width*4),y*(f.width*4+1)+1);return Buffer.concat([Buffer.from([137,80,78,71,13,10,26,10]),chunk('IHDR',h),chunk('IDAT',deflateSync(rows)),chunk('IEND',Buffer.alloc(0))]);};
const capture=async name=>{await api.pause();const f=await api.capture({format:'rgba'}),p=png(f);assert.equal(f.width,480);assert.equal(f.height,480);await writeFile(join(out,name+'.png'),p);const entry={name,guestSeconds:(await api.observe()).stats.time,rgbaSha256:hash(f.pixels),pngSha256:hash(p)};receipt.frames.push(entry);await api.start();return entry;};
const button=async id=>{const down=await api.input({type:'button',id,pressed:true});try{await advance(.25);}finally{const up=await api.input({type:'button',id,pressed:false});receipt.actions.push({type:'button',id,down,up});}};
let failure;
try {
 await api.configure(createDefaultScene(catalog,'espressif-esp-mosaico'));
 await api.loadFirmware({file:new File([image],'hello-mosaico-complete_0.1.3.bin')});await api.start();
 await wait(s=>s.serial.includes('IRIS_OTA_HEALTHY partition=main_app')&&s.stats.time>=2);
 const welcome=await capture('language');receipt.checks.push('ROM-to-official-main-app','real-480x480-framebuffer');
 const down=await api.input({type:'touch',x:350,y:300,down:true});await advance(.2);const up=await api.input({type:'touch',x:350,y:300,down:false});receipt.actions.push({type:'touch',x:350,y:300,down,up});await advance(2);
 const chinese=await capture('chinese-home');assert.notEqual(chinese.rgbaSha256,welcome.rgbaSha256);receipt.checks.push('CST9220-language-choice-changes-frame');
 await button('ai');await wait(s=>s.serial.includes('CO5300 brightness set to 20%')&&s.serial.includes('Module manager stopped'));await advance(.4);await capture('standby');
 await button('ai');await wait(s=>s.serial.includes('Wake source=1'));await advance(.5);await capture('awake');receipt.checks.push('AI-button-standby-and-wake-guest-output');
 await button('boot');await advance(.5);receipt.checks.push('Boot-input-acknowledged-no-application-action-claimed');
 receipt.capabilities=await api.capabilities();assert.equal(receipt.capabilities.execution.mode,'wasm-interpreter');assert.equal(receipt.capabilities.network.softAp,false);assert.equal(receipt.capabilities.input.touch,true);assert.deepEqual(receipt.capabilities.input.buttons.sort(),['ai','boot']);
 const state=await api.observe();assert.equal(state.firmware.executionMode,'wasm-interpreter');receipt.guestSeconds=state.stats.time;receipt.stats=state.stats;
 receipt.serialEvidence=state.serial.split(/\r?\n/).filter(s=>/ESP-ROM:esp32s31|IRIS_READY|IRIS_OTA_HEALTHY|Controller identified|CO5300 initialized|CO5300 brightness set|Module manager stopped|Wake source=|PSRAM ID read error|tuning fail/.test(s));
 receipt.limitations=['Local import does not prove publisher CORS or portable remote replay.','Boot short-press application behavior not observed.','No physical RF, cloud, BLE, SoftAP, voice, camera/modules, optical brightness calibration or hardware timing acceptance.','NAND guest model is volatile across a new browser runtime; host IMU samples are unavailable.'];
} catch(e){failure=String(e.stack||e);process.exitCode=1;}
finally {receipt.status=failure?'failed':'pass';if(failure){receipt.failure=failure;const s=await api.observe().catch(()=>({}));receipt.serialTail=s.serial?.slice(-2500);receipt.lastError=s.lastError;}await api.dispose();await writeFile(join(out,'receipt.json'),JSON.stringify(receipt,null,2)+'\n');console.log(JSON.stringify({status:receipt.status,checks:receipt.checks,frames:receipt.frames,failure}));}
