// Public runtime -> production Worker/WASM -> real RV32I -> RMT/GPIO/UART.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {join} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {waveshareC6ZeroProbe} from './fixtures/waveshare-c6-zero-probe.mjs';
const root=new URL('../',import.meta.url),origin='https://waveshare-c6-zero.invalid';
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
  throw Error('C6 Zero guest timeout: '+JSON.stringify(await runtime.observe()));
}
test('three exact C6 Zero capacity/board profiles execute real RMT and BOOT input through the public Worker, including reload',async()=>{
  const requests=[];
  const runtime=createPlatformRuntime({catalog,workerFactory,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',fetchImpl:async value=>{
    const url=new URL(value);assert.equal(url.origin,origin);requests.push(url.pathname);
    if(url.pathname.startsWith('/sram-'))return new Response(waveshareC6ZeroProbe(Number(url.pathname.match(/sram-(\d+)/)[1])));
    if(process.env.ESP32SIM_ROM_DIR&&url.pathname==='/wasm/fw/esp32c6_rev0_rom.elf')return new Response(await readFile(join(process.env.ESP32SIM_ROM_DIR,'esp32c6_rev0_rom.elf')));
    return new Response(await readFile(new URL('web'+url.pathname,root)));
  }});
  try{
    for(const [id,flashMb] of [
      ['waveshare-esp32-c6-zero',8],['waveshare-esp32-c6-zero-fh4',4],['waveshare-esp32-c6-zero-b',8],
    ]){
      const expected=[0x34,0x12,0x56];
      const scene=createDefaultScene(catalog,id);scene.firmware=null;await runtime.configure(scene);
      await runtime.loadFirmware({url:origin+'/sram-'+flashMb+'.bin',bootMode:'app-direct'});await runtime.start();
      const ready=await until(runtime,s=>s.serialByPort?.uart0?.includes('BOOT UP\n')&&JSON.stringify(s.leds)===JSON.stringify([expected]));
      assert.equal(ready.frame,null);assert(ready.stats.insns>0,id+' executes guest instructions');
      for(const [pressed,marker] of [[true,'BOOT DOWN\n'],[false,'BOOT UP\n']]){
        const before=(await runtime.observe()).serialByPort.uart0.length;
        await runtime.input({type:'button',id:'boot',pressed});
        await until(runtime,s=>s.serialByPort?.uart0?.slice(before).includes(marker));
      }
      await runtime.reset();await until(runtime,s=>s.serialByPort?.uart0?.startsWith('ZERO C6 SRAM READY\n')&&JSON.stringify(s.leds)===JSON.stringify([expected]));await runtime.pause();
    }
    assert(requests.every(path=>path.endsWith('esp32sim.wasm')||path.endsWith('esp32c6_rev0_rom.elf')||path.startsWith('/sram-')),'Only authored diagnostic and exact original C6 ROM may be fetched');
  }finally{await runtime.dispose();}
});
