#!/usr/bin/env node
// Opt-in remote-firmware probe. Firmware bytes stay in RAM; only logs/frame PNGs
// and a hash receipt are written. Supply the upstream URL and SHA-256 explicitly.
import assert from 'node:assert/strict';
import {readFile, mkdir, writeFile} from 'node:fs/promises';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {deflateSync} from 'node:zlib';
import {createInterface} from 'node:readline';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';

const [url, expectedHash, output = 'output/official-tdeck', duration = '30', scenario] = process.argv.slice(2);
if (!url || !/^[a-f0-9]{64}$/.test(expectedHash || '') || !/^https:\/\//.test(url)) throw new Error('Usage: node tools/probe-official-tdeck.mjs <https-url> <sha256> [output-dir] [guest-seconds]');
const seconds = Number(duration); if (!(seconds > 0 && seconds <= 300)) throw new Error('Guest seconds must be 0…300');
const clockTimeout = Number(process.env.ESP32SIM_PROBE_CLOCK_TIMEOUT_MS || 10000);
if (!(clockTimeout >= 1000 && clockTimeout <= 120000)) throw new Error('Clock timeout must be 1000…120000 ms');
const root = new URL('../', import.meta.url), catalog = {};
for (const name of ['products','accessories','firmwares']) catalog[name] = JSON.parse(await readFile(new URL(`web/platform/catalog/${name}.json`,root))).items;
const product=catalog.products.find(p=>p.id==='lilygo-t-deck');assert.equal(product.runtime.board,'tdeck');
await mkdir(output,{recursive:true});
const fetchImpl = async (value,options) => new URL(value).hostname === 'platform.example' ? new Response(await readFile(new URL(`web${new URL(value).pathname}`,root))) : String(value)===url && process.env.ESP32SIM_FIRMWARE_FILE ? new Response(await readFile(process.env.ESP32SIM_FIRMWARE_FILE)) : fetch(value,options);
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
const checkpoint=async (name,region)=>{
  assert.match(name,/^[a-z0-9-]+$/,'Checkpoint name must stay within output directory');
  if(!latestFrame)throw new Error(`No frame at ${name}`);
  const nonBlack=latestFrame.pixels.reduce((n,v,i)=>n+(i%4!==3&&v!==0?1:0),0);
  const state=await runtime.observe(),sha256=createHash('sha256').update(latestFrame.pixels).digest('hex');
  await writeFile(`${output}/${name}.png`,png(latestFrame));
  stages.push({name,guestSeconds:await queryClock(),lastStatSeconds:state.stats.time,frameNumber:latestFrame.frameNumber,nonBlackChannels:nonBlack,sha256});
  if(region){const {x,y,width,height,sha256}=region;assert.ok([x,y,width,height].every(Number.isInteger)&&x>=0&&y>=0&&width>0&&height>0&&x+width<=latestFrame.width&&y+height<=latestFrame.height);const hash=createHash('sha256');for(let row=y;row<y+height;row++)hash.update(latestFrame.pixels.subarray((row*latestFrame.width+x)*4,(row*latestFrame.width+x+width)*4));stages.at(-1).region={x,y,width,height,sha256:hash.digest('hex')};if(sha256)assert.equal(stages.at(-1).region.sha256,sha256,`Official framebuffer region at ${name}`);}
  console.log('CHECKPOINT',JSON.stringify(stages.at(-1)));
  assert.ok(nonBlack>0 && nonBlack<latestFrame.width*latestFrame.height*3,'Official UI must contain actual nonblank pixels, not an initialized solid framebuffer');
};
const waitGuest=async target=>{
  const wall=Date.now();
  while(true){const state=await runtime.observe();if(state.status!=='running')throw new Error(`Runtime entered ${state.status}: ${JSON.stringify(state.lastError)}`);if(serial.includes('Guru Meditation Error'))throw new Error('Official firmware panicked; see serial.log');if(await queryClock()>=target)return;if(Date.now()-wall>240000)throw new Error(`Wall timeout waiting for guest ${target}s`);await new Promise(resolve=>setTimeout(resolve,10));}
};
try{
  const scene=createDefaultScene(catalog,'lilygo-t-deck');scene.firmware=null;
  await runtime.configure(scene);await runtime.loadFirmware({url,sha256:expectedHash});await runtime.start();
  await waitGuest(seconds);await checkpoint('boot');
  await assert.rejects(runtime.input({type:'matrix-key',row:3,column:0,pressed:true}),{code:'INVALID_INPUT'});
  if(scenario){
    const interactive=scenario==='interactive';
    const steps=interactive?createInterface({input:process.stdin,terminal:false}):JSON.parse(await readFile(scenario,'utf8'));
    if(interactive){await runtime.pause();console.log('READY FOR JSON STEP');}
    for await(const value of steps){
      if(interactive && value==='quit')break;
      const step=interactive?JSON.parse(value):value;
      if(interactive)await runtime.start();
      if(step.wait)await waitGuest((await queryClock())+step.wait);
      for(const input of step.inputs||[]){const receipt=await runtime.input(input.event);if(input.holdSeconds)await waitGuest(receipt.guestTime+input.holdSeconds);}
      await waitGuest((await queryClock())+(step.settleSeconds||1));await checkpoint(step.name,step.region);
      if(step.changedFrom){const previous=stages.find(s=>s.name===step.changedFrom);assert.ok(previous);assert.notEqual(stages.at(-1).sha256,previous.sha256,'Input must change real official framebuffer');}
      if(interactive){await runtime.pause();console.log('READY FOR JSON STEP');}
    }
    if(interactive)steps.close();
  }
  await runtime.pause();
}catch(e){failure={message:e.message,code:e.code};process.exitCode=1;}
finally{
  const state=await runtime.observe();
  await writeFile(`${output}/serial.log`,serial);
  if(latestFrame)await writeFile(`${output}/frame.png`,png(latestFrame));
  await writeFile(`${output}/receipt.json`,JSON.stringify({url,expectedHash,wasmSha256:createHash('sha256').update(await readFile(new URL('web/wasm/esp32sim.wasm',root))).digest('hex'),firmware:state.firmware,storage:state.storage,stats:state.stats,frame:latestFrame?{width:latestFrame.width,height:latestFrame.height,frameNumber:latestFrame.frameNumber,sha256:createHash('sha256').update(latestFrame.pixels).digest('hex')}:null,lastError:state.lastError,inputs,stages,failure,elapsedMs:Date.now()-started},null,2)+'\n');
  await runtime.dispose();
}
