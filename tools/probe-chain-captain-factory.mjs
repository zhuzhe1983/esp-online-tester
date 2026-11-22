#!/usr/bin/env node
// Chain Captain C160 factory UserDemo acceptance: real ROM boot in the page worker WASM,
// framebuffer capture, physical button input on G1/G4/G5 and the M5PM1 power key.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {basename} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {installAutomation} from '../web/platform/automation.js';

const args=process.argv.slice(2);
const arg=name=>{const i=args.indexOf(name);return i>=0?args[i+1]:undefined;};
const url=arg('--url')||'https://github.com/m5stack/M5Chain-Captain-UserDemo/releases/download/V0.2/C160-M5Chain-Captain-UserDemo-V0.2_0x0.bin';
const expected=arg('--sha256')||'fe8404adc64c12638eeec92be6fd545ec456a257bd784509b6c269ee6edc67b0';
const seconds=Number(arg('--seconds')||4);
const dump=args.includes('--dump-serial');
assert(/^https:\/\//.test(url));assert(/^[a-f0-9]{64}$/.test(expected));
const productId='m5stack-c160';
const root=new URL('../',import.meta.url),baseUrl='https://chain-captain-factory.invalid/platform/';
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const response=await fetch(url,{signal:AbortSignal.timeout(60000)});assert(response.ok,`Firmware HTTP ${response.status}`);
const firmware=new Uint8Array(await response.arrayBuffer());assert.equal(hash(firmware),expected,'Changed firmware must be separately reviewed.');
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const fetchImpl=async value=>{const target=new URL(value);if(String(target)===url)return new Response(firmware);assert.equal(target.origin,'https://chain-captain-factory.invalid');try{return new Response(await readFile(new URL(target.pathname.endsWith('.elf')?'web/wasm/fw/'+basename(target.pathname):'web'+target.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root),thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl,assetBaseUrl:'https://chain-captain-factory.invalid/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'chain-captain-factory-probe'})});
const receipt={productId,checkedAt:new Date().toISOString(),command:'node tools/probe-chain-captain-factory.mjs [--url <factory url> --sha256 <factory sha256> --seconds <n> --dump-serial]',source:{url,resolvedUrl:response.url,bytes:firmware.length,sha256:hash(firmware)},backend:'actual-page-worker-wasm-jit',firmwarePatched:false,wasmSha256:hash(await readFile(new URL('web/wasm/esp32sim.wasm',root))),checks:[],observations:[]};
const wait=async predicate=>{const end=Date.now()+120000;while(Date.now()<end){const state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(JSON.stringify(state.lastError));await new Promise(r=>setTimeout(r,50));}throw Error(`Factory probe timed out: ${(await runtime.observe()).serial.slice(-3000)}`);};
const advance=async secs=>{const start=(await runtime.observe()).stats.time||0;return wait(s=>(s.stats.time||0)>=start+secs);};
const frameSha=pixels=>hash(Buffer.from(pixels.buffer,pixels.byteOffset,pixels.byteLength));
try {
 const scene=createDefaultScene(catalog,productId);
 assert.equal(scene.productId,productId);
 scene.view={...scene.view,mode:'embed',deviceView:'3d'};
 scene.firmware={url,sha256:expected};
 await runtime.configure(scene);
 await runtime.loadFirmware();
 await runtime.start();
 await advance(seconds);
 const booted=await runtime.pause();
 receipt.firmware=booted.firmware;receipt.stats=booted.stats;
 const frame=await runtime.capture({format:'rgba'});
 assert.equal(frame.width,240);assert.equal(frame.height,240);
 assert(new Set(frame.pixels).size>10,'Expected a real nonblank factory framebuffer');
 receipt.checks.push(`rom-boot-nonblank-240x240-colours-${new Set(frame.pixels).size}`);
 receipt.frameSha256=frameSha(frame.pixels);
 receipt.serialBytes=booted.serial.length;
 receipt.serialUart0Bytes=(booted.serialByPort.uart0||'').length;
 if(dump)console.log('=== uart0 ===\n'+(booted.serialByPort.uart0||'').slice(-6000));
 const uart0=booted.serialByPort.uart0||'';
 // Factory identity and the modelled I2C subset must actually answer.
 for(const marker of ['Project name:     ChainCaptain-UserDemo','App version:      V0.2','ESP-IDF:          v5.5.5','Found 8MB PSRAM device','M5PM1 initialized at address 0x6E','HAL-IMU: BMI270 initialized','HAL-RGB: RGB initialized','HAL-IR: IR initialized','HAL-DISP: display initialized','[Launcher] on create']) {
   assert(uart0.includes(marker),`Factory boot log is missing ${marker}`);
   receipt.checks.push('boot-log:'+marker);
 }
 // Unmodelled peripherals must show up as guest-visible failures, not silence.
 receipt.unmodelled=Object.fromEntries([...uart0.matchAll(/HAL-RTC: RX8130 [a-z]+ read failed: (\w+)|HAL-RTC: RX8130 flag read failed: (\w+)/g)].map(m=>[m[0],m[1]||m[2]]));
 assert(uart0.includes('HAL-RTC: RX8130'),'Expected the guest RTC driver to report the unmodelled 0x32 device');
 await runtime.start();
 const api=installAutomation(runtime,{target:{location:{href:baseUrl}}});
 // Physical user keys on G1/G4/G5: press, hold sample, release sample.
 for(const id of ['button-a','button-b','button-c']) {
   const before=(await runtime.observe()).serial.length;
   await api.press(id,{durationMs:180});
   const held=await runtime.capture({format:'rgba'});
   const state=await advance(0.4);
   const added=state.serial.slice(before);
   const released=await runtime.capture({format:'rgba'});
   receipt.observations.push({id,input:'board gpio '+id,serialDeltaBytes:added.length,frameChanged:Buffer.from(held.pixels).equals(Buffer.from(released.pixels))===false,heldFrameSha256:frameSha(held.pixels),releasedFrameSha256:frameSha(released.pixels),serialDelta:added.slice(0,400)});
 }
 // M5PM1 power key: single click is the factory wake/start gesture, double click powers off.
 const powerPress=await runtime.input({type:'button',id:'power',pressed:true});
 await advance(0.4);
 const powerState=await runtime.observe();
 receipt.powerInputMessage=powerPress.message;
 receipt.hardwareDiagnostics=powerState.hardwareDiagnostics;
 const afterPower=await runtime.capture({format:'rgba'});
 receipt.checks.push(`power-key-accepted:${!!powerPress.message}`);
 receipt.leds=powerState.leds;receipt.imu=powerState.imu;receipt.frame=powerState.frame;
 receipt.status='pass';
 receipt.guestSeconds=powerState.stats.time;
 receipt.artifacts=(powerState.firmware.artifacts||[]).map(({kind,sha256,bytes})=>({kind,sha256,bytes}));
 receipt.serialEvidence=[...new Set((powerState.serialByPort.uart0||'').split(/\r?\n/).filter(line=>/boot:|chain|captain|Hal|hal|IOE|PM1|esp_image|cpu_start|gpio|i2c|spi/i.test(line)))].slice(0,40);
 if(dump)console.log('=== final uart0 tail ===\n'+(powerState.serialByPort.uart0||'').slice(-4000));
 console.log(JSON.stringify(receipt,null,2));
} finally {await runtime.dispose();}
