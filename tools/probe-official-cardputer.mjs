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

const [url, expectedHash, output = 'output/official-cardputer', duration = '30', scenario] = process.argv.slice(2);
if (!url || !/^[a-f0-9]{64}$/.test(expectedHash || '') || !/^https:\/\//.test(url)) throw new Error('Usage: node tools/probe-official-cardputer.mjs <https-url> <sha256> [output-dir] [guest-seconds]');
const seconds = Number(duration); if (!(seconds > 0 && seconds <= 300)) throw new Error('Guest seconds must be 0…300');
const clockTimeout = Number(process.env.ESP32SIM_PROBE_CLOCK_TIMEOUT_MS || 10000);
if (!(clockTimeout >= 1000 && clockTimeout <= 120000)) throw new Error('Clock timeout must be 1000…120000 ms');
const root = new URL('../', import.meta.url), catalog = {};
for (const name of ['products','accessories','firmwares']) catalog[name] = JSON.parse(await readFile(new URL(`web/platform/catalog/${name}.json`,root))).items;
const productId=process.env.ESP32SIM_PROBE_PRODUCT_ID || 'm5stack-k132';if(!['m5stack-k132','m5stack-k132-v11'].includes(productId))throw Error('Unsupported product for this original Cardputer probe');
const product=catalog.products.find(p=>p.id===productId);product.runtime={...product.runtime,board:'cardputer',status:'partial',flashMb:8,psramMb:0,touch:false};product.display={...product.display,width:240,height:135,touch:false};
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
  if(event.type==='serial'){serial+=event.data;process.stdout.write(event.data);}
  if(event.type==='frame')latestFrame=event;
  if(event.type==='input')inputs.push(event);
  if(event.type==='stats'){const second=Math.floor(Number(event.stats.time));if(second!==lastSecond){lastSecond=second;console.log(JSON.stringify({guestSeconds:second,insns:event.stats.insns}));}}
}});
const started=Date.now();let failure=null;const stages=[];
const lowerCanvasHash=frame=>{const hash=createHash('sha256');for(let y=60;y<135;y++)hash.update(frame.pixels.subarray((y*frame.width+36)*4,((y+1)*frame.width)*4));return hash.digest('hex');};
const menuInteriorHash=(frame,dx=0)=>{const hash=createHash('sha256');for(let y=55;y<130;y++)hash.update(frame.pixels.subarray((y*frame.width+48+dx)*4,(y*frame.width+208+dx)*4));return hash.digest('hex');};
const checkpoint=async name=>{
  if(!latestFrame)throw new Error(`No frame at ${name}`);
  const nonBlack=latestFrame.pixels.reduce((n,v,i)=>n+(i%4!==3&&v!==0?1:0),0);
  const state=await runtime.observe(),sha256=createHash('sha256').update(latestFrame.pixels).digest('hex');
  await writeFile(`${output}/${name}.png`,png(latestFrame));
  stages.push({name,guestSeconds:await queryClock(),lastStatSeconds:state.stats.time,frameNumber:latestFrame.frameNumber,nonBlackChannels:nonBlack,sha256,lowerCanvasSha256:lowerCanvasHash(latestFrame)});
  console.log('CHECKPOINT',JSON.stringify(stages.at(-1)));
  assert.ok(nonBlack>0 && nonBlack<latestFrame.width*latestFrame.height*3,'Official UI must contain actual nonblank pixels, not an initialized solid framebuffer');
};
const waitGuest=async target=>{
  const wall=Date.now();
  while(true){const state=await runtime.observe();if(state.status!=='running')throw new Error(`Runtime entered ${state.status}: ${JSON.stringify(state.lastError)}`);if(serial.includes('Guru Meditation Error'))throw new Error('Official firmware panicked; see serial.log');if(await queryClock()>=target)return;if(Date.now()-wall>240000)throw new Error(`Wall timeout waiting for guest ${target}s`);await new Promise(resolve=>setTimeout(resolve,10));}
};
const settleMenu=async()=>{
  const from=await queryClock();let changed=from,hash=lowerCanvasHash(latestFrame);
  while(true){const now=await queryClock(),next=lowerCanvasHash(latestFrame);if(next!==hash){hash=next;changed=now;}if(now-changed>=0.75)return;if(now-from>8)throw new Error('Official menu animation did not settle within 8 guest seconds');await waitGuest(now+0.1);}
};
const matrix=async(row,column)=>{
  try{const receipt=await runtime.input({type:'matrix-key',row,column,pressed:true});await waitGuest(receipt.guestTime+0.1);}
  finally{await runtime.input({type:'matrix-key',row,column,pressed:false});}
  await waitGuest((await queryClock())+1);
  await settleMenu();
};
try{
  const scene=createDefaultScene(catalog,productId);scene.firmware=null;
  await runtime.configure(scene);await runtime.loadFirmware({url,sha256:expectedHash});await runtime.start();
  await waitGuest(seconds);await checkpoint('boot');
  assert.match(serial,/Autodetect\] board_M5Cardputer/,'Official firmware must detect the original hardware');
  assert.doesNotMatch(serial,/Autodetect\] board_M5VAMeter/,'IR load must prevent the wrong panel geometry');
  assert.match(serial,/start boot anim/,'Official launcher must reach its real boot page');
  assert.doesNotMatch(serial,/Guru Meditation Error|ESP_ERROR_CHECK failed/,'Official firmware must not panic');
  if(scenario==='controls'){
    await matrix(2,13);await checkpoint('launcher');
    const originalMenu=menuInteriorHash(latestFrame);
    assert.notEqual(stages.at(-1).sha256,stages[0].sha256,'Real key release must leave the factory boot page');
    assert.notEqual(stages.at(-1).lowerCanvasSha256,stages[0].lowerCanvasSha256,'The lower canvas must update too: partial GDMA must not leave the boot image');
    await matrix(3,12);await checkpoint('right');
    assert.notEqual(stages.at(-1).sha256,stages.at(-2).sha256,'Right key must move the official launcher');
    await matrix(3,10);await checkpoint('left');
    // Original factory menu can settle two pixels away after the first transition.
    // Require every interior pixel to match, allowing only that measured integer shift.
    const returnShift=[-2,-1,0,1,2].find(dx=>menuInteriorHash(latestFrame,dx)===originalMenu);
    assert.notEqual(returnShift,undefined,'Left returns to the same official menu pixels within two pixels of menu positioning');
    stages.at(-1).menuReturnTranslationX=returnShift;
  }
  await runtime.pause();
}catch(e){failure={message:e.message,code:e.code};process.exitCode=1;}
finally{
  const state=await runtime.observe();
  await writeFile(`${output}/serial.log`,serial);
  if(latestFrame)await writeFile(`${output}/frame.png`,png(latestFrame));
  await writeFile(`${output}/receipt.json`,JSON.stringify({productId,url,expectedHash,wasmSha256:createHash('sha256').update(await readFile(new URL('web/wasm/esp32sim.wasm',root))).digest('hex'),firmware:state.firmware,storage:state.storage,stats:state.stats,frame:latestFrame?{width:latestFrame.width,height:latestFrame.height,frameNumber:latestFrame.frameNumber,sha256:createHash('sha256').update(latestFrame.pixels).digest('hex')}:null,lastError:state.lastError,inputs,stages,failure,elapsedMs:Date.now()-started},null,2)+'\n');
  await runtime.dispose();
}
