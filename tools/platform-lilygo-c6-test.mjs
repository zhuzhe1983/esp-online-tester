// Public runtime -> Worker -> WASM -> RV32I guest -> physical GPIO/SPI -> GC9107.
// Requires built WASM, integrated C6 products and the unchanged official C6 ROM.
// ESP32SIM_TEST_ROOT optionally selects another actual integrated checkout;
// no catalog entries, ROM bytes, board pins or framebuffer data are substituted.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {resolve, basename, sep} from 'node:path';
import {pathToFileURL} from 'node:url';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {LILYGO_C6_PROBE, lilygoC6Probe} from './fixtures/lilygo-c6-probe.mjs';

const root = process.env.ESP32SIM_TEST_ROOT
  ? pathToFileURL(resolve(process.env.ESP32SIM_TEST_ROOT) + sep) : new URL('../', import.meta.url);
const {createPlatformRuntime} = await import(new URL('web/platform/runtime.js', root));
const {createDefaultScene} = await import(new URL('web/platform/catalog.js', root));
const {validateAppImage} = await import(new URL('web/platform/app-image.js', root));
const {createJitHost} = await import(new URL('web/wasm/jit.mjs', root));
const catalog = Object.fromEntries(await Promise.all(['products', 'accessories', 'firmwares'].map(async key =>
  [key, JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`, root))).items])));
const origin = 'https://lilygo-c6.invalid';
const serial = state => state.serialByPort?.uart0 || '';
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
// Same official esp-rom-elfs 20260528 digest pinned in tools/fetch-rom-elfs.sh.
const ROM_SHA256 = '788e1d38724aeb8fd974fa10c4a7b089c02627d35342ce84b9e0b12b239f3551';

async function waitFor(runtime, label, predicate) {
  const deadline = Date.now() + 45000;
  let state;
  while (Date.now() < deadline) {
    state = await runtime.observe();
    if (predicate(state)) return state;
    if (state.status === 'error') throw new Error(label + ': ' + JSON.stringify(state.lastError));
    await delay(15);
  }
  throw new Error(label + ' timed out: ' + JSON.stringify({status: state?.status, stats: state?.stats, serial: serial(state || {}), error: state?.lastError}));
}

const workerFactory = url => {
  const real = new URL('web' + url.pathname, root);
  const source = `import {parentPort} from 'node:worker_threads';
globalThis.onmessage=null;
globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);
await import(${JSON.stringify(String(real))});
parentPort.on('message',data=>globalThis.onmessage({data}));`;
  const thread = new Worker(new URL('data:text/javascript,' + encodeURIComponent(source)));
  const adapter = {postMessage: (...args) => thread.postMessage(...args), terminate: () => thread.terminate()};
  thread.on('message', data => adapter.onmessage?.({data}));
  thread.on('error', error => adapter.onerror?.(error));
  return adapter;
};

test('all three exact LilyGO C6 boards construct in the real WASM with 4/0 MB', async () => {
  const ids = ['lilygo-t7-c6', 'lilygo-t-knob', LILYGO_C6_PROBE.id];
  let wasm;
  const messages = [], jit = createJitHost(() => wasm);
  const {instance} = await WebAssembly.instantiate(await readFile(new URL('web/wasm/esp32sim.wasm', root)), {
    env: {...jit.imports, host_log: (ptr, len) => messages.push(new TextDecoder().decode(new Uint8Array(wasm.memory.buffer, ptr, len)))},
  });
  wasm = instance.exports;
  for (const id of ids) {
    const product = catalog.products.find(item => item.id === id);
    assert(product, id + ' missing from actual catalog');
    assert.equal(product.runtime.board, id);
    assert.deepEqual([product.runtime.flashMb, product.runtime.psramMb], [4, 0]);
    const name = new TextEncoder().encode(id), ptr = wasm.esp32sim_alloc(name.length);
    let emulator;
    try {
      new Uint8Array(wasm.memory.buffer).set(name, ptr);
      emulator = wasm.esp32sim_new(ptr, name.length, product.runtime.flashMb, product.runtime.psramMb);
      assert(emulator, id + ': ' + messages.join('\n'));
    } finally {
      if (emulator) wasm.esp32sim_delete(emulator);
      wasm.esp32sim_free(ptr, name.length);
    }
  }
});

test('real C6 SRAM guest paints every GC9107 pixel and responds to public BOOT input', {timeout: 120000}, async () => {
  const requests = [], probe = LILYGO_C6_PROBE, image = lilygoC6Probe();
  const imagePath = '/original-probes/lilygo-t-qt-c6.bin';
  const runtime = createPlatformRuntime({catalog, workerFactory, baseUrl: origin + '/platform/', assetBaseUrl: origin + '/', fetchImpl: async value => {
    const url = new URL(value); assert.equal(url.origin, origin); requests.push(url.pathname);
    if (url.pathname === imagePath) return new Response(image);
    assert(/\/(esp32sim\.wasm|esp32c6_rev0_rom\.elf)$/.test(url.pathname), 'Unexpected firmware/ROM request: ' + url.pathname);
    let bytes;
    try {bytes = await readFile(new URL('web' + url.pathname, root));}
    catch (error) {
      if (error.code !== 'ENOENT' || !process.env.ESP32SIM_ROM_DIR || !url.pathname.endsWith('esp32c6_rev0_rom.elf')) throw error;
      bytes = await readFile(resolve(process.env.ESP32SIM_ROM_DIR, basename(url.pathname)));
    }
    if (url.pathname.endsWith('esp32c6_rev0_rom.elf')) assert.equal(createHash('sha256').update(bytes).digest('hex'), ROM_SHA256, 'Keep the configured official C6 ROM unchanged');
    return new Response(bytes);
  }});
  try {
    const product = catalog.products.find(item => item.id === probe.id);
    assert(product, 'T-QT-C6 missing from actual catalog');
    assert.equal(product.runtime.board, probe.id);
    assert.deepEqual([product.runtime.flashMb, product.runtime.psramMb], [4, 0]);
    assert.deepEqual([product.display.width, product.display.height], [128, 128]);
    assert.equal(product.controls.find(control => control.id === probe.button)?.pin, probe.buttonPin);
    const scene = createDefaultScene(catalog, probe.id); scene.firmware = null;
    await runtime.configure(scene);
    assert.deepEqual(await validateAppImage(image, probe.id, 4 << 20), {imageBytes: image.length, chipId: 13});
    const sha256 = createHash('sha256').update(image).digest('hex');
    await runtime.loadFirmware({url: origin + imagePath, sha256, bootMode: 'app-direct'});
    await runtime.start();
    await waitFor(runtime, 'C6 initial guest frame', state => serial(state).startsWith('LILYGO C6 SRAM READY\n') && serial(state).includes('FRAME COMPLETE\n') && state.frame);

    async function checkFrame(color) {
      const expected = [...color, 255], deadline = Date.now() + 5000;
      let frame;
      do {
        frame = await runtime.capture({format: 'rgba'});
        if (frame.pixels.every((value, index) => value === expected[index % 4])) break;
        await delay(20);
      } while (Date.now() < deadline);
      assert.deepEqual([frame.width, frame.height], [128, 128]);
      assert.equal(frame.pixels.length, 128 * 128 * 4);
      const wrong = frame.pixels.findIndex((value, index) => value !== expected[index % 4]);
      assert.equal(wrong, -1, 'Incomplete guest RGB framebuffer at byte ' + wrong);
    }
    await checkFrame([255, 0, 0]);
    const started = await runtime.observe();
    assert.equal(started.firmware.appDirect, true);
    assert.equal(started.firmware.source, 'remote');
    assert.deepEqual(started.firmware.stubs, []);
    assert.equal(started.firmware.artifacts.find(artifact => artifact.kind === 'app')?.sha256, sha256);
    assert.equal(started.firmware.artifacts.find(artifact => artifact.kind === 'rom')?.sha256, ROM_SHA256);
    for (const [pressed, marker, color] of [[true, 'BOOT DOWN\n', [0, 255, 0]], [false, 'BOOT UP\n', [255, 0, 0]]]) {
      const before = serial(await runtime.observe()).length;
      await runtime.input({type: 'button', id: probe.button, pressed});
      await waitFor(runtime, 'C6 fresh GPIO input', state => serial(state).slice(before).includes(marker) && serial(state).slice(before).includes('FRAME COMPLETE\n'));
      await checkFrame(color);
    }
    await runtime.pause();
    const paused = await runtime.observe(); await delay(50);
    assert.equal((await runtime.observe()).stats.time, paused.stats.time);
    await runtime.reset();
    await waitFor(runtime, 'C6 same image after reset', state => serial(state).startsWith('LILYGO C6 SRAM READY\n') && serial(state).includes('FRAME COMPLETE\n') && state.frame);
    await checkFrame([255, 0, 0]);
    await runtime.pause();
    assert(requests.some(path => path.endsWith('esp32sim.wasm')));
    assert(requests.some(path => path.endsWith('esp32c6_rev0_rom.elf')));
    assert(requests.includes(imagePath));
    assert(requests.every(path => path === imagePath || /\/(esp32sim\.wasm|esp32c6_rev0_rom\.elf)$/.test(path)), 'Only the authored image, real WASM and unchanged C6 ROM may be fetched');
  } finally {await runtime.dispose();}
});
