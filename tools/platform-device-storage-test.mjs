import assert from 'node:assert/strict';
import {createDeviceStorage,deviceStorageScope} from '../web/platform/device-storage.js';

const scope=deviceStorageScope({productId:'test-device',board:'none',chip:'esp32s3',flashBytes:8192,firmwareSha256:'a'.repeat(64)});
const a=createDeviceStorage({indexedDB:null,dbName:'storage-unit'}), b=createDeviceStorage({indexedDB:null,dbName:'storage-unit'});
const notifications=[];b.subscribe(change=>notifications.push(change));
try {
  const initial=await a.read(scope);assert.equal(initial.persistent,false);assert.equal(initial.row.pages.length,0);assert.match(initial.warning,/session/);
  const page=new Uint8Array(4096);page[71]=19;
  const saved=await a.save(scope,{epoch:0,revision:0,pages:[{offset:4096,data:page.buffer}]});
  assert.equal(saved.row.revision,1);assert.equal(saved.backend,'memory');assert.equal(notifications.length,1);
  page[71]=0;assert.equal(new Uint8Array((await b.read(scope)).row.pages[0].data)[71],19,'stored bytes are isolated from caller mutation');
  await assert.rejects(()=>b.save(scope,{epoch:0,revision:0,pages:[]}),{code:'STORAGE_CONFLICT'});
  const cleared=await b.clear(scope);assert.equal(cleared.row.epoch,1);assert.equal(cleared.row.revision,2);
  await assert.rejects(()=>a.save(scope,{epoch:0,revision:1,pages:[]}),{code:'STORAGE_CONFLICT'},'stale tab cannot resurrect data after clear');
  assert.equal((await a.read(scope)).row.pages.length,0);
  for (const field of ['productId','board','chip','firmwareSha256','flashBytes']) {
    const isolated=deviceStorageScope({...scope,[field]:field==='flashBytes'?16384:field==='firmwareSha256'?'b'.repeat(64):`other-${scope[field]}`});
    assert.equal((await a.read(isolated)).row.revision,0,`${field} isolates storage`);
  }
  for(const entry of [{offset:1,data:new ArrayBuffer(4096)},{offset:8192,data:new ArrayBuffer(4096)},{offset:0,data:new ArrayBuffer(4095)}]) {
    await assert.rejects(()=>a.save(scope,{epoch:1,revision:2,pages:[entry]}),{code:'STORAGE_CORRUPT'});
    assert.equal((await a.read(scope)).row.revision,2,'invalid page has no partial write');
  }
  const independent=createDeviceStorage({indexedDB:null,dbName:'another-db'});
  try {assert.equal((await independent.read(scope)).row.revision,0);} finally {independent.close();}
} finally {a.close();b.close();}
const blocked=createDeviceStorage({indexedDB:{open(){throw Error('denied');}},dbName:'blocked'});
try {
  assert.equal((await blocked.read(scope)).backend,'memory');
  assert.equal((await blocked.save(scope,{epoch:0,revision:0,pages:[]})).persistent,false);
  await assert.rejects(()=>blocked.clear(scope),{code:'STORAGE_CLEAR_FAILED'},'unavailable DB must not claim durable clear');
} finally {blocked.close();}
console.log('device storage: immutable pages, identity isolation, revision conflicts, clear epochs, malformed bounds and honest memory fallback passed');
