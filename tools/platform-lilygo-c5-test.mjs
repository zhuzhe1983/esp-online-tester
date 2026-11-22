// Actual public runtime -> Worker -> WASM -> RV32I -> GPIO -> ST7735 proof.
// The original SRAM diagnostic is not a factory-firmware compatibility claim.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {Worker} from 'node:worker_threads';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {lilygoC5Probe} from './fixtures/lilygo-c5-probe.mjs';

const root=new URL('../',import.meta.url),origin='https://lilygo-c5.invalid';
const catalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async key=>[key,JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items])));
const workerFactory=url=>{
  const real=new URL('web'+url.pathname,root);
  const source=`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`;
  const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(source)));
  const adapter={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};
  thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',error=>adapter.onerror?.(error));return adapter;
};
const serial=state=>state.serialByPort?.uart0||'';
async function waitFor(runtime,label,predicate){
  const end=Date.now()+45000;
  while(Date.now()<end){const state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(JSON.stringify(state.lastError));await new Promise(resolve=>setTimeout(resolve,15));}
  throw Error(label+' timed out: '+JSON.stringify(await runtime.observe()));
}
test('newest LilyGO C5 boards execute real SRAM GPIO firmware and expose only implemented capabilities',async()=>{
  const requests=[];
  const runtime=createPlatformRuntime({catalog,workerFactory,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',fetchImpl:async value=>{
    const url=new URL(value);assert.equal(url.origin,origin);requests.push(url.pathname);assert(!url.pathname.includes('rom.elf'),'C5 must not load a substituted ROM');
    return new Response(await readFile(new URL('web'+url.pathname,root)));
  }});
  try{
    for(const productId of ['lilygo-t-dongle-c5','lilygo-t7-c5']){
      const scene=createDefaultScene(catalog,productId);scene.firmware=null;await runtime.configure(scene);
      const capabilities=await runtime.capabilities();
      assert.deepEqual(capabilities.execution.bootModes,['app-direct']);
      assert.equal(capabilities.sdCard.available,false);assert.equal(capabilities.deviceStorage.available,false);assert.equal(capabilities.network.available,false);
      await runtime.loadFirmware({file:new File([lilygoC5Probe()],'lilygo-c5-sram.bin')});await runtime.start();
      const dongle=productId==='lilygo-t-dongle-c5';
      await waitFor(runtime,'initial GPIO framebuffer',s=>serial(s).includes('FRAME COMPLETE\n')&&(!dongle||s.frame));
      async function checkFrame(color){
        if(!dongle){assert.equal((await runtime.observe()).frame,null);return;}
        const end=Date.now()+5000;let frame;
        do{
          frame=await runtime.capture({format:'rgba'});
          if(frame.pixels.every((value,index)=>value===[...color,255][index%4]))break;
          await new Promise(resolve=>setTimeout(resolve,20));
        }while(Date.now()<end);
        assert.deepEqual([frame.width,frame.height],[80,160]);
        for(let at=0;at<frame.pixels.length;at+=4)assert.deepEqual([...frame.pixels.subarray(at,at+4)],[...color,255]);
      }
      await checkFrame([255,0,0]);
      for(const [pressed,marker,color] of [[true,'BOOT DOWN\n',[0,255,0]],[false,'BOOT UP\n',[255,0,0]]]){
        const before=serial(await runtime.observe()).length;
        await runtime.input({type:'button',id:'boot',pressed});
        await waitFor(runtime,'fresh real GPIO input',s=>serial(s).slice(before).includes(marker)&&serial(s).slice(before).includes('FRAME COMPLETE\n'));
        await checkFrame(color);
      }
      await runtime.pause();const paused=await runtime.observe();await new Promise(resolve=>setTimeout(resolve,50));assert.equal((await runtime.observe()).stats.time,paused.stats.time);
      await runtime.reset();await waitFor(runtime,'same SRAM image after reset',s=>serial(s).startsWith('LILYGO C5 SRAM READY\n')&&serial(s).includes('FRAME COMPLETE\n')&&(!dongle||s.frame));await checkFrame([255,0,0]);
      await runtime.pause();
    }
    assert(requests.every(path=>path.endsWith('esp32sim.wasm')));
  }finally{await runtime.dispose();}
});
