#!/usr/bin/env node
// Executes the original project SRAM diagnostic through the public runtime,
// production Worker and WASM. No browser, factory firmware or RF claim.
import assert from 'node:assert/strict';
import {readFile, writeFile, mkdir} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {Worker} from 'node:worker_threads';
import {resolve, join} from 'node:path';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createSdCardStorage} from '../web/platform/sd-card-storage.js';

const output = resolve(process.argv[2] || 'output/waveshare-c5-lcd147-public');
const root = new URL('../', import.meta.url), origin = 'https://waveshare-acceptance.invalid';
const sha256 = data => createHash('sha256').update(data).digest('hex');
await mkdir(output, {recursive: true});
const catalog = Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async name =>
  [name, JSON.parse(await readFile(new URL(`web/platform/catalog/${name}.json`, root))).items])));
const fixtures = JSON.parse(await readFile(new URL('tools/fixtures/waveshare-c5-firmwares.json', root))).items;
const firmware = fixtures.find(f => f.id === 'waveshare-c5-lcd147-sram-diagnostic');
assert.equal(firmware.role, 'example');
const bytes = await readFile(new URL('web/platform/firmware/waveshare-c5-lcd147-probe.bin', root));
assert.equal(sha256(bytes), firmware.source.sha256);
const requests = [], products = [], messages = [];
const workerFactory = url => {
  const real = new URL('web' + url.pathname, root);
  const source = `import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`;
  const thread = new Worker(new URL('data:text/javascript,' + encodeURIComponent(source)));
  const adapter = {postMessage: (...args) => {
    if (['create','start','text'].includes(args[0].op)) messages.push(args[0]);
    thread.postMessage(...args);
  }, terminate: () => thread.terminate()};
  thread.on('message', data => adapter.onmessage?.({data}));
  thread.on('error', error => adapter.onerror?.(error));
  return adapter;
};
const storage = {indexedDB: null, BroadcastChannel: null, dbName: 'waveshare-c5-lcd147-' + Date.now()};
const runtime = createPlatformRuntime({catalog, workerFactory, baseUrl: origin + '/platform/', assetBaseUrl: origin + '/',
  deviceStorage: createDeviceStorage(storage), sdStorage: createSdCardStorage(storage), fetchImpl: async value => {
    const url = new URL(value, origin); assert.equal(url.origin, origin); requests.push(url.pathname);
    assert(!url.pathname.endsWith('_rom.elf'), 'SRAM execution must not substitute a different chip ROM');
    return new Response(await readFile(new URL('web' + url.pathname, root)));
  }});
