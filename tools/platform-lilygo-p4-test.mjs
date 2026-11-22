// Real public runtime -> real Worker -> production WASM -> P4 RV32I -> XL9535.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {Worker} from 'node:worker_threads';
import {basename,join} from 'node:path';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {validateAppImage} from '../web/platform/app-image.js';
import {lilygoP4Probe} from './fixtures/lilygo-p4-probe.mjs';
const root=new URL('../',import.meta.url),origin='https://lilygo-p4.invalid';
const serial=state=>state.serialByPort?.uart0||'';
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const sentInputs=[];
const workerFactory=url=>{
  const source=`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(new URL('web'+url.pathname,root)))});parentPort.on('message',data=>globalThis.onmessage({data}));`;
  const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(source)));
  const adapter={postMessage:(...args)=>{if(args[0].op==='text')sentInputs.push(JSON.parse(args[0].data));thread.postMessage(...args);},terminate:()=>thread.terminate()};
  thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',error=>adapter.onerror?.(error));return adapter;
};
async function waitFor(runtime,predicate){
  const until=Date.now()+30000;let state;
  do{state=await runtime.observe();if(predicate(state))return state;if(state.status==='error'||serial(state).includes('FAILURE'))throw Error(JSON.stringify(state));await delay(15);}while(Date.now()<until);
  throw Error('P4 guest timeout '+JSON.stringify({status:state.status,serial:serial(state),error:state.lastError,stats:state.stats}));
}
test('T-Display-P4 guest verifies I2C0 XL9535, GPIO35 input, pause/reset and absence of fabricated DSI frames',async()=>{
  const catalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async key=>[key,JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items])));
  const image=lilygoP4Probe(),requests=[];
  const runtime=createPlatformRuntime({catalog,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',workerFactory,fetchImpl:async url=>{
    const path=new URL(url,origin).pathname;requests.push(path);
    if(path==='/p4-probe.bin')return new Response(image);
    if(path.endsWith('/esp32sim.wasm'))return new Response(await readFile(new URL('web/wasm/esp32sim.wasm',root)));
    if(path.endsWith('/esp32p4_rev0_rom.elf')){
      try{return new Response(await readFile(new URL('web/wasm/fw/'+basename(path),root)));}
      catch(error){
        // CI keeps mask ROMs outside the served web tree.
        if(error.code!=='ENOENT'||!process.env.ESP32SIM_ROM_DIR)throw error;
        return new Response(await readFile(join(process.env.ESP32SIM_ROM_DIR,basename(path))));
      }
    }
    throw Error('Unexpected real P4 runtime dependency: '+path);
  }});
  try{
    const scene=createDefaultScene(catalog,'lilygo-t-display-p4');scene.firmware=null;await runtime.configure(scene);
    assert.deepEqual(await validateAppImage(image,'lilygo-t-display-p4',16<<20),{imageBytes:image.length,chipId:18});
    const sha256=createHash('sha256').update(image).digest('hex');
    await runtime.loadFirmware({url:origin+'/p4-probe.bin',sha256,bootMode:'app-direct'});await runtime.start();
    let state=await waitFor(runtime,state=>serial(state).includes('LILYGO P4 SRAM READY\n')&&serial(state).includes('BOOT UP\n'));
    assert(serial(state).startsWith('XL9535 OK\n'));assert.equal(state.frame,null);
    assert.equal(state.storage.scope.chip,'esp32p4');assert.equal(state.storage.scope.flashBytes,16<<20);
    assert.deepEqual(state.firmware.stubs,[]);assert.equal(state.firmware.artifacts.find(a=>a.kind==='app').sha256,sha256);
    for(const [pressed,marker] of [[true,'BOOT DOWN\n'],[false,'BOOT UP\n']]){
      const before=serial(await runtime.observe()).length;await runtime.input({type:'button',id:'boot',pressed});
      state=await waitFor(runtime,state=>serial(state).slice(before).includes(marker));assert.equal(state.frame,null);
    }
    await runtime.pause();const paused=await runtime.observe();await delay(50);assert.equal((await runtime.observe()).stats.time,paused.stats.time);
    assert.deepEqual(sentInputs.filter(input=>input.t==='board-button'),[{t:'board-button',id:'boot',pressed:1},{t:'board-button',id:'boot',pressed:0}]);
    // Public reset reconstructs the Worker/machine; native same-machine MCU
    // reset preservation is checked separately in esp32p4/tests/lilygo.rs.
    await runtime.reset();await waitFor(runtime,state=>serial(state).startsWith('XL9535 OK\n')&&serial(state).includes('BOOT UP\n'));await runtime.pause();
    assert(requests.some(path=>path.endsWith('esp32p4_rev0_rom.elf')));assert(requests.some(path=>path.endsWith('esp32sim.wasm')));
  }finally{await runtime.dispose();}
});
