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
import {createFat32Image} from '../web/platform/sd-card-image.js';

const [url, expectedHash, output = 'output/ink4-sd', duration = '8'] = process.argv.slice(2);
if (!url || !/^[a-f0-9]{64}$/.test(expectedHash || '') || !/^https:\/\//.test(url)) throw new Error('Usage: node tools/probe-ink4-sd.mjs <https-url> <sha256> [output-dir] [guest-seconds]');
const seconds = Number(duration); if (!(seconds > 0 && seconds <= 300)) throw new Error('Guest seconds must be 0…300');
const clockTimeout = Number(process.env.ESP32SIM_PROBE_CLOCK_TIMEOUT_MS || 10000);
if (!(clockTimeout >= 1000 && clockTimeout <= 120000)) throw new Error('Clock timeout must be 1000…120000 ms');
const root = new URL('../', import.meta.url), catalog = {};
for (const name of ['products','accessories','firmwares']) catalog[name] = JSON.parse(await readFile(new URL(`web/platform/catalog/${name}.json`,root))).items;
const productId='metalio-ink4';
const product=catalog.products.find(p=>p.id===productId);assert.ok(product.runtime.board,'Catalog must declare the real board');
await mkdir(output,{recursive:true});
const downloaded=new Map();
const fetchImpl=async(value,options)=>{
  if(new URL(value).hostname==='platform.example')return new Response(await readFile(new URL(`web${new URL(value).pathname}`,root)));
  const key=String(value);if(!downloaded.has(key)){const response=await fetch(value,options);assert.ok(response.ok,`Download failed: ${response.status}`);downloaded.set(key,await response.arrayBuffer());}return new Response(downloaded.get(key).slice(0));
};
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
  if(event.type==='serial'&&event.source!=='emulator'){serial+=event.data;process.stdout.write(event.data);}
  if(event.type==='serial'&&event.source==='emulator')emulator+=event.data;
  if(event.type==='frame')latestFrame=event;
  if(event.type==='input')inputs.push(event);
  if(event.type==='stats'){const second=Math.floor(Number(event.stats.time));if(second!==lastSecond){lastSecond=second;console.log(JSON.stringify({guestSeconds:second,insns:event.stats.insns}));}}
}});
const started=Date.now();let failure=null;const stages=[],checks=[];
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const checkpoint=async name=>{
  assert.ok(latestFrame,`No frame at ${name}`);assert.equal(latestFrame.width,480);assert.equal(latestFrame.height,800);
  const state=await runtime.observe(),sha256=createHash('sha256').update(latestFrame.pixels).digest('hex');
  const black=latestFrame.pixels.reduce((n,v,i)=>n+(i%4===0&&v===0?1:0),0);
  assert.ok(black>1000&&black<480*800-1000,'Real UI must have both black and white pixels');
  await writeFile(`${output}/${name}.png`,png(latestFrame));
  stages.push({name,guestSeconds:await queryClock(),frameNumber:latestFrame.frameNumber,blackPixels:black,sha256,status:state.status});
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
// Test-owned card: one known root file, then the real firmware creates its own
// directories. Never invoke this harness against browser/user storage.
const marker=new TextEncoder().encode('INK4 SDMMC persistence probe\n');
const makeCard=()=>{
  const bytes=createFat32Image(),v=new DataView(bytes.buffer),start=v.getUint32(454,true),b=start*512,reserved=v.getUint16(b+14,true),fat=v.getUint32(b+36,true),data=(start+reserved+2*fat)*512;
  for(let i=0;i<2;i++)v.setUint32((start+reserved+i*fat)*512+12,0x0fffffff,true);
  bytes.set(new TextEncoder().encode('PROBE   TXT'),data+32);bytes[data+43]=0x20;v.setUint16(data+58,3,true);v.setUint32(data+60,marker.length,true);bytes.set(marker,data+512);
  for(const info of [b+512,b+7*512]){v.setUint32(info+488,v.getUint32(info+488,true)-1,true);v.setUint32(info+492,4,true);}return bytes;
};
const inspectCard=bytes=>{
  const v=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength),start=v.getUint32(454,true),b=start*512,sector=v.getUint16(b+11,true),cluster=sector*bytes[b+13],reserved=v.getUint16(b+14,true),fats=bytes[b+16],fat=v.getUint32(b+36,true),fatAt=(start+reserved)*sector,dataAt=(start+reserved+fats*fat)*sector;
  assert.equal(sector,512);assert.equal(fats,2);assert.ok(cluster>=512);const entries=[];
  const chain=first=>{const parts=[],seen=new Set();let c=first;while(c>=2&&c<0x0ffffff8){assert.ok(!seen.has(c)&&seen.size<65536,'Invalid FAT chain');seen.add(c);const at=dataAt+(c-2)*cluster;assert.ok(at+cluster<=bytes.length);parts.push(bytes.subarray(at,at+cluster));c=v.getUint32(fatAt+c*4,true)&0x0fffffff;}return Buffer.concat(parts);};
  const visit=(first,prefix,depth)=>{assert.ok(depth<16);const dir=chain(first);let names=[];for(let i=0;i<dir.length;i+=32){if(!dir[i])break;if(dir[i]===0xe5){names=[];continue;}if(dir[i+11]===15){const chars=[1,3,5,7,9,14,16,18,20,22,24,28,30].map(at=>dir.readUInt16LE(i+at)).filter(c=>c&&c!==0xffff);names[(dir[i]&31)-1]=String.fromCharCode(...chars);continue;}const short=dir.subarray(i,i+8).toString('ascii').trim()+(dir.subarray(i+8,i+11).toString('ascii').trim()?'.'+dir.subarray(i+8,i+11).toString('ascii').trim():'');const name=names.length?names.join(''):short;names=[];if(dir[i+11]&8||name==='.'||name==='..')continue;const c=(dir.readUInt16LE(i+20)<<16)|dir.readUInt16LE(i+26),size=dir.readUInt32LE(i+28),path=prefix+'/'+name;if(dir[i+11]&16){entries.push({path,directory:true});visit(c,path,depth+1);}else{const content=chain(c).subarray(0,size);entries.push({path,bytes:size,sha256:sha(content)});if(path==='/PROBE.TXT')assert.deepEqual(content,Buffer.from(marker));}}};
  visit(v.getUint32(b+44,true),'',0);return entries;
};
const storageCheckpoint=async name=>{
  await runtime.pause();await runtime.flushSdCard();const state=await runtime.observe(),card=await runtime.exportSdCard(),files=inspectCard(new Uint8Array(card.bytes));
  checks.push({name,sdCard:state.sdCard,imageSha256:sha(new Uint8Array(card.bytes)),files});
  assert.ok(files.some(f=>f.path==='/PROBE.TXT'),'Firmware must preserve supplied root input');
  assert.ok(files.some(f=>f.directory&&f.path.toLowerCase()==='/metalio'),'Firmware must create its application directory');
  for(const dir of ['recordings','books','fonts','a2ui_cache','chat_log','wallpaper'])assert.ok(files.some(f=>f.directory&&f.path.toLowerCase()==='/metalio/e-ink/'+dir),`Firmware must create ${dir}`);
  assert.ok(state.sdCard.changedPages>0);return files;
};
const assertMounted=from=>{const recent=serial.slice(from);assert.ok(recent.includes('SD卡挂载成功!'),'Official firmware must mount FAT32');assert.ok(recent.includes('PROBE.TXT'),'Guest root directory listing must read supplied marker');assert.ok(!recent.includes('SD卡初始化错误'),'Mount must not report an initialization failure');};
try {
  const scene=createDefaultScene(catalog,productId);await runtime.configure(scene);assert.equal((await runtime.capabilities()).sdCard.available,true);await runtime.loadFirmware({url,sha256:expectedHash});
  assert.equal((await runtime.observe()).sdCard.bytes,64*1024*1024);await runtime.attachSdCard({bytes:makeCard()});assert.equal((await runtime.observe()).status,'ready');await runtime.start();
  await waitGuest(seconds);assertMounted(0);await checkpoint('home');const firstFiles=await storageCheckpoint('first-boot');
  await runtime.start();await tap(395,670);await checkpoint('settings');await tap(65,400);await checkpoint('storage');
  const recent=serial;assert.ok(recent.includes('SettingsScreen: settings ready'),'Touch must enter the real settings screen');
  await button(['home'],0.15);await waitGuest((await queryClock())+3);await checkpoint('cover-home');
  await button(['volume-up'],0.15);assert.ok(serial.includes('VOLUME_UP (P1.0): volume='),'Semantic volume-up must be consumed by firmware');
  await button(['volume-down'],0.15);assert.ok(serial.includes('VOLUME_DOWN (P0.7): volume='),'Semantic volume-down must be consumed by firmware');
  await runtime.pause();await runtime.flushSdCard();const beforeReset=serial.length;await runtime.reset();await waitGuest(seconds);assertMounted(beforeReset);await checkpoint('reset-home');const resetFiles=await storageCheckpoint('reset');
  assert.deepEqual(resetFiles,firstFiles,'Reset must retain the supplied file and firmware-created directory entries');
  const beforeReload=serial.length;await runtime.loadFirmware({url,sha256:expectedHash});assert.equal((await runtime.observe()).sdCard.restored,true);await runtime.start();await waitGuest(seconds);assertMounted(beforeReload);await checkpoint('reload-home');const reloadFiles=await storageCheckpoint('reload');
  assert.deepEqual(reloadFiles,firstFiles,'Reload must restore the supplied file and firmware-created directory entries');
}catch(e){failure={message:e.message,code:e.code};process.exitCode=1;}
finally {
  try {
  const state=await runtime.observe();
  await writeFile(`${output}/serial.log`,serial);await writeFile(`${output}/emulator.log`,emulator);
  await writeFile(`${output}/receipt.json`,JSON.stringify({productId,board:product.runtime.board,url,expectedHash,
    wasmSha256:createHash('sha256').update(await readFile(new URL('web/wasm/esp32sim.wasm',root))).digest('hex'),
    firmware:state.firmware,stats:state.stats,stages,checks,inputs,failure,storageScope:'Test-owned in-memory store; reset/load persistence, not browser IndexedDB durability.',elapsedMs:Date.now()-started},null,2)+'\n');
  } finally { await runtime.dispose(); }
}
