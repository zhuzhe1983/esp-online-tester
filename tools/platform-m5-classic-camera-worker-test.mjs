// Public API -> production Worker/WASM -> mask ROM -> original LX6 program.
// Host image changes are stimulus only; guest SCCB/I2S/DMA reads decide pass/fail.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
import {resolve,basename} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createSdCardStorage} from '../web/platform/sd-card-storage.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {m5ClassicCameraProbe} from './fixtures/m5-classic-camera-probe.mjs';
const root=new URL('../',import.meta.url),origin='https://m5-classic-camera.invalid';
const hash=b=>createHash('sha256').update(b).digest('hex');
const catalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async k=>[k,JSON.parse(await readFile(new URL(`web/platform/catalog/${k}.json`,root))).items])));
const report=JSON.parse(await readFile(new URL('docs/platform/research/m5-classic-camera-runtime.json',root)));
const rom=await readFile(process.env.ESP32SIM_ROM_DIR?resolve(process.env.ESP32SIM_ROM_DIR,'esp32_rev300_rom.elf'):new URL('web/wasm/fw/esp32_rev300_rom.elf',root));
const wasm=await readFile(process.env.ESP32SIM_WASM||new URL('web/wasm/esp32sim.wasm',root)),delay=ms=>new Promise(r=>setTimeout(r,ms));
const workerFactory=url=>{const source=`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(d,t)=>parentPort.postMessage(d,t);await import(${JSON.stringify(new URL('web'+url.pathname,root).href)});parentPort.on('message',data=>globalThis.onmessage({data}));`;const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(source)));const adapter={postMessage:(...a)=>thread.postMessage(...a),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const serial=s=>s.serialByPort?.uart0||'';
async function wait(runtime,label,predicate,assertions=[]){const end=Date.now()+60000;let state;while(Date.now()<end){state=await runtime.observe();const text=serial(state);if(text.includes('FAIL ')){const id=/FAIL CAMERA ASSERT #([0-9A-F]{2})/.exec(text);throw Error(label+': '+(id?assertions[parseInt(id[1],16)]:'guest failure')+'; guest '+text);}if(predicate(state))return state;if(state.status==='error')throw Error(label+': '+JSON.stringify(state.lastError));await delay(15);}throw Error(label+': '+JSON.stringify({status:state?.status,serial:serial(state||{}),stats:state?.stats}));}
const picture=rgb=>new Uint8Array([...new TextEncoder().encode('P6\n1 1\n255\n'),...rgb]);
test('fourteen exact M5 camera SKUs capture explicit host pictures through original guest DMA; PoE adds real wired SPI ARP',{timeout:900000},async t=>{
 const images=new Map([['/diagnostic/esp32_rev300_rom.elf',rom]]),receipt={schemaVersion:1,checkedAt:new Date().toISOString(),nodeVersion:process.version,wasmSha256:hash(wasm),romSha256:hash(rom),probeSourceSha256:hash(await readFile(new URL('fixtures/m5-classic-camera-probe.mjs',import.meta.url))),harnessSha256:hash(await readFile(new URL(import.meta.url))),backend:'public-runtime-production-worker-wasm-mask-ROM-original-LX6-SRAM',products:[],physicalValidation:false,factoryFirmwareValidation:false};
 const runtime=createPlatformRuntime({catalog,workerFactory,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'classic-camera-worker-test'}),sdStorage:createSdCardStorage({indexedDB:null,BroadcastChannel:null,dbName:'classic-camera-worker-test'}),fetchImpl:async value=>{const url=new URL(value);assert.equal(url.origin,origin);if(images.has(url.pathname))return new Response(images.get(url.pathname));if(url.pathname.endsWith('/esp32sim.wasm'))return new Response(wasm);if(url.pathname.endsWith('_rom.elf')&&process.env.ESP32SIM_ROM_DIR)return new Response(await readFile(resolve(process.env.ESP32SIM_ROM_DIR,basename(url.pathname))));return new Response(await readFile(new URL('web'+url.pathname,root)));}});
 try{
  const selected=process.env.M5_CAMERA_WORKER_PRODUCTS?.split(',');
  for(const profile of report.products.filter(p=>!selected||selected.includes(p.id))){
   const p=catalog.products.find(p=>p.id===profile.id);assert.equal(p.runtime.board,profile.board);assert.equal(p.runtime.status,'partial');
   const scene=createDefaultScene(catalog,p.id);scene.firmware=null;await runtime.configure(scene);const caps=await runtime.capabilities();assert.equal(caps.camera.hostPicture,true);assert.equal(caps.camera.physicalCapture,false);
   const image=m5ClassicCameraProbe(profile),path='/diagnostic/'+p.id+'.bin';images.set(path,image);
   // The real classic mask ROM loads this original SRAM diagnostic at Flash0x1000.
   // A declared diagnostic app/partition table satisfies the public bundle contract.
   const table=Buffer.alloc(0xc00,255);table.writeUInt16LE(0x50aa);table[2]=0;table[3]=0;table.writeUInt32LE(0x10000,4);table.writeUInt32LE(0x10000,8);table.fill(0,12,32);table.write('camera-probe',12);const tablePath='/diagnostic/partitions.bin';images.set(tablePath,table);
   const manifest={board:profile.board,flash_mb:p.runtime.flashMb,psram_mb:p.runtime.psramMb,app_direct:false,stubs:[],files:{rom:{url:origin+'/diagnostic/esp32_rev300_rom.elf',sha256:hash(rom)},bootloader:{url:origin+path,sha256:hash(image)},ptable:{url:origin+tablePath,sha256:hash(table)},app:{url:origin+path,sha256:hash(image)}}};
   const manifestPath='/diagnostic/'+p.id+'.json',manifestBytes=new TextEncoder().encode(JSON.stringify(manifest));images.set(manifestPath,manifestBytes);await runtime.loadFirmware({manifestUrl:origin+manifestPath,sha256:hash(manifestBytes)});
   const red=picture([255,0,0]),redReceipt=await runtime.setCameraPicture({bytes:red});assert.equal(redReceipt.accepted,true);assert.equal(redReceipt.sha256,hash(red));await runtime.start();
   await wait(runtime,p.id+' red DMA',s=>serial(s).includes('RED DMA PASS\n'),image.assertions);
   const green=picture([0,255,0]),greenReceipt=await runtime.setCameraPicture({bytes:green});assert.equal(greenReceipt.accepted,true);assert.equal(greenReceipt.sha256,hash(green));
   const state=await wait(runtime,p.id+' green DMA and reset',s=>serial(s).includes('CAMERA PROBE PASS\n'),image.assertions);const text=serial(state);
   for(const marker of ['WRONG SCCB PASS','SENSOR PID PASS','WRONG DVP PASS','RED DMA PASS','RESET PASS','GREEN DMA PASS','CAMERA PROBE PASS'])assert.ok(text.includes(marker+'\n'),p.id+' missing '+marker);
   if(p.id.startsWith('m5stack-u121'))for(const marker of ['W5500 FIFO PASS','W5500 PHASE PASS','WIRED ARP PASS'])assert.ok(text.includes(marker+'\n'),p.id+' missing '+marker);
   assert.equal(state.firmware.appDirect,false);assert.deepEqual(state.firmware.stubs,[]);await runtime.pause();
   const row={productId:p.id,board:profile.board,sensor:profile.sensor,flashMb:p.runtime.flashMb,psramMb:p.runtime.psramMb,probeSha256:hash(image),probeBytes:image.length,guestSerial:text,rawFrameWordsCheckedPerColor:16,hostInputs:[redReceipt,greenReceipt],wrongSccbNack:true,wrongD0NoDma:true,resetNoDmaAndReinitialized:true,w5500:p.id.startsWith('m5stack-u121')?{fullDuplexFifo:true,hardwareCommandAddressPhases:true,wiredArpFrameBytesChecked:44,irqAndRecv:true}:null};receipt.products.push(row);t.diagnostic(JSON.stringify(row));
  }
  receipt.limitations=['Original SRAM diagnostic is loaded by the unmodified mask ROM as a diagnostic bootloader; this is not a manufacturer image, SDK startup or factory compatibility claim.','Host PPM pixels are sensor input; every acceptance decision follows guest SCCB/LEDC/GPIO/I2S/native DMA memory operations. No host-painted camera framebuffer is accepted as execution.','W5500 test uses actual guest SPI Socket0 MACRAW and the independent local wired VirtualNet peer; TCP/UDP hardwired socket offload, physical PHY/PoE/FCS and internet reachability are not validated.','Node uses the production runtime/Worker/WASM path without browser GPU rendering or physical hardware. Camera optics, ISP, JPEG encoder and analog/battery/RTC behavior are excluded.'];
  if(process.env.M5_CAMERA_WORKER_RECEIPT)await writeFile(process.env.M5_CAMERA_WORKER_RECEIPT,JSON.stringify(receipt,null,2)+'\n');
 }finally{await runtime.dispose();}
});
