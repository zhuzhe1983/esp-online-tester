// Public setCameraPicture -> production Worker -> compiled WASM -> real LX7
// I2C/GPIO/LCD_CAM/GDMA -> guest RAM/CRC/UART. An input ACK is never success.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve,basename,dirname} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {crc32} from 'node:zlib';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createSdCardStorage} from '../web/platform/sd-card-storage.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {M5_CAMERA_PROFILES,m5CameraCaptureProbe} from './fixtures/m5-camera-capture-probe.mjs';
const root=new URL('../',import.meta.url),origin='https://m5-camera-worker.invalid';
const hash=b=>createHash('sha256').update(b).digest('hex'),hex32=n=>(n>>>0).toString(16).padStart(8,'0');
const catalogBytes=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async k=>[k,await readFile(new URL(`web/platform/catalog/${k}.json`,root))])));
const catalog=Object.fromEntries(Object.entries(catalogBytes).map(([k,b])=>[k,JSON.parse(b).items]));
const productionEntrySha256=Object.fromEntries(await Promise.all(['web/platform/runtime.js','web/wasm/worker.js','web/wasm/jit.mjs','web/wasm/pacing.mjs'].map(async file=>[file,hash(await readFile(new URL(file,root)))])));
const wasm=await readFile(process.env.ESP32SIM_WASM||new URL('web/wasm/esp32sim.wasm',root));
const selectedIds=process.env.M5_CAMERA_IDS?.split(',').filter(Boolean),profiles=selectedIds?M5_CAMERA_PROFILES.filter(p=>selectedIds.includes(p.id)):M5_CAMERA_PROFILES;
if(selectedIds)assert.equal(profiles.length,new Set(selectedIds).size,'M5_CAMERA_IDS must contain known exact product IDs');
const jpegs=JSON.parse(await readFile(new URL('fixtures/m5-camera-jpeg.json',import.meta.url)));
const delay=ms=>new Promise(r=>setTimeout(r,ms)),serial=s=>s.serialByPort?.uart0||'';
const captures=text=>[...text.matchAll(/CAPTURE EOF=([01]) HEAD=([a-f0-9]{32}) CRC=([a-f0-9]{8}) END\n/g)].map(m=>({eof:Number(m[1]),head:m[2],crc32:m[3]}));
const ppm=rgb=>Buffer.concat([Buffer.from('P6\n1 1\n255\n'),Buffer.from(rgb)]);
const bmp=rgb=>{const b=Buffer.alloc(58);b.write('BM');b.writeUInt32LE(58,2);b.writeUInt32LE(54,10);b.writeUInt32LE(40,14);b.writeInt32LE(1,18);b.writeInt32LE(1,22);b.writeUInt16LE(1,26);b.writeUInt16LE(24,28);b.writeUInt32LE(4,34);b.set([rgb[2],rgb[1],rgb[0]],54);return b;};
async function wait(runtime,label,predicate,from=0){const end=Date.now()+30000;let state;while(Date.now()<end){state=await runtime.observe();const text=serial(state).slice(from),found=predicate(state,text);if(found)return{state,found};if(state.status==='error')throw Error(label+': '+JSON.stringify(state.lastError));await delay(15);}throw Error(label+': '+JSON.stringify({status:state?.status,serial:serial(state||{}).slice(-2000),stats:state?.stats}));}
test('six exact camera products consume public host pictures in actual LX7 DMA RAM',{timeout:360000},async t=>{
 assert.equal(hex32(crc32(Buffer.from('123456789'))),'cbf43926','independent standard CRC32 reference');
 const images=new Map(),workerInputs=[],romHashes={};
 const workerFactory=url=>{const source=`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(d,t)=>parentPort.postMessage(d,t);await import(${JSON.stringify(new URL('web'+url.pathname,root).href)});parentPort.on('message',data=>globalThis.onmessage({data}));`;const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(source)));const adapter={postMessage:(...a)=>{const m=a[0];if(m?.op==='load'&&m.kind===7)workerInputs.push({op:m.op,kind:m.kind,bytes:m.data?.byteLength});thread.postMessage(...a);},terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
 const runtime=createPlatformRuntime({catalog,workerFactory,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'m5-camera-worker'}),sdStorage:createSdCardStorage({indexedDB:null,BroadcastChannel:null,dbName:'m5-camera-worker'}),fetchImpl:async value=>{const url=new URL(value);assert.equal(url.origin,origin);if(images.has(url.pathname))return new Response(images.get(url.pathname));if(url.pathname.endsWith('/esp32sim.wasm'))return new Response(wasm);const data=await readFile(url.pathname.endsWith('_rom.elf')&&process.env.ESP32SIM_ROM_DIR?resolve(process.env.ESP32SIM_ROM_DIR,basename(url.pathname)):new URL('web'+url.pathname,root));if(url.pathname.endsWith('_rom.elf'))romHashes[basename(url.pathname)]=hash(data);return new Response(data);}});
 const receipt={schemaVersion:1,checkedAt:new Date().toISOString(),backend:'public-runtime-production-worker-wasm-original-LX7-SRAM-camera-DMA-RAM-CRC-UART',nodeVersion:process.version,wasmSha256:hash(wasm),productionEntrySha256,catalogFileSha256:Object.fromEntries(Object.entries(catalogBytes).map(([k,b])=>[k,hash(b)])),probeSourceSha256:hash(await readFile(new URL('fixtures/m5-camera-capture-probe.mjs',import.meta.url))),jpegFixtureSha256:hash(await readFile(new URL('fixtures/m5-camera-jpeg.json',import.meta.url))),harnessSha256:hash(await readFile(new URL(import.meta.url))),products:[],physicalValidation:false,factoryFirmwareValidation:false};
 try{
  for(const p of profiles){
   const product=catalog.products.find(item=>item.id===p.id);assert.equal(product.runtime.board,p.board);const scene=createDefaultScene(catalog,p.id);scene.firmware=null;await runtime.configure(scene);
   const caps=await runtime.capabilities();assert.equal(caps.camera?.hostPicture,true,p.id+' must advertise sensor-input support');assert.equal(typeof runtime.setCameraPicture,'function');assert.equal(caps.camera.recording,false);assert.equal(caps.camera.shareable,false);
   const row={productId:p.id,board:p.board,sensor:p.sensor,dimensions:[p.width,p.height],inputs:[],negative:[]};
   async function load(options={}){const image=m5CameraCaptureProbe(p.id,options),path='/diagnostic/'+p.id+'-'+row.inputs.length+'-'+row.negative.length+'.bin';images.set(path,image);await runtime.loadFirmware({url:origin+path,sha256:hash(image),bootMode:'app-direct'});return hash(image);}
   async function picture(bytes){const input=await runtime.setCameraPicture({bytes:new Uint8Array(bytes)});assert.equal(input.accepted,true);assert.equal(input.delivery,'sensor-input');assert.equal(input.sha256,hash(bytes));assert.equal(input.bytes,bytes.length);assert.equal(input.physicalCapture,false);return input;}
   async function observed(expected,eof,from=0){const want={eof,head:Buffer.from(expected.subarray(0,16)).toString('hex'),crc32:hex32(crc32(expected))};const {state,found}=await wait(runtime,p.id+' DMA '+JSON.stringify(want),(_s,text)=>captures(text).find(c=>c.eof===want.eof&&c.head===want.head&&c.crc32===want.crc32),from);return{...found,expectedSha256:hash(expected),serialTail:serial(state).slice(-400)};}
   const imageHash=await load();let before=serial(await runtime.observe()).length;
   // PPM and BMP are independent decoded-picture inputs. Their source bytes
   // differ; both colors must be consumed by the already-running same guest.
   const red=ppm([255,0,0]),green=bmp([0,255,0]),redWire=Buffer.from([248,0]),greenWire=Buffer.from([7,224]);
   const rawExpected=color=>Buffer.concat(Array.from({length:p.width*p.height},()=>color));
   const first=await picture(red);await runtime.start();row.inputs.push({format:'PPM',probeSha256:imageHash,input:first,guest:await observed(rawExpected(redWire),1,before)});
   before=serial(await runtime.observe()).length;const second=await picture(green);row.inputs.push({format:'BMP',input:second,guest:await observed(rawExpected(greenWire),1,before)});await runtime.pause();
   const state=await runtime.observe();assert.equal(state.firmware.appDirect,true);assert.deepEqual(state.firmware.stubs,[]);
   // Public reset replaces the Worker: the private host image is gone. The
   // guest sees the explicit model default (black) until the caller resupplies.
   await runtime.reset();row.inputs.push({format:'default-black-after-worker-reset',guest:await observed(Buffer.alloc(p.width*p.height*2),1)});await runtime.pause();
   before=serial(await runtime.observe()).length;await picture(green);await runtime.start();row.inputs.push({format:'explicit-resupply-after-reset',guest:await observed(rawExpected(greenWire),1,before)});await runtime.pause();
   for(const fault of ['route','clock','power']){
    const probeSha256=await load({fault});await picture(red);before=serial(await runtime.observe()).length;await runtime.start();row.negative.push({fault,probeSha256,guest:await observed(Buffer.alloc(p.width*p.height*2,0xa5),0,before)});await runtime.pause();
   }
   let retainedExpected;
   if(p.sensor==='gc0308'){
    const j=jpegs.items.find(j=>j.width===8&&j.height===4);await assert.rejects(runtime.setCameraPicture({bytes:new Uint8Array(Buffer.from(j.base64,'base64'))}),/jpeg|format|unsupported|reject/i);row.jpegUnsupported=true;await load();await picture(green);await runtime.start();retainedExpected=rawExpected(greenWire);await observed(retainedExpected,1);await runtime.pause();
   }else{
    const j=jpegs.items.find(j=>j.width===p.width&&j.height===p.height),bytes=Buffer.from(j.base64,'base64');assert.equal(hash(bytes),j.sha256);assert.equal(bytes.length,j.bytes);
    const probeSha256=await load({format:'jpeg',jpegBytes:bytes.length});const input=await picture(bytes);before=serial(await runtime.observe()).length;await runtime.start();row.inputs.push({format:'baseline-JPEG-verbatim',probeSha256,input,guest:await observed(bytes,1,before)});retainedExpected=bytes;await runtime.pause();
   }
   await assert.rejects(runtime.setCameraPicture({bytes:new Uint8Array([0,1,2,3])}),/image|picture|camera|format|reject/i);
   await assert.rejects(runtime.setCameraPicture({bytes:new Uint8Array(8*1024*1024+1)}),/large|limit|size|8|camera|picture/i);
   before=serial(await runtime.observe()).length;await runtime.start();row.inputs.push({format:'invalid-input-rejection-keeps-previous-source',guest:await observed(retainedExpected,1,before)});await runtime.pause();
   receipt.products.push(row);t.diagnostic(JSON.stringify(row));
  }
  assert(workerInputs.length>=profiles.length*2,'public pictures must reach the production Worker load kind7 channel');receipt.romSha256=romHashes;receipt.workerInputMessages=workerInputs;receipt.limitations=['Original app-direct LX7 SRAM diagnostics; no official factory image or full ESP-IDF startup validation.','Host uploads enter the production sensor-input API only. All sensor and DMA setup, RAM byte reads and CRC32 computations execute in the guest.','Synthetic single-color raw inputs and valid baseline JPEG files test transport, not real optics/ISP/encoder, UVC, microphone, cloud or physical power behavior.','Node production Worker/WASM execution does not validate browser rendering or a physical camera.'];
  if(process.env.M5_CAMERA_WORKER_RECEIPT){await mkdir(dirname(resolve(process.env.M5_CAMERA_WORKER_RECEIPT)),{recursive:true});await writeFile(process.env.M5_CAMERA_WORKER_RECEIPT,JSON.stringify(receipt,null,2)+'\n');}
 }finally{await runtime.dispose();}
});
