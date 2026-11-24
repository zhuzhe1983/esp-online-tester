#!/usr/bin/env node
// Source-independent geometric checks on the actual production screen builder.
import assert from 'node:assert/strict';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createManufacturerDisplayGeometry} from '../web/platform/manufacturer-model.js';

function sameGeometry(actual,expected){
  for(const key of ['position','normal','uv'])assert.deepEqual(actual.getAttribute(key).array,expected.getAttribute(key).array);
  assert.deepEqual(actual.index.array,expected.index.array);actual.dispose();expected.dispose();
}
sameGeometry(createManufacturerDisplayGeometry({sizeMm:[40,60]}),new THREE.PlaneGeometry(40,60));
sameGeometry(createManufacturerDisplayGeometry({sizeMm:[40,60],cornerRadiusMm:0}),new THREE.PlaneGeometry(40,60));
sameGeometry(createManufacturerDisplayGeometry({shape:'circle',sizeMm:[32.4,32.4]}),new THREE.CircleGeometry(16.2,96));
for(const radius of [-1,NaN,Infinity,8.7,'2.3'])assert.throws(()=>createManufacturerDisplayGeometry({sizeMm:[17.39,32.35],cornerRadiusMm:radius}),/Invalid rounded display/);
// Float32 position rounding can lie a few nanometres outside the exact double
// width; UVs must still remain within the texture, including for other sizes.
const fractional=createManufacturerDisplayGeometry({sizeMm:[36.7,51.7],cornerRadiusMm:2.4});
assert.ok([...fractional.getAttribute('uv').array].every(v=>v>=0&&v<=1));fractional.dispose();

// C5 LCD1.47's manufacturer PDF: 17.39 x 32.35 mm AA, R2.30.
const width=17.39,height=32.35,radius=2.3,geometry=createManufacturerDisplayGeometry({shape:'rounded-rectangle',sizeMm:[width,height],cornerRadiusMm:radius});
geometry.computeBoundingBox();const size=geometry.boundingBox.getSize(new THREE.Vector3());
assert.ok(Math.abs(size.x-width)<2e-6&&Math.abs(size.y-height)<2e-6&&size.z===0);
const positions=geometry.getAttribute('position'),uv=geometry.getAttribute('uv');
for(let i=0;i<positions.count;i++){
  assert.ok(uv.getX(i)>=0&&uv.getX(i)<=1&&uv.getY(i)>=0&&uv.getY(i)<=1);
  assert.ok(Math.abs(uv.getX(i)-(positions.getX(i)/width+.5))<1e-6);
  assert.ok(Math.abs(uv.getY(i)-(positions.getY(i)/height+.5))<1e-6);
}
const mesh=new THREE.Mesh(geometry,new THREE.MeshBasicMaterial());mesh.updateMatrixWorld(true);
const hit=(x,y)=>new THREE.Raycaster(new THREE.Vector3(x,y,10),new THREE.Vector3(0,0,-1)).intersectObject(mesh)[0];
for(const [x,y]of [[0,0],[0,height/2-.01],[width/2-.01,0],[width/2-radius+.5,height/2-radius+.5]]){
  const intersection=hit(x,y);assert.ok(intersection);
  assert.ok(Math.abs(intersection.uv.x-(x/width+.5))<1e-6&&Math.abs(intersection.uv.y-(y/height+.5))<1e-6);
}
for(const sx of [-1,1])for(const sy of [-1,1]){
  assert.equal(hit(sx*(width/2-.05),sy*(height/2-.05)),undefined);
  assert.ok(hit(sx*(width/2-radius+radius*.7),sy*(height/2-radius+radius*.7)));
}
const points=[new THREE.Vector3(),new THREE.Vector3(),new THREE.Vector3()],index=geometry.index;let area=0;
for(let i=0;i<index.count;i+=3){for(let j=0;j<3;j++)points[j].fromBufferAttribute(positions,index.getX(i+j));area+=points[1].clone().sub(points[0]).cross(points[2].clone().sub(points[0])).length()/2;}
const analytic=width*height-(4-Math.PI)*radius*radius;assert.ok(Math.abs(area-analytic)<.03);
console.log(JSON.stringify({status:'pass',rectangleAndCircle:'unchanged',roundedSizeMm:[size.x,size.y],cornerRadiusMm:radius,triangulatedAreaMm2:area,analyticAreaMm2:analytic,uv:'normalized rectangle; corners clipped; ray interpolation matches pixel coordinates',scope:'Actual production geometry and CPU rays; no WebGL or physical display calibration.'},null,2));
geometry.dispose();mesh.material.dispose();
