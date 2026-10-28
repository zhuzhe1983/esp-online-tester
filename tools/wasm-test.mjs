#!/usr/bin/env node
// Run the real WebAssembly module the way the page does — instantiate web/wasm/esp32sim.wasm,
// drive the C ABI (docs/wasm.md) through a firmware manifest, drain the outbox — and check that
// the firmware boots to its console output with no panic. The native ABI tests (wasm/tests/abi.rs)
// compile the same crate for the host, so they can never see a wasm-only abort; this can.
//
//   tools/wasm-test.mjs [manifest ...]          default: hello c3-hello
//   ESP32SIM_ROM_DIR=dir                        mask ROM ELFs not found next to the manifests
//   ESP32SIM_NO_WASM_JIT=1                      interpreter-only manifest runs
//   ESP32SIM_WASM=path                          another module, so two builds can be timed on one manifest
//
// A manifest (web/wasm/fw/<name>.json) names the board, sizes, stubs and files exactly as the
// page reads them. Each run boots and executes `seconds` (3) of emulated time in 2 M-cycle
// slices, then expects a `board` message, the expected console line, at least `min_frames` panel
// frames when the manifest says so, and a clean run.
import { readFileSync, existsSync, readdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { homedir } from 'node:os';
import { createJitHost } from '../web/wasm/jit.mjs';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const fwDir = process.env.FW_DIR || join(root, 'web', 'wasm', 'fw');
const names = process.argv.slice(2).length ? process.argv.slice(2) : ['hello', 'c3-hello'];
const EXPECT = { console: 'Hello world!', seconds: 3 };

function romPath(file) {
  for (const d of [fwDir, process.env.ESP32SIM_ROM_DIR].filter(Boolean)) { const p = join(d, file); if (existsSync(p)) return p; }
  const base = join(homedir(), '.espressif', 'tools', 'esp-rom-elfs');
  if (existsSync(base)) for (const rel of readdirSync(base).sort().reverse()) { const p = join(base, rel, file); if (existsSync(p)) return p; }
  throw new Error(`${file}: not in ${fwDir}, ESP32SIM_ROM_DIR or ~/.espressif/tools/esp-rom-elfs`);
}

// ESP32SIM_WASM names another module, so two builds can be timed on the same manifest.
const wasmBytes = readFileSync(process.env.ESP32SIM_WASM || join(root, 'web', 'wasm', 'esp32sim.wasm'));
const enc = new TextEncoder(), dec = new TextDecoder();
let failures = 0;

function dispatchJit(w, emu, mem, cache, disabled, cycles) {
  if (!w.esp32sim_jit_prepare) return false;
  const id = w.esp32sim_jit_prepare(emu, cycles, Date.now());
  if (id === 0) return false;
  if (disabled.has(id)) { w.esp32sim_jit_abort(emu); return false; }
  try {
    let instance = cache.get(id);
    if (!instance) {
      const p = w.esp32sim_jit_module_ptr(emu), len = w.esp32sim_jit_module_len(emu);
      const module = new WebAssembly.Module(mem().slice(p, p + len));
      instance = new WebAssembly.Instance(module, { env: { memory: w.memory } });
      cache.set(id, instance);
    }
    instance.exports.run();
    if (w.esp32sim_jit_commit(emu) === 1) return true;
  } catch (_) {
    w.esp32sim_jit_abort(emu);
  }
  disabled.add(id);
  return false;
}

/// A network manifest (`"nodes": [...]`) boots several motes on one medium through the
/// `esp32sim_net_*` ABI and checks each node's console and frame counters, the way the page does.
async function runNetwork(name, m) {
  const logs = [];
  let w;
  const blockJit = createJitHost(() => w);
  const { instance } = await WebAssembly.instantiate(wasmBytes, { env: { ...blockJit.imports, host_profile_now: () => performance.now(), host_log: (p, n) => logs.push(dec.decode(mem().subarray(p, p + n))) } });
  w = instance.exports;
  const mem = () => new Uint8Array(w.memory.buffer);
  const withBytes = (bytes, f) => { const p = w.esp32sim_alloc(bytes.length); mem().set(bytes, p); try { return f(p, bytes.length); } finally { w.esp32sim_free(p, bytes.length); } };
  const file = (rel) => readFileSync(rel.endsWith('_rom.elf') && !rel.includes('/') ? romPath(rel) : join(fwDir, rel));

  const net = w.esp32sim_net_new(m.slice_ns || 0);
  const kinds = { rom: 0, bootloader: 1, ptable: 2, app: 3, flash: 5 };
  const text = m.nodes.map(() => '');
  m.nodes.forEach((node, i) => {
    const mac = (node.mac || `02:00:00:00:00:0${i + 1}`).split(':').map(h => parseInt(h, 16));
    withBytes(new Uint8Array(mac), (mp) => withBytes(enc.encode(node.board || m.board || 'none'),
      (bp, bn) => w.esp32sim_net_add(net, mp, node.flash_mb || m.flash_mb || 2, (node.start_ms || 0) * 1e6, node.x || 0, node.y || 0, bp, bn)));
    for (const [k, v] of Object.entries({ ...(m.files || {}), ...(node.files || {}) })) {
      for (const rel of [].concat(v)) {
        const rc = withBytes(new Uint8Array(file(rel)), (p, n) => w.esp32sim_net_load(net, i, k === 'elf' ? 4 : kinds[k], p, n));
        if (rc !== 0) throw new Error(`node ${i}: load ${k} ${rel} failed: ${logs.join(' | ')}`);
      }
    }
    for (const s of [].concat(node.stubs || m.stubs || [])) {
      const [sym, ...values] = s.split('='); const val = values.length ? values.join('=') : undefined;
      const name = ((node.symbols || m.symbols) || {})[sym] || sym;   // as the page: a symbols map resolves a stub without shipping the ELF; a node's own wins
      if (withBytes(enc.encode(name + (val === undefined ? '' : '=' + val)), (p, n) => w.esp32sim_net_stub_spec(net, i, p, n)) !== 0) throw new Error(`node ${i}: stub ${sym}: ${logs.join(' | ')}`);
    }
  });
  if (w.esp32sim_net_boot(net) !== 0) throw new Error(`boot failed: ${logs.join(' | ')}`);

  const t0 = Date.now();
  const until = (m.seconds || 30) * 1e9;
  const stepNs = 20e6;
  for (let t = stepNs; t <= until; t += stepNs) {
    w.esp32sim_net_run(net, t);
    m.nodes.forEach((_, i) => { const n = w.esp32sim_net_console_take(net, i); if (n) text[i] += dec.decode(mem().subarray(w.esp32sim_net_console_ptr(net), w.esp32sim_net_console_ptr(net) + n)); });
  }
  const stat = (i, k) => w.esp32sim_net_stat(net, i, k);
  const problems = [];
  if (logs.some(l => l.includes('panic'))) problems.push(`panicked: ${logs.find(l => l.includes('panic'))}`);
  m.nodes.forEach((node, i) => {
    const want = node.expect || m.expect;
    if (want && !text[i].includes(want)) problems.push(`node ${i} console never showed ${JSON.stringify(want)} (${text[i].length} bytes)`);
    if (node.min_tx !== undefined && stat(i, 0) < node.min_tx) problems.push(`node ${i} sent ${stat(i, 0)} frames, wanted ${node.min_tx}`);
    if (node.min_rx !== undefined && stat(i, 1) < node.min_rx) problems.push(`node ${i} took ${stat(i, 1)} frames, wanted ${node.min_rx}`);
  });
  const wall = (Date.now() - t0) / 1000;
  const per = m.nodes.map((_, i) => `n${i} tx ${stat(i, 0)}/rx ${stat(i, 1)}`).join(', ');
  w.esp32sim_net_delete(net);
  if (problems.length) { failures++; console.error(`FAIL ${name}: ${problems.join('; ')}\n  console tails:\n${text.map((t, i) => `  [${i}] ${t.slice(-260)}`).join('\n')}`); }
  else console.log(`ok   ${name}: ${m.nodes.length} nodes, ${per}, ${(m.seconds || 30)} s simulated in ${wall.toFixed(1)} s wall (${((m.seconds || 30) / wall).toFixed(0)}x real time)`);
}

async function testJitHandoff() {
  let w;
  const blockJit = createJitHost(() => w);
  const { instance } = await WebAssembly.instantiate(wasmBytes, { env: { ...blockJit.imports, host_profile_now: () => performance.now(), host_log() {} } });
  w = instance.exports;
  const mem = () => new Uint8Array(w.memory.buffer);
  const withBytes = (bytes, f) => { const p = w.esp32sim_alloc(bytes.length); mem().set(bytes, p); try { return f(p, bytes.length); } finally { w.esp32sim_free(p, bytes.length); } };
  const emu = withBytes(enc.encode('none'), (p, n) => w.esp32sim_new(p, n, 1, 0));
  const entry = 0x40370000;
  const q = 256; // one default scheduling quantum (q256)
  const program = new Uint8Array(q * 2 + 3);
  for (let i = 0; i < q; i++) program.set([0x0c, 0x03], i * 2); // movi.n a3,0
  program.set([0x06, 0xff, 0xff], q * 2);                       // j .
  const app = new Uint8Array(24 + 8 + program.length), view = new DataView(app.buffer);
  app[0] = 0xe9; app[1] = 1; view.setUint32(4, entry, true);
  view.setUint32(24, entry, true); view.setUint32(28, program.length, true);
  app.set(program, 32);
  if (withBytes(app, (p, n) => w.esp32sim_load(emu, 3, p, n)) !== 0 || w.esp32sim_boot(emu, 1) !== 0) throw new Error('JIT fixture boot failed');
  if (!dispatchJit(w, emu, mem, new Map(), new Set(), q)) throw new Error('JIT handoff did not commit');
  if (w.esp32sim_cycles(emu) !== q || w.esp32sim_insns(emu) !== q) throw new Error('JIT handoff accounting mismatch');
  w.esp32sim_delete(emu);
  console.log('ok   wasm JIT handoff: shared-memory block committed at a scheduler boundary');
}

async function runManifest(name) {
  const m = JSON.parse(readFileSync(join(fwDir, `${name}.json`), 'utf8'));
  if (m.nodes) return runNetwork(name, m);
  const logs = [];
  const blockJit = createJitHost(() => w);
  const { instance } = await WebAssembly.instantiate(wasmBytes, { env: { ...blockJit.imports, host_profile_now: () => performance.now(), host_log: (p, n) => logs.push(dec.decode(mem().subarray(p, p + n))) } });
  const w = instance.exports;
  const mem = () => new Uint8Array(w.memory.buffer);
  const withBytes = (bytes, f) => { const p = w.esp32sim_alloc(bytes.length); mem().set(bytes, p); try { return f(p, bytes.length); } finally { w.esp32sim_free(p, bytes.length); } };
  const file = (rel) => readFileSync(rel.endsWith('_rom.elf') && !rel.includes('/') ? romPath(rel) : join(fwDir, rel));

  const emu = withBytes(enc.encode(m.board), (p, n) => w.esp32sim_new(p, n, m.flash_mb | 0, m.psram_mb | 0));
  if (!emu) throw new Error(`esp32sim_new(${m.board}) returned null: ${logs.join(' | ')}`);
  const kinds = { rom: 0, bootloader: 1, ptable: 2, app: 3, flash: 5, script: 6, picture: 7 };
  for (const [k, v] of Object.entries(m.files || {})) {
    for (const rel of [].concat(v)) {
      const rc = withBytes(new Uint8Array(file(rel)), (p, n) => w.esp32sim_load(emu, k === 'elf' ? 4 : kinds[k], p, n));
      if (rc !== 0) throw new Error(`load ${k} ${rel} failed: ${logs.join(' | ')}`);
    }
  }
  for (const [off, rel] of Object.entries(m.flash_at || {})) withBytes(new Uint8Array(file(rel)), (p, n) => w.esp32sim_load_at(emu, Number(off) >>> 0, p, n));
  // a co-processor (`"coproc"`, board `esp32p4…+c6`): its images go to the C6, as the page loads them
  for (const [k, v] of Object.entries((m.coproc || {}).files || {})) {
    const rc = withBytes(new Uint8Array(file(v)), (p, n) => w.esp32sim_load(emu, (k === 'elf' ? 4 : kinds[k]) | 0x100, p, n));
    if (rc !== 0) throw new Error(`load co-processor ${k} ${v} failed: ${logs.join(' | ')}`);
  }
  for (const [off, rel] of Object.entries((m.coproc || {}).flash_at || {})) withBytes(new Uint8Array(file(rel)), (p, n) => w.esp32sim_load_at(emu, (Number(off) | 0x80000000) >>> 0, p, n));
  for (const s of m.stubs || []) { const [sym, ...values] = s.split('='); const val = values.length ? values.join('=') : undefined; const name = (m.symbols || {})[sym] || sym;   // as the page: a symbols map resolves a stub without the ELF
    if (withBytes(enc.encode(name + (val === undefined ? '' : '=' + val)), (p, n) => w.esp32sim_stub_spec(emu, p, n)) !== 0) throw new Error('invalid stub: ' + s); }
  if (m.wifi && withBytes(enc.encode(m.wifi), (p, n) => w.esp32sim_wifi(emu, p, n)) !== 0) throw new Error('invalid WiFi configuration');
  w.esp32sim_set_jit(emu, process.env.ESP32SIM_NO_WASM_JIT ? 0 : 1);
  if (w.esp32sim_boot(emu, 0) !== 0) throw new Error(`boot failed: ${logs.join(' | ')}`);

  const hz = w.esp32sim_cpu_hz(emu);
  let board = null, text = '', frames = 0;
  const drain = () => {
    const n = w.esp32sim_out_take(emu);
    for (let i = 0; i < n; i++) {
      const kind = w.esp32sim_out_kind(emu, i), p = w.esp32sim_out_ptr(emu, i), len = w.esp32sim_out_len(emu, i);
      if (kind !== 1) { frames++; continue; }
      const msg = JSON.parse(dec.decode(mem().subarray(p, p + len)));
      if (msg.t === 'board') board = msg.name;
      if (msg.t === 'serial') text += msg.data;
    }
  };
  const target = hz * (m.seconds || EXPECT.seconds);
  const t0 = Date.now();
  while (w.esp32sim_cycles(emu) < target) {
    const rc = w.esp32sim_run(emu, 2_000_000, Date.now());
    if (rc !== 0) throw new Error(`esp32sim_run stopped with ${rc} at ${(w.esp32sim_cycles(emu) / hz).toFixed(3)} s: ${logs.slice(-3).join(' | ')}`);
    drain();
  }
  drain();
  const panics = logs.filter(l => l.includes('panic'));
  const problems = [];
  if (panics.length) problems.push(`panicked: ${panics[0]}`);
  if (!board) problems.push('no board message');
  if (!text.includes(m.expect || EXPECT.console)) problems.push(`console never showed ${JSON.stringify(m.expect || EXPECT.console)}; got ${text.length} bytes`);
  if (m.min_frames !== undefined && frames < m.min_frames) problems.push(`${frames} binary frames, wanted ${m.min_frames}: the firmware never drew its panel`);
  const insns = w.esp32sim_insns(emu);
  if (w.esp32sim_profile_report && (process.env.CENSUS_OUT || process.env.CENSUS_STDOUT)) {
    const at = logs.length;
    w.esp32sim_profile_report(emu);
    const report = logs.slice(at).join('\n');
    if (process.env.CENSUS_OUT) (await import('node:fs')).writeFileSync(process.env.CENSUS_OUT, report);
    if (process.env.CENSUS_STDOUT) {
      for (const line of report.split('\n')) if (/ex153|wasm-region|^core=|wasm-profile\]/.test(line)) console.log(line);
      console.log('jit_insns', w.esp32sim_block_jit_insns(emu), 'insns', insns, 'jitstats', JSON.stringify(blockJit.stats));
    }
  }
  w.esp32sim_delete(emu);
  const wall = (Date.now() - t0) / 1000;
  if (problems.length) { failures++; console.error(`FAIL ${name}: ${problems.join('; ')}\n  logs: ${logs.slice(0, 5).join('\n        ')}\n  console tail: ${text.slice(-400)}`); }
  else console.log(`ok   ${name}: board ${board}, ${(insns / 1e6).toFixed(1)} M insns in ${wall.toFixed(1)} s wall (${(insns / 1e6 / wall).toFixed(1)} Minsn/s), ${text.split('\n').length} console lines, ${frames} binary frames`);
}

if (!process.env.CENSUS_STDOUT) try { await testJitHandoff(); } catch (e) { failures++; console.error(`FAIL wasm JIT handoff: ${e.message}`); }
for (const n of names) { try { await runManifest(n); } catch (e) { failures++; console.error(`FAIL ${n}: ${e.message}`); } }
process.exit(failures ? 1 : 0);
