#!/usr/bin/env node
// Actual original SRAM guest through the public runtime / production Worker.
// Factory firmware and browser/WebGL acceptance are separate boundaries.
import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {Worker} from 'node:worker_threads';
import {gzipSync,gunzipSync} from 'node:zlib';
import {resolve,join} from 'node:path';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createSdCardStorage} from '../web/platform/sd-card-storage.js';

const root=new URL('../',import.meta.url),origin='https://waveshare-acceptance.invalid';
const profiles={touch169:{width:240,suffix:'1-69',height:280,flash:16,sd:false,colors:3360,bootPixels:44928},touch28:{width:240,suffix:'2-8',height:320,flash:32,sd:true,colors:3840,bootPixels:53248},touch35:{suffix:'3-5',width:320,height:480,flash:32,sd:true,colors:7680,bootPixels:119808},lcd273:{suffix:'2-73',width:320,height:320,flash:32,sd:true,colors:5120,bootPixels:73728,touch:false}};
const profile=process.argv[3]||'touch169',config=profiles[profile];assert(config,'Choose touch169, touch28, touch35 or lcd273');
const output=resolve(process.argv[2]||`output/waveshare-c5-${profile}-public`);await mkdir(output,{recursive:true});
const hash=b=>createHash('sha256').update(b).digest('hex');
const catalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async key=>[key,JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items])));
const product=catalog.products.find(p=>p.id===`waveshare-esp32-c5-${config.touch===false?'':'touch-'}lcd-${config.suffix}`);
const fixtures=JSON.parse(await readFile(new URL('tools/fixtures/waveshare-c5-firmwares.json',root))).items;
const firmware=fixtures.find(f=>f.id===`waveshare-c5-${profile}-sram-diagnostic`);
const binary=await readFile(new URL(`web/platform/firmware/waveshare-c5-${profile}-probe.bin`,root));assert.equal(hash(binary),firmware.source.sha256);
const factory=catalog.firmwares.find(f=>f.id===product.defaultFirmwareId);assert.equal(factory.role,'factory');assert.equal(factory.status,'trial');assert.equal(factory.simulationVerified,false);assert.equal(factory.source.bootMode,'rom');
const requests=[];
const workerFactory=url=>{
  const source=`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(new URL('web'+url.pathname,root)))});parentPort.on('message',data=>globalThis.onmessage({data}));`;
  const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(source)));
  const adapter={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};
  thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',error=>adapter.onerror?.(error));return adapter;
};
const storage={indexedDB:null,BroadcastChannel:null,dbName:'waveshare-'+profile+'-'+Date.now()};
const runtime=createPlatformRuntime({catalog,workerFactory,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',deviceStorage:createDeviceStorage(storage),sdStorage:createSdCardStorage(storage),fetchImpl:async value=>{
  const url=new URL(value,origin);assert.equal(url.origin,origin);requests.push(url.pathname);
  assert(!url.pathname.endsWith('_rom.elf'),'SRAM guest must not load an unrelated chip ROM');
  return new Response(await readFile(new URL('web'+url.pathname,root)));
}});
const started=Date.now(),inputs=[],nativeComparisons=[];let state,proof,beforeSd;
const serial=s=>s.serialByPort?.uart0||'';
async function waitFor(label,predicate){
  const deadline=Date.now()+120000;
  while(Date.now()<deadline){state=await runtime.observe();if(state.status==='error')throw Error(JSON.stringify(state.lastError));if(predicate(state))return state;await new Promise(r=>setTimeout(r,15));}
  throw Error(label+' timed out: '+JSON.stringify(state));
}
function diff(a,b){let n=0;for(let i=0;i<a.length;i+=4)if(a[i]!==b[i]||a[i+1]!==b[i+1]||a[i+2]!==b[i+2])n++;return n;}
async function capture(name,predicate=()=>true){
  const deadline=Date.now()+10000;let frame;
  do{frame=await runtime.capture({format:'rgba'});if(predicate(frame))break;await new Promise(r=>setTimeout(r,20));}while(Date.now()<deadline);
  assert(predicate(frame),'Completed '+name+' frame');assert.deepEqual([frame.width,frame.height],[config.width,config.height]);
  const nativeName=name==='touch-release'?'released':name;
  const supplied=process.argv[4];
  const native=supplied?await readFile(join(resolve(supplied),nativeName+'.rgb565le')):gunzipSync(await readFile(new URL(`docs/evidence/waveshare-c5-${profile}-2026-11-23/${nativeName}.rgb565le.gz`,root)));
  assert.equal(native.length,frame.width*frame.height*2);
  const rgba=new Uint8Array(frame.width*frame.height*4);
  for(let i=0,j=0;i<native.length;i+=2,j+=4){const rgb=native.readUInt16LE(i);rgba[j]=Math.round((rgb>>11)*255/31);rgba[j+1]=Math.round(((rgb>>5)&63)*255/63);rgba[j+2]=Math.round((rgb&31)*255/31);rgba[j+3]=255;}
  assert.equal(hash(frame.pixels),hash(rgba),`${profile} ${name}: public frame must equal native RGB565 converted by the production rule`);
  nativeComparisons.push({name,nativeRgb565Sha256:hash(native),convertedRgbaSha256:hash(rgba),equal:true});
  await writeFile(join(output,name+'.rgba.gz'),gzipSync(frame.pixels,{mtime:0}));return frame;
}
try{
  assert.deepEqual([product.runtime.flashMb,product.runtime.psramMb],[config.flash,8]);
  await runtime.configure(createDefaultScene(catalog,product.id));const caps=await runtime.capabilities();
  assert.equal(caps.network.available,false);assert.equal(caps.sdCard.available,config.sd);assert.deepEqual(caps.execution.bootModes,['app-direct']);
  await runtime.loadFirmware(firmware.source);
  if(config.sd){beforeSd=await runtime.exportSdCard();assert.equal(beforeSd.bytes.length,64*1024*1024);}
  await runtime.start();
  await waitFor('PSRAM/display initialization',s=>serial(s).includes(`C5 ${profile.toUpperCase()} READY\n`)&&s.frame);
  const at=state.stats.time;await waitFor('published completed frame',s=>s.stats.time>=at+.03);
  const ready=await capture('ready');const colors=new Set();for(let i=0;i<ready.pixels.length;i+=4)colors.add(ready.pixels.slice(i,i+3).join(','));assert.equal(colors.size,config.colors);
  await waitFor('quiet control',s=>s.stats.time>=at+.06);assert.equal(diff(ready.pixels,(await capture('quiet')).pixels),0);
  const touchSteps=[
    {name:'touch',input:{type:'touch',x:80,y:110,down:true},marker:'C5 TOUCH DOWN 0050 006E\n',changed:1024},
    {name:'touch-release',input:{type:'touch',x:80,y:110,down:false},marker:'C5 TOUCH UP 0050 006E\n',changed:0},
  ];
  const keySteps=['prev','next'].flatMap(id=>[
    {name:id,input:{type:'button',id,pressed:true},marker:`C5 ${id.toUpperCase()} DOWN\n`,changed:26752},
    {name:id+'-release',input:{type:'button',id,pressed:false},marker:`C5 ${id.toUpperCase()} UP\n`,changed:0},
  ]);
  if(config.touch===false)await assert.rejects(runtime.input({type:'touch',x:80,y:110,down:true}));
  for(const step of [
    ...(config.touch===false?keySteps:touchSteps),
    {name:'boot',input:{type:'button',id:'boot',pressed:true},marker:'C5 BOOT DOWN\n',changed:config.bootPixels},
    {name:'boot-release',input:{type:'button',id:'boot',pressed:false},marker:'C5 BOOT UP\n',changed:0},
  ]){
    const offset=serial(state).length;const receipt=await runtime.input(step.input);inputs.push(receipt);
    await waitFor(step.name,s=>serial(s).slice(offset).includes(step.marker));
    await capture(step.name,f=>diff(ready.pixels,f.pixels)===step.changed);
  }
  await runtime.pause();state=await runtime.observe();
  assert(serial(state).includes('PSRAM'),'Guest must report its PSRAM test');
  let sd;
  if(config.sd){const after=await runtime.exportSdCard();assert.equal(hash(after.bytes),hash(beforeSd.bytes),'The entire default FAT32 card must be restored');assert(serial(state).includes('C5 SD ORIGINAL BLOCK RESTORED\n'));sd={bytes:beforeSd.bytes.length,format:state.sdCard.format,beforeSha256:hash(beforeSd.bytes),afterSha256:hash(after.bytes),originalImageRestoredExactly:true,guestFilesystemMounted:false};}
  proof={status:'passed',execution:'public-runtime-production-worker-wasm-in-node',browserUiVerified:false,factoryVerified:false,productId:product.id,flashMb:config.flash,psramMb:8,capabilities:caps,
    frame:{width:config.width,height:config.height,rgbaSha256:hash(ready.pixels),colors:colors.size,quietChangedPixels:0,...(config.touch===false?{prevChangedPixels:26752,nextChangedPixels:26752}:{touchChangedPixels:1024}),bootChangedPixels:config.bootPixels,releaseRestoredExactly:true},inputs,nativeComparisons,stats:state.stats,sdCard:state.sdCard,sd,uart:serial(state)};
}catch(error){state=await runtime.observe();proof={status:'failed',message:error.message,code:error.code,lastError:state.lastError,stats:state.stats,uart:serial(state),inputs};process.exitCode=1;}
finally{
  Object.assign(proof,{schemaVersion:1,checkedAtUTC:new Date().toISOString(),firmwareSha256:hash(binary),wasmSha256:hash(await readFile(new URL('web/wasm/esp32sim.wasm',root))),wallSeconds:(Date.now()-started)/1000,requests});
  await writeFile(join(output,'result.json'),JSON.stringify(proof,null,2)+'\n');console.log(JSON.stringify({status:proof.status,message:proof.message,frame:proof.frame,wallSeconds:proof.wallSeconds}));await runtime.dispose();
}
