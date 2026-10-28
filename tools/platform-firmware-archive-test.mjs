import test from 'node:test';import assert from 'node:assert/strict';
import {firmwareZip} from './fixtures/firmware-zip.mjs';
import {validFirmwareArchive,extractFirmwareZip} from '../web/platform/firmware-archive.js';
import {createDefaultScene,validateScene,encodeScene,decodeScene,firmwareSourceMatches} from '../web/platform/catalog.js';
import {createFirmwareHistory} from '../web/platform/firmware-history.js';
import {readFile} from 'node:fs/promises';
const source={url:'https://example.org/firmware.zip',sha256:'a'.repeat(64),archive:{format:'zip',entry:'firmware.bin'}},bytes=Buffer.from('a real compressed payload\n'.repeat(1000));
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
