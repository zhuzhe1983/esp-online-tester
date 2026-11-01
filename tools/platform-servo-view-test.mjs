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
  const nativeFetch = globalThis.fetch;
  try { globalThis.fetch = async url => new Response(String(url).includes('model.json') ? JSON.stringify(metadata) : binary); const model=createManufacturerModel(product,null);await model.ready;return model; }
  finally { globalThis.fetch=nativeFetch; }
}
function dispose(model) { model.dispose();model.group.traverse(object=>{object.geometry?.dispose();for(const material of [].concat(object.material || []))material.dispose();}); }

test('StackChan retains neutral CAD coordinates and moves head, display and controls with nested feedback joints',async()=>{
 const model=await load();try {
  const {group,parts}=model,joints=group.userData.servoJoints;group.updateMatrixWorld(true);
  assert.equal(joints.length,2);assert.equal(joints[1].parent,joints[0]);
  near(world(parts['shell-main']),new THREE.Vector3());near(world(parts['servo-shell']),new THREE.Vector3());near(world(parts.screen),new THREE.Vector3(0,2,23.5));
  const fixed=world(parts['shell-base']),screen0=world(parts.screen),glass0=world(parts['front-glass']);
  applyServoValues(joints,[{id:'pan',positionDegrees:90,targetDegrees:0},{id:'tilt',positionDegrees:45}]);group.updateMatrixWorld(true);
  near(world(parts['shell-base']),fixed);assert.ok(world(parts.screen).distanceTo(screen0)>20);
  const pose=world(parts.screen);applyServoValues(joints,[{id:'pan',positionDegrees:90,targetDegrees:-90,enabled:false},{id:'tilt',positionDegrees:45,enabled:false}]);group.updateMatrixWorld(true);near(world(parts.screen),pose);
  for(const name of ['front-glass','pcb-main','usb-c','grove-a','live-display','cover-key:touch-pad-front','rgb-window-left-0']) assert.equal(group.getObjectByName(name).parent,joints[1],`${name} must follow tilt`);
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
   const center=world(mesh),sign=index<6?-1:1,ray=new THREE.Raycaster(center.clone().add(new THREE.Vector3(sign*30,0,0)),new THREE.Vector3(-sign,0,0));
   assert.equal(pickVisibleSurface(ray,model.group)?.object,mesh,`RGB ${index} is in a visible light-guide opening`);
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
