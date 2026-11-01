import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createFirmwareCache,firmwareCache,MAX_CACHED_FIRMWARE_BYTES} from '../web/platform/firmware-cache.js';
import {firmwareRecoverySource,importFirmwareDownload} from '../web/platform/firmware-download-recovery.js';
import {createPlatformRuntime,sha256} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';

const baseUrl='https://platform.example/platform/';
const data=new Uint8Array([0xe9,1,2,3]),source={url:'https://publisher.example/release.bin',sha256:await sha256(data)};
const failure={code:'FETCH_FAILED',details:{url:source.url}};
const file=()=>new File([data],'release.bin');

test('only the failed cross-origin pinned BIN offers recovery',()=>{
  assert.deepEqual(firmwareRecoverySource(failure,source,baseUrl),source);
  assert.deepEqual(firmwareRecoverySource({...failure,code:'FIRMWARE_RELAY_FAILED'},source,baseUrl),source);
  for(const code of ['HTTP_ERROR','FETCH_ABORTED','HASH_MISMATCH','CHIP_MISMATCH'])assert.equal(firmwareRecoverySource({...failure,code},source,baseUrl),null);
  for(const other of [{url:source.url},{...source,sha256:'bad'},{...source,manifestUrl:source.url},{...source,file:file()},{...source,url:'javascript:alert(1)'},{...source,url:'https://user:password@publisher.example/release.bin'}])assert.equal(firmwareRecoverySource(failure,other,baseUrl),null);
  assert.equal(firmwareRecoverySource({code:'FETCH_FAILED',details:{url:baseUrl+'rom.elf'}},source,baseUrl),null);
  assert.equal(firmwareRecoverySource(failure,source,source.url),null);
  assert.equal(firmwareRecoverySource({code:'FETCH_FAILED'},source,baseUrl),null);
});

test('matching bytes retain the URL/hash identity and work without IndexedDB',async()=>{
  const cache=createFirmwareCache({indexedDB:null});
  assert.deepEqual(await importFirmwareDownload(file(),source,{cache}),{bytes:4,persistent:false});
  assert.deepEqual((await cache.get(source.url,source.sha256)).data,data);
  assert.equal(await cache.get('https://other.example/release.bin',source.sha256),null);
});

test('wrong bytes, empty files and oversized files do not enter the cache',async()=>{
  const cache=createFirmwareCache({indexedDB:null});
  await assert.rejects(()=>importFirmwareDownload(new File(['wrong version'],'release.bin'),source,{cache}),{code:'HASH_MISMATCH'});
  await assert.rejects(()=>importFirmwareDownload(new File([],'release.bin'),source,{cache}),{code:'INVALID_FILE'});
  let read=false;
  await assert.rejects(()=>importFirmwareDownload({size:MAX_CACHED_FIRMWARE_BYTES+1,arrayBuffer(){read=true;}},source,{cache}),{code:'INVALID_FILE'});
  assert.equal(read,false);assert.equal(await cache.get(source.url,source.sha256),null);
  await assert.rejects(()=>importFirmwareDownload({size:8,arrayBuffer:()=>data.buffer},source,{cache}),{code:'INVALID_FILE'});
  await assert.rejects(()=>importFirmwareDownload(file(),{url:source.url},{cache}),{code:'INVALID_SOURCE'});
});

test('cancellation while reading prevents cache writes; discarded cache is reported',async()=>{
  const cache=createFirmwareCache({indexedDB:null}),controller=new AbortController();
  await assert.rejects(()=>importFirmwareDownload({size:data.length,async arrayBuffer(){controller.abort();return data.buffer;}},source,{cache,signal:controller.signal}),{code:'FETCH_ABORTED'});
  assert.equal(await cache.get(source.url,source.sha256),null);
  await assert.rejects(()=>importFirmwareDownload(file(),source,{cache:{async putVerified(){return false;},async get(){return null;}}}),{code:'CACHE_UNAVAILABLE'});
});

test('runtime retry reads the recovered bytes and still rejects invalid image layout before creating a worker',async()=>{
  const catalog={};
  for(const name of ['products','accessories','firmwares'])catalog[name]=JSON.parse(await readFile(new URL(`../web/platform/catalog/${name}.json`,import.meta.url),'utf8')).items;
  let downloads=0,workers=0;
  const runtime=createPlatformRuntime({catalog,baseUrl,fetchImpl:async()=>{downloads++;throw new TypeError('Failed to fetch');},workerFactory(){workers++;throw Error('Must not execute invalid firmware');}});
  const scene=createDefaultScene(catalog,'lilygo-t-deck');scene.firmware={...source,bootMode:'rom'};
  try {
    await runtime.configure(scene);
    await assert.rejects(()=>runtime.loadFirmware(),{code:'FETCH_FAILED'});
    await importFirmwareDownload(file(),source);
    await assert.rejects(()=>runtime.loadFirmware(),{code:'INCOMPLETE_IMAGE'});
    assert.equal(downloads,1);assert.equal(workers,0);
    assert.deepEqual((await runtime.observe()).scene.firmware,scene.firmware);
  } finally {await runtime.dispose();await firmwareCache.remove(source.url,source.sha256);}
});
