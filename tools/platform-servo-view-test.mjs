import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import test from 'node:test';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {createServoJoints,applyServoValues} from '../web/platform/servo-view.js';
import {applyRgbLedValues} from '../web/platform/rgb-led-view.js';
import {pickVisibleSurface} from '../web/platform/scene-interaction.js';
const root = new URL('../web/platform/assets/m5stack-k151/',import.meta.url);
const metadata = JSON.parse(await fs.readFile(new URL('model.json',root)));
const binary = await fs.readFile(new URL('geometry.bin',root));
const product = {id:'m5stack-k151',model:{url:'./assets/m5stack-k151/model.json'},display:{width:320,height:240},controls:['front','middle','back'].map(id=>({id:`touch-pad-${id}`,type:'button'}))};
const near = (a,b) => assert.ok(a.distanceTo(b)<1e-6,`${a.toArray()} != ${b.toArray()}`);
const world = object => object.getWorldPosition(new THREE.Vector3());
async function load() {
  const nativeFetch = globalThis.fetch, nativeTextureLoad = THREE.TextureLoader.prototype.loadAsync;
  try { THREE.TextureLoader.prototype.loadAsync = async () => new THREE.Texture(); globalThis.fetch = async url => new Response(String(url).includes('model.json') ? JSON.stringify(metadata) : binary); const model=createManufacturerModel(product,null);await model.ready;return model; }
  finally { globalThis.fetch=nativeFetch; THREE.TextureLoader.prototype.loadAsync=nativeTextureLoad; }
}
function dispose(model) { model.dispose();model.group.traverse(object=>{object.geometry?.dispose();for(const material of [].concat(object.material || []))material.dispose();}); }

test('StackChan retains neutral CAD coordinates and moves head, display and controls with nested feedback joints',async()=>{
 const model=await load();try {
  const {group,parts}=model,joints=group.userData.servoJoints;group.updateMatrixWorld(true);
  assert.equal(joints.length,2);assert.equal(joints[1].parent,joints[0]);
  near(world(parts['shell-main']),new THREE.Vector3());near(world(parts['servo-shell']),new THREE.Vector3());near(world(parts.screen),new THREE.Vector3(0,0,38.2532019043));
  const fixed=world(parts['shell-base']),screen0=world(parts.screen),glass0=world(parts['front-glass']);
  applyServoValues(joints,[{id:'pan',positionDegrees:90,targetDegrees:0},{id:'tilt',positionDegrees:45}]);group.updateMatrixWorld(true);
  near(world(parts['shell-base']),fixed);assert.ok(world(parts.screen).distanceTo(screen0)>20);
  const pose=world(parts.screen);applyServoValues(joints,[{id:'pan',positionDegrees:90,targetDegrees:-90,enabled:false},{id:'tilt',positionDegrees:45,enabled:false}]);group.updateMatrixWorld(true);near(world(parts.screen),pose);
  for(const name of ['shell-front','front-glass','camera-lens','pcb-main','pcb-power','pcb-touch','grove-b','grove-c','label-side-left','label-cores3-top','light-guide-left','usb-c','grove-a','live-display','cover-key:touch-pad-front','rgb-window-left-0']) assert.equal(group.getObjectByName(name).parent,joints[1],`${name} must follow tilt`);
  for(const name of ['servo-pan-body','servo-tilt-body','servo-pan-gear','servo-tilt-gear']) assert.equal(group.getObjectByName(name).parent,joints[0],`${name} stays fixed to the moving stand`);
  for(const name of ['servo-arm','bearing-fixture','servo-tilt-output']) assert.equal(group.getObjectByName(name).parent,joints[1]);
  assert.equal(group.getObjectByName('servo-pan-output').parent,group,'Pan output shaft is fixed to the base');
  const normal=new THREE.Vector3(0,0,1).transformDirection(parts.screen.matrixWorld),ray=new THREE.Raycaster(pose.clone().addScaledVector(normal,100),normal.clone().negate());
  assert.equal(pickVisibleSurface(ray,group)?.object,parts.screen,'Moved framebuffer remains the nearest visible touch surface');
  for(let i=0;i<10;i++){applyServoValues(joints,[]);applyServoValues(joints,[{id:'pan',positionDegrees:-70},{id:'tilt',positionDegrees:80}]);}applyServoValues(joints,[]);group.updateMatrixWorld(true);
  near(world(parts.screen),screen0);near(world(parts['front-glass']),glass0);
  applyServoValues(joints,[{id:'pan',positionDegrees:NaN,targetDegrees:50}]);group.updateMatrixWorld(true);near(world(parts.screen),screen0);
 } finally {dispose(model);}
});

