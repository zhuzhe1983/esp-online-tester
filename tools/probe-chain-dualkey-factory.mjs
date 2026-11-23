// Run official and optional diagnostic binaries in the actual page worker/WASM.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {readFile} from 'node:fs/promises';
import {Worker} from 'node:worker_threads';
import {installAutomation} from '../web/platform/automation.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createPlatformRuntime} from '../web/platform/runtime.js';

const root = new URL('../web/', import.meta.url);
const productId = 'm5stack-c147';
const testOrigin = 'https://chain-dualkey-probe.invalid';
const platform = new URL('/platform/', testOrigin);
const fixtureUrl = new URL('/c147-fixture.bin', testOrigin).href;
const sha256 = data => createHash('sha256').update(data).digest('hex');
const read = path => readFile(new URL(path, root));
const args = process.argv.slice(2);
function arg(name) {
  const index = args.indexOf(name);
  return index < 0 ? null : args[index + 1];
}
const entries = await Promise.all(['products','firmwares','accessories']
  .map(async key => [key, JSON.parse(await read(`platform/catalog/${key}.json`)).items]));
const catalog = Object.fromEntries(entries);
const product = catalog.products.find(entry => entry.id === productId);
const factory = catalog.firmwares.find(entry => entry.id === product.defaultFirmwareId);
const localFactory = arg('--factory');
const fixtureFile = arg('--fixture');
const factoryBytes = localFactory ? await readFile(localFactory) :
  new Uint8Array(await (await fetch(factory.source.url, {signal:AbortSignal.timeout(60000)})).arrayBuffer());
assert.equal(sha256(factoryBytes), factory.source.sha256, 'Factory bytes must match the catalog digest');
const fixture = fixtureFile ? await readFile(fixtureFile) : null;

async function fetchImpl(input) {
  const url = new URL(input);
  if (url.href === factory.source.url) return new Response(factoryBytes);
  if (fixture && url.href === fixtureUrl) return new Response(fixture);
  assert.equal(url.origin, testOrigin);
  try { return new Response(await read(url.pathname.slice(1))); }
  catch (error) {
    if (error.code !== 'ENOENT') throw error;
    return new Response(null,{status:404});
  }
}

function workerFactory(url) {
  const path = new URL(url.pathname.slice(1),root);
  const script = `import {parentPort} from 'node:worker_threads';
    globalThis.onmessage = null;
    globalThis.postMessage = (message, transfer) => parentPort.postMessage(message, transfer);
    await import(${JSON.stringify(path.href)});
    parentPort.on('message', data => globalThis.onmessage({data}));`;
  const thread = new Worker(new URL(`data:text/javascript,${encodeURIComponent(script)}`));
  const worker = {postMessage:(...values)=>thread.postMessage(...values), terminate:()=>thread.terminate()};
  thread.on('message', data=>worker.onmessage?.({data}));
  thread.on('error', error=>worker.onerror?.(error));
  return worker;
}

const runtime = createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl:platform.href,assetBaseUrl:testOrigin+'/',
  deviceStorage:createDeviceStorage({indexedDB:null,dbName:'c147-wasm-probe'})});
const api = installAutomation(runtime,{target:{location:{href:platform.href}}});
const pins = state => [0n,17n].map(pin=>Number((BigInt('0x'+state.stats.gpio_in)>>pin)&1n));
const elapsed = state => Number(state.stats.time ?? 0);

async function waitFor(predicate, meaning) {
  const stop = Date.now() + 90000;
  while (Date.now() < stop) {
    const state = await api.observe();
    if (predicate(state)) return state;
    if (state.status === 'error') throw Error(meaning + ': ' + JSON.stringify(state.lastError));
    await new Promise(resolve=>setTimeout(resolve,20));
  }
  const state = await api.observe();
  throw Error(`${meaning} timed out at guest ${elapsed(state)} s, LEDs ${JSON.stringify(state.leds)}`);
}

async function open(source) {
  await api.configure({...createDefaultScene(catalog,productId), firmware:source});
  await api.loadFirmware();
  await api.start();
  const result = await api.observe();
  assert.equal(result.firmware.appDirect,false);
  assert.equal(result.firmware.executionMode,'wasm-jit');
  return result.firmware.artifacts.map(({kind,sha256,bytes})=>({kind,sha256,bytes}));
}

const report = {productId, checkedAt:new Date().toISOString(), backend:'page-worker-wasm-jit',
  wasmSha256:sha256(await read('wasm/esp32sim.wasm')), factory:{source:factory.source.url, patched:false,
    transport:localFactory?'local verified publisher download':'publisher URL'}};
