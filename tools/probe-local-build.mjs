#!/usr/bin/env node
// Explicitly opt-in: node tools/probe-local-build.mjs --build <directory>
// No user firmware copies, remote requests, absolute paths or raw serial in receipt.
import {readFile} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {inspectLocalBuild} from '../web/platform/local-firmware.js';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
const index=process.argv.indexOf('--build');if(index<0||!process.argv[index+1])throw Error('Supply --build <directory> containing flasher_args.json.');
const folder=resolve(process.argv[index+1]), root=new URL('../',import.meta.url);
const selected=[new File([await readFile(join(folder,'flasher_args.json'))],'flasher_args.json')];
const inspected=await inspectLocalBuild(selected);
for(const path of inspected.missingFiles){const file=new File([await readFile(join(folder,path))],path.split('/').at(-1));Object.defineProperty(file,'webkitRelativePath',{value:path});selected.push(file);}
const layout=await inspectLocalBuild(selected);
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const fetchImpl=async url=>{const parsed=new URL(url);if(parsed.origin!=='https://local-probe.invalid')throw Error('Probe only permits local platform assets.');try{return new Response(await readFile(new URL('web'+parsed.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{url=new URL('web'+url.pathname,root);const worker=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(url))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(...a)=>worker.postMessage(...a),terminate:()=>worker.terminate()};worker.on('message',data=>adapter.onmessage?.({data}));worker.on('error',e=>adapter.onerror?.(e));return adapter;};
const frameHashes=new Set(), began=performance.now();let lastFrame=null;
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl:'https://local-probe.invalid/platform/',assetBaseUrl:'https://local-probe.invalid/',onEvent:e=>{if(e.type==='frame'){const hash=createHash('sha256').update(e.pixels).digest('hex');frameHashes.add(hash);lastFrame={width:e.width,height:e.height,frameNumber:e.frameNumber,sha256:hash};}}});
let outcome='timeout';
try{
  const scene=createDefaultScene(catalog,'m5stack-stopwatch');scene.firmware=null;await runtime.configure(scene);
  await runtime.loadFirmware({format:'build',files:selected});await runtime.start();
  const deadline=performance.now()+30000;let nextLog=0;
  while(performance.now()<deadline){const state=await runtime.observe();if(performance.now()>nextLog){console.error(JSON.stringify({phase:'running',guestSeconds:Number(state.stats.time)||0,frameNumber:state.frame?.frameNumber||0,serialBytes:state.serial.length}));nextLog=performance.now()+5000;}if(state.status==='error'){outcome='emulator-error';break;}if((state.frame?.frameNumber||0)>=3&&Number(state.stats.time)>=3){outcome='boot-and-frame';break;}await new Promise(r=>setTimeout(r,100));}
  if(outcome==='boot-and-frame')try{await runtime.pause();}catch{outcome='pause-timeout';}const state=await runtime.observe();
  console.log(JSON.stringify({schemaVersion:'1',checkedAt:new Date().toISOString(),command:'node tools/probe-local-build.mjs --build <user-build-directory>',outcome,layout:{chip:layout.chip,flashSizeBytes:layout.flashSizeBytes,segments:state.firmware?.localLayout.segments.map(({name,...rest})=>rest)},source:state.firmware?.source,appDirect:state.firmware?.appDirect,executionMode:state.firmware?.executionMode,status:state.status,guestSeconds:Number(state.stats.time),elapsedSeconds:(performance.now()-began)/1000,frame:lastFrame,distinctFrameHashes:frameHashes.size,serial:{bytes:state.serial.length,sha256:createHash('sha256').update(state.serial).digest('hex'),appMainLogDetected:/main_task|app_main|Starting app/.test(state.serial)},privacy:'User file paths, names and serial content excluded; binaries read only in memory.'},null,2));
  if(outcome!=='boot-and-frame')process.exitCode=1;
}finally{await runtime.dispose();}
