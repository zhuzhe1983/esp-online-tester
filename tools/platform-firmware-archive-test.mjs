import test from 'node:test';import assert from 'node:assert/strict';
import {firmwareZip,firmwareZipEntries} from './fixtures/firmware-zip.mjs';
import {createHash} from 'node:crypto';
import {validFirmwareArchive,extractFirmwareZip,validFirmwareZipEntry,extractFirmwareZipEntry,MAX_FIRMWARE_ZIP_BYTES,MAX_FIRMWARE_ZIP_ENTRIES,MAX_FIRMWARE_ZIP_DECLARED_BYTES} from '../web/platform/firmware-archive.js';
import {createDefaultScene,validateScene,encodeScene,decodeScene,firmwareSourceMatches} from '../web/platform/catalog.js';
import {createFirmwareHistory} from '../web/platform/firmware-history.js';
import {readFile} from 'node:fs/promises';
const source={url:'https://example.org/firmware.zip',sha256:'a'.repeat(64),archive:{format:'zip',entry:'firmware.bin'}},bytes=Buffer.from('a real compressed payload\n'.repeat(1000));
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const entrySource=(zip,entry='app.bin')=>({...source,sha256:hash(bytes),archive:{format:'zip-entry',entry,sha256:hash(zip)}});
test('manifest ZIP segments select exact pinned files without guessing a flash layout',async()=>{
 for(const method of [0,8])for(const descriptor of [false,true]){
  const zip=firmwareZipEntries([{data:'read the offsets',name:'README_flash.txt',method,descriptor},{data:bytes,name:'app.bin',method,descriptor},{data:'boot bytes',name:'bootloader.bin',method,descriptor}]);
  const src=entrySource(zip);assert.equal(validFirmwareZipEntry(src),true);assert.equal(validFirmwareArchive(src),false,'multi-file packages are not merged image sources');
  assert.deepEqual(Buffer.from(await extractFirmwareZipEntry(zip,src)),bytes);
  assert.equal(Buffer.from(await extractFirmwareZipEntry(zip,entrySource(zip,'bootloader.bin'))).toString(),'boot bytes');
  await assert.rejects(extractFirmwareZipEntry(zip,entrySource(zip,'missing.bin')),{code:'INVALID_FIRMWARE_ARCHIVE'});
  await assert.rejects(extractFirmwareZipEntry(zip,{...src,archive:{...src.archive,sha256:'0'.repeat(64)}}),{code:'HASH_MISMATCH'});
  await assert.rejects(extractFirmwareZipEntry(zip,src,{maxBytes:bytes.length-1}),{code:'INVALID_FIRMWARE_ARCHIVE'});
 }
});
test('manifest ZIP parser rejects duplicate paths, unsafe ignored entries and overlapping records',async()=>{
 for(const name of ['app.bin','../ignored.txt','folder/ignored.txt']){
  const zip=firmwareZipEntries([{data:bytes,name:'app.bin'},{data:'ignored',name}]);await assert.rejects(extractFirmwareZipEntry(zip,entrySource(zip)),{code:'INVALID_FIRMWARE_ARCHIVE'});
 }
 const zip=firmwareZipEntries([{data:bytes,name:'app.bin'},{data:'ignored',name:'README.txt'}]),cd=zip.readUInt32LE(zip.length-6),second=cd+46+Buffer.byteLength('app.bin');
 for(const mutate of [z=>z.writeUInt32LE(0,second+42),z=>z.writeUInt32LE(0xa0000000,second+38),z=>z.writeUInt16LE(1,second+8),z=>z.writeUInt32LE(0xffffffff,second+24)]){
  const bad=Buffer.from(zip);mutate(bad);await assert.rejects(extractFirmwareZipEntry(bad,entrySource(bad)),{code:'INVALID_FIRMWARE_ARCHIVE'});
 }
 const aborted=new AbortController();aborted.abort();await assert.rejects(extractFirmwareZipEntry(zip,entrySource(zip),{signal:aborted.signal}),{code:'FETCH_ABORTED'});
});
test('manifest ZIP accepts 64 entries and rejects 65 without increasing download limits',async()=>{
 assert.equal(MAX_FIRMWARE_ZIP_ENTRIES,64);assert.equal(MAX_FIRMWARE_ZIP_BYTES,64*1024*1024);
 for(const count of [64,65]){
  const zip=firmwareZipEntries([{data:bytes,name:'app.bin'},...Array.from({length:count-1},(_,i)=>({data:'unused',name:`other-${i}.bin`}))]);
  if(count===64)assert.deepEqual(Buffer.from(await extractFirmwareZipEntry(zip,entrySource(zip))),bytes);
  else await assert.rejects(extractFirmwareZipEntry(zip,entrySource(zip)),{code:'INVALID_FIRMWARE_ARCHIVE'});
 }
 const zip=firmwareZipEntries([{data:bytes,name:'app.bin'}]);
 await assert.rejects(extractFirmwareZipEntry(new Uint8Array(MAX_FIRMWARE_ZIP_BYTES+1),entrySource(zip)),{code:'INVALID_FIRMWARE_ARCHIVE'});
});
test('declared ZIP totals have a separate 128 MiB bound and ignored members are not inflated',async()=>{
 assert.equal(MAX_FIRMWARE_ZIP_DECLARED_BYTES,128*1024*1024);
 const zip=firmwareZipEntries([{data:bytes,name:'app.bin'},{data:'unused',name:'other-a.bin'},{data:'unused',name:'other-b.bin'}]);
 const cd=zip.readUInt32LE(zip.length-6);let cursor=cd;
 const positions=[];for(let i=0;i<3;i++){positions.push(cursor);cursor+=46+zip.readUInt16LE(cursor+28)+zip.readUInt16LE(cursor+30)+zip.readUInt16LE(cursor+32);}
 // Matching local/directory declarations exercise the arithmetic without
 // allocating or inflating ignored payloads. Only app.bin is decoded and CRC checked.
 const declare=(z,index,size)=>{const at=positions[index],local=z.readUInt32LE(at+42);z.writeUInt32LE(size,at+24);z.writeUInt32LE(size,local+22);};
 declare(zip,1,MAX_FIRMWARE_ZIP_BYTES);declare(zip,2,MAX_FIRMWARE_ZIP_BYTES-bytes.length);
 assert.deepEqual(Buffer.from(await extractFirmwareZipEntry(zip,entrySource(zip),{maxBytes:bytes.length})),bytes);
 const over=Buffer.from(zip);declare(over,2,MAX_FIRMWARE_ZIP_BYTES-bytes.length+1);
 await assert.rejects(extractFirmwareZipEntry(over,entrySource(over)),{code:'INVALID_FIRMWARE_ARCHIVE'});
 await assert.rejects(extractFirmwareZipEntry(zip,entrySource(zip),{maxBytes:bytes.length-1}),{code:'INVALID_FIRMWARE_ARCHIVE'});
});
test('ZIP stored/deflated BINs and streaming data descriptors decode exact bytes',async()=>{
 for(const method of [0,8])for(const descriptor of [false,true])assert.deepEqual(Buffer.from(await extractFirmwareZip(firmwareZip(bytes,{method,descriptor}),source)),bytes);
});
test('ZIP enforces file identity, count, lengths, checksums and non-encryption',async()=>{
 const zip=firmwareZip(bytes),end=zip.length-22,cd=zip.readUInt32LE(end+16);
 for(const change of [z=>z.writeUInt16LE(2,end+10),z=>z.writeUInt16LE(1,cd+8),z=>z.writeUInt16LE(99,cd+10),z=>z.writeUInt32LE(0xffffffff,cd+24),z=>z.writeUInt32LE(1,cd+42),z=>z[30]^=1,z=>z.writeUInt32LE(1,14),z=>z.writeUInt32LE(1,cd+16),z=>z.writeUInt32LE(1,cd+24),z=>z.writeUInt32LE(0xa0000000,cd+38)]){
  const bad=Buffer.from(zip);change(bad);await assert.rejects(extractFirmwareZip(bad,source));
 }
 await assert.rejects(extractFirmwareZip(zip.subarray(0,-1),source));
 await assert.rejects(extractFirmwareZip(zip,source,{maxBytes:bytes.length-1}),{code:'INVALID_FIRMWARE_ARCHIVE'});
 await assert.rejects(extractFirmwareZip(zip,{...source,archive:{format:'zip',entry:'other.bin'}}),{code:'INVALID_FIRMWARE_ARCHIVE'});
 const abort=new AbortController();abort.abort();await assert.rejects(extractFirmwareZip(zip,source,{signal:abort.signal}),{code:'FETCH_ABORTED'});
 for(const archive of [{format:'rar',entry:'firmware.bin'},{format:'zip',entry:'../firmware.bin'},{format:'zip',entry:'a/firmware.bin'},{format:'zip',entry:'firmware.bin',extra:true}])assert.equal(validFirmwareArchive({...source,archive}),false);
 assert.equal(validFirmwareArchive({...source,sha256:undefined}),false);
});
test('archive identity survives scene/history roundtrip and cannot match a raw BIN',async()=>{
 const catalog={};for(const name of ['products','accessories','firmwares'])catalog[name]=JSON.parse(await readFile(new URL(`../web/platform/catalog/${name}.json`,import.meta.url))).items;
 const scene={...createDefaultScene(catalog,'lilygo-t-embed-cc1101'),firmware:source};assert.equal(validateScene(scene,catalog).valid,true);assert.deepEqual(decodeScene(encodeScene(scene)).firmware,source);
 assert.equal(validateScene({...scene,firmware:{...source,sha256:undefined}},catalog).valid,false);
 assert.equal(firmwareSourceMatches(source,{url:source.url,sha256:source.sha256}),false);assert.equal(firmwareSourceMatches(source,structuredClone(source)),true);
 const memory={getItem(){return this.value||null;},setItem(k,v){this.value=v;}};const history=createFirmwareHistory(()=>memory);assert.equal(history.remember({...source,kind:'merged',productId:scene.productId,lastUsedAt:1}),true);assert.deepEqual(createFirmwareHistory(()=>memory).list()[0].archive,source.archive);
});
