#!/usr/bin/env node
// Caller-selected official image: bytes stay in RAM and execute in the actual page worker.
import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {basename} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createSdCardStorage} from '../web/platform/sd-card-storage.js';
import {createDefaultScene} from '../web/platform/catalog.js';
const args=process.argv.slice(2),arg=name=>args[args.indexOf(name)+1];
for(const key of ['--url','--sha256'])assert(args.includes(key)&&arg(key),`Required ${key}; explicitly select the K159-B factory image.`);
const url=arg('--url'),expected=arg('--sha256'),root=new URL('../',import.meta.url),baseUrl='https://paperdiy-factory.invalid/platform/';
assert(/^https:\/\//.test(url));assert(/^[a-f0-9]{64}$/.test(expected));
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const response=await fetch(url,{signal:AbortSignal.timeout(45000)});assert(response.ok,`Firmware HTTP ${response.status}`);
const firmware=new Uint8Array(await response.arrayBuffer());assert.equal(hash(firmware),expected,'Changed firmware must be separately reviewed.');
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const fetchImpl=async value=>{const target=new URL(value);if(String(target)===url)return new Response(firmware);assert.equal(target.origin,'https://paperdiy-factory.invalid');try{return new Response(await readFile(new URL(target.pathname.endsWith('.elf')?'web/wasm/fw/'+basename(target.pathname):'web'+target.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root),thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl,assetBaseUrl:'https://paperdiy-factory.invalid/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'paperdiy-factory-probe'}),sdStorage:createSdCardStorage({indexedDB:null,BroadcastChannel:null,dbName:'paperdiy-factory-sd'})});
const receipt={checkedAt:new Date().toISOString(),command:'node tools/probe-paperdiy-factory.mjs --url <catalog-k159-b-factory-url> --sha256 <catalog-sha256>',source:{url,resolvedUrl:response.url,bytes:firmware.length,sha256:hash(firmware)},backend:'actual-page-worker-wasm-jit',firmwarePatched:false,wasmSha256:hash(await readFile(new URL('web/wasm/esp32sim.wasm',root))),checks:[]};
const wait=async predicate=>{const end=Date.now()+180000;while(Date.now()<end){const state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(JSON.stringify(state.lastError));await new Promise(r=>setTimeout(r,50));}throw Error(`Factory probe timed out: ${(await runtime.observe()).serial.slice(-2000)}`);};
const advance=async seconds=>{const start=(await runtime.observe()).stats.time||0;return wait(s=>(s.stats.time||0)>=start+seconds);};
const save=async (name,frame)=>{if(args.includes('--output')){const {join}=await import('node:path');await mkdir(arg('--output'),{recursive:true});await writeFile(join(arg('--output'),`${name}.rgba`),frame.pixels);}return {width:frame.width,height:frame.height,sha256:hash(frame.pixels)};};
const pixel=(frame,x,y)=>Array.from(frame.pixels.slice((y*frame.width+x)*4,(y*frame.width+x)*4+4));
// The fixed manufacturer's benchmark writes/verifies 1 MiB then unlinks its
// unique PDTxxxxx.BIN. Inspect its deleted directory entry and retained sectors,
// independent of the SPI model's response counters or a painted UI indication.
function benchmarkFile(bytes) {
 const v=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength),u16=p=>v.getUint16(p,true),u32=p=>v.getUint32(p,true);
 const boot=u32(454)*512,sector=u16(boot+11),spc=bytes[boot+13],fat=boot+u16(boot+14)*sector,fatBytes=u32(boot+36)*sector,data=fat+bytes[boot+16]*fatBytes,root=data+(u32(boot+44)-2)*spc*sector;
 assert.equal(sector,512);assert.equal(spc,1);assert.equal(bytes[boot+16],2);
 for(let at=root;at<root+spc*sector&&bytes[at];at+=32){
  if(bytes[at]!==0xe5||String.fromCharCode(...bytes.subarray(at+1,at+3))!=='DT'||String.fromCharCode(...bytes.subarray(at+8,at+11))!=='BIN')continue;
  const size=u32(at+28),cluster=u16(at+26)+(u16(at+20)<<16),start=data+(cluster-2)*spc*sector;
  assert.equal(size,1024*1024);assert(start>=data&&start+size<=bytes.length);
  const content=bytes.subarray(start,start+size);for(let i=0;i<size;i++)assert.equal(content[i],(i*31+17)&255,`Official benchmark byte ${i}`);
  for(let c=cluster;c<cluster+size/sector;c++){assert.equal(u32(fat+c*4),0,'Successful unlink releases all clusters');assert.equal(u32(fat+fatBytes+c*4),0,'Both FAT copies agree');}
  return {bytes:size,sha256:hash(content),deleted:true,verifiedContiguousSectors:size/sector};
 }
 return null;
}
const inspectSd=async()=>{await runtime.flushSdCard();const state=await runtime.observe(),bytes=(await runtime.exportSdCard()).bytes;return {card:state.sdCard,sha256:hash(bytes),benchmark:benchmarkFile(bytes)};};
try {
 const scene=createDefaultScene(catalog,'m5stack-k159-b');scene.firmware={url,sha256:expected};await runtime.configure(scene);await runtime.loadFirmware();await runtime.start();
 console.error('phase: official first factory boot');await wait(s=>s.serial.includes('Sending M5PM1 shutdown command'));await advance(.4);await runtime.pause();
 const first=await runtime.capture({format:'rgba'});assert.equal(first.width,540);assert.equal(first.height,960);assert(first.pixels.some((v,i)=>i%4!==3&&v===0));receipt.first=await save('first',first);receipt.checks.push('rom-boot-first-factory-shutdown-logo');
 const storage=await runtime.flushDeviceStorage();assert(!storage.pristine);assert(storage.changedPages>0);receipt.changedPages=storage.changedPages;
 console.error('phase: power cycle retaining guest NVS');await runtime.reset();await wait(s=>s.serial.includes('Display ready: 540x960')&&s.stats.time>3);await runtime.pause();
 let state=await runtime.observe();assert(!state.serial.includes('First factory boot'),'NVS factory_done must survive reset');assert(state.serial.includes('TF mounted: SDHC/SDXC'),'The fixed official firmware must mount the actual SPI card');assert(state.sdCard.present);const main=await runtime.capture({format:'rgba'});assert.notDeepEqual(main.pixels,first.pixels);receipt.main=await save('main',main);receipt.checks.push('factory-nvs-retained-after-host-restart','official-SPI-SDHC-FAT32-mount');await runtime.start();
 for(const [id,y] of [['button-a',620],['button-b',731]]) {
  console.error(`phase: ${id} held border`);await runtime.input({type:'button',id,pressed:true});await advance(.9);await runtime.pause();const held=await runtime.capture({format:'rgba'});
  assert.deepEqual(pixel(held,30,y),[0,0,0,255]);receipt[id]=await save(id,held);await runtime.start();await runtime.input({type:'button',id,pressed:false});await advance(.9);await runtime.pause();const released=await runtime.capture({format:'rgba'});
  assert.deepEqual(pixel(released,30,y),[255,255,255,255]);receipt.checks.push(`${id}-physical-gpio-held-black-released-white-border`);await runtime.start();
 }
 console.error('phase: official B-click TF benchmark');
 for(let attempt=0;attempt<20;attempt++){await advance(2);await runtime.pause();receipt.sd=await inspectSd();if(receipt.sd.benchmark)break;await runtime.start();}
 assert(receipt.sd.benchmark,'B click must complete the official 1 MiB write/read/unlink path');assert(!receipt.sd.card.pristine);receipt.benchmark=await save('benchmark',await runtime.capture({format:'rgba'}));receipt.checks.push('official-B-click-one-MiB-benchmark-exact-pattern-and-unlink');
 state=await runtime.observe();receipt.guestSeconds=state.stats.time;const uart=state.serialByPort.uart0||'';receipt.serialEvidence=[...new Set(uart.replace(/\x1b\[[0-9;]*m/g,'').split(/\r?\n/).filter(line=>/PaperDIY|Display ready|TF mounted|TF mount failed|Wi-Fi scan|Wi-Fi station/.test(line)))];const scans=[...uart.matchAll(/Wi-Fi scan complete: (\d+) networks/g)];receipt.wifiScan={completed:scans.length,networkCounts:scans.map(match=>Number(match[1])),internetVerified:false};assert(scans.length>=2&&scans.every(match=>Number(match[1])===1),'Opening and A click must each complete a scan of the configured virtual AP');receipt.checks.push('official-opening-and-A-click-WiFi-scans-find-virtual-AP');
 console.error('phase: exact card reset and fresh-worker persistence');
 await runtime.reset();await wait(s=>s.serial.includes('TF mounted: SDHC/SDXC')&&s.stats.time>3);await runtime.pause();receipt.sdReset=await inspectSd();assert.deepEqual(receipt.sdReset.benchmark,receipt.sd.benchmark);assert.equal(receipt.sdReset.sha256,receipt.sd.sha256);receipt.checks.push('reset-remount-retains-exact-SD-image');
 await runtime.loadFirmware();assert((await runtime.observe()).sdCard.restored);assert.equal(hash((await runtime.exportSdCard()).bytes),receipt.sd.sha256);receipt.checks.push('new-worker-restores-exact-SD-image-before-execution');
 state=await runtime.observe();receipt.status='pass';receipt.artifacts=state.firmware.artifacts.map(({kind,sha256,bytes})=>({kind,sha256,bytes}));receipt.limitations=['Black/white digital terminal response only, not 16-gray optical calibration.','SPI SD is an immediate digital model; the displayed benchmark speed is not physical-card performance. The benchmark removes its own file; the receipt independently checks the retained sectors and freed FAT entries.','FPC and true power cycling/sleep are unverified. Host restart retains flash but is not a physical PMIC power button. Wi-Fi scan observations do not verify Internet access.','No physical hardware used. Host input duration is not a cycle-exact timing guarantee.'];console.log(JSON.stringify(receipt,null,2));
} finally {await runtime.dispose();}
