#!/usr/bin/env node
// Caller-selected official image: bytes stay in RAM and execute in the actual page worker.
import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
import {basename} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {installAutomation} from '../web/platform/automation.js';
const args=process.argv.slice(2),arg=name=>args[args.indexOf(name)+1];
for(const key of ['--sha256','--mode'])assert(args.includes(key)&&arg(key),`Required ${key}`);
assert(args.includes('--url')!==args.includes('--file'),'Supply exactly one caller-selected --url or --file.');
const mode=arg('--mode'),url=args.includes('--url')?arg('--url'):null,expected=arg('--sha256'),root=new URL('../',import.meta.url),baseUrl='https://dial-v11-official.invalid/platform/';
assert(['factory','uiflow'].includes(mode));if(url)assert(/^https:\/\//.test(url));assert(/^[a-f0-9]{64}$/.test(expected));
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const response=url?await fetch(url,{signal:AbortSignal.timeout(45000)}):null;if(response)assert(response.ok,`Firmware HTTP ${response.status}`);
const firmware=new Uint8Array(response?await response.arrayBuffer():await readFile(arg('--file')));assert.equal(hash(firmware),expected,'Changed firmware must be separately reviewed.');
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const fetchImpl=async value=>{const target=new URL(value);if(String(target)===url)return new Response(firmware);assert.equal(target.origin,'https://dial-v11-official.invalid');try{return new Response(await readFile(new URL(target.pathname.endsWith('.elf')?'web/wasm/fw/'+basename(target.pathname):'web'+target.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root),thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl,assetBaseUrl:'https://dial-v11-official.invalid/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'dial-v11-official-probe'})});
const receipt={checkedAt:new Date().toISOString(),command:'node tools/probe-dial-official.mjs --mode '+mode+' --url/--file <caller-selected-image> --sha256 <explicit-sha256>',mode,source:{url,resolvedUrl:response?.url||null,localInput:!url,bytes:firmware.length,sha256:hash(firmware)},backend:'actual-page-worker-wasm-jit',firmwarePatched:false,wasmSha256:hash(await readFile(new URL('web/wasm/esp32sim.wasm',root))),checks:[]};
const wait=async predicate=>{const end=Date.now()+90000;while(Date.now()<end){const state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(JSON.stringify(state.lastError));await new Promise(r=>setTimeout(r,50));}throw Error(`Official probe timed out: ${(await runtime.observe()).serial.slice(-2000)}`);};
const advance=async seconds=>{const start=(await runtime.observe()).stats.time||0;return wait(s=>(s.stats.time||0)>=start+seconds);};
try {
 const scene=createDefaultScene(catalog,'m5stack-k130-v11');scene.firmware=url?{url,sha256:expected}:null;await runtime.configure(scene);await runtime.loadFirmware(url?undefined:{file:new File([firmware],'caller-selected-merged.bin')});await runtime.start();await advance(mode==='uiflow'?6:3);
 const capture=async name=>{await runtime.pause();const f=await runtime.capture({format:'rgba'});assert.equal(f.width,240);assert.equal(f.height,240);if(args.includes('--frames'))await writeFile(arg('--frames')+'-'+name+'.rgba',f.pixels);receipt[name+'Sha256']=hash(f.pixels);await runtime.start();return f.pixels;};
 const start=await capture('boot');assert(new Set(start).size>10);receipt.checks.push('rom-boot-real-nonblank-240x240-frame');
 const api=installAutomation(runtime,{target:{location:{href:baseUrl}}});
 if(mode==='uiflow') {
  // UIFlow 2.5.3 samples the physical quadrature into its three-page selector.
  // This fixed image routes -1 to WIFI & SERVER SETTING; touch selects PASS.
  await runtime.input({type:'encoder',id:'encoder',steps:-1});await advance(.8);const settings=await capture('turned');assert.notDeepEqual(settings,start);receipt.checks.push('physical-encoder-changes-official-app-page');
  await runtime.input({type:'touch',x:180,y:74,down:true});await advance(.2);await runtime.input({type:'touch',x:180,y:74,down:false});await advance(.5);const changed=await capture('touched');assert.notDeepEqual(changed,settings);assert.deepEqual([...settings.subarray((130*240+100)*4,(130*240+100)*4+4)],[255,255,255,255]);assert.deepEqual([...changed.subarray((130*240+100)*4,(130*240+100)*4+4)],[222,223,222,255]);receipt.checks.push('ft3267-touch-selects-official-password-field');
 } else {
  const before=(await runtime.observe()).serial.length;await api.press('button',{durationMs:150});await advance(.5);const lcd=await capture('pressed');assert.notDeepEqual(lcd,start);receipt.checks.push('gpio42-enters-official-lcd-test');
  await runtime.input({type:'encoder',id:'encoder',steps:1});await advance(.5);const page=await capture('turned');assert.notDeepEqual(page,lcd);receipt.checks.push('pcnt-encoder-changes-official-lcd-test-page');
  await runtime.input({type:'touch',x:100,y:120,down:true});await advance(.2);await runtime.input({type:'touch',x:100,y:120,down:false});await advance(.2);const touch=await capture('touched');assert.notDeepEqual(touch,page);assert.deepEqual([...touch.subarray((120*240+100)*4,(120*240+100)*4+4)],[0,0,255,255]);receipt.checks.push('ft3267-touch-renders-blue-point-at-exact-guest-coordinate');
  await api.press('button',{durationMs:150});await advance(.6);const exited=await capture('exited');assert.notDeepEqual(exited,touch);receipt.checks.push('gpio42-exits-official-lcd-test');receipt.serialMarkers=(await runtime.observe()).serial.slice(before).split(/\r?\n/).filter(l=>/LCD_Test|launcher:/.test(l)).map(l=>l.replace(/\x1b\[[0-9;]*m/g,''));
 }
 await runtime.pause();const state=await runtime.observe();receipt.status='pass';receipt.guestSeconds=state.stats.time;receipt.artifacts=state.firmware.artifacts.map(({kind,sha256,bytes})=>({kind,sha256,bytes}));receipt.limitations=['No real hardware test; no wireless connection, RTC/RFID, PWM brightness, audio or power/sleep validation.','Pixel changes are interpreted alongside retained screenshots and fixed upstream source; no OCR or complete firmware compatibility claim.'];console.log(JSON.stringify(receipt,null,2));
} finally {await runtime.dispose();}
