#!/usr/bin/env node
// Optional publisher-firmware acceptance through the public runtime, production
// Worker and WASM. Supply independently downloaded, hash-verified input files.
// This is not browser/UI acceptance and does not mirror third-party firmware.
import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {Worker} from 'node:worker_threads';
import {resolve,join,basename} from 'node:path';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createSdCardStorage} from '../web/platform/sd-card-storage.js';

const [mode,firmwarePath,cardPath,outputPath]=process.argv.slice(2);
assert(['default','basic'].includes(mode)&&firmwarePath&&cardPath&&outputPath,
  'Usage: node tools/waveshare-epaper133-acceptance.mjs <default|basic> <01_factory.bin> <fresh.img> <output-directory>');
const root=new URL('../',import.meta.url),origin='https://waveshare-acceptance.invalid';
const output=resolve(outputPath),hash=bytes=>createHash('sha256').update(bytes).digest('hex');
await mkdir(output,{recursive:true});
const catalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async key=>
  [key,JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items])));
const product=catalog.products.find(p=>p.id==='waveshare-esp32-s3-epaper-13-3e6');
const firmware=catalog.firmwares.find(f=>f.id===product.defaultFirmwareId);
assert.equal(firmware.role,'factory');assert.equal(firmware.status,'trial');
const firmwareBytes=await readFile(firmwarePath),card=await readFile(cardPath);
assert.equal(hash(firmwareBytes),firmware.source.sha256);assert.equal(card.length,64*1024*1024);
const requests=[],inputs=[],workerMessages=[];
const workerFactory=url=>{
  const source=`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(new URL('web'+url.pathname,root)))});parentPort.on('message',data=>globalThis.onmessage({data}));`;
  const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(source)));
  const adapter={postMessage:(...args)=>{
    if(['create','start','text'].includes(args[0].op))workerMessages.push(args[0]);
    thread.postMessage(...args);
  },terminate:()=>thread.terminate()};
  thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',error=>adapter.onerror?.(error));return adapter;
};
const storageOptions={indexedDB:null,BroadcastChannel:null,dbName:'waveshare-epaper133-acceptance-'+mode+'-'+Date.now()};
const runtime=createPlatformRuntime({catalog,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',workerFactory,
  deviceStorage:createDeviceStorage(storageOptions),sdStorage:createSdCardStorage(storageOptions),fetchImpl:async value=>{
    const url=new URL(value,origin);requests.push(String(url));
    if(String(url)===firmware.source.url)return new Response(firmwareBytes);
    if(url.origin===origin){
      const path=new URL('web'+url.pathname,root);
      try{return new Response(await readFile(path));}
      catch(error){
        if(error.code==='ENOENT'&&process.env.ESP32SIM_ROM_DIR&&url.pathname.endsWith('_rom.elf'))
          return new Response(await readFile(join(process.env.ESP32SIM_ROM_DIR,basename(url.pathname))));
        throw error;
      }
    }
    throw Error('Unexpected acceptance dependency '+String(url));
  }});
const started=Date.now(),deadline=started+15*60*1000;
let lastSecond=-1,state,result;
async function waitForTime(seconds){
  while(Date.now()<deadline){
    state=await runtime.observe();
    if(state.status==='error')throw Error(JSON.stringify(state.lastError));
    const time=Number(state.stats?.time||0),whole=Math.floor(time);
    if(whole!==lastSecond){lastSecond=whole;console.log(JSON.stringify({guestSeconds:time,status:state.status,frame:state.frame,wallSeconds:(Date.now()-started)/1000}));}
    if(time>=seconds)return state;
    if(!['running','ready'].includes(state.status))throw Error('Unexpected stop '+state.status);
    await new Promise(resolve=>setTimeout(resolve,20));
  }
  throw Error('Wall timeout before guest second '+seconds);
}
async function press(at,duration){
  await waitForTime(at);
  const down=await runtime.input({type:'button',id:'boot',pressed:true});
  await waitForTime(down.guestTime+duration);
  const up=await runtime.input({type:'button',id:'boot',pressed:false});
  inputs.push({requestedAt:at,requestedDuration:duration,down,up});
  console.log(JSON.stringify({input:inputs.at(-1)}));
}
try{
  await runtime.configure(createDefaultScene(catalog,product.id));
  const prepared=await runtime.observe();assert.equal(prepared.sdCard.bytes,64*1024*1024);assert.equal(prepared.sdCard.format,'fat32');
  await runtime.attachSdCard({bytes:new Uint8Array(card)});
  await runtime.loadFirmware(firmware.source);
  state=await runtime.observe();assert.equal(state.sdCard.mounted,true);assert.equal(state.firmware.appDirect,false);
  await runtime.start();
  if(mode==='basic')for(const [at,duration] of [[2,1.8],[5,.12],[6.5,1.8],[10,.12]])await press(at,duration);
  await waitForTime(mode==='basic'?40:6);await runtime.pause();state=await runtime.observe();
  let frameProof=null;
  if(mode==='basic'){
    const frame=await runtime.capture({format:'rgba'}),rgb=new Uint8Array(frame.width*frame.height*3);
    for(let i=0;i<frame.width*frame.height;i++)rgb.set(frame.pixels.subarray(i*4,i*4+3),i*3);
    const rgbHash=hash(rgb);
    assert.equal(rgbHash,'313454cc97570419845bd3bc8c9d718bcaa57ddf2914b4f994949e69d97fce49','Public Worker must deliver the actual guest-decoded six-color BMP');
    assert.deepEqual([frame.width,frame.height],[1600,1200]);
    frameProof={width:frame.width,height:frame.height,rgb888Sha256:rgbHash,allWhite:rgb.every(value=>value===255)};
    await writeFile(join(output,'frame.rgb'),rgb);
    await writeFile(join(output,'frame.ppm'),Buffer.concat([Buffer.from(`P6\n${frame.width} ${frame.height}\n255\n`),rgb]));
  }else{
    // The Worker intentionally emits no initial preview as a guest frame.
    assert.equal(state.frame,null);assert.equal(state.stats.frames,0);
    await assert.rejects(runtime.capture({format:'rgba'}),{code:'NO_FRAME'});
  }
  result={schemaVersion:1,mode,status:'passed',execution:'public-runtime-production-worker-wasm-in-node',browserUiVerified:false,
    firmwareSha256:hash(firmwareBytes),cardSha256:hash(card),wasmSha256:hash(await readFile(new URL('web/wasm/esp32sim.wasm',root))),
    frame:frameProof,inputs,stats:state.stats,sdCard:state.sdCard,
    guestSeconds:state.stats?.time,wallSeconds:(Date.now()-started)/1000,checkedAtUTC:new Date().toISOString(),requests,workerMessages};
}catch(error){
  state=await runtime.observe();result={schemaVersion:1,mode,status:'failed',message:error.message,code:error.code,inputs,stats:state.stats,
    guestSeconds:state.stats?.time,lastError:state.lastError,wallSeconds:(Date.now()-started)/1000,checkedAtUTC:new Date().toISOString(),requests,workerMessages};
  process.exitCode=1;
}finally{
  await writeFile(join(output,'console.txt'),JSON.stringify(state?.serialByPort||{},null,2)+'\n');
  await writeFile(join(output,'result.json'),JSON.stringify(result,null,2)+'\n');
  console.log(JSON.stringify(result));await runtime.dispose();
}
