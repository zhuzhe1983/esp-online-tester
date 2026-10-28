// Catalog-to-production-WASM identity guard. This does not execute firmware.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {chipIdForBoard} from '../web/platform/app-image.js';
import {createJitHost} from '../web/wasm/jit.mjs';

const products=JSON.parse(await readFile(new URL('../web/platform/catalog/products.json',import.meta.url))).items;
const ids=['m5stack-c124','m5stack-k159-b','m5stack-c151','m5stack-c139','m5stack-s007','m5stack-s007-v033','m5stack-c008','m5stack-c008-b','m5stack-c125','m5stack-dk001','m5stack-k131','m5stack-k131-v11','m5stack-k134-v11','m5stack-c008-c','m5stack-c126-echo','m5stack-c008-b-v11','m5stack-k147'];
let w;const host=createJitHost(()=>w),enc=new TextEncoder(),dec=new TextDecoder();
w=(await WebAssembly.instantiate(await readFile(new URL('../web/wasm/esp32sim.wasm',import.meta.url)),{env:{...host.imports,host_profile_now:()=>performance.now(),host_log(){}}})).instance.exports;
for(const id of ids){
 const product=products.find(p=>p.id===id);assert.ok(product,id);const r=product.runtime;
 assert.equal(r.status,'partial',id);assert.ok(r.board,id);assert.equal(chipIdForBoard(r.board),{esp32:0,esp32s3:9,esp32c6:13}[product.chip],id);
 const bytes=enc.encode(r.board),p=w.esp32sim_alloc(bytes.length);new Uint8Array(w.memory.buffer).set(bytes,p);
 const e=w.esp32sim_new(p,bytes.length,r.flashMb,r.psramMb||0);w.esp32sim_free(p,bytes.length);assert.notEqual(e,0,`${id}: production board registration`);
 try {
  // Boot only initializes the ABI and announces board/chip; no ROM or instructions run.
  assert.equal(w.esp32sim_boot(e,0),0,id);let announced;
  for(let i=0,n=w.esp32sim_out_take(e);i<n;i++)if(w.esp32sim_out_kind(e,i)===1){const x=JSON.parse(dec.decode(new Uint8Array(w.memory.buffer,w.esp32sim_out_ptr(e,i),w.esp32sim_out_len(e,i))));if(x.t==='board')announced=x;}
  assert.ok(announced,id);assert.equal(announced.chip.toLowerCase().replaceAll('-',''),product.chip,id);assert.equal(announced.name,r.board,id);
 } finally {w.esp32sim_delete(e);}
}
console.log(`ok ${ids.length} integrated product identities match their actual production WASM board/chip; no firmware compatibility claim`);
