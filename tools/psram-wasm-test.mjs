#!/usr/bin/env node
// Actual WASM + native ABI comparison using the retained public P5/P6 fixtures.
// Build the native example and wasm with --features jit-tests first. Required:
// ESP32SIM_ROM_DIR, ARDUINO2_FIRMWARE_DIR, ARDUINO3_FIRMWARE_DIR, CARGO_TARGET_DIR.
// --lifecycle runs just identical target-independent bus/factory/reset assertions.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { createJitHost } from '../web/wasm/jit.mjs';
const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const target = process.env.CARGO_TARGET_DIR || join(root, 'target');
const hash = s => createHash('sha256').update(s).digest('hex');
const wasm = readFileSync(process.env.ESP32SIM_WASM || join(target, 'wasm32-unknown-unknown/release/esp32sim_wasm.wasm'));
let w;
const logs = [], jitHost = createJitHost(() => w);
const enc = new TextEncoder(), dec = new TextDecoder();
const mem = () => new Uint8Array(w.memory.buffer); // reacquire after memory.grow
const { instance } = await WebAssembly.instantiate(wasm, { env: { ...jitHost.imports,
  host_profile_now: () => performance.now(), host_log: (p,n) => logs.push(dec.decode(mem().subarray(p,p+n))) } });
w = instance.exports;
try { assert.equal(w.esp32sim_test_classic_psram(), 4); }
catch (error) { console.error(logs.join('\n')); throw error; }
if (process.argv.includes('--lifecycle')) { console.log('PASS actual WASM classic lifecycle'); process.exit(0); }
const withBytes = (bytes, f) => { const p = w.esp32sim_alloc(bytes.length); mem().set(bytes,p);
  try { return f(p,bytes.length); } finally { w.esp32sim_free(p,bytes.length); } };
const serialLines = (messages, prefix) => messages.filter(m => m.t === 'serial').map(m => m.data).join('')
  .split(/\r?\n/).filter(l => l.startsWith(prefix.toUpperCase() + ' '));
function check(lines, prefix, size) {
  const text = lines.join('\n');
  assert(text.includes(`${prefix.toUpperCase()} result=${size ? 'pass' : 'no-psram'}`), text);
  if (size) {
    for (const [pass, h] of [[1,'20c19dc5'],[2,'9dce9dc5']]) {
      const line = lines.find(l => l.includes(`pass=${pass} allocation=1048576`));
      assert(line?.endsWith(`errors=0 fnv1a=${h}`), text);
      const addr = parseInt(line.match(/address=([0-9a-f]+)/)[1],16);
      assert(addr >= 0x3f800000 && addr <= 0x3fb00000);
    }
    assert(text.includes(prefix === 'p5' ? 'found=1 size=4192123 heap=4194303' : 'found=1 size=4194304 heap=4194304'), text);
  } else {
    assert(text.includes('found=0 size=0 heap=0 free=0'), text);
    assert(text.includes('absent-allocation=unavailable'), text);
    assert(!text.includes('pass=1'), text);
  }
}
const results = [];
for (const jit of [0,1]) for (const [prefix,size] of [['p5',4],['p5',0],['p6',4],['p6',0],['p5',4],['p6',4]]) {
  if (process.env.PSRAM_CASE && process.env.PSRAM_CASE !== `${prefix}:${size}:${jit}`) continue;
  const fw = process.env[prefix === 'p5' ? 'ARDUINO2_FIRMWARE_DIR' : 'ARDUINO3_FIRMWARE_DIR'];
  assert(fw, 'set both Arduino firmware directories to retained stock builds');
  const native = spawnSync('nice', ['-n','10', join(target,'release/examples/psram-qualification'), fw,prefix,String(size),String(jit)], {encoding:'utf8',maxBuffer:16<<20});
  assert.equal(native.status,0, native.stderr + native.stdout);
  const nativeMessages = native.stdout.trim().split('\n').map(l => JSON.parse(l));
  const e = withBytes(enc.encode('esp32'), (p,n) => w.esp32sim_new(p,n,4,size));
  assert(e);
  try {
    for (const [kind,path] of [[0,join(process.env.ESP32SIM_ROM_DIR,'esp32_rev300_rom.elf')],
      [1,join(fw,`${prefix}_probe.ino.bootloader.bin`)], [2,join(fw,`${prefix}_probe.ino.partitions.bin`)], [3,join(fw,`${prefix}_probe.ino.bin`)]]) {
      assert.equal(withBytes(readFileSync(path),(p,n) => w.esp32sim_load(e,kind,p,n)),0);
    }
    w.esp32sim_set_jit(e,jit);
    assert.equal(w.esp32sim_boot(e,0),0);
    for (const boot of [1,2]) {
      if (boot === 2) w.esp32sim_test_classic_reboot(e);
      const messages = [];
      for (let slice=0;slice<5000;slice++) {
        assert.equal(w.esp32sim_run(e,100000,0),0,logs.slice(-5).join('\n'));
        const n = w.esp32sim_out_take(e);
        for (let i=0;i<n;i++) if (w.esp32sim_out_kind(e,i) === 1) {
          const p=w.esp32sim_out_ptr(e,i), len=w.esp32sim_out_len(e,i);
          messages.push(JSON.parse(dec.decode(mem().subarray(p,p+len))));
        }
        if (messages.filter(m=>m.t==='serial').map(m=>m.data).join('').includes(prefix.toUpperCase()+' result=')) break;
      }
      const lines = serialLines(messages,prefix);
      const expected = serialLines(nativeMessages.filter(m=>m.boot===boot).map(m=>m.message),prefix);
      check(lines,prefix,size); check(expected,prefix,size);
      assert.deepEqual(lines,expected,`native/WASM serial milestones differ ${prefix} size=${size} jit=${jit} boot=${boot}`);
      const nativeJit=nativeMessages.find(m=>m.boot===boot && m.message.t==='qualification').message.jit_insns;
      const wasmJit=w.esp32sim_test_classic_jit_insns(e);
      if (!jit) { assert.equal(nativeJit,0); assert.equal(wasmJit,0); }
      else { assert(nativeJit>0,'native JIT enabled but never retired'); assert(wasmJit>0,'WASM JIT enabled but never retired'); }
      const row={prefix,size,jit,boot,native_jit_insns:nativeJit,wasm_jit_insns:wasmJit,serial_lines:lines,serial_sha256:hash(lines.join('\n'))};
      const prior=results.find(r=>r.prefix===prefix && r.size===size);
      if(prior) assert.equal(row.serial_sha256,prior.serial_sha256,'reboot/mode/recreated machine serial equality');
      results.push(row);
      if(process.env.PSRAM_RESULTS) writeFileSync(process.env.PSRAM_RESULTS,JSON.stringify({wasm_sha256:hash(wasm),complete:false,case:process.env.PSRAM_CASE || 'all',comparisons:results.length,results},null,2)+'\n');
      console.log(`PASS ${prefix} size=${size} jit=${jit} boot=${boot} serial=${row.serial_sha256}`);
    }
  } catch (error) { console.error(logs.slice(-12).join('\n')); throw error; }
  finally { w.esp32sim_delete(e); }
}
assert(!logs.some(l=>l.includes('panic')));
const receipt={wasm_sha256:hash(wasm),complete:true,case:process.env.PSRAM_CASE || 'all',comparisons:results.length,results};
if(process.env.PSRAM_RESULTS) writeFileSync(process.env.PSRAM_RESULTS,JSON.stringify(receipt,null,2)+'\n');
console.log(`PASS ${results.length} native/WASM boot comparisons; ${results.length/2} successive WASM machines in one instance`);
