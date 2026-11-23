// Headless acceptance for the pinned K016-C factory image and production WASM.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { createServer } from 'node:http';
import { createRequire } from 'node:module';
import { extname, resolve, sep } from 'node:path';
import { once } from 'node:events';

const [factoryPath, outputPath] = process.argv.slice(2);
assert.ok(factoryPath && outputPath, 'Usage: node tools/fixtures/stickc-factory-browser-check.mjs <factory.bin> <evidence-dir>');
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.ESP_TEST_PLAYWRIGHT ?? 'playwright');
const pin = '8bd01902abdc0f0f6bfbf36a563de027d01cc9640e59c30756cc7315b06ccb03';
const romPin = '920b70635440517866aab2230964a570d2cf2b676658d93c52fbac108c1cca31';
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const factory = await readFile(factoryPath);
assert.equal(hash(factory), pin);
const web = resolve('web');
const wasm = await readFile(resolve(web, 'wasm/esp32sim.wasm'));
assert.equal(hash(await readFile(resolve(web, 'wasm/fw/esp32_rev300_rom.elf'))), romPin);
await mkdir(outputPath, { recursive: true });

const mime = {
  '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript',
  '.json': 'application/json', '.css': 'text/css', '.wasm': 'application/wasm',
};
const server = createServer(async (request, response) => {
  try {
    const { pathname } = new URL(request.url, 'http://localhost');
    if (pathname === '/__stickc-factory.bin') {
      response.writeHead(200, { 'Content-Type': 'application/octet-stream' }).end(factory);
      return;
    }
    const path = resolve(web, '.' + decodeURIComponent(pathname));
    if (!path.startsWith(web + sep)) {
      response.writeHead(403).end();
      return;
    }
    const bytes = await readFile(path);
    response.writeHead(200, { 'Content-Type': mime[extname(path)] ?? 'application/octet-stream' }).end(bytes);
  } catch {
    response.writeHead(404).end();
  }
});
server.listen(0, '127.0.0.1');
await once(server, 'listening');

async function advance(page, seconds) {
  await page.evaluate(async seconds => {
    const { espTester } = window;
    const start = (await espTester.observe()).stats.time || 0;
    const deadline = performance.now() + 180_000;
    while (performance.now() < deadline) {
      const state = await espTester.observe();
      if (state.status === 'error') throw Error(JSON.stringify(state.lastError));
      if (state.stats.time >= start + seconds) return;
      await new Promise(resolve => setTimeout(resolve, 50));
    }
    throw Error('Guest time did not advance');
  }, seconds);
}

async function capture(page, name) {
  const frame = await page.evaluate(async () => {
    const png = await espTester.capture({ format: 'png' });
    const { pixels } = await espTester.capture({ format: 'rgba' });
    let nonblack = 0;
    for (let i = 0; i < pixels.length; i += 4) {
      if (pixels[i] || pixels[i + 1] || pixels[i + 2]) nonblack++;
    }
    return { ...png, nonblack };
  });
  const bytes = Buffer.from(frame.dataUrl.split(',')[1], 'base64');
  assert.deepEqual([frame.width, frame.height], [80, 160]);
  assert.ok(frame.nonblack > 0, name + ' stayed blank');
  await writeFile(resolve(outputPath, name + '.png'), bytes);
  return { width: frame.width, height: frame.height, frameNumber: frame.frameNumber,
    nonblack: frame.nonblack, sha256: hash(bytes) };
}

