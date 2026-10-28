#!/usr/bin/env node
// Explicit external-image test: full ROM boot and real worker/API inputs.
// Supply TDISPLAY_FACTORY_BIN; no third-party firmware is mirrored or substituted.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {readFile,writeFile} from 'node:fs/promises';
import {Worker} from 'node:worker_threads';
import {basename,join} from 'node:path';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {installAutomation} from '../web/platform/automation.js';

assert.ok(process.env.TDISPLAY_FACTORY_BIN,'Set TDISPLAY_FACTORY_BIN to the pinned official firmware.bin; see docs/evidence/classic-light-sleep-tdisplay/README.md');
const firmware=await readFile(process.env.TDISPLAY_FACTORY_BIN);
const sha256=createHash('sha256').update(firmware).digest('hex');
assert.equal(sha256,'85a2598f363a96c4d4eb4aebad5af81c286653eab0d20543110546e4d2c7b9ed');
const root=new URL('../',import.meta.url),baseUrl='https://platform.example/platform/';
const catalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async name=>[name,JSON.parse(await readFile(new URL(`web/platform/catalog/${name}.json`,root),'utf8')).items])));
const product=catalog.products.find(item=>item.id==='lilygo-ttgo-t-display');
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
    assert.deepEqual([frame.width,frame.height],[135,240]);
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
try {
  await api.configure(createDefaultScene(catalog,product.id));
  await api.loadFirmware({url:'https://platform.example/factory.bin',sha256,bootMode:'rom'});
  await api.start();
  await guestTime(8);
  await checkpoint('menu','95c5ea57a02cb645');
  await hold(35,0.2);
  await api.waitFor({serialIncludes:'Voltage :0.00V',timeoutMs:30000});
  await checkpoint('voltage','4006054ab84cdaa8');
  await hold(0,0.2);
  await checkpoint('scan','e84d73cc740787bd');
  const before=Number((await api.observe()).stats.time);
  await hold(35,1.5);
  await guestTime(before+8);
  await checkpoint('deep-sleep','26993a53c3c439a5');
  const asleep=await api.observe();
  await guestTime(Number(asleep.stats.time)+1);
  const stillAsleep=await api.observe();
  assert.equal(stillAsleep.stats.insns,asleep.stats.insns,'both CPU instruction totals stay parked');
  await hold(35,0.2);
  await checkpoint('wake-menu','95c5ea57a02cb645');
  const state=await api.observe();
  assert.equal((state.serial.match(/Start\r?\n/g)||[]).length,2);
  assert.ok(!state.serial.includes('RTCWDT'));
  assert.ok(state.serial.includes('DEEPSLEEP_RESET'));
  receipt.passed=true;
  if(process.env.TDISPLAY_RECEIPT) await writeFile(process.env.TDISPLAY_RECEIPT,JSON.stringify(receipt,null,2)+'\n');
} finally {await api.dispose();}
