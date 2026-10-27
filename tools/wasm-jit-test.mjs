#!/usr/bin/env node
// Build with: tools/wasm-jit-test.sh
import { readFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { createJitHost } from '../web/wasm/jit.mjs';
let w;
const host = createJitHost(() => w);
const bytes = readFileSync(new URL('../target/wasm32-unknown-unknown/release/esp32sim_wasm.wasm', import.meta.url));
w = (await WebAssembly.instantiate(bytes, {env: {...host.imports, host_profile_now: () => performance.now(), host_log(p,n) {
  console.error(new TextDecoder().decode(new Uint8Array(w.memory.buffer,p,n)));
}}})).instance.exports;
const waveform = process.argv[2] === 'gpio-waveform';
const grouped = process.argv[2] === '--group';
if (process.argv[2] && !waveform && !grouped) throw Error('usage: wasm-jit-test.mjs [gpio-waveform | --group INDEX]');
if (!waveform && !grouped) {
  // In one long WASM call, releasing table slots and explicit GC did not bound
  // V8 heap growth (docs/evidence/wasm-jit-test-isolation-2026-10-27). Fresh
  // processes bound test memory without changing the production JIT host.
  const groups = w.esp32sim_test_block_jit_group_count();
  if (!Number.isInteger(groups) || groups < 1 || groups > 256) throw Error('invalid JIT test group count');
  let cases = 0, compiled = 0;
  for (let group = 0; group < groups; group++) {
    const child = spawnSync(process.execPath, [...process.execArgv, fileURLToPath(import.meta.url), '--group', String(group)], {encoding:'utf8', maxBuffer:4*1024*1024});
    if (child.status !== 0 || child.error || child.signal) throw Error(`JIT group ${group} failed (${child.error || child.signal || child.status})\n${child.stdout || ''}${child.stderr || ''}`);
    const lines = child.stdout.trim().split('\n');
    const result = JSON.parse(lines.at(-1));
    if (result.group !== group || !Number.isInteger(result.cases) || result.cases < 1 || !Number.isInteger(result.compiled) || result.compiled < 0 || result.compiled !== result.released) throw Error(`incomplete JIT group ${group}`);
    cases += result.cases; compiled += result.compiled;
    console.log(`PASS group ${group + 1}/${groups}: ${result.cases} cases; ${result.compiled} modules released; peak RSS ${result.maxRssKiB} KiB`);
  }
  if (!compiled) throw Error('suite never compiled generated modules');
  console.log(`PASS: ${cases} WASM differential cases across ${groups} isolated groups; ${compiled} compiled modules released`);
  process.exit(0);
}
const group = grouped ? Number(process.argv[3]) : null;
if (grouped && (!Number.isInteger(group) || group < 0 || group >= w.esp32sim_test_block_jit_group_count())) throw Error('invalid JIT test group');
const count = waveform ? w.esp32sim_test_gpio_waveform() : w.esp32sim_test_block_jit_group(group);
if (!count || host.stats.failed || (waveform && !host.stats.compiled) || host.stats.compiled !== host.stats.released) {
  throw Error(`incomplete JIT test run: ${JSON.stringify(host.stats)}`);
}
console.log(waveform ? `PASS: ${count} GPIO waveform cases; ${host.stats.compiled} compiled modules released` : JSON.stringify({group, cases:count, compiled:host.stats.compiled, released:host.stats.released, maxRssKiB:process.resourceUsage().maxRSS}));
