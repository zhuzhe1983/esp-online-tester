import test from 'node:test';
import assert from 'node:assert/strict';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createPlateGeometry} from '../web/platform/plate-geometry.js';
import {createPcbPlaceholderGeometry} from '../web/platform/pcb-placeholder.js';

const outlineMm=[[-5,-4],[5,-4],[5,4],[1,4],[1,2],[-1,2],[-1,4],[-5,4]];
const hole=[[-3,-1],[-2,-1],[-2,1],[-3,1]];
const spec={outlineMm,holesMm:[hole],depthMm:1};
function hit(mesh,x,y,back=false) {
  return new THREE.Raycaster(new THREE.Vector3(x,y,back?-10:10),new THREE.Vector3(0,0,back?1:-1)).intersectObject(mesh).length>0;
}
test('explicit outline cuts a notch and a through-hole in both plate faces',()=>{
  const geometry=createPlateGeometry(spec),mesh=new THREE.Mesh(geometry,new THREE.MeshBasicMaterial());
  geometry.computeBoundingBox();assert.deepEqual(geometry.boundingBox.min.toArray(),[-5,-4,-.5]);assert.deepEqual(geometry.boundingBox.max.toArray(),[5,4,.5]);
  for(const back of [false,true]) {assert.equal(hit(mesh,3,0,back),true);assert.equal(hit(mesh,0,3,back),false);assert.equal(hit(mesh,-2.5,0,back),false);assert.equal(hit(mesh,0,0,back),true);}
  let area=0;const points=geometry.attributes.position;
  for(let i=0;i<points.count;i+=3)if([i,i+1,i+2].every(n=>Math.abs(points.getZ(n)-.5)<1e-6)) {
    const a=new THREE.Vector3().fromBufferAttribute(points,i),b=new THREE.Vector3().fromBufferAttribute(points,i+1),c=new THREE.Vector3().fromBufferAttribute(points,i+2);
    area+=b.sub(a).cross(c.sub(a)).length()/2;
  }
  assert.ok(Math.abs(area-(80-4-2))<1e-6,'top area excludes the independent 4 mm² notch and 2 mm² hole');
  geometry.dispose();mesh.material.dispose();
});
test('ring winding does not fill holes or invert outer faces',()=>{
  const geometry=createPlateGeometry({...spec,outlineMm:[...outlineMm].reverse(),holesMm:[[...hole].reverse()]}),mesh=new THREE.Mesh(geometry,new THREE.MeshBasicMaterial());
  assert.equal(hit(mesh,0,0),true);assert.equal(hit(mesh,0,0,true),true);assert.equal(hit(mesh,-2.5,0),false);geometry.dispose();mesh.material.dispose();
});
test('malformed, crossing, touching and nested plate contours are rejected',()=>{
  for(const replacement of [null,[],[[0,0],[1,0],[0,0]],[[0,0],[1,1],[0,1],[1,0]],[[0,0],[1,0],[2,0]],[[0,0],[1,0],[NaN,1]]])assert.throws(()=>createPlateGeometry({...spec,outlineMm:replacement}),/Invalid plate outline/);
  for(const holesMm of [null,[hole,hole],[[[-6,0],[-4,0],[-4,1]]],[[[-5,0],[-4,0],[-4,1]]],[[[-4,-2],[0,-2],[0,2],[-4,2]],hole]])assert.throws(()=>createPlateGeometry({...spec,holesMm}),/Invalid plate holes/);
  for(const depthMm of [0,-1,NaN,'1',5001])assert.throws(()=>createPlateGeometry({...spec,depthMm}),/Invalid plate depth/);
});
test('PCB polygon stays within declared size and legacy envelopes remain available',()=>{
  const board=createPcbPlaceholderGeometry([10,8,1],'polygon',spec);board.computeBoundingBox();assert.equal(board.boundingBox.max.z,.5);board.dispose();
  assert.throws(()=>createPcbPlaceholderGeometry([9,8,1],'polygon',spec),/exceeds declared dimensions/);
  for(const shape of ['box','disc','capsule'])createPcbPlaceholderGeometry([10,10,1],shape).dispose();
});
