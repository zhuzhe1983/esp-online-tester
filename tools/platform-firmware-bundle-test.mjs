import test from 'node:test';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {firmwareZipEntries} from './fixtures/firmware-zip.mjs';
import {extractFirmwareZip,MAX_FIRMWARE_ZIP_BYTES,MAX_FIRMWARE_ZIP_ENTRIES} from '../web/platform/firmware-archive.js';
import {validFirmwareBundle,extractFirmwareBundle,MAX_FIRMWARE_BUNDLE_BYTES,MAX_FIRMWARE_BUNDLE_ENTRIES,MAX_FIRMWARE_BUNDLE_DECLARED_BYTES} from '../web/platform/firmware-bundle-archive.js';
const hash=b=>createHash('sha256').update(b).digest('hex');
const payload=Buffer.from('independent firmware bytes\n'.repeat(200));
const member='Exact-Demo/Firmwork/Factory.bin';
const source=zip=>({url:'https://example.org/exact-demo.zip',sha256:hash(payload),archive:{format:'zip-bundle',entry:member,sha256:hash(zip)}});
const extract=(zip,s=source(zip),options={})=>extractFirmwareBundle(zip,s,{maxBytes:payload.length,decodeSingle:extractFirmwareZip,...options});
function directory(zip){const end=zip.length-22,cd=zip.readUInt32LE(end+16),out=[];let at=cd;for(let i=0;i<zip.readUInt16LE(end+10);i++){out.push(at);at+=46+zip.readUInt16LE(at+28)+zip.readUInt16LE(at+30)+zip.readUInt16LE(at+32);}return out;}

test('exact nested factory member is read from a large source tree; legacy ZIP limits stay intact',async()=>{
  assert.equal(MAX_FIRMWARE_ZIP_BYTES,64*1024*1024);assert.equal(MAX_FIRMWARE_ZIP_ENTRIES,64);
  assert.equal(MAX_FIRMWARE_BUNDLE_BYTES,128*1024*1024);assert.equal(MAX_FIRMWARE_BUNDLE_ENTRIES,16384);
  const entries=[{name:'Exact-Demo/',data:''},{name:member,data:payload,descriptor:true},...Array.from({length:6090},(_,i)=>({name:`Exact-Demo/source/tree/file-${i}.cpp`,data:'unselected source'}))];
  const zip=firmwareZipEntries(entries),central=directory(zip);
  // Actual manufacturers include Unix directory-mode entries in the tree.
  zip.writeUInt32LE(0x41ed0000,central[0]+38);
  assert.equal(validFirmwareBundle(source(zip)),true);
  const originalHash=hash(zip);
  assert.deepEqual(Buffer.from(await extract(zip)),payload);
  assert.equal(hash(zip),originalHash,'Archive bytes, including Buffer-backed input, stay unchanged');
  await assert.rejects(extract(zip,source(zip),{maxBytes:payload.length-1}),{code:'INVALID_FIRMWARE_ARCHIVE'});
});

test('both container and selected image hashes are independent requirements',async()=>{
  const zip=firmwareZipEntries([{name:member,data:payload}]);
  await assert.rejects(extract(zip,{...source(zip),sha256:'0'.repeat(64)}),{code:'HASH_MISMATCH'});
  await assert.rejects(extract(zip,{...source(zip),archive:{...source(zip).archive,sha256:'0'.repeat(64)}}),{code:'HASH_MISMATCH'});
  for(const s of [{...source(zip),sha256:undefined},{...source(zip),archive:{format:'zip-bundle',entry:member}},
                  {...source(zip),archive:{...source(zip).archive,extra:true}}, {...source(zip),manifestUrl:'https://example.org/guess.json'}])assert.equal(validFirmwareBundle(s),false);
});

test('unselected members are not inflated, and their declared total remains bounded',async()=>{
  const entries=[{name:member,data:payload},...Array.from({length:4},(_,i)=>({name:`source/unused-${i}.txt`,data:'small'}))];
  const zip=firmwareZipEntries(entries),positions=directory(zip);
  const declare=(z,index,size)=>{const at=positions[index],local=z.readUInt32LE(at+42);z.writeUInt32LE(size,at+24);z.writeUInt32LE(size,local+22);};
  for(let i=1;i<5;i++)declare(zip,i,128*1024*1024-(i===4?payload.length:0));
  assert.equal(MAX_FIRMWARE_BUNDLE_DECLARED_BYTES,512*1024*1024);
  // The unrelated compressed streams cannot produce their declared sizes.
  // Success proves only the pinned BIN was inflated, without source-tree allocation.
  assert.deepEqual(Buffer.from(await extract(zip)),payload);
  const over=Buffer.from(zip);declare(over,4,128*1024*1024-payload.length+1);
  await assert.rejects(extract(over),{code:'INVALID_FIRMWARE_ARCHIVE'});
});

test('relative-path checks reject traversal, alternate separators, duplicates and links',async()=>{
  for(const name of ['../Factory.bin','a/../../Factory.bin','/Factory.bin','C:/Factory.bin','a\\Factory.bin','a//Factory.bin','a/./Factory.bin','a/../Factory.bin','a/Factory.bin\0','a/ Factory.bin']){
    assert.equal(validFirmwareBundle({...source(Buffer.alloc(0)),archive:{...source(Buffer.alloc(0)).archive,entry:name}}),false,name);
    const zip=firmwareZipEntries([{name:member,data:payload},{name,data:'unselected'}]);
    await assert.rejects(extract(zip),{code:'INVALID_FIRMWARE_ARCHIVE'},name);
  }
  const duplicate=firmwareZipEntries([{name:member,data:payload},{name:member,data:'other'}]);
  await assert.rejects(extract(duplicate),{code:'INVALID_FIRMWARE_ARCHIVE'});
  const zip=firmwareZipEntries([{name:member,data:payload},{name:'source/unused',data:'unused'}]),second=directory(zip)[1];
  const link=Buffer.from(zip);link.writeUInt32LE(0xa1ff0000,second+38);
  await assert.rejects(extract(link),{code:'INVALID_FIRMWARE_ARCHIVE'});
});

test('local/central identity, overlap, CRC, encryption, ZIP64 and cancellation are checked',async()=>{
  const zip=firmwareZipEntries([{name:member,data:payload,method:0},{name:'source/readme.txt',data:'unused'}]),[first,second]=directory(zip);
  for(const mutate of [z=>z.writeUInt32LE(0,second+42),z=>z.writeUInt16LE(1,first+8),z=>z.writeUInt16LE(45,first+6),z=>z.writeUInt32LE(0xffffffff,first+24),z=>z[30]^=1]){
    const bad=Buffer.from(zip);mutate(bad);await assert.rejects(extract(bad),{code:'INVALID_FIRMWARE_ARCHIVE'});
  }
  const crc=Buffer.from(zip);crc[30+Buffer.byteLength(member)]^=1;
  await assert.rejects(extract(crc),{code:'INVALID_FIRMWARE_ARCHIVE'});
  const controller=new AbortController();controller.abort();
  await assert.rejects(extract(zip,source(zip),{signal:controller.signal}),{code:'FETCH_ABORTED'});
  await assert.rejects(extract(new Uint8Array(MAX_FIRMWARE_BUNDLE_BYTES+1),source(zip)),{code:'INVALID_FIRMWARE_ARCHIVE'});
});
