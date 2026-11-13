import test from 'node:test';
import assert from 'node:assert/strict';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createStickerGeometry} from '../web/platform/sticker-geometry.js';
import {readStickers,saveStickers,validateStickerCollection,STICKER_LIMITS} from '../web/platform/sticker-store.js';
import {applyShellAppearance} from '../web/platform/shell-appearance.js';
import {SHELL_PRESETS} from '../web/platform/appearance.js';

const image=()=>({id:'one',name:'Image.png',blob:new Blob([new Uint8Array(32)],{type:'image/png'}),aspect:1,size:.4,rotation:0,x:0,y:0,anchor:{part:'shell',point:[0,0,1],normal:[0,0,1]}});
test('decals clip to real shell triangles without filling gaps or painting the reverse face',()=>{
  const source=new THREE.BoxGeometry(20,20,2),mesh=new THREE.Mesh(source),sticker=image(),geometry=createStickerGeometry(mesh,sticker,20),p=geometry.getAttribute('position'),uv=geometry.getAttribute('uv');
  assert.ok(p.count>=6);for(let i=0;i<p.count;i++){assert.ok(Math.abs(p.getZ(i)-1)<1e-6);assert.ok(Math.abs(p.getX(i))<=4.001);assert.ok(Math.abs(p.getY(i))<=4.001);assert.ok(uv.getX(i)>=0&&uv.getX(i)<=1&&uv.getY(i)>=0&&uv.getY(i)<=1);}
  // Two disconnected pieces with a two-millimetre gap: no generated triangle may bridge it.
  const split=new THREE.BufferGeometry();split.setAttribute('position',new THREE.Float32BufferAttribute([-8,-8,1,-1,-8,1,-1,8,1,-8,-8,1,-1,8,1,-8,8,1,1,-8,1,8,-8,1,8,8,1,1,-8,1,8,8,1,1,8,1],3));split.computeVertexNormals();
  const clipped=createStickerGeometry(new THREE.Mesh(split),{...sticker,size:1},20).getAttribute('position');
  assert.ok(clipped.count>0);for(let i=0;i<clipped.count;i+=3){const xs=[0,1,2].map(j=>clipped.getX(i+j));assert.ok(xs.every(x=>x<=-1)||xs.every(x=>x>=1));}
  assert.equal(mesh.geometry,source,'Projection must not rewrite manufacturer geometry');geometry.dispose();source.dispose();split.dispose();
});
test('decal rotation and offset change the projected footprint on the same surface',()=>{
  const mesh=new THREE.Mesh(new THREE.PlaneGeometry(100,100));const item={...image(),aspect:2,anchor:{part:'shell',point:[0,0,0],normal:[0,0,1]}};
  const a=createStickerGeometry(mesh,item,100),b=createStickerGeometry(mesh,{...item,rotation:90,x:.1},100);a.computeBoundingBox();b.computeBoundingBox();
  assert.deepEqual(a.boundingBox.getSize(new THREE.Vector3()).toArray(),[40,20,0]);assert.deepEqual(b.boundingBox.getSize(new THREE.Vector3()).toArray().map(Math.round),[20,40,0]);assert.ok(Math.abs(b.boundingBox.getCenter(new THREE.Vector3()).y-10)<.001);
});
test('private image collections enforce limits and stay isolated by product when persistence is unavailable',async()=>{
  const first=image();assert.equal(validateStickerCollection([first])[0],first);
  for(const patch of [{size:2},{rotation:NaN},{blob:new Blob(['<svg/>'],{type:'image/svg+xml'})},{anchor:{...first.anchor,normal:[0,0,0]}}])assert.throws(()=>validateStickerCollection([{...first,...patch}]));
  assert.throws(()=>validateStickerCollection(Array.from({length:STICKER_LIMITS.count+1},(_,i)=>({...first,id:String(i)}))));
  assert.equal(await saveStickers('test-product-a',[first]),false);await saveStickers('test-product-b',[]);
  const restored=await readStickers('test-product-a');assert.equal(restored.items.length,1);assert.equal(restored.items[0].name,first.name);assert.equal((await readStickers('test-product-b')).items.length,0);assert.equal(restored.persistent,false);
});
test('titanium uses thin-film interference and resets without retaining iridescence',()=>{
  const mesh=new THREE.Mesh(new THREE.BoxGeometry(1,1,1),new THREE.MeshStandardMaterial());
  applyShellAppearance(mesh,{shell:SHELL_PRESETS['titanium-spectrum']});assert.equal(mesh.material.iridescence,1);assert.ok(mesh.material.metalness>.7);
  const shader={vertexShader:THREE.ShaderLib.physical.vertexShader,fragmentShader:THREE.ShaderLib.physical.fragmentShader};mesh.material.onBeforeCompile(shader);
  assert.match(shader.fragmentShader,/material\.iridescenceThickness = mix\(180\.0, 620\.0/);
  applyShellAppearance(mesh,{shell:SHELL_PRESETS.original});assert.equal(mesh.material.iridescence,0);assert.equal(mesh.material.metalness,0);mesh.material.dispose();
});
