import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {applyShellAppearance} from '../web/platform/shell-appearance.js';

const catalog=JSON.parse(await readFile(new URL('../web/platform/catalog/products.json',import.meta.url)));
const product=catalog.items.find(item=>item.id==='m5stack-k159-b');
const nativeFetch=globalThis.fetch,nativeTextureLoad=THREE.TextureLoader.prototype.loadAsync;
let model;
try {
  globalThis.fetch=async url=>new Response(await readFile(new URL(new URL(url).pathname,'file://')));
  THREE.TextureLoader.prototype.loadAsync=async()=>new THREE.Texture();
  model=createManufacturerModel(product,null);const metadata=await model.ready;model.group.updateMatrixWorld(true);
  const hit=(origin,direction)=>new THREE.Raycaster(new THREE.Vector3(...origin),new THREE.Vector3(...direction)).intersectObject(model.group,true)[0];
  for(const x of [-29.2,0,29.2])for(const y of [-47.8,4,55.8])assert.equal(hit([x,y,20],[0,0,-1]).object,model.screen,'The framebuffer must fill the opening without mounting tabs crossing it');
  for(const y of [-48.6,56.8])assert.equal(hit([0,y,20],[0,0,-1]).object.name,'display-black-border','The former top gap must be backed by the display border');
  assert.equal(hit([15.3,-21,-20],[0,0,1]).object.name,'esp32-shield-marking');
  assert.equal(hit([1.5,-20,-20],[0,0,1]).object.name,'m2-brass-insert');
  const insertHole=hit([0,-20,-20],[0,0,1]);assert.equal(insertHole.object.name,'m2-recess-interior');assert.ok(insertHole.point.z>-2.3,'The mounting insert has a recessed open center');
  assert.equal(hit([-9.85,-13.3,-20],[0,0,1]).object.name,'battery-connector-body');
  const cardMouth=hit([50,-39.15,-.8],[-1,0,0]);assert.equal(cardMouth.object.name,'microsd-back');assert.ok(cardMouth.point.x<31,'The microSD mouth remains recessed behind the shell opening');
  for(const [y,id] of [[-15,'button-a'],[-27,'button-b']])assert.equal(hit([-50,y,0],[1,0,0]).object.userData.control,id);
  assert.equal(model.parts['button-power'].userData.control,undefined,'Appearance changes must not invent a GPIO power key');
  const size=new THREE.Box3().setFromObject(model.group).getSize(new THREE.Vector3());
  for(const [axis,expected] of [['x',68.2],['y',121.27],['z',6.3]])assert.ok(Math.abs(size[axis]-expected)<.05,`${axis} matches the independent assembled dimension drawing`);
  assert.ok(Math.abs(metadata.presentation.screen.sizeMm[0]/metadata.presentation.screen.sizeMm[1]-540/960)<1e-8,'The screen preserves native framebuffer aspect');
  const fixed=['display-black-border','esp32-shield','battery-connector-body','m2-brass-insert','microsd-top','button-user-a','cad-connector-insert'].map(id=>[id,model.parts[id].material.color.getHexString(),model.parts[id].material.opacity]);
  for(const shell of [{color:'#d7ab52',opacity:1,finish:'metal'},{color:'#8456cf',opacity:.36,finish:'plastic'}]){
    for(const mesh of model.group.userData.shellMeshes)applyShellAppearance(mesh,{shell});
    for(const [id,color,opacity] of fixed){assert.equal(model.parts[id].material.color.getHexString(),color);assert.equal(model.parts[id].material.opacity,opacity);}
  }
  const before=model.group.children.map(part=>[part,part.position.clone()]);
  assert.ok(model.group.userData.inspection);model.group.userData.inspection.setExpanded(true);
  assert.equal(model.parts['case-back'].position.z,-18);assert.equal(model.parts['pcb-main'].position.z,-.6);
  model.group.userData.inspection.setExpanded(false);
  for(const [part,position] of before)assert.deepEqual(part.position.toArray(),position.toArray(),'Inspection reset restores the exact assembly');
  console.log(JSON.stringify({productId:product.id,screenForeground:'pass',topGap:'closed by display border',visibleRearInserts:'pass',recessedCardMouth:'pass',buttonRouting:'pass',materialIsolation:'pass',inspectionReset:'pass',assembledSizeMm:size.toArray()}));
} finally {
  model?.dispose();model?.group.traverse(object=>{object.geometry?.dispose();for(const material of [].concat(object.material||[]))material.dispose();});
  globalThis.fetch=nativeFetch;THREE.TextureLoader.prototype.loadAsync=nativeTextureLoad;
}
