import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {test} from 'node:test';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
const platform=new URL('../web/platform/',import.meta.url),asset=new URL('assets/espressif-esp-mosaico/',platform);
const product=JSON.parse(await readFile(new URL('catalog/products.json',platform))).items.find(p=>p.id==='espressif-esp-mosaico');
const metadata=JSON.parse(await readFile(new URL('model.json',asset))),binary=await readFile(new URL('geometry.bin',asset));
test('Mosaico renderer keeps sourced geometry, live screen, real inputs and replaceable UNKNOWN boards',async()=>{
 assert.equal(createHash('sha256').update(binary).digest('hex'),metadata.artifacts['geometry.bin']);
 const original=globalThis.fetch,load=THREE.TextureLoader.prototype.loadAsync;let model;
 try{
  globalThis.fetch=async url=>new Response(await readFile(new URL(new URL(url).pathname.split('/platform/')[1],platform)));
  THREE.TextureLoader.prototype.loadAsync=async()=>new THREE.Texture();
  const texture=new THREE.Texture();model=createManufacturerModel(product,texture);await model.ready;
  assert.equal(model.screen.material.map,texture,'Screen must use supplied runtime texture');
  assert.equal(model.screen.geometry.parameters.width,38.39);assert.equal(model.screen.geometry.parameters.height,38.39);
  assert.deepEqual(model.buttons.map(x=>x.userData.control).sort(),['ai','boot']);
  assert.equal(model.parts['button-power'].userData.control,undefined,'Hardware power controller must not become a fake GPIO');
  for(const id of ['pcb-main','pcb-base'])assert.equal(model.parts[id].userData.kind,'pcb-placeholder');
  assert.equal(model.group.userData.inspectionError,undefined);
  const before=model.group.children.map(x=>x.position.toArray());
  for(let i=0;i<20;i++){model.group.userData.inspection.setExpanded(true);model.group.userData.inspection.setExpanded(false);}
  assert.deepEqual(model.group.children.map(x=>x.position.toArray()),before,'Repeated inspection/reset must not drift');
  model.group.updateMatrixWorld(true);
  for(const [name,axis] of [['button-boot',new THREE.Vector3(0,-1,0)],['button-ai',new THREE.Vector3(0,1,0)]]){
   const button=model.parts[name],center=new THREE.Box3().setFromObject(button).getCenter(new THREE.Vector3());
   const hit=new THREE.Raycaster(center.clone().addScaledVector(axis,25),axis.clone().negate()).intersectObjects(model.group.children,true)[0];
   assert.equal(hit.object,button,`${name} must be the visible nearest input surface`);
  }
  assert.equal(model.connectors['usb-c'].allowConnections,false,'Unverified insertion geometry must stay visual only');
  for(const shell of model.group.userData.shellMeshes)assert.equal(shell.name,'shell');
 }finally{globalThis.fetch=original;THREE.TextureLoader.prototype.loadAsync=load;model?.dispose();model?.group.traverse(x=>{x.geometry?.dispose();for(const m of [].concat(x.material||[]))m.dispose();});}
});
