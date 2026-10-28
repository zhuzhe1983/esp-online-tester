#!/usr/bin/env node
// Explicit external-image test: full ROM boot and real worker/API inputs.
// Supply TEMBED_FACTORY_BIN; no third-party firmware is mirrored or substituted.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {readFile,writeFile} from 'node:fs/promises';
import {Worker} from 'node:worker_threads';
import {basename,join} from 'node:path';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {installAutomation} from '../web/platform/automation.js';

assert.ok(process.env.TEMBED_FACTORY_BIN,'Set TEMBED_FACTORY_BIN to the pinned official firmware.bin; see docs/platform/lilygo-tembed-runtime.md');
const firmware=await readFile(process.env.TEMBED_FACTORY_BIN);
const sha256=createHash('sha256').update(firmware).digest('hex');
assert.equal(sha256,'e7a00ad05e6228d1508c82c79993c2bde25c6be415df611a4b7cae66c119c050');
const root=new URL('../',import.meta.url),baseUrl='https://platform.example/platform/';
const catalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async name=>[name,JSON.parse(await readFile(new URL(`web/platform/catalog/${name}.json`,root),'utf8')).items])));
const product=catalog.products.find(item=>item.id==='lilygo-t-embed-cc1101');
const buttons=Object.fromEntries(product.controls.filter(item=>Number.isInteger(item.pin??item.gpio)).map(item=>[item.pin??item.gpio,item.id]));
const fileFetch=async value=>{
  const url=new URL(value);
  if(url.pathname==='/factory.bin') return new Response(firmware);
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
const receipt={firmwareSha256:sha256,boot:'rom',checkpoints:[]};
const guestTime=async seconds=>{
  const end=Date.now()+180000;
  while(Date.now()<end) {
    const state=await api.observe();
    assert.notEqual(state.status,'error',JSON.stringify(state.lastError));
    if(Number(state.stats.time)>=seconds) return state;
    await new Promise(resolve=>setTimeout(resolve,50));
  }
  throw new Error(`Guest did not reach ${seconds} seconds`);
};
const frameHash=frame=>{
  let hash=0xcbf29ce484222325n;
  for(let i=0;i<frame.pixels.length;i+=4) {
    const pixel=((frame.pixels[i]>>3)<<11)|((frame.pixels[i+1]>>2)<<5)|(frame.pixels[i+2]>>3);
    for(const b of [pixel&255,pixel>>8]) hash=BigInt.asUintN(64,(hash^BigInt(b))*0x100000001b3n);
  }
  return hash.toString(16).padStart(16,'0');
};
const checkpoint=async (name,expected)=>{
  const end=Date.now()+30000;
  let hash;
  while(Date.now()<end) {
    const frame=await api.capture({format:'rgba'});
    assert.deepEqual([frame.width,frame.height],[320,170]);
    hash=frameHash(frame);
    if(hash===expected) {
      const state=await api.observe();
      receipt.checkpoints.push({name,guestSeconds:state.stats.time,frameFnv64:hash,insns:state.stats.insns});
      console.log(`ok factory ${name}: ${hash}`);return;
    }
    await new Promise(resolve=>setTimeout(resolve,100));
  }
  assert.equal(hash,expected,`factory ${name} framebuffer`);
};
const hold=async (pin,seconds)=>{
  const start=Number((await api.observe()).stats.time);
  await api.input({type:'button',id:buttons[pin],pressed:true});
  try {await guestTime(start+seconds);}
  finally {await api.input({type:'button',id:buttons[pin],pressed:false});}
};
const advance=async seconds=>guestTime(Number((await api.observe()).stats.time)+seconds);
const turn=async steps=>{await api.input({type:'encoder',id:'encoder',steps});await advance(.2);};
const tap=async pin=>{await hold(pin,.12);await advance(.15);};
const ledLevel=async level=>{
  const end=Date.now()+15000;
  while(Date.now()<end) {
    const state=await api.observe();
    if(state.leds.length===8 && state.leds.every(rgb=>Math.max(...rgb)===level)) {
      receipt.checkpoints.push({name:`rmt-brightness-${level}`,leds:state.leds,guestSeconds:state.stats.time});
      console.log(`ok actual GPIO14 RMT output: 8 LEDs at ${level}`);return;
    }
    await new Promise(resolve=>setTimeout(resolve,30));
  }
  assert.fail(`Guest RGB output did not reach ${level}`);
};
try {
  await api.configure(createDefaultScene(catalog,product.id));
  await api.loadFirmware({url:'https://platform.example/factory.bin',sha256,bootMode:'rom'});
  await api.start();await guestTime(2);
  await api.waitFor({serialIncludes:'Hardware profile: Mixed / Unknown  allowMenu=1',timeoutMs:30000});
  await tap(6);await checkpoint('menu','61fc3a6e3c6aff88');
  await turn(1);await checkpoint('quadrature-clockwise','df3395d50c34b146');
  await turn(-1);await checkpoint('quadrature-counterclockwise','61fc3a6e3c6aff88');
  for(let i=0;i<8;i++) await tap(6);
  await tap(0);await checkpoint('tft-summary','b1bdc75bf4e7ed76');
  // The actual factory page binds positive rotation to its Back focus.
  await turn(1);await tap(0);await tap(6);await tap(0);
  await ledLevel(24);await tap(6);await ledLevel(48);
  // Leaving the page executes the guest's strip.clear()/show(), not a UI reset.
  await turn(1);await tap(0);
  const end=Date.now()+10000;
  while(Date.now()<end) {
    if((await api.observe()).leds.every(rgb=>rgb.every(value=>value===0))) break;
    await new Promise(resolve=>setTimeout(resolve,30));
  }
  assert.deepEqual((await api.observe()).leds,Array.from({length:8},()=>[0,0,0]));
  const state=await api.observe();
  assert.ok(!state.serial.includes('RTCWDT')&&!state.serial.includes('Guru Meditation'));
  receipt.passed=true;
  receipt.scope='ROM boot; incomplete hardware profile retained; SPI display, GPIO0/6, quadrature, RMT8; no analog/radio/sleep claim';
  if(process.env.TEMBED_RECEIPT) await writeFile(process.env.TEMBED_RECEIPT,JSON.stringify(receipt,null,2)+'\n');
} finally {await api.dispose();}
