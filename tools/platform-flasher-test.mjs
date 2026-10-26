#!/usr/bin/env node
// Protocol and validation tests only: no Web Serial chooser or physical device is used.
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { createFlasher, prepareFlashPlan, validateFlashLayout, flasherCapabilities } from '../web/platform/flasher.js';
import { createDefaultScene } from '../web/platform/catalog.js';
import { withPlatformTestFixtures } from './fixtures/platform-catalog.mjs';
import SparkMD5 from '../web/platform/vendor/spark-md5-3.0.2/spark-md5.js';
import { ESPLoader, Transport } from '../web/platform/vendor/esptool-js-0.7.0/bundle.js';

const root = new URL('../', import.meta.url), baseUrl = 'https://platform.example/platform/index.html', publicCatalog = {};
for (const name of ['products', 'accessories', 'firmwares']) publicCatalog[name] = JSON.parse(await readFile(new URL(`web/platform/catalog/${name}.json`, root))).items;
const catalog = await withPlatformTestFixtures(publicCatalog, baseUrl);
const scene = createDefaultScene(catalog, 'm5stack-stopwatch');
const watchlist = JSON.parse(await readFile(new URL('web/platform/catalog/firmware-watchlist.json', root)));
scene.firmware = { manifestUrl: 'https://platform.example/wasm/fw/demo-stopwatch.json' };
const requests = [];
const fetchImpl = async (url, options) => {
  requests.push({ url, options });
  assert.equal(options.cache, 'no-store');
  assert.equal(options.credentials, 'omit');
  const bytes = await readFile(new URL(`web${new URL(url).pathname}`, root));
  return new Response(bytes);
};
const md5 = data => SparkMD5.ArrayBuffer.hash(data.buffer.slice(data.byteOffset, data.byteOffset + data.byteLength));
assert.equal(typeof ESPLoader.prototype.writeFlash, 'function');
assert.equal(typeof Transport.prototype.disconnect, 'function');
assert.equal(md5(new TextEncoder().encode('abc')), '900150983cd24fb0d6963f7d28e17f72');
const vendor = JSON.parse(await readFile(new URL('web/platform/vendor/flasher-vendor.json', root)));
for (const file of vendor.files) assert.equal(createHash('sha256').update(await readFile(new URL(`web/platform/vendor/${file.file}`, root))).digest('hex'), file.sha256);
console.log('ok vendored official esptool/MD5 exports, known MD5 vector and pinned vendor hashes');

const plan = await prepareFlashPlan({ scene, catalog, fetchImpl, baseUrl });
assert.deepEqual(plan.files.map(file => file.address), [0, 0x8000, 0x10000]);
assert.equal(requests.some(request => request.url.endsWith('_rom.elf')), false, 'ROM ELF must never be downloaded for hardware flashing');
assert.equal(plan.receipt.persistence, 'user-browser-cache-for-pinned-remote-artifacts');
assert.throws(() => validateFlashLayout([plan.files[0], { ...plan.files[1], address: 0 }], plan.product), { code: 'FLASH_LAYOUT' });
await assert.rejects(() => prepareFlashPlan({ scene, catalog, fetchImpl, baseUrl, source: { ...scene.firmware, sha256: '0'.repeat(64) } }), { code: 'HASH_MISMATCH' });
await assert.rejects(() => prepareFlashPlan({ scene, catalog, fetchImpl, baseUrl, watchEntry: { compatibility: { status: 'reference-only', productIds: [scene.productId] } } }), { code: 'REFERENCE_ONLY' });
await assert.rejects(() => prepareFlashPlan({ scene, catalog, fetchImpl, baseUrl, watchlist, source: { url: watchlist.entries.find(entry => entry.compatibility.status === 'reference-only').assets[0].url } }), { code: 'REFERENCE_ONLY' });
const official = watchlist.entries.find(entry => entry.id === 'm5stack-stopwatch-official');
for (const url of [official.assets[0].url, official.assets[0].upstreamUrl]) {
  const changed = new Uint8Array([1, 2, 3]), chosenHash = createHash('sha256').update(changed).digest('hex');
  await assert.rejects(() => prepareFlashPlan({ scene, catalog, watchlist, baseUrl, source: { url, sha256: chosenHash }, fetchImpl: async () => new Response(changed) }), { code: 'HASH_MISMATCH' });
}
console.log('ok flash plan: complete pinned images, verified offsets and sector overlap rejection; reference-only entries blocked');
const localSource={format:'segments',files:plan.files.map((entry,i)=>({file:new File([entry.data],['bootloader.bin','partition-table.bin','app.bin'][i]),offset:entry.address}))};
const localPlan=await prepareFlashPlan({scene,catalog,source:localSource,fetchImpl:()=>{throw Error('Local hardware plan must not fetch remote bytes');},baseUrl});
assert.deepEqual(localPlan.files.map(x=>x.address),[0,0x8000,0x10000]);
assert.deepEqual(localPlan.files.map(x=>x.data.length),plan.files.map(x=>x.data.length));
assert.ok(localPlan.receipt.artifacts.every(x=>x.url===null));
assert.equal(localPlan.files.reduce((n,x)=>n+x.data.length,0),plan.files.reduce((n,x)=>n+x.data.length,0),'Unspecified flash gaps must not become hardware writes');
console.log('ok local split hardware plan: original segments only, no gap erasure or remote fetch; no hardware connected');


