import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {readFile} from 'node:fs/promises';
import {test} from 'node:test';
import {selectBleProfile,verifyBleProfile,verifyBleStoredPages,loadBleProfile} from '../web/platform/ble-profile.js';

const digest = async bytes => createHash('sha256').update(bytes).digest('hex');
const image = Uint8Array.from({length:128},(_,i)=>i);
const symbols = Uint8Array.of(0x7f,0x45,0x4c,0x46);
const selected = {id:'fixture',board:'stackchan',sha256:await digest(image),bytes:image.length,image};
const profile = {id:selected.id,board:selected.board,format:'merged',firmwareSha256:selected.sha256,firmwareBytes:selected.bytes,symbols:{file:'symbols.elf',sha256:await digest(symbols),bytes:symbols.length},windows:[{offset:16,bytes:32,sha256:await digest(image.subarray(16,48)),symbol:'fixture-entry'}],mutableFlashRanges:[{offset:96,bytes:32,name:'nvs'}]};

test('only exact registered firmware identities may select a virtual BLE profile',async()=>{
  const artifact={kind:'flash',sha256:selected.sha256,bytes:image.length};
  assert.equal(selectBleProfile('stackchan',[{kind:'flash',data:image}],[artifact]),null);
  assert.equal(selectBleProfile('cores3',[{kind:'flash',data:image}],[artifact]),null);
  const known={kind:'flash',sha256:'411578a2ebca2cfe3541fdc32aeddbc4703cf912a5daa7e1253f69306d6e1d87',bytes:12783789},file={kind:'flash',data:image};
  assert.equal(selectBleProfile('stackchan',[file],[known]).id,'stackchan-factory-1.5.1');
  assert.equal(selectBleProfile('cores3',[file],[known]),null);
  assert.equal(selectBleProfile('stackchan',[file,{kind:'app',data:image}],[known,{...artifact,kind:'app'}]),null,'Another executable load must not override the audited image');
  assert.equal(selectBleProfile('stackchan',[{...file,at:0x20000}],[{...known,at:0x20000}]),null,'A matching file at another offset is not the same executable mapping');
  let reads=0;assert.equal(await loadBleProfile(null,{read:()=>{reads++;},digest}),null);assert.equal(reads,0);
});

test('firmware, symbols and hook windows must independently match before enabling HCI',async()=>{
  await verifyBleProfile(profile,selected,symbols,digest);
  for(const mutation of [
    value=>{value.board='cores3';},
    value=>{value.firmwareSha256='0'.repeat(64);},
    value=>{value.symbols.sha256='0'.repeat(64);},
    value=>{value.symbols.file='../symbols.elf';},
    value=>{value.windows[0].sha256='0'.repeat(64);},
    value=>{value.windows[0].offset=120;},
    value=>{value.windows=[];},
    value=>{value.mutableFlashRanges[0]={offset:0,bytes:128};},
  ]) {
    const changed=structuredClone(profile);mutation(changed);
    await assert.rejects(verifyBleProfile(changed,selected,symbols,digest),{code:'BLE_PROFILE_MISMATCH'});
  }
  const changed=image.slice();changed[70]^=1;
  await assert.rejects(verifyBleProfile(profile,{...selected,image:changed},symbols,digest),/Firmware bytes changed/);
});

test('saved NVS may change while executable or boot-selection changes reject the profile without modifying storage',()=>{
  const data=image.slice();data[99]^=1;const pages=new Map([[0,data.buffer]]),original=data.slice();
  verifyBleStoredPages(profile,image,pages);assert.deepEqual(data,original);
  data[30]^=1;const modified=data.slice();assert.throws(()=>verifyBleStoredPages(profile,image,pages),/Device storage has been preserved/);assert.deepEqual(data,modified);
});

test('profile assets cannot escape their local asset directory',async()=>{
  for(const file of ['../symbols.elf','https://example.test/symbols.elf','/symbols.elf','symbols.elf?version=1']) {
    const bad={...profile,symbols:{...profile.symbols,file}},urls=[],bytes=new TextEncoder().encode(JSON.stringify(bad));
    await assert.rejects(loadBleProfile({...selected,url:'./assets/firmware-profiles/fixture/profile.json',profileSha256:await digest(bytes)},{digest,read:async url=>{urls.push(url);return bytes;}}),/local file/);
    assert.equal(urls.length,1,'A rejected path must never be fetched');
  }
  await assert.rejects(loadBleProfile({...selected,url:'./assets/firmware-profiles/fixture/profile.json',profileSha256:'0'.repeat(64)},{digest,read:async()=>new TextEncoder().encode(JSON.stringify(profile))}),/pinned checksum/);
});

test('the deployed profile and symbol-only ELF match the registry checksums',async()=>{
  const known={kind:'flash',sha256:'411578a2ebca2cfe3541fdc32aeddbc4703cf912a5daa7e1253f69306d6e1d87',bytes:12783789};
  const registered=selectBleProfile('stackchan',[{kind:'flash',data:image}],[known]);
  const url=new URL(registered.url,new URL('../web/platform/ble-profile.js',import.meta.url));
  const bytes=await readFile(url);assert.equal(await digest(bytes),registered.profileSha256);
  const metadata=JSON.parse(bytes),elf=await readFile(new URL(metadata.symbols.file,url));
  assert.equal(metadata.firmwareSha256,known.sha256);assert.equal(elf.length,metadata.symbols.bytes);assert.equal(await digest(elf),metadata.symbols.sha256);
  assert.equal(elf.readUInt32LE(24),0,'Metadata ELF has no executable entry');
  assert.equal(elf.readUInt16LE(44),0,'Metadata ELF has no loadable program headers');
});
