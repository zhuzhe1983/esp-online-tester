import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {Texture,TextureLoader,Raycaster,Vector3} from '../web/platform/vendor/three.module.min.js';
import {applyShellAppearance} from '../web/platform/shell-appearance.js';
const platform=new URL('../web/platform/',import.meta.url);
const products=JSON.parse(await readFile(new URL('catalog/products.json',platform))).items;
const ids=['m5stack-k002','m5stack-k001-v26','m5stack-k001-b','m5stack-c001-cnc','m5stack-k049'];
const nativeFetch=globalThis.fetch,nativeLoad=TextureLoader.prototype.loadAsync;
try {
 globalThis.fetch=async url=>new Response(await readFile(fileURLToPath(new URL(url))));TextureLoader.prototype.loadAsync=async()=>new Texture();
 for(const id of ids) {
  const p=products.find(p=>p.id===id);assert.ok(p);const url=new URL(p.model.url,platform),m=JSON.parse(await readFile(url));
  for(const [name,digest]of Object.entries(m.artifacts))assert.equal(createHash('sha256').update(await readFile(new URL(name,url))).digest('hex'),digest,id+' source '+name);
  const model=createManufacturerModel(p,null);await model.ready;model.group.updateMatrixWorld(true);assert.equal(model.group.userData.inspectionError,undefined);
  if(id!=='m5stack-k049') {
   assert.equal(model.buttons.length,3);assert.deepEqual(p.controls.map(c=>c.pin),[39,38,37]);
   for(const button of model.buttons){const pos=button.getWorldPosition(new Vector3());const ray=new Raycaster(new Vector3(pos.x,pos.y,10),new Vector3(0,0,-1));assert.ok(ray.intersectObject(model.parts['front-buttons'])[0],id+' actual CAD key face');}
   const glass=model.parts['front-glass'].material,original=glass.color.getHexString();
   for(const shell of model.group.userData.shellMeshes)applyShellAppearance(shell,{shell:{color:'#8054b3',opacity:.35,finish:'plastic'}});
   assert.equal(glass.color.getHexString(),original,'Shell appearance does not recolour optical glass');
  } else {assert.equal(p.runtime.board,'m5paper-original');assert.ok(model.parts['paper-labels']);assert.equal(m.textures[0].sourceId,'photo-3');}
  model.group.userData.inspection?.setProgress(1);model.group.userData.inspection?.setProgress(0);assert.equal(model.group.userData.inspection?.progress,0);model.dispose();
 }
 console.log('ok five classic additions: all artifact hashes, five real model loads, twelve CAD key hits, material isolation, inspection reset and Paper runtime boundary');
}finally{globalThis.fetch=nativeFetch;TextureLoader.prototype.loadAsync=nativeLoad;}
