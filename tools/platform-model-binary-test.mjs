import test from 'node:test';
import assert from 'node:assert/strict';
import {gzipSync} from 'node:zlib';
import {readModelBinary} from '../web/platform/model-binary.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {packModelGeometry} from './model-geometry-pack.mjs';

test('gzip preserves the actual manufacturer mesh coordinates, normals and bounds',async()=>{
  const vertices=new Float32Array([-1,-2,0,0,0,1,3,-2,0,0,0,1,0,4,0,0,0,1]);
  const nativeFetch=globalThis.fetch,models=[],snapshots=[];
  try {
    for(const compression of [undefined,'gzip']){
      const binary=compression?'geometry.bin.gz':'geometry.bin';
      const metadata={unit:'mm',strideFloats:6,binary,binaryCompression:compression,parts:[{name:'pcb',role:'pcb',byteOffset:0,vertexCount:3}]};
      globalThis.fetch=async url=>new Response(new URL(url).pathname.endsWith('model.json')?JSON.stringify(metadata):compression?gzipSync(vertices):vertices.buffer);
      const model=createManufacturerModel({id:'compressed-fixture',model:{url:'./assets/compressed-fixture/model.json'}},null);models.push(model);await model.ready;
      const geometry=model.parts.pcb.geometry;geometry.computeBoundingBox();
      snapshots.push({floats:[...geometry.attributes.position.data.array],bounds:[...geometry.boundingBox.min.toArray(),...geometry.boundingBox.max.toArray()]});
    }
    assert.deepEqual(snapshots[0],snapshots[1]);assert.deepEqual(snapshots[1].floats,[...vertices]);assert.deepEqual(snapshots[1].bounds,[-1,-2,0,3,4,0]);
  } finally {globalThis.fetch=nativeFetch;for(const model of models){model.dispose();model.parts.pcb?.geometry.dispose();model.parts.pcb?.material.dispose();}}
});

test('geometry decoding rejects truncated input and bounds stored and expanded bytes',async()=>{
  const source=new Uint8Array(1024),compressed=gzipSync(source);
  await assert.rejects(readModelBinary(new Response(compressed.subarray(0,compressed.length-4)),'gzip'));
  await assert.rejects(readModelBinary(new Response(source),'br'),/Unsupported/);
  await assert.rejects(readModelBinary(new Response(source),undefined,128),/exceeds 128 bytes/);
  await assert.rejects(readModelBinary(new Response(compressed),'gzip',128),/exceeds 128 bytes/);
  await assert.rejects(readModelBinary(new Response('x',{headers:{'Content-Length':'1024'}}),undefined,128),/exceeds 128 bytes/);
  let canceled=false;
  const stream=new ReadableStream({pull(controller){controller.enqueue(source.subarray(0,64));},cancel(){canceled=true;}});
  await assert.rejects(readModelBinary(new Response(stream),undefined,128),/exceeds 128 bytes/);
  assert.equal(canceled,true,'An oversized producer is canceled, not consumed indefinitely');
});

test('published vertex transport preserves every float bit including signed zero',async()=>{
  const vertices=new Float32Array(15000*6);
  for(let i=0;i<15000;i++){vertices[i*6]=(i%100)/7;vertices[i*6+1]=Math.floor(i/100)/13;vertices[i*6+2]=-0;vertices[i*6+5]=1;}
  const original=Buffer.from(vertices.buffer),packed=packModelGeometry(original);
  assert.ok(['gzip-xor-shuffle-24','gzip-split-indexed-12'].includes(packed.compression));
  assert.ok(packed.data.length<gzipSync(original,{level:9}).length);
  const decoded=await readModelBinary(new Response(packed.data),packed.compression);
  assert.deepEqual(Buffer.from(decoded),original);
  assert.ok(Object.is(new Float32Array(decoded)[2],-0));
  await assert.rejects(readModelBinary(new Response(gzipSync(new Uint8Array(25))),'gzip-xor-shuffle-24'),/stride/);
  await assert.rejects(readModelBinary(new Response(packed.data),packed.compression,128),/exceeds 128 bytes/);
  await assert.rejects(readModelBinary(new Response(packed.data.subarray(0,-4)),packed.compression));
});

test('split position/normal dictionaries reject invalid counts, indices and reconstructed sizes',async()=>{
  const wire=Buffer.alloc(44);wire.writeUInt32LE(24,0);wire.writeUInt32LE(1,4);wire.writeUInt32LE(1,8);
  const expected=Buffer.from(new Float32Array([1,2,3,0,0,1]).buffer);expected.copy(wire,12);
  const mode='gzip-split-indexed-12';
  assert.deepEqual(Buffer.from(await readModelBinary(new Response(gzipSync(wire)),mode)),expected);
  const invalidIndex=Buffer.from(wire);invalidIndex[36]=1;
  await assert.rejects(readModelBinary(new Response(gzipSync(invalidIndex)),mode),/vertex index/);
  const huge=Buffer.alloc(12);huge.writeUInt32LE(2048,0);
  await assert.rejects(readModelBinary(new Response(gzipSync(huge)),mode,1024),/exceeds 1024 bytes/);
  const invalidCount=Buffer.from(wire);invalidCount.writeUInt32LE(2,4);
  await assert.rejects(readModelBinary(new Response(gzipSync(invalidCount)),mode),/geometry size/);
  await assert.rejects(readModelBinary(new Response(gzipSync(wire.subarray(0,-1))),mode),/geometry size/);
});
