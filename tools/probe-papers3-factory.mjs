#!/usr/bin/env node
// Caller-selected official image: bytes stay in RAM and execute in the actual page worker.
import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {basename} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createDefaultScene} from '../web/platform/catalog.js';
const args=process.argv.slice(2),arg=name=>args[args.indexOf(name)+1];
for(const key of ['--url','--sha256'])assert(args.includes(key)&&arg(key),`Required ${key}; explicitly select the C139 factory image.`);
const url=arg('--url'),expected=arg('--sha256'),root=new URL('../',import.meta.url),baseUrl='https://papers3-factory.invalid/platform/';
assert(/^https:\/\//.test(url));assert(/^[a-f0-9]{64}$/.test(expected));
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const response=await fetch(url,{signal:AbortSignal.timeout(45000)});assert(response.ok,`Firmware HTTP ${response.status}`);
const firmware=new Uint8Array(await response.arrayBuffer());assert.equal(hash(firmware),expected,'Changed firmware must be separately reviewed.');
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const fetchImpl=async value=>{const target=new URL(value);if(String(target)===url)return new Response(firmware);assert.equal(target.origin,'https://papers3-factory.invalid');try{return new Response(await readFile(new URL(target.pathname.endsWith('.elf')?'web/wasm/fw/'+basename(target.pathname):'web'+target.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root),thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl,assetBaseUrl:'https://papers3-factory.invalid/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'papers3-factory-probe'})});
const receipt={checkedAt:new Date().toISOString(),command:'node tools/probe-papers3-factory.mjs --url <catalog-c139-factory-url> --sha256 <catalog-sha256>',source:{url,resolvedUrl:response.url,bytes:firmware.length,sha256:hash(firmware)},backend:'actual-page-worker-wasm-jit',firmwarePatched:false,wasmSha256:hash(await readFile(new URL('web/wasm/esp32sim.wasm',root))),checks:[]};
const wait=async predicate=>{const end=Date.now()+180000;while(Date.now()<end){const state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(JSON.stringify(state.lastError));await new Promise(r=>setTimeout(r,50));}throw Error(`Factory probe timed out: ${(await runtime.observe()).serial.slice(-2000)}`);};
const advance=async seconds=>{const start=(await runtime.observe()).stats.time||0;return wait(s=>(s.stats.time||0)>=start+seconds);};
const save=async (name,frame)=>{if(args.includes('--output')){const {join}=await import('node:path');await mkdir(arg('--output'),{recursive:true});await writeFile(join(arg('--output'),`${name}.rgba`),frame.pixels);}return {width:frame.width,height:frame.height,sha256:hash(frame.pixels)};};
const darkPixels=(frame,x,y,w,h)=>{let count=0;for(let yy=y;yy<y+h;yy++)for(let xx=x;xx<x+w;xx++)if(frame.pixels[(yy*frame.width+xx)*4]<128)count++;return count;};
const crop=(frame,x,y,w,h)=>{const out=new Uint8Array(w*h*4);for(let row=0;row<h;row++)out.set(frame.pixels.subarray(((y+row)*frame.width+x)*4,((y+row)*frame.width+x+w)*4),row*w*4);return hash(out);};
try {
 const scene=createDefaultScene(catalog,'m5stack-c139');scene.firmware={url,sha256:expected};await runtime.configure(scene);await runtime.loadFirmware();await runtime.start();
 console.error('phase: official factory boot');await wait(s=>s.serial.includes('AppWifi')&&s.stats.time>14);await runtime.pause();
 let state=await runtime.observe();assert(state.serial.includes('board_M5PaperS3'));const first=await runtime.capture({format:'rgba'});assert.equal(first.width,540);assert.equal(first.height,960);assert(first.pixels.some((v,i)=>i%4!==3&&v===0));receipt.first=await save('first',first);receipt.checks.push('rom-boot-real-factory-test-ui');
 const buzzerBefore=crop(first,30,270,160,160);await runtime.start();
 const tap=async(x,y)=>{const down=await runtime.input({type:'touch',x,y,down:true});await advance(.15);const up=await runtime.input({type:'touch',x,y,down:false});await advance(4.5);return {down,up};};
 receipt.buzzerInput=await tap(116,330);await runtime.pause();const toggled=await runtime.capture({format:'rgba'});receipt.buzzer=await save('buzzer',toggled);assert.notEqual(crop(toggled,30,270,160,160),buzzerBefore,'Touch must change the official buzzer icon');receipt.checks.push('GT911-read-clear-press-release-toggles-official-buzzer-icon-no-audio-claim');
 await runtime.start();receipt.buzzerReturnInput=await tap(116,330);await runtime.pause();const returned=await runtime.capture({format:'rgba'});receipt.returned=await save('returned',returned);// Check the interior mute slash: optical edge pixels differ after partial versus full refresh.
 receipt.muteSlash={region:[112,326,10,10],first:darkPixels(first,112,326,10,10),on:darkPixels(toggled,112,326,10,10),returned:darkPixels(returned,112,326,10,10)};assert(receipt.muteSlash.first>40&&receipt.muteSlash.on<10&&receipt.muteSlash.returned>40,'Second touch must restore the mute slash');receipt.checks.push('second-GT911-tap-restores-official-icon');
 const imuBefore=crop(returned,5,10,140,215);await runtime.start();await runtime.setImu({source:'agent',quaternion:[0,0,Math.SQRT1_2,Math.SQRT1_2],angularVelocityDps:[5,6,7]});await advance(2);await runtime.pause();const motion=await runtime.capture({format:'rgba'});assert.notEqual(crop(motion,5,10,140,215),imuBefore,'BMI270 guest text must change');receipt.imu=await save('imu',motion);receipt.checks.push('explicit-virtual-BMI270-sample-changes-official-text');
 await runtime.start();const serialAt=(await runtime.observe()).serial.length;receipt.scanInput=await tap(225,640);await wait(s=>s.serial.slice(serialAt).includes('start wifi scan task'));await advance(3);await runtime.pause();const scanned=await runtime.capture({format:'rgba'});receipt.scan=await save('scan',scanned);receipt.checks.push('physical-GT911-scan-touch-starts-fresh-guest-WiFi-task');
 state=await runtime.observe();receipt.status='pass';receipt.guestSeconds=state.stats.time;receipt.artifacts=state.firmware.artifacts.map(({kind,sha256,bytes})=>({kind,sha256,bytes}));receipt.serialEvidence=[...new Set((state.serialByPort.uart0||'').replace(/\x1b\[[0-9;]*m/g,'').split(/\r?\n/).filter(line=>/board_M5PaperS3|AppWifi|AppBuzzer|AppImu|start wifi scan task|wifi scan completed/.test(line)))];receipt.limitations=['Digital black/white endpoint, not calibrated 16-gray optical pulse integration.','One contact; only default GT911 scan/ready/clear and polling input are covered, not multi-touch/config/gesture firmware.','RTC, TF, audio, PMS150G physical power key and host networking are absent. Buzzer icon change does not claim sound; empty Wi-Fi scan does not claim RF.','Explicit BMI270 samples use virtual axes; physical mounting/scene rotation not calibrated. No physical hardware used.'];console.log(JSON.stringify(receipt,null,2));
} finally {await runtime.dispose();}