const started = Date.now();
let state, result;
const serial = s => s.serialByPort?.uart0 || '';
async function waitFor(label, predicate) {
  const deadline = Date.now() + 90000;
  while (Date.now() < deadline) {
    state = await runtime.observe();
    if (state.status === 'error') throw Error(JSON.stringify(state.lastError));
    if (predicate(state)) return state;
    await new Promise(resolve => setTimeout(resolve, 15));
  }
  throw Error(label + ' timed out: ' + JSON.stringify(state));
}
async function capture(name, predicate = () => true) {
  let frame;
  const deadline = Date.now()+5000;
  // UART and stats can arrive before the rate-limited frame delivery.
  do {
    frame=await runtime.capture({format:'rgba'});
    if(predicate(frame))break;
    await new Promise(resolve=>setTimeout(resolve,20));
  } while(Date.now()<deadline);
  assert(predicate(frame),'Timed out waiting for completed '+name+' frame');
  assert.deepEqual([frame.width, frame.height], [172, 320]);
  await writeFile(join(output, name + '.rgba'), frame.pixels);
  return frame;
}
const changedPixels = (a,b) => {
  let n=0;
  for(let i=0;i<a.length;i+=4) if(a[i]!==b[i]||a[i+1]!==b[i+1]||a[i+2]!==b[i+2]||a[i+3]!==b[i+3]) n++;
  return n;
};
try {
  for (const productId of ['waveshare-esp32-c5-lcd-1-47','waveshare-esp32-c5-lcd-1-47-m']) {
    const product = catalog.products.find(p=>p.id===productId);
    assert.deepEqual([product.runtime.flashMb, product.runtime.psramMb], [4,0]);
    const factory = catalog.firmwares.find(f=>f.id===product.defaultFirmwareId);
    assert.equal(factory.role,'factory'); assert.equal(factory.status,'trial'); assert.equal(factory.simulationVerified,false); assert.equal(factory.source.bootMode,'rom');
    await runtime.configure(createDefaultScene(catalog,productId));
    const caps = await runtime.capabilities();
    assert.deepEqual(caps.execution.bootModes,['app-direct']);
    assert.equal(caps.network.available,false); assert.equal(caps.deviceStorage.available,false); assert.equal(caps.sdCard.available,true);
    state=await runtime.observe(); assert.equal(state.sdCard.bytes,64*1024*1024); assert.equal(state.sdCard.format,'fat32');
    await runtime.loadFirmware(firmware.source);
    const before=await runtime.exportSdCard();
    await runtime.start();
    await waitFor('diagnostic completion',s=>serial(s).includes('C5 LCD147 READY SD PASS\n')&&s.frame);
    // Wait one more public stats/frame cycle after the UART marker.
    const readyTime=state.stats.time;
    await waitFor('completed frame',s=>s.stats.time>=readyTime+.03);
    const prefix=productId.endsWith('-m')?'headers':'base';
    const ready=await capture(prefix+'-ready');
    const colors=new Set();for(let i=0;i<ready.pixels.length;i+=4)colors.add(ready.pixels.slice(i,i+3).join(','));
    assert(colors.size>100,'Guest must render a nontrivial diagnostic');
    const quietStart=state.stats.time; await waitFor('quiet control',s=>s.stats.time>=quietStart+.03);
    const quiet=await capture(prefix+'-quiet'); assert.equal(changedPixels(ready.pixels,quiet.pixels),0);
    let uartOffset=serial(state).length;
    const down=await runtime.input({type:'button',id:'boot',pressed:true});
    await waitFor('BOOT down',s=>serial(s).slice(uartOffset).includes('C5 BOOT DOWN\n')&&s.stats.time>=down.guestTime+.02);
    const pressed=await capture(prefix+'-pressed',f=>changedPixels(ready.pixels,f.pixels)===140*256), changed=changedPixels(ready.pixels,pressed.pixels);
    assert.equal(changed,140*256);
    uartOffset=serial(state).length;
    const up=await runtime.input({type:'button',id:'boot',pressed:false});
    await waitFor('BOOT release',s=>serial(s).slice(uartOffset).includes('C5 BOOT UP\n')&&s.stats.time>=up.guestTime+.02);
    const released=await capture(prefix+'-released',f=>changedPixels(ready.pixels,f.pixels)===0); assert.equal(changedPixels(ready.pixels,released.pixels),0);
    await runtime.pause(); state=await runtime.observe();
    const after=await runtime.exportSdCard(); assert.equal(after.bytes.length,before.bytes.length);
    assert.equal(sha256(after.bytes),sha256(before.bytes),'The entire default FAT32 card must be restored');
    assert(serial(state).includes('C5 SD ORIGINAL BLOCK RESTORED\n'));
    const proof={productId,status:'passed',capabilities:caps,flashMb:4,psramMb:0,
      frame:{width:172,height:320,rgbaSha256:sha256(ready.pixels),colors:colors.size,quietChangedPixels:0,bootChangedPixels:changed,releaseRestoredExactly:true},
      sd:{bytes:before.bytes.length,format:'fat32',beforeSha256:sha256(before.bytes),afterSha256:sha256(after.bytes),originalImageRestoredExactly:true,guestFilesystemMounted:false},
      inputs:[down,up],stats:state.stats,uart:serial(state)};
    products.push(proof); console.log(JSON.stringify({productId,status:'passed',frame:proof.frame,guestSeconds:state.stats.time}));
  }
  result={status:'passed',execution:'public-runtime-production-worker-wasm-in-node',browserUiVerified:false,factoryVerified:false,products};
} catch(error) {
  state=await runtime.observe(); result={status:'failed',message:error.message,code:error.code,lastError:state.lastError,stats:state.stats,uart:serial(state),products};
  process.exitCode=1;
} finally {
  Object.assign(result,{schemaVersion:1,checkedAtUTC:new Date().toISOString(),firmwareSha256:sha256(bytes),wasmSha256:sha256(await readFile(new URL('web/wasm/esp32sim.wasm',root))),wallSeconds:(Date.now()-started)/1000,requests,workerMessages:messages});
  await writeFile(join(output,'result.json'),JSON.stringify(result,null,2)+'\n');
  console.log(JSON.stringify({status:result.status,message:result.message,wallSeconds:result.wallSeconds}));
  await runtime.dispose();
}
