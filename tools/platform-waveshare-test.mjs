// Registry/ROM routing and real CPU acceptance for the newly integrated boards.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {join} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createJitHost} from '../web/wasm/jit.mjs';
import {chipForBoard,chipIdForBoard,validateBoardRevision,isSramOnlyBoard} from '../web/platform/app-image.js';
import {hasSdCard} from '../web/platform/board-compatibility.js';
import {hasTfSlot} from '../web/platform/tf-card-capability.js';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createSdCardStorage} from '../web/platform/sd-card-storage.js';
const root=new URL('../',import.meta.url),origin='https://waveshare-test.invalid';
const catalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async key=>
  [key,JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items])));
const products=catalog.products.filter(p=>p.manufacturer==='Waveshare'&&p.runtime?.board);

test('P4 revision routing never silently selects S3 or mixes ECO0 and ECO5 binaries',()=>{
  function header(min,max=399){const bytes=new Uint8Array(24),v=new DataView(bytes.buffer);bytes[0]=0xe9;v.setUint16(12,18,true);v.setUint16(15,min,true);v.setUint16(17,max,true);return bytes;}
  for(const board of ['waveshare-p4-core','waveshare-p4-nano','waveshare-p4-module-32mb-ipex','waveshare-p4-lcd35','p4-lcd35','waveshare-p4-lcd35-v32','esp32p4-rev3']){
    assert.equal(chipForBoard(board),'esp32p4');assert.equal(chipIdForBoard(board),18);
    validateBoardRevision(header(300),board);validateBoardRevision(header(301),board);
    for(const bytes of [header(100,199),header(302),header(300,300)])assert.throws(()=>validateBoardRevision(bytes,board),{code:'CHIP_REVISION_UNSUPPORTED'});
  }
  validateBoardRevision(header(100,199),'stamp-p4');
  validateBoardRevision(header(100,199),'waveshare-p4-lcd35-v13');
  for(const bytes of [header(101,199),header(301),header(0,99)])assert.throws(()=>validateBoardRevision(bytes,'waveshare-p4-lcd35-v13'),{code:'CHIP_REVISION_UNSUPPORTED'});
  assert.throws(()=>validateBoardRevision(header(301),'stamp-p4'),{code:'CHIP_REVISION_UNSUPPORTED'});
  assert.equal(chipForBoard('waveshare-c5-lcd147'),'esp32c5');assert.equal(isSramOnlyBoard('waveshare-c5-lcd147'),false);
  for(const p of products)if(hasSdCard(p.runtime.board))assert(hasTfSlot(p),p.id+' implemented transport requires a physical slot');
  for(const board of ['waveshare-p4-core','waveshare-p4-module-16mb','waveshare-p4-core-dev-kit'])assert.equal(hasSdCard(board),false);
});

test('every advertised Waveshare board constructs in production WASM and exact new memory profiles reject mismatches',async()=>{
  let wasm;const logs=[];const jit=createJitHost(()=>wasm);
  const {instance}=await WebAssembly.instantiate(await readFile(new URL('web/wasm/esp32sim.wasm',root)),{
    env:{...jit.imports,host_log:(ptr,len)=>logs.push(new TextDecoder().decode(new Uint8Array(wasm.memory.buffer,ptr,len)))},
  });wasm=instance.exports;
  for(const product of products){
    const profile=product.runtime,bytes=new TextEncoder().encode(profile.board),ptr=wasm.esp32sim_alloc(bytes.length);
    new Uint8Array(wasm.memory.buffer).set(bytes,ptr);
    try{
      logs.length=0;const emulator=wasm.esp32sim_new(ptr,bytes.length,profile.flashMb,profile.psramMb);
      assert(emulator,product.id+': '+logs.join('\n'));wasm.esp32sim_delete(emulator);
      assert.equal(chipForBoard(profile.board),product.chip);
      if(profile.board.startsWith('waveshare-p4-')||profile.board.startsWith('waveshare-c5-')){
        const invalid=wasm.esp32sim_new(ptr,bytes.length,profile.flashMb,profile.psramMb===0?8:0);
        if(invalid)wasm.esp32sim_delete(invalid);
        assert.equal(invalid,0,product.id+' must reject a different physical PSRAM capacity');
      }
    }finally{wasm.esp32sim_free(ptr,bytes.length);}
  }
});

