// Public runtime -> production Worker/WASM -> original LX7 SRAM guest -> pixels.
// No Rust implementation, UI animation, UART injection or factory image is used.
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
import {m5VameterCardInputProbe} from './fixtures/m5-vameter-card-input-probe.mjs';
const root=new URL('../',import.meta.url),origin='https://m5-input-probe.invalid';
const hash=b=>createHash('sha256').update(b).digest('hex'),delay=ms=>new Promise(r=>setTimeout(r,ms));
const catalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async k=>[k,JSON.parse(await readFile(new URL(`web/platform/catalog/${k}.json`,root))).items])));
const wasm=await readFile(process.env.ESP32SIM_WASM||new URL('web/wasm/esp32sim.wasm',root));
const romPath=process.env.ESP32SIM_ROM_DIR?resolve(process.env.ESP32SIM_ROM_DIR,'esp32s3_rev0_rom.elf'):new URL('web/wasm/fw/esp32s3_rev0_rom.elf',root),rom=await readFile(romPath);
const workerFactory=url=>{const source=`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(d,t)=>parentPort.postMessage(d,t);await import(${JSON.stringify(new URL('web'+url.pathname,root).href)});parentPort.on('message',data=>globalThis.onmessage({data}));`;const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(source)));const adapter={postMessage:(...a)=>thread.postMessage(...a),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const serial=s=>s.serialByPort?.uart0||'';
async function wait(runtime,label,predicate){const end=Date.now()+30000;let state;while(Date.now()<end){state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(label+': '+JSON.stringify(state.lastError));await delay(10);}throw Error(label+': '+JSON.stringify({status:state?.status,serial:serial(state||{}),stats:state?.stats}));}
test('VAMeter and Xiaozhi Card public inputs change actual guest-rendered pixels',{timeout:180000},async t=>{
 const images=new Map(),receipt={checkedAt:new Date().toISOString(),backend:'public-runtime-production-worker-wasm-original-Xtensa-SRAM',wasm:{bytes:wasm.length,sha256:hash(wasm)},rom:{name:basename(String(romPath)),bytes:rom.length,sha256:hash(rom)},sources:{},products:[],physicalValidation:false,factoryFirmwareValidation:false};
 for(const file of ['web/platform/runtime.js','web/wasm/worker.js','tools/fixtures/m5-vameter-card-input-probe.mjs','tools/platform-vameter-card-input-runtime-test.mjs']){const b=await readFile(new URL(file,root));receipt.sources[file]={bytes:b.length,sha256:hash(b)};}
 const runtime=createPlatformRuntime({catalog,workerFactory,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'vameter-card-input-test'}),sdStorage:createSdCardStorage({indexedDB:null,BroadcastChannel:null,dbName:'vameter-card-input-test'}),fetchImpl:async value=>{const url=new URL(value);assert.equal(url.origin,origin);if(images.has(url.pathname))return new Response(images.get(url.pathname));if(url.pathname.endsWith('/esp32sim.wasm'))return new Response(wasm);if(url.pathname.endsWith('/esp32s3_rev0_rom.elf'))return new Response(rom);return new Response(await readFile(new URL('web'+url.pathname,root)));}});
 try{
  for(const [id,board]of [['m5stack-k136','vameter'],['m5stack-k146','xiaozhi-card']])await t.test(id,async()=>{
   const product=catalog.products.find(p=>p.id===id);assert.equal(product.runtime.board,board);assert.equal(product.runtime.status,'partial');
   const scene=createDefaultScene(catalog,id);scene.firmware=null;await runtime.configure(scene);const caps=await runtime.capabilities();
   if(board==='vameter'){assert(caps.input.buttons.includes('encoder-press'));assert(caps.input.encoders.some(c=>c.id==='encoder'));}else assert.equal(caps.input.touch,true);
   const image=m5VameterCardInputProbe(board),path='/diagnostic/'+id+'.bin';images.set(path,image);await runtime.loadFirmware({url:origin+path,sha256:hash(image),bootMode:'app-direct'});await runtime.start();await wait(runtime,id+' initial frame',s=>serial(s).includes('FRAME COMPLETE\n')&&s.frame);
   const row={productId:id,board,probeSha256:hash(image),probeBytes:image.length,stages:[]};
   async function check(stage,expected,inputReceipt){
    let frame,actual;const end=Date.now()+15000;
    do{frame=await runtime.capture({format:'rgba'});assert.deepEqual([frame.width,frame.height],board==='vameter'?[240,240]:[176,264]);actual=expected.map(({x,y})=>Array.from(frame.pixels.slice((y*frame.width+x)*4,(y*frame.width+x)*4+4)));if(expected.every((p,j)=>p.rgba.every((v,i)=>actual[j][i]===v)))break;await delay(15);}while(Date.now()<end);
    assert.deepEqual(actual,expected.map(p=>p.rgba),id+' '+stage+' guest framebuffer');row.stages.push({stage,...(inputReceipt?{inputReceipt}:{}),pixels:expected,frameSha256:hash(frame.pixels)});
   }
   async function input(event,marker,stage,pixels){
    const before=serial(await runtime.observe()).length,ack=await runtime.input(event);await wait(runtime,id+' consumed '+stage,s=>serial(s).slice(before).includes(marker)&&serial(s).slice(before).includes('FRAME COMPLETE\n'));await check(stage,pixels,ack);
   }
   if(board==='vameter'){
    const p=(press,encoder)=>[{x:0,y:0,rgba:[...press,255]},{x:1,y:0,rgba:[...encoder,255]}];
    await check('initial released and no encoder edges',p([255,0,0],[0,0,255]));
    await input({type:'button',id:'encoder-press',pressed:true},'PRESS DOWN\n','press',p([0,255,0],[0,0,255]));
    await input({type:'button',id:'encoder-press',pressed:false},'PRESS UP\n','release',p([255,0,0],[0,0,255]));
    await input({type:'encoder',id:'encoder',steps:1},'ENCODER EDGE\n','encoder step',p([255,0,0],[255,255,0]));
    const state=await wait(runtime,id+' full quadrature cycle',s=>(serial(s).match(/ENCODER EDGE\n/g)||[]).length>=4);row.guestEncoderEdges=(serial(state).match(/ENCODER EDGE\n/g)||[]).length;
   }else{
    const p=black=>[{x:175,y:263,rgba:[...(black?[0,0,0]:[255,255,255]),255]}];
    await check('initial no contact',p(false));
    await input({type:'touch',x:91,y:42,down:true},'TOUCH DOWN\n','touch down',p(true));
    await input({type:'touch',x:91,y:42,down:false},'TOUCH UP\n','touch release',p(false));
   }
   assert.equal(row.stages[0].frameSha256,row.stages[2].frameSha256,'release restores the entire initial guest framebuffer');assert.notEqual(row.stages[0].frameSha256,row.stages[1].frameSha256,'down changes the framebuffer');
   const state=await runtime.observe();assert.equal(state.firmware.appDirect,true);assert.deepEqual(state.firmware.stubs,[]);row.guestSerial=serial(state);row.appDirect=true;row.stubs=[];receipt.products.push(row);t.diagnostic(JSON.stringify(row));await runtime.pause();
  });
  assert.equal(receipt.products.length,2,'both products must reach verified guest pixels before writing a success receipt');
  receipt.limitations=['Original authored SRAM diagnostics only; no manufacturer factory image was loaded or validated.','Actual public runtime input, production Worker and WASM guest MMIO are exercised; browser pointer hit testing is outside this check.','No physical hardware, orientation/optical calibration, INA226 measurements, power, audio, storage, radio, modem or dock acceptance.','Negative physical-route/reset cases remain covered by the native vameter_xiaozhi_card tests; this check closes the JavaScript transport path.'];
  if(process.env.M5_TWO_INPUT_RUNTIME_RECEIPT)await writeFile(process.env.M5_TWO_INPUT_RUNTIME_RECEIPT,JSON.stringify(receipt,null,2)+'\n');
 }finally{await runtime.dispose();}
});
