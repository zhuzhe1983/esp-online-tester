// Actual platform -> Worker -> independent S2 WASM -> Xtensa -> GPIO/ST7789.
// Authored SRAM diagnostics prove this subset; no manufacturer firmware is substituted.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {Worker} from 'node:worker_threads';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';

const root=new URL('../',import.meta.url),origin='https://lilygo-s2.invalid';
const catalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async key=>[key,JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items])));
const workerFactory=url=>{
  const real=new URL('web'+url.pathname,root);
  const source=`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`;
  const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(source)));
  const adapter={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};
  thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',error=>adapter.onerror?.(error));return adapter;
};
const serial=state=>state.serialByPort?.uart0||'';
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
async function waitFor(runtime,label,predicate){
  const deadline=Date.now()+45000;
  while(Date.now()<deadline){const state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(label+': '+JSON.stringify(state.lastError));await delay(20);}
  const state=await runtime.observe();throw Error(label+' timed out: '+JSON.stringify({status:state.status,serial:serial(state),error:state.lastError}));
}
test('all three S2 products execute real SRAM guests, GPIO input and UART echo without ROM or Flash emulation',{timeout:180000},async()=>{
  const requests=[],runtime=createPlatformRuntime({catalog,workerFactory,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',fetchImpl:async value=>{
    const url=new URL(value);assert.equal(url.origin,origin);requests.push(url.pathname);
    assert(url.pathname.endsWith('esp32sim.wasm'),'S2 must not fetch a ROM or a replacement image: '+url.pathname);
    return new Response(await readFile(new URL('web'+url.pathname,root)));
  }});
  try{
    for(const productId of ['lilygo-t-dongle-esp32-s2-1-14-inch-lcd-board','lilygo-esp32-s2','lilygo-esp32-s2-woor']){
      const dongle=productId.includes('dongle'),name=dongle?'dongle-sram-diagnostic':'s2-sram-console';
      const fixture=await readFile(new URL(`esp32s2/tests/fixtures/${name}.bin`,root));
      const receipt=JSON.parse(await readFile(new URL(`esp32s2/tests/fixtures/${name}.json`,root)));
      assert.equal(createHash('sha256').update(fixture).digest('hex'),receipt.sha256);assert.equal(fixture.length,receipt.bytes);
      const scene=createDefaultScene(catalog,productId);scene.firmware=null;await runtime.configure(scene);
      const caps=await runtime.capabilities();assert.deepEqual(caps.execution.bootModes,['app-direct']);assert.equal(caps.execution.sramPhysicalBytes,320*1024);assert.equal(caps.execution.sramAliased,true);
      for(const value of [caps.network.available,caps.audio.input.available,caps.audio.output.available,caps.deviceStorage.available,caps.sdCard.available,caps.firmware.mergedImage])assert.equal(value,false);
      await runtime.loadFirmware({file:new File([fixture],name+'.bin')});await runtime.start();
      await waitFor(runtime,'initial S2 guest',state=>serial(state).includes('S2 SRAM READY\n')&&serial(state).includes(dongle?'FRAME RED\n':'BUTTON UP\n')&&(!dongle||state.frame));
      async function checkFrame(color){
        if(!dongle){assert.equal((await runtime.observe()).frame,null);return;}
        const expected=[...color,255],deadline=Date.now()+8000;let frame;
        do{frame=await runtime.capture({format:'rgba'});if(frame.pixels.every((value,index)=>value===expected[index%4]))break;await delay(25);}while(Date.now()<deadline);
        assert.deepEqual([frame.width,frame.height],[135,240]);
        assert.equal(frame.pixels.findIndex((value,index)=>value!==expected[index%4]),-1,'All ST7789 pixels originate from the actual S2 software-SPI guest');
      }
      await checkFrame([255,0,0]);
      for(const [pressed,marker,color] of [[true,'BUTTON DOWN\n',[0,255,0]],[false,'BUTTON UP\n',[255,0,0]]]){
        const before=serial(await runtime.observe()).length;await runtime.input({type:'button',id:'boot',pressed});
        await waitFor(runtime,'fresh S2 BOOT input',state=>serial(state).slice(before).includes(marker)&&(!dongle||serial(state).slice(before).includes(pressed?'FRAME GREEN\n':'FRAME RED\n')));
        await checkFrame(color);
      }
      const before=serial(await runtime.observe()).length;await runtime.input({type:'serial',port:'uart0',data:'S2-ECHO\n'});
      await waitFor(runtime,'actual UART RX/TX echo',state=>serial(state).slice(before).includes('S2-ECHO\n'));
      await assert.rejects(()=>runtime.input({type:'serial',port:'usb',data:'x'}),{code:'UNSUPPORTED_PORT'});
      await runtime.pause();const paused=await runtime.observe();await delay(50);assert.equal((await runtime.observe()).stats.time,paused.stats.time);
      await runtime.reset();await waitFor(runtime,'S2 reset reloads the same authored SRAM image',state=>serial(state).startsWith('S2 SRAM READY\n')&&serial(state).includes(dongle?'FRAME RED\n':'BUTTON UP\n')&&(!dongle||state.frame));
      await checkFrame([255,0,0]);await runtime.pause();
    }
    // Structurally valid S2 image, containing an S3-only PIE opcode. It must
    // reach the S2 instruction gate and surface a real execution failure.
    const invalidIsa=new Uint8Array(48),header=new DataView(invalidIsa.buffer);
    invalidIsa[0]=0xe9;invalidIsa[1]=1;header.setUint32(4,0x40020000,true);header.setUint16(12,2,true);
    header.setUint32(24,0x40020000,true);header.setUint32(28,4,true);invalidIsa[32]=4;invalidIsa[47]=0xef^4;
    const scene=createDefaultScene(catalog,'lilygo-esp32-s2');scene.firmware=null;await runtime.configure(scene);
    await runtime.loadFirmware({file:new File([invalidIsa],'s2-reject-s3-pie.bin')});
    await runtime.start().catch(error=>assert.equal(error.code,'EMULATOR_STOPPED'));
    const deadline=Date.now()+10000;let stopped;
    do{stopped=await runtime.observe();if(stopped.status==='error')break;await delay(20);}while(Date.now()<deadline);
    assert.equal(stopped.status,'error');assert.equal(stopped.lastError.code,'EMULATOR_STOPPED');
    assert.equal(stopped.frame,null);assert.equal(serial(stopped),'');
    assert(requests.length>=4&&requests.every(path=>path.endsWith('esp32sim.wasm')));
  }finally{await runtime.dispose();}
});