test('seven P4 modules/options boot unchanged IDF example using ECO5 ROM through public Worker',async()=>{
  const requests=[];
  const workerFactory=url=>{
    const source=`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(new URL('web'+url.pathname,root)))});parentPort.on('message',data=>globalThis.onmessage({data}));`;
    const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(source)));
    const adapter={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};
    thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',error=>adapter.onerror?.(error));return adapter;
  };
  const store={indexedDB:null,BroadcastChannel:null,dbName:'waveshare-p4-example-'+Date.now()};
  const runtime=createPlatformRuntime({catalog,workerFactory,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',deviceStorage:createDeviceStorage(store),sdStorage:createSdCardStorage(store),fetchImpl:async value=>{
    const url=new URL(value,origin);assert.equal(url.origin,origin);requests.push(url.pathname);
    if(process.env.ESP32SIM_ROM_DIR&&url.pathname==='/wasm/fw/esp32p4_rev300_rom.elf')return new Response(await readFile(join(process.env.ESP32SIM_ROM_DIR,'esp32p4_rev300_rom.elf')));
    return new Response(await readFile(new URL('web'+url.pathname,root)));
  }});
  try{
    // Keep the original seven exact targets even when an audited factory image
    // becomes a product's default. This test deliberately runs the pinned IDF
    // example; default-firmware policy must not silently remove boot coverage.
    const firmware=catalog.firmwares.find(f=>f.id==='waveshare-p4-rev3-idf-hello');
    assert(firmware?.source,'The pinned P4 rev3 IDF example must remain available');
    assert.equal(firmware.role,'example');
    const targets=[
      ['waveshare-esp32-p4-core','36096','waveshare-p4-core',16],
      ['waveshare-esp32-p4-core-32mb','36097','waveshare-p4-core-32mb',32],
      ['waveshare-esp32-p4-module-16mb','28101','waveshare-p4-module-16mb',16],
      ['waveshare-esp32-p4-module-32mb','32604','waveshare-p4-module-32mb',32],
      ['waveshare-esp32-p4-module-16mb-ipex','35057','waveshare-p4-module-16mb-ipex',16],
      ['waveshare-esp32-p4-module-32mb-ipex','35058','waveshare-p4-module-32mb-ipex',32],
      ['waveshare-esp32-p4-poe-eth','32088','waveshare-p4-poe-eth',32],
    ];
    assert.equal(new Set(targets.map(([id])=>id)).size,7);
    const selected=targets.map(([id,sku,board,flashMb])=>{
      const product=products.find(p=>p.id===id);assert(product,id+' must remain registered');
      assert(firmware.productIds.includes(id),id+' must retain explicit example compatibility');
      assert.equal(product.sku,sku);assert.equal(product.chip,'esp32p4');
      assert.deepEqual({board:product.runtime.board,flashMb:product.runtime.flashMb,psramMb:product.runtime.psramMb},{board,flashMb,psramMb:32});
      return product;
    });
    for(const product of selected){
      await runtime.configure(createDefaultScene(catalog,product.id));
      assert.equal((await runtime.capabilities()).network.available,false);
      await runtime.loadFirmware(firmware.source);await runtime.start();
      const deadline=Date.now()+45000;let state;
      do{
        state=await runtime.observe();
        if(state.status==='error')throw Error(JSON.stringify(state.lastError));
        if((state.serialByPort?.uart0||'').includes('Hello world!'))break;
        await new Promise(resolve=>setTimeout(resolve,15));
      }while(Date.now()<deadline);
      assert((state.serialByPort?.uart0||'').includes('Hello world!'),product.id+' '+JSON.stringify(state));
      assert.equal(state.frame,null);assert.equal(state.storage.scope.chip,'esp32p4');
      assert.equal(state.storage.scope.flashBytes,product.runtime.flashMb*1024*1024);
      assert.deepEqual(state.firmware.stubs,[]);await runtime.pause();
    }
    assert(requests.some(p=>p.endsWith('/esp32p4_rev300_rom.elf')));
    assert(!requests.some(p=>p.endsWith('/esp32p4_rev0_rom.elf')||p.endsWith('/esp32s3_rev0_rom.elf')));
  }finally{await runtime.dispose();}
});
