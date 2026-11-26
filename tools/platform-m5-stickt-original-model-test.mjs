import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile,mkdtemp,rm} from 'node:fs/promises';
import {execFileSync} from 'node:child_process';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {Texture,Box3,Vector3,Quaternion,Raycaster} from '../web/platform/vendor/three.module.min.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
const root=new URL('../',import.meta.url),base=new URL('web/platform/assets/m5stack-k016-t/',root);
test('original K016-T retains exact-SKU photos and reconstructs the 30 x 48 x 29 envelope with UNKNOWN PCB',async()=>{
 const p=JSON.parse(await readFile(new URL('web/platform/catalog/products.json',root))).items.find(p=>p.id==='m5stack-k016-t');
 assert.deepEqual([p.dimensions.width,p.dimensions.height,p.dimensions.depth],[30,48,29]);assert.equal(p.dimensions.basis,'official');
 const plan=JSON.parse(await readFile(new URL('model-plan.json',base)));assert.equal(plan.kind,'photo');assert(plan.sources.some(s=>s.id==='original-docs'&&s.url.includes('/docs/en/core/m5stickt.md')));assert(plan.derivedParts.length===0);
 const previous=globalThis.fetch;globalThis.fetch=async value=>{const u=new URL(value);u.search='';return new Response(await readFile(u));};const tex=new Texture(),model=createManufacturerModel(p,tex);
 try{
  await model.ready;model.group.updateMatrixWorld(true);assert.equal(model.group.userData.inspectionError,undefined);assert(model.parts['pcb-main']);
  const meshes=[];model.group.traverse(part=>{if(part.isMesh)meshes.push(part);});
  // Zero-opacity input planes extend 0.01mm beyond the case for picking. They
  // are not physical hardware and must not inflate its official envelope.
  const physicalBounds=new Box3();
  for(const mesh of meshes){
   let visible=true;for(let parent=mesh;parent;parent=parent.parent)visible&&=parent.visible;
   if(visible&&[mesh.material].flat().some(material=>material.visible&&material.opacity>0))physicalBounds.union(new Box3().setFromObject(mesh));
  }
  const envelope=physicalBounds.getSize(new Vector3()).toArray();
  for(const[i,v]of[30,48,29].entries())assert(Math.abs(envelope[i]-v)<.00001);
  const rawBounds=new Box3().setFromObject(model.group);
  assert(Math.abs(rawBounds.max.x-physicalBounds.max.x-.01)<.00001,'Input-only extension explains the whole-scene Box3 difference');
  assert.deepEqual(model.buttons.map(part=>part.userData.control),['button-a','button-b','power']);
  assert.deepEqual(p.controls.map(control=>[control.id,control.pin]),[['button-a',37],['button-b',39],['power',undefined]]);
  assert.equal(p.controls.find(control=>control.id==='power').transport,'board');
  for(const id of ['button-b','power']){
   const region=plan.presentation.buttons.find(button=>button.id===id),part=model.parts['cover-key:'+id];
   assert.deepEqual(region.sourceIds,['photo-1']);assert.equal(region.supportingPart,'shell-wall');
   assert.match(region.notes,/estimate/);assert.equal(part.material.opacity,0);assert.equal(part.material.depthWrite,false);
   const target=part.getWorldPosition(new Vector3()),normal=new Vector3(0,0,1).applyQuaternion(part.getWorldQuaternion(new Quaternion()));
   const hit=direction=>new Raycaster(target.clone().addScaledVector(direction,100),direction.clone().negate()).intersectObjects(meshes,false)[0]?.object;
   assert.ok(hit(normal)===part,id+' is the nearest source-visible side input');
   assert.ok(hit(normal.clone().negate())!==part,id+' remains blocked through the opposite wall');
  }
  assert.equal(model.screen.material.map,tex);const initial=model.group.children.map(p=>[p,p.position.clone()]);model.group.userData.inspection.setExpanded(true);model.group.userData.inspection.setExpanded(false);for(const[p,v]of initial)assert.deepEqual(p.position.toArray(),v.toArray());
 }finally{model.dispose();tex.dispose();globalThis.fetch=previous;}
 const scratch=await mkdtemp(join(tmpdir(),'stickt-original-model-'));try{execFileSync('python',[fileURLToPath(new URL('tools/build-m5-legacy-kit-models.py',root)),'m5stack-k016-t','--output',scratch]);for(const f of['geometry.bin','model.json'])assert.deepEqual(await readFile(join(scratch,f)),await readFile(new URL(f,base)));}finally{await rm(scratch,{recursive:true,force:true});}
});
