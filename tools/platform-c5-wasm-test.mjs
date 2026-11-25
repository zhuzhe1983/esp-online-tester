// Actual compiled WASM ABI, cold setup only. No esp32sim_run call, original
// firmware execution, framebuffer success or browser acceptance is implied.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {createJitHost} from '../web/wasm/jit.mjs';
const paths=[process.env.ESP32SIM_C5_WASM,process.env.ESP32SIM_C5_ROM,process.env.ESP32SIM_C5_FACTORY];
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
test('external_actual_C5_WASM_loads_original_inputs_and_rejects_corruption_without_running_firmware',{skip:paths.every(Boolean)?false:'set ESP32SIM_C5_WASM, ESP32SIM_C5_ROM and ESP32SIM_C5_FACTORY to the private WASM and unchanged pinned inputs'},async()=>{
 const [binary,rom,factory]=(await Promise.all(paths.map(path=>readFile(path)))).map(bytes=>new Uint8Array(bytes));
 assert.equal(hash(rom),'49693d745eb4b4e6e6c77be4f5213765d545d8c7a1d9742a8a619bf596c4e27a');assert.equal(hash(factory),'cf10a4358d03898458c16390f3f16ceb0306befe2260160de0919d755b920942');
 let wasm;const logs=[],jit=createJitHost(()=>wasm),{instance}=await WebAssembly.instantiate(binary,{env:{...jit.imports,host_log:(p,n)=>logs.push(new TextDecoder().decode(new Uint8Array(wasm.memory.buffer,p,n)))}});wasm=instance.exports;
 const withBytes=(bytes,call)=>{const p=wasm.esp32sim_alloc(bytes.length);try{new Uint8Array(wasm.memory.buffer,p,bytes.length).set(bytes);return call(p,bytes.length);}finally{wasm.esp32sim_free(p,bytes.length);}};
 const create=()=>withBytes(new TextEncoder().encode('waveshare-c5-lcd273'),(p,n)=>wasm.esp32sim_new(p,n,32,8));let e=create();assert(e,logs.join('\n'));
 const load=(kind,bytes)=>withBytes(bytes,(p,n)=>wasm.esp32sim_load(e,kind,p,n)),flash=()=>new Uint8Array(wasm.memory.buffer,wasm.esp32sim_flash_ptr(e),wasm.esp32sim_flash_len(e));
 try{
  assert.equal(wasm.esp32sim_boot(e,0),1);assert.equal(load(0,rom),0);assert.equal(load(5,factory),0,logs.join('\n'));assert.equal(flash().length,32*1024*1024);assert.equal(hash(flash().subarray(0,factory.length)),hash(factory));
  const corruptRom=rom.slice();corruptRom[0x10000]^=1;assert.equal(load(0,corruptRom),1);
  for(const at of [0x200c,0x2000f,0x20020,0x8004]){const corrupt=factory.slice();corrupt[at]^=1;assert.equal(load(5,corrupt),1);assert.equal(hash(flash().subarray(0,factory.length)),hash(factory));}
  const page=new Uint8Array(4096).fill(0x5a);assert.equal(withBytes(page,(p,n)=>wasm.esp32sim_load_at(e,32*1024*1024-4096,p,n)),0);assert.equal(withBytes(page,(p,n)=>wasm.esp32sim_load_at(e,32*1024*1024,p,n)),1);assert.equal(flash().at(-1),0x5a);
  assert.equal(wasm.esp32sim_boot(e,0),0);assert.equal(wasm.esp32sim_insns(e),0);assert.equal(wasm.esp32sim_cycles(e),0);
  // Like the platform mode switch, start SRAM in a fresh machine. Its strict
  // erased-tail contract must not absorb a previously staged merged container.
  wasm.esp32sim_delete(e);e=create();assert(e);assert.equal(load(0,rom),0);
  const sram=new Uint8Array(48),v=new DataView(sram.buffer);sram[0]=0xe9;sram[1]=1;sram[12]=23;sram[15]=100;sram[17]=199;v.setUint32(4,0x40800000,true);v.setUint32(24,0x40800000,true);v.setUint32(28,4,true);sram[32]=0x6f;sram[47]=0xef^0x6f;
  assert.equal(load(3,sram),0);assert.equal(wasm.esp32sim_boot(e,1),0);assert.equal(wasm.esp32sim_boot(e,0),1,'SRAM mode disabled the bus ROM profile');
  const sramFlashHash=hash(flash());assert.equal(load(0,rom),1,'same-handle ROM reload cannot recreate a cold epoch after SRAM startup');assert.equal(hash(flash()),sramFlashHash);assert.equal(wasm.esp32sim_boot(e,0),1);assert.equal(wasm.esp32sim_insns(e),0);assert.equal(wasm.esp32sim_cycles(e),0);
  wasm.esp32sim_delete(e);e=create();assert(e);assert.equal(load(0,rom),0);assert.equal(load(5,factory),0,logs.join('\n'));assert.equal(wasm.esp32sim_boot(e,0),0);assert.equal(wasm.esp32sim_insns(e),0);assert.equal(wasm.esp32sim_cycles(e),0);
 }finally{wasm.esp32sim_delete(e);}
});
