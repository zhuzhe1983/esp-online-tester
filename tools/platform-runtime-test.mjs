#!/usr/bin/env node
// Boundary checks plus the actual browser worker/WASM protocol in a Node Worker.
// Build web/wasm/esp32sim.wasm and fetch demo ROMs before running this test.
import assert from 'node:assert/strict';
import { readFile, mkdtemp, writeFile, rm } from 'node:fs/promises';
import { Worker } from 'node:worker_threads';
import { basename, join } from 'node:path';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';
import { createPlatformRuntime, decodeFramebuffer, createGpsNmea, fetchBytes, resolveFirmwareUrl, sha256, validateMergedImage } from '../web/platform/runtime.js';
import { createDefaultScene, decodeScene } from '../web/platform/catalog.js';
import { installAutomation } from '../web/platform/automation.js';
import { withPlatformTestFixtures, HELLO_PRODUCT_ID, UNAVAILABLE_PRODUCT_ID } from './fixtures/platform-catalog.mjs';

const root = new URL('../', import.meta.url), publicCatalog = {};
for (const name of ['products', 'accessories', 'firmwares']) publicCatalog[name] = JSON.parse(await readFile(new URL(`web/platform/catalog/${name}.json`, root), 'utf8')).items;
const baseUrl = 'https://platform.example/platform/index.html';
assert.equal(publicCatalog.products.some(product => product.id === HELLO_PRODUCT_ID || product.id === 'generic-esp32s3'), false);
const catalog = await withPlatformTestFixtures(publicCatalog, baseUrl);
const workerInputDelay = Number(process.env.ESP32SIM_TEST_WORKER_DELAY_MS || 0);
assert.ok(Number.isFinite(workerInputDelay) && workerInputDelay >= 0 && workerInputDelay <= 5000, 'ESP32SIM_TEST_WORKER_DELAY_MS must be 0…5000');
const responseOverrides = new Map();
const readArtifactFile = async (path, romDir = process.env.ESP32SIM_ROM_DIR) => {
  try { return await readFile(path); }
  catch (e) {
    const name = basename(path instanceof URL ? fileURLToPath(path) : path);
    if (e.code === 'ENOENT' && romDir && name.endsWith('_rom.elf')) return readFile(join(romDir, name));
    throw e;
  }
};
const fileFetch = async value => {
  if (responseOverrides.has(String(value))) return new Response(responseOverrides.get(String(value)));
  const url = new URL(value), path = url.protocol === 'file:' ? url : new URL(`web${url.pathname}`, root);
  try { const bytes = await readArtifactFile(path); return new Response(bytes, { headers: { 'content-length': String(bytes.length) } }); }
  catch { return new Response('Missing test artifact', { status: 404 }); }
};
const workerFactory = url => {
  if (url.protocol !== 'file:') url = new URL(`web${url.pathname}`, root);
  const script = `import { parentPort } from 'node:worker_threads';
    globalThis.onmessage = null;
    globalThis.postMessage = (message, transfer) => parentPort.postMessage(message, transfer);
    await import(${JSON.stringify(String(url))});
    const inputDelay = ${workerInputDelay};
    const sleeper = new Int32Array(new SharedArrayBuffer(4));
    parentPort.on('message', data => {
      if (data.op === 'text' && inputDelay > 0) Atomics.wait(sleeper, 0, 0, inputDelay);
      globalThis.onmessage({data});
    });`;
  const thread = new Worker(new URL(`data:text/javascript,${encodeURIComponent(script)}`));
  const adapter = { postMessage: (...args) => thread.postMessage(...args), terminate: () => thread.terminate() };
  thread.on('message', data => adapter.onmessage?.({ data }));
  thread.on('error', event => adapter.onerror?.(event));
  return adapter;
};
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const expectCode = async (operation, code) => assert.rejects(operation, e => e.code === code, `expected ${code}`);

// Exercise the CI layout without moving the checkout's ROMs while another task may serve them.
const romDirectory = await mkdtemp(join(tmpdir(), 'esp32sim-platform-rom-'));
try {
  const rom = await readArtifactFile(new URL('web/wasm/fw/esp32s3_rev0_rom.elf', root));
  await writeFile(join(romDirectory, 'esp32s3_rev0_rom.elf'), rom);
  assert.deepEqual(await readArtifactFile(join(romDirectory, 'absent-site-copy', 'esp32s3_rev0_rom.elf'), romDirectory), rom);
  await assert.rejects(() => readArtifactFile(join(romDirectory, 'absent.bin'), romDirectory), { code: 'ENOENT' });
} finally { await rm(romDirectory, { recursive: true, force: true }); }
console.log('ok CI ROM lookup: missing site ROM falls back to the supplied directory; other files do not');

