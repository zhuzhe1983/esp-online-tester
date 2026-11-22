#!/usr/bin/env node
// Execute an unchanged catalog-selected Seeed image in the actual browser worker
// under Node. Optional local-map is an investigator's URL->downloaded-file map;
// it does not change any catalog hash and is never a browser firmware mirror.
import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {join,basename} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';

const args=process.argv.slice(2),arg=(key,fallback)=>args.includes(key)?args[args.indexOf(key)+1]:fallback;
const id=arg('--product'),seconds=Number(arg('--seconds','8')),output=arg('--output','target/seeed-runtime');
assert(/^seeed-[a-z0-9-]+$/.test(id),'Use --product with an exact Seeed catalog ID.');
assert(Number.isFinite(seconds)&&seconds>0&&seconds<=120,'--seconds must be 0..120.');
const captureAt=arg('--capture-at')?.split(',').map(Number)??[];
assert(captureAt.length<=12&&captureAt.every((time,i)=>Number.isInteger(time)&&time>(i?captureAt[i-1]:seconds)&&time<=120),'--capture-at needs up to 12 increasing whole guest seconds after --seconds, at most 120.');
const root=new URL('../',import.meta.url),baseUrl='https://seeed-probe.invalid/platform/index.html',hash=b=>createHash('sha256').update(b).digest('hex');
const catalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async key=>[key,JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items])));
const product=catalog.products.find(p=>p.id===id),firmware=catalog.firmwares.find(f=>f.id===product?.defaultFirmwareId);
assert(product?.runtime?.board&&firmware?.source,'Product needs a real board and pinned default image.');
const localMap=arg('--local-map')?JSON.parse(await readFile(arg('--local-map'),'utf8')):{},resourceReads=[];
const localResponse=async(url,path,transport)=>{const data=await readFile(path);resourceReads.push({url,transport,bytes:data.length,sha256:hash(data)});return new Response(data);};
const fetchImpl=async value=>{
  const u=new URL(value);
  if(localMap[u.href])return localResponse(u.href,localMap[u.href],'investigator-local-map');
  if(u.origin===new URL(baseUrl).origin){
    const path=u.pathname.endsWith('_rom.elf')&&process.env.ESP32SIM_ROM_DIR?join(process.env.ESP32SIM_ROM_DIR,basename(u.pathname)):new URL(`web${u.pathname}`,root);
    try{return await localResponse(u.href,path,'repository-or-official-rom');}catch{return new Response('',{status:404});}
  }
  assert(!args.includes('--local-only'),`Unmapped source in local-only probe: ${u.href}`);
  return fetch(u,{signal:AbortSignal.timeout(60000),credentials:'omit',cache:'no-store'});
};
const workerFactory=url=>{
  const real=new URL(`web${url.pathname}`,root),script=`import{parentPort}from'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`;
  const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(script))),adapter={postMessage:(...a)=>thread.postMessage(...a),terminate:()=>thread.terminate()};
  thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;
};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl,assetBaseUrl:'https://seeed-probe.invalid/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:`${id}-probe`})});
const receipt={checkedAt:new Date().toISOString(),productId:id,firmwareId:firmware.id,backend:'actual-platform-worker-wasm',firmwarePatched:false,wasmSha256:hash(await readFile(new URL('web/wasm/esp32sim.wasm',root))),probeSha256:hash(await readFile(new URL(import.meta.url))),workerSha256:hash(await readFile(new URL('web/wasm/worker.js',root))),inputs:[],frames:[],resourceReads};
if(captureAt.length)receipt.observationDeadlines=captureAt;
const wait=async target=>{const deadline=Date.now()+240000;for(;;){const s=await runtime.observe();if(s.status==='error')throw Error(JSON.stringify(s.lastError));if(s.stats.time>=target)return s;assert(Date.now()<deadline,`Guest time timeout: ${s.stats.time}; ${s.serial.slice(-1000)}`);await new Promise(r=>setTimeout(r,20));}};
let previousFrame=null;
const capture=async label=>{
  await runtime.pause();const s=await runtime.observe();let f;
  if((await runtime.capabilities()).lora.available){receipt.radioSnapshots??=[];receipt.radioSnapshots.push({label,state:await runtime.readLoRa()});}
  await writeFile(join(output,`${id}-${label}.serial.txt`),s.serial);
  try{f=await runtime.capture({format:'rgba'});}catch(e){if(e.code!=='NO_FRAME')throw e;receipt.frames.push({label,guestSeconds:s.stats.time,insns:s.stats.insns,error:e.code});return s;}
  let nonwhite=0,nonblack=0,totalRgb=0,changed=0;const colors=new Set();
  for(let i=0;i<f.pixels.length;i+=4){
    const [r,g,b]=f.pixels.subarray(i,i+3);nonwhite+=r!==255||g!==255||b!==255;nonblack+=r!==0||g!==0||b!==0;totalRgb+=r+g+b;colors.add((r<<16)|(g<<8)|b);
    if(previousFrame?.length===f.pixels.length)changed+=r!==previousFrame[i]||g!==previousFrame[i+1]||b!==previousFrame[i+2];
  }
  const record={label,width:f.width,height:f.height,guestSeconds:s.stats.time,insns:s.stats.insns,nonwhitePixels:nonwhite,nonblackPixels:nonblack,colors:colors.size,meanRgb:totalRgb/(f.width*f.height*3),changedPixelsFromPrevious:previousFrame?.length===f.pixels.length?changed:null,rgbaSha256:hash(f.pixels),serialSha256:hash(s.serial),serialBytes:Buffer.byteLength(s.serial)};
  receipt.frames.push(record);previousFrame=f.pixels.slice();await writeFile(join(output,`${id}-${label}.rgba`),f.pixels);return s;
};
await mkdir(output,{recursive:true});
try {
  const scene=createDefaultScene(catalog,id);scene.firmware=structuredClone(firmware.source);if(scene.firmware.manifestUrl)scene.firmware.manifestUrl=new URL(scene.firmware.manifestUrl,baseUrl).href;await runtime.configure(scene);
  if(arg('--network-json')){const options=JSON.parse(await readFile(arg('--network-json'),'utf8'));receipt.networkFixture={sha256:hash(JSON.stringify(options)),router:(await runtime.configureNetwork(options)).router};}
  await runtime.loadFirmware();const loaded=await runtime.observe();receipt.initialFlash=loaded.storage?.scope;receipt.loadedFirmware=loaded.firmware;await runtime.start();await wait(seconds);await capture('boot');
  // These are observation deadlines, not synthetic button/touch events.
  for(const time of captureAt){await runtime.start();await wait(time);await capture(`observe-${time}s`);}
  if(arg('--lora-json')){
    const fixtureBytes=await readFile(arg('--lora-json')),fixture=JSON.parse(fixtureBytes),settle=Number(fixture.settleSeconds??3);
    assert(Number.isFinite(settle)&&settle>0&&settle<=15,'LoRa settleSeconds must be 0..15.');
    // The caller specifies the peer's complete radio config. Never copy or
    // repair it from the receiver: a mismatched packet must remain rejected.
    const before=await runtime.readLoRa();await runtime.start();const accepted=await runtime.receiveLoRa(fixture.packet);
    await wait(accepted.guestTime+settle);await capture('radio-peer');
    receipt.radioInput={fixtureSha256:hash(fixtureBytes),fixture,before,accepted,after:await runtime.readLoRa()};
  }
  if(arg('--inputs')){
    const inputs=JSON.parse(await readFile(arg('--inputs'),'utf8'));assert(Array.isArray(inputs)&&inputs.length<=30);
    for(const [i,event]of inputs.entries()){
      assert(event&&typeof event.label==='string'&&/^[a-z0-9-]+$/.test(event.label));
      const hold=Number(event.holdSeconds??0.5),settle=Number(event.settleSeconds??0.3),repeat=event.repeatSeconds===undefined?null:Number(event.repeatSeconds);
      assert(Number.isFinite(hold)&&hold>=0&&hold<=10&&Number.isFinite(settle)&&settle>=0&&settle<=15);
      assert(repeat===null||Number.isFinite(repeat)&&repeat>=0.02&&repeat<=hold);
      await runtime.start();const down=await runtime.input(event.down),at=down.guestTime,repeated=[];
      if(repeat!==null)for(let next=at+repeat;next<at+hold;next+=repeat){await wait(next);repeated.push(await runtime.input(event.down));}
      await wait(at+hold);if(event.captureHeld){await capture(`${i}-${event.label}-held`);await runtime.start();}
      const up=event.up?await runtime.input(event.up):null;await wait((up?.guestTime??(await runtime.observe()).stats.time)+settle);
      receipt.inputs.push({label:event.label,event:structuredClone(event),down,up,repeated});await capture(`${i}-${event.label}`);
    }
  }
  receipt.status='observed';
}catch(e){receipt.status='error';receipt.error={code:e.code||e.name,message:e.message};process.exitCode=1;
}finally{
  const state=await runtime.observe();receipt.artifacts=state.firmware?.artifacts;receipt.stats=state.stats;receipt.serialTail=state.serial.slice(-12000);receipt.serialSha256=hash(state.serial);receipt.serialBytes=Buffer.byteLength(state.serial);receipt.sdCard=state.sdCard;receipt.network=await runtime.readNetwork();
  receipt.limitations=['An execution receipt is not automatic compatibility certification. Inspect the real frame and guest output.','No browser DOM/WebGL/CORS or physical hardware claim; all bytes executed by the actual WASM worker.'];
  await writeFile(join(output,`${id}.serial.txt`),state.serial);await writeFile(join(output,`${id}.json`),JSON.stringify(receipt,null,2)+'\n');console.log(JSON.stringify({productId:id,frames:receipt.frames,stats:state.stats,status:receipt.status,...(receipt.error?{error:receipt.error}:{})}));
  await runtime.dispose();
}
