#!/usr/bin/env node
// Internal, source-built IDF firmware on the real platform worker/WASM path.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {resolve,join,basename} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {inspectLocalBuild} from '../web/platform/local-firmware.js';
import {createDefaultScene} from '../web/platform/catalog.js';
const argument=name=>{const i=process.argv.indexOf(name);return i<0?null:process.argv[i+1];};
const build=argument('--build');if(!build)throw Error('Build tools/fixtures/stampfly-firmware with ESP-IDF 5.5.1 and supply --build <build-directory>.');
const folder=resolve(build),root=new URL('../',import.meta.url),origin='https://stampfly-fixture.invalid',hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const files=[new File([await readFile(join(folder,'flasher_args.json'))],'flasher_args.json')],inputHashes={};
for(const path of (await inspectLocalBuild(files)).missingFiles){const bytes=await readFile(join(folder,path)),file=new File([bytes],basename(path));inputHashes[path]=hash(bytes);Object.defineProperty(file,'webkitRelativePath',{value:path});files.push(file);}
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const products=argument('--product')?[argument('--product')]:['m5stack-k138','m5stack-k138-v11'];
assert(products.every(id=>['m5stack-k138','m5stack-k138-v11'].includes(id)),'Only the two independently identified StampFly products are accepted.');
const fetchImpl=async value=>{const url=new URL(value);assert.equal(url.origin,origin);try{return new Response(await readFile(new URL('web'+url.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{
 const real=new URL('web'+url.pathname,root),thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));
 const adapter={postMessage:(message,...args)=>thread.postMessage(message.op==='create'&&process.argv.includes('--no-jit')?{...message,jit:false}:message,...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;
};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'stampfly-probe'})});
const receipt={checkedAt:new Date().toISOString(),command:'node tools/probe-stampfly.mjs --build <fixture-build-directory>',backend:'actual-wasm-worker',jit:!process.argv.includes('--no-jit'),firmware:'internal ESP-IDF 5.5.1 protocol fixture',wasmSha256:hash(await readFile(new URL('web/wasm/esp32sim.wasm',root))),inputHashes,products:[],limitations:['Internal fixture only; this is not an official factory firmware compatibility result.','No framebuffer, flight dynamics, thrust/RPM calibration, radio controller or physical sensor timing is claimed.','BMI270 embedded configuration bytes are transferred completely, but the sensor program itself is not executed.','VL53L3CX, BMP280, BMM150 and PMW3901 are not exercised by this fixture.']};
const delay=ms=>new Promise(r=>setTimeout(r,ms));
const wait=async(label,predicate)=>{const end=Date.now()+180000;while(Date.now()<end){const state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(JSON.stringify(state.lastError));await delay(20);}const state=await runtime.observe();throw Error(`${label} timed out: ${JSON.stringify({leds:state.leds,motors:state.motors})}\n${state.serial.slice(-4000)}`);};
const colors=[[255,0,0],[0,255,0],[0,0,255],[0,0,0]],pins=[5,42,10,41],ids=['front-left','front-right','rear-left','rear-right'];
const same=(a,b)=>JSON.stringify(a)===JSON.stringify(b);
const duties=step=>pins.map((_,i)=>step===3?0:32*(i+1+step));
const outputMatches=(state,step)=>same(state.leds,[colors[step],...(step?[[32,0,0],[0,32,0]]:[[0,32,0],[0,0,32]])])&&state.motors?.length===4&&state.motors.every((m,i)=>m.id===ids[i]&&m.gpio===pins[i]&&Math.abs(m.duty-duties(step)[i]/256)<1/65535&&Math.abs(m.frequencyHz-150000)<1500);
try {
 for(const productId of products){
  const record={productId,checks:[],outputs:[]};receipt.products.push(record);
  const scene=createDefaultScene(catalog,productId);scene.firmware=null;await runtime.configure(scene);await runtime.loadFirmware({format:'build',files});await runtime.start();
  const gateColor=productId.endsWith('-v11')?[0,0,0]:[255,0,0];
  const low=await wait('G38 low',s=>s.serial.includes('STAMPFLY_GATE_LOW'));
  assert(!low.serial.includes('STAMPFLY_GATE_HIGH_NO_FRAME'),'Missed the low-gate observation window.');assert.deepEqual(low.leds,[gateColor,[0,32,0],[0,0,32]]);
  const high=await wait('G38 high without another RMT frame',s=>s.serial.includes('STAMPFLY_GATE_HIGH_NO_FRAME'));
  assert(!high.serial.includes('STAMPFLY_READY'),'Missed the high-gate observation window.');assert.deepEqual(high.leds,[gateColor,[0,32,0],[0,0,32]]);
  record.gate={low:low.leds,highBeforeNextFrame:high.leds};record.checks.push('product-specific-G38-gate-before-and-after-power-without-invented-frame');
  let state=await wait('real GPIO/SPI/I2C/PWM initialization',s=>s.serial.includes('STAMPFLY_READY')&&s.serial.includes('STAMPFLY_BUTTON 1')&&outputMatches(s,0));
  for(const expected of ['STAMPFLY_INA 5449 3220 4000','STAMPFLY_I2C_WRONG_ADDRESS NACK','STAMPFLY_SPI_OTHER_CS ff','STAMPFLY_IMU_ID 24','STAMPFLY_IMU_INIT 01'])assert(state.serial.includes(expected),expected);
  record.checks.push('INA3221-big-endian-identity-channel2-bus-voltage-and-unoccupied-address-NACK','BMI270-SPI-address-dummy-data-CS46-and-wrong-CS12-open-bus','complete-Bosch-8192-byte-configuration-upload-and-ready-status');record.outputs.push({step:0,leds:state.leds,motors:state.motors});
  for(const step of [1,2,3,0]){
   const at=(await runtime.observe()).serial.length;await runtime.input({type:'button',id:'button',pressed:true});state=await wait(`fresh press step ${step}`,s=>s.serial.slice(at).includes('STAMPFLY_BUTTON 0')&&s.serial.slice(at).includes(`STAMPFLY_OUTPUT ${step} ${duties(step).join(' ')}`)&&outputMatches(s,step));
   record.outputs.push({step,leds:state.leds,motors:state.motors});const releaseAt=state.serial.length;await runtime.input({type:'button',id:'button',pressed:false});await wait('fresh release',s=>s.serial.slice(releaseAt).includes('STAMPFLY_BUTTON 1'));
  }
  record.checks.push('fresh-G0-press-release-drives-three-RGBs-and-four-distinct-routed-PWM-channels-including-zero-duty');
  await assert.rejects(runtime.input({type:'button',id:'unknown',pressed:true}));await assert.rejects(runtime.setImu({source:'scene',quaternion:[0,0,0,1]}));await assert.rejects(runtime.setImu({quaternion:[0,0,0,0]}));assert.equal((await runtime.observe()).frame,null);
  record.checks.push('unknown-button-invalid-quaternion-and-uncalibrated-scene-IMU-rejected-no-framebuffer');
  let at=(await runtime.observe()).serial.length;await runtime.setImu({source:'agent',quaternion:[0,0,0,1],angularVelocityDps:[0,0,0]});state=await wait('fresh neutral IMU read',s=>s.serial.slice(at).includes('STAMPFLY_ACCEL 0 4096 0')&&s.serial.slice(at).includes('STAMPFLY_GYRO 0 0 0'));record.imuSamples={neutral:state.serial.slice(at).split(/\r?\n/).filter(l=>/^STAMPFLY_(ACCEL|GYRO) /.test(l))};
  at=(await runtime.observe()).serial.length;await runtime.setImu({source:'agent',quaternion:[0,0,Math.SQRT1_2,Math.SQRT1_2],angularVelocityDps:[125,0,0]});state=await wait('fresh rotated IMU read',s=>s.serial.slice(at).includes('STAMPFLY_ACCEL 4096 0 0')&&s.serial.slice(at).includes('STAMPFLY_GYRO 2048 0 0'));record.imuSamples.rotated=state.serial.slice(at).split(/\r?\n/).filter(l=>/^STAMPFLY_(ACCEL|GYRO) /.test(l));
  record.checks.push('explicit-virtual-orientation-reaches-guest-SPI-accel-8g-and-gyro-2000dps-registers');
  await runtime.pause();const paused=await runtime.observe();await delay(100);const still=await runtime.observe();assert.equal(still.serial,paused.serial);assert.deepEqual(still.motors,paused.motors);assert.deepEqual(still.motorOutput,paused.motorOutput);assert.equal(still.stats.time,paused.stats.time);
  at=still.serial.length;await runtime.start();await wait('resume clears gyro and retains accel',s=>s.serial.slice(at).includes('STAMPFLY_ACCEL 4096 0 0')&&s.serial.slice(at).includes('STAMPFLY_GYRO 0 0 0'));record.checks.push('pause-halts-guest-output-and-PWM-clock-resume-clears-gyro');
  at=(await runtime.observe()).serial.length;await runtime.input({type:'button',id:'button',pressed:true});await wait('held button before reset',s=>s.serial.slice(at).includes('STAMPFLY_BUTTON 0')&&outputMatches(s,1));await runtime.reset();state=await wait('reset releases button and resets outputs',s=>s.serial.includes('STAMPFLY_BUTTON 1')&&outputMatches(s,0));assert(!state.serial.includes('STAMPFLY_BUTTON 0'));assert.equal(state.frame,null);
  record.checks.push('fresh-worker-reset-releases-held-G0-and-restores-initial-PWM-and-RGB');await runtime.pause();record.status='pass';record.guestSeconds=state.stats.time;record.serialEvidence=state.serial.split(/\r?\n/).filter(l=>/^STAMPFLY_/.test(l)).filter((l,i,a)=>a.indexOf(l)===i);
 }
 receipt.status='pass';console.log(JSON.stringify(receipt,null,2));
} finally {await runtime.dispose();}
