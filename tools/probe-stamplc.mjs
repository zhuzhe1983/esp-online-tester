#!/usr/bin/env node
// Internal independently built IDF firmware on the real page worker/WASM path.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {resolve,join,basename} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {inspectLocalBuild} from '../web/platform/local-firmware.js';
import {createDefaultScene} from '../web/platform/catalog.js';
const at=process.argv.indexOf('--build');if(at<0||!process.argv[at+1])throw Error('Supply --build <ESP-IDF 5.5.1 stamplc-firmware build directory>.');
const folder=resolve(process.argv[at+1]),root=new URL('../',import.meta.url),origin='https://stamplc-fixture.invalid',hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const files=[new File([await readFile(join(folder,'flasher_args.json'))],'flasher_args.json')],inputHashes={};
for(const path of (await inspectLocalBuild(files)).missingFiles){const bytes=await readFile(join(folder,path)),file=new File([bytes],basename(path));inputHashes[path]=hash(bytes);Object.defineProperty(file,'webkitRelativePath',{value:path});files.push(file);}
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const fetchImpl=async value=>{const url=new URL(value);assert.equal(url.origin,origin);try{return new Response(await readFile(new URL('web'+url.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root),thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(m,...args)=>thread.postMessage(m,...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'stamplc-probe'})});
const receipt={checkedAt:new Date().toISOString(),command:'node tools/probe-stamplc.mjs --build <fixture-build-directory>',backend:'actual-wasm-worker',productId:'m5stack-k141',firmware:'internal ESP-IDF 5.5.1 protocol fixture',wasmSha256:hash(await readFile(new URL('web/wasm/esp32sim.wasm',root))),inputHashes,checks:[],frames:[],inputs:[],limitations:['This is independent diagnostic firmware, not factory compatibility proof.','Relay digital GPIO drive/readback is modeled; physical contacts/load, optocoupler voltage thresholds and external input stimuli are not.','INA226/LM75/RX8130/SD/CAN/RS485/buzzer/physical power/network remain unsupported.','LCD analog initialization delays, optical response and RGB brightness are not physically calibrated.']};
const delay=ms=>new Promise(r=>setTimeout(r,ms)),serial=s=>s.serialByPort.uart0||s.serial,same=(a,b)=>JSON.stringify(a)===JSON.stringify(b);
const wait=async(label,predicate)=>{const end=Date.now()+180000;while(Date.now()<end){const s=await runtime.observe();if(predicate(s))return s;if(s.status==='error')throw Error(JSON.stringify(s.lastError));await delay(20);}const s=await runtime.observe();throw Error(`${label} timed out ${JSON.stringify({leds:s.leds,frame:s.frame})}\n${serial(s).slice(-4000)}`);};
const expected=mask=>{const p=new Uint8Array(240*135*4);for(let y=0;y<135;y++)for(let x=0;x<240;x++){const rgb=mask<0?[0,0,0]:mask?[(mask&4)?255:0,(mask&2)?255:0,(mask&1)?255:0]:y<67?(x<120?[255,0,0]:[0,255,0]):(x<120?[0,0,255]:[255,255,255]);p.set([...rgb,255],(y*240+x)*4);}return hash(p);};
const capture=async(label,mask)=>{const wanted=expected(mask),end=Date.now()+30000;while(Date.now()<end){const f=await runtime.capture({format:'rgba'});if(f.width===240&&f.height===135&&hash(f.pixels)===wanted){receipt.frames.push({label,width:f.width,height:f.height,rgbaSha256:wanted});return;}await delay(20);}assert.fail('Frame mismatch '+label);};
try {
 const scene=createDefaultScene(catalog,'m5stack-k141');scene.firmware=null;await runtime.configure(scene);await runtime.loadFirmware({format:'build',files});await runtime.start();
 let s=await wait('written pixels with backlight off',s=>serial(s).includes('STAMPLC_STAGE BACKLIGHT_OFF')&&s.frame?.width===240);await capture('backlight-off',-1);
 s=await wait('four corners',s=>serial(s).includes('STAMPLC_STAGE FOUR_CORNERS'));await capture('four-guest-SPI-quadrants',0);
 s=await wait('Stamp module gate low',s=>serial(s).includes('STAMPLC_STAGE STAMP_GATE_LOW'));assert.deepEqual(s.leds,[[0,0,0],[0,0,0]]);
 s=await wait('Stamp module gate high without fresh frame',s=>serial(s).includes('STAMPLC_STAGE STAMP_GATE_HIGH_NO_FRAME'));assert.deepEqual(s.leds,[[0,0,0],[0,0,0]]);
 s=await wait('ready',s=>serial(s).includes('STAMPLC_BUTTONS 0 IRQ 1 00 1 RELAYS 00')&&same(s.leds,[[0,0,0],[32,0,16]]));
 for(const line of ['STAMPLC_RESET_HELD NACK NACK','STAMPLC_IDS a2 23','STAMPLC_WRONG_ADDRESS NACK','STAMPLC_WRONG_PINS NACK','STAMPLC_WRONG_CS ffffff','STAMPLC_AW_DEFAULT 0f 0f INPUT 00','STAMPLC_RELAY4 08'])assert(serial(s).includes(line),line);
 receipt.checks.push('actual-ROM-and-IDF-boot','shared-G3-reset-holds-both-I2C-expanders','I2C13-15-identities-wrong-address-wrong-pins-and-SPI-CS10-rejection','all-32400-SPI-pixels-fixed-glass-rotation-and-expander-backlight','AW9523-address59-default-open-drain-and-fourth-output-readback','independent-G21-RMT-and-G38-Stamp-LED-supply');
 for(const [id,mask,relay] of [['button-a',4,'01'],['button-b',2,'02'],['button-c',1,'04']]){
  let start=serial(s).length;await runtime.input({type:'button',id,pressed:true});s=await wait('fresh '+id,s=>serial(s).slice(start).includes(`STAMPLC_BUTTONS ${mask} IRQ 0 0${mask} 1 RELAYS ${relay}`)&&same(s.leds,[[(mask&4)?255:0,(mask&2)?255:0,(mask&1)?255:0],[32,0,16]]));const record={id,pressed:serial(s).slice(start).split(/\r?\n/).filter(l=>/^STAMPLC_BUTTONS/.test(l)),leds:s.leds};receipt.inputs.push(record);await capture(id,mask);
  start=serial(s).length;await runtime.input({type:'button',id,pressed:false});s=await wait('fresh release '+id,s=>serial(s).slice(start).includes('STAMPLC_BUTTONS 0 IRQ 1 00 1 RELAYS 00'));record.released=serial(s).slice(start).split(/\r?\n/).filter(l=>/^STAMPLC_BUTTONS/.test(l));await capture(id+'-released',0);
 }
 receipt.checks.push('three-fresh-PI4IO-key-press-release-reads-and-latched-IRQ14-clear-drive-LCD-RGB-relay-readback');
 await assert.rejects(runtime.input({type:'button',id:'power',pressed:true}));await assert.rejects(runtime.input({type:'touch',x:1,y:1,down:true}));receipt.checks.push('no-invented-power-key-or-touch');
 await runtime.pause();const paused=await runtime.observe();await assert.rejects(runtime.input({type:'button',id:'button-a',pressed:true}));await delay(100);const still=await runtime.observe();assert.equal(serial(still),serial(paused));assert.equal(still.stats.time,paused.stats.time);assert.deepEqual(still.leds,paused.leds);await runtime.start();receipt.checks.push('pause-stops-guest-time-and-input');
 let start=serial(await runtime.observe()).length;await runtime.input({type:'button',id:'button-a',pressed:true});await wait('held key before reset',s=>serial(s).slice(start).includes('STAMPLC_BUTTONS 4'));
 await runtime.reset();s=await wait('reset released keys',s=>serial(s).includes('STAMPLC_READY')&&serial(s).includes('STAMPLC_BUTTONS 0 IRQ 1 00 1 RELAYS 00')&&same(s.leds,[[0,0,0],[32,0,16]]));assert(!serial(s).includes('STAMPLC_BUTTONS 4'));await capture('platform-reset',0);receipt.checks.push('platform-reset-releases-held-key-and-reinitializes-GRAM-expanders-and-LEDs');
 await runtime.pause();receipt.guestSeconds=s.stats.time;receipt.serialEvidence=serial(s).split(/\r?\n/).filter(l=>/^STAMPLC_/.test(l));receipt.status='pass';console.log(JSON.stringify(receipt,null,2));
}finally{await runtime.dispose();}
