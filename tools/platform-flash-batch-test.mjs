#!/usr/bin/env node
// Injected serial protocol only: no chooser, USB device, or physical write.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {createDefaultScene} from '../web/platform/catalog.js';
import {withPlatformTestFixtures} from './fixtures/platform-catalog.mjs';
import {prepareFlashPlan,createFlasher,prepareFlashSession,chooseFlashPort,activateFlashSession,releaseFlashSession} from '../web/platform/flasher.js';
import {createFlashBatch} from '../web/platform/flash-batch.js';
import {createFlashHardReset} from '../web/platform/flash-reset.js';
import {ESPLoader} from '../web/platform/vendor/esptool-js-0.7.0/bundle.js';

const root=new URL('../',import.meta.url),baseUrl='https://flash.example/platform/index.html',raw={};
for(const name of ['products','accessories','firmwares'])raw[name]=JSON.parse(await readFile(new URL(`web/platform/catalog/${name}.json`,root))).items;
const catalog=await withPlatformTestFixtures(raw,baseUrl),scene=createDefaultScene(catalog,'m5stack-stopwatch');
scene.firmware={manifestUrl:'https://flash.example/wasm/fw/demo-stopwatch.json'};
const fetchImpl=async url=>new Response(await readFile(new URL(`web${new URL(url).pathname}`,root)));
const originalPlan=await prepareFlashPlan({scene,catalog,fetchImpl,baseUrl});
const source=()=>({format:'segments',files:originalPlan.files.map((entry,index)=>({file:new File([entry.data],`${index}.bin`),offset:entry.address}))});
const tick=()=>new Promise(resolve=>setTimeout(resolve,0));
async function until(test) {for(let i=0;i<100 && !test();i++)await tick();assert.ok(test(),'bounded async condition');}

function fixture() {
  const serial=new EventTarget(),selected=[],calls={chooser:0,prepares:0,connects:0,writes:0,active:0,maxActive:0,disconnects:0,hashes:[]};
  const navigator={serial,userActivation:{isActive:false}};
  serial.requestPort=()=>{calls.chooser++;assert.equal(navigator.userActivation.isActive,true);const next=selected.shift();return next instanceof Error?Promise.reject(next):Promise.resolve(next);};
  serial.getPorts=()=>{throw Error('Unrelated authorized ports must never be enumerated.');};
  const notify=(type,port)=>{const event=new Event(type);event.port=port;serial.dispatchEvent(event);};
  const port=(name,extra={})=>({name,getInfo:()=>({usbVendorId:0x303a,usbProductId:0x1001}),...extra});
  class Transport {
    constructor(port) {this.port=port;}
    async disconnect(){calls.disconnects++;calls.active--;this.port.closed=true;}
    async setDTR(value){(this.port.signals ||= []).push(['DTR',value]);}
    async setRTS(value){(this.port.signals ||= []).push(['RTS',value]);}
  }
  class Loader {
    constructor(options){this.transport=options.transport;this.port=this.transport.port;this.chip={CHIP_NAME:this.port.chip || 'ESP32-S3'};this.reset=options.resetConstructors.hardReset;}
    async main(){calls.connects++;calls.active++;calls.maxActive=Math.max(calls.maxActive,calls.active);}
    async getSecurityInfo(){return {flashCryptCnt:0,parsedFlags:{SECURE_BOOT_EN:!!this.port.secured,SECURE_DOWNLOAD_ENABLE:false}};}
    async detectFlashSize(){return this.port.size || '16MB';}
    flashSizeBytes(size){return parseInt(size)*1024*1024;}
    async writeFlash(options){
      calls.writes++;assert.equal(options.eraseAll,false);assert.equal(options.flashMode,'keep');assert.equal(options.flashFreq,'keep');assert.equal(options.flashSize,'keep');
      calls.hashes.push(options.fileArray.map(file=>createHash('sha256').update(file.data).digest('hex')));
      for(const file of options.fileArray)assert.equal(options.calculateMD5Hash(file.data),createHash('md5').update(file.data).digest('hex'));
      options.reportProgress(0,1,2);
      await new Promise(resolve=>{this.port.release=resolve;});
      if(this.port.fail){this.port.fail=false;throw Object.assign(Error('Protocol failed'),{code:'TEST_PROTOCOL_ERROR'});}
      options.reportProgress(options.fileArray.length-1,2,2);
      // esptool is allowed to work on its local copy; no next device may inherit it.
      options.fileArray[0].data[0]=0;
    }
    async after(mode){assert.equal(mode,'hard_reset');await this.reset(this.transport,false,async()=>{}).reset();}
  }
  const dependencies={navigator,secureContext:true,baseUrl,fetchImpl,loadWatchlistData:async()=>{calls.prepares++;return {entries:[]};},loadTools:async()=>({Transport,ESPLoader:Loader,md5:data=>createHash('md5').update(data).digest('hex')})};
  const batch=createFlashBatch(dependencies);
  const click=fn=>{navigator.userActivation.isActive=true;try{return fn();}finally{navigator.userActivation.isActive=false;}};
  const choose=async value=>{selected.push(value);return click(()=>batch.addDevice());};
  return {batch,calls,port,notify,choose,click,dependencies,selected,navigator};
}

