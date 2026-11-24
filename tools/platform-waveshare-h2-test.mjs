// Public runtime -> production Worker/WASM -> independent H2 RV32 core/MMIO.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {Worker} from 'node:worker_threads';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {validateAppImage,chipForBoard,chipIdForBoard,H2_SRAM_RANGES} from '../web/platform/app-image.js';
import {waveshareH2CoreProbe} from './fixtures/waveshare-h2-core-probe.mjs';
const root=new URL('../',import.meta.url),origin='https://waveshare-h2.invalid';
const catalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async n=>[n,JSON.parse(await readFile(new URL(`web/platform/catalog/${n}.json`,root))).items])));
const workerFactory=url=>{
  const source=`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(new URL('web'+url.pathname,root)))});parentPort.on('message',data=>globalThis.onmessage({data}));`;
  const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(source)));
  const adapter={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};
  thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;
};
async function until(runtime,predicate){
  const deadline=Date.now()+30000;
  do{const s=await runtime.observe();if(predicate(s))return s;if(s.status==='error')throw Error(JSON.stringify(s.lastError));await new Promise(r=>setTimeout(r,15));}while(Date.now()<deadline);
  throw Error('H2 guest timeout: '+JSON.stringify(await runtime.observe()));
}
test('four exact H2 sale options execute actual H2 RMT/UART/BOOT and reload with no substituted ROM',async()=>{
  const requests=[];
  const runtime=createPlatformRuntime({catalog,workerFactory,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',fetchImpl:async value=>{
    const url=new URL(value);assert.equal(url.origin,origin);requests.push(url.pathname);
    return new Response(await readFile(new URL('web'+url.pathname,root)));
  }});
  try{
    for(const id of ['waveshare-esp32-h2-zero','waveshare-esp32-h2-zero-m','waveshare-esp32-h2-dev-kit-n4','waveshare-esp32-h2-dev-kit-n4-m']){
      const scene=createDefaultScene(catalog,id);scene.firmware=null;await runtime.configure(scene);
      const caps=await runtime.capabilities();assert.deepEqual(caps.execution.bootModes,['app-direct']);
      assert.deepEqual(caps.execution.sramRanges,H2_SRAM_RANGES);assert.equal(caps.firmware.targetRevision,null);
      for(const available of [caps.network.available,caps.ble.available,caps.sdCard.available,caps.deviceStorage.available,caps.audio.output.available])assert.equal(available,false);
      const file=new File([waveshareH2CoreProbe()],'authored-h2-sram-diagnostic.bin');
      await assert.rejects(runtime.loadFirmware({file,bootMode:'rom'}),e=>e.code==='H2_SRAM_REQUIRED');
      await runtime.loadFirmware({file});await runtime.start();
      // UART/RMT output and periodic counters arrive as independent observations.
      const ready=await until(runtime,s=>s.stats.insns>0&&s.serialByPort?.uart0?.includes('BOOT UP\n')&&JSON.stringify(s.leds)===JSON.stringify([[0x34,0x12,0x56]]));
      assert.equal(ready.frame,null);assert(ready.stats.insns>0,id+' executes guest instructions');
      for(const [pressed,marker] of [[true,'BOOT DOWN\n'],[false,'BOOT UP\n']]){
        const before=(await runtime.observe()).serialByPort.uart0.length;
        await runtime.input({type:'button',id:'boot',pressed});
        await until(runtime,s=>s.serialByPort?.uart0?.slice(before).includes(marker));
      }
      await runtime.reset();await until(runtime,s=>s.serialByPort?.uart0?.startsWith('CORE H2 SRAM READY\n')&&JSON.stringify(s.leds)===JSON.stringify([[0x34,0x12,0x56]]));await runtime.pause();
    }
    assert(requests.every(path=>path.endsWith('esp32sim.wasm')),'H2 does not fetch C6 or any other ROM');
  }finally{await runtime.dispose();}
});
test('H2 image boundary rejects other chips, unknown wafer restrictions, mapped Flash and wrong capacity before execution',async()=>{
  for(const board of ['waveshare-h2-zero','waveshare-h2-dev-kit-n4']){
    assert.equal(chipForBoard(board),'esp32h2');assert.equal(chipIdForBoard(board),16);
    const valid=waveshareH2CoreProbe();assert.equal((await validateAppImage(valid,board,4<<20)).entry,0x40800000);
    for(const [patch,expected] of [
      [b=>b.writeUInt16LE(13,12),'CHIP_MISMATCH'],
      [b=>b.writeUInt16LE(100,15),'CHIP_REVISION_UNSUPPORTED'],
      [b=>b.writeUInt16LE(199,17),'CHIP_REVISION_UNSUPPORTED'],
      [b=>b[3]=0x30,'FLASH_CAPACITY_UNSUPPORTED'],
      [b=>b.writeUInt32LE(0x40850000,24),'H2_SRAM_REQUIRED'],
      [b=>b.writeUInt32LE(0x50001000,24),'H2_SRAM_REQUIRED'],
      [b=>b.writeUInt32LE(0x42000000,24),'H2_SRAM_REQUIRED'],
      [b=>b[40]^=1,'APP_IMAGE_CHECKSUM'],
    ]){
      const b=Buffer.from(valid);patch(b);await assert.rejects(validateAppImage(b,board,4<<20),e=>e.code===expected);
    }
    await assert.rejects(validateAppImage(valid,board,8<<20),e=>e.code==='FLASH_CAPACITY_UNSUPPORTED');
  }
});