test('Top pad hit regions are independently visible, while lower shell occludes them from below',async()=>{
 const model=await load();try {
  for(const [id,z] of [['front',12],['middle',0],['back',-12]]) {
   const ray=new THREE.Raycaster(new THREE.Vector3(0,100,z),new THREE.Vector3(0,-1,0));
   assert.equal(pickVisibleSurface(ray,model.group)?.object.userData.control,`touch-pad-${id}`);
   ray.set(new THREE.Vector3(0,-100,z),new THREE.Vector3(0,1,0));assert.notEqual(pickVisibleSurface(ray,model.group)?.object.userData.control,`touch-pad-${id}`);
  }
 }finally{dispose(model);}
});

test('Twelve optical cells show only supplied RGB chain values and remain independent of shell materials',async()=>{
 const model=await load();try {
  const meshes=model.group.userData.rgbLedMeshes;assert.equal(meshes.length,12);
  for(const [index,mesh] of meshes.entries()) {
   assert.equal(mesh.userData.ledIndex,index);assert.ok(!model.group.userData.shellMeshes.includes(mesh));
   const attribute=mesh.geometry.getAttribute('position');let largest=0,center,normal;const a=new THREE.Vector3(),b=new THREE.Vector3(),c=new THREE.Vector3();
   for(let i=0;i<attribute.count;i+=3){a.fromBufferAttribute(attribute,i);b.fromBufferAttribute(attribute,i+1);c.fromBufferAttribute(attribute,i+2);const cross=new THREE.Vector3().subVectors(b,a).cross(new THREE.Vector3().subVectors(c,a));if(cross.length()>largest){largest=cross.length();center=a.clone().add(b).add(c).multiplyScalar(1/3);normal=cross.normalize();}}
   center=mesh.localToWorld(center);normal.transformDirection(mesh.matrixWorld);
   for(const outward of [normal,new THREE.Vector3(index<6?-1:1,0,0),new THREE.Vector3(0,1,0),normal.clone().add(new THREE.Vector3(0,0,.65)).normalize()]) {
    const ray=new THREE.Raycaster(center.clone().addScaledVector(outward,30),outward.clone().negate());assert.equal(pickVisibleSurface(ray,model.group)?.object,mesh,`RGB ${index} retains its actual CAD emitting surface in an oblique opening view`);
   }
  }
  const values=meshes.map((_,i)=>[i*20,0,255-i*20]);applyRgbLedValues(meshes,values);assert.deepEqual(meshes.map(m=>m.userData.rgb),values);
  applyRgbLedValues(meshes,[]);assert.deepEqual(meshes.map(m=>m.userData.rgb),Array.from({length:12},()=>[0,0,0]));
 }finally{dispose(model);}
});

test('Invalid or ambiguous articulation declarations fail before claiming duplicate parts',()=>{
 const base={id:'pan',axis:[0,1,0],pivotMm:[0,0,0],parts:['body']};
 for(const definition of [null,{...base,axis:[0,0,0]},{...base,axis:[1.7e308,1.7e308,1.7e308]},{...base,pivotMm:[0,Infinity,0]},{...base,direction:0},{...base,parent:'missing'},{...base,parts:['missing']},{...base,parts:['body','body']}]) {
  const group=new THREE.Group(),body=new THREE.Object3D();body.name='body';group.add(body);assert.throws(()=>createServoJoints(group,[definition]),/Invalid servo|Servo parent/);
 }
});

