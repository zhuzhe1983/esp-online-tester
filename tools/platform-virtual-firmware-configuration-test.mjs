import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createSdCardStorage} from '../web/platform/sd-card-storage.js';
import {installAutomation} from '../web/platform/automation.js';
import {encodeUIFlow2} from '../web/platform/flash-configuration.js';
import {virtualConfigurationPlan,encodeVirtualConfiguration,replaceConfigurationPages} from '../web/platform/virtual-firmware-configuration.js';

const profile={type:'uiflow2',name:'Private config label',server:'private.example.invalid',ssid:'Private SSID',password:'Private Test Password'};
function image(){
  const bytes=new Uint8Array(16*1024*1024).fill(255),v=new DataView(bytes.buffer);
  for(const at of [0,0x10000]){bytes.fill(0,at,at+36);bytes[at]=0xe9;bytes[at+1]=1;bytes[at+12]=9;v.setUint32(at+28,4,true);}
  [[1,2,0x9000,0x6000,'nvs'],[0,0,0x10000,0x100000,'factory'],[1,0x82,0x110000,0xef0000,'vfs']].forEach(([type,subtype,offset,size,name],i)=>{
    const at=0x8000+32*i;bytes.fill(0,at,at+32);v.setUint16(at,0x50aa,true);bytes[at+2]=type;bytes[at+3]=subtype;v.setUint32(at+4,offset,true);v.setUint32(at+8,size,true);bytes.set(new TextEncoder().encode(name),at+12);
  });return bytes;
}
const baseline=image(),options={flashBytes:baseline.length};
test('current Flash validation rejects encrypted, malformed, overlapping, wrong-capacity, unsupported and app-direct layouts',()=>{
  assert.deepEqual(encodeVirtualConfiguration(virtualConfigurationPlan(baseline,options),profile),encodeUIFlow2(profile));
  for(const mutate of [
    (b,v)=>v.setUint32(0x801c,1,true),
    (b,v)=>v.setUint32(0x803c,1,true),
    (b,v)=>v.setUint32(0x8008,0x5000,true),
    (b,v)=>v.setUint32(0x8044,0xe000,true),
    (b,v)=>v.setUint32(0x8048,0xffffffff,true),
    (b,v)=>v.setUint32(0x8008,0,true),
    (b,v)=>v.setUint16(0x8060,0x1234,true),
    (b,v)=>v.setUint32(0x8044,0x110001,true),
    b=>b.set(b.slice(0x8000,0x8020),0x8060),
  ]) {const bytes=baseline.slice();mutate(bytes,new DataView(bytes.buffer));assert.throws(()=>virtualConfigurationPlan(bytes,options),{code:'FLASH_CONFIG_INVALID'});}
  for(const change of [{flashBytes:baseline.length-4096},{appDirect:true},{partitionTableOffset:0x8001},{partitionTableOffset:0x1000000}])assert.throws(()=>virtualConfigurationPlan(baseline,{...options,...change}),{code:'FLASH_CONFIG_INVALID'});
  assert.throws(()=>encodeVirtualConfiguration(virtualConfigurationPlan(baseline,options),null),{code:'FLASH_CONFIG_INVALID'});
  assert.throws(()=>encodeVirtualConfiguration(virtualConfigurationPlan(baseline,options),{...profile,type:'uiflow1'}),{code:'FLASH_CONFIG_INVALID'});
});
test('replacement changes exactly six NVS pages, preserving every unrelated byte and the source image',()=>{
  const other=new Uint8Array(4096).fill(19),old=new Map([[0x200000,other.buffer],[0x9000,new ArrayBuffer(4096)]]),original=baseline.slice();
  const encoded=encodeUIFlow2(profile),next=replaceConfigurationPages(baseline,old,encoded);
  assert.equal(next.get(0x200000),other.buffer);assert.equal(old.size,2);assert.deepEqual(baseline,original);
  const restored=baseline.slice();for(const [offset,data]of next)restored.set(new Uint8Array(data),offset);
  assert.deepEqual(restored.subarray(0x9000,0xf000),encoded);
  assert.deepEqual(restored.subarray(0,0x9000),baseline.subarray(0,0x9000));assert.deepEqual(restored.subarray(0xf000,0x200000),baseline.subarray(0xf000,0x200000));
  assert.equal(next.has(0xe000),false,'erased final NVS page is omitted when identical to the original image');
});

const catalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async type=>[type,JSON.parse(await readFile(new URL(`../web/platform/catalog/${type}.json`,import.meta.url))).items])));
const defer=()=>{let resolve;const promise=new Promise(r=>resolve=r);return {promise,resolve};};
// Lifecycle protocol fixture only; actual guest execution is a separate opt-in probe.
function harness({productId='m5stack-stopwatch',storageOptions={},fetchGate}={}) {
  const dbName=`virtual-config-${crypto.randomUUID()}`,store=createDeviceStorage({indexedDB:null,BroadcastChannel:null,dbName}),sd=createSdCardStorage({indexedDB:null,BroadcastChannel:null,dbName}),events=[],workers=[];
  const deviceStorage={...store,...storageOptions};let fetches=0;
  const workerFactory=()=>{
    const w={flash:new Uint8Array(baseline.length).fill(255),sd:null,terminated:false,watch:null,shadow:null,postMessage(m){let response;
      if(m.op==='init')response={ready:true};if(m.op==='create')response={created:true,flashStorage:true};
      if(m.op==='load'){if(m.kind===5)w.flash.set(new Uint8Array(m.data));response={loaded:m.at===undefined?m.kind:`at${m.at}`,ok:true};}
      if(m.op==='imu')response={imuApplied:m.inputId,ok:true,time:0};
      if(m.op==='flash-snapshot')response={flashReply:m.requestId,data:w.flash.slice().buffer};
      if(m.op==='flash-restore'){for(const p of m.pages)w.flash.set(new Uint8Array(p.data),p.offset);response={flashReply:m.requestId,ok:true};}
      if(m.op==='flash-watch'){w.watch=m.requestId;w.shadow=w.flash.slice();response={flashReply:m.requestId,ok:true};}
      if(m.op==='flash-flush'){
        const pages=[];for(let at=0;at<w.flash.length;at+=4096)if(!w.flash.subarray(at,at+4096).every((v,i)=>v===w.shadow[at+i]))pages.push({offset:at,data:w.flash.slice(at,at+4096).buffer});
        w.shadow=w.flash.slice();if(pages.length)queueMicrotask(()=>w.onmessage?.({data:{flashDelta:{watchId:w.watch,pages}}}));response={flashReply:m.requestId,ok:true};
      }
      if(m.op.startsWith('sd-'))response={sdReply:m.requestId,ok:true};if(m.op==='sd-attach')w.sd=new Uint8Array(m.data).slice();if(m.op==='sd-export')response.data=w.sd.slice().buffer;
      if(m.op==='start')response={started:true};if(m.op==='resume')response={resumed:true};if(m.op==='stop')response={stopped:0};
      if(response)queueMicrotask(()=>w.onmessage?.({data:response}));
    },terminate(){w.terminated=true;}};workers.push(w);return w;
  };
  const runtime=createPlatformRuntime({catalog,workerFactory,deviceStorage,sdStorage:sd,onEvent:event=>events.push(structuredClone(event)),baseUrl:'https://example.invalid/platform/',assetBaseUrl:'https://example.invalid/',fetchImpl:async url=>{if(String(url).endsWith('.wasm')){fetches++;await fetchGate?.(fetches);}return new Response(String(url).endsWith('firmware.bin')?baseline:new Uint8Array([1,2,3]));}});
  const scene=createDefaultScene(catalog,productId),source={url:'https://example.invalid/firmware.bin'};
  return {runtime,store,sd,events,workers,dbName,scene,source,load:async()=>{await runtime.configure(scene);await runtime.loadFirmware(source);}};
}
test('explicit virtual write preserves source hash, filesystem/SD, reset/reload persistence, private receipts and stale-session fences',async()=>{
  const h=harness({productId:'m5stack-c139'}),{runtime,store,workers}=h;
  try {
    assert.equal((await runtime.inspectFirmwareConfiguration()).available,false);
    await h.load();const original=await runtime.observe(),scope=original.storage.scope;
    const media=new Uint8Array(1024*1024);media[991]=87;await runtime.attachSdCard({bytes:media});
    workers.at(-1).flash[0x200123]=61;await runtime.flushDeviceStorage();
    const inspected=await runtime.inspectFirmwareConfiguration();assert.equal(inspected.available,true);assert.ok(inspected.sessionId);
    const after=await runtime.writeFirmwareConfiguration(profile,{sessionId:inspected.sessionId});
    assert.equal(after.status,'ready');assert.equal(after.storage.scope.key,scope.key);assert.deepEqual(after.firmware,original.firmware);assert.deepEqual(after.scene,original.scene);
    assert.equal(workers.at(-1).flash[0x200123],61);assert.deepEqual(workers.at(-1).flash.subarray(0x9000,0xf000),encodeUIFlow2(profile));assert.equal((await runtime.exportSdCard()).bytes[991],87);
    assert.deepEqual(Object.keys(after.firmwareConfiguration).sort(),['namespace','partition','type','writtenAt']);
    const api=installAutomation(runtime,{target:{location:{href:'https://example.invalid/platform/'}}}),shared=await api.share({autostart:false});
    for(const secret of [profile.name,profile.server,profile.ssid,profile.password]){assert.ok(!JSON.stringify(h.events).includes(secret));assert.ok(!JSON.stringify(shared).includes(secret));assert.ok(!JSON.stringify(after).includes(secret));}
    await assert.rejects(()=>runtime.writeFirmwareConfiguration(profile,{sessionId:inspected.sessionId}),{code:'SESSION_REPLACED'});
    await runtime.reset();assert.equal(workers.at(-1).flash[0x200123],61);assert.deepEqual(workers.at(-1).flash.subarray(0x9000,0xf000),encodeUIFlow2(profile));
    await runtime.loadFirmware(h.source);assert.equal((await runtime.observe()).storage.scope.key,scope.key);assert.deepEqual(workers.at(-1).flash.subarray(0x9000,0xf000),encodeUIFlow2(profile));
    await runtime.writeFirmwareConfiguration({...profile,ssid:'Second private SSID'});assert.deepEqual(workers.at(-1).flash.subarray(0x9000,0xf000),encodeUIFlow2({...profile,ssid:'Second private SSID'}));
    assert.equal((await store.read(scope)).row.pages.some(page=>page.offset===0x200000),true);
    await runtime.clearDeviceStorage();assert.equal((await runtime.observe()).firmwareConfiguration,null);
  } finally {await runtime.dispose();assert.ok(workers.every(w=>w.terminated));}
});
test('write validates changed current partition table and never redirects queued work to the next session',async()=>{
  const h=harness();try {
    await h.load();const original=await h.runtime.observe();h.workers.at(-1).flash[0x801c]=1;
    assert.equal((await h.runtime.inspectFirmwareConfiguration()).available,false);
    await assert.rejects(()=>h.runtime.writeFirmwareConfiguration(profile),{code:'FLASH_CONFIG_INVALID'});
    assert.ok(!(await h.store.read(original.storage.scope)).row.pages.some(p=>p.offset===0x9000));
    const change=h.runtime.configure(h.scene),write=h.runtime.writeFirmwareConfiguration(profile);
    await change;await assert.rejects(()=>write,{code:'SESSION_REPLACED'});
  } finally {await h.runtime.dispose();}
});
test('CAS conflict rejects the configuration without partial NVS replacement',async()=>{
  const h=harness({storageOptions:{async save(){throw Object.assign(new Error('concurrent save'),{code:'STORAGE_CONFLICT'});}}});
  try {await h.load();const scope=(await h.runtime.observe()).storage.scope;await assert.rejects(()=>h.runtime.writeFirmwareConfiguration(profile),{code:'STORAGE_CONFLICT'});assert.equal((await h.store.read(scope)).row.pages.length,0);assert.equal((await h.runtime.observe()).status,'error');}
  finally {await h.runtime.dispose();}
});
test('automation freezes the chosen profile and fences changes during recording lifecycle shutdown',async()=>{
  const h=harness();try {
    await h.load();const api=installAutomation(h.runtime,{target:{}}),selected={...profile};
    const writing=api.writeFirmwareConfiguration(selected);selected.ssid='Changed after invocation';
    await writing;assert.deepEqual(h.workers.at(-1).flash.subarray(0x9000,0xf000),encodeUIFlow2(profile));
    const change=api.configure(h.scene),stale=api.writeFirmwareConfiguration(profile);stale.catch(()=>{});
    await change;await assert.rejects(()=>stale,e=>['SESSION_REPLACED','STORAGE_NOT_LOADED'].includes(e.code));
  } finally {await h.runtime.dispose();}
});
test('cancel after the atomic commit aborts reinitialization and preserves configuration only in the original scope',async()=>{
  const entered=defer(),release=defer(),h=harness({fetchGate:async count=>{if(count===2){entered.resolve();await release.promise;}}});
  try {
    await h.load();const scope=(await h.runtime.observe()).storage.scope;
    const writing=h.runtime.writeFirmwareConfiguration(profile);writing.catch(()=>{});await entered.promise;
    const cancelling=h.runtime.cancelPendingLoad();release.resolve();
    await assert.rejects(()=>writing,e=>e.code==='FETCH_ABORTED'&&e.details.configurationCommitted===true);await cancelling;
    assert.equal((await h.runtime.observe()).status,'configured');const row=(await h.store.read(scope)).row;
    assert.ok(row.pages.some(p=>p.offset===0x9000));await h.runtime.loadFirmware(h.source);assert.deepEqual(h.workers.at(-1).flash.subarray(0x9000,0xf000),encodeUIFlow2(profile));
  } finally {release.resolve();await h.runtime.dispose();}
});
test('replacement requested during post-commit reload cannot return a stale ready result to the caller',async()=>{
  const entered=defer(),release=defer(),h=harness({fetchGate:async count=>{if(count===2){entered.resolve();await release.promise;}}});
  try {
    await h.load();const scope=(await h.runtime.observe()).storage.scope,writing=h.runtime.writeFirmwareConfiguration(profile);writing.catch(()=>{});await entered.promise;
    const replacement=h.runtime.configure(h.scene);release.resolve();
    await assert.rejects(()=>writing,e=>e.code==='SESSION_REPLACED'&&e.details.configurationCommitted===true);await replacement;
    assert.equal((await h.runtime.observe()).status,'configured');assert.ok((await h.store.read(scope)).row.pages.some(p=>p.offset===0x9000));
  } finally {release.resolve();await h.runtime.dispose();}
});
