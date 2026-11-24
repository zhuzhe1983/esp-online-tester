#!/usr/bin/env node
// Optional unmodified vendor-firmware execution through the public runtime and
// production Worker/WASM in Node. Native matched-time controls are separate.
import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {Worker} from 'node:worker_threads';
import {gzipSync} from 'node:zlib';
import {resolve,join,basename} from 'node:path';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createSdCardStorage} from '../web/platform/sd-card-storage.js';

const [firmwareDirectory,outputDirectory,...onlyBoards]=process.argv.slice(2);
assert(firmwareDirectory&&outputDirectory,'Usage: node tools/waveshare-tca-rgb-acceptance.mjs <downloaded-factory-root> <output-dir> [board ...]');
const root=new URL('../',import.meta.url),origin='https://waveshare-acceptance.invalid',output=resolve(outputDirectory);
const hash=data=>createHash('sha256').update(data).digest('hex');
const catalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async name=>
  [name,JSON.parse(await readFile(new URL(`web/platform/catalog/${name}.json`,root))).items])));
const selected=catalog.products.filter(p=>p.family?.id==='waveshare-s3-tca-rgb'&&(!onlyBoards.length||onlyBoards.includes(p.runtime.board)));
assert(selected.length>0);await mkdir(output,{recursive:true});
const wasmSha256=hash(await readFile(new URL('web/wasm/esp32sim.wasm',root))),results=[];
for(const product of selected){
  const directory=join(output,product.id);await mkdir(directory,{recursive:true});
  const firmware=catalog.firmwares.find(f=>f.id===product.defaultFirmwareId);
  assert.equal(firmware.role,'factory');assert.equal(firmware.status,'trial');
  const url=new URL(firmware.source.url),repo=url.pathname.split('/')[2];
  const binary=await readFile(join(resolve(firmwareDirectory),repo,'firmware',basename(url.pathname)));
  assert.equal(hash(binary),firmware.source.sha256);
  const requests=[],inputs=[];
  const workerFactory=url=>{
    const source=`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(new URL('web'+url.pathname,root)))});parentPort.on('message',data=>globalThis.onmessage({data}));`;
    const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(source)));
    const adapter={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};
    thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',error=>adapter.onerror?.(error));return adapter;
  };
  const storage={indexedDB:null,BroadcastChannel:null,dbName:product.id+'-public-'+Date.now()};
  const runtime=createPlatformRuntime({catalog,workerFactory,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',
    deviceStorage:createDeviceStorage(storage),sdStorage:createSdCardStorage(storage),fetchImpl:async value=>{
      const requested=new URL(value,origin);requests.push(String(requested));
      if(String(requested)===firmware.source.url)return new Response(binary);
      assert.equal(requested.origin,origin,'No undeclared remote dependencies');
      return new Response(await readFile(new URL('web'+requested.pathname,root)));
    }});
  const started=Date.now(),deadline=started+6*60*1000;
  let state,proof,whole=-1;
  async function time(seconds){
    while(Date.now()<deadline){
      state=await runtime.observe();
      if(state.status==='error')throw Error(JSON.stringify(state.lastError));
      if(Math.floor(state.stats.time||0)!==whole){whole=Math.floor(state.stats.time||0);console.log(JSON.stringify({board:product.runtime.board,guestSeconds:state.stats.time||0,frame:state.frame,wallSeconds:(Date.now()-started)/1000}));}
      if((state.stats.time||0)>=seconds)return state;
      await new Promise(resolve=>setTimeout(resolve,10));
    }
    throw Error('Guest time '+seconds+' not reached before wall deadline');
  }
  async function capture(name){
    const f=await runtime.capture({format:'rgba'});assert.deepEqual([f.width,f.height],[product.display.width,product.display.height]);
    await writeFile(join(directory,name+'.rgba.gz'),gzipSync(f.pixels,{mtime:0}));
    const colors=new Set();for(let i=0;i<f.pixels.length;i+=4)colors.add(f.pixels.slice(i,i+3).join(','));
    return {...f,sha256:hash(f.pixels),colors:colors.size,observedAtGuestSeconds:state.stats.time};
  }
  function diff(a,b){let n=0;for(let i=0;i<a.length;i+=4)if(a[i]!==b[i]||a[i+1]!==b[i+1]||a[i+2]!==b[i+2])n++;return n;}
  try{
    await runtime.configure(createDefaultScene(catalog,product.id));
    assert.equal((await runtime.capabilities()).sdCard.available,true);
    await runtime.loadFirmware(firmware.source);state=await runtime.observe();
    assert.equal(state.sdCard.bytes,64*1024*1024);assert.equal(state.sdCard.mounted,true);assert.equal(state.sdCard.format,'fat32');
    await runtime.start();await time(2.1);const initial=await capture('initial');assert(initial.colors>20);
    await time(2.2);
    if(product.runtime.touch){
      const x=product.display.height===640?440:420,from=product.display.height===640?530:360,to=product.display.height===640?130:120;
      const down=await runtime.input({type:'touch',x,y:from,down:true});inputs.push(down);await time(down.guestTime+.06);
      for(let i=1;i<=20;i++){
        const receipt=await runtime.input({type:'touch',x,y:Math.round(from+(to-from)*i/20),down:true});inputs.push(receipt);await time(receipt.guestTime+.02);
      }
      const up=await runtime.input({type:'touch',x,y:to,down:false});inputs.push(up);await time(up.guestTime+.4);
    }else{
      await assert.rejects(()=>runtime.input({type:'touch',x:120,y:120,down:true}),{code:'UNSUPPORTED_CONTROL'});
      await time(3.2);
    }
    const early=await capture('early-after-input');
    await time(8);await runtime.pause();state=await runtime.observe();const late=await capture('late');
    const uart=state.serialByPort?.uart0||'';
    assert(uart.includes('SD card initialization successful!'),'Original factory application must mount the default FAT32 card');
    assert(uart.includes('65027072'),'Original factory application must report the expected filesystem capacity');
    proof={status:'passed',execution:'public-runtime-production-worker-wasm-in-node',browserUiVerified:false,completeFactoryVerified:false,
      firmwareSha256:hash(binary),wasmSha256,productId:product.id,board:product.runtime.board,
      frame:{width:initial.width,height:initial.height,initialRgbaSha256:initial.sha256,initialColors:initial.colors,
        earlyRgbaSha256:early.sha256,earlyObservedAtGuestSeconds:early.observedAtGuestSeconds,initialToEarlyChangedPixels:diff(initial.pixels,early.pixels),lateRgbaSha256:late.sha256},
      inputs,stats:state.stats,sdCard:state.sdCard,factoryFilesystemMounted:true,
      limitations:['Public input acknowledgments prove delivery; native matched-time control runs establish the early-touch attribution.','The public initial-to-early pixel difference includes ordinary time-dependent changes.','Physical TF presence and attached transport do not by themselves establish a guest filesystem mount.','Later BLE-stage responsiveness remains incomplete.']};
  }catch(error){state=await runtime.observe();proof={status:'failed',productId:product.id,board:product.runtime.board,message:error.message,lastError:state.lastError,stats:state.stats,sdCard:state.sdCard,inputs,wasmSha256,firmwareSha256:hash(binary)};process.exitCode=1;}
  finally{
    Object.assign(proof,{schemaVersion:1,checkedAtUTC:new Date().toISOString(),wallSeconds:(Date.now()-started)/1000,requests});
    await writeFile(join(directory,'result.json'),JSON.stringify(proof,null,2)+'\n');
    await writeFile(join(directory,'uart.json'),JSON.stringify(state?.serialByPort||{},null,2)+'\n');
    results.push(proof);console.log(JSON.stringify({productId:product.id,status:proof.status,message:proof.message,wallSeconds:proof.wallSeconds}));await runtime.dispose();
  }
}
await writeFile(join(output,'results.json'),JSON.stringify(results,null,2)+'\n');
