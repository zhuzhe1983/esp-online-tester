#!/usr/bin/env node
// The public product uses its existing ESP32 board through the same worker as the UI.
// Stock MicroPython plus the repository price-screen fixture is a test, not factory firmware.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {Worker} from 'node:worker_threads';
import {basename,join} from 'node:path';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {installAutomation} from '../web/platform/automation.js';

const root=new URL('../',import.meta.url),baseUrl='https://platform.example/platform/';
const catalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async name=>[name,JSON.parse(await readFile(new URL(`web/platform/catalog/${name}.json`,root),'utf8')).items])));
const product=catalog.products.find(item=>item.id==='lilygo-ttgo-t-display');
assert.ok(product,'Original TTGO T-Display is in the public catalog');
assert.equal(product.runtime.board,'ttgo-tdisplay');
assert.equal(product.runtime.status,'partial');
assert.equal(product.runtime.flashMb,4);
assert.equal(product.runtime.psramMb,0);
const buttons=Object.fromEntries(product.controls.filter(item=>Number.isInteger(item.pin??item.gpio)).map(item=>[item.pin??item.gpio,item.id]));
assert.ok(buttons[0]&&buttons[35],'Both physical GPIO buttons must be present');
const fileFetch=async value=>{
  const url=new URL(value),path=new URL(`web${url.pathname}`,root);
  try { return new Response(await readFile(path)); }
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
const serial=async (command,expected)=>{
  const ready=api.waitFor({serialIncludes:expected,port:'uart0',fromNow:true,timeoutMs:30000});
  await Promise.all([runtime.input({type:'serial',port:'uart0',data:command}),ready]);
};
try {
  await runtime.configure(createDefaultScene(catalog,product.id));
  await runtime.loadFirmware({manifestUrl:'https://platform.example/wasm/fw/tdisplay-prices.json'});
  await runtime.start();
  await api.waitFor({serialIncludes:'DRAWN demo',port:'uart0',timeoutMs:30000});
  const frame=await runtime.capture({format:'rgba'});
  assert.deepEqual([frame.width,frame.height],[135,240]);
  assert.ok(frame.pixels.some((value,index)=>index%4!==3&&value>0),'ST7789 shows pixels produced by the guest');
  console.log('ok original T-Display: ROM boot, stock MicroPython and real 135×240 ST7789 framebuffer');
  await serial('\x03','>>>');
  // The demo configures GPIO0 only; enable GPIO35's input buffer as real button firmware does.
  await serial("from machine import Pin;p0=Pin(0,Pin.IN);p35=Pin(35,Pin.IN);print('INPUTS_READY')\r",'INPUTS_READY\r\n');
  const readButtons=async (tag,left,right)=>serial(`print('${tag}',p0.value(),p35.value())\r`,`${tag} ${left} ${right}\r\n`);
  await readButtons('RELEASED',1,1);
  for(const [pin,left,right] of [[0,0,1],[35,1,0]]) {
    await runtime.input({type:'button',id:buttons[pin],pressed:true});
    try { await readButtons(`PRESSED${pin}`,left,right); }
    finally { await runtime.input({type:'button',id:buttons[pin],pressed:false}); }
    await readButtons(`RELEASED${pin}`,1,1);
  }
  await runtime.input({type:'button',id:buttons[0],pressed:true});
  await runtime.input({type:'button',id:buttons[35],pressed:true});
  await readButtons('BOTH',0,0);
  await runtime.pause();
  await runtime.input({type:'button',id:buttons[0],pressed:false});
  await runtime.input({type:'button',id:buttons[35],pressed:false});
  await runtime.start();
  await readButtons('PAUSED_RELEASE',1,1);
  console.log('ok original T-Display: guest GPIO0/35 readings confirm each press, release, combination and paused release');
} finally { await api.dispose(); }
