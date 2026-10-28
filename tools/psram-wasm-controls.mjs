#!/usr/bin/env node
// Pin no-PSRAM classic/S3 console and work across two actual WASM builds.
// ESP32SIM_WASM=<module> node tools/psram-wasm-controls.mjs <output.json>
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { dirname,join } from 'node:path';
import { createJitHost } from '../web/wasm/jit.mjs';
const root=join(dirname(fileURLToPath(import.meta.url)),'..');
const fw=process.env.ESP32SIM_ROM_DIR || join(root,'web/wasm/fw');
const bytes=readFileSync(process.env.ESP32SIM_WASM || join(root,'web/wasm/esp32sim.wasm'));
const sha=b=>createHash('sha256').update(b).digest('hex');
const results=[];
for(const jit of [0,1]) for(const [board,rom,prefix] of [['esp32','esp32_rev300_rom.elf','classic-hello'],['none','esp32s3_rev0_rom.elf','hello']]) {
  let w; const logs=[]; const host=createJitHost(()=>w), dec=new TextDecoder(), enc=new TextEncoder();
  const mem=()=>new Uint8Array(w.memory.buffer);
  const {instance}=await WebAssembly.instantiate(bytes,{env:{...host.imports,host_profile_now:()=>performance.now(),host_log:(p,n)=>logs.push(dec.decode(mem().subarray(p,p+n)))}});
  w=instance.exports;
  const withBytes=(b,f)=>{const p=w.esp32sim_alloc(b.length); mem().set(b,p); try{return f(p,b.length);}finally{w.esp32sim_free(p,b.length);}};
  const e=withBytes(enc.encode(board),(p,n)=>w.esp32sim_new(p,n,4,0)); assert(e);
  for(const [kind,file] of [[0,rom],[1,`public/${prefix}-bootloader.bin`],[2,`public/${prefix}-ptable.bin`],[3,`public/${prefix}_world.bin`]]) {
    assert.equal(withBytes(readFileSync(join(fw,file)),(p,n)=>w.esp32sim_load(e,kind,p,n)),0);
  }
  w.esp32sim_set_jit(e,jit); assert.equal(w.esp32sim_boot(e,0),0);
  let serial=''; const end=w.esp32sim_cpu_hz(e)*3;
  while(w.esp32sim_cycles(e)<end) {
    assert.equal(w.esp32sim_run(e,1000000,0),0);
    const n=w.esp32sim_out_take(e);
    for(let i=0;i<n;i++) if(w.esp32sim_out_kind(e,i)===1) {
      const p=w.esp32sim_out_ptr(e,i),len=w.esp32sim_out_len(e,i);
      const m=JSON.parse(dec.decode(mem().subarray(p,p+len))); if(m.t==='serial')serial+=m.data;
    }
  }
  assert(serial.includes('Hello world!'),serial);
  assert(!logs.some(l=>l.includes('panic')));
  results.push({board,jit,cycles:w.esp32sim_cycles(e),insns:w.esp32sim_insns(e),serial_bytes:Buffer.byteLength(serial),serial_sha256:sha(serial)});
  w.esp32sim_delete(e);
}
writeFileSync(process.argv[2],JSON.stringify({wasm_sha256:sha(bytes),results},null,2)+'\n');
console.log(JSON.stringify(results));
