import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {TextureLoader,Texture,Raycaster,Vector3} from '../web/platform/vendor/three.module.min.js';
import {chipForBoard} from '../web/platform/app-image.js';
const products=JSON.parse(await readFile(new URL('../web/platform/catalog/products.json',import.meta.url))).items;
const nativeFetch=globalThis.fetch,nativeLoad=TextureLoader.prototype.loadAsync;
try {
 globalThis.fetch=async url=>new Response(await readFile(fileURLToPath(new URL(url))));
 TextureLoader.prototype.loadAsync=async()=>new Texture();
 for(const id of ['m5stack-k123','m5stack-k124-b']) {
  const product=products.find(p=>p.id===id);assert.equal(chipForBoard(product.runtime.board),'esp32');
  const model=createManufacturerModel(product,null);await model.ready;model.group.updateMatrixWorld(true);
  assert.deepEqual(model.buttons.map(b=>b.userData.control),['button-a','button-b','button-c']);
  for(const [i,key] of model.buttons.entries()) {
   const xyz=key.getWorldPosition(new Vector3());assert.equal(xyz.x,(i-1)*10);
   const bank=model.parts[id==='m5stack-k123'?'button-bank':'button-panel'];
   const hit=new Raycaster(new Vector3(xyz.x,xyz.y,20),new Vector3(0,0,-1)).intersectObject(bank)[0];
   assert.ok(hit&&Math.abs(hit.point.z-16.5)<.001,'Each hit region has a real native CAD key face beneath it');
  }
  const original=model.buttons.map(k=>k.position.toArray());const inspect=model.group.userData.inspection;
  inspect?.setProgress?.(1);inspect?.setProgress?.(0);assert.deepEqual(model.buttons.map(k=>k.position.toArray()),original);
  model.dispose();
 }
 const core=products.find(p=>p.id==='m5stack-k048'),model=createManufacturerModel(core,null);await model.ready;model.group.updateMatrixWorld(true);
 const usb=model.parts['usb-c'];const origin=usb.localToWorld(new Vector3(0,.9,2)),direction=new Vector3(0,0,-1).transformDirection(usb.matrixWorld);
 const ray=new Raycaster(origin,direction).intersectObject(usb,true)[0];assert.equal(ray.object.name,'usb-c-rear');assert.ok(usb.worldToLocal(ray.point).z<-3,'The opening is a real cavity rather than a black plate at the mouth');model.dispose();
 const vameter=products.find(p=>p.id==='m5stack-k136');assert.equal(vameter.controls.find(c=>c.id==='encoder-press').pin,2);assert.equal(vameter.runtime.board,'vameter');assert.equal(vameter.runtime.status,'partial');
 console.log('ok M5 classic audit: six source-CAD key faces, classic chip routing, recessed USB cavity, VAMeter partial board identity');
} finally {globalThis.fetch=nativeFetch;TextureLoader.prototype.loadAsync=nativeLoad;}
