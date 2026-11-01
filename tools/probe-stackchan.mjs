#!/usr/bin/env node
// Exercise source-built K151 firmware on the production worker/WASM path; no simulated host replies.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {resolve,join,basename} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {inspectLocalBuild} from '../web/platform/local-firmware.js';
import {installAutomation} from '../web/platform/automation.js';
import {createDefaultScene} from '../web/platform/catalog.js';

const index=process.argv.indexOf('--build');
if(index<0||!process.argv[index+1])throw Error('Build examples/platform-stackchan with ESP-IDF 5.4.3; supply --build <build-directory>.');
const folder=resolve(process.argv[index+1]),root=new URL('../',import.meta.url),origin='https://stackchan-fixture.invalid',baseUrl=origin+'/platform/';
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const files=[new File([await readFile(join(folder,'flasher_args.json'))],'flasher_args.json')],inputHashes={};
for(const path of (await inspectLocalBuild(files)).missingFiles){const bytes=await readFile(join(folder,path)),file=new File([bytes],basename(path));inputHashes[path]=hash(bytes);Object.defineProperty(file,'webkitRelativePath',{value:path});files.push(file);}
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const manifest=JSON.stringify({board:'stackchan',flash_mb:16,psram_mb:8,files:{rom:'esp32s3_rev0_rom.elf',bootloader:'bootloader/bootloader.bin',ptable:'partition_table/partition-table.bin',app:'platform_stackchan.bin'},artifact_sha256:inputHashes});
const fetchImpl=async value=>{const url=new URL(value);assert.equal(url.origin,origin);try{if(url.pathname==='/fixture/manifest.json')return new Response(manifest);if(url.pathname.startsWith('/fixture/')&&!url.pathname.endsWith('.elf'))return new Response(await readFile(join(folder,url.pathname.slice('/fixture/'.length))));if(url.pathname.endsWith('.elf'))return new Response(await readFile(new URL('web/wasm/fw/'+basename(url.pathname),root)));return new Response(await readFile(new URL('web'+url.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root);const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(message,...args)=>thread.postMessage(message.op==='create'&&process.argv.includes('--no-jit')?{...message,jit:false}:message,...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl,assetBaseUrl:origin+'/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'stackchan-probe'})});
const receipt={schemaVersion:'1',checkedAt:new Date().toISOString(),command:'node tools/probe-stackchan.mjs --build <fixture-build-directory>',backend:'actual-wasm-worker',firmware:'internal ESP-IDF 5.4.3 protocol fixture',inputHashes,wasmSha256:hash(await readFile(new URL('web/wasm/esp32sim.wasm',root))),checks:[]};
const wait=async predicate=>{const end=Date.now()+120000;while(Date.now()<end){const state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(JSON.stringify(state.lastError));await new Promise(r=>setTimeout(r,50));}const state=await runtime.observe();throw Error(`Fixture timed out: ${state.serial.slice(-3000)}; servos=${JSON.stringify(state.servos)}`);};
const padsText='TOUCH_PADS front=0 middle=0 back=0 raw=0';
const axis=(state,id)=>state.servos?.find(value=>value.id===id);
const ready=state=>state.serial.includes('PLATFORM_READY stackchan')&&state.serial.includes(padsText)&&state.leds?.length===12&&state.frame?.width===320;
const closeTo=(value,expected)=>Math.abs(value-expected)<0.001;
const everyLed=(state,color)=>state.leds?.length===12&&state.leds.every(value=>JSON.stringify(value)===JSON.stringify(color));
const neutral=state=>closeTo(axis(state,'pan')?.positionDegrees,0)&&closeTo(axis(state,'tilt')?.positionDegrees,0);
async function action(event,text,predicate=()=>true){const at=(await runtime.observe()).serial.length;await runtime.input(event);return wait(state=>state.serial.slice(at).includes(text)&&predicate(state));}
try {
  const scene=createDefaultScene(catalog,'m5stack-k151');scene.firmware=null;await runtime.configure(scene);await runtime.loadFirmware({format:'build',files});await runtime.start();
  let state=await wait(s=>ready(s)&&neutral(s));
  for(const name of ['expander-version','yaw-goal-feedback','pitch-goal-feedback','wrong-tx-no-response','motor-power-off-no-response','motor-power-recovery-response'])assert.ok(state.serial.includes(`CHECK ${name} PASS`),name);
  assert.ok(!state.serial.includes('CHECK ')||!state.serial.match(/CHECK [^\r\n]* FAIL/));
  receipt.checks.push('real-rom-boot-expander-identity-servo-feedback-wrong-route-and-motor-power-gates');
  assert.equal(state.frame.height,240);const initialFrame=await runtime.capture({format:'rgba'});receipt.initialFrameSha256=hash(initialFrame.pixels);
  const padCases=[['front','TOUCH_PADS front=3 middle=0 back=0 raw=48',[255,0,0],'pan',40],['middle','TOUCH_PADS front=0 middle=3 back=0 raw=12',[0,255,0],'tilt',60],['back','TOUCH_PADS front=0 middle=0 back=3 raw=3',[0,0,255],'pan',-40]];
  for(const [id,text,color,joint,degrees] of padCases){
    state=await action({type:'button',id:`touch-pad-${id}`,pressed:true},text,s=>everyLed(s,color)&&closeTo(axis(s,joint)?.positionDegrees,degrees));
    const frame=await runtime.capture({format:'rgba'});assert.notEqual(hash(frame.pixels),receipt.initialFrameSha256);
    await action({type:'button',id:`touch-pad-${id}`,pressed:false},padsText,neutral);
  }
  receipt.checks.push('three-separate-si12t-pads-consumed-by-guest-drive-rgb-servo-feedback-and-framebuffer');
  await action({type:'button',id:'touch-pad-front',pressed:true},'TOUCH_PADS front=3 middle=0 back=0 raw=48');
  await action({type:'button',id:'touch-pad-back',pressed:true},'TOUCH_PADS front=3 middle=0 back=3 raw=51');
  await action({type:'button',id:'touch-pad-front',pressed:false},'TOUCH_PADS front=0 middle=0 back=3 raw=3');
  await action({type:'button',id:'touch-pad-back',pressed:false},padsText,neutral);
  receipt.checks.push('independent-simultaneous-pad-press-and-release');
  await action({type:'touch',x:100,y:120,down:true},'TOUCH_SCREEN count=1 x=100 y=120');
  await action({type:'touch',x:100,y:120,down:false},'TOUCH_SCREEN count=0');
  receipt.checks.push('ft6336-screen-contact-is-separate-from-head-pads');
  await assert.rejects(runtime.input({type:'button',id:'touch-pad-unknown',pressed:true}));
  await runtime.input({type:'button',id:'touch-pad-front',pressed:true});
  await runtime.reset();await runtime.start();state=await wait(s=>ready(s)&&neutral(s));
  assert.equal(state.servoSupply,true);receipt.checks.push('platform-reset-releases-pad-and-requires-fresh-guest-output');
  const api=installAutomation(runtime,{target:{location:{href:baseUrl}}});
  const remote=createDefaultScene(catalog,'m5stack-k151');remote.firmware={manifestUrl:origin+'/fixture/manifest.json',sha256:hash(manifest)};
  await runtime.configure(remote);await runtime.loadFirmware();await api.recordingStart();await wait(s=>ready(s)&&neutral(s));
  await action({type:'button',id:'touch-pad-front',pressed:true},'TOUCH_PADS front=3 middle=0 back=0 raw=48',s=>everyLed(s,[255,0,0]));
  await action({type:'button',id:'touch-pad-front',pressed:false},padsText,neutral);
  const recorded=await api.recordingStop();const replay=await api.recordingPlay(recorded,{timeoutMs:120000});assert.equal(replay.result,'completed');
  state=await wait(s=>s.serial.includes('TOUCH_PADS front=3 middle=0 back=0 raw=48')&&neutral(s));
  receipt.checks.push('record-and-replay-pad-events-through-real-worker');
  receipt.replay={events:recorded.events,result:replay.result,bitExact:false};
  await runtime.pause();receipt.status='pass';receipt.guestSeconds=state.stats.time;receipt.servos=state.servos;receipt.leds=state.leds;
  receipt.serialEvidence=state.serial.split(/\r?\n/).filter(line=>/^(CHECK |SERVO_|TOUCH_|PLATFORM_READY)/.test(line));
  console.log(JSON.stringify(receipt,null,2));
} finally {await runtime.dispose();}
