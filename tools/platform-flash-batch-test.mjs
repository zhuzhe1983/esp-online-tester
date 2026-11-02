#!/usr/bin/env node
// Injected serial protocol only: no chooser, USB device, or physical write.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {createDefaultScene} from '../web/platform/catalog.js';
import {withPlatformTestFixtures} from './fixtures/platform-catalog.mjs';
import {prepareFlashPlan,createFlasher,prepareFlashSession,chooseFlashPort,activateFlashSession,releaseFlashSession} from '../web/platform/flasher.js';
import {createFlashBatch} from '../web/platform/flash-batch.js';
import {createFlashHardReset,resetFlashedDevice} from '../web/platform/flash-reset.js';
import {createFlashTransport,waitForFlashClose} from '../web/platform/flash-transport.js';
import {ESPLoader,Transport as SerialTransport} from '../web/platform/vendor/esptool-js-0.7.0/bundle.js';

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
    async disconnect(){
      if(this.port.closeFailure)throw Error('Port close failed');
      if(this.port.holdClose)await new Promise(resolve=>{this.port.finishClose=resolve;});
      calls.disconnects++;calls.active--;this.port.closed=true;
    }
    async setDTR(value){(this.port.signals ||= []).push(['DTR',value]);}
    async setRTS(value){(this.port.signals ||= []).push(['RTS',value]);}
  }
  class Loader {
    constructor(options){this.transport=options.transport;this.port=this.transport.port;this.chip={CHIP_NAME:this.port.chip || 'ESP32-S3'};this.reset=options.resetConstructors.hardReset;}
    async main(){calls.connects++;calls.active++;calls.maxActive=Math.max(calls.maxActive,calls.active);}
    async getSecurityInfo(){return {flashCryptCnt:0,parsedFlags:{SECURE_BOOT_EN:!!this.port.secured,SECURE_DOWNLOAD_ENABLE:false}};}
    async detectFlashSize(){return this.port.size || '16MB';}
    flashSizeBytes(size){return parseInt(size)*1024*1024;}
    async readReg(address){return address===0x60004038?8:0;}
    async writeReg(){assert.equal(this.port.verified,true,'Registers must only be written after firmware verification');}
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
      this.port.verified=true;
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

for(const scenario of [
  {chip:'ESP32-S3',usb:true,strap:0,option:0,watchdog:true},
  {chip:'ESP32-S3',usb:false,detected:true,strap:0,option:0,watchdog:true},
  {chip:'ESP32-S3',usb:true,strap:8,option:0,watchdog:false},
  {chip:'ESP32-S3',usb:true,strap:0,option:1,watchdog:false},
  {chip:'ESP32-S3',usb:false,strap:0,option:0,watchdog:false},
  {chip:'ESP32-C3',usb:true,strap:0,option:0,watchdog:false},
  {chip:'ESP32-C6',usb:true,strap:0,option:0,watchdog:false},
]) {
  const events=[],loader={chip:{CHIP_NAME:scenario.chip,usesUsbJtagSerial:async()=>!!scenario.detected},transport:{setDTR:async value=>events.push(['DTR',value])},
    readReg:async address=>{events.push(['read',address]);return address===0x60004038?scenario.strap:scenario.option;},
    writeReg:async(...args)=>events.push(['write',...args]),after:async mode=>events.push(['after',mode])};
  await resetFlashedDevice(loader,{getInfo:()=>scenario.usb?{usbVendorId:0x303a,usbProductId:0x1001}:{usbVendorId:0x10c4,usbProductId:0xea60}},{wait:async ms=>events.push(['wait',ms])});
  const confirmed=scenario.chip==='ESP32-S3' && (scenario.usb || scenario.detected);
  const prefix=confirmed?[['write',0x6000812c,0,1],['read',0x60004038],['read',0x6000812c]]:[];
  assert.deepEqual(events,scenario.watchdog?[...prefix,['DTR',false],['write',0x600080b0,0x50d83aa1],['write',0x6000809c,2000],['write',0x60008098,0xd0000102],['write',0x600080b0,0],['wait',500]]:[...prefix,['after','hard_reset']]);
}
console.log('ok S3 native-USB manual-download watchdog reset, exact register order/masks and chip/USB/strap gates');

{
  const failure=Error('USB disconnected'),frames=[];let closed=0;
  const port={readable:null,writable:new WritableStream({write(bytes){frames.push([...bytes]);throw failure;}}),async close(){closed++;this.writable=null;}};
  const transport=createFlashTransport(SerialTransport,port);
  await assert.rejects(()=>transport.write(new Uint8Array([0xc0,0xdb,0x01])),failure);
  assert.equal(port.writable.locked,false,'Rejected Web Serial writes must release their writer');
  assert.deepEqual(frames,[[0xc0,0xdb,0xdc,0xdb,0xdd,0x01,0xc0]]);
  await waitForFlashClose(transport.disconnect(),50);assert.equal(closed,1);
  let rejectLate;const late=new Promise((_,reject)=>{rejectLate=reject;});
  await assert.rejects(()=>waitForFlashClose(late,1),{code:'SERIAL_CLOSE_FAILED'});rejectLate(failure);await tick();
}
console.log('ok real vendor transport framing, rejected writer release, bounded close and handled late rejection');

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
  const f=fixture();let release;
  const batch=createFlashBatch({...f.dependencies,loadWatchlistData:()=>new Promise(resolve=>{release=()=>resolve({entries:[]});})});
  const preparing=batch.prepare({scene,catalog,source:source()});batch.stop();release();await assert.rejects(()=>preparing);
  assert.equal(batch.observe().status,'stopped');assert.equal(f.calls.connects,0);
  const again=batch.prepare({scene,catalog,source:source()});release();await again;assert.equal(batch.observe().status,'ready');
  await batch.dispose();await f.batch.dispose();
}
console.log('ok stopping preparation ignores late results and permits a clean next preparation');

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
  const f=fixture();await f.batch.prepare({scene,catalog,source:source()});
  const wrong=f.port('wrong',{chip:'ESP32-S31'}),right=f.port('right'),first=await f.choose(wrong);
  f.click(()=>f.batch.start());await until(()=>f.batch.observe().status==='done');
  const failed=f.batch.observe(),receipt=failed.receipt;
  assert.equal(failed.items[0].error.code,'CHIP_MISMATCH');assert.match(failed.items[0].message,/ESP32-S31.*ESP32-S3.*未写入 Flash.*更换设备/);
  assert.equal(f.calls.writes,0);
  await assert.rejects(()=>f.batch.replaceDevice(first.id),{code:'USER_GESTURE_REQUIRED'});
  f.selected.push(new DOMException('Cancelled','NotFoundError'));
  await assert.rejects(()=>f.click(()=>f.batch.replaceDevice(first.id)),{name:'NotFoundError'});
  assert.deepEqual(f.batch.observe(),failed,'Cancelled selection preserves the failed row and firmware');
  f.selected.push(wrong);await assert.rejects(()=>f.click(()=>f.batch.replaceDevice(first.id)),{code:'FLASH_SAME_DEVICE'});
  assert.deepEqual(f.batch.observe(),failed);
  f.selected.push(right);await f.click(()=>f.batch.replaceDevice(first.id));
  let state=f.batch.observe();assert.equal(state.items.length,1);assert.equal(state.items[0].id,first.id);assert.equal(state.items[0].status,'ready');assert.equal(state.counts.queued,1);
  assert.equal(state.items[0].error,null);assert.match(state.items[0].logs.join('\n'),/ESP32-S31/);assert.deepEqual(state.receipt,receipt);
  f.notify('disconnect',wrong);assert.equal(f.batch.observe().items[0].connected,true,'Old port no longer controls this row');
  f.notify('connect',right);await tick();assert.equal(f.calls.connects,1,'Replacement never opens or writes the port');
  assert.throws(()=>f.batch.retry(first.id),{code:'USER_GESTURE_REQUIRED'});
  f.click(()=>f.batch.retry(first.id));await until(()=>f.calls.writes===1);right.release();await until(()=>f.batch.observe().status==='done');
  assert.equal(f.batch.observe().counts.done,1);assert.equal(f.calls.prepares,1);assert.equal(f.calls.disconnects,2);
  assert.deepEqual(f.calls.hashes[0],originalPlan.files.map(file=>createHash('sha256').update(file.data).digest('hex')));
  await assert.rejects(()=>f.click(()=>f.batch.replaceDevice(first.id)),{code:'FLASH_BUSY'});
  await f.batch.dispose();
}
console.log('ok wrong-chip replacement, cancelled/same-port chooser, pinned bytes, detached old port, explicit write authorization');