test('Joint membership cannot reparent an ancestor into its descendant',()=>{
 const group=new THREE.Group(),body=new THREE.Object3D();body.name='body';group.add(body);
 assert.throws(()=>createServoJoints(group,[{id:'pan',axis:[0,1,0],pivotMm:[0,0,0],parts:['body']},{id:'tilt',parent:'pan',axis:[1,0,0],pivotMm:[0,0,0],parts:['servo:pan']}]),/Invalid servo part membership/);
});

test('StackChan assembled envelope and CoreS3 optical landmarks match independent official dimensions',async()=>{
 const model=await load();try {
  const {group,parts}=model;group.updateMatrixWorld(true);
  // K151 Model_Size.png:54 ×70.5mm; head depth61.5mm. Glass has a declared0.05mm presentation offset.
  const bounds=names=>names.reduce((box,name)=>box.union(new THREE.Box3().setFromObject(parts[name])),new THREE.Box3());
  const head=bounds(['shell-main','shell-front']).getSize(new THREE.Vector3()),height=bounds(['shell-main','shell-front','shell-base']).getSize(new THREE.Vector3()).y;
  assert.ok(Math.abs(head.x-54)<.05);assert.ok(Math.abs(head.z-61.5)<.05);assert.ok(Math.abs(height-70.5)<.05);
  near(world(parts.screen),new THREE.Vector3(0,0,38.2532019043));
  // CoreS3 DXF optical centers, relative to the LCD center (not to model-generated reference points).
  for(const [name,x,y] of [['camera-lens',0,-21],['microphone-left',-14,-20.25],['microphone-right',14,-20.25],['ltr-window-1',6.58,-21],['ltr-window-2',8.98,-21]]) {
   const position=world(group.getObjectByName(name));assert.ok(Math.abs(position.x-x)<1e-6);assert.ok(Math.abs(position.y-y)<1e-6);
  }
  assert.equal(metadata.derivedParts.filter(part=>part.kind==='pcb-placeholder').length,5,'All five board layers retain explicit UNKNOWN geometry');
  for(const name of ['label-base','label-side-left','label-side-right','label-cores3-top']) assert.ok(!group.userData.shellMeshes.includes(group.getObjectByName(name)),'Paper labels keep their own material');
 }finally{dispose(model);}
});


test('RGB output partitions the original curved light-guide crowns without overlay boxes or added surface area',async()=>{
 const model=await load();try {
  const meshArea=mesh=>{let sum=0;const p=mesh.geometry.getAttribute('position'),a=new THREE.Vector3(),b=new THREE.Vector3(),c=new THREE.Vector3();for(let i=0;i<p.count;i+=3){a.fromBufferAttribute(p,i);b.fromBufferAttribute(p,i+1);c.fromBufferAttribute(p,i+2);sum+=b.sub(a).cross(c.sub(a)).length()/2;}return sum;};
  assert.equal(metadata.derivedParts.filter(part=>part.ledIndex!==undefined).length,0,'No extra solid light windows may protrude through the shell');
  for(const side of ['left','right']) {
   const rows=metadata.parts.filter(part=>part.name.startsWith(`rgb-window-${side}-`));assert.equal(rows.length,6);
   const area=rows.reduce((sum,part)=>sum+meshArea(model.parts[part.name]),0);assert.ok(Math.abs(area-59.841491690158996)<1e-4,'Union of six zones keeps the independently measured original crown area');
   for(const row of rows){assert.equal(row.sourcePlaneClip.axis,1);assert.ok(row.sourcePlaneClip.minMm>=-15.5&&row.sourcePlaneClip.maxMm<=15.5);}
  }
  const board=metadata.derivedParts.find(part=>part.id==='pcb-touch').geometry;
  for(const part of metadata.derivedParts.filter(part=>part.id.startsWith('led-package-'))) {
   const [x,y,z]=part.geometry.centerMm,[w,h,d]=part.geometry.sizeMm;
   assert.ok(Math.abs(x)+w/2<=board.sizeMm[0]/2);assert.ok(Math.abs(z-board.centerMm[2])+d/2<=board.sizeMm[1]/2);
   assert.ok(Math.abs(y-h/2-(board.centerMm[1]+board.sizeMm[2]/2))<1e-8,'Estimated package underside sits on the estimated board top');
  }
 }finally{dispose(model);}
});
