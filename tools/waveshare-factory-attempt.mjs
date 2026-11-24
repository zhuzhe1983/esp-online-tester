#!/usr/bin/env node
// Bounded original vendor firmware (factory or explicit official example) attempt
// through the same runtime/Worker used by the platform.
// Caller supplies the downloaded original; its bytes must match the catalog SHA.
// No vendor binary is copied into the repository or served by the delivery site.
import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {gzipSync} from 'node:zlib';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createSdCardStorage} from '../web/platform/sd-card-storage.js';
import {extractFirmwareZip} from '../web/platform/firmware-archive.js';

const [productId,firmwarePath,outputPath,secondsText='8',firmwareId]=process.argv.slice(2);
assert(productId&&firmwarePath&&outputPath,'Usage: product-id original-vendor.bin output-dir [guest-seconds] [explicit-firmware-id]');
const seconds=Number(secondsText);assert(Number.isFinite(seconds)&&seconds>0&&seconds<=60);
const root=new URL('../',import.meta.url),origin='https://waveshare-attempt.invalid',out=resolve(outputPath);
const sha=b=>createHash('sha256').update(b).digest('hex');await mkdir(out,{recursive:true});
const catalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async n=>
  [n,JSON.parse(await readFile(new URL(`web/platform/catalog/${n}.json`,root))).items])));
const product=catalog.products.find(p=>p.id===productId);assert(product);
const firmware=catalog.firmwares.find(f=>f.id===(firmwareId||product.defaultFirmwareId));assert.ok(['factory','example','official-demo'].includes(firmware?.role));
assert.ok(firmware.productIds.includes(productId),'Original image must explicitly target the selected exact product');
assert.equal(firmware.publisher,'Waveshare');assert.equal(firmware.status,'trial');
const original=await readFile(firmwarePath),binary=firmware.source.archive
  ? await extractFirmwareZip(original,firmware.source,{maxBytes:product.runtime.flashMb*1024*1024}) : original;
assert.equal(sha(binary),firmware.source.sha256);
const requests=[];
const workerFactory=url=>{
  const source=`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(new URL('web'+url.pathname,root)))});parentPort.on('message',data=>globalThis.onmessage({data}));`;
  const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(source)));
  const adapter={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};
  thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',error=>adapter.onerror?.(error));return adapter;
};
const store={indexedDB:null,BroadcastChannel:null,dbName:'waveshare-original-attempt-'+Date.now()};
const runtime=createPlatformRuntime({catalog,workerFactory,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',
  deviceStorage:createDeviceStorage(store),sdStorage:createSdCardStorage(store),fetchImpl:async value=>{
    const url=new URL(value,origin);requests.push(String(url));
    if(String(url)===firmware.source.url)return new Response(original);
    assert.equal(url.origin,origin,'Unexpected dependency');return new Response(await readFile(new URL('web'+url.pathname,root)));
  }});
let state,result;const start=Date.now();
try{
  await runtime.configure(createDefaultScene(catalog,productId));await runtime.loadFirmware(firmware.source);await runtime.start();
  const deadline=Date.now()+240000;
  do{state=await runtime.observe();if(state.status==='error')throw Error(JSON.stringify(state.lastError));
    if((state.stats?.time||0)>=seconds)break;await new Promise(r=>setTimeout(r,20));
  }while(Date.now()<deadline);
  assert((state.stats?.time||0)>=seconds,'Bounded guest duration was not reached');await runtime.pause();state=await runtime.observe();
  let frame=null;
  if(state.frame){const f=await runtime.capture({format:'rgba'}),colors=new Set();
    for(let i=0;i<f.pixels.length;i+=4)colors.add((f.pixels[i]<<16)|(f.pixels[i+1]<<8)|f.pixels[i+2]);
    frame={width:f.width,height:f.height,rgbaSha256:sha(f.pixels),colors:colors.size};
    await writeFile(join(out,'frame.rgba.gz'),gzipSync(f.pixels,{mtime:0}));
  }
  result={status:'bounded-attempt-completed',frame,stats:state.stats,sdCard:state.sdCard};
}catch(error){state=await runtime.observe();result={status:'attempt-blocked',message:error.message,lastError:state.lastError,stats:state.stats,sdCard:state.sdCard};process.exitCode=1;}
finally{
  Object.assign(result,{schemaVersion:1,productId,board:product.runtime.board,firmwareId:firmware.id,
    firmwareUrl:firmware.source.url,firmwareSha256:sha(binary),wasmSha256:sha(await readFile(new URL('web/wasm/esp32sim.wasm',root))),
    firmwareRole:firmware.role,archive:firmware.source.archive||null,downloadSha256:sha(original),
    checkedAt:new Date().toISOString(),wallSeconds:(Date.now()-start)/1000,requestedGuestSeconds:seconds,requests,
    execution:'public-runtime-production-worker-wasm-in-node',simulationVerified:false,browserUiVerified:false,hardwareValidated:false,
    limitations:['This records the bounded original-image execution and observed output only. It does not certify the whole factory application.','No 3D DOM/WebGL, touch interaction, RF, audio or physical hardware acceptance is inferred.']});
  await writeFile(join(out,'result.json'),JSON.stringify(result,null,2)+'\n');
  await writeFile(join(out,'uart.json'),JSON.stringify(state?.serialByPort||{},null,2)+'\n');
  console.log(JSON.stringify(result));await runtime.dispose();
}