{
  const f=fixture();await f.batch.prepare({scene,catalog,source:source()});
  const a=f.port('active'),b=f.port('queued'),c=f.port('replacement'),first=await f.choose(a),second=await f.choose(b);
  f.selected.push(a);await assert.rejects(()=>f.click(()=>f.batch.replaceDevice(second.id)),{code:'FLASH_DUPLICATE_DEVICE'});
  f.click(()=>f.batch.start({concurrency:1}));await until(()=>f.calls.writes===1);
  await assert.rejects(()=>f.click(()=>f.batch.replaceDevice(first.id)),{code:'FLASH_BUSY'});
  let resolveChooser;f.navigator.serial.requestPort=()=>new Promise(resolve=>{resolveChooser=resolve;});
  const replacing=f.click(()=>f.batch.replaceDevice(second.id));a.release();await until(()=>f.batch.observe().counts.active===0);
  assert.equal(f.calls.writes,1,'Queued old port cannot launch while replacement chooser is pending');
  resolveChooser(c);await replacing;await tick();assert.equal(f.calls.writes,1);assert.equal(f.batch.observe().status,'ready');
  f.click(()=>f.batch.start({concurrency:1}));await until(()=>f.calls.writes===2);c.release();await until(()=>f.batch.observe().status==='done');
  assert.equal(b.release,undefined);assert.equal(f.batch.observe().counts.done,2);await f.batch.dispose();
}
console.log('ok duplicate/active-port guards and queued replacement paused across parallel job completion');

