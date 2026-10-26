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

const root = new URL('../', import.meta.url), catalog = {};
for (const name of ['products', 'accessories', 'firmwares']) catalog[name] = JSON.parse(await readFile(new URL(`web/platform/catalog/${name}.json`, root), 'utf8')).items;
const baseUrl = 'https://platform.example/platform/index.html';
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
    parentPort.on('message', data => globalThis.onmessage({data}));`;
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

const runtime = createPlatformRuntime({ catalog, fetchImpl: fileFetch, workerFactory, baseUrl, assetBaseUrl: 'https://platform.example/' });
const target = { location: { href: baseUrl } }, api = installAutomation(runtime, { target });
assert.equal(target.espTester.version, '1.0.0');
try {
  await expectCode(() => runtime.start(), 'NOT_READY');
  await runtime.configure(createDefaultScene(catalog, 'm5stack-papermono'));
  assert.equal((await runtime.capabilities()).executable, false);
  await expectCode(() => runtime.loadFirmware({ manifestUrl: 'https://platform.example/wasm/fw/hello.json' }), 'UNSUPPORTED_PRODUCT');
  await runtime.configure(createDefaultScene(catalog, 'generic-esp32s3'));
  await expectCode(() => runtime.capture(), 'NO_FRAME');
  await expectCode(() => runtime.loadFirmware({ manifestUrl: 'https://platform.example/wasm/fw/hello.json', sha256: '0'.repeat(64) }), 'HASH_MISMATCH');
  await runtime.loadFirmware({ manifestUrl: 'https://platform.example/wasm/fw/hello.json' });
  assert.equal((await runtime.observe()).status, 'ready');
  await runtime.start();
  const running = await api.waitFor({ serialIncludes: 'Hello world!', port: 'uart0', timeoutMs: 30000 });
  assert.equal(running.status, 'running');
  await expectCode(() => runtime.input({ type: 'serial', port: 'uart2', data: 'x' }), 'UNSUPPORTED_PORT');
  await expectCode(() => runtime.input({ type: 'button', id: 'button-a', pressed: true }), 'UNSUPPORTED_CONTROL');
  await runtime.updateSceneMetadata({ view: { mode: 'embed', deviceView: '2d' } });
  assert.equal((await runtime.observe()).status, 'running');
  const shared = new URL(await api.share({ mode: 'embed' }));
  assert.equal(shared.pathname, '/platform/embed.html');
  assert.equal(decodeScene(shared.hash.slice('#scene='.length)).firmware.manifestUrl, 'https://platform.example/wasm/fw/hello.json');
  const paused = await runtime.pause(), pausedInsns = paused.stats.insns;
  assert.ok(pausedInsns > 0, 'Real guest instructions must execute before pause');
  await delay(150);
  assert.equal((await runtime.observe()).stats.insns, pausedInsns, 'Pause must freeze guest execution');
  // Rapid cycles leave queued callbacks from old loop generations; they must never restart a paused worker.
  for (let i = 0; i < 5; i++) { await runtime.start(); await runtime.pause(); }
  const afterRapid = await runtime.observe();
  await delay(150);
  assert.equal((await runtime.observe()).stats.insns, afterRapid.stats.insns);
  await runtime.start(); await delay(300);
  const resumed = await runtime.observe();
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
  await expectCode(() => runtime.loadFirmware({ manifestUrl: 'https://platform.example/wasm/fw/hello.json' }), 'BOARD_MISMATCH');
  const badPins = JSON.parse(await readFile(new URL('web/wasm/fw/demo-stopwatch.json', root), 'utf8'));
  badPins.artifact_sha256 = { ...badPins.artifact_sha256, [badPins.files.app]: '0'.repeat(64) };
  responseOverrides.set('https://platform.example/wasm/fw/bad-pins.json', JSON.stringify(badPins));
  await expectCode(() => runtime.loadFirmware({ manifestUrl: 'https://platform.example/wasm/fw/bad-pins.json' }), 'HASH_MISMATCH');
  await runtime.loadFirmware({ manifestUrl: 'https://platform.example/wasm/fw/demo-stopwatch.json' });
  await runtime.start();
  await api.waitFor({ serialIncludes: 'PLATFORM_READY stopwatch', timeoutMs: 30000 });
  const display = await runtime.capture({ format: 'rgba' });
  assert.deepEqual([display.width, display.height], [468, 466]);
  assert.ok(display.pixels.some((value, index) => index % 4 !== 3 && value > 80), 'Firmware must draw actual bright pixels');
  const buttonA = api.waitFor({ serialIncludes: 'BUTTON_A running', fromNow: true });
  await api.press('button-a', { durationMs: 150 }); await buttonA;
  const buttonB = api.waitFor({ serialIncludes: 'BUTTON_B reset', fromNow: true });
  await api.press('button-b', { durationMs: 150 }); await buttonB;
  const touch = api.waitFor({ serialIncludes: 'TOUCH down', fromNow: true });
  await runtime.input({ type: 'touch', x: 234, y: 233, down: true }); await touch;
  await runtime.input({ type: 'touch', x: 234, y: 233, down: false });
  await expectCode(() => runtime.input({ type: 'touch', x: 468, y: 0, down: true }), 'INVALID_INPUT');
  const gps = api.waitFor({ serialIncludes: 'GPS_RX $GPGGA', fromNow: true });
  const injected = await runtime.injectGps({ accessoryId: 'gps-test', timestamp: '2026-01-02T03:04:05Z' });
  await gps;
  assert.equal(injected.delivery, 'logical-uart-rx');
  const gpsState = await runtime.observe();
  assert.ok(Object.values(gpsState.serialByPort).some(text => text.includes('GPS_RX $GPRMC,030405.00,A,3113.82400,N,12128.42200,E')));
  assert.ok((await runtime.capture({ format: 'rgba' })).frameNumber > display.frameNumber, 'Inputs must result in new firmware framebuffer output');
  console.log('ok StopWatch real firmware: 468×466 framebuffer, A/B GPIO, CST820 touch, GPS RMC+GGA through UART1 RX');

  const [boot, table, app] = await Promise.all(['platform-stopwatch-bootloader.bin', 'platform-stopwatch-ptable.bin', 'platform-stopwatch.bin'].map(name => readFile(new URL(`web/wasm/fw/public/${name}`, root))));
  const localImage = new Uint8Array(0x10000 + app.length).fill(255);
  localImage.set(boot, 0); localImage.set(table, 0x8000); localImage.set(app, 0x10000);
  await runtime.loadFirmware({ file: new Blob([localImage]), sha256: await sha256(localImage) });
  await runtime.start(); await api.waitFor({ serialIncludes: 'PLATFORM_READY stopwatch', timeoutMs: 30000 });
  await expectCode(() => api.share(), 'LOCAL_FIRMWARE_NOT_SHAREABLE');
  console.log('ok local merged StopWatch flash: complete app validation, SHA-256, automatic ROM load, real boot; file URLs excluded from share');
} finally { await runtime.dispose(); }
assert.equal((await runtime.observe()).status, 'disposed');
await expectCode(() => runtime.start(), 'DISPOSED');
console.log('ok platform session disposal');