try {
  report.factory.artifacts = await open(factory.source);
  const markers = [
    'ESP-ROM:esp32s3-20210327','SPI_FAST_FLASH_BOOT','Project name:     M5DualKey-UserDemo',
    'App version:      V0.7-2-g82d7fa6','ESP-IDF:          v5.5.1-dirty',
    'UART1 initialized successfully (TX:48, RX:47, Baud:115200)',
    'UART2 initialized successfully (TX:6, RX:5, Baud:115200)',
    'Battery: 0.00V [0%] (LOW!)','BLE_INIT: BT controller compile version',
  ];
  const same = (a,b) => JSON.stringify(a) === JSON.stringify(b);
  const boot = await waitFor(state=>elapsed(state)>=2.8 &&
    markers.every(marker=>state.serialByPort.usb?.includes(marker)) &&
    same(state.leds,[[255,0,0],[255,0,0]]),'V0.8 ROM boot, application and two red LEDs');
  assert.equal(boot.frame,null);
  assert.deepEqual(pins(boot),[1,1]);
  report.factory.boot={markers, guestSeconds:elapsed(boot), leds:boot.leds, frame:boot.frame, gpio:pins(boot)};
  report.factory.input=[];
  for (const chord of [['key1'],['key2'],['key1','key2']]) {
    const before = await api.observe();
    for (const id of chord) await api.input({type:'button',id,pressed:true});
    const expected = ['key1','key2'].map(id=>Number(!chord.includes(id)));
    const held = await waitFor(state=>elapsed(state)>=elapsed(before)+(chord.length===2?5:.25) &&
      same(pins(state),expected), 'factory GPIO pressure');
    for (const id of chord) await api.input({type:'button',id,pressed:false});
    const released = await waitFor(state=>elapsed(state)>=elapsed(held)+.4 && same(pins(state),[1,1]),
      'factory GPIO release');
    const extra = released.serialByPort.usb.slice(before.serialByPort.usb.length);
    report.factory.input.push({chord,held:pins(held),released:pins(released),
      heldSeconds:elapsed(held),releaseSeconds:elapsed(released),leds:released.leds,
      callbackObserved:extra.includes('keyboard_cb:'),ledChangeObserved:!same(released.leds,before.leds)||!same(held.leds,before.leds)});
  }

  if (fixture) {
    const source = {url:fixtureUrl,sha256:sha256(fixture),bootMode:'rom'};
    report.fixture={artifacts:await open(source), source:'locally built IDF C147_GPIO_RGB fixture routed through a test-only origin'};
    const keys = async (one,two,previous='') => {
      const text = `C147 KEYS ${one} ${two}`, leds=[[0,two ? 191 : 0,0],[one ? 127 : 0,0,0]];
      const state = await waitFor(state=>state.serialByPort.uart0?.slice(previous.length).includes(text) &&
        same(state.leds,leds),text);
      return {state,text,leds};
    };
    const ready = await keys(0,0);
    assert(ready.state.serialByPort.uart0.includes('C147 READY'));
    await assert.rejects(api.input({type:'button',id:'key3',pressed:true}),error=>error.code==='UNSUPPORTED_CONTROL');
    const cycle = async () => {
      const observations=[];
      let previous=(await api.observe()).serialByPort.uart0;
      for (const [id,pressed,one,two] of [['key1',true,1,0],['key2',true,1,1],['key1',false,0,1],['key2',false,0,0]]) {
        await api.input({type:'button',id,pressed});
        const result = await keys(one,two,previous);
        previous=result.state.serialByPort.uart0;
        // Leave observable separation between edges for recorded playback.
        const next=elapsed(result.state)+.15;
        await waitFor(state=>elapsed(state)>=next,'fixture edge spacing');
        observations.push({id,pressed,guestMarker:result.text,leds:result.leds,guestSeconds:elapsed(result.state)});
      }
      return observations;
    };
    report.fixture.inputs=await cycle();
    await api.recordingStart();
    const reset=await keys(0,0);
    assert(reset.state.serialByPort.uart0.includes('C147 READY'));
    await cycle();
    const recording=await api.recordingStop();
    assert.equal(recording.events.length,4);
    const replay=await api.recordingPlay(recording);
    const after=await keys(0,0);
    const observed=[...after.state.serialByPort.uart0.matchAll(/C147 KEYS [01] [01]/g)].map(match=>match[0]);
    assert.deepEqual(observed,['C147 KEYS 0 0', ...report.fixture.inputs.map(row=>row.guestMarker)]);
    assert.equal(replay.result,'completed');
    assert.equal(replay.events.length,4);
    report.fixture.replay={result:replay.result,eventCount:replay.events.length, guestMarkers:observed, leds:after.leds};
  }
  console.log(JSON.stringify(report,null,2));
} finally { await api.dispose(); }
