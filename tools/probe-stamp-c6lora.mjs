#!/usr/bin/env node
// Real source-built C6 firmware, platform API and worker/WASM. No output mocks.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {resolve,join,basename} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {inspectLocalBuild} from '../web/platform/local-firmware.js';
import {createDefaultScene} from '../web/platform/catalog.js';
const argument=name=>{const i=process.argv.indexOf(name);return i<0?null:process.argv[i+1];};
const build=argument('--build');if(!build)throw Error('Supply --build <ESP-IDF 5.5.1 stamp-c6lora-firmware build directory>.');
const folder=resolve(build),root=new URL('../',import.meta.url),origin='https://stamp-c6lora-fixture.invalid',hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const files=[new File([await readFile(join(folder,'flasher_args.json'))],'flasher_args.json')],inputHashes={};
for(const path of (await inspectLocalBuild(files)).missingFiles){const bytes=await readFile(join(folder,path)),file=new File([bytes],basename(path));inputHashes[path]=hash(bytes);Object.defineProperty(file,'webkitRelativePath',{value:path});files.push(file);}
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const fetchImpl=async value=>{const url=new URL(value);assert.equal(url.origin,origin);try{return new Response(await readFile(new URL('web'+url.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{
 const real=new URL('web'+url.pathname,root),thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));
 const adapter={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;
};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'stamp-c6lora-probe'})});
const receipt={checkedAt:new Date().toISOString(),command:'node tools/probe-stamp-c6lora.mjs --build <fixture-build-directory>',backend:'actual-wasm-worker',firmware:'internal ESP-IDF 5.5.1 protocol fixture',productId:'m5stack-s012',wasmSha256:hash(await readFile(new URL('web/wasm/esp32sim.wasm',root))),inputHashes,checks:[],outputs:[],limitations:['Not official factory firmware compatibility or physical hardware validation.','LoRa packets are explicit digital peer inputs; no RF propagation, receive gain, antenna loss, Wi-Fi association, LoRaWAN service or calibrated silicon timing.','The bare module has no framebuffer, RGB or mapped user button.']};
const delay=ms=>new Promise(r=>setTimeout(r,ms));
const serialEvidence=new Set();
const observe=async()=>{const state=await runtime.observe(),serial=state.serialByPort.uart0??state.serial;for(const line of serial.split(/\r?\n/).slice(0,-1))if(/^C6LORA_/.test(line))serialEvidence.add(line);return {...state,serial};};
const rxComplete=(state,at,line)=>{const part=state.serial.slice(at),found=part.indexOf(line);return found>=0&&part.slice(found+line.length).includes('C6LORA_RX_READY');};
let traced=0;
const wait=async(label,predicate)=>{const end=Date.now()+Number(argument('--timeout-ms')||120000);while(Date.now()<end){const state=await observe();if(process.argv.includes('--verbose')){if(state.serial.length<traced)traced=0;process.stderr.write(state.serial.slice(traced));traced=state.serial.length;}if(predicate(state))return state;if(state.status==='error')throw Error(JSON.stringify(state.lastError));await delay(20);}const state=await observe();throw Error(`${label} timed out; stats=${JSON.stringify(state.stats)} radio=${JSON.stringify(await runtime.readLoRa())}\n${state.serial.slice(-6000)}`);};
const serialCommand=async(text,marker)=>{const at=(await observe()).serial.length;await runtime.input({type:'serial',port:'uart0',data:text});return wait(marker,s=>s.serial.slice(at).includes(marker));};
try {
 const scene=createDefaultScene(catalog,'m5stack-s012');scene.firmware=null;await runtime.configure(scene);await runtime.loadFirmware({format:'build',files});await runtime.start();
 let state=await wait('C6 boot, routed Tx and RX mode',s=>s.serial.includes('C6LORA_RX_READY'));
 const mode=argument('--spi-mode');if(mode)assert(state.serial.includes(`C6LORA_SPI_MODE ${mode}`));receipt.spiMode=mode||'unspecified';
 for(const line of ['C6LORA_IO_ID a2','C6LORA_WRONG_ADDRESS NACK','C6LORA_WRONG_SDA NACK','C6LORA_WRONG_CS ff','C6LORA_RADIO SX1261','C6LORA_TX_DONE irq=0001 dio1=1'])assert(state.serial.includes(line),line);
 assert.equal(state.frame,null);assert.equal((await runtime.capabilities()).lora.available,true);
 let radio=await runtime.readLoRa();assert.equal(radio.txCount,1);assert.equal(radio.rxCount,0);assert(radio.transmitted.some(p=>p.payloadHex==='5354414d502d43364c4f5241'));
 receipt.outputs.push({phase:'boot',radio});receipt.checks.push('real-C6-boot-PI4IO-identity-address-and-SDA-negative-SPI-version-and-CS-negative-TX-DIO1');
 const packet={config:radio.config,payloadHex:'41434b',rssiDbm:-62,snrQuarterDb:28,crcError:false};
 await assert.rejects(runtime.receiveLoRa({...packet,config:{...packet.config,frequency:packet.config.frequency+1}}),e=>e.code==='LORA_PACKET_REJECTED');
 await assert.rejects(runtime.receiveLoRa({...packet,payloadHex:'invalid'}));
 let at=state.serial.length;await runtime.receiveLoRa(packet);state=await wait('fresh guest RX payload and DIO1',s=>rxComplete(s,at,'C6LORA_RX irq=0002 dio1=1 bytes=41434b'));
 assert.equal((await runtime.readLoRa()).rxCount,1);receipt.checks.push('matching-explicit-peer-packet-reaches-guest-SPI-buffer-and-DIO1-mismatched-channel-and-malformed-payload-rejected');
 at=state.serial.length;await runtime.receiveLoRa({...packet,payloadHex:'435243',crcError:true});state=await wait('CRC flag reaches guest',s=>rxComplete(s,at,'C6LORA_RX irq=0042 dio1=1 bytes=435243'));receipt.checks.push('CRC-error-flag-is-guest-visible');receipt.outputs.push({phase:'two-peer-packets',radio:await runtime.readLoRa()});
 await serialCommand('a','C6LORA_ANT_OFF');await assert.rejects(runtime.receiveLoRa(packet),e=>e.code==='LORA_PACKET_REJECTED');assert.equal((await runtime.readLoRa()).antennaEnabled,false);
 await serialCommand('l','C6LORA_LNA_OFF');await assert.rejects(runtime.receiveLoRa(packet),e=>e.code==='LORA_PACKET_REJECTED');
 await serialCommand('e','C6LORA_RX_READY');at=(await observe()).serial.length;await runtime.receiveLoRa(packet);await wait('fresh receive after RF path recovery',s=>rxComplete(s,at,'bytes=41434b'));receipt.checks.push('guest-I2C-controls-antenna-switch-and-receive-LNA-recovery-requires-fresh-packet');
 await runtime.pause();const paused=await observe(),pausedRadio=await runtime.readLoRa();await delay(100);assert.equal((await observe()).stats.time,paused.stats.time);assert.equal((await observe()).serial,paused.serial);assert.deepEqual(await runtime.readLoRa(),pausedRadio);receipt.checks.push('pause-freezes-guest-clock-and-radio-state');
 await runtime.start();await serialCommand('r','C6LORA_RESET_LOW');radio=await runtime.readLoRa();assert.equal(radio.resetHigh,false);assert.equal(radio.irq,0);await assert.rejects(runtime.receiveLoRa(packet),e=>e.code==='LORA_PACKET_REJECTED');receipt.outputs.push({phase:'reset-asserted',radio});
 await runtime.reset();state=await wait('fresh platform reboot',s=>s.serial.includes('C6LORA_RX_READY'));radio=await runtime.readLoRa();assert.equal(radio.txCount,1);assert.equal(radio.rxCount,0);receipt.checks.push('guest-reset-pin-clears-radio-and-platform-reset-recreates-fresh-state');
 await serialCommand('b','C6LORA_SOFTWARE_REBOOT');at=(await observe()).serial.indexOf('C6LORA_SOFTWARE_REBOOT');state=await wait('guest software reset reinitializes persistent expander',s=>s.serial.slice(at).includes('C6LORA_RX_READY'));receipt.checks.push('guest-software-reboot-reinitializes-existing-expander-and-radio');
 await runtime.pause();receipt.outputs.push({phase:'software-reboot',radio:await runtime.readLoRa()});receipt.serialEvidence=[...serialEvidence];receipt.status='pass';console.log(JSON.stringify(receipt,null,2));
} catch(error) {receipt.status='failed';receipt.error=String(error.message).split('\n')[0];receipt.serialEvidence=[...serialEvidence];receipt.outputs.push({phase:'failure',radio:await runtime.readLoRa().catch(()=>null)});console.log(JSON.stringify(receipt,null,2));throw error;} finally {await runtime.dispose();}
