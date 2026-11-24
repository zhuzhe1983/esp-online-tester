import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createFirmwareCache,firmwareCache,MAX_CACHED_FIRMWARE_BYTES} from '../web/platform/firmware-cache.js';
import {firmwareRecoverySource,importFirmwareDownload} from '../web/platform/firmware-download-recovery.js';
import {createPlatformRuntime,sha256} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {firmwareArchiveCacheKey} from '../web/platform/firmware-archive.js';

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

test('manifest recovery is bound to the verified manifest and exact executable member',()=>{
  const manifest={manifestUrl:'./fixed.json',sha256:'a'.repeat(64)};
  const binding={manifestUrl:baseUrl+'fixed.json',manifestSha256:manifest.sha256,kind:'app',artifact:source};
  const error={...failure,details:{...failure.details,manifestRecovery:binding}};
  assert.deepEqual(firmwareRecoverySource(error,manifest,baseUrl),source);
  for(const patch of [{manifestSha256:'b'.repeat(64)},{manifestUrl:baseUrl+'other.json'},{kind:'rom'},
    {artifact:{...source,sha256:undefined}},{artifact:{...source,url:'https://other.example/release.bin'}}]) {
    assert.equal(firmwareRecoverySource({...error,details:{...error.details,manifestRecovery:{...binding,...patch}}},manifest,baseUrl),null);
  }
  assert.equal(firmwareRecoverySource(error,{manifestUrl:manifest.manifestUrl},baseUrl),null);
  for(const code of ['HASH_MISMATCH','HTTP_ERROR','FETCH_ABORTED'])assert.equal(firmwareRecoverySource({...error,code},manifest,baseUrl),null);
  assert.equal(firmwareRecoverySource({...error,details:{...error.details,url:binding.manifestUrl}},manifest,baseUrl),null);
});

test('real manifest loading recovers three separately pinned members and leaves a missing ROM unrecoverable',async()=>{
  const catalog={};
  for(const name of ['products','accessories','firmwares'])catalog[name]=JSON.parse(await readFile(new URL(`../web/platform/catalog/${name}.json`,import.meta.url),'utf8')).items;
  const entries=[],files={};
  for(const [i,kind] of ['bootloader','ptable','app'].entries()) {
    const bytes=new Uint8Array([0xe9,i,2,3]);
    const item={url:'https://publisher.example/three-parts.zip',sha256:await sha256(bytes),archive:{format:'zip',entry:`${kind}.bin`}};
    entries.push({kind,item,bytes});files[kind]=item;
  }
  const manifest=new TextEncoder().encode(JSON.stringify({board:'tdeck',flash_mb:16,psram_mb:8,files}));
  const source={manifestUrl:baseUrl+'recovery-three.json',sha256:await sha256(manifest)};
  const requests=[];let workers=0;
  const runtime=createPlatformRuntime({catalog,baseUrl,assetBaseUrl:'https://platform.example/',fetchImpl:async url=>{
    requests.push(String(url));if(String(url)===baseUrl+'recovery-three.json')return new Response(manifest);
    throw new TypeError('Failed to fetch');
  },workerFactory(){workers++;throw Error('A missing ROM must prevent worker creation');}});
  const scene=createDefaultScene(catalog,'lilygo-t-deck');scene.firmware=source;
  try {
    await runtime.configure(scene);
    for(const {kind,item,bytes} of entries) {
      let failure;try {await runtime.loadFirmware();}catch(error){failure=error;}
      assert.equal(failure?.code,'FETCH_FAILED');assert.equal(failure.details.manifestRecovery.kind,kind);
      const pinned=firmwareRecoverySource(failure,source,baseUrl);assert.deepEqual(pinned,item);
      await assert.rejects(()=>importFirmwareDownload(new File(['different member'],kind+'.bin'),pinned),{code:'HASH_MISMATCH'});
      await importFirmwareDownload(new File([bytes],kind+'.bin'),pinned);
    }
    await assert.rejects(()=>runtime.loadFirmware(),error=>{
      assert.equal(error.code,'FETCH_FAILED');assert.equal(error.details.manifestRecovery,undefined);
      assert.equal(firmwareRecoverySource(error,source,baseUrl),null);assert.match(error.details.url,/_rom\.elf$/);return true;
    });
    assert.equal(workers,0);assert.equal(requests.filter(url=>url.endsWith('three-parts.zip')).length,3);
    assert.deepEqual((await runtime.observe()).scene.firmware,source);
  } finally {
    await runtime.dispose();
    for(const {item} of entries)await firmwareCache.remove(firmwareArchiveCacheKey(item),item.sha256);
    await firmwareCache.remove(baseUrl+'recovery-three.json',source.sha256);
  }
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

for(const archive of [undefined,{format:'zip',entry:'merged-binary.bin'}])test(`runtime retry reads recovered ${archive?'ZIP entry':'raw BIN'} bytes and checks image layout before creating a worker`,async()=>{
  const catalog={};
  for(const name of ['products','accessories','firmwares'])catalog[name]=JSON.parse(await readFile(new URL(`../web/platform/catalog/${name}.json`,import.meta.url),'utf8')).items;
  let downloads=0,workers=0;
  const runtime=createPlatformRuntime({catalog,baseUrl,fetchImpl:async()=>{downloads++;throw new TypeError('Failed to fetch');},workerFactory(){workers++;throw Error('Must not execute invalid firmware');}});
  const recovery={...source,...(archive?{archive}:{})};
  assert.deepEqual(firmwareRecoverySource(failure,recovery,baseUrl),recovery);
  const scene=createDefaultScene(catalog,'lilygo-t-deck');scene.firmware={...recovery,bootMode:'rom'};
  try {
    await runtime.configure(scene);
    await assert.rejects(()=>runtime.loadFirmware(),{code:'FETCH_FAILED'});
    await importFirmwareDownload(file(),recovery);
    await assert.rejects(()=>runtime.loadFirmware(),{code:'INCOMPLETE_IMAGE'});
    assert.equal(downloads,1);assert.equal(workers,0);
    assert.deepEqual((await runtime.observe()).scene.firmware,scene.firmware);
  } finally {await runtime.dispose();await firmwareCache.remove(firmwareArchiveCacheKey(recovery),source.sha256);}
});