for(const end of ['stop','dispose']) {
  const f=fixture();await f.batch.prepare({scene,catalog,source:source()});const first=await f.choose(f.port('original'));
  let resolveChooser;f.navigator.serial.requestPort=()=>new Promise(resolve=>{resolveChooser=resolve;});
  const replacing=f.click(()=>f.batch.replaceDevice(first.id));await f.batch[end]();resolveChooser(f.port('late'));
  await assert.rejects(()=>replacing,{code:end==='dispose'?'FLASH_SESSION_INVALID':'FLASH_CANCELLED'});
  assert.equal(f.calls.connects,0);assert.equal(f.batch.observe().items[0].status,'cancelled');await f.batch.dispose();
}
console.log('ok stop/dispose discard late replacement selections without opening any port');

for(const extra of [{chip:'ESP32-S31'},{size:'8MB'},{secured:true}]) {
  const f=fixture();await f.batch.prepare({scene,catalog,source:source()});f.click(()=>f.batch.start({mode:'sequential'}));
  const first=await f.choose(f.port('wrong',{chip:'ESP32-C3'}));await until(()=>f.batch.observe().counts.active===0);
  f.selected.push(f.port('replacement',extra));await f.click(()=>f.batch.replaceDevice(first.id));
  await tick();assert.equal(f.calls.connects,1,'Continuous mode does not automatically write replacements');
  f.click(()=>f.batch.retry(first.id));await until(()=>f.batch.observe().counts.active===0);
  assert.equal(f.calls.connects,2);assert.equal(f.calls.writes,0);assert.equal(f.batch.observe().counts.error,1);await f.batch.dispose();
}
console.log('ok continuous replacement requires a click and repeats chip/capacity/security checks');

{
  const f=fixture();await f.batch.prepare({scene,catalog,source:source()});const p=f.port('close-failure',{closeFailure:true});await f.choose(p);f.click(()=>f.batch.start());
  await until(()=>f.calls.writes===1);p.release();await until(()=>f.batch.observe().status==='done');
  const item=f.batch.observe().items[0];assert.equal(item.status,'error');assert.equal(item.error.code,'SERIAL_CLOSE_FAILED');assert.equal(item.flashVerified,true);assert.equal(item.mayBePartial,false);
  assert.match(item.message,/已写入并通过校验/);assert.equal(f.calls.disconnects,0);
  f.click(()=>f.batch.retry(item.id));await until(()=>f.batch.observe().status==='done');assert.equal(f.batch.observe().items[0].error.code,'FLASH_PORT_BUSY');assert.equal(f.calls.writes,1);await f.batch.dispose();
}
console.log('ok close failure stays visible as verified-write/close-error and keeps the port locked');

{
  const f=fixture(),prepared=await prepareFlashSession({scene,catalog,source:source()},f.dependencies),p=f.port('late-close',{holdClose:true});
  f.selected.push(p);await f.click(()=>chooseFlashPort(prepared,f.dependencies));f.click(()=>activateFlashSession(prepared,true,f.dependencies));
  const flasher=createFlasher(f.dependencies),writing=flasher.flash({prepared,port:p});
  const rejected=assert.rejects(()=>writing,{code:'SERIAL_CLOSE_FAILED'});
  await until(()=>f.calls.writes===1);p.release();await rejected;
  assert.equal(flasher.observe().flashVerified,true);assert.equal(flasher.observe().mayBePartial,false);
  await assert.rejects(()=>createFlasher(f.dependencies).flash({prepared,port:p}),{code:'FLASH_PORT_BUSY'});
  p.holdClose=false;p.finishClose();await until(()=>f.calls.disconnects===1);
  const next=createFlasher(f.dependencies).flash({prepared,port:p});await until(()=>f.calls.writes===2);p.release();await next;
  assert.equal(f.calls.disconnects,2);releaseFlashSession(prepared);await f.batch.dispose();
}
console.log('ok timed-out cleanup retains the busy port until actual late closure completes');

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
