import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {test} from 'node:test';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
const platform=new URL('../web/platform/',import.meta.url),asset=new URL('assets/espressif-esp-mosaico/',platform);
const product=JSON.parse(await readFile(new URL('catalog/products.json',platform))).items.find(p=>p.id==='espressif-esp-mosaico');
const metadata=JSON.parse(await readFile(new URL('model.json',asset))),binary=await readFile(new URL('geometry.bin',asset));
test('Mosaico retains all original CAD triangles, assembly provenance and original license',()=>{
 const cad=metadata.parts.filter(p=>p.basis==='manufacturer-cad');
 assert.equal(cad.reduce((n,p)=>n+p.vertexCount/3,0),23732);
 for(const [id,count] of [[2,19916],[4,2976],[6,840]])assert.equal(cad.filter(p=>p.sourceObjectId===id).reduce((n,p)=>n+p.vertexCount/3,0),count);
 assert.deepEqual(metadata.sourceGeometry.presentationMatrixRows,[[-1,0,0,0],[0,-1,0,0],[0,0,1,-5.550025463104248],[0,0,0,1]]);
 assert.equal(metadata.sourceGeometry.printingPlateTransformsUsed,false);
 assert.equal(metadata.sourceGeometry.omittedTriangles,0);
 assert.equal(metadata.sourceGeometry.sourceSha256,'aa84074593344c37684cc2f36c58b5698ed5b695246e701ff05e49d325c176b3');
 assert.match(metadata.provenance.license,/MakerWorld Exclusive License/);
 assert.equal(metadata.parts.some(p=>p.name==='rear-panel'),false,'3MF supplies no rear cover');
 assert.deepEqual(cad.filter(p=>p.sourceObjectId===4).map(p=>p.name),['button-boot','button-power','button-link']);
 assert.ok(cad.every(p=>p.sourceObjectId!==4 || p.assemblyGroup==='official-white-dual-button'));
});
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
  const power=model.parts['button-power'],powerCenter=new THREE.Box3().setFromObject(power).getCenter(new THREE.Vector3());
  const powerHit=new THREE.Raycaster(powerCenter.clone().add(new THREE.Vector3(0,-25,0)),new THREE.Vector3(0,1,0)).intersectObjects(model.group.children,true)[0];
  assert.equal(powerHit.object,power);assert.equal(powerHit.object.userData.control,undefined);
  for(const [x,y,u,v] of [[0,0,.5,.5],[-38.39*.375,38.39*.375,.125,.875]]){
   const hit=new THREE.Raycaster(new THREE.Vector3(x,y,40),new THREE.Vector3(0,0,-1)).intersectObjects(model.group.children,true)[0];
   assert.equal(hit.object,model.screen,'Touch rays must reach the runtime display before the cover');
   assert.ok(Math.abs(hit.uv.x-u)<1e-6&&Math.abs(hit.uv.y-v)<1e-6,'Screen UV orientation must retain guest touch coordinates');
  }
  assert.equal(new THREE.Raycaster(new THREE.Vector3(0,0,-40),new THREE.Vector3(0,0,1)).intersectObject(model.parts.shell).length,0,'Original shell remains open front-to-back');
  const usbHit=new THREE.Raycaster(new THREE.Vector3(0,-40,-1.8),new THREE.Vector3(0,1,0)).intersectObject(model.parts.shell);
  assert.ok(usbHit.every(hit=>hit.point.y>0),'USB entry must remain an actual hole through the near shell wall');
  assert.equal(model.connectors['usb-c'].allowConnections,false,'Unverified insertion geometry must stay visual only');
  for(const shell of model.group.userData.shellMeshes)assert.equal(shell.name,'shell');
 }finally{globalThis.fetch=original;THREE.TextureLoader.prototype.loadAsync=load;model?.dispose();model?.group.traverse(x=>{x.geometry?.dispose();for(const m of [].concat(x.material||[]))m.dispose();});}
});
