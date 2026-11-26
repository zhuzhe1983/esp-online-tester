// Public runtime -> real production Worker/WASM -> original LX6 SRAM program.
// The host sends product input events; actual guest SPI updates the glass.
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
import {m5ClassicPaperProbe} from './fixtures/m5-classic-paper-probe.mjs';
import {m5ClassicDisplayProbe} from './fixtures/m5-classic-display-probe.mjs';
const root=new URL('../',import.meta.url),origin='https://m5-classic-paper.invalid';
const hash=b=>createHash('sha256').update(b).digest('hex');
const catalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async k=>[k,JSON.parse(await readFile(new URL(`web/platform/catalog/${k}.json`,root))).items])));
const rom=await readFile(process.env.ESP32SIM_ROM_DIR?resolve(process.env.ESP32SIM_ROM_DIR,'esp32_rev300_rom.elf'):new URL('web/wasm/fw/esp32_rev300_rom.elf',root));
const wasm=await readFile(process.env.ESP32SIM_WASM||new URL('web/wasm/esp32sim.wasm',root)),delay=ms=>new Promise(r=>setTimeout(r,ms));
const workerFactory=url=>{const source=`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(d,t)=>parentPort.postMessage(d,t);await import(${JSON.stringify(new URL('web'+url.pathname,root).href)});parentPort.on('message',data=>globalThis.onmessage({data}));`;const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(source)));const adapter={postMessage:(m,...a)=>thread.postMessage(m.op==='create'&&process.env.M5_CLASSIC_INTERPRETER==='1'?{...m,jit:false}:m,...a),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const serial=s=>s.serialByPort?.uart0||'';
async function wait(runtime,label,predicate){const end=Date.now()+30000;let state;while(Date.now()<end){state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(label+': '+JSON.stringify(state.lastError));await delay(15);}throw Error(label+': '+JSON.stringify({status:state?.status,serial:serial(state||{}),stats:state?.stats}));}
test('Exact classic M5 products reach LX6 GPIO and real panel framebuffer',{timeout:240000},async t=>{
 const images=new Map([['/diagnostic/esp32_rev300_rom.elf',rom]]),receipt={schemaVersion:1,checkedAt:new Date().toISOString(),nodeVersion:process.version,jitEnabled:process.env.M5_CLASSIC_INTERPRETER!=='1',wasmSha256:hash(wasm),romSha256:hash(rom),probeSourceSha256:hash(await readFile(new URL('fixtures/m5-classic-paper-probe.mjs',import.meta.url))),displayProbeSourceSha256:hash(await readFile(new URL('fixtures/m5-classic-display-probe.mjs',import.meta.url))),harnessSha256:hash(await readFile(new URL(import.meta.url))),backend:'public-runtime-production-worker-wasm-mask-ROM-original-LX6-SRAM',products:[],physicalValidation:false,factoryFirmwareValidation:false};
 const runtime=createPlatformRuntime({catalog,workerFactory,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'classic-paper-worker-test'}),sdStorage:createSdCardStorage({indexedDB:null,BroadcastChannel:null,dbName:'classic-paper-worker-test'}),fetchImpl:async value=>{const url=new URL(value);assert.equal(url.origin,origin);if(images.has(url.pathname))return new Response(images.get(url.pathname));if(url.pathname.endsWith('/esp32sim.wasm'))return new Response(wasm);if(url.pathname.endsWith('_rom.elf')&&process.env.ESP32SIM_ROM_DIR)return new Response(await readFile(resolve(process.env.ESP32SIM_ROM_DIR,basename(url.pathname))));return new Response(await readFile(new URL('web'+url.pathname,root)));}});
 try{
  for(const[id,board,control,pin,width,height,changed,thermal=false,touch=false]of[['m5stack-k016-p-se','stickc-plus-se','button-a',37,135,240,1],['m5stack-k016-p2','stickc-plus2','button-a',37,135,240,1],['m5stack-k016-b','m5stick-oled','button',35,64,128,8],['m5stack-k049','m5paper-original','rotary-press',38,540,960,4],['m5stack-k049-c','m5paper-comm','rotary-press',38,540,960,4],['m5stack-k016-t','stickt-original','button-a',37,135,240,1],['m5stack-k126','tlite','button-a',37,135,240,1],['m5stack-k014-b','bala-fire-original','button-a',39,320,240,1],['m5stack-k016-t','stickt-original','thermal',null,135,240,1,true],['m5stack-k126','tlite','thermal',null,135,240,1,true],['m5stack-k049-b','m5paper-v11','touch',null,540,960,4,false,true],['m5stack-k049','m5paper-original','touch',null,540,960,4,false,true],['m5stack-k049-c','m5paper-comm','touch',null,540,960,4,false,true]]){
   if(process.env.M5_CLASSIC_COMPLETION_WORKER_MODE==='touch'&&!touch)continue;
   if(process.env.M5_CLASSIC_COMPLETION_WORKER_IDS&&!process.env.M5_CLASSIC_COMPLETION_WORKER_IDS.split(',').includes(id))continue;
   const p=catalog.products.find(p=>p.id===id);assert.equal(p.runtime.board,board);assert.equal(p.runtime.status,'partial');if(!thermal&&!touch)assert.equal(p.controls.find(c=>c.id===control).pin,pin);
   const scene=createDefaultScene(catalog,id);scene.firmware=null;await runtime.configure(scene);const caps=await runtime.capabilities();if(thermal)assert.equal(caps.thermal.calibrated,false);else if(touch)assert.equal(caps.input.touch,true);else assert(caps.input.buttons.includes(control));
   const paper=board.startsWith('m5paper-'),oled=board==='m5stick-oled';const image=paper?m5ClassicPaperProbe(board,{touch}):m5ClassicDisplayProbe(board,{key:pin??37,thermal}),path='/diagnostic/'+id+'.bin';images.set(path,image);
   // Classic ESP32 deliberately requires mask-ROM boot. The original diagnostic
   // is the SRAM bootloader at flash0x1000; a declared original app and partition
   // table satisfy the normal public bundle contract. No ROM stubs or SDK used.
   const table=Buffer.alloc(0xc00,255);table.writeUInt16LE(0x50aa);table[2]=0;table[3]=0;table.writeUInt32LE(0x10000,4);table.writeUInt32LE(0x10000,8);table.fill(0,12,32);table.write('probe',12);
   const tablePath='/diagnostic/partitions.bin';images.set(tablePath,table);
   const manifest={board,flash_mb:p.runtime.flashMb,psram_mb:p.runtime.psramMb,app_direct:false,stubs:[],files:{rom:{url:origin+'/diagnostic/esp32_rev300_rom.elf',sha256:hash(rom)},bootloader:{url:origin+path,sha256:hash(image)},ptable:{url:origin+tablePath,sha256:hash(table)},app:{url:origin+path,sha256:hash(image)}}};
   const manifestPath='/diagnostic/'+id+'.json',manifestBytes=new TextEncoder().encode(JSON.stringify(manifest));images.set(manifestPath,manifestBytes);await runtime.loadFirmware({manifestUrl:origin+manifestPath,sha256:hash(manifestBytes)});await runtime.start();await wait(runtime,id+' initial released frame',s=>serial(s).includes((thermal?'SENSOR BASELINE':touch?'TOUCH UP':'BUTTON UP')+'\nFRAME COMPLETE\n')&&s.frame);
   const pixels=[];
   async function check(pressed,minFrame=1){let f;const end=Date.now()+30000;
    // Paper publishes at 8Hz of guest time. A busy shared host can run below
    // 0.01x; wait for the real output predicate, not a five-second wall delay.
    do{const state=await runtime.observe();if(state.frame?.frameNumber>=minFrame&&state.frame.frameNumber!==f?.frameNumber)f=await runtime.capture({format:'rgba'});
     if(f&&f.pixels[0]===(pressed?0:255)&&f.pixels[1]===(paper||oled?(pressed?0:255):pressed?255:0))break;await delay(20);
    }while(Date.now()<end);
    assert(f,id+' no fresh guest framebuffer: '+JSON.stringify((await runtime.observe()).stats));assert.deepEqual([f.width,f.height],[width,height]);
    // Expected coordinates derive from the controller window and physical glass,
    // independent of the runtime input encoder and framebuffer conversion path.
    const expected=paper?(pressed?[0,0,0,255]:[255,255,255,255]):oled?(pressed?[0,0,0,255]:[255,255,255,255]):pressed?[0,255,0,255]:[255,0,0,255];
    assert.deepEqual(Array.from(f.pixels.slice(0,4)),expected,id+' actual guest-drawn pixel');
    if(paper){for(let y=0;y<height;y++)for(let x=0;x<width;x++){const at=(y*width+x)*4,value=pressed&&x===0&&y<4?0:255;assert.deepEqual(Array.from(f.pixels.slice(at,at+4)),[value,value,value,255]);}}
    const row={pressed,frameNumber:f.frameNumber,pixel:[0,0,...Array.from(f.pixels.slice(0,4))],frameSha256:hash(f.pixels)};pixels.push(row);return row;
   }
   await check(false);await assert.rejects(runtime.input({type:'button',id:'not-a-product-control',pressed:true}),{code:'UNSUPPORTED_CONTROL'});
   for(const pressed of[true,false]){const beforeState=await runtime.observe(),before=serial(beforeState).length,minFrame=beforeState.frame.frameNumber+1;await runtime.input(thermal?{type:'thermal-environment',background:pressed?7000:8000,hotspot:9000,centerX:80,centerY:60,radius:12}:touch?{type:'touch',x:120,y:240,down:pressed}:{type:'button',id:control,pressed});await wait(runtime,id+' guest consumed '+pressed,s=>serial(s).slice(before).includes((thermal?'SENSOR '+(pressed?'CHANGED':'BASELINE'):(touch?'TOUCH ':'BUTTON ')+(pressed?'DOWN':'UP'))+'\nFRAME COMPLETE\n'));await check(pressed,minFrame);}
   const state=await runtime.observe();if(thermal){const reports=serial(state).match(/SENSOR (BASELINE|CHANGED)\n/g);assert.deepEqual(reports,['SENSOR BASELINE\n','SENSOR CHANGED\n','SENSOR BASELINE\n'],id+' only host scene changes produce the selected sensor-pixel transition');}assert.equal(state.firmware.appDirect,false);assert.deepEqual(state.firmware.stubs,[]);await runtime.pause();
   const row={productId:id,board,control,pin,thermal,touch,flashMb:p.runtime.flashMb,psramMb:p.runtime.psramMb,probeSha256:hash(image),guestSerial:serial(state),pixels,unknownControlRejected:true};receipt.products.push(row);t.diagnostic(JSON.stringify(row));
  }
  receipt.limitations=['Original SRAM diagnostics loaded by the real mask ROM as a diagnostic bootloader; no manufacturer firmware or full SDK boot validation.','OLED physical scan direction and alignment are not hardware calibrated.','Public runtime/production Worker/WASM path under Node, without browser GPU rendering or physical hardware.','RTC, radio, SD, audio, analog LUT/ghosting and physical optical timing are outside this integration probe.'];
  assert(receipt.products.length>0,'Product filter must select at least one diagnostic');
  if(process.env.M5_CLASSIC_COMPLETION_WORKER_RECEIPT)await writeFile(process.env.M5_CLASSIC_COMPLETION_WORKER_RECEIPT,JSON.stringify(receipt,null,2)+'\n');
 }finally{await runtime.dispose();}
});
