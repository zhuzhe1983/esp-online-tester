import test from 'node:test';
import assert from 'node:assert/strict';
import {gzipSync} from 'node:zlib';
import {readModelBinary} from '../web/platform/model-binary.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';

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