// The real vendor loader must invoke our constructor, not only a fake after().
for(const usb of [false,true]) {
  const signals=[],transport={getInfo:()=>'',setDTR:async value=>signals.push(['DTR',value]),setRTS:async value=>signals.push(['RTS',value])};
  const loader=new ESPLoader({transport,baudrate:115200,terminal:{clean(){},writeLine(){}},resetConstructors:{hardReset:(transport,usingUsb)=>createFlashHardReset(transport,usingUsb,async ms=>signals.push(['wait',ms]))}});
  await loader.after('hard_reset',usb);
  assert.deepEqual(signals,usb?[['DTR',false],['RTS',true],['wait',200],['RTS',false],['wait',200]]:[['DTR',false],['RTS',true],['wait',100],['RTS',false]]);
}
console.log('ok real vendored loader uses asserted RTS reset; USB-OTG timing and released boot pin');

{
  const f=fixture(),input={scene,catalog,source:source()};await f.batch.prepare(input);
  input.source.files[0].offset=0x4000;input.scene={productId:'wrong'};
  const receipt=f.batch.observe().receipt;receipt.artifacts[0].sha256='wrong';
  const a=f.port('a'),b=f.port('b',{fail:true}),c=f.port('c');
  const first=await f.choose(a),again=await f.choose(a);assert.equal(first.id,again.id);assert.equal(f.batch.observe().items.length,1);
  await f.choose(b);await f.choose(c);
  assert.throws(()=>f.batch.start(),{code:'USER_GESTURE_REQUIRED'});
  f.click(()=>f.batch.start({mode:'parallel',concurrency:2}));
  await until(()=>f.calls.writes===2);assert.equal(f.calls.maxActive,2);
  a.release();b.release();await until(()=>f.calls.writes===3);c.release();await until(()=>f.batch.observe().status==='done');
  let state=f.batch.observe();assert.deepEqual(state.counts,{total:3,queued:0,active:0,done:2,error:1,cancelled:0});
  assert.equal(f.calls.prepares,1);assert.equal(f.calls.disconnects,3);
  for(const hashes of f.calls.hashes)assert.deepEqual(hashes,originalPlan.files.map(file=>createHash('sha256').update(file.data).digest('hex')));
  assert.notEqual(state.receipt.artifacts[0].sha256,'wrong');
  const failed=state.items.find(item=>item.status==='error');
  assert.throws(()=>f.batch.retry(failed.id),{code:'USER_GESTURE_REQUIRED'});
  f.click(()=>f.batch.retry(failed.id));await until(()=>f.calls.writes===4);b.release();await until(()=>f.batch.observe().status==='done');
  state=f.batch.observe();assert.equal(state.counts.done,3);assert.equal(state.items.find(item=>item.id===failed.id).attempt,2);
  assert.deepEqual(a.signals,[['DTR',false],['RTS',true],['RTS',false]]);
  await f.batch.dispose();
}
console.log('ok bounded parallel writes, immutable firmware, unique ports, independent failure/retry, per-device reset/cleanup');

{
  const f=fixture();await f.batch.prepare({scene,catalog,source:source()});f.click(()=>f.batch.start({mode:'sequential'}));
  const ignored=f.port('unrelated');f.notify('connect',ignored);await tick();assert.equal(f.calls.writes,0);
  const a=f.port('a'),b=f.port('b'),first=await f.choose(a);await until(()=>f.calls.writes===1);
  await f.choose(a);assert.equal(f.batch.observe().items.length,1);
  await f.choose(b);assert.equal(f.calls.writes,1);a.release();await until(()=>f.calls.writes===2);b.release();await until(()=>f.batch.observe().counts.done===2);
  assert.equal(f.batch.observe().running,true);assert.equal(f.calls.maxActive,1);
  f.notify('disconnect',a);f.notify('connect',a);await f.choose(a);await tick();assert.equal(f.calls.writes,2,'A completed device must not be flashed again on reconnect or repeated selection');
  assert.throws(()=>f.batch.retry(first.id),{code:'FLASH_BUSY'});
  const c=f.port('c'),third=await f.choose(c);await until(()=>f.calls.writes===3);f.notify('disconnect',c);c.release();await until(()=>f.batch.observe().counts.active===0);
  assert.equal(f.batch.observe().items.find(item=>item.id===third.id).error.code,'DEVICE_DISCONNECTED');
  assert.throws(()=>f.click(()=>f.batch.retry(third.id)),{code:'DEVICE_DISCONNECTED'});
  f.notify('connect',c);await tick();assert.equal(f.calls.writes,3,'Reconnect only updates status; retry requires a click');
  f.click(()=>f.batch.retry(third.id));await until(()=>f.calls.writes===4);c.release();await until(()=>f.batch.observe().counts.done===3);
  f.batch.stop();assert.equal(f.batch.observe().status,'stopped');assert.equal(f.calls.prepares,1);
  await f.batch.dispose();
}
console.log('ok continuous sequential session, explicit new device authorization, no automatic repeat, disconnected device retry');

