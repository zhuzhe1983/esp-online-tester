// Public product runtime -> production Worker/WASM -> original ROM-loaded LX6.
// UART commands on U120 are this diagnostic's test controls, not its Grove I2C protocol.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve,basename,dirname} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {m5UnitControllerProbe} from './fixtures/m5-unit-controller-probe.mjs';
const root=new URL('../',import.meta.url),origin='https://m5-unit-probe.invalid',hash=b=>createHash('sha256').update(b).digest('hex');
const catalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async k=>[k,JSON.parse(await readFile(new URL(`web/platform/catalog/${k}.json`,root))).items])));
const rom=await readFile(process.env.ESP32SIM_ROM_DIR?resolve(process.env.ESP32SIM_ROM_DIR,'esp32_rev300_rom.elf'):new URL('web/wasm/fw/esp32_rev300_rom.elf',root));
const wasm=await readFile(process.env.ESP32SIM_WASM||new URL('web/wasm/esp32sim.wasm',root));
const workerFactory=url=>{const source=`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(d,t)=>parentPort.postMessage(d,t);await import(${JSON.stringify(new URL('web'+url.pathname,root).href)});parentPort.on('message',data=>globalThis.onmessage({data}));`;const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(source)));const adapter={postMessage:(...a)=>thread.postMessage(...a),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const serial=s=>s.serialByPort?.uart0||'',delay=ms=>new Promise(r=>setTimeout(r,ms));
async function wait(runtime,label,predicate){const end=Date.now()+45000;let state;while(Date.now()<end){state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(label+': '+JSON.stringify(state.lastError));await delay(20);}throw Error(label+': '+JSON.stringify({status:state?.status,serial:serial(state||{}),stats:state?.stats,frame:state?.frame,leds:state?.leds}));}
const profiles=[['m5stack-k117','atomu'],['m5stack-u120','unit-lcd'],['m5stack-u149','unit-thermal2'],['m5stack-k115','atom-psram-display']];
test('four exact Unit/Atom public products consume inputs in real LX6 firmware and publish real hardware output',{timeout:360000},async t=>{
 const images=new Map([['/diagnostic/esp32_rev300_rom.elf',rom]]),receipt={schemaVersion:1,checkedAt:new Date().toISOString(),backend:'public-runtime-production-worker-wasm-mask-ROM-original-LX6-SRAM',wasmSha256:hash(wasm),romSha256:hash(rom),probeSourceSha256:hash(await readFile(new URL('fixtures/m5-unit-controller-probe.mjs',import.meta.url))),harnessSha256:hash(await readFile(new URL(import.meta.url))),products:[],physicalValidation:false,factoryFirmwareValidation:false};
 const receiptPath=process.env.M5_UNIT_WORKER_RECEIPT;
 const runtime=createPlatformRuntime({catalog,workerFactory,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'m5-unit-worker-test'}),fetchImpl:async value=>{const url=new URL(value);assert.equal(url.origin,origin);if(images.has(url.pathname))return new Response(images.get(url.pathname));if(url.pathname.endsWith('/esp32sim.wasm'))return new Response(wasm);if(url.pathname.endsWith('_rom.elf')&&process.env.ESP32SIM_ROM_DIR)return new Response(await readFile(resolve(process.env.ESP32SIM_ROM_DIR,basename(url.pathname))));return new Response(await readFile(new URL('web'+url.pathname,root)));}});
 async function start(id,board,wrongRoute){const p=catalog.products.find(p=>p.id===id),scene=createDefaultScene(catalog,id);scene.firmware=null;await runtime.configure(scene);const image=m5UnitControllerProbe(board,{wrongRoute}),path='/diagnostic/'+id+(wrongRoute?'-wrong':'')+'.bin';images.set(path,image);
  const table=Buffer.alloc(0xc00,255);table.writeUInt16LE(0x50aa);table[2]=0;table[3]=0;table.writeUInt32LE(0x10000,4);table.writeUInt32LE(0x10000,8);table.fill(0,12,32);table.write('unit-probe',12);const tablePath='/diagnostic/partitions.bin';images.set(tablePath,table);
  const manifest={board,flash_mb:p.runtime.flashMb,psram_mb:p.runtime.psramMb,app_direct:false,stubs:[],files:{rom:{url:origin+'/diagnostic/esp32_rev300_rom.elf',sha256:hash(rom)},bootloader:{url:origin+path,sha256:hash(image)},ptable:{url:origin+tablePath,sha256:hash(table)},app:{url:origin+path,sha256:hash(image)}}};
  const manifestPath='/diagnostic/'+id+'.json',bytes=new TextEncoder().encode(JSON.stringify(manifest));images.set(manifestPath,bytes);await runtime.loadFirmware({manifestUrl:origin+manifestPath,sha256:hash(bytes)});await runtime.start();return{productId:id,board,wrongRoute,probeSha256:hash(image),checks:[]};
 }
 async function pixel(rgb,w,h){let f;const end=Date.now()+10000;do{try{f=await runtime.capture({format:'rgba'});if(f.width===w&&f.height===h&&rgb.every((v,i)=>f.pixels[i]===v))break;}catch(error){if(error.code!=='NO_FRAME')throw error;}await delay(25);}while(Date.now()<end);assert(f,'No guest display published');assert.deepEqual([f.width,f.height],[w,h]);assert.deepEqual(Array.from(f.pixels.slice(0,4)),[...rgb,255]);assert.equal(Array.from(f.pixels).filter((v,i)=>i%4!==3&&i>=4&&v!==0).length,0,'Guest wrote only pixel0; no host fill');return hash(f.pixels);}
 try{
  for(const[id,board]of profiles){const row=await start(id,board,false),checks=row.checks;
   if(board==='unit-thermal2'){
    await wait(runtime,id+' sensor default',s=>serial(s).includes('THERMAL RAW1000\n'));checks.push('guest-I2C21/22-MLX90640-0x0400-read1000');
    let before=serial(await runtime.observe()).length;await runtime.input({type:'thermal-environment',background:1234,hotspot:4321,centerX:0,centerY:0,radius:0});await wait(runtime,id+' hot ADC',s=>serial(s).slice(before).includes('THERMAL RAW4321\n'));checks.push('public-environment-next-timed-sensor-frame-guest-read4321');
    before=serial(await runtime.observe()).length;await runtime.input({type:'thermal-environment',background:1234,hotspot:4321,centerX:31,centerY:23,radius:0});await wait(runtime,id+' background ADC',s=>serial(s).slice(before).includes('THERMAL RAW1234\n'));checks.push('moving-scene-changes-real-guest-sensor-word-not-temperature-or-framebuffer');assert.equal((await runtime.observe()).frame,null);
    await assert.rejects(runtime.input({type:'thermal-environment',background:0,hotspot:32768,centerX:0,centerY:0,radius:0}),{code:'INVALID_INPUT'});
   }else if(board==='unit-lcd'){
    await wait(runtime,id+' initial red',s=>serial(s).includes('UNIT LCD RED\n')&&s.frame);row.redFrameSha256=await pixel([255,0,0],135,240);
    await runtime.input({type:'serial',port:'uart0',data:'g'});await wait(runtime,id+' diagnostic UART green',s=>serial(s).includes('UNIT LCD GREEN\n'));row.greenFrameSha256=await pixel([0,255,0],135,240);checks.push('guest-ST7789-SPI13/15-CS5-DC23-BL4-inverted','diagnostic-UART-command-consumed-by-guest-real-green-pixel');
    await runtime.input({type:'serial',port:'uart0',data:'r'});await wait(runtime,id+' UART red again',s=>serial(s).split('UNIT LCD RED\n').length>=3);await pixel([255,0,0],135,240);
   }else{
    await wait(runtime,id+' initial released output',s=>serial(s).includes('BUTTON UP RED\n')&&(board==='atomu'?JSON.stringify(s.leds?.[0])==='[255,0,0]':s.frame));
    if(board==='atom-psram-display'){assert(serial(await runtime.observe()).includes('PSRAM2MB OK\n'));row.redFrameSha256=await pixel([255,0,0],4,3);checks.push('guest-PSRAM-cache-MMU-first-and-last-physical-page');}else assert.equal((await runtime.observe()).frame,null);
    for(const pressed of[true,false]){const before=serial(await runtime.observe()).length;await runtime.input({type:'button',id:'button',pressed});await wait(runtime,id+' guest key '+pressed,s=>serial(s).slice(before).includes(pressed?'BUTTON DOWN GREEN\n':'BUTTON UP RED\n')&&(board!=='atomu'||JSON.stringify(s.leds?.[0])===(pressed?'[0,255,0]':'[255,0,0]')));if(board==='atom-psram-display')row[pressed?'greenFrameSha256':'releasedFrameSha256']=await pixel(pressed?[0,255,0]:[255,0,0],4,3);}
    checks.push('public-G39-press-release-consumed-by-guest',board==='atomu'?'guest-RMT27-SK6812-red-green-red':'guest-SPI-FPGA-external-monitor-red-green-red');
   }
   let state=await runtime.observe();assert.equal(state.firmware.appDirect,false);assert.deepEqual(state.firmware.stubs,[]);row.guestSeconds=state.stats.time;row.guestSerial=serial(state);
   // Reset while the visible/held/environment state differs from cold boot.
   // A new ROM boot must clear old UART/pixels and release the product key.
   if(board==='unit-lcd'){await runtime.input({type:'serial',port:'uart0',data:'g'});await pixel([0,255,0],135,240);}
   else if(board!=='unit-thermal2'){await runtime.input({type:'button',id:'button',pressed:true});await wait(runtime,id+' held before reset',s=>serial(s).split('BUTTON DOWN GREEN\n').length>=3&&(board!=='atomu'||JSON.stringify(s.leds?.[0])==='[0,255,0]'));if(board==='atomu')assert.deepEqual((await runtime.observe()).leds,[[0,255,0]]);else await pixel([0,255,0],4,3);}
   await runtime.reset();
   if(board==='unit-thermal2'){await wait(runtime,id+' reset default ADC',s=>serial(s).includes('THERMAL RAW1000\n'));state=await runtime.observe();assert(!serial(state).includes('THERMAL RAW4321\n'));assert(!serial(state).includes('THERMAL RAW1234\n'));}
   else if(board==='unit-lcd'){await wait(runtime,id+' reset red',s=>serial(s).includes('UNIT LCD RED\n'));await pixel([255,0,0],135,240);}
   else{await wait(runtime,id+' reset released',s=>serial(s).includes('BUTTON UP RED\n')&&(board!=='atomu'||JSON.stringify(s.leds?.[0])==='[255,0,0]'));if(board==='atom-psram-display'){await pixel([255,0,0],4,3);assert(serial(await runtime.observe()).includes('PSRAM2MB OK\n'));}assert(!serial(await runtime.observe()).includes('BUTTON DOWN GREEN\n'));}
   row.resetSerial=serial(await runtime.observe());assert.equal(row.resetSerial.split('ets Jul 29 2019').length,2,'fresh serial contains one actual ROM boot');checks.push('public-reset-reboots-mask-ROM-clears-old-output-and-restores-cold-inputs');await runtime.pause();receipt.products.push(row);t.diagnostic(JSON.stringify(row));
   // Each physical route has a negative original image: host stimuli still reach
   // the same product, but the guest's disconnected peripheral cannot fabricate output.
   const negative=await start(id,board,true);
   await wait(runtime,id+' wrong-route execution',s=>serial(s).includes(board==='unit-thermal2'?'THERMAL RAW OTHER\n':board==='unit-lcd'?'UNIT LCD RED\n':'BUTTON UP RED\n'));
   state=await runtime.observe();if(board==='atomu')assert.deepEqual(state.leds,[],'Disconnected RMT has no first frame; do not fabricate a black ring');else if(board==='unit-thermal2')assert(!serial(state).includes('THERMAL RAW1000\n'));else if(state.frame){const f=await runtime.capture({format:'rgba'});assert(Array.from(f.pixels).every((v,i)=>i%4===3?v===255:v===0));}
   negative.checks.push('wrong-physical-peripheral-route-does-not-produce-valid-output');negative.guestSerial=serial(state);receipt.products.push(negative);await runtime.pause();
  }
  receipt.limitations=['Original SRAM diagnostics run as ROM-loaded diagnostic bootloaders, not official factory applications or full SDK boot.','U120 UART commands are private diagnostic controls; external Grove I2C slave is not validated.','U149 synthetic raw ADC uses public virtual calibration and guest-cycle frames; no physical temperature calibration.','K115 capture is external FPGA digital memory, not LCD glass or validated physical HDMI.','Node production Worker/WASM only; browser DOM/WebGL and physical devices remain separately assessed.'];
  receipt.status='passed';
 }catch(error){receipt.status='failed';receipt.error={name:error.name,message:error.message};throw error;}finally{await runtime.dispose();if(receiptPath){await mkdir(dirname(receiptPath),{recursive:true});await writeFile(receiptPath,JSON.stringify(receipt,null,2)+'\n');}}
});
