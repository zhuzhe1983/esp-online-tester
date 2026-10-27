#!/usr/bin/env node
// Opt-in remote-firmware probe. Firmware bytes stay in RAM; only logs/frame PNGs
// and a hash receipt are written. Supply the upstream URL and SHA-256 explicitly.
import assert from 'node:assert/strict';
import {readFile, mkdir, writeFile} from 'node:fs/promises';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {deflateSync} from 'node:zlib';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createFat32Image} from '../web/platform/sd-card-image.js';
import {createDefaultScene} from '../web/platform/catalog.js';

const [url, expectedHash, output = 'output/official-papermono', duration = '30', scenario, productId='m5stack-papermono'] = process.argv.slice(2);
if (!url || !/^[a-f0-9]{64}$/.test(expectedHash || '') || !/^https:\/\//.test(url)) throw new Error('Usage: node tools/probe-official-papermono.mjs <https-url> <sha256> [output-dir] [guest-seconds]');
const seconds = Number(duration); if (!(seconds > 0 && seconds <= 300)) throw new Error('Guest seconds must be 0…300');
const clockTimeout = Number(process.env.ESP32SIM_PROBE_CLOCK_TIMEOUT_MS || 10000);
if (!(clockTimeout >= 1000 && clockTimeout <= 120000)) throw new Error('Clock timeout must be 1000…120000 ms');
const root = new URL('../', import.meta.url), catalog = {};
for (const name of ['products','accessories','firmwares']) catalog[name] = JSON.parse(await readFile(new URL(`web/platform/catalog/${name}.json`,root))).items;
assert.ok(['m5stack-papermono','m5stack-c153-lite'].includes(productId),'Choose full PaperMono or Lite');
const product=catalog.products.find(p=>p.id===productId);assert.ok(product.runtime.board,'Catalog must declare the real board');
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
let serial='',emulator='', latestFrame=null, lastSecond=-1;const inputs=[];
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl:'https://platform.example/platform/index.html',assetBaseUrl:'https://platform.example/',onEvent(event){
  if(event.type==='serial'&&event.source==='uart0'){serial+=event.data;process.stdout.write(event.data);}
  if(event.type==='serial'&&event.source==='emulator')emulator+=event.data;
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
  stages.push({name,guestSeconds:await queryClock(),lastStatSeconds:state.stats.time,frameNumber:latestFrame.frameNumber,nonBlackChannels:nonBlack,sha256,leds:state.leds,sdCard:state.sdCard,nfc:regionHash(latestFrame,122,361,230,49),loraRx:regionHash(latestFrame,140,45,330,50),brightness:regionHash(latestFrame,0,118,94,28),mute:regionHash(latestFrame,366,480,107,42)});
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
  const scene=createDefaultScene(catalog,productId);scene.firmware=null;
  await runtime.configure(scene);await runtime.loadFirmware({url,sha256:expectedHash});
  if(scenario==='sd-rgb') {
    // Public generated data only: a short root-directory file proves guest directory reads.
    const bytes=createFat32Image(),v=new DataView(bytes.buffer),start=v.getUint32(454,true),b=start*512,reserved=v.getUint16(b+14,true),fat=v.getUint32(b+36,true),root=(start+reserved+2*fat)*512;
    bytes.set(new TextEncoder().encode('HELLO   TXT'),root+32);bytes[root+43]=0x20;v.setUint16(root+58,3,true);v.setUint32(root+60,17,true);bytes.set(new TextEncoder().encode('ESP SD read test\n'),root+512);
    for(let i=0;i<2;i++)v.setUint32((start+reserved+i*fat)*512+12,0x0fffffff,true);
    await runtime.attachSdCard({bytes});
  }
  await runtime.start();
  await waitGuest(seconds);await checkpoint('boot');
  if(scenario==='lora') {
    assert.equal(productId,'m5stack-papermono');console.log('LORA_BOOT',JSON.stringify(await runtime.readLoRa()));assert.match(serial,/LoRa init OK rx_state=0/);
    const boot=await runtime.readLoRa();assert.equal(boot.rxCount,0);assert.equal(boot.txCount,0);
    await tap(423,157);await waitGuest((await queryClock())+8);await checkpoint('lora-sent');
    const tx=await runtime.readLoRa();assert.equal(tx.txCount,1);assert.equal(tx.transmitted[0].payloadHex,'48656c6c6f21');assert.equal(tx.rxCount,0);
    const packet={config:{frequency:0x36400000,sf:12,bandwidth:3,codingRate:4,syncWord:0x3444,implicitHeader:false,crc:true,invertIq:false},payloadHex:'50454552',rssiDbm:-70,snrQuarterDb:24,crcError:false};
    assert.deepEqual(tx.config,packet.config);
    const input=await runtime.receiveLoRa(packet);await waitGuest((await queryClock())+8);await checkpoint('lora-received');
    const rx=await runtime.readLoRa();assert.equal(rx.rxCount,1);assert.equal(stages.at(-1).loraRx,'6b6cadc313042a506d2d5b94f0af47656ddd78a8b5f539fe1ef0739e5a55aef4','Fixed UserDemo must render the visually verified Rx PEER glyph region');
    await writeFile(`${output}/lora.json`,JSON.stringify({boot,tx,packet,input,rx},null,2)+'\n');
  }
  if(scenario==='nfc'){
    assert.equal(productId,'m5stack-papermono');assert.match(serial,/Board variant detected: Pro/);
    await tap(423,385);await checkpoint('nfc-started');await waitGuest((await queryClock())+12);await checkpoint('nfc-finished');
    assert.equal(stages.at(-1).nfc,'759d6aec3de60367a804f0589b1e5447597fd16a756decc8b067f9062b37b339','Fixed UserDemo 1.2.0 must finish with the verified NO CARD glyph region, not merely enter SCANNING');
  }
  if(scenario==='lite')assert.match(serial,/Board variant detected: Lite/);
  if(scenario==='controls'){
    await button(['button-b'],0.15);await checkpoint('button-b');
    await button(['button-a'],0.15);await checkpoint('button-a');
    assert.notEqual(stages[1].brightness,stages[0].brightness,'B must change the actual brightness label');
    assert.equal(stages[2].brightness,stages[0].brightness,'A must restore the actual brightness label');
    await tap(45,78);await checkpoint('touch-plus');
    assert.notEqual(stages[3].brightness,stages[2].brightness,'Touch + must change the actual brightness label');
    await tap(412,500);await checkpoint('touch-mute');
    assert.notEqual(stages[4].mute,stages[3].mute,'Touch mute must change the actual firmware icon');
    await tap(412,500);await checkpoint('touch-unmute');
    assert.equal(stages[5].mute,stages[3].mute,'Second touch must restore the actual firmware mute icon');
  }
  if(scenario==='sd-rgb') {
    const before=(await runtime.observe()).leds[0];assert.deepEqual(before,[255,0,0]);
    await tap(65,300);await checkpoint('rgb-red-off');assert.deepEqual((await runtime.observe()).leds[0],[0,0,0]);
    await tap(65,332);await checkpoint('rgb-green-on');assert.deepEqual((await runtime.observe()).leds[0],[0,255,0]);
    await tap(65,364);await checkpoint('rgb-blue-on');assert.deepEqual((await runtime.observe()).leds[0],[0,255,255]);
    await tap(100,700);await checkpoint('sd-directory');assert.notEqual(stages.at(-1).sha256,stages.at(-2).sha256);
    const exported=await runtime.exportSdCard();assert.equal(exported.bytes.length,64*1024*1024);assert.ok(new TextDecoder().decode(exported.bytes).includes('HELLO   TXT'));
    await runtime.pause();await runtime.reset();assert.equal((await runtime.observe()).sdCard.restored,true);await runtime.start();await waitGuest(seconds);await checkpoint('sd-after-reset');
  }
  if(scenario==='crosspoint') {await runtime.flushSdCard();assert.ok((await runtime.observe()).sdCard.changedPages>0,'CrossPoint must write real card sectors');const bytes=(await runtime.exportSdCard()).bytes;await writeFile(`${output}/sd-write-summary.json`,JSON.stringify({bytes:bytes.length,sha256:createHash('sha256').update(bytes).digest('hex'),changedPages:(await runtime.observe()).sdCard.changedPages},null,2));}
  await runtime.pause();
}catch(e){failure={message:e.message,code:e.code};process.exitCode=1;}
finally{
  const state=await runtime.observe();
  await writeFile(`${output}/serial.log`,serial);
  await writeFile(`${output}/emulator.log`,emulator);
  if(latestFrame)await writeFile(`${output}/frame.png`,png(latestFrame));
  await writeFile(`${output}/receipt.json`,JSON.stringify({productId,board:product.runtime.board,url,expectedHash,wasmSha256:createHash('sha256').update(await readFile(new URL('web/wasm/esp32sim.wasm',root))).digest('hex'),firmware:state.firmware,storage:state.storage,sdCard:state.sdCard,leds:state.leds,stats:state.stats,frame:latestFrame?{width:latestFrame.width,height:latestFrame.height,frameNumber:latestFrame.frameNumber,sha256:createHash('sha256').update(latestFrame.pixels).digest('hex')}:null,lastError:state.lastError,inputs,stages,failure,elapsedMs:Date.now()-started},null,2)+'\n');
  await runtime.dispose();
}
