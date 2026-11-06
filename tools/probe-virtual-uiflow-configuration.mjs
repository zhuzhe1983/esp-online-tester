#!/usr/bin/env node
// Opt-in original-firmware guest test. Requires existing production WASM + C61 ROM.
// Test credentials are deliberately fictional and never printed in the receipt.
import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createSdCardStorage} from '../web/platform/sd-card-storage.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {createCoreMatrixRepl} from './fixtures/corematrix-uiflow/raw-repl.mjs';

const args=process.argv.slice(2),[file,expectedHash]=args,option=name=>{const i=args.indexOf(name);return i<0?undefined:args[i+1];};
if(!file||!/^[a-f0-9]{64}$/.test(expectedHash||''))throw Error('Supply <original-corematrix.bin> <sha256> [--boot-option 1|2] [--receipt file.json]. Requires web/wasm/esp32sim.wasm and web/wasm/fw/esp32c61_rev100_rom.elf.');
const bootOption=Number(option('--boot-option')||1);assert.ok([1,2].includes(bootOption));
const root=new URL('../',import.meta.url),origin='https://virtual-configuration-fixture.invalid',hash=bytes=>createHash('sha256').update(bytes).digest('hex'),bytes=await readFile(file);
assert.equal(hash(bytes),expectedHash,'Caller supplied original image SHA-256 mismatch');
const wasmSha256=hash(await readFile(new URL('web/wasm/esp32sim.wasm',root))),catalog={};
for(const name of ['products','accessories','firmwares'])catalog[name]=JSON.parse(await readFile(new URL(`web/platform/catalog/${name}.json`,root))).items;
const fetchImpl=async value=>{const url=new URL(value);assert.equal(url.origin,origin);try{return new Response(await readFile(new URL('web'+url.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{
  const real=new URL('web'+url.pathname,root),thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));
  const adapter={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',error=>adapter.onerror?.(error));return adapter;
};
const dbName='virtual-configuration-probe-'+crypto.randomUUID(),profile={name:'Isolated virtual NVS fixture',type:'uiflow2',server:'uiflow2.m5stack.com',ssid:'Virtual Configuration Fixture',password:'Fictional_fixture_password',bootOption};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',deviceStorage:createDeviceStorage({indexedDB:null,BroadcastChannel:null,dbName}),sdStorage:createSdCardStorage({indexedDB:null,BroadcastChannel:null,dbName})});
const repl=createCoreMatrixRepl(runtime),source={file:new File([bytes],'original-corematrix.bin')},checks=[];
const enter=async()=>{await repl.wait(state=>state.serial.includes('MicroPython')&&state.stats.time>1);await repl.enter();};
const verify=async suffix=>{
  const result=await repl.exec(`import esp32\nn=esp32.NVS('uiflow')\nassert n.get_str('server')==${JSON.stringify(profile.server)}\nassert n.get_str('ssid0')==${JSON.stringify(profile.ssid)}\nassert n.get_str('pswd0')==${JSON.stringify(profile.password)}\nassert n.get_u8('boot_option')==${bootOption}\nassert n.get_str('net_mode')=='WIFI'\nassert n.get_str('protocol')=='DHCP'\nprint('VIRTUAL_UIFLOW2_NVS_OK')\n`);
  assert.match(result,/VIRTUAL_UIFLOW2_NVS_OK/);checks.push('guest-NVS-get_str-get_u8-'+suffix);
  if(bootOption===2){
    const network=await repl.exec("import network,time\nw=network.WLAN(network.STA_IF)\nfor _ in range(100):\n if w.isconnected():\n  break\n time.sleep_ms(50)\nassert w.isconnected()\nassert w.ifconfig()[0]=='10.0.2.15'\nprint('VIRTUAL_UIFLOW2_AUTO_NETWORK_OK')\n");
    assert.match(network,/VIRTUAL_UIFLOW2_AUTO_NETWORK_OK/);checks.push('official-startup-automatic-WPA2-and-guest-DHCP-'+suffix);
  }
};
const preserve=async write=>{
  const result=await repl.exec(`import os,hashlib,binascii\n${write?"with open('/flash/virtual-configuration.txt','w') as f:\n f.write('VIRTUAL_CONFIGURATION_FILE')\nos.sync()\n":''}with open('/flash/virtual-configuration.txt') as f:\n assert f.read()=='VIRTUAL_CONFIGURATION_FILE'\nwith open('/flash/boot.py','rb') as f:\n boot_hash=binascii.hexlify(hashlib.sha256(f.read()).digest()).decode()\nprint('VIRTUAL_UIFLOW2_FILE_OK',boot_hash)\n`);
  const digest=result.match(/VIRTUAL_UIFLOW2_FILE_OK ([0-9a-f]{64})/)?.[1];assert.ok(digest);return digest;
};
const sdProgram=write=>`from machine import I2C,Pin,SDCard\nfrom driver.m5ioe1 import M5IOE1\nimport M5,os\nM5.begin()\nbus=I2C(0,sda=Pin(0),scl=Pin(1),freq=400000)\nio=M5IOE1(bus,addr=0x4f)\nio.set_pin_mode(1,1)\nio.set_pin_drive(1,0)\nio.set_pin_value(1,True)\nsd=SDCard(slot=2,sck=Pin(25),miso=Pin(26),mosi=Pin(27),cs=Pin(28),freq=1000000)\nos.mount(sd,'/sd')\n${write?"with open('/sd/virtual-configuration.txt','w') as f:\n f.write('VIRTUAL_CONFIGURATION_SD')\nos.sync()\n":''}with open('/sd/virtual-configuration.txt') as f:\n assert f.read()=='VIRTUAL_CONFIGURATION_SD'\nprint('VIRTUAL_UIFLOW2_SD_OK')\n`;
let receipt;
try {
  const scene=createDefaultScene(catalog,'m5stack-c158');scene.firmware=null;await runtime.configure(scene);await runtime.configureNetwork({autoProvision:false,ssid:profile.ssid,password:profile.password});await runtime.loadFirmware(source);
  const original=await runtime.observe(),inspection=await runtime.inspectFirmwareConfiguration();assert.equal(inspection.available,true);
  await runtime.start();await enter();const bootHash=await preserve(true);assert.match(await repl.exec(sdProgram(true)),/VIRTUAL_UIFLOW2_SD_OK/);
  // Write while running; the API must pause and preserve guest-written Flash/SD.
  const configured=await runtime.writeFirmwareConfiguration(profile,{sessionId:inspection.sessionId});assert.equal(configured.status,'ready');assert.deepEqual(configured.firmware,original.firmware);assert.equal(configured.storage.scope.key,original.storage.scope.key);
  await runtime.start();await enter();await verify('initial-write');assert.equal(await preserve(false),bootHash);assert.match(await repl.exec(sdProgram(false)),/VIRTUAL_UIFLOW2_SD_OK/);checks.push('guest-VFS-and-SPI-SD-preserved-after-NVS-replacement');
  await runtime.reset();await enter();await verify('host-reset');assert.equal(await preserve(false),bootHash);
  await runtime.loadFirmware(source);assert.equal((await runtime.observe()).storage.scope.key,original.storage.scope.key);await runtime.start();await enter();await verify('same-image-reload');assert.equal(await preserve(false),bootHash);assert.match(await repl.exec(sdProgram(false)),/VIRTUAL_UIFLOW2_SD_OK/);
  await runtime.pause();checks.push('original-image-and-boot.py-hashes-unchanged','original-image-storage-scope-retained');
  receipt={checkedAt:new Date().toISOString(),status:'pass',backend:'production-worker-WASM-original-UIFlow2',sourceSha256:expectedHash,wasmSha256,bootOption,checks,bootPySha256:bootHash,configuration:(await runtime.observe()).firmwareConfiguration,guestSeconds:(await runtime.observe()).stats.time,limitations:['Only the virtual Flash NVS partition is rebuilt; no physical device was used.','No UIFlow cloud/M5Things compatibility is inferred from NVS reads.','Probe storage uses an isolated memory backend; browser IndexedDB persistence is covered by the existing storage backend and browser checks.'],privacy:'Only assertion labels and artifact hashes are retained; no NVS values, credentials, source code, serial tails or filesystem contents.'};
  if(option('--receipt'))await writeFile(option('--receipt'),JSON.stringify(receipt,null,2)+'\n');console.log(JSON.stringify(receipt,null,2));
} finally {await runtime.dispose();}
