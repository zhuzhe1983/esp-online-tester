#!/usr/bin/env node
// Opt-in, bounded probe of the exact catalog-pinned full official flash image.
// Downloaded firmware and writable Flash stay in RAM. Only sanitized JSON is saved.
// No app extraction, app-direct fallback, guest patches, stubs or input injection.
import assert from 'node:assert/strict';
import {readFile, mkdir, writeFile} from 'node:fs/promises';
import {resolve, dirname, basename, sep} from 'node:path';
import {pathToFileURL, fileURLToPath} from 'node:url';
import {execFileSync} from 'node:child_process';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';

const args = process.argv.slice(2);
const value = (flag, fallback) => args.includes(flag) ? args[args.indexOf(flag) + 1] : fallback;
assert(args.every((arg, i) => i % 2 ? true : ['--output', '--duration-ms'].includes(arg)), 'Unknown argument');
const durationMs = Number(value('--duration-ms', '45000'));
assert(Number.isInteger(durationMs) && durationMs >= 1000 && durationMs <= 60000, 'Duration must be 1,000–60,000 ms');
const output = resolve(value('--output', 'output/cloudzao-claw154-official/receipt.json'));
assert(output.endsWith('.json'), 'Output must be a JSON receipt, never a firmware file');
const root = process.env.ESP32SIM_TEST_ROOT
  ? pathToFileURL(resolve(process.env.ESP32SIM_TEST_ROOT) + sep) : new URL('../', import.meta.url);
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const safeUrl = value => {const url = new URL(value); url.search = ''; url.hash = ''; return String(url);};
const sanitize = text => String(text)
  .replace(/\x1b\[[0-9;]*m/g, '')
  .replace(/https?:\/\/[^\s"<>]+/g, value => {try {return safeUrl(value);} catch {return '[URL]';}})
  .replace(/\b(?:[0-9a-f]{2}:){5}[0-9a-f]{2}\b/gi, '[MAC]')
  .replace(/\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b/gi, '[UUID]')
  .replace(/\b(?:\d{1,3}\.){3}\d{1,3}\b/g, '[IPv4]')
  .replace(/\b(password|passwd|token|secret|api[_-]?key|ssid)\s*[:=]\s*[^\s,;]+/gi, '$1=[REDACTED]');
const excerpt = text => {
  const clean = sanitize(text);
  return clean.length <= 7000 ? clean : clean.slice(0, 2500) + '\n[... middle omitted ...]\n' + clean.slice(-4500);
};
assert.equal(globalThis.indexedDB, undefined, 'Run under Node without a persistent IndexedDB shim');
const [{createPlatformRuntime}, {createDefaultScene}, {createDeviceStorage}, {createSdCardStorage}] = await Promise.all([
  import(new URL('web/platform/runtime.js', root)), import(new URL('web/platform/catalog.js', root)),
  import(new URL('web/platform/device-storage.js', root)), import(new URL('web/platform/sd-card-storage.js', root)),
]);
const catalog = Object.fromEntries(await Promise.all(['products', 'accessories', 'firmwares'].map(async name =>
  [name, JSON.parse(await readFile(new URL(`web/platform/catalog/${name}.json`, root))).items])));
const product = catalog.products.find(item => item.id === 'cloudzao-claw-154');
const official = catalog.firmwares.find(item => item.id === 'cloudzao-claw-154-official');
assert.equal(product?.runtime.board, 'cloudzao-claw-154');
assert.deepEqual([product.runtime.flashMb, product.runtime.psramMb], [16, 8]);
assert(official?.productIds.includes(product.id) && official.role === 'official');
assert(/^https:\/\/github\.com\/CloudZao\/Metalio-Claw-1\.54\/releases\/download\//.test(official.source.url));
assert(/^[a-f0-9]{64}$/.test(official.source.sha256));
const receipt = {
  schemaVersion: '1', checkedAt: new Date().toISOString(), productId: product.id, board: product.runtime.board,
  firmwareId: official.id, backend: 'actual-platform-runtime-worker-wasm',
  runtimeCommit: execFileSync('git', ['rev-parse', 'HEAD'], {cwd: fileURLToPath(root), encoding: 'utf8'}).trim(),
  scriptSha256: hash(await readFile(fileURLToPath(import.meta.url))),
  runtimeSources: Object.fromEntries(await Promise.all(['web/platform/runtime.js', 'web/wasm/worker.js', 'web/wasm/esp32sim.wasm'].map(async path => [path, hash(await readFile(new URL(path, root)))]))),
  requestedWallMs: durationMs, firmwarePatched: false, appReplaced: false, appDirect: false, stubs: [],
  persistence: 'Firmware download, firmware cache and writable Flash are process-memory only; JSON evidence only on disk.',
  inputInjected: false, physicalHardwareValidated: false, cloudValidated: false, audioValidated: false,
  officialApplicationVerified: false, workerOperations: {}, workerLoads: [], checkpoints: [],
};
console.log(JSON.stringify({phase: 'download', firmwareId: official.id, url: official.source.url}));
const response = await fetch(official.source.url, {signal: AbortSignal.timeout(45000)});
assert(response.ok, `Official firmware HTTP ${response.status}`);
const firmware = new Uint8Array(await response.arrayBuffer());
assert.equal(hash(firmware), official.source.sha256, 'Catalog-pinned full-image SHA-256 mismatch');
assert.equal(firmware.length, official.sourceIdentity.bytes, 'Full-image byte count mismatch');
receipt.source = {url: official.source.url, resolvedUrl: safeUrl(response.url), bytes: firmware.length,
  sha256: hash(firmware), mediaType: response.headers.get('content-type')};
console.log(JSON.stringify({phase: 'download-verified', bytes: firmware.length, sha256: receipt.source.sha256}));

const origin = 'https://cloudzao-claw154-probe.invalid';
let thread, runtime, serial = '', emulator = '', frameEvents = 0, workerFault = null;
const workerFactory = url => {
  const real = new URL('web' + url.pathname, root);
  const source = `import {parentPort} from 'node:worker_threads';
globalThis.onmessage=null;
globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);
await import(${JSON.stringify(String(real))});
parentPort.on('message',data=>globalThis.onmessage({data}));`;
  thread = new Worker(new URL('data:text/javascript,' + encodeURIComponent(source)));
  const adapter = {postMessage(message, ...rest) {
    receipt.workerOperations[message.op] = (receipt.workerOperations[message.op] || 0) + 1;
    assert(!['stub', 'ble-hci'].includes(message.op), 'No guest patch/stub profile is permitted in this probe');
    if (message.op === 'create') receipt.workerConfig = {board: message.board, flashMb: message.flash_mb, psramMb: message.psram_mb, jit: message.jit !== false};
    if (message.op === 'load') receipt.workerLoads.push({kind: message.kind, ...(message.at !== undefined ? {at: message.at} : {}), bytes: message.data.byteLength, sha256: hash(new Uint8Array(message.data))});
    if (message.op === 'start') assert.equal(message.appDirect, false, 'Full image must boot through the ROM');
    return thread.postMessage(message, ...rest);
  }, terminate: () => thread.terminate()};
  thread.on('message', data => adapter.onmessage?.({data}));
  thread.on('error', error => {workerFault = sanitize(error.message); adapter.onerror?.(error);});
  return adapter;
};
runtime = createPlatformRuntime({catalog, workerFactory, requestTimeout: 5000,
  baseUrl: origin + '/platform/', assetBaseUrl: origin + '/',
  deviceStorage: createDeviceStorage({indexedDB: null, BroadcastChannel: null, dbName: 'cloudzao-official-probe'}),
  sdStorage: createSdCardStorage({indexedDB: null, BroadcastChannel: null, dbName: 'cloudzao-official-probe-sd'}),
  fetchImpl: async value => {
    const url = new URL(value);
    if (String(url) === official.source.url) return new Response(firmware);
    assert.equal(url.origin, origin, 'Unexpected external request');
    assert(/\/(esp32sim\.wasm|esp32s3_rev0_rom\.elf)$/.test(url.pathname), 'Only the actual WASM and official S3 ROM may be loaded');
    let bytes;
    try {bytes = await readFile(new URL('web' + url.pathname, root));}
    catch (error) {
      if (error.code !== 'ENOENT' || !process.env.ESP32SIM_ROM_DIR || !url.pathname.endsWith('esp32s3_rev0_rom.elf')) throw error;
      bytes = await readFile(resolve(process.env.ESP32SIM_ROM_DIR, basename(url.pathname)));
    }
    return new Response(bytes);
  },
  onEvent(event) {
    if (event.type === 'serial') {
      if (event.source === 'emulator') emulator = (emulator + event.data).slice(-30000);
      else serial = (serial + event.data).slice(-65536);
    }
    if (event.type === 'frame') frameEvents++;
  },
});
const probeStarted = Date.now();
let executionStarted = null;
try {
  const scene = createDefaultScene(catalog, product.id);
  await runtime.configure(scene);
  await runtime.loadFirmware({...official.source, format: 'merged', bootMode: 'rom'});
  const loaded = await runtime.observe();
  assert.equal(loaded.firmware.appDirect, false);
  assert.deepEqual(loaded.firmware.stubs, []);
  assert.equal(loaded.storage.restored, false, 'Probe must begin from the original Flash image');
  const flashLoads = receipt.workerLoads.filter(item => item.kind === 5);
  assert.equal(flashLoads.length, 1);
  assert.deepEqual({bytes: flashLoads[0].bytes, sha256: flashLoads[0].sha256}, {bytes: firmware.length, sha256: official.source.sha256});
  assert(receipt.workerLoads.every(item => [0, 5].includes(item.kind)), 'Only complete Flash and ROM are loaded');
  receipt.firmware = {source: loaded.firmware.source, appDirect: loaded.firmware.appDirect,
    executionMode: loaded.firmware.executionMode, stubs: loaded.firmware.stubs, artifacts: loaded.firmware.artifacts};
  executionStarted = Date.now();
  await runtime.start();
  console.log(JSON.stringify({phase: 'execution-started', wallLimitMs: durationMs}));
  let lastCheckpoint = -10000;
  while (Date.now() - executionStarted < durationMs) {
    const state = await runtime.observe(), wallMs = Date.now() - executionStarted;
    if (wallMs - lastCheckpoint >= 10000) {
      const checkpoint = {wallMs, status: state.status, stats: state.stats, frame: state.frame};
      receipt.checkpoints.push(checkpoint); lastCheckpoint = wallMs;
      console.log(JSON.stringify({phase: 'running', wallMs, status: state.status, guestSeconds: state.stats.time, insns: state.stats.insns}));
    }
    if (state.status !== 'running') {receipt.stopReason = 'runtime-left-running'; break;}
    await delay(100);
  }
  receipt.executionWallMs = Date.now() - executionStarted;
  receipt.beforePause = await runtime.observe().then(state => ({status: state.status, stats: state.stats, lastError: state.lastError}));
  if (receipt.beforePause.status === 'running') await runtime.pause();
  receipt.stopReason ||= 'requested-wall-budget';
} catch (error) {
  receipt.failure = {code: error.code || 'PROBE_ERROR', message: sanitize(error.message)};
  receipt.stopReason = 'probe-error';
  process.exitCode = 1;
} finally {
  try {
    const state = await runtime.observe();
    receipt.final = {status: state.status, stats: state.stats, lastError: state.lastError,
      storage: {backend: state.storage.backend, restored: state.storage.restored, changedPages: state.storage.changedPages}, frame: state.frame};
    if (state.frame) {
      const frame = await runtime.capture({format: 'rgba'});
      let nonblackPixels = 0, opaquePixels = 0;
      const colors = new Set();
      for (let i = 0; i < frame.pixels.length; i += 4) {
        const rgb = frame.pixels[i] * 65536 + frame.pixels[i + 1] * 256 + frame.pixels[i + 2];
        if (rgb !== 0) nonblackPixels++;
        if (frame.pixels[i + 3] === 255) opaquePixels++;
        colors.add(rgb);
      }
      receipt.frame = {width: frame.width, height: frame.height, frameNumber: frame.frameNumber,
        rgbaSha256: hash(frame.pixels), nonblackPixels, opaquePixels, distinctRgbColors: colors.size};
    } else receipt.frame = null;
    receipt.frameEvents = frameEvents;
    // ESP-IDF may mirror console bytes to USB and UART0. Keep the actual UART0
    // stream, not an interleaving that would make one boot look like two boots.
    serial = state.serialByPort?.uart0 || serial;
    const interesting = [...new Set(sanitize(serial).split(/\r?\n/).filter(line =>
      /boot:|app_init:|main_task:|Lcd|Display|ST7789|CST816|touch|AtUart:.*failed|ML307|esp_psram|ES8311|Guru Meditation|assert failed|abort\(\)/i.test(line)))];
    receipt.serial = {source: state.serialByPort?.uart0 ? 'uart0' : 'combined-fallback',
      portCharacterCounts: Object.fromEntries(Object.entries(state.serialByPort || {}).map(([port, text]) => [port, text.length])),
      retainedCharacters: serial.length, excerpt: excerpt(serial),
      importantLines: interesting.length <= 44 ? interesting : [...interesting.slice(0, 32), '[... middle omitted ...]', ...interesting.slice(-12)]};
    receipt.emulator = {retainedCharacters: emulator.length, excerpt: excerpt(emulator)};
    receipt.workerFault = workerFault;
    receipt.totalProbeWallMs = Date.now() - probeStarted;
    receipt.executionWallMs ??= executionStarted === null ? 0 : Date.now() - executionStarted;
    receipt.limitations = [
      'This is a bounded Node Worker/WASM probe, not browser CORS or browser UI acceptance.',
      'A running emulator, ROM/bootloader output or black initial frame does not establish application startup.',
      'No physical hardware, GPIO electrical timing, microphone/speaker, ML307 cellular or cloud-account validation.',
      'Unmodified full official image is loaded at Flash offset zero; no extracted substitute app or guest patch.',
      'Serial evidence omits its middle when long and redacts MAC/UUID/IPv4/credential-like fields and URL queries.',
    ];
  } finally {
    await Promise.race([runtime.dispose(), delay(5000)]);
    await thread?.terminate();
    await mkdir(dirname(output), {recursive: true});
    await writeFile(output, JSON.stringify(receipt, null, 2) + '\n');
    console.log(JSON.stringify({phase: 'complete', output, stopReason: receipt.stopReason,
      status: receipt.final?.status, stats: receipt.final?.stats, frame: receipt.frame, failure: receipt.failure}));
  }
}
