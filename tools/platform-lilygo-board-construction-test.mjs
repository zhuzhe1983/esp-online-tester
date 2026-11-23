// Verify every advertised LilyGO runtime reaches a real compiled board factory.
// No product catalog or WASM exports are mocked. Protocol execution is covered
// by the family-specific guest probes and native MMIO tests.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createJitHost} from '../web/wasm/jit.mjs';
import {chipForBoard} from '../web/platform/app-image.js';

test('every advertised LilyGO runtime constructs with its real memory profile in production WASM',async()=>{
  const products=JSON.parse(await readFile(new URL('../web/platform/catalog/products.json',import.meta.url))).items
    .filter(product=>product.id.startsWith('lilygo-')&&product.runtime?.board);
  assert(products.length>=126,'The complete reviewed inventory includes 126 original/new board profiles');
  products.push({id:'bare-s2-backend',runtime:{board:'esp32s2',flashMb:4,psramMb:0,status:'diagnostic'}});
  let wasm;const logs=[];const jit=createJitHost(()=>wasm);
  const {instance}=await WebAssembly.instantiate(await readFile(new URL('../web/wasm/esp32sim.wasm',import.meta.url)),{
    env:{...jit.imports,host_log:(ptr,len)=>logs.push(new TextDecoder().decode(new Uint8Array(wasm.memory.buffer,ptr,len)))},
  });wasm=instance.exports;
  for(const product of products){
    const bytes=new TextEncoder().encode(product.runtime.board),ptr=wasm.esp32sim_alloc(bytes.length);let emulator=0;
    logs.length=0;
    try{
      new Uint8Array(wasm.memory.buffer).set(bytes,ptr);
      emulator=wasm.esp32sim_new(ptr,bytes.length,product.runtime.flashMb,product.runtime.psramMb);
      assert(emulator,product.id+': '+logs.join('\n'));
      assert(!logs.some(line=>/unknown board|requires|panic/i.test(line)),product.id+': '+logs.join('\n'));
      if(product.runtime.status==='diagnostic')assert(!logs.some(line=>/starting without PSRAM/i.test(line)),product.id+': '+logs.join('\n'));
      assert(['esp32','esp32s2','esp32s3','esp32c3','esp32c5','esp32c6','esp32p4'].includes(chipForBoard(product.runtime.board)));
    }finally{
      if(emulator)wasm.esp32sim_delete(emulator);
      wasm.esp32sim_free(ptr,bytes.length);
    }
  }
});
