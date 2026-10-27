#!/usr/bin/env node
// Run the source-built internal AtomS3R fixture on the actual page worker/WASM path.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {resolve,join,basename} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {inspectLocalBuild} from '../web/platform/local-firmware.js';
import {createDefaultScene} from '../web/platform/catalog.js';
const index=process.argv.indexOf('--build');if(index<0||!process.argv[index+1])throw Error('Build tools/fixtures/atoms3r-firmware with ESP-IDF 5.5.1 and supply --build <build-directory>.');
const folder=resolve(process.argv[index+1]),root=new URL('../',import.meta.url),baseUrl='https://atoms3r-fixture.invalid/platform/';
const files=[new File([await readFile(join(folder,'flasher_args.json'))],'flasher_args.json')],inputHashes={};
for(const path of (await inspectLocalBuild(files)).missingFiles){const bytes=await readFile(join(folder,path)),file=new File([bytes],basename(path));inputHashes[path]=createHash('sha256').update(bytes).digest('hex');Object.defineProperty(file,'webkitRelativePath',{value:path});files.push(file);}
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const fetchImpl=async value=>{const url=new URL(value);assert.equal(url.origin,'https://atoms3r-fixture.invalid');try{return new Response(await readFile(new URL('web'+url.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root);const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl,assetBaseUrl:'https://atoms3r-fixture.invalid/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'atoms3r-probe'})});
const receipt={checkedAt:new Date().toISOString(),command:'node tools/probe-atoms3r.mjs --build <fixture-build-directory>',backend:'actual-wasm-worker',firmware:'internal ESP-IDF 5.5.1 protocol fixture',inputHashes,checks:[]};
const wait=async predicate=>{const end=Date.now()+90000;while(Date.now()<end){const state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(JSON.stringify(state.lastError));await new Promise(r=>setTimeout(r,50));}throw Error(`Fixture timed out: ${(await runtime.observe()).serial.slice(-4000)}`);};
try {
  const scene=createDefaultScene(catalog,'m5stack-c126');scene.firmware=null;await runtime.configure(scene);await runtime.loadFirmware({format:'build',files});await runtime.start();
  await wait(s=>s.serial.includes('ATOMS3R_PANEL_ID 83 76 00 00')&&s.serial.includes('ATOMS3R_IMU_ID 24')&&s.serial.includes('ATOMS3R_IMU_INIT 01')&&s.serial.includes('ATOMS3R_PSRAM 8388608 bytes read-write passed')&&s.serial.includes('ATOMS3R_PACKAGE 1')&&s.serial.includes('ATOMS3R_BUTTON 1')&&s.frame?.width===128);receipt.checks.push('real-boot-gpio41-idle-bmi270-init-package-pico-8m-octal-psram');
  await runtime.pause();const frame=await runtime.capture({format:'rgba'});assert.equal(frame.width,128);assert.equal(frame.height,128);
  for(let y=0;y<128;y++)for(let x=0;x<128;x++)assert.deepEqual([...frame.pixels.subarray((y*128+x)*4,(y*128+x)*4+4)],x<64?[255,0,0,255]:[0,255,0,255]);
  receipt.frameSha256=createHash('sha256').update(frame.pixels).digest('hex');receipt.checks.push('guest-spi-frame-every-pixel-red-green');
  await runtime.start();
  for(const brightness of [0,128,255]) {
    const serialAt=(await runtime.observe()).serial.length;
    await runtime.input({type:'button',id:'button',pressed:true});await wait(s=>s.serial.slice(serialAt).includes(`ATOMS3R_BACKLIGHT ${brightness}`));
    await runtime.input({type:'button',id:'button',pressed:false});await wait(s=>s.serial.slice(serialAt).includes('ATOMS3R_BUTTON 1'));await runtime.pause();
    const dim=await runtime.capture({format:'rgba'});
    for(let y=0;y<128;y++)for(let x=0;x<128;x++){const r=Math.floor(31*brightness/255),g=Math.floor(63*brightness/255);const expected=x<64?[(r<<3)|(r>>2),0,0,255]:[0,(g<<2)|(g>>4),0,255];assert.deepEqual([...dim.pixels.subarray((y*128+x)*4,(y*128+x)*4+4)],expected);}
    receipt.checks.push(`guest-gpio41-press-release-and-lp5562-backlight-${brightness}-every-pixel`);await runtime.start();
  }
  await runtime.setImu({quaternion:[0,0,0,1]});await wait(s=>s.serial.includes('ATOMS3R_ACCEL 0 4096 0'));
  await runtime.setImu({quaternion:[0,0,Math.SQRT1_2,Math.SQRT1_2],angularVelocityDps:[125,0,0]});await wait(s=>s.serial.includes('ATOMS3R_ACCEL 4096 0 0')&&s.serial.includes('ATOMS3R_GYRO 2048 0 0'));receipt.checks.push('host-orientation-to-guest-bmi270-i2c-accel-8g-and-gyro-2000dps');
  await runtime.pause();const pauseAt=(await runtime.observe()).serial.length;await runtime.start();const state=await wait(s=>s.serial.slice(pauseAt).includes('ATOMS3R_GYRO 0 0 0')&&s.serial.slice(pauseAt).includes('ATOMS3R_ACCEL 4096 0 0'));receipt.checks.push('pause-clears-gyro-keeps-accel-in-guest-registers');
  await runtime.pause();receipt.status='pass';receipt.guestSeconds=state.stats.time;receipt.serialPort='uart0';receipt.serialEvidence=(state.serialByPort.uart0||'').split(/\r?\n/).filter(l=>/^ATOMS3R_/.test(l)).filter((l,i,a)=>a.indexOf(l)===i);console.log(JSON.stringify(receipt,null,2));
} finally {await runtime.dispose();}
