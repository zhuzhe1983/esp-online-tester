// Public runtime -> real Worker -> WASM -> Xtensa -> GPIO/SPI3 -> framebuffer.
// Runs an original SRAM diagnostic, not CloudZao's official release firmware.
// The probe exercises the display and BOOT GPIO only; it does not test touch.
// Requires a built web/wasm/esp32sim.wasm and the official ESP32-S3 ROM ELF.
// ESP32SIM_TEST_ROOT may select an integrated checkout; ESP32SIM_ROM_DIR may
// supply its ROM directory when the ROM is absent from web/wasm/fw/.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {resolve, basename, sep} from 'node:path';
import {pathToFileURL} from 'node:url';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {cloudzaoClaw154Probe} from './fixtures/cloudzao-claw154-probe.mjs';

const root = process.env.ESP32SIM_TEST_ROOT
  ? pathToFileURL(resolve(process.env.ESP32SIM_TEST_ROOT) + sep) : new URL('../', import.meta.url);
const {createPlatformRuntime} = await import(new URL('web/platform/runtime.js', root));
const {createDefaultScene} = await import(new URL('web/platform/catalog.js', root));
const {validateAppImage} = await import(new URL('web/platform/app-image.js', root));
const catalog = Object.fromEntries(await Promise.all(['products', 'accessories', 'firmwares'].map(async key =>
  [key, JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`, root))).items])));
const productId = 'cloudzao-claw-154', width = 240, height = 240;
const origin = 'https://cloudzao-probe.invalid';
const appPath = '/original-probes/cloudzao-claw154.bin';
const wasmPath = '/wasm/esp32sim.wasm', romPath = '/wasm/fw/esp32s3_rev0_rom.elf';
const serial = state => state.serialByPort?.uart0 || '';
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));

function selectedProduct() {
  const product = catalog.products.find(item => item.id === productId);
  assert(product, `${productId} is missing from the actual integrated catalog`);
  return product;
}

async function waitFor(runtime, label, predicate) {
  const deadline = Date.now() + 45000;
  let state;
  while (Date.now() < deadline) {
    state = await runtime.observe();
    if (predicate(state)) return state;
    if (state.status === 'error') throw new Error(label + ': ' + JSON.stringify(state.lastError));
    await delay(15);
  }
  throw new Error(label + ' timed out: ' + JSON.stringify({status: state?.status, stats: state?.stats,
    serial: serial(state || {}), error: state?.lastError}));
}

async function checkFrame(runtime, color) {
  const expected = [...color, 255], deadline = Date.now() + 5000;
  let frame, wrong;
  do {
    frame = await runtime.capture({format: 'rgba'});
    assert.deepEqual([frame.width, frame.height], [width, height]);
    assert.equal(frame.pixels.length, width * height * 4, 'Capture must contain every RGBA pixel');
    wrong = frame.pixels.findIndex((value, index) => value !== expected[index % 4]);
    if (wrong === -1) return;
    await delay(20);
  } while (Date.now() < deadline);
  assert.equal(wrong, -1, `Incomplete guest RGB framebuffer at byte ${wrong}`);
}

test('Claw1.54 catalog selects its own 16/8 MiB S3 board and original diagnostic image', async () => {
  const product = selectedProduct();
  assert.equal(product.chip, 'esp32s3');
  assert.equal(product.runtime.board, productId);
  assert.deepEqual([product.runtime.flashMb, product.runtime.psramMb], [16, 8]);
  assert.deepEqual([product.display.width, product.display.height], [width, height]);
  assert.equal(product.controls.find(control => control.id === 'boot')?.pin, 0);
  const image = cloudzaoClaw154Probe();
  assert.deepEqual(await validateAppImage(image, productId, 16 << 20), {imageBytes: image.length, chipId: 9});
  const projectName = 'cloudzao-claw154-probe';
  assert.equal(image.subarray(32 + 48, 32 + 48 + projectName.length).toString(), projectName);
});

test('Claw1.54 real Worker/WASM draws red, BOOT green/released red, pauses and resets', {timeout: 180000}, async () => {
  const product = selectedProduct(), image = cloudzaoClaw154Probe(), appSha = sha256(image);
  const reads = [], sent = [];
  const workerFactory = url => {
    assert.equal(url.origin, origin);
    assert.equal(url.pathname, '/wasm/worker.js');
    const real = new URL('web' + url.pathname, root);
    const source = `import {parentPort} from 'node:worker_threads';
globalThis.onmessage=null;
globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);
await import(${JSON.stringify(String(real))});
parentPort.on('message',data=>globalThis.onmessage({data}));`;
    const thread = new Worker(new URL('data:text/javascript,' + encodeURIComponent(source)));
    const adapter = {
      postMessage(data, ...args) {
        // Observe real messages before ArrayBuffer transfer, without replacing
        // worker logic, guest memory, ROM functions or framebuffer generation.
        sent.push({op: data.op, board: data.board, flashMb: data.flash_mb, psramMb: data.psram_mb,
          appDirect: data.appDirect, kind: data.kind,
          ...(data.op === 'init' ? {sha256: sha256(new Uint8Array(data.wasm))} : {}),
          ...(data.op === 'load' ? {sha256: sha256(new Uint8Array(data.data))} : {})});
        thread.postMessage(data, ...args);
      },
      terminate: () => thread.terminate(),
    };
    thread.on('message', data => adapter.onmessage?.({data}));
    thread.on('error', error => adapter.onerror?.(error));
    return adapter;
  };
  const runtime = createPlatformRuntime({catalog, workerFactory, baseUrl: origin + '/platform/',
    assetBaseUrl: origin + '/', fetchImpl: async value => {
      const url = new URL(value);
      assert.equal(url.origin, origin, 'This test never downloads vendor firmware or network substitutes');
      assert([appPath, wasmPath, romPath].includes(url.pathname), 'Unexpected asset: ' + url.pathname);
      let bytes = image;
      if (url.pathname !== appPath) {
        try {bytes = await readFile(new URL('web' + url.pathname, root));}
        catch (error) {
          if (error.code !== 'ENOENT' || url.pathname !== romPath || !process.env.ESP32SIM_ROM_DIR) throw error;
          bytes = await readFile(resolve(process.env.ESP32SIM_ROM_DIR, basename(url.pathname)));
        }
        assert.deepEqual([...bytes.subarray(0, 4)], url.pathname === wasmPath ? [0, 97, 115, 109] : [127, 69, 76, 70]);
        assert(bytes.length > 1024, 'Expected a real build or ROM, not an empty fixture');
      }
      reads.push({path: url.pathname, bytes: bytes.length, sha256: sha256(bytes)});
      return new Response(bytes);
    }});
  try {
    const scene = createDefaultScene(catalog, productId);
    scene.firmware = null;
    await runtime.configure(scene);
    const configured = await runtime.capabilities();
    assert.equal(configured.backend, 'esp32sim-wasm');
    assert.equal(configured.executable, true);
    assert.equal(configured.execution.mode, 'wasm-jit');
    assert(configured.input.buttons.includes('boot'));
    // This is an advertised-capability check only; the guest does not read CST816S.
    assert.equal(configured.input.touch, !!(product.runtime.touch || product.display.touch));
    assert(configured.unsupported.includes('arbitrary-firmware-compatibility'));
    assert.equal(configured.framebuffer.source, 'emulator');
    assert.equal(configured.framebuffer.available, false);
    for (const method of ['configure', 'loadFirmware', 'start', 'pause', 'reset', 'dispose']) assert(configured.lifecycle.includes(method));

    await runtime.loadFirmware({url: origin + appPath, sha256: appSha, bootMode: 'app-direct'});
    await runtime.start();
    await waitFor(runtime, 'initial guest frame', state => serial(state).startsWith('CLOUDZAO CLAW154 SRAM READY\n')
      && serial(state).includes('FRAME COMPLETE\n') && state.frame);
    await checkFrame(runtime, [255, 0, 0]);
    const started = await runtime.observe();
    assert.equal(started.status, 'running');
    assert.equal(started.firmware.appDirect, true);
    assert.equal(started.firmware.source, 'remote');
    assert.equal(started.scene.firmware.bootMode, 'app-direct');
    assert.deepEqual(started.firmware.stubs, []);
    assert.deepEqual(started.firmware.artifacts.map(artifact => artifact.kind).sort(), ['app', 'rom']);
    assert.equal(started.firmware.artifacts.find(artifact => artifact.kind === 'app')?.sha256, appSha);
    assert.equal((await runtime.capabilities()).framebuffer.available, true);

    for (const [pressed, marker, color] of [[true, 'BUTTON DOWN\n', [0, 255, 0]], [false, 'BUTTON UP\n', [255, 0, 0]]]) {
      const before = serial(await runtime.observe()).length;
      await runtime.input({type: 'button', id: 'boot', pressed});
      await waitFor(runtime, 'fresh BOOT GPIO acknowledgement', state => serial(state).slice(before).includes(marker)
        && serial(state).slice(before).includes('FRAME COMPLETE\n'));
      await checkFrame(runtime, color);
    }
    await waitFor(runtime, 'guest instruction statistics', state => state.stats.insns > 0);
    const paused = await runtime.pause();
    assert.equal(paused.status, 'paused');
    assert(paused.stats.insns > 0);
    await delay(80);
    const stillPaused = await runtime.observe();
    assert.equal(stillPaused.stats.insns, paused.stats.insns, 'Pause must freeze real guest execution');
    assert.equal(stillPaused.stats.time, paused.stats.time);

    await runtime.reset();
    const rebooted = await waitFor(runtime, 'same diagnostic after reset', state => serial(state).startsWith('CLOUDZAO CLAW154 SRAM READY\n')
      && serial(state).includes('FRAME COMPLETE\n') && state.frame);
    await checkFrame(runtime, [255, 0, 0]);
    assert.equal(serial(rebooted).split('CLOUDZAO CLAW154 SRAM READY\n').length - 1, 1, 'Reset must discard the previous console');
    assert.equal(rebooted.firmware.appDirect, true);
    assert.deepEqual(rebooted.firmware.stubs, []);
    assert.equal(rebooted.firmware.artifacts.find(artifact => artifact.kind === 'app')?.sha256, appSha);
    await runtime.pause();

    const wasmRead = reads.find(read => read.path === wasmPath), romRead = reads.find(read => read.path === romPath);
    assert(wasmRead && romRead, 'Both the built WASM and official S3 ROM must be read');
    assert.equal(started.firmware.artifacts.find(artifact => artifact.kind === 'rom')?.sha256, romRead.sha256);
    assert(reads.every(read => [appPath, wasmPath, romPath].includes(read.path)));
    const creates = sent.filter(message => message.op === 'create');
    assert.equal(creates.length, 2, 'Initial load and reset must each construct a real emulator');
    for (const message of creates) assert.deepEqual([message.board, message.flashMb, message.psramMb], [productId, 16, 8]);
    const inits = sent.filter(message => message.op === 'init'), starts = sent.filter(message => message.op === 'start');
    assert.equal(inits.length, 2);
    assert(inits.every(message => message.sha256 === wasmRead.sha256), 'Worker must receive the actual read WASM');
    assert.equal(starts.length, 2);
    assert(starts.every(message => message.appDirect === true));
    assert.equal(sent.filter(message => message.op === 'stub').length, 0, 'No ROM stubs may enter this probe');
    for (const [kind, hash] of [[0, romRead.sha256], [3, appSha]]) {
      const loads = sent.filter(message => message.op === 'load' && message.kind === kind);
      assert.equal(loads.length, 2);
      assert(loads.every(message => message.sha256 === hash), 'Worker must receive exact authored app and ROM bytes');
    }
  } finally {await runtime.dispose();}
});
