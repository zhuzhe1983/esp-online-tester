// Actual Three.js geometry and model interactions; no GPU or browser acceptance.
// Files are read locally and image decoding is replaced by an empty texture.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {applyShellAppearance} from '../web/platform/shell-appearance.js';

const product=(JSON.parse(await readFile(new URL('../web/platform/catalog/products.json',import.meta.url)))).items.find(p=>p.id==='cloudzao-claw-154');
const modelUrl=new URL('../web/platform/assets/cloudzao-claw-154/model.json',import.meta.url);
const metadata=JSON.parse(await readFile(modelUrl));

test('Claw1.54 model assets retain source hashes and declare single-view uncertainty',async()=>{
  assert.equal(product.model.type,'source-derived');
  assert.equal(metadata.provenance.kind,'photo-derived');
  assert.equal(metadata.referencePhotos.length,1);
  assert.equal(metadata.debugPreview.isPhysicalReplica,false);
  assert.equal(metadata.debugPreview.physicalDimensionsVerified,false);
  assert.equal(metadata.presentation.screen.texture,undefined);
  for(const [name,digest]of Object.entries(metadata.artifacts)){
    assert.equal(createHash('sha256').update(await readFile(new URL(name,modelUrl))).digest('hex'),digest,name);
  }
});

test('Claw1.54 actual model keeps five inputs, live screen and UNKNOWN through inspection and colors',async()=>{
  const oldFetch=globalThis.fetch,oldLoad=THREE.TextureLoader.prototype.loadAsync;
  const framebuffer=new THREE.Texture();let model;
  globalThis.fetch=async url=>{const local=new URL(url);local.search='';return new Response(await readFile(local));};
  THREE.TextureLoader.prototype.loadAsync=async()=>new THREE.Texture();
  try{
    model=createManufacturerModel(product,framebuffer);await model.ready;
    assert.equal(model.group.userData.inspectionError,undefined);
    assert.equal(model.parts['pcb-main'].userData.label,'UNKNOWN');
    assert.equal(model.screen.material.map,framebuffer);
    assert.ok(Math.abs(model.screen.geometry.parameters.width-1.54*25.4/Math.sqrt(2))<1e-7);
    assert.deepEqual(model.buttons.map(button=>button.userData.control).sort(),['boot','home','next','power','previous']);
    assert.equal(model.connectors['usb-c'].allowConnections,false,'The nominal USB face must remain visual only');
    assert.equal(model.connectors['usb-c'].anchor.userData.visualOnly,true);
    assert.equal(model.connectors['usb-c'].pinOrder,undefined,'An uncertain USB face must not invent a connector pinout');
    const inspection=model.group.userData.inspection,base=model.group.children.map(part=>[part.name,part.position.toArray()]);
    const screenAt=model.screen.position.clone();
    inspection.setExpanded(true);assert.notDeepEqual(model.screen.position.toArray(),screenAt.toArray());
    inspection.setExpanded(false);assert.deepEqual(model.group.children.map(part=>[part.name,part.position.toArray()]),base);
    const fixed=['front-glass-frame','debug-boot','debug-power'];
    const colors=fixed.map(name=>model.parts[name].material.color.getHexString());
    for(const appearance of [{shell:{color:'#d8ad44',opacity:1,finish:'metal'}},{shell:{color:'#8a5bd6',opacity:.25,finish:'plastic'}}]){
      for(const shell of model.group.userData.shellMeshes)applyShellAppearance(shell,appearance);
      assert.deepEqual(fixed.map(name=>model.parts[name].material.color.getHexString()),colors);
      assert.equal(model.screen.material.map,framebuffer);
    }
  }finally{
    globalThis.fetch=oldFetch;THREE.TextureLoader.prototype.loadAsync=oldLoad;model?.dispose();framebuffer.dispose();
    model?.group.traverse(object=>{object.geometry?.dispose();for(const material of [].concat(object.material||[]))material.dispose();});
  }
});