assert.equal(resolveFirmwareUrl('../firmware.bin', baseUrl), 'https://platform.example/firmware.bin');
assert.throws(() => resolveFirmwareUrl('javascript:alert(1)', baseUrl), { code: 'INVALID_URL' });
assert.throws(() => resolveFirmwareUrl('http://example.com/fw.bin', baseUrl), { code: 'MIXED_CONTENT' });
assert.throws(() => resolveFirmwareUrl('https://user:secret@example.com/fw.bin', baseUrl), { code: 'INVALID_URL' });
await expectCode(() => fetchBytes('https://example.com/no-cors.bin', { fetchImpl: async () => { throw new TypeError('Failed to fetch'); } }), 'FETCH_FAILED');
await expectCode(() => fetchBytes('https://example.com/large.bin', { maxBytes: 3, fetchImpl: async () => new Response(new Uint8Array(4)) }), 'FILE_TOO_LARGE');
await expectCode(() => fetchBytes('https://example.com/absent.bin', { fetchImpl: async () => new Response('', { status: 404 }) }), 'HTTP_ERROR');
assert.equal(await sha256(new TextEncoder().encode('abc')), 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad');

// Actual ReadableStream chunks, without network buffering or fabricated percentages.
const streamed = (headers = {}) => new Response(new ReadableStream({
  start(controller) { controller.enqueue(new Uint8Array([1, 2])); controller.enqueue(new Uint8Array([3, 4, 5])); controller.close(); },
}), { headers });
for (const [headers, expectedTotal] of [[{'content-length':'5'},5], [{},null], [{'content-length':'2','content-encoding':'gzip'},null]]) {
  const events = [];
  assert.deepEqual([...await fetchBytes('https://example.com/chunks.bin', { fetchImpl: async () => streamed(headers), onProgress: event => events.push(event) })], [1,2,3,4,5]);
  assert.deepEqual(events.map(event => event.loadedBytes), [0,2,5,5]);
  assert.ok(events.every(event => event.totalBytes === expectedTotal));
  assert.deepEqual(events.map(event => event.done), [false,false,false,true]);
}
const abortedEvents = [], streamAbort = new AbortController(); let streamCancelled = false;
await expectCode(() => fetchBytes('https://example.com/cancel.bin', {
  signal: streamAbort.signal,
  fetchImpl: async () => new Response(new ReadableStream({ start(controller) { controller.enqueue(new Uint8Array([1,2])); }, cancel() { streamCancelled = true; } })),
  onProgress(event) { abortedEvents.push(event); if (event.loadedBytes === 2) streamAbort.abort(); },
}), 'FETCH_ABORTED');
assert.equal(streamCancelled, true); assert.equal(abortedEvents.some(event => event.done), false);
const failedEvents = []; let pulls = 0;
await expectCode(() => fetchBytes('https://example.com/broken.bin', {
  fetchImpl: async () => new Response(new ReadableStream({ pull(controller) { if (pulls++ === 0) controller.enqueue(new Uint8Array([1])); else controller.error(new Error('Transport disconnected')); } })),
  onProgress: event => failedEvents.push(event),
}), 'FETCH_FAILED');
assert.equal(failedEvents.some(event => event.done), false);
console.log('ok streaming download progress: known/unknown/encoded totals, chunk bytes, completion, abort cancellation and transport failure');

const wireFrame = new Uint8Array([1, 2, 0, 1, 0, 0, 248, 224, 7]);
const decoded = decodeFramebuffer(wireFrame);
assert.deepEqual([decoded.width, decoded.height], [2, 1]);
assert.deepEqual([...decoded.pixels], [255, 0, 0, 255, 0, 255, 0, 255]);
assert.throws(() => decodeFramebuffer(wireFrame.slice(0, -1)), { code: 'INVALID_FRAME' });
assert.throws(() => validateMergedImage(new Uint8Array([0xe9, 1]), 'stopwatch', 16 * 1024 * 1024), { code: 'INCOMPLETE_IMAGE' });
const merged = new Uint8Array(0x10100), mergedView = new DataView(merged.buffer);
for (const at of [0, 0x10000]) { merged[at] = 0xe9; merged[at + 1] = 1; merged[at + 12] = 9; mergedView.setUint32(at + 28, 4, true); }
merged[0x8000] = 0xaa; merged[0x8001] = 0x50;
mergedView.setUint32(0x8004, 0x10000, true); mergedView.setUint32(0x8008, 0x100000, true);
assert.equal(validateMergedImage(merged, 'stopwatch', 16 * 1024 * 1024), true);
assert.throws(() => validateMergedImage(merged.slice(0, 0x10020), 'stopwatch', 16 * 1024 * 1024), { code: 'INCOMPLETE_IMAGE' });
merged[12] = 5;
assert.throws(() => validateMergedImage(merged, 'stopwatch', 16 * 1024 * 1024), { code: 'CHIP_MISMATCH' });

const nmea = createGpsNmea({ latitude: 31.2304, longitude: 121.4737, timestamp: '2026-01-02T03:04:05Z' });
assert.match(nmea, /^\$GPRMC,030405\.00,A,3113\.82400,N,12128\.42200,E,/);
for (const line of nmea.trim().split('\r\n')) {
  const [body, checksum] = line.slice(1).split('*');
  assert.equal([...body].reduce((sum, char) => sum ^ char.charCodeAt(0), 0), parseInt(checksum, 16));
}
assert.match(createGpsNmea({ latitude: 89.9999999999, longitude: -179.9999999999 }), /9000\.00000,N,18000\.00000,W/);
assert.throws(() => createGpsNmea({ latitude: NaN, longitude: 0 }), { code: 'INVALID_GPS_FIX' });
console.log('ok platform boundaries: URL/CORS/size/hash, RGB565, merged-image rejection, GPS checksums');

const loadEvents = [];
const runtime = createPlatformRuntime({ catalog, fetchImpl: fileFetch, workerFactory, baseUrl, assetBaseUrl: 'https://platform.example/', onEvent: event => { if (event.type === 'load-progress') loadEvents.push(event); } });
const target = { location: { href: baseUrl } }, api = installAutomation(runtime, { target });
assert.equal(target.espTester.version, '1.0.0');
let currentStep = 'initial validation';
const step = async (name, work) => {
  currentStep = name; console.log(`check ${name}`);
  const result = await work(); console.log(`ok ${name}`); return result;
};
const waitForStats = (predicate, timeoutMs = 30000) => new Promise((resolve, reject) => {
  let unsubscribe = () => {};
  const finish = (error, state) => { clearTimeout(timer); unsubscribe(); error ? reject(error) : resolve(state); };
  const timer = setTimeout(() => finish(new Error('Timed out waiting for guest execution to advance.')), timeoutMs);
  const check = async () => { const state = await runtime.observe(); if (predicate(state.stats)) finish(null, state); };
  unsubscribe = runtime.subscribe(event => { if (event.type === 'stats') void check(); });
  void check();
});
const holdUntilSerial = async (down, up, serialIncludes) => {
  // Observe before injecting, then keep the input down until firmware confirms it.
  // A fixed host duration is not evidence that a slow guest sampled the input.
  const confirmation = api.waitFor({ serialIncludes, port: 'uart0', fromNow: true });
  try { await Promise.all([runtime.input(down), confirmation]); }
  finally { await runtime.input(up); }
};
try {
  await expectCode(() => runtime.start(), 'NOT_READY');
  await runtime.configure(createDefaultScene(catalog, UNAVAILABLE_PRODUCT_ID));
  assert.equal((await runtime.capabilities()).executable, false);
  await expectCode(() => runtime.loadFirmware({ manifestUrl: 'https://platform.example/wasm/fw/hello.json' }), 'UNSUPPORTED_PRODUCT');
  await runtime.configure(createDefaultScene(catalog, HELLO_PRODUCT_ID));
  await expectCode(() => runtime.capture(), 'NO_FRAME');
  await expectCode(() => runtime.loadFirmware({ manifestUrl: 'https://platform.example/wasm/fw/hello.json', sha256: '0'.repeat(64) }), 'HASH_MISMATCH');
  const firstLoadProgress = loadEvents.length;
  await runtime.loadFirmware({ manifestUrl: 'https://platform.example/wasm/fw/hello.json' });
  assert.equal((await runtime.observe()).status, 'ready');
  const progress = loadEvents.slice(firstLoadProgress);
  assert.equal(new Set(progress.map(event => event.loadId)).size, 1);
  for (const kind of ['manifest','rom','bootloader','ptable','app','wasm']) assert.ok(progress.some(event => event.phase === 'download' && event.kind === kind && event.done), `Missing completed download for ${kind}`);
  const preparation = progress.at(-1);
  assert.equal(preparation.phase, 'prepare'); assert.ok(progress.some(event=>event.stage==='worker')); assert.equal(preparation.stage, 'device-storage');
  assert.equal(preparation.totalBytes, null); assert.equal(preparation.loadedBytes, null);
  assert.equal(new Set(progress.filter(event => event.phase === 'download').map(event => event.fileIndex)).size, 6);
  const readyProgressCount = loadEvents.length; await delay(5); assert.equal(loadEvents.length, readyProgressCount, 'A ready load must not emit late progress');
  await runtime.start();
  const running = await api.waitFor({ serialIncludes: 'Hello world!', port: 'uart0', timeoutMs: 30000 });
  assert.equal(running.status, 'running');
  await expectCode(() => runtime.input({ type: 'serial', port: 'uart2', data: 'x' }), 'UNSUPPORTED_PORT');
  await expectCode(() => runtime.input({ type: 'button', id: 'button-a', pressed: true }), 'UNSUPPORTED_CONTROL');
  await runtime.updateSceneMetadata({ view: { mode: 'embed', deviceView: '2d' } });
  assert.equal((await runtime.observe()).status, 'running');
  const shared = new URL(await api.share({ mode: 'embed' }));
  assert.equal(shared.pathname, '/platform/embed.html');
  assert.equal(decodeScene(new URLSearchParams(shared.hash.slice(1)).get('scene')).firmware.manifestUrl, 'https://platform.example/wasm/fw/hello.json');
  const paused = await runtime.pause(), pausedInsns = paused.stats.insns;
  assert.ok(pausedInsns > 0, 'Real guest instructions must execute before pause');
  await delay(150);
  assert.equal((await runtime.observe()).stats.insns, pausedInsns, 'Pause must freeze guest execution');
  // Rapid cycles leave queued callbacks from old loop generations; they must never restart a paused worker.
  for (let i = 0; i < 5; i++) { await runtime.start(); await runtime.pause(); }
  const afterRapid = await runtime.observe();
  await delay(150);
  assert.equal((await runtime.observe()).stats.insns, afterRapid.stats.insns);
  await runtime.start();
  const resumed = await waitForStats(stats => stats.insns > pausedInsns);
  assert.ok(resumed.stats.insns > pausedInsns, 'Resume must continue the existing instruction stream');
  assert.equal((resumed.serialByPort.uart0.match(/Hello world!/g) || []).length, 1, 'Resume must not boot a fresh firmware');
  await runtime.reset(); await api.waitFor({ serialIncludes: 'Hello world!', port: 'uart0', timeoutMs: 30000 });
  assert.equal(((await runtime.observe()).serialByPort.uart0.match(/Hello world!/g) || []).length, 1, 'Reset must clear old console and reboot');
  await runtime.pause();
  await expectCode(() => api.waitFor({ serialIncludes: 'Hello world!', fromNow: true, timeoutMs: 25 }), 'WAIT_TIMEOUT');
  console.log('ok real worker/WASM: hello boot, pause freezes instructions, resume preserves machine, reset reboots, share pins hash');

  const stopwatchScene = createDefaultScene(catalog, 'm5stack-stopwatch');
  stopwatchScene.accessories.push({ id: 'gps-test', catalogId: 'm5stack-gps-u032', parameters: { latitude: 31.2304, longitude: 121.4737 }, wiring: { mode: 'logical', port: 'uart1' } });
  await runtime.configure(stopwatchScene);
  await expectCode(() => api.setGpsFix({accessoryId: 'gps-test', latitude: 0, longitude: 0, positionSource: {kind: 'ip-estimate', ip: 'must-not-retain'}}), 'INVALID_INPUT');
  const gpsUpdated = await api.setGpsFix({accessoryId: 'gps-test', latitude: 0, longitude: 0, positionSource: {kind: 'ip-estimate', provider: 'test', city: 'Example'}});
  assert.equal(gpsUpdated.parameters.latitude, 0); assert.equal(gpsUpdated.parameters.positionSource.ip, undefined);
  assert.equal((await runtime.observe()).status, 'configured');
  await expectCode(() => api.setGpsFix({accessoryId: 'gps-test', latitude: 91}), 'INVALID_GPS_FIX');
  await runtime.setGpsFix({accessoryId: 'gps-test', latitude: 31.2304, longitude: 121.4737, positionSource: {kind: 'manual'}});
  await runtime.setImu({quaternion: [Math.SQRT1_2,0,0,Math.SQRT1_2]});
  assert.ok(Math.abs((await runtime.observe()).imu.accelG[2] + 1) < 1e-9);
  await expectCode(() => runtime.setImu({quaternion:[0,0,0,0]}), 'INVALID_IMU');
  await expectCode(() => runtime.loadFirmware({ manifestUrl: 'https://platform.example/wasm/fw/hello.json' }), 'BOARD_MISMATCH');
  const badPins = JSON.parse(await readFile(new URL('web/wasm/fw/demo-stopwatch.json', root), 'utf8'));
  badPins.artifact_sha256 = { ...badPins.artifact_sha256, [badPins.files.app]: '0'.repeat(64) };
  responseOverrides.set('https://platform.example/wasm/fw/bad-pins.json', JSON.stringify(badPins));
  await expectCode(() => runtime.loadFirmware({ manifestUrl: 'https://platform.example/wasm/fw/bad-pins.json' }), 'HASH_MISMATCH');
  await step('StopWatch boot and firmware readiness', async () => {
    await runtime.loadFirmware({ manifestUrl: 'https://platform.example/wasm/fw/demo-stopwatch.json' });
    assert.equal((await runtime.observe()).firmware.executionMode, 'wasm-interpreter');
    assert.equal((await runtime.capabilities()).execution.mode, 'wasm-interpreter');
    await runtime.start();
    await api.waitFor({ serialIncludes: 'PLATFORM_READY stopwatch', port: 'uart0', timeoutMs: 30000 });
  });
  const display = await runtime.capture({ format: 'rgba' });
  const imuReceipt = await runtime.setImu({quaternion:[0,0,0,1], angularVelocityDps:[12,0,0]});
  assert.equal(imuReceipt.delivery, 'register-state'); assert.ok(imuReceipt.guestTime >= 0);
  assert.deepEqual(imuReceipt.accelG, [0,1,0]); assert.deepEqual(imuReceipt.gyroDps, [12,0,0]);
  await runtime.setImu({enabled:false});
  await runtime.setImu({source:'scene',quaternion:[1,0,0,0]});
  assert.deepEqual((await runtime.observe()).imu.quaternion,[0,0,0,1]);
  assert.deepEqual((await runtime.observe()).imu.gyroDps,[0,0,0]);
  await runtime.setImu({enabled:true});
  assert.deepEqual([display.width, display.height], [468, 466]);
  assert.ok(display.pixels.some((value, index) => index % 4 !== 3 && value > 80), 'Firmware must draw actual bright pixels');
  await step('StopWatch button A guest acknowledgement', () => holdUntilSerial(
    { type: 'button', id: 'button-a', pressed: true }, { type: 'button', id: 'button-a', pressed: false }, 'BUTTON_A running'));
  await step('StopWatch button B guest acknowledgement', () => holdUntilSerial(
    { type: 'button', id: 'button-b', pressed: true }, { type: 'button', id: 'button-b', pressed: false }, 'BUTTON_B reset'));
  await step('StopWatch CST820 touch guest acknowledgement', () => holdUntilSerial(
    { type: 'touch', x: 234, y: 233, down: true }, { type: 'touch', x: 234, y: 233, down: false }, 'TOUCH down'));
  await step('paused input cleanup and recordable receipt', async () => {
    await runtime.pause();
    await expectCode(() => runtime.input({ type: 'button', id: 'button-a', pressed: true }), 'NOT_RUNNING');
    const events = [], unsubscribe = runtime.subscribe(event => { if (event.type === 'input') events.push(event); });
    try {
      const release = { type: 'button', id: 'button-a', pressed: false };
      const receipt = await runtime.input(release);
      assert.deepEqual(events[0].event, release);
      assert.equal(events[0].receipt.guestTime, receipt.guestTime);
      assert.equal(events[0].receipt.delivery, 'queued');
      assert.deepEqual(events[0].receipt.frame, { width: 468, height: 466 });
      assert.equal((await runtime.observe()).status, 'paused');
    } finally { unsubscribe(); }
    await runtime.start();
  });
  await expectCode(() => runtime.input({ type: 'touch', x: 468, y: 0, down: true }), 'INVALID_INPUT');
  const injected = await step('StopWatch GPS RMC and GGA reception', async () => {
    const gps = api.waitFor({ serialIncludes: 'GPS_RX $GPGGA', port: 'uart0', fromNow: true });
    const [result] = await Promise.all([runtime.injectGps({ accessoryId: 'gps-test', timestamp: '2026-01-02T03:04:05Z' }), gps]);
    return result;
  });
  assert.equal(injected.delivery, 'logical-uart-rx');
  const gpsState = await runtime.observe();
  assert.ok(Object.values(gpsState.serialByPort).some(text => text.includes('GPS_RX $GPRMC,030405.00,A,3113.82400,N,12128.42200,E')));
  assert.ok((await runtime.capture({ format: 'rgba' })).frameNumber > display.frameNumber, 'Inputs must result in new firmware framebuffer output');
  console.log('ok StopWatch real firmware: 468×466 framebuffer, A/B GPIO, CST820 touch, GPS RMC+GGA through UART1 RX');

  const [boot, table, app] = await Promise.all(['platform-stopwatch-bootloader.bin', 'platform-stopwatch-ptable.bin', 'platform-stopwatch.bin'].map(name => readFile(new URL(`web/wasm/fw/public/${name}`, root))));
  const localImage = new Uint8Array(0x10000 + app.length).fill(255);
  localImage.set(boot, 0); localImage.set(table, 0x8000); localImage.set(app, 0x10000);
  await step('local merged StopWatch firmware readiness', async () => {
    await runtime.loadFirmware({ file: new Blob([localImage]), sha256: await sha256(localImage) });
    await runtime.start(); await api.waitFor({ serialIncludes: 'PLATFORM_READY stopwatch', port: 'uart0', timeoutMs: 30000 });
  });
  await expectCode(() => api.share(), 'LOCAL_FIRMWARE_NOT_SHAREABLE');
  console.log('ok local merged StopWatch flash: complete app validation, SHA-256, automatic ROM load, real boot; file URLs excluded from share');
  const localFiles = [new File([boot], 'bootloader.bin'), new File([table], 'partition-table.bin'), new File([app], 'app.bin')];
  await step('local split StopWatch firmware ROM boot', async () => {
    await runtime.loadFirmware({format:'segments', files:localFiles.map((file,i)=>({file,offset:[0,0x8000,0x10000][i]}))});
    const ready = await runtime.observe();
    assert.equal(ready.firmware.source,'local'); assert.equal(ready.firmware.appDirect,false);
    assert.deepEqual(ready.firmware.localLayout.segments.map(x=>x.offset),[0,0x8000,0x10000]);
    assert.equal(ready.firmware.artifacts[0].url,null);
    await runtime.start(); await api.waitFor({serialIncludes:'PLATFORM_READY stopwatch',port:'uart0',timeoutMs:30000});
    const deadline=Date.now()+30000;
    while (!(await runtime.observe()).frame && Date.now()<deadline) await delay(50);
    assert.ok((await runtime.observe()).frame?.frameNumber > 0, 'Split build must produce a real framebuffer');
    await expectCode(()=>api.share(),'LOCAL_FIRMWARE_NOT_SHAREABLE');
  });
  const argsFile = new File([JSON.stringify({flash_files:{'0x0':'bootloader/bootloader.bin','0x8000':'partition_table/partition-table.bin','0x10000':'app.bin'},extra_esptool_args:{chip:'esp32s3'},flash_settings:{flash_size:'16MB'}})],'flasher_args.json');
  await step('local ESP-IDF build layout ROM boot', async () => {
    await runtime.loadFirmware({format:'build',files:[argsFile,...localFiles]});
    assert.equal((await runtime.observe()).firmware.source,'local');
    await runtime.start(); await api.waitFor({serialIncludes:'PLATFORM_READY stopwatch',port:'uart0',timeoutMs:30000});
    await expectCode(()=>api.share(),'LOCAL_FIRMWARE_NOT_SHAREABLE');
  });
  console.log('ok local split/build flash: actual ROM+bootloader+ptable+app execution, real framebuffer and nonshareable local receipts');

} catch (error) {
  const state = await runtime.observe();
  console.error(`FAIL ${currentStep}: ${error.code || 'ASSERTION'} ${error.message}`);
  console.error(JSON.stringify({ status: state.status, stats: state.stats, frame: state.frame, lastError: state.lastError,
    serialTail: Object.fromEntries(Object.entries(state.serialByPort).map(([port, text]) => [port, text.slice(-2500)])) }, null, 2));
  throw error;
} finally { await runtime.dispose(); }
assert.equal((await runtime.observe()).status, 'disposed');
await expectCode(() => runtime.start(), 'DISPOSED');
console.log('ok platform session disposal');

// Exercise the real runtime cancellation path rather than the recording test's
// stand-in. Cancellation must leave a reusable configured session and no worker.
let releaseFetch, workersCreated = 0;
const beganFetch = new Promise(resolve => { releaseFetch = resolve; });
const cancellingRuntime = createPlatformRuntime({ catalog, baseUrl, fetchImpl: (url, options) => new Promise((resolve, reject) => {
  releaseFetch();
  options.signal.addEventListener('abort', () => reject(new DOMException('Aborted', 'AbortError')), { once: true });
}), workerFactory() { workersCreated++; throw new Error('Cancelled load must not create a worker'); } });
try {
  await cancellingRuntime.configure(createDefaultScene(catalog, 'm5stack-stopwatch'));
  const load = cancellingRuntime.loadFirmware({ manifestUrl: 'https://platform.example/wasm/fw/demo-stopwatch.json' });
  const rejected = expectCode(() => load, 'FETCH_ABORTED');
  await beganFetch;
  await cancellingRuntime.cancelPendingLoad(); await rejected;
  const cancelled = await cancellingRuntime.observe();
  assert.equal(cancelled.status, 'configured'); assert.equal(cancelled.firmware, null); assert.equal(workersCreated, 0);
  await cancellingRuntime.updateSceneMetadata({ view: { mode: 'embed', deviceView: '2d' } });
  assert.equal((await cancellingRuntime.observe()).scene.view.mode, 'embed');
} finally { await cancellingRuntime.dispose(); }
console.log('ok real runtime download cancellation: aborted fetch, no worker/receipt, session remains reusable');

// A queued scene replacement invalidates progress immediately, even if a custom
// fetch implementation finishes the old stream later. No worker is needed here.
let finishOldStream, sawOldChunk;
const firstChunk = new Promise(resolve => { sawOldChunk = resolve; }), staleEvents = [];
const staleRuntime = createPlatformRuntime({ catalog, baseUrl, fetchImpl: async () => new Response(new ReadableStream({
  start(controller) { controller.enqueue(new TextEncoder().encode('{')); finishOldStream = () => { controller.enqueue(new TextEncoder().encode('invalid}')); controller.close(); }; },
})), onEvent(event) { if (event.type === 'load-progress') { staleEvents.push(event); if (event.loadedBytes > 0) sawOldChunk(); } } });
try {
  await staleRuntime.configure(createDefaultScene(catalog,'m5stack-stopwatch'));
  const oldLoad = staleRuntime.loadFirmware({manifestUrl:'https://example.com/old.json'});
  const rejected = expectCode(() => oldLoad, 'INVALID_MANIFEST');
  await firstChunk;
  const count = staleEvents.length;
  const replacement = staleRuntime.configure(createDefaultScene(catalog,'m5stack-papermono'));
  finishOldStream(); await rejected; await replacement;
  assert.equal(staleEvents.length,count,'Scene replacement must suppress old stream and verification progress');
  assert.equal((await staleRuntime.observe()).scene.productId,'m5stack-papermono');
} finally { await staleRuntime.dispose(); }
console.log('ok load progress identity: real manifest/artifact/WASM stages, ready boundary and stale scene-replacement suppression');

// Transport regression only: deliberately deliver old stat messages before a matching
// input receipt. These controlled messages are not firmware execution evidence.
const transport = {
  inputs: [],
  postMessage(message) {
    if (message.op === 'init') queueMicrotask(() => this.emit({ ready: true }));
    else if (message.op === 'create') queueMicrotask(() => this.emit({ created: true }));
    else if (message.op === 'load') queueMicrotask(() => this.emit({ loaded: message.kind, ok: true }));
    else if (message.op === 'start') queueMicrotask(() => this.emit({ started: true }));
    else if (message.op === 'imu') queueMicrotask(() => this.emit({ imuApplied: message.inputId, ok: true, time: 0 }));
    else if (message.op === 'text') this.inputs.push(message);
  },
  emit(data) { this.onmessage?.({ data }); },
  terminate() {},
};
const transportRuntime = createPlatformRuntime({ catalog, fetchImpl: fileFetch, workerFactory: () => transport, baseUrl, assetBaseUrl: 'https://platform.example/' });
const flush = () => new Promise(resolve => setImmediate(resolve));
try {
  const scene = createDefaultScene(catalog, 'm5stack-stopwatch');
  scene.accessories.push({ id: 'gps-contract', catalogId: 'm5stack-gps-u032', parameters: {}, wiring: { mode: 'logical', port: 'uart1' } });
  await transportRuntime.configure(scene);
  await transportRuntime.loadFirmware({ manifestUrl: 'https://platform.example/wasm/fw/demo-stopwatch.json' });
  await transportRuntime.start();
  const pending = transportRuntime.injectGps({ accessoryId: 'gps-contract', latitude: 31.2304, longitude: 121.4737, timestamp: '2026-01-02T03:04:05Z' });
  // Attach a rejection handler immediately so a regression cannot escape the final cleanup.
  const outcome = pending.then(value => ({ value }), error => ({ error }));
  await flush();
  const count = Math.ceil(nmea.length / 64);
  for (let index = 0; index < count; index++) {
    assert.equal(transport.inputs.length, index + 1, 'Only one unacknowledged NMEA chunk may be in flight');
    const inputId = transport.inputs[index].inputId, time = (index + 1) * 10;
    transport.emit({ text: JSON.stringify({ t: 'stat', time }) });
    transport.emit({ inputQueued: inputId + 1000, time });
    await flush();
    assert.equal(transport.inputs.length, index + 1, 'Old stats and unrelated receipts must not send another chunk');
    transport.emit({ inputQueued: inputId, time }); await flush();
    if (index + 1 === count) break;
    transport.emit({ text: JSON.stringify({ t: 'stat', time: time + 0.02 }) }); await flush();
    assert.equal(transport.inputs.length, index + 1, 'Guest must advance 40 ms after the matching input receipt');
    transport.emit({ text: JSON.stringify({ t: 'stat', time: time + 0.041 }) }); await flush();
  }
  const result = await outcome;
  if (result.error) throw result.error;
  assert.equal(transport.inputs.map(message => JSON.parse(message.data).data).join(''), result.value.data, 'NMEA chunks must reconstruct every byte in order');
  console.log('ok delayed-stat transport regression: matching worker receipt anchors each GPS chunk; unrelated receipts and stale stats cannot advance it');
} finally { await transportRuntime.dispose(); }

// Input contract only: hardware FIFO/scan/IRQ is tested in Rust and the opt-in
// official Cardputer probe. This transport stand-in is not firmware evidence.
for(const matrixBoard of ['cardputer-adv','cardputer']){
const matrixCatalog=structuredClone(catalog),matrixProduct=matrixCatalog.products.find(p=>p.id==='m5stack-k132-adv');
matrixProduct.runtime={...matrixProduct.runtime,board:matrixBoard,flashMb:8,psramMb:0,status:'partial'};
const matrixManifest=JSON.parse(await readFile(new URL('web/wasm/fw/hello.json',root),'utf8'));matrixManifest.board=matrixBoard;matrixManifest.psram_mb=0;
responseOverrides.set('https://platform.example/wasm/fw/cardputer-contract.json',JSON.stringify(matrixManifest));
const matrixMessages=[],matrixTransport={
  postMessage(m){let reply;if(m.op==='init')reply={ready:true};else if(m.op==='create')reply={created:true};else if(m.op==='load')reply={loaded:m.kind,ok:true};else if(m.op==='start')reply={started:true};else if(m.op==='stop')reply={stopped:true};else if(m.op==='text'){matrixMessages.push(JSON.parse(m.data));reply={inputQueued:m.inputId,time:0.25};}if(reply)queueMicrotask(()=>this.onmessage?.({data:reply}));},terminate(){},
};
const matrixRuntime=createPlatformRuntime({catalog:matrixCatalog,fetchImpl:fileFetch,workerFactory:()=>matrixTransport,baseUrl,assetBaseUrl:'https://platform.example/'});
try{
  await matrixRuntime.configure(createDefaultScene(matrixCatalog,matrixProduct.id));
  assert.deepEqual((await matrixRuntime.capabilities()).input.keyboard,{rows:4,columns:14,model:matrixBoard==='cardputer'?'gpio-matrix-74hc138':'tca8418',layout:matrixBoard});
  await matrixRuntime.loadFirmware({manifestUrl:'https://platform.example/wasm/fw/cardputer-contract.json'});await matrixRuntime.start();
  for(const event of [{row:-1,column:0,pressed:true},{row:4,column:0,pressed:true},{row:0,column:14,pressed:true},{row:0.5,column:0,pressed:true},{row:0,column:0,pressed:1}])await expectCode(()=>matrixRuntime.input({type:'matrix-key',...event}),'INVALID_INPUT');
  await matrixRuntime.input({type:'matrix-key',row:2,column:13,pressed:true});
  assert.deepEqual(matrixMessages,[{t:'matrix-key',row:2,column:13,pressed:1}]);
  await matrixRuntime.pause();await matrixRuntime.input({type:'matrix-key',row:2,column:13,pressed:false});
  await expectCode(()=>matrixRuntime.input({type:'matrix-key',row:2,column:13,pressed:true}),'NOT_RUNNING');
  assert.deepEqual(matrixMessages.at(-1),{t:'matrix-key',row:2,column:13,pressed:0});
}finally{await matrixRuntime.dispose();}
console.log('ok matrix-key contract: physical 4×14 bounds, independent hardware event, matching receipt and paused release');

}
