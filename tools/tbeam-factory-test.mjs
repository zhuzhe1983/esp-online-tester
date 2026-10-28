#!/usr/bin/env node
// Exact official factory image through the public runtime and worker, without
// app-direct boot or stubs. Supply the downloaded, hash-pinned input explicitly.
// Keep this opt-in check outside the platform-*-test.mjs CI glob, like the other
// device factory checks: its external firmware input is not a CI fixture.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {Worker} from 'node:worker_threads';
import {basename,join} from 'node:path';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {installAutomation} from '../web/platform/automation.js';

assert.ok(process.env.TBEAM_FACTORY_BIN,'Set TBEAM_FACTORY_BIN to the pinned official factory image');
const root=new URL('../',import.meta.url),baseUrl='https://platform.example/platform/';
const catalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async name=>[name,JSON.parse(await readFile(new URL(`web/platform/catalog/${name}.json`,root),'utf8')).items])));
const product=catalog.products.find(p=>p.id==='lilygo-t-beam-v1-2');
const firmware=catalog.firmwares.find(f=>f.id===product.defaultFirmwareId);
const fileFetch=async value=>{
  const url=new URL(value);
  if(url.href===firmware.source.url) return new Response(await readFile(process.env.TBEAM_FACTORY_BIN));
  assert.equal(url.origin,'https://platform.example','Unexpected network target');
  const path=new URL(`web${url.pathname}`,root);
  try {return new Response(await readFile(path));}
  catch(error) {
    if(error.code==='ENOENT'&&process.env.ESP32SIM_ROM_DIR&&basename(path.pathname).endsWith('_rom.elf')) return new Response(await readFile(join(process.env.ESP32SIM_ROM_DIR,basename(path.pathname))));
    return new Response('Missing test artifact',{status:404});
  }
};
const workerFactory=url=>{
  const source=new URL(`web${url.pathname}`,root);
  const script=`import {parentPort} from 'node:worker_threads'; globalThis.onmessage=null; globalThis.postMessage=(message,transfer)=>parentPort.postMessage(message,transfer); await import(${JSON.stringify(String(source))}); parentPort.on('message',data=>globalThis.onmessage({data}));`;
  const thread=new Worker(new URL(`data:text/javascript,${encodeURIComponent(script)}`));
  const adapter={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};
  thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',error=>adapter.onerror?.(error));return adapter;
};
const runtime=createPlatformRuntime({catalog,fetchImpl:fileFetch,workerFactory,baseUrl,assetBaseUrl:'https://platform.example/'});
const api=installAutomation(runtime,{target:{location:{href:baseUrl}}});
// Full factory GNSS baud probing executes billions of guest instructions.
// Bound host wait independently of the unchanged guest-time checkpoints.
const guest=async seconds=>{
  const until=Date.now()+300000;
  while(Date.now()<until) {const s=await api.observe();assert.notEqual(s.status,'error',JSON.stringify(s.lastError));if(Number(s.stats.time)>=seconds)return;await new Promise(r=>setTimeout(r,20));}
  throw Error('Guest-time timeout');
};
const press=async(id,duration=.15)=>{
  const start=Number((await api.observe()).stats.time);
  await api.input({type:'button',id,pressed:true});
  try {await guest(start+duration);} finally {await api.input({type:'button',id,pressed:false});}
  await guest(start+duration+.65);
};
try {
  await api.configure(createDefaultScene(catalog,product.id));
  await api.loadFirmware(firmware.source);await api.start();
  // SoftAP logs before setup's remaining delays and initial blocking radio TX.
  // The native test reaches its first interactive checkpoint at guest 42 s.
  await guest(42);
  assert.ok((await api.observe()).serialByPort.uart0.includes('SoftAP started'),'Factory startup completed by guest 42 s');
  await press('user');await api.waitFor({port:'uart0',serialIncludes:'Start transmit',timeoutMs:30000});
  await api.waitFor({port:'uart0',serialIncludes:'transmission finished',timeoutMs:30000});
  for(let i=0;i<12;i++) {if((await api.observe()).serial.includes('Start receive'))break;await press('user');await guest(Number((await api.observe()).stats.time)+i*.013);}
  assert.ok((await api.observe()).serial.includes('Start receive'),'GPIO38 selected RX');
  assert.equal((await api.readLoRa()).rxCount,0,'No automatic loopback or injected startup packet');
  const packet={config:{frequency:0x36400000,sf:10,bandwidth:4,codingRate:2,syncWord:0x1424,implicitHeader:false,crc:false,invertIq:false},payloadHex:'07000000',rssiDbm:-70,snrQuarterDb:24,crcError:false};
  await assert.rejects(api.receiveLoRa({...packet,config:{...packet.config,frequency:0x36400040}}),e=>e.code==='LORA_PACKET_REJECTED');
  await api.receiveLoRa(packet);await api.waitFor({port:'uart0',serialIncludes:'[RadioRx] Received packet! COUNT:7',timeoutMs:30000});
  await press('power',.2);await api.waitFor({port:'uart0',serialIncludes:'isPekeyShortPress',timeoutMs:30000});
  await api.input({type:'serial',port:'uart0',data:'freq:915.0\n'});await api.waitFor({port:'uart0',serialIncludes:'Frequency set to 915.00 MHz',timeoutMs:30000});
  await api.pause();const s=await api.observe(),radio=await api.readLoRa();
  assert.equal(radio.config.frequency,959447040);assert.equal(radio.rxCount,1);assert.ok(radio.txCount>0);
  for(const text of ['AXP2101 PMU init succeeded','[SX1276]:LoRa Initialized Success!','Failed to find Display','PSRAM is disable!']) assert.ok(s.serial.includes(text),text);
  assert.equal((s.serial.match(/setupBoards/g)||[]).length,1);assert.equal(s.firmware.appDirect,false);assert.deepEqual(s.firmware.stubs,[]);
  console.log(JSON.stringify({passed:true,guestSeconds:s.stats.time,instructions:s.stats.insns,txCount:radio.txCount,rxCount:radio.rxCount,frequency:radio.config.frequency,sourceSha256:s.firmware.artifacts.find(a=>a.kind==='flash').sha256}));
} finally {await api.dispose();}
