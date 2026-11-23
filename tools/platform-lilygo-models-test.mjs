// Inventory-wide binary and provenance checks. They do not claim physical fit
// or factory-firmware execution; those need independent per-device evidence.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {gunzipSync} from 'node:zlib';
const root=new URL('../',import.meta.url),hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const read=async path=>JSON.parse(await readFile(new URL(path,root)));
const patches=await read('docs/platform/research/lilygo-model-catalog-patches-2026-11-22.json');
const products=(await read('web/platform/catalog/products.json')).items;
test('all 122 previously missing LilyGO previews have intact geometry and explicit reconstruction limits',async()=>{
  assert.equal(patches.items.length,122);assert.equal(new Set(patches.items.map(item=>item.id)).size,122);
  for(const patch of patches.items){
    const product=products.find(item=>item.id===patch.id);assert.ok(product);
    assert.equal(product.model.url,patch.model.url);
    const base=new URL(`web/platform/assets/${patch.id}/`,root);
    const modelBytes=await readFile(new URL('model.json',base)),model=JSON.parse(modelBytes);
    const storedGeometry=await readFile(new URL(model.binary,base));
    assert.equal(hash(storedGeometry),model.artifacts[model.binary],patch.id+' stored geometry identity');
    assert([undefined,'gzip'].includes(model.binaryCompression));
    const geometry=model.binaryCompression==='gzip'?gunzipSync(storedGeometry,{maxOutputLength:64*1024*1024}):storedGeometry;
    for(const [file,digest] of Object.entries(model.artifacts))if(file!==model.binary)assert.equal(hash(await readFile(new URL(file,base))),digest,patch.id+' retained source '+file);
    assert.equal(model.id,patch.id);assert.equal(model.strideFloats,6);assert.equal(model.unit,'mm');
    const view=new DataView(geometry.buffer,geometry.byteOffset,geometry.byteLength);let end=0;
    for(const part of model.parts){
      assert.equal(part.byteOffset,end);assert.equal(part.vertexCount%3,0);
      end+=part.vertexCount*24;assert(end<=geometry.length);
      for(let at=part.byteOffset;at<end;at+=4)assert(Number.isFinite(view.getFloat32(at,true)),patch.id+' finite mesh');
    }
    assert.equal(end,geometry.length);assert(model.limitations.length>0);
    const check=JSON.parse(await readFile(new URL('build-check.json',base)));
    assert.equal(check.modelSha256,hash(modelBytes));assert.equal(check.geometrySha256,hash(geometry));assert.equal(check.exportPassed,true);
    if(model.binaryCompression==='gzip')assert.equal(check.storedGeometrySha256,hash(storedGeometry));
    if(['screen-only','debug-placeholder'].includes(patch.mode)){
      assert.notEqual(model.debugPreview?.isPhysicalReplica,true);
      if(product.dimensions)assert.equal(product.dimensions.basis,'official','Only independently sourced physical dimensions may accompany a debug preview');
    }
    if(product.runtime.board && product.display){
      assert(model.presentation?.screen,patch.id+' runtime display needs a real framebuffer surface');
      const [w,h]=model.presentation.screen.sizeMm;
      const ratio=product.display.width/product.display.height;
      assert(Math.abs(w/h-ratio)<.02||Math.abs(h/w-ratio)<.02,patch.id+' live screen aspect ratio');
    }
  }
});

test('T4-S3 and T5 S3 Pro place their live display on the assembled cover', async () => {
  const covers = {'lilygo-t4-s3':'display-glass','lilygo-t5-e-paper-s3-pro':'display-cover'};
  for (const [id, coverName] of Object.entries(covers)) {
    const model = await read(`web/platform/assets/${id}/model.json`);
    const cover = model.parts.find(part => part.name === coverName);
    assert.equal(cover?.basis, 'manufacturer-cad', id);
    assert.ok(model.parts.some(part => part.role === 'shell'), id);
    assert.ok(model.parts.some(part => part.role === 'pcb'), id);
    assert.ok(!model.parts.some(part => part.name === 'debug-screen-frame'), id);

    const {sizeMm, positionMm, rotation} = model.presentation.screen;
    const [x, y, z] = positionMm, angle = rotation[2];
    const width = Math.abs(Math.cos(angle)) * sizeMm[0] + Math.abs(Math.sin(angle)) * sizeMm[1];
    const height = Math.abs(Math.sin(angle)) * sizeMm[0] + Math.abs(Math.cos(angle)) * sizeMm[1];
    const [[left, bottom], [right, top, front]] = cover.boundsMm;
    assert.ok(left < x - width / 2 && right > x + width / 2, id);
    assert.ok(bottom < y - height / 2 && top > y + height / 2, id);
    assert.ok(z > front && z - front < 0.05, id);

    const displayGroup = model.presentation.inspectionGroups.find(group => group.parts.includes('live-display'));
    assert.ok(displayGroup?.parts.includes(coverName), id);
    assert.ok(displayGroup.offsetMm[2] > 0, id);
  }
});
