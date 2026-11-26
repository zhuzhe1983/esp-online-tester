import test from 'node:test';
import assert from 'node:assert/strict';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createModelInspection} from '../web/platform/model-inspection.js';
import {createServoJoints,applyServoValues} from '../web/platform/servo-view.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';

const sources=[{id:'cad'}],declaration={id:'head-open',nested:true,parts:['cover'],offsetMm:[4,-2,12],sourceIds:['cad'],notes:'Source assembly axis; viewing clearance only.'};
const world=object=>object.getWorldPosition(new THREE.Vector3());
const near=(actual,expected)=>assert(actual.distanceTo(expected)<1e-9,`${actual.toArray()} != ${expected.toArray()}`);
const contains=(group,object)=>{for(let current=object;current;current=current.parent)if(current===group)return true;return false;};
function nestedAssembly(){
 const group=new THREE.Group(),parent=new THREE.Group(),inner=new THREE.Group(),cover=new THREE.Mesh(new THREE.BoxGeometry(2,3,4),new THREE.MeshBasicMaterial());
 group.rotation.set(.23,-.13,.34);group.scale.set(1.2,.8,1.4);parent.name='outer';parent.rotation.set(.3,.7,-.2);parent.scale.set(.7,1.6,1.3);parent.position.set(2,-4,8);inner.name='inner';inner.rotation.set(-.1,.2,.6);inner.position.set(-3,4,2);cover.name='cover';cover.position.set(4,2,-5);cover.rotation.set(.1,.2,.3);group.add(parent);parent.add(inner);inner.add(cover);group.updateMatrixWorld(true);return{group,parent,inner,cover};
}
test('nested inspection is opt-in and resolves an exact globally unique descendant name',()=>{
 const {group,cover}=nestedAssembly(),before=cover.position.clone();
 assert.throws(()=>createModelInspection(group,[{...declaration,nested:undefined}],sources),/ownership/);
 for(const nested of [null,1,'true'])assert.throws(()=>createModelInspection(group,[{...declaration,nested}],sources),/Invalid/);
 const twin=new THREE.Object3D();twin.name='cover';group.add(twin);
 assert.throws(()=>createModelInspection(group,[declaration],sources),/ownership/);
 const old=createModelInspection(group,[{...declaration,nested:false}],sources);old.setExpanded(true);near(cover.position,before);near(twin.position,new THREE.Vector3(4,-2,12));old.setExpanded(false);
 group.remove(twin);const inspection=createModelInspection(group,[declaration],sources);assert(inspection);inspection.setExpanded(true);assert(!cover.position.equals(before));inspection.setExpanded(false);assert.deepEqual(cover.position.toArray(),before.toArray());
});
test('nested offsets bind assembled model axes through rotated scaled parents without reparenting or mesh edits',()=>{
 const {group,inner,cover}=nestedAssembly(),parent=cover.parent,position=cover.position.clone(),rotation=cover.quaternion.clone(),scale=cover.scale.clone(),material=cover.material,vertices=cover.geometry.attributes.position.array.slice(),base=world(cover);
 const delta=group.localToWorld(new THREE.Vector3(...declaration.offsetMm)).sub(group.localToWorld(new THREE.Vector3()));
 const inspection=createModelInspection(group,[declaration],sources);
 for(const progress of [.25,1,.6,0,1,0]){inspection.setProgress(progress);near(world(cover),base.clone().addScaledVector(delta,progress));assert.equal(cover.parent,parent);assert.deepEqual(cover.quaternion.toArray(),rotation.toArray());assert.deepEqual(cover.scale.toArray(),scale.toArray());assert(cover.material===material);assert.deepEqual(cover.geometry.attributes.position.array,vertices);}
 assert.deepEqual(cover.position.toArray(),position.toArray());assert(contains(inner,cover));
 const singular=nestedAssembly();singular.parent.scale.x=0;assert.throws(()=>createModelInspection(singular.group,[declaration],sources),/parent transform/);
});
test('ancestor/descendant moving, hidden and companion ownership is rejected atomically in either order',()=>{
 for(const reverse of [false,true])for(const hidden of [false,true]){
  const {group,inner,cover}=nestedAssembly(),position=cover.position.clone(),visible=cover.visible;
  let definitions=[{...declaration,id:'ancestor',parts:['outer']},{...declaration,id:'child',parts:hidden?['inner']:['cover'],hideParts:hidden?['cover']:[]}];if(reverse)definitions.reverse();
  assert.throws(()=>createModelInspection(group,definitions,sources),/ownership/);assert.deepEqual(cover.position.toArray(),position.toArray());assert.equal(cover.visible,visible);assert.equal(inner.parent.name,'outer');
 }
 const {group,cover}=nestedAssembly(),photo=new THREE.Mesh(cover.geometry,cover.material);photo.name='photo';group.add(photo);cover.userData.inspectionCompanions=[photo];
 for(const parts of [['photo'],['cover','photo'],['cover','cover']])assert.throws(()=>createModelInspection(group,[{...declaration,parts}],sources),/ownership/);
 photo.add(new THREE.Object3D());photo.children[0].name='photo-child';assert.throws(()=>createModelInspection(group,[declaration,{...declaration,id:'photo-child',parts:['photo-child']}],sources),/ownership/);
 group.remove(photo);assert.throws(()=>createModelInspection(group,[declaration],sources),/ownership/);
});
test('nested source assemblies carry registered external and already nested photo companions once',()=>{
 const {group,inner,cover}=nestedAssembly(),external=new THREE.Mesh(cover.geometry,cover.material),nested=new THREE.Mesh(cover.geometry,cover.material),photoParent=new THREE.Group();
 external.name='photo-external';nested.name='photo-nested';photoParent.name='photo-parent';photoParent.rotation.set(.4,-.2,.1);photoParent.scale.set(1.5,.8,1.2);group.add(photoParent);photoParent.add(external);cover.add(nested);cover.userData.inspectionCompanions=[external,nested];group.updateMatrixWorld(true);
 const base=world(cover),photoBase=world(external),childAt=nested.position.clone(),delta=group.localToWorld(new THREE.Vector3(...declaration.offsetMm)).sub(group.localToWorld(new THREE.Vector3())),initialParent=cover.parent;
 const inspection=createModelInspection(group,[declaration],sources);inspection.setProgress(1);near(world(cover),base.clone().add(delta));near(world(external),photoBase.clone().add(delta));assert.deepEqual(nested.position.toArray(),childAt.toArray());assert.equal(cover.parent,initialParent);assert.equal(external.parent,photoParent);assert.equal(nested.parent,cover);inspection.setProgress(0);near(world(cover),base);near(world(external),photoBase);
 const other=new THREE.Object3D();other.name='other';group.add(other);
 const hidden=createModelInspection(group,[{...declaration,parts:['other'],hideParts:['inner']}],sources);hidden.setExpanded(true);assert.equal(inner.visible,false);assert.equal(external.visible,false);assert.equal(photoParent.visible,true);hidden.setExpanded(false);assert.equal(inner.visible,true);assert.equal(external.visible,true);
});
function articulated(){
 const group=new THREE.Group(),stand=new THREE.Object3D(),cover=new THREE.Object3D(),photo=new THREE.Object3D(),fixed=new THREE.Object3D();
 stand.name='stand';cover.name='cover';photo.name='photo';fixed.name='fixed';stand.position.set(0,-3,2);cover.position.set(1,4,8);photo.position.set(1,4,8);fixed.position.set(0,-9,0);cover.userData.inspectionCompanions=[photo];group.add(stand,cover,photo,fixed);
 const joints=createServoJoints(group,[{id:'pan',axis:[0,1,0],pivotMm:[0,-3,2],parts:['stand']},{id:'tilt',parent:'pan',axis:[1,0,0],pivotMm:[0,1,2],direction:-1,parts:['cover','photo']}]);
 group.updateMatrixWorld(true);return{group,cover,photo,fixed,joints};
}
test('inspection inherits live servo poses and reassembles at the current feedback pose without joint writes',()=>{
 const model=articulated(),reference=articulated(),parents=[model.cover.parent,model.photo.parent],local=model.cover.position.clone(),inspection=createModelInspection(model.group,[declaration],sources),offset=new THREE.Vector3(...declaration.offsetMm);
 for(const [pan,tilt,progress]of[[0,0,1],[72,38,1],[-45,65,.4],[130,-25,0],[-80,-70,1],[15,60,0]]){
  const feedback=[{id:'pan',positionDegrees:pan},{id:'tilt',positionDegrees:tilt}];applyServoValues(model.joints,feedback);applyServoValues(reference.joints,feedback);reference.group.updateMatrixWorld(true);inspection.setProgress(progress);
  for(let i=0;i<2;i++){assert.deepEqual(model.joints[i].quaternion.toArray(),reference.joints[i].quaternion.toArray());assert.equal(model.joints[i].userData.positionDegrees,[pan,tilt][i]);}
  const rotation=reference.joints[1].getWorldQuaternion(new THREE.Quaternion()),delta=offset.clone().applyQuaternion(rotation).multiplyScalar(progress);near(world(model.cover),world(reference.cover).add(delta));near(world(model.photo),world(reference.photo).add(delta));near(world(model.fixed),world(reference.fixed));assert.equal(model.cover.parent,parents[0]);assert.equal(model.photo.parent,parents[1]);
 }
 assert.deepEqual(model.cover.position.toArray(),local.toArray());
 inspection.setProgress(1);applyServoValues(model.joints,[{id:'pan',positionDegrees:35},{id:'tilt',positionDegrees:-40}]);applyServoValues(reference.joints,[{id:'pan',positionDegrees:35},{id:'tilt',positionDegrees:-40}]);model.group.updateMatrixWorld(true);reference.group.updateMatrixWorld(true);
 near(world(model.cover),world(reference.cover).add(offset.clone().applyQuaternion(reference.joints[1].getWorldQuaternion(new THREE.Quaternion()))));inspection.setProgress(0);near(world(model.cover),world(reference.cover));
 assert.throws(()=>createModelInspection(model.group,[{...declaration,parts:['servo:pan','cover']}],sources),/ownership/);
});
test('actual loader resolves articulated source, photo companion, paper and physical hit surface after servo reparenting',async()=>{
 const old={fetch:globalThis.fetch,load:THREE.TextureLoader.prototype.loadAsync},vertices=new Float32Array([-2,-2,0,0,0,1,2,-2,0,0,0,1,0,2,0,0,0,1]);
 const metadata={provenance:{sources},unit:'mm',strideFloats:6,binary:'geometry.bin',parts:[{name:'cover',role:'shell',byteOffset:0,vertexCount:3,texture:{file:'photo.png',projection:'xy',boundsMm:[-2,-2,2,2],side:'front'}}],derivedParts:[{id:'paper',kind:'label',texture:'paper.png',geometry:{sizeMm:[2,2],centerMm:[0,0,.01]}}],presentation:{buttons:[{id:'button',shape:'rectangle',sizeMm:[1,1],positionMm:[0,0,.02]}],servoJoints:[{id:'pan',axis:[0,1,0],pivotMm:[0,0,0],parts:['cover','cover-photo-surface','paper','cover-key:button']}],inspectionGroups:[{...declaration,parts:['cover','paper','cover-key:button']}]}},product={id:'nested-fixture',model:{url:'./assets/nested-fixture/model.json'},controls:[{id:'button',type:'button'}]};
 let model;try{
  globalThis.fetch=async url=>new Response(String(url).endsWith('model.json')?JSON.stringify(metadata):vertices.buffer);THREE.TextureLoader.prototype.loadAsync=async()=>new THREE.Texture();model=createManufacturerModel(product,null);await model.ready;
  assert.equal(model.group.userData.inspectionError,undefined);const {inspection,servoJoints}=model.group.userData,photo=model.group.getObjectByName('cover-photo-surface'),hit=model.parts['cover-key:button'];
  assert.equal(model.parts.cover.parent,servoJoints[0]);assert.equal(photo.parent,servoJoints[0]);const photoAt=photo.position.clone();
  inspection.setProgress(.5);near(photo.position,photoAt.clone().addScaledVector(new THREE.Vector3(...declaration.offsetMm),.5));applyServoValues(servoJoints,[{id:'pan',positionDegrees:67}]);model.group.updateMatrixWorld(true);
  const at=world(hit),normal=new THREE.Vector3(0,0,1).transformDirection(hit.matrixWorld),ray=new THREE.Raycaster(at.clone().addScaledVector(normal,100),normal.clone().negate());assert.equal(ray.intersectObject(model.group,true)[0]?.object?.name,hit.name);
  const pose=servoJoints[0].quaternion.clone();inspection.setProgress(0);assert.deepEqual(photo.position.toArray(),photoAt.toArray());assert.deepEqual(servoJoints[0].quaternion.toArray(),pose.toArray());assert.deepEqual([...photo.geometry.getAttribute('position').data.array],[...vertices]);
 }finally{model?.dispose();globalThis.fetch=old.fetch;THREE.TextureLoader.prototype.loadAsync=old.load;}
});
