import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {fetchBytes, createPlatformRuntime} from '../web/platform/runtime.js';
import {inspectLocalBuild, assembleLocalFirmware} from '../web/platform/local-firmware.js';
import {prepareFlashPlan} from '../web/platform/flasher.js';
import {createDefaultScene} from '../web/platform/catalog.js';

const MIB=1024*1024, limit=256*MIB, baseUrl='https://example.org/platform/';
const catalog={};
for(const name of ['products','accessories','firmwares'])catalog[name]=JSON.parse(await readFile(new URL(`../web/platform/catalog/${name}.json`,import.meta.url),'utf8')).items;

test('a real 256 MiB streamed body is accepted at the default boundary',async()=>{
  const chunk=new Uint8Array(32*MIB).fill(0x5a);let count=0;
  const bytes=await fetchBytes(baseUrl+'large.bin',{fetchImpl:async()=>new Response(new ReadableStream({pull(controller){if(count++<8)controller.enqueue(chunk);else controller.close();}}),{headers:{'content-length':String(limit)}})});
  assert.equal(bytes.length,limit);assert.equal(bytes[0],0x5a);assert.equal(bytes.at(-1),0x5a);
});

test('one byte above 256 MiB is rejected before reading and cancels the body',async()=>{
  let cancelled=false,reads=0;
  await assert.rejects(()=>fetchBytes(baseUrl+'oversize.bin',{fetchImpl:async()=>({ok:true,headers:new Headers({'content-length':String(limit+1)}),body:{cancel:async()=>{cancelled=true;},getReader:()=>{reads++;throw Error('must not read');}}})}),error=>error.code==='FILE_TOO_LARGE'&&error.message.includes('256 MiB'));
  assert.equal(cancelled,true);assert.equal(reads,0);
});

test('large JSON manifests pass the former 256 KiB gate in runtime and flash preparation',async()=>{
  const manifest=JSON.stringify({board:'stopwatch',flash_mb:16,files:{},padding:' '.repeat(512*1024)});
  const fetchImpl=async()=>new Response(manifest,{headers:{'content-length':String(manifest.length)}});
  const scene=createDefaultScene(catalog,'m5stack-stopwatch');scene.firmware={manifestUrl:baseUrl+'large.json'};
  const runtime=createPlatformRuntime({catalog,baseUrl,fetchImpl,workerFactory:()=>{throw Error('Must not execute firmware');}});
  try{
    await runtime.configure(scene);
    assert.equal((await runtime.capabilities()).firmware.maxFileBytes,limit);
    await assert.rejects(()=>runtime.loadFirmware(),{code:'INCOMPLETE_IMAGE'});
    await assert.rejects(()=>prepareFlashPlan({scene,catalog,baseUrl,fetchImpl}),{code:'FLASH_LAYOUT'});
  }finally{await runtime.dispose();}
});

test('local build metadata accepts 256 MiB files while product Flash capacity still applies',async()=>{
  const config=new File([JSON.stringify({flash_files:{'0x0':'image.bin'},padding:' '.repeat(512*1024)})],'flasher_args.json');
  let reads=0;
  const image={name:'image.bin',size:limit,arrayBuffer:async()=>{reads++;throw Error('oversized for device: must not read');}};
  assert.equal((await inspectLocalBuild([config,image])).segments.length,1);
  await assert.rejects(()=>inspectLocalBuild([config,{...image,size:limit+1}]),{code:'INVALID_FILE'});
  await assert.rejects(()=>assembleLocalFirmware({format:'build',files:[config,image]},{board:'stopwatch',flashBytes:16*MIB}),{code:'FLASH_OVERFLOW'});
  assert.equal(reads,0);
});
