import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {applyShellAppearance} from '../web/platform/shell-appearance.js';
import {applyRgbLedValues} from '../web/platform/rgb-led-view.js';

const root=new URL('../',import.meta.url),catalog=JSON.parse(await readFile(new URL('web/platform/catalog/products.json',root)));
const product=catalog.items.find(p=>p.id==='m5stack-c153-lite'),nativeFetch=globalThis.fetch,nativeTextureLoad=THREE.TextureLoader.prototype.loadAsync;
let model;
try {
  globalThis.fetch=async url=>new Response(await readFile(new URL(new URL(url).pathname,'file://')));
  // Texture pixels are checked in the headless browser; this test exercises real geometry and routing.
  THREE.TextureLoader.prototype.loadAsync=async()=>new THREE.Texture();
  model=createManufacturerModel(product,null);await model.ready;model.group.updateMatrixWorld(true);
  const physical=Object.values(model.parts).filter(p=>p?.isMesh&&p.name!=='paper-labels');
  const hit=(origin,direction,objects=physical)=>new THREE.Raycaster(new THREE.Vector3(...origin),new THREE.Vector3(...direction)).intersectObjects(objects,false)[0];
  for(const [x,y] of [[-27,0],[27,0],[0,47],[0,-46]]) {
    const first=hit([x,y,20],[0,0,-1]);assert.equal(first.object.name,'front-bezel','Front cover must hide the CAD mounting ledge');assert.ok(Math.abs(first.point.z-3.8)<1e-5);
  }
  assert.equal(hit([0,2.41,20],[0,0,-1]).object,model.screen,'Framebuffer must remain the foremost screen surface');
  const rear=hit([0,-32.05,-20],[0,0,1]);assert.equal(rear.object.name,'rear-battery-cover');assert.ok(Math.abs(rear.point.z+4.025)<1e-5);
  const led=hit([-50,10,0],[1,0,0]);assert.equal(led.object.name,'rgb-led-diffuser');assert.ok(Math.abs(led.point.x+30.515)<1e-5);
  assert.equal(model.group.userData.rgbLedMeshes[0],led.object,'Side diffuser must use the production guest-output index');
  for(const [y,id] of [[38,'button-a'],[26,'button-b']])assert.equal(hit([-50,y,0],[1,0,0]).object.userData.control,id,'Actual side key must be the nearest registered input surface');
  assert.equal(model.parts['power-button'].userData.control,undefined,'Power controller behavior is not invented');
  for(const rgb of [[255,0,0],[0,255,0],[0,255,255]]) {
    applyRgbLedValues(model.group.userData.rgbLedMeshes,[rgb]);assert.deepEqual(led.object.userData.rgb,rgb);assert.ok(led.object.material.emissive.toArray().some(v=>v>0));
    for(let i=0;i<3;i++)assert.equal(led.object.material.emissive.toArray()[i]>0,rgb[i]>0);
  }
  const fixed=['front-bezel','rear-battery-cover','button-a','button-b','power-button','paper-labels','rgb-led-diffuser'].map(id=>[id,model.parts[id].material.color.getHexString(),model.parts[id].material.opacity]);
  for(const shell of [{color:'#d6b24a',opacity:1,finish:'metal'},{color:'#8054b3',opacity:.35,finish:'plastic'}]) {
    for(const mesh of model.group.userData.shellMeshes)applyShellAppearance(mesh,{shell});
    for(const [id,color,opacity] of fixed){assert.equal(model.parts[id].material.color.getHexString(),color);assert.equal(model.parts[id].material.opacity,opacity);}
  }
  applyRgbLedValues(model.group.userData.rgbLedMeshes,[]);assert.deepEqual(led.object.userData.rgb,[0,0,0]);assert.deepEqual(led.object.material.emissive.toArray(),[0,0,0]);assert.equal(led.object.material.color.getHexString(),'bdc2bc');
  const bounds=new THREE.Box3();for(const mesh of physical){mesh.geometry.computeBoundingBox();bounds.union(mesh.geometry.boundingBox.clone().applyMatrix4(mesh.matrixWorld));}
  const size=bounds.getSize(new THREE.Vector3());assert.ok(Math.abs(size.x-62)<.05&&Math.abs(size.y-101)<.05&&Math.abs(size.z-8)<.05,'All visible physical meshes must stay within the drawing plus 0.05mm rendering tolerance');
  console.log(JSON.stringify({productId:product.id,frontCoverOcclusion:'pass',screenForeground:'pass',rearFacing:'pass',sideLedOutputRouting:'pass',sideButtonInputRouting:'pass',materialIsolation:'pass',assembledSizeMm:size.toArray()}));
} finally {
  model?.dispose();model?.group.traverse(o=>{o.geometry?.dispose();for(const m of [].concat(o.material||[]))m.dispose();});
  globalThis.fetch=nativeFetch;THREE.TextureLoader.prototype.loadAsync=nativeTextureLoad;
}
