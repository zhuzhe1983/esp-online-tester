import test from 'node:test';
import assert from 'node:assert/strict';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {frameAssembly} from '../web/platform/viewer-camera.js';
import {createPinchGesture} from '../web/platform/viewer-gestures.js';

function projected(root,camera){
  const box=new THREE.Box2(),point=new THREE.Vector3();
  root.traverseVisible(mesh=>{if(!mesh.isMesh)return;const positions=mesh.geometry.getAttribute('position');for(let i=0;i<positions.count;i++){point.fromBufferAttribute(positions,i).applyMatrix4(mesh.matrixWorld).project(camera);box.expandByPoint(new THREE.Vector2(point.x,point.y));}});
  return box;
}
test('asymmetric CAD stays centred and fits portrait, landscape and immersive viewports after rotation',()=>{
  const assembly=new THREE.Group(),body=new THREE.Mesh(new THREE.BoxGeometry(68,121,7),new THREE.MeshBasicMaterial());body.position.set(72,-35,11);assembly.add(body);
  for(const aspect of [.46,1,2.2])for(const fill of [.86,.92])for(const angles of [[-.13,-.25,.025],[.3,1.3,.1],[-.4,3.4,.3]]){
    assembly.rotation.set(...angles);const camera=new THREE.PerspectiveCamera(32,aspect,2,3000);frameAssembly(camera,assembly,{fill});
    const box=projected(assembly,camera),center=box.getCenter(new THREE.Vector2()),size=box.getSize(new THREE.Vector2());
    assert.ok(center.length()<1e-6,`centred for aspect ${aspect}: ${center.toArray()}`);
    assert.ok(Math.max(size.x,size.y)<=fill*2+.03);
    assert.ok(Math.max(size.x,size.y)>fill*1.75,'model fills the limiting screen dimension');
    assert.ok(camera.position.z>2);
  }
});
test('hidden helpers do not shrink or offset the device; zoom preserves projected centring',()=>{
  const root=new THREE.Group(),mesh=new THREE.Mesh(new THREE.BoxGeometry(50,70,6),new THREE.MeshBasicMaterial());root.add(mesh);
  const camera=new THREE.PerspectiveCamera(32,1.5,2,3000);frameAssembly(camera,root);const initial=camera.position.clone();
  const hidden=new THREE.Group();hidden.visible=false;const helper=new THREE.Mesh(new THREE.BoxGeometry(1000,1000,1000),new THREE.MeshBasicMaterial());hidden.position.x=-900;hidden.add(helper);root.add(hidden);frameAssembly(camera,root);assert.deepEqual(camera.position.toArray(),initial.toArray());
  const before=projected(root,camera).getSize(new THREE.Vector2());frameAssembly(camera,root,{zoom:.75});const after=projected(root,camera);assert.ok(after.getSize(new THREE.Vector2()).y>before.y);assert.ok(after.getCenter(new THREE.Vector2()).length()<1e-6);
});

test('two fingers zoom after releasing guest input and do not resume a stale single-finger gesture',()=>{
  const calls=[],pinch=createPinchGesture({onStart:()=>calls.push('release'),onScale:value=>calls.push(value)});
  assert.equal(pinch.down(1,0,0),false);assert.equal(pinch.move(1,5,0),false);
  assert.equal(pinch.down(2,105,0),true);assert.deepEqual(calls,['release']);
  assert.equal(pinch.move(2,205,0),true);assert.deepEqual(calls,['release',2]);
  assert.equal(pinch.up(2),true);assert.equal(pinch.move(1,20,0),true);assert.equal(calls.length,2);
  assert.equal(pinch.up(1),true);assert.equal(pinch.active,false);assert.equal(pinch.down(3,0,0),false);
  pinch.down(4,0,0);pinch.move(4,0,0);assert.ok(calls.every(value=>value==='release'||Number.isFinite(value)));
  pinch.reset();assert.equal(pinch.active,false);assert.equal(pinch.move(4,40,0),false);
});