function fixture({ chip = 'ESP32-S3', size = '16MB', secured = false, activated = true, denied = false, cancelAtBlock = false } = {}) {
  const calls = { chooser: 0, main: 0, writes: 0, blocks: 0, disconnect: 0, reset: 0 }, events = [];
  let flasher;
  class FakeTransport { constructor(port) { assert.equal(port.testOnly, true); } async disconnect() { calls.disconnect++; } }
  class FakeLoader {
    constructor(options) { assert.equal(options.baudrate, 115200); this.chip = { CHIP_NAME: chip }; }
    async main() { calls.main++; }
    async getSecurityInfo() { return { flashCryptCnt: 0, parsedFlags: { SECURE_BOOT_EN: secured, SECURE_DOWNLOAD_ENABLE: false } }; }
    async detectFlashSize() { return size; }
    flashSizeBytes(value) { return parseInt(value) * 1024 * 1024; }
    async writeFlash(options) {
      calls.writes++;
      assert.equal(options.eraseAll, false); assert.equal(options.flashMode, 'keep'); assert.equal(options.flashFreq, 'keep'); assert.equal(options.flashSize, 'keep');
      for (let index = 0; index < options.fileArray.length; index++) {
        const bytes = options.fileArray[index].data;
        assert.equal(options.calculateMD5Hash(bytes), createHash('md5').update(bytes).digest('hex'));
        options.reportProgress(index, 0, bytes.length);
        calls.blocks++; options.reportProgress(index, bytes.length, bytes.length);
        if (cancelAtBlock) flasher.cancel();
      }
    }
    async after(mode) { assert.equal(mode, 'hard_reset'); calls.reset++; }
  }
  const navigator = { userActivation: { isActive: activated }, serial: { requestPort() { calls.chooser++; return denied ? Promise.reject(new DOMException('No device chosen', 'NotFoundError')) : Promise.resolve({ testOnly: true }); } } };
  flasher = createFlasher({ navigator, secureContext: true, fetchImpl, baseUrl, loadWatchlistData: async () => watchlist, loadTools: async () => ({ ESPLoader: FakeLoader, Transport: FakeTransport, md5 }), onEvent(event) { events.push(event); } });
  return { flasher, calls, events, navigator };
}
assert.equal(flasherCapabilities({ navigator: {}, secureContext: true }).supported, false);
assert.equal(flasherCapabilities({ navigator: { serial: { requestPort() {} } }, document: { permissionsPolicy: { allowsFeature: () => false } }, secureContext: true }).supported, false);
for (const [options, code] of [[{ activated: false }, 'USER_GESTURE_REQUIRED'], [{ denied: true }, 'FLASH_CANCELLED'], [{ chip: 'ESP32-C3' }, 'CHIP_MISMATCH'], [{ size: '8MB' }, 'FLASH_SIZE_MISMATCH'], [{ secured: true }, 'SECURED_DEVICE']]) {
  const test = fixture(options);
  await assert.rejects(() => test.flasher.flash({ scene, catalog }), { code });
  assert.equal(test.calls.writes, 0); assert.equal(test.calls.reset, 0);
  if (!options.activated && options.activated !== undefined) assert.equal(test.calls.chooser, 0);
  if (test.calls.main) assert.equal(test.calls.disconnect, 1);
}
const reference = fixture();
await assert.rejects(() => reference.flasher.flash({ scene, catalog, watchEntry: { compatibility: { status: 'reference-only', productIds: [] } } }), { code: 'REFERENCE_ONLY' });
assert.equal(reference.calls.chooser, 0);
const success = fixture(), flashed = success.flasher.flash({ scene, catalog });
assert.equal(success.calls.chooser, 1, 'Chooser must run synchronously in the original user gesture');
await flashed;
assert.equal(success.flasher.observe().status, 'done'); assert.equal(success.calls.writes, 1); assert.equal(success.calls.reset, 1); assert.equal(success.calls.disconnect, 1);
assert.ok(success.events.filter(event => event.type === 'state').every(event => event.state.progress >= 0 && event.state.progress <= 1));
const stopped = fixture({ cancelAtBlock: true });
await assert.rejects(() => stopped.flasher.flash({ scene, catalog }), { code: 'FLASH_CANCELLED' });
assert.equal(stopped.calls.blocks, 1); assert.equal(stopped.calls.reset, 0); assert.equal(stopped.calls.disconnect, 1); assert.equal(stopped.flasher.observe().mayBePartial, true);
console.log('ok injected flasher protocol: user gesture, chip/flash/security gates, no erase-all, MD5, progress, cancellation and port cleanup; no hardware accessed');
