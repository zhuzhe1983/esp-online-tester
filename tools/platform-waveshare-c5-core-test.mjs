// Public runtime -> production Worker/WASM -> real RV32I -> RMT/GPIO/UART.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {Worker} from 'node:worker_threads';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {waveshareC5CoreProbe} from './fixtures/waveshare-c5-core-probe.mjs';
const root=new URL('../',import.meta.url),origin='https://waveshare-c5-core.invalid';
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
  throw Error('C5 core guest timeout: '+JSON.stringify(await runtime.observe()));
}
test('five exact C5 core profiles execute real RMT and BOOT input through the public Worker, including reload',async()=>{
  const requests=[];
  const runtime=createPlatformRuntime({catalog,workerFactory,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',fetchImpl:async value=>{
    const url=new URL(value);assert.equal(url.origin,origin);requests.push(url.pathname);
    return new Response(await readFile(new URL('web'+url.pathname,root)));
  }});
  try{
    for(const [id,expected] of [
      ['waveshare-esp32-c5-mini-kit',[0x12,0x34,0x56]],['waveshare-esp32-c5-pico',[0x12,0x34,0x56]],
      ['waveshare-esp32-c5-zero',[0x34,0x12,0x56]],['waveshare-esp32-c5-wifi6-kit-n16r8',[0x34,0x12,0x56]],
      ['waveshare-esp32-c5-wifi6-kit-n32r8',[0x34,0x12,0x56]],
    ]){
      const scene=createDefaultScene(catalog,id);scene.firmware=null;await runtime.configure(scene);
      const caps=await runtime.capabilities();assert.deepEqual(caps.execution.bootModes,['app-direct']);
      for(const available of [caps.network.available,caps.sdCard.available,caps.deviceStorage.available,caps.audio.output.available])assert.equal(available,false);
      await runtime.loadFirmware({file:new File([waveshareC5CoreProbe()],'original-core-sram-diagnostic.bin')});await runtime.start();
      const ready=await until(runtime,s=>s.serialByPort?.uart0?.includes('BOOT UP\n')&&JSON.stringify(s.leds)===JSON.stringify([expected]));
      assert.equal(ready.frame,null);assert(ready.stats.insns>0,id+' executes guest instructions');
      for(const [pressed,marker] of [[true,'BOOT DOWN\n'],[false,'BOOT UP\n']]){
        const before=(await runtime.observe()).serialByPort.uart0.length;
        await runtime.input({type:'button',id:'boot',pressed});
        await until(runtime,s=>s.serialByPort?.uart0?.slice(before).includes(marker));
      }
      await runtime.reset();await until(runtime,s=>s.serialByPort?.uart0?.startsWith('CORE C5 SRAM READY\n')&&JSON.stringify(s.leds)===JSON.stringify([expected]));await runtime.pause();
    }
    assert(requests.every(path=>path.endsWith('esp32sim.wasm')),'No substituted ROM or diagnostic dependency');
  }finally{await runtime.dispose();}
});
