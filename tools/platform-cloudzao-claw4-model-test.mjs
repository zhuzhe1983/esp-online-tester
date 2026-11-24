// Load the real geometry and inspection behavior; image decoding has a test double.
// This check is not a browser screenshot, firmware execution or physical fit test.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {applyShellAppearance} from '../web/platform/shell-appearance.js';
import {firmwareRunPolicy,firmwareFlashPolicy} from '../web/platform/firmware-policy.js';

const json=async path=>JSON.parse(await readFile(new URL(path,import.meta.url),'utf8'));
const product=(await json('../web/platform/catalog/products.json')).items.find(item=>item.id==='cloudzao-claw4');
assert.ok(product,'Claw4 must be present in the published catalog');
const modelUrl=new URL('../web/platform/assets/cloudzao-claw4/model.json',import.meta.url);
const metadata=JSON.parse(await readFile(modelUrl,'utf8'));
const hash=data=>createHash('sha256').update(data).digest('hex');

test('Claw4 source assets retain exact original bytes and never supply photo screen pixels',async()=>{
  for(const source of metadata.provenance.sources){
    const bytes=await readFile(new URL(source.archiveName,modelUrl));
    assert.equal(hash(bytes),source.sha256,source.id);
    assert.equal(bytes.byteLength,source.bytes,source.id);
  }
  for(const [name,digest]of Object.entries(metadata.artifacts))assert.equal(hash(await readFile(new URL(name,modelUrl))),digest,name);
  assert.equal(metadata.provenance.kind,'image-measurement');
  assert.equal(metadata.debugPreview.physicalDimensionsVerified,false);
  assert.equal(metadata.debugPreview.liveFramebuffer,false);
  assert.equal(metadata.presentation.screen.texture,undefined);
});

test('Claw4 actual model loads, exposes UNKNOWN, resets inspection and isolates shell colors',async()=>{
  const oldFetch=globalThis.fetch,oldLoad=THREE.TextureLoader.prototype.loadAsync;
  const texture=new THREE.Texture();let model;
  globalThis.fetch=async url=>{const local=new URL(url);local.search='';return new Response(await readFile(local));};
  THREE.TextureLoader.prototype.loadAsync=async()=>new THREE.Texture();
  try{
    model=createManufacturerModel(product,texture);await model.ready;
    assert.equal(model.group.userData.inspectionError,undefined);
    assert.equal(model.parts['pcb-main'].userData.kind,'pcb-placeholder');
    assert.equal(model.parts['pcb-main'].userData.label,'UNKNOWN');
    assert.equal(model.screen.material.map,texture,'Display must use the caller framebuffer/blank texture');
    assert.ok(Math.abs(model.screen.geometry.parameters.width-3.95*25.4/Math.sqrt(2))<1e-7);
    assert.equal(model.buttons.length,0,'Unavailable runtime must not invent working model buttons');
    assert.deepEqual(Object.keys(model.connectors),[],'No unverified physical cable anchors');
    const inspection=model.group.userData.inspection,base=model.group.children.map(p=>[p.name,p.position.toArray()]);
    for(let i=0;i<5;i++){inspection.setExpanded(true);assert.equal(inspection.expanded,true);inspection.setExpanded(false);}
    assert.deepEqual(model.group.children.map(p=>[p.name,p.position.toArray()]),base);
    const fixed=['front-cover','rear-cover','power-key','camera-lens','rear-logo'];
    const before=fixed.map(name=>model.parts[name].material.color?.getHexString());
    const shells=model.group.userData.shellMeshes;
    assert.equal(shells.length,7);
    for(const value of [{shell:{color:'#d8ad44',opacity:1,finish:'metal'}},{shell:{color:'#8a5bd6',opacity:.25,finish:'plastic'}}]){
      for(const shell of shells)applyShellAppearance(shell,value);
      assert.deepEqual(fixed.map(name=>model.parts[name].material.color?.getHexString()),before);
    }
  }finally{
    globalThis.fetch=oldFetch;THREE.TextureLoader.prototype.loadAsync=oldLoad;model?.dispose();texture.dispose();
    model?.group.traverse(object=>{object.geometry?.dispose();for(const material of [].concat(object.material||[]))material.dispose();});
  }
});

test('Claw4 preview cannot run another board or promise unsupported online P4 flashing',()=>{
  const source={url:'https://example.invalid/official-image.bin'};
  assert.equal(product.chip,'esp32p4');assert.equal(product.runtime.board,null);
  assert.equal(firmwareRunPolicy(product,{source}).canRun,false);
  assert.equal(firmwareRunPolicy(product,{source}).reason,'board-unavailable');
  assert.equal(firmwareFlashPolicy(product,source).reason,'FLASH_UNSUPPORTED_CHIP');
  assert.equal(product.interfaces.find(port=>port.type==='microSD').id,'micro-sd');
});