let browser;
try {
  const origin = 'http://127.0.0.1:' + server.address().port;
  browser = await chromium.launch({
    executablePath: process.env.ESP_TEST_CHROME, headless: true,
    args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader'],
  });
  const page = await browser.newPage({ viewport: { width: 1440, height: 1100 } });
  const workers = [];
  page.on('worker', worker => workers.push(new URL(worker.url()).pathname));
  await page.goto(origin + '/platform/index.html');
  await page.waitForFunction(() => !!window.espTester);
  const receipt = await page.evaluate(async ({ pin }) => {
    const { espTester } = window;
    const { loadCatalog, createDefaultScene } = await import('./catalog.js');
    const catalog = await loadCatalog();
    const scene = createDefaultScene(catalog, 'm5stack-k016-c');
    const product = catalog.products.find(item => item.id === scene.productId);
    const selected = catalog.firmwares.find(item => item.id === product.defaultFirmwareId);
    if (selected?.source?.sha256 !== pin) throw Error('Unexpected default firmware pin');
    scene.firmware = selected.source;
    let transport = 'official-cdn';
    await espTester.configure(scene);
    try {
      await espTester.loadFirmware();
    } catch (error) {
      if (!['FETCH_FAILED', 'CORS_OR_NETWORK', 'ARTIFACT_FETCH_FAILED', 'HTTP_ERROR'].includes(error.code)
          && !/fetch|network|http|timed out/i.test(error.message)) throw error;
      transport = 'same-origin-checked-official-bytes';
      scene.firmware = { url: location.origin + '/__stickc-factory.bin', sha256: pin, bootMode: 'rom' };
      await espTester.configure(scene);
      await espTester.loadFirmware();
    }
    await espTester.start();
    return { transport, firmware: (await espTester.observe()).firmware };
  }, { pin });
  const { firmware } = receipt;
  assert.equal(firmware.executionMode, 'wasm-jit');
  assert.equal(firmware.appDirect, false);
  assert.deepEqual(firmware.stubs, []);
  assert.ok(firmware.artifacts.some(artifact => artifact.sha256 === pin));
  assert.ok(firmware.artifacts.some(artifact => artifact.sha256 === romPin));

  await advance(page, 12);
  const idle = await capture(page, 'idle');
  const uart = await page.evaluate(async () => (await espTester.observe()).serial);
  assert.match(uart, /SPI_FAST_FLASH_BOOT/);
  assert.match(uart, /M5StickC initializing\.\.\.OK/);
  assert.doesNotMatch(uart, /Guru Meditation|StoreProhibited|Brownout detector/);
  await page.locator('#view-2d').click();
  await page.evaluate(() => {
    window.stickcEdges = [];
    espTester.subscribe(message => {
      if (message.type === 'input') {
        const { id, pressed } = message.event;
        if (id === 'button-a' || id === 'button-b') window.stickcEdges.push({ id, pressed });
      }
    });
  });
  await page.locator('.device-button[data-control="button-a"]').first().hover();
  await page.mouse.down();
  await advance(page, 0.18);
  await page.mouse.up();
  await advance(page, 0.5);
  const rtc = await capture(page, 'rtc');
  await advance(page, 1.3);
  const rtcTick = await capture(page, 'rtc-tick');
  await page.keyboard.down('b');
  await advance(page, 0.18);
  await page.keyboard.up('b');
  await advance(page, 0.5);
  const mic = await capture(page, 'mic');
  const edges = await page.evaluate(() => window.stickcEdges);
  assert.deepEqual(edges, [
    { id: 'button-a', pressed: true }, { id: 'button-a', pressed: false },
    { id: 'button-b', pressed: true }, { id: 'button-b', pressed: false },
  ]);
  assert.equal(new Set([idle.sha256, rtc.sha256, rtcTick.sha256, mic.sha256]).size, 4);
  assert.ok(workers.some(path => path.endsWith('/worker.js')), 'No production worker observed');
  const report = {
    transport: receipt.transport, executionMode: firmware.executionMode, appDirect: firmware.appDirect,
    stubs: firmware.stubs, flashSha256: pin, romSha256: romPin,
    wasm: { sha256: hash(wasm), bytes: wasm.length }, idle, rtc, rtcTick, mic, edges,
    uart: ['SPI_FAST_FLASH_BOOT', 'M5StickC initializing...OK'], workers,
  };
  await writeFile(resolve(outputPath, 'receipt.json'), JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify(report, null, 2));
} finally {
  try {
    await browser?.close();
  } finally {
    await new Promise(resolve => server.close(resolve));
  }
}
