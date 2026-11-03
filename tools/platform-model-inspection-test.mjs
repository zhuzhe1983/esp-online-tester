import test from 'node:test';
import assert from 'node:assert/strict';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createModelInspection} from '../web/platform/model-inspection.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';

const sources=[{id:'source-cover'}];
const definition = {id:'cover',parts:['cover','paper','cap'],offsetMm:[4,-2,12],sourceIds:['source-cover'],notes:'Source-declared cover group; viewing clearance only.'};
function assembly() {
  const group=new THREE.Group();group.rotation.set(.2,-.3,.4);
  for(const name of ['cover','paper','cap','pcb']) {
    const part=new THREE.Mesh(new THREE.BoxGeometry(2,2,.1),new THREE.MeshBasicMaterial());part.name=name;part.position.set(2,-3,5);part.rotation.set(.1,.2,.3);group.add(part);
  }
  return group;
}
test('declared inspection offsets use model axes, preserve transforms/materials and reset without accumulation',()=>{
  const group=assembly(),base=group.children.map(p=>({p,position:p.position.clone(),quaternion:p.quaternion.clone(),material:p.material}));
  const inspection=createModelInspection(group,[definition],sources);assert.equal(inspection.expanded,false);
  for(let repeat=0;repeat<30;repeat++) {
    inspection.setExpanded(true);inspection.setExpanded(true);
    for(const {p,position,quaternion,material} of base) {
      const expected=position.clone();if(p.name!=='pcb')expected.add(new THREE.Vector3(...definition.offsetMm));
      assert.deepEqual(p.position.toArray(),expected.toArray());assert.deepEqual(p.quaternion.toArray(),quaternion.toArray());assert.equal(p.material,material);
      assert.ok(p.getWorldPosition(new THREE.Vector3()).distanceTo(group.localToWorld(expected.clone()))<1e-10);
    }
    inspection.setExpanded(false);inspection.setExpanded(false);
    for(const {p,position} of base)assert.deepEqual(p.position.toArray(),position.toArray());
  }
  assert.throws(()=>inspection.setExpanded('yes'),/boolean/);assert.equal(inspection.expanded,false);
});

test('invalid inspection definitions reject atomically, including duplicate or nested ownership',()=>{
  const cases=[null,{},[null],[{...definition,offsetMm:[0,0,0]}],[{...definition,offsetMm:[0,NaN,1]}],[{...definition,offsetMm:[0,0,5001]}],
    [{...definition,parts:['absent']}],[{...definition,parts:['cover','cover']}],[definition,definition],
    [definition,{...definition,id:'second',parts:['paper']}],[{...definition,sourceIds:[]}],[{...definition,notes:''}],
    [definition,{...definition,id:'second',parts:['absent']}]];
  for(const definitions of cases){const group=assembly(),before=group.children.map(p=>p.position.toArray());assert.throws(()=>createModelInspection(group,definitions,sources),/Invalid/);assert.deepEqual(group.children.map(p=>p.position.toArray()),before);}
  const group=assembly(),nested=new THREE.Object3D();nested.name='nested';group.children[0].add(nested);
  assert.throws(()=>createModelInspection(group,[{...definition,parts:['nested']}],sources),/ownership/);
  assert.equal(createModelInspection(group,undefined),null);assert.equal(createModelInspection(group,[]),null);
});

test('loader moves actual projected photo twin, paper and control hit mesh with the cover only',async()=>{
  const fetch=globalThis.fetch,load=THREE.TextureLoader.prototype.loadAsync;
  const vertices=new Float32Array([-2,-2,0,0,0,1,2,-2,0,0,0,1,0,2,0,0,0,1]);
  const product={id:'inspection-fixture',model:{url:'./assets/inspection-fixture/model.json'},controls:[{id:'button',type:'button'}]};
  const metadata={provenance:{sources},unit:'mm',strideFloats:6,binary:'geometry.bin',parts:[{name:'cover',role:'shell',byteOffset:0,vertexCount:3,texture:{file:'photo.png',projection:'xy',boundsMm:[-2,-2,2,2],side:'front'}}],
    derivedParts:[{id:'paper',kind:'label',texture:'paper.png',geometry:{sizeMm:[2,2],centerMm:[0,0,.01]}},{id:'pcb',kind:'box',geometry:{sizeMm:[4,4,.1],centerMm:[0,0,-1]}}],
    presentation:{buttons:[{id:'button',shape:'rectangle',sizeMm:[1,1],positionMm:[0,0,.02]}],inspectionGroups:[{...definition,parts:['cover','paper','cover-key:button']}]} };
  let model;
  try {
    THREE.TextureLoader.prototype.loadAsync=async()=>new THREE.Texture();globalThis.fetch=async url=>new Response(String(url).endsWith('model.json')?JSON.stringify(metadata):vertices.buffer);
    model=createManufacturerModel(product,null);await model.ready;const photo=model.group.getObjectByName('cover-photo-surface'),hit=model.parts['cover-key:button'],board=model.parts.pcb;
    const base=[photo,hit,model.parts.paper,board].map(p=>p.position.clone());model.group.userData.inspection.setExpanded(true);
    assert.deepEqual(photo.position.toArray(),definition.offsetMm);assert.deepEqual(board.position.toArray(),base[3].toArray());assert.equal(hit.userData.control,'button');assert.equal(model.buttons.length,1);
    const ray=new THREE.Raycaster(new THREE.Vector3(4,-2,100),new THREE.Vector3(0,0,-1));model.group.updateMatrixWorld(true);
    assert.equal(ray.intersectObject(model.group,true)[0].object,hit,'physical hit surface follows the separated cap');
    model.group.userData.inspection.setExpanded(false);[photo,hit,model.parts.paper,board].forEach((p,i)=>assert.deepEqual(p.position.toArray(),base[i].toArray()));
    assert.deepEqual([...photo.geometry.getAttribute('position').data.array],[...vertices],'inspection never modifies projected source vertices');
  } finally {model?.dispose();globalThis.fetch=fetch;THREE.TextureLoader.prototype.loadAsync=load;}
});

test('malformed optional inspection keeps assembled model available and cannot invent product controls',async()=>{
  const fetch=globalThis.fetch,warn=console.warn,vertices=new Float32Array([-1,-1,0,0,0,1,1,-1,0,0,0,1,0,1,0,0,0,1]);let model;
  try {
    const metadata={provenance:{sources},unit:'mm',strideFloats:6,binary:'geometry.bin',parts:[{name:'cover',byteOffset:0,vertexCount:3}],presentation:{inspectionGroups:[{...definition,parts:['cover','cover-key:unknown']}]}};
    globalThis.fetch=async url=>new Response(String(url).endsWith('model.json')?JSON.stringify(metadata):vertices.buffer);const warnings=[];console.warn=value=>warnings.push(value);
    model=createManufacturerModel({id:'fixture',model:{url:'./assets/fixture/model.json'},controls:[]},null);await model.ready;
    assert.equal(model.group.userData.inspection,undefined);assert.match(model.group.userData.inspectionError,/ownership/);assert.equal(warnings.length,1);assert.deepEqual(model.buttons,[]);assert.deepEqual(model.parts.cover.position.toArray(),[0,0,0]);
  } finally {model?.dispose();globalThis.fetch=fetch;console.warn=warn;}
});
