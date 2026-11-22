// Real public runtime -> Worker -> WASM -> Xtensa -> GPIO/GP-SPI -> display.
// Requires built web/wasm/esp32sim.wasm and the configured official S3 ROM.
// Optional ESP32SIM_TEST_ROOT runs against another integrated checkout; every
// product/runtime/asset still comes from that checkout's actual catalog.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {resolve, basename, sep} from 'node:path';
import {pathToFileURL} from 'node:url';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {LILYGO_S3_PROBES, lilygoS3Probe} from './fixtures/lilygo-s3-probe.mjs';

const root = process.env.ESP32SIM_TEST_ROOT
  ? pathToFileURL(resolve(process.env.ESP32SIM_TEST_ROOT) + sep) : new URL('../', import.meta.url);
const {createPlatformRuntime} = await import(new URL('web/platform/runtime.js', root));
const {createDefaultScene} = await import(new URL('web/platform/catalog.js', root));
const {validateAppImage} = await import(new URL('web/platform/app-image.js', root));
const {createJitHost} = await import(new URL('web/wasm/jit.mjs', root));
const catalog = Object.fromEntries(await Promise.all(['products', 'accessories', 'firmwares'].map(async key =>
  [key, JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`, root))).items])));
const origin = 'https://lilygo-s3.invalid';
const serial = state => state.serialByPort?.uart0 || '';
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));

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

test('all 19 explicit LilyGO S3 display boards construct in the real WASM build', async () => {
  const profiles = JSON.parse(await readFile(new URL('docs/platform/research/lilygo-runtime-profiles-2026-11-22.json', root))).items;
  assert.equal(profiles.length, 19);
  let wasm;
  const messages = [];
  const jit = createJitHost(() => wasm);
  const {instance} = await WebAssembly.instantiate(await readFile(new URL('web/wasm/esp32sim.wasm', root)), {
    env: {...jit.imports, host_log: (ptr, len) => messages.push(new TextDecoder().decode(new Uint8Array(wasm.memory.buffer, ptr, len)))},
  });
  wasm = instance.exports;
  for (const profile of profiles) {
    const product = catalog.products.find(item => item.id === profile.productId);
    assert(product, profile.productId + ' missing from actual catalog');
    assert.equal(product.runtime.board, profile.boardId);
    const name = new TextEncoder().encode(product.runtime.board);
    const ptr = wasm.esp32sim_alloc(name.length);
    let emulator;
    try {
      new Uint8Array(wasm.memory.buffer).set(name, ptr);
      emulator = wasm.esp32sim_new(ptr, name.length, product.runtime.flashMb, product.runtime.psramMb);
      assert(emulator, product.id + ': ' + messages.join('\n'));
    } finally {
      if (emulator) wasm.esp32sim_delete(emulator);
      wasm.esp32sim_free(ptr, name.length);
    }
  }
});

test('three real S3 guest images drive full frames and public button input', {timeout: 180000}, async () => {
  const requests = [], images = new Map();
  const runtime = createPlatformRuntime({catalog, workerFactory, baseUrl: origin + '/platform/', assetBaseUrl: origin + '/', fetchImpl: async value => {
    const url = new URL(value); assert.equal(url.origin, origin); requests.push(url.pathname);
    if (images.has(url.pathname)) return new Response(images.get(url.pathname));
    let bytes;
    try {bytes = await readFile(new URL('web' + url.pathname, root));}
    catch (error) {
      if (error.code !== 'ENOENT' || !process.env.ESP32SIM_ROM_DIR || !url.pathname.endsWith('esp32s3_rev0_rom.elf')) throw error;
      bytes = await readFile(resolve(process.env.ESP32SIM_ROM_DIR, basename(url.pathname)));
    }
    return new Response(bytes);
  }});
  try {
    for (const probe of LILYGO_S3_PROBES) {
      const product = catalog.products.find(item => item.id === probe.id);
      assert(product, probe.id + ' missing from actual catalog');
      assert.equal(product.runtime.board, probe.id);
      assert.deepEqual([product.runtime.flashMb, product.runtime.psramMb], [16, 8]);
      assert.deepEqual([product.display.width, product.display.height], [probe.width, probe.height]);
      assert.equal(product.controls.find(control => control.id === probe.button)?.pin, probe.buttonPin);
      const scene = createDefaultScene(catalog, probe.id); scene.firmware = null;
      await runtime.configure(scene);
      const image = lilygoS3Probe(probe.id);
      assert.deepEqual(await validateAppImage(image, probe.id, 16 << 20), {imageBytes: image.length, chipId: 9});
      const imagePath = '/original-probes/' + probe.id + '.bin';
      images.set(imagePath, image);
      const sha256 = createHash('sha256').update(image).digest('hex');
      // S3's public app-direct route accepts raw URLs; local-file app-direct is
      // currently reserved for C5. This adapter serves our exact authored bytes.
      await runtime.loadFirmware({url: origin + imagePath, sha256, bootMode: 'app-direct'});
      await runtime.start();
      await waitFor(runtime, probe.id + ' initial guest frame', state => serial(state).includes('FRAME COMPLETE\n') && state.frame);

      async function checkFrame(color) {
        const expected = [...color, 255], deadline = Date.now() + 5000;
        let frame;
        do {
          frame = await runtime.capture({format: 'rgba'});
          if (frame.pixels.every((value, index) => value === expected[index % 4])) break;
          await delay(20);
        } while (Date.now() < deadline);
        assert.deepEqual([frame.width, frame.height], [probe.width, probe.height], probe.id);
        const wrong = frame.pixels.findIndex((value, index) => value !== expected[index % 4]);
        assert.equal(wrong, -1, probe.id + ' incomplete RGB framebuffer at byte ' + wrong);
      }
      await checkFrame([255, 0, 0]);
      const started = await runtime.observe();
      assert.equal(started.firmware.appDirect, true);
      assert.equal(started.firmware.source, 'remote');
      assert.deepEqual(started.firmware.stubs, []);
      assert.equal(started.firmware.artifacts.find(artifact => artifact.kind === 'app')?.sha256, sha256);
      for (const [pressed, marker, color] of [[true, 'BUTTON DOWN\n', [0, 255, 0]], [false, 'BUTTON UP\n', [255, 0, 0]]]) {
        const before = serial(await runtime.observe()).length;
        await runtime.input({type: 'button', id: probe.button, pressed});
        await waitFor(runtime, probe.id + ' fresh GPIO input', state => serial(state).slice(before).includes(marker) && serial(state).slice(before).includes('FRAME COMPLETE\n'));
        await checkFrame(color);
      }
      await runtime.pause();
      const paused = await runtime.observe(); await delay(50);
      assert.equal((await runtime.observe()).stats.time, paused.stats.time);
      await runtime.reset();
      await waitFor(runtime, probe.id + ' same image after reset', state => serial(state).startsWith('LILYGO S3 SRAM READY\n') && serial(state).includes('FRAME COMPLETE\n') && state.frame);
      await checkFrame([255, 0, 0]);
      await runtime.pause();
    }
    assert(requests.some(path => path.endsWith('esp32sim.wasm')));
    assert(requests.some(path => path.endsWith('esp32s3_rev0_rom.elf')));
    assert(requests.every(path => images.has(path) || /\/(esp32sim\.wasm|esp32s3_rev0_rom\.elf)$/.test(path)), 'No factory firmware or substituted ROM may enter the test');
  } finally {await runtime.dispose();}
});
