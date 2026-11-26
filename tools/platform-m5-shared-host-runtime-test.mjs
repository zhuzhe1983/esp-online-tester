// Actual public runtime -> production Worker/WASM -> original LX7 SRAM program.
// No manufacturer image is substituted or assigned to a different SKU.
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
import {m5SharedHostProbe} from './fixtures/m5-shared-host-probe.mjs';
const root=new URL('../',import.meta.url),origin='https://m5-shared-host.invalid';
const hash=b=>createHash('sha256').update(b).digest('hex');
const catalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async k=>[k,JSON.parse(await readFile(new URL(`web/platform/catalog/${k}.json`,root))).items])));
const wasmPath=process.env.ESP32SIM_WASM||new URL('web/wasm/esp32sim.wasm',root);
const wasm=await readFile(wasmPath),delay=ms=>new Promise(r=>setTimeout(r,ms));
const workerFactory=url=>{const source=`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(d,t)=>parentPort.postMessage(d,t);await import(${JSON.stringify(new URL('web'+url.pathname,root).href)});parentPort.on('message',data=>globalThis.onmessage({data}));`;const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(source)));const adapter={postMessage:(...a)=>thread.postMessage(...a),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const serial=s=>s.serialByPort?.uart0||'';
async function wait(runtime,label,predicate){const end=Date.now()+30000;let state;while(Date.now()<end){state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(label+': '+JSON.stringify(state.lastError));await delay(10);}throw Error(label+': '+JSON.stringify({status:state?.status,serial:serial(state||{}),stats:state?.stats}));}
test('K159 and C156 shared physical host profiles produce real pixels after independent public inputs',{timeout:180000},async t=>{
 const images=new Map(),receipt={checkedAt:new Date().toISOString(),wasmSha256:hash(wasm),backend:'public-runtime-production-worker-wasm-original-Xtensa-SRAM',products:[],physicalValidation:false,factoryFirmwareValidation:false};
 const runtime=createPlatformRuntime({catalog,workerFactory,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'shared-host-test'}),sdStorage:createSdCardStorage({indexedDB:null,BroadcastChannel:null,dbName:'shared-host-test'}),fetchImpl:async value=>{const url=new URL(value);assert.equal(url.origin,origin);if(images.has(url.pathname))return new Response(images.get(url.pathname));if(url.pathname.endsWith('/esp32sim.wasm'))return new Response(wasm);if(url.pathname.endsWith('/esp32s3_rev0_rom.elf')&&process.env.ESP32SIM_ROM_DIR)return new Response(await readFile(resolve(process.env.ESP32SIM_ROM_DIR,basename(url.pathname))));return new Response(await readFile(new URL('web'+url.pathname,root)));}});
 try{
  for(const [id,board]of [['m5stack-k159','paperdiy'],['m5stack-c156','cores3']]){
   const product=catalog.products.find(p=>p.id===id);assert.equal(product.runtime.board,board);assert.equal(product.runtime.status,'partial');assert.deepEqual([product.runtime.flashMb,product.runtime.psramMb],[16,8]);assert.equal(product.firmwareTargetId,undefined,'runtime reuse cannot inherit firmware identity');
   const scene=createDefaultScene(catalog,id);scene.firmware=null;await runtime.configure(scene);const caps=await runtime.capabilities();assert.equal(caps.servos.available,false,'C156 has no StackChan base');assert.equal(caps.sdCard.defaultBytes,64<<20);assert.equal(caps.sdCard.defaultInserted,true);
   const image=m5SharedHostProbe(board),path='/diagnostic/'+id+'.bin';images.set(path,image);await runtime.loadFirmware({url:origin+path,sha256:hash(image),bootMode:'app-direct'});await runtime.start();await wait(runtime,id+' initial frame',s=>serial(s).includes('FRAME COMPLETE\n')&&s.frame);
   const pixels=[];
   async function check(color){const expected=[...color,255],x=board==='paperdiy'?539:0,y=board==='paperdiy'?400:0;let frame;const end=Date.now()+5000;do{frame=await runtime.capture({format:'rgba'});const at=(y*frame.width+x)*4;if(expected.every((v,i)=>frame.pixels[at+i]===v))break;await delay(15);}while(Date.now()<end);const at=(y*frame.width+x)*4;assert.deepEqual(Array.from(frame.pixels.slice(at,at+4)),expected,id+' actual guest pixel');assert.deepEqual([frame.width,frame.height],board==='paperdiy'?[540,960]:[320,240]);pixels.push({pixel:[x,y,...expected],frameSha256:hash(frame.pixels)});}
   await check(board==='paperdiy'?[0,0,0]:[255,0,0]);
   const cases=board==='paperdiy'?['button-a','button-b']:['touch'];
   for(const control of cases)for(const down of [true,false]){
    const before=serial(await runtime.observe()).length;
    await runtime.input(control==='touch'?{type:'touch',x:91,y:42,down}:{type:'button',id:control,pressed:down});
    await wait(runtime,id+' consumed '+control,s=>serial(s).slice(before).includes((control==='touch'?'TOUCH':'BUTTON')+(down?' DOWN\n':' UP\n'))&&serial(s).slice(before).includes('FRAME COMPLETE\n'));
    await check(board==='paperdiy'?(down?[255,255,255]:[0,0,0]):(down?[0,255,0]:[255,0,0]));
   }
   const state=await runtime.observe();assert.equal(state.firmware.appDirect,true);assert.deepEqual(state.firmware.stubs,[]);assert.deepEqual(state.servos,[]);await runtime.pause();
   // Real guests transfer the same bytes with missing power programming or an
   // incorrect physical clock route. Neither may manufacture display pixels.
   const negativeChecks=[];
   for(const [label,options]of [['unpowered',{power:false}],['wrong-route',{correctRoute:false}]]){
    const negative=m5SharedHostProbe(board,options),np='/diagnostic/'+id+'-'+label+'.bin';images.set(np,negative);await runtime.loadFirmware({url:origin+np,sha256:hash(negative),bootMode:'app-direct'});await runtime.start();const output=await wait(runtime,label+' negative',s=>serial(s).includes('FRAME COMPLETE\n'));
    if(output.frame){const dark=await runtime.capture({format:'rgba'}),background=board==='paperdiy'?255:0;assert(dark.pixels.every((v,i)=>v===(i%4===3?255:background)),label+' physical path cannot manufacture pixels');}else await assert.rejects(runtime.capture({format:'rgba'}),{code:'NO_FRAME'});await runtime.pause();negativeChecks.push(label);
   }
   const row={productId:id,board,flashMb:16,psramMb:8,probeSha256:hash(image),guestSerial:serial(state),pixels,negativeChecks,servoBaseAbsent:true};receipt.products.push(row);t.diagnostic(JSON.stringify(row));
  }
  receipt.limitations=['Diagnostic SRAM images are original test programs, not manufacturer factory firmware.','ILI9342C path only; later ILI9342E initialization is not validated.','PaperDIY scans every physical source/gate row in portrait coordinates, without grayscale or optical calibration.','No new GPU rendering, factory firmware, radio, audio or physical-device acceptance.'];
  if(process.env.M5_SHARED_RUNTIME_RECEIPT)await writeFile(process.env.M5_SHARED_RUNTIME_RECEIPT,JSON.stringify(receipt,null,2)+'\n');
 }finally{await runtime.dispose();}
});
