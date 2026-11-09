// The receipt's page-side run: a firmware with its ELF under the virtual BLE controller in the
// WASM build, the same peer commands as a CLI `--script` at the same emulated times.
//   node ble-virtual-run.mjs MODULE BOARD ROM BOOTLOADER PTABLE APP ELF SCRIPT SECONDS
// BOARD is `esp32sim_new`'s (none = the S3, esp32c3, esp32c6, esp32); SCRIPT holds lines
// `SECONDS ble COMMAND`. Prints the kept lines as `[ble] LINE` (the CLI's stderr form), then the
// UART console prefixed `uart: `.
import { readFileSync } from 'node:fs';
import { createJitHost } from '../../../web/wasm/jit.mjs';

const [modulePath, board, rom, bootloader, ptable, app, elfPath, scriptPath, seconds] = process.argv.slice(2);
const logs = [];
let w;
const dec = new TextDecoder(), enc = new TextEncoder();
const mem = () => new Uint8Array(w.memory.buffer);
const jit = createJitHost(() => w);
const { instance } = await WebAssembly.instantiate(readFileSync(modulePath), { env: {
  ...jit.imports, host_profile_now: () => performance.now(),
  host_log: (p, n) => logs.push(dec.decode(mem().subarray(p, p + n))),
} });
w = instance.exports;
const bytes = (data, f) => { const p = w.esp32sim_alloc(data.length); mem().set(data, p); try { return f(p, data.length); } finally { w.esp32sim_free(p, data.length); } };

const emu = bytes(enc.encode(board), (p, n) => w.esp32sim_new(p, n, 4, 0));
if (!emu) throw new Error(`esp32sim_new(${board}): ${logs.join(' | ')}`);
for (const [kind, path] of [[0, rom], [1, bootloader], [2, ptable], [3, app]]) {
  if (bytes(readFileSync(path), (p, n) => w.esp32sim_load(emu, kind, p, n)) !== 0) throw new Error(`load ${path}: ${logs.join(' | ')}`);
}
if (bytes(readFileSync(elfPath), (p, n) => w.esp32sim_ble_virtual(emu, p, n)) !== 0) {
  console.log(`esp32sim_ble_virtual: 1 (${logs.at(-1)})`);
  process.exit(0);
}
if (w.esp32sim_boot(emu, 0) !== 0) throw new Error(`boot: ${logs.join(' | ')}`);

const hz = Number(w.esp32sim_cpu_hz(emu));
const commands = readFileSync(scriptPath, 'utf8').split('\n').map(l => l.trim()).filter(Boolean)
  .map(l => { const [at, ble, ...rest] = l.split(/\s+/); if (ble !== 'ble') throw new Error(l); return [Number(at), rest.join(' ')]; });
let uart = '';
const lines = [];
const drain = () => {
  for (let n; (n = w.esp32sim_ble_virtual_take(emu)) > 0;) {
    const p = w.esp32sim_ble_ptr(emu);
    lines.push(dec.decode(mem().subarray(p, p + n)));
  }
  const n = w.esp32sim_out_take(emu);
  for (let i = 0; i < n; i++) {
    if (w.esp32sim_out_kind(emu, i) !== 1) continue;
    const p = w.esp32sim_out_ptr(emu, i), len = w.esp32sim_out_len(emu, i);
    const msg = JSON.parse(dec.decode(mem().subarray(p, p + len)));
    if (msg.t === 'serial') uart += msg.data;
  }
};
const runTo = cycles => {
  while (w.esp32sim_cycles(emu) < cycles) {
    const left = Math.min(cycles - w.esp32sim_cycles(emu), 2_000_000);
    const rc = w.esp32sim_run(emu, left, 0);
    if (rc !== 0) throw new Error(`esp32sim_run ${rc}: ${logs.slice(-3).join(' | ')}`);
    drain();
  }
};
for (const [at, command] of commands) {
  runTo(Math.round(at * hz));
  if (bytes(enc.encode(command), (p, n) => w.esp32sim_ble_virtual_command(emu, p, n)) !== 0) throw new Error(`${command}: ${logs.at(-1)}`);
}
runTo(Math.round(Number(seconds) * hz));
drain();
for (const line of lines) console.log(`[ble] ${line}`);
for (const line of uart.split('\n').filter(Boolean)) console.log(`uart: ${line}`);