{
  const f=fixture();await f.batch.prepare({scene,catalog,source:source()});
  await assert.rejects(()=>f.choose(new DOMException('No device chosen','NotFoundError')),{name:'NotFoundError'});
  assert.equal(f.batch.observe().choosing,false);assert.equal(f.batch.observe().items.length,0);
  const a=f.port('a'),b=f.port('b');const first=await f.choose(a);await f.choose(b);f.click(()=>f.batch.start({concurrency:1}));await until(()=>f.calls.writes===1);
  f.batch.cancel(first.id);a.release();await until(()=>f.calls.writes===2);f.batch.stop();b.release();await until(()=>f.batch.observe().status==='stopped');
  assert.equal(f.batch.observe().counts.cancelled,2);assert.equal(f.calls.disconnects,2);assert.equal(a.signals,undefined,'Never reset after a cancelled write');
  await f.batch.dispose();
}
console.log('ok chooser cancellation, isolated active cancellation, stop queue, no reset after interrupted write');

{
  const f=fixture();await f.batch.prepare({scene,catalog,source:source()});const a=f.port('a'),b=f.port('b');await f.choose(a);await f.choose(b);
  f.click(()=>f.batch.start({concurrency:1}));await until(()=>f.calls.writes===1);const disposed=f.batch.dispose();a.release();await disposed;
  assert.equal(f.calls.writes,1);assert.equal(f.calls.disconnects,1);f.notify('connect',b);await tick();assert.equal(f.calls.writes,1);
  await assert.rejects(()=>f.batch.addDevice(),{code:'FLASH_SESSION_CLOSED'});
}
console.log('ok disposing cancels active write, clears queued work and removes connection listeners');

{
  const f=fixture();await f.batch.prepare({scene,catalog,source:source()});
  let resolveChooser;f.navigator.serial.requestPort=()=>new Promise(resolve=>{resolveChooser=resolve;});
  const selecting=f.click(()=>f.batch.addDevice());f.batch.stop();resolveChooser(f.port('late'));
  await assert.rejects(()=>selecting,{code:'FLASH_CANCELLED'});assert.equal(f.batch.observe().items.length,0);assert.equal(f.calls.connects,0);
  await f.batch.dispose();
}
console.log('ok stopping an outstanding chooser cannot enqueue a late selection');

{
  const f=fixture();await f.batch.prepare({scene,catalog,source:source()});
  const a=f.port('a'),b=f.port('b');await f.choose(a);const second=await f.choose(b);f.click(()=>f.batch.start({concurrency:1}));await until(()=>f.calls.writes===1);
  f.notify('disconnect',b);a.release();await until(()=>f.batch.observe().status==='done');
  assert.equal(f.calls.writes,1);assert.equal(f.batch.observe().items.find(item=>item.id===second.id).status,'error');await f.batch.dispose();
}
console.log('ok a queued unplugged device is not silently retried or allowed to stall completion');

for(const extra of [{chip:'ESP32-C3'},{size:'8MB'},{secured:true}]) {
  const f=fixture();await f.batch.prepare({scene,catalog,source:source()});await f.choose(f.port('incompatible',extra));f.click(()=>f.batch.start());
  await until(()=>f.batch.observe().status==='done');assert.equal(f.calls.writes,0);assert.equal(f.calls.disconnects,1);assert.equal(f.batch.observe().counts.error,1);await f.batch.dispose();
}
console.log('ok every batch device independently checks chip, Flash capacity and security before writing');

{
  const f=fixture(),prepared=await prepareFlashSession({scene,catalog,source:source()},f.dependencies),p=f.port('token');
  const flasher=createFlasher(f.dependencies);
  await assert.rejects(()=>flasher.flash({prepared,port:p}),{code:'FLASH_SESSION_INVALID'});
  f.click(()=>activateFlashSession(prepared,true,f.dependencies));
  await assert.rejects(()=>flasher.flash({prepared,port:p}),{code:'FLASH_SESSION_INVALID'});
  f.selected.push(p);await f.click(()=>chooseFlashPort(prepared,f.dependencies));
  const writing=flasher.flash({prepared,port:p});await until(()=>f.calls.writes===1);
  await assert.rejects(()=>createFlasher(f.dependencies).flash({prepared,port:p}),{code:'FLASH_PORT_BUSY'});
  p.release();await writing;releaseFlashSession(prepared);assert.equal(f.calls.disconnects,1);await f.batch.dispose();
}
console.log('ok opaque session guards, selected port authorization, cross-controller port lock');
