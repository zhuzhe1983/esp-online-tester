import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createJitHost} from '../web/wasm/jit.mjs';
import {chipForBoard} from '../web/platform/app-image.js';

test('every runnable M5Stack SKU constructs through the actual production WASM ABI with its declared chip and memory',async()=>{
 const products=JSON.parse(await readFile(new URL('../web/platform/catalog/products.json',import.meta.url))).items.filter(p=>p.manufacturer==='M5Stack'&&p.runtime?.board&&p.runtime.status!=='unavailable');
 let wasm;const logs=[],jit=createJitHost(()=>wasm);
 const {instance}=await WebAssembly.instantiate(await readFile(new URL('../web/wasm/esp32sim.wasm',import.meta.url)),{env:{...jit.imports,host_log:(ptr,len)=>logs.push(new TextDecoder().decode(new Uint8Array(wasm.memory.buffer,ptr,len)))}});wasm=instance.exports;
 for(const product of products){
  const profile=product.runtime,bytes=new TextEncoder().encode(profile.board),ptr=wasm.esp32sim_alloc(bytes.length);
  new Uint8Array(wasm.memory.buffer).set(bytes,ptr);
  try{
   logs.length=0;assert.equal(chipForBoard(profile.board),product.chip,product.id+' must retain its own chip family');
   const emulator=wasm.esp32sim_new(ptr,bytes.length,profile.flashMb,profile.psramMb);
   assert(emulator,product.id+': '+logs.join('\n'));
   wasm.esp32sim_delete(emulator);
  }finally{wasm.esp32sim_free(ptr,bytes.length);}
 }
 console.log(`${products.length} M5Stack SKU constructions passed; no factory application compatibility inferred.`);
});
