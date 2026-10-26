#!/usr/bin/env node
// Opt-in remote-firmware probe. Firmware bytes stay in RAM; only logs/frame PNGs
// and a hash receipt are written. Supply the upstream URL and SHA-256 explicitly.
import assert from 'node:assert/strict';
import {readFile, mkdir, writeFile} from 'node:fs/promises';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {deflateSync} from 'node:zlib';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';

const [url, expectedHash, output = 'output/official-stopwatch', duration = '30', scenario] = process.argv.slice(2);
if (!url || !/^[a-f0-9]{64}$/.test(expectedHash || '') || !/^https:\/\//.test(url)) throw new Error('Usage: node tools/probe-official-stopwatch.mjs <https-url> <sha256> [output-dir] [guest-seconds]');
const seconds = Number(duration); if (!(seconds > 0 && seconds <= 300)) throw new Error('Guest seconds must be 0…300');
const clockTimeout = Number(process.env.ESP32SIM_PROBE_CLOCK_TIMEOUT_MS || 10000);
if (!(clockTimeout >= 1000 && clockTimeout <= 120000)) throw new Error('Clock timeout must be 1000…120000 ms');
const root = new URL('../', import.meta.url), catalog = {};
for (const name of ['products','accessories','firmwares']) catalog[name] = JSON.parse(await readFile(new URL(`web/platform/catalog/${name}.json`,root))).items;
await mkdir(output,{recursive:true});
const fetchImpl = async (value,options) => new URL(value).hostname === 'platform.example' ? new Response(await readFile(new URL(`web${new URL(value).pathname}`,root))) : fetch(value,options);
let queryClock;
const workerFactory = path => {
  path = new URL(`web${new URL(path).pathname}`,root);
  const script = `import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(m,t)=>parentPort.postMessage(m,t);await import(${JSON.stringify(String(path))});parentPort.on('message',data=>globalThis.onmessage({data}));`;
  const thread = new Worker(new URL(`data:text/javascript,${encodeURIComponent(script)}`));
  const adapter = {postMessage:(message,...args)=>thread.postMessage(message.op==='create'&&(process.env.ESP32SIM_NO_WASM_JIT||process.env.ESP32SIM_FORCE_WASM_JIT)?{...message,jit:!!process.env.ESP32SIM_FORCE_WASM_JIT}:message,...args),terminate:()=>thread.terminate()};
  let clockSequence=0;const clocks=new Map();
  // '{}' is an ignored protocol message: it adds no bytes/events to any guest
  // device. Reuse its queue receipt to sample actual cycles between stat pushes.
  queryClock=()=>new Promise((resolve,reject)=>{const inputId=`probe-clock-${++clockSequence}`;const timer=setTimeout(()=>{clocks.delete(inputId);reject(new Error(`Probe clock query timeout (${clockTimeout} ms)`));},clockTimeout);clocks.set(inputId,time=>{clearTimeout(timer);resolve(time);});adapter.postMessage({op:'text',data:'{}',inputId});});
  thread.on('message',data=>{if(clocks.has(data.inputQueued)){clocks.get(data.inputQueued)(data.time);clocks.delete(data.inputQueued);}else adapter.onmessage?.({data});});thread.on('error',event=>adapter.onerror?.(event));return adapter;
};
const crc32 = bytes => {let n=0xffffffff;for(const value of bytes){n^=value;for(let i=0;i<8;i++)n=(n>>>1)^((n&1)?0xedb88320:0);}return (n^0xffffffff)>>>0;};
const png = frame => {
  const chunk=(type,bytes)=>{const name=Buffer.from(type),size=Buffer.alloc(4),crc=Buffer.alloc(4);size.writeUInt32BE(bytes.length);crc.writeUInt32BE(crc32(Buffer.concat([name,bytes])));return Buffer.concat([size,name,bytes,crc]);};
  const header=Buffer.alloc(13);header.writeUInt32BE(frame.width);header.writeUInt32BE(frame.height,4);header[8]=8;header[9]=6;
  const rows=Buffer.alloc((frame.width*4+1)*frame.height);for(let y=0;y<frame.height;y++)rows.set(frame.pixels.subarray(y*frame.width*4,(y+1)*frame.width*4),y*(frame.width*4+1)+1);
  return Buffer.concat([Buffer.from([137,80,78,71,13,10,26,10]),chunk('IHDR',header),chunk('IDAT',deflateSync(rows)),chunk('IEND',Buffer.alloc(0))]);
};
let serial='', latestFrame=null, lastSecond=-1;const inputs=[];
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl:'https://platform.example/platform/index.html',assetBaseUrl:'https://platform.example/',onEvent(event){
  if(event.type==='serial'&&event.source==='uart0'){serial+=event.data;process.stdout.write(event.data);}
  if(event.type==='frame')latestFrame=event;
  if(event.type==='input')inputs.push(event);
  if(event.type==='stats'){const second=Math.floor(Number(event.stats.time));if(second!==lastSecond){lastSecond=second;console.log(JSON.stringify({guestSeconds:second,insns:event.stats.insns}));}}
}});
const started=Date.now();let failure=null;const stages=[];
const regionHash=(frame,x,y,w,h)=>{const hash=createHash('sha256');for(let row=y;row<y+h;row++)hash.update(frame.pixels.subarray((row*frame.width+x)*4,(row*frame.width+x+w)*4));return hash.digest('hex');};
const checkpoint=async name=>{
  if(!latestFrame)throw new Error(`No frame at ${name}`);
  const nonBlack=latestFrame.pixels.reduce((n,v,i)=>n+(i%4!==3&&v!==0?1:0),0);
  const state=await runtime.observe(),sha256=createHash('sha256').update(latestFrame.pixels).digest('hex');
  await writeFile(`${output}/${name}.png`,png(latestFrame));
  stages.push({name,guestSeconds:await queryClock(),lastStatSeconds:state.stats.time,frameNumber:latestFrame.frameNumber,nonBlackChannels:nonBlack,sha256,bodyHash:regionHash(latestFrame,0,50,468,416),timerHash:regionHash(latestFrame,0,148,468,112),menuIconHash:regionHash(latestFrame,80,90,308,220),lapsHash:regionHash(latestFrame,0,270,468,196)});
  console.log('CHECKPOINT',JSON.stringify(stages.at(-1)));
};
const waitGuest=async target=>{
  const wall=Date.now();
  while(true){const state=await runtime.observe();if(state.status!=='running')throw new Error(`Runtime entered ${state.status}: ${JSON.stringify(state.lastError)}`);if(serial.includes('Guru Meditation Error'))throw new Error('Official firmware panicked; see serial.log');if(await queryClock()>=target)return;if(Date.now()-wall>240000)throw new Error(`Wall timeout waiting for guest ${target}s`);await new Promise(resolve=>setTimeout(resolve,10));}
};
const button=async(ids,seconds)=>{
  let receipt;
  try{for(const id of ids)receipt=await runtime.input({type:'button',id,pressed:true});await waitGuest(receipt.guestTime+seconds);}
  finally{for(const id of ids){const released=await runtime.input({type:'button',id,pressed:false});if(seconds<0.5&&receipt&&released.guestTime-receipt.guestTime>0.5)throw new Error(`SHORT_PRESS_OVERRUN: short press lasted ${(released.guestTime-receipt.guestTime).toFixed(3)} guest seconds`);}}
  await waitGuest((await queryClock())+1.5);
};
const tap=async(x,y)=>{
  try{const receipt=await runtime.input({type:'touch',x,y,down:true});await waitGuest(receipt.guestTime+0.15);}
  finally{await runtime.input({type:'touch',x,y,down:false});}
  await waitGuest((await queryClock())+1);
};
try{
  await runtime.configure(createDefaultScene(catalog,'m5stack-stopwatch'));
  await runtime.loadFirmware({url,sha256:expectedHash});await runtime.start();
  await waitGuest(seconds);await checkpoint('boot');
  if(scenario==='imu'){
    if(serial.includes('[HAL-IMU] init failed')||serial.includes('BMI270_BMM150: Error'))throw new Error('BMI270 initialization failed');
    await button(['button-a','button-b'],1.5);await checkpoint('imu-menu-start');
    // V0.5 launcher has native touch arrows. Use them here; this sensor probe
    // does not depend on host scheduling a GPIO release below the 500 ms hold threshold.
    for(let index=0;index<4;index++)await tap(434,233);
    await checkpoint('imu-menu');await tap(234,233);
    if(!serial.includes('[IMU] on open'))throw new Error('Menu navigation did not enter the official IMU application');
    const h=Math.SQRT1_2;
    for(const [name,quaternion] of [['upright',[0,0,0,1]],['tilt-x',[h,0,0,h]],['tilt-z',[0,0,h,h]],['inverted',[1,0,0,0]],['upright-again',[0,0,0,1]]]){
      const receipt=await runtime.setImu({quaternion,angularVelocityDps:[0,0,0]});
      await waitGuest(receipt.guestTime+2);await checkpoint(`imu-${name}`);stages.at(-1).imu=receipt;
    }
    const gyro=await runtime.setImu({quaternion:[0,0,0,1],angularVelocityDps:[0,0,90]});
    await waitGuest(gyro.guestTime+1);await checkpoint('imu-gyro');stages.at(-1).imu=gyro;
    await runtime.setImu({enabled:false});await waitGuest((await queryClock())+1);await checkpoint('imu-disabled');
    const frozen=await runtime.observe();await runtime.setImu({source:'scene',quaternion:[1,0,0,0]});
    assert.deepEqual((await runtime.observe()).imu.quaternion,frozen.imu.quaternion,'Disabled scene coupling must hold the orientation');
    const samples=stages.filter(stage=>stage.name.startsWith('imu-')&&stage.imu);
    assert.ok(new Set(samples.map(stage=>stage.bodyHash)).size>=4,'Real firmware IMU frames must change for distinct gravity/rotation inputs');
    if(serial.includes('imu invalid'))throw new Error('Official HAL rejected the IMU');
  }
  if(scenario==='controls'){
    await button(['button-a','button-b'],1.5);await checkpoint('menu');
    await button(['button-b'],0.15);await checkpoint('next');
    await button(['button-a'],0.15);await checkpoint('previous');
    await tap(234,233);await checkpoint('app');
    await button(['button-a','button-b'],1.5);await checkpoint('return-menu');
    let receipt=await runtime.input({type:'touch',x:234,y:15,down:true});
    try{for(const y of [50,100,150,220,300]){await waitGuest(receipt.guestTime+0.12);receipt=await runtime.input({type:'touch',x:234,y,down:true});}await waitGuest(receipt.guestTime+0.15);}
    finally{await runtime.input({type:'touch',x:234,y:300,down:false});}
    await waitGuest((await queryClock())+1.5);await checkpoint('battery');
    await button(['button-b'],0.15);await button(['button-b'],0.15);await checkpoint('stopwatch-menu');
    await tap(234,233);await checkpoint('stopwatch-idle');
    if(!serial.includes('[Stopwatch] on open'))throw new Error('Touch did not enter the official Stopwatch application');
    await button(['button-b'],0.15);await checkpoint('stopwatch-running');
    await button(['button-a'],0.15);await checkpoint('stopwatch-lap');
    await button(['button-b'],0.15);await checkpoint('stopwatch-paused');
    await waitGuest((await queryClock())+2);await checkpoint('stopwatch-paused-stable');
    await button(['button-a'],0.15);await checkpoint('stopwatch-reset');
    const at=name=>stages.find(stage=>stage.name===name);
    assert.notEqual(at('menu').menuIconHash,at('next').menuIconHash,'B must change the menu icon');
    assert.equal(at('menu').menuIconHash,at('previous').menuIconHash,'A must return to the preceding menu icon');
    assert.notEqual(at('stopwatch-idle').timerHash,at('stopwatch-running').timerHash,'B must start the elapsed timer');
    assert.notEqual(at('stopwatch-idle').lapsHash,at('stopwatch-lap').lapsHash,'A while running must add a lap');
    assert.equal(at('stopwatch-paused').timerHash,at('stopwatch-paused-stable').timerHash,'B must hold the elapsed time while paused');
    assert.equal(at('stopwatch-idle').bodyHash,at('stopwatch-reset').bodyHash,'A while paused must restore the initial stopwatch body, excluding the independent wall clock');
    if(stages.some(stage=>stage.nonBlackChannels<1000))throw new Error('An official UI checkpoint is effectively black');
    if(new Set(stages.map(stage=>stage.sha256)).size<5)throw new Error('Inputs did not produce enough distinct official UI states');
  }
  await runtime.pause();
}catch(error){failure={code:error.code,message:error.message};console.error(error);}
finally{
  const state=await runtime.observe();
  await writeFile(`${output}/serial.log`,serial);
  if(latestFrame)await writeFile(`${output}/frame.png`,png(latestFrame));
  await writeFile(`${output}/receipt.json`,JSON.stringify({url,expectedHash,firmware:state.firmware,stats:state.stats,frame:latestFrame?{width:latestFrame.width,height:latestFrame.height,frameNumber:latestFrame.frameNumber,sha256:createHash('sha256').update(latestFrame.pixels).digest('hex')}:null,lastError:state.lastError,inputs,stages,failure,elapsedMs:Date.now()-started},null,2)+'\n');
  await runtime.dispose();
}
if(failure)process.exitCode=1;
