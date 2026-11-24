// Dynamic delivery subsets use the real production WASM and the exact board factories.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createJitHost} from '../web/wasm/jit.mjs';
import {chipForBoard} from '../web/platform/app-image.js';
import {WAVESHARE_BOARD_CAPABILITIES} from '../web/platform/waveshare-board-families.js';
import {hasSdCard} from '../web/platform/board-compatibility.js';
import {hasTfSlot} from '../web/platform/tf-card-capability.js';

test('every delivered Waveshare SKU constructs with its exact chip and memory; wrong capacities are rejected', async()=>{
  const products=JSON.parse(await readFile(new URL('../web/platform/catalog/products.json',import.meta.url))).items
    .filter(p=>p.manufacturer==='Waveshare'&&p.runtime?.board&&p.runtime.status!=='unavailable');
  let wasm;const logs=[],jit=createJitHost(()=>wasm);
  const {instance}=await WebAssembly.instantiate(await readFile(new URL('../web/wasm/esp32sim.wasm',import.meta.url)),{
    env:{...jit.imports,host_log:(ptr,len)=>logs.push(new TextDecoder().decode(new Uint8Array(wasm.memory.buffer,ptr,len)))},
  });wasm=instance.exports;
  for(const product of products){
    const profile=product.runtime,bytes=new TextEncoder().encode(profile.board),ptr=wasm.esp32sim_alloc(bytes.length);
    new Uint8Array(wasm.memory.buffer).set(bytes,ptr);
    try{
      logs.length=0;const emulator=wasm.esp32sim_new(ptr,bytes.length,profile.flashMb,profile.psramMb);
      assert(emulator,product.id+': '+logs.join('\n'));
      try{
        if(hasSdCard(profile.board)){
          const size=1<<20,media=wasm.esp32sim_alloc(size);
          try{
            new Uint8Array(wasm.memory.buffer,media,size).fill(0x5a);
            assert.equal(wasm.esp32sim_load(emulator,8,media,size),0,product.id+' advertised SD transport must reach the board through the public ABI: '+logs.join('\n'));
            assert.equal(wasm.esp32sim_sd_len(emulator),size);
            assert.equal(new Uint8Array(wasm.memory.buffer,wasm.esp32sim_sd_ptr(emulator),size)[512],0x5a);
          }finally{wasm.esp32sim_free(media,size);}
        }
      }finally{wasm.esp32sim_delete(emulator);}
      assert.equal(chipForBoard(profile.board),product.chip,product.id);
      if(hasSdCard(profile.board))assert(hasTfSlot(product),product.id+' SD transport needs its physical slot');
      if(WAVESHARE_BOARD_CAPABILITIES[profile.board]){
        for(const [flash,psram] of [[profile.flashMb===4?8:4,profile.psramMb],[profile.flashMb,profile.psramMb===0?8:0]]){
          const invalid=wasm.esp32sim_new(ptr,bytes.length,flash,psram);
          if(invalid)wasm.esp32sim_delete(invalid);
          assert.equal(invalid,0,product.id+' cannot silently replace physical memory capacities');
        }
      }
    }finally{wasm.esp32sim_free(ptr,bytes.length);}
  }
});
