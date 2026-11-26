// Real local mesh/raycast/material checks. Image decode is stubbed, not rendered.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {applyShellAppearance} from '../web/platform/shell-appearance.js';
import {chipForBoard} from '../web/platform/app-image.js';
import {hasTfSlot} from '../web/platform/tf-card-capability.js';
import {Texture,TextureLoader,Raycaster,Vector3,Box3} from '../web/platform/vendor/three.module.min.js';
const base=new URL('../web/platform/',import.meta.url);
const products=JSON.parse(await readFile(new URL('catalog/products.json',base),'utf8')).items;
const ids=['m5stack-k007-v26','m5stack-k049-c','m5stack-u017-pcba','m5stack-u038','m5stack-u082-f'];
const nativeFetch=globalThis.fetch,nativeLoad=TextureLoader.prototype.loadAsync;
const hash=bytes=>createHash('sha256').update(bytes).digest('hex'),rows=[];
try{
 globalThis.fetch=async url=>new Response(await readFile(new URL(url)));
 TextureLoader.prototype.loadAsync=async()=>new Texture();
 for(const id of ids){
  const product=products.find(p=>p.id===id);assert.ok(product);
  const url=new URL(product.model.url,base),metadata=JSON.parse(await readFile(url,'utf8'));
  for(const [file,digest]of Object.entries(metadata.artifacts))assert.equal(hash(await readFile(new URL(file,url))),digest,`${id} ${file}`);
  const model=createManufacturerModel(product,new Texture());await model.ready;model.group.updateMatrixWorld(true);
  assert.ok(model.parts['pcb-main']);
  const unchanged=[...model.group.children].filter(m=>m.isMesh&&!model.group.userData.shellMeshes.includes(m)).map(m=>[m,m.material.color?.getHexString(),m.material.opacity]);
  for(const shell of [{color:'#d5ad40',opacity:1,finish:'metal'},{color:'#8054b3',opacity:.35,finish:'plastic'}]){
   for(const mesh of model.group.userData.shellMeshes)applyShellAppearance(mesh,{shell});
   for(const [mesh,color,opacity]of unchanged){assert.equal(mesh.material.color?.getHexString(),color,`${id} non-shell tint`);assert.equal(mesh.material.opacity,opacity,`${id} non-shell opacity`);}
  }
  if(id==='m5stack-k007-v26'){
   assert.equal(product.runtime.board,'m5stack-basic');assert.equal(chipForBoard(product.runtime.board),'esp32');assert.equal(product.runtime.psramMb,8);assert.equal(hasTfSlot(product),true);
   for(const c of product.controls){const part=model.parts[c.id];part.geometry.computeBoundingBox();const box=part.geometry.boundingBox;const p=box.getCenter(new Vector3());p.z=100;const hit=new Raycaster(p,new Vector3(0,0,-1)).intersectObject(model.group,true).find(h=>h.object.userData.inputOccluder);assert.equal(hit?.object.userData.control,c.id);}
  }else if(['m5stack-u017-pcba','m5stack-u038','m5stack-u082-f'].includes(id)){assert.equal(product.runtime.board,id);assert.equal(product.runtime.status,'partial');assert.equal(chipForBoard(id),'esp32');}else if(id==='m5stack-k049-c'){assert.equal(product.runtime.board,'m5paper-comm');assert.equal(product.runtime.status,'partial');}else{assert.equal(product.runtime.board,null);assert.equal(product.runtime.status,'unavailable');}
  if(id==='m5stack-k049-c')assert.equal(hasTfSlot(product),true);
  if(id==='m5stack-u017-pcba'){assert.equal(model.group.userData.shellMeshes.length,0,'BarePCB is not a recolorable shell');assert.equal(metadata.derivedParts.find(p=>p.id==='pcb-main').geometry.labelSide,'back');}
  const bounds=new Box3().setFromObject(model.group).getSize(new Vector3());
  rows.push({id,modelSha256:hash(await readFile(url)),geometrySha256:metadata.artifacts[metadata.binary],loaded:true,assetHashesVerified:true,nonShellMaterialIsolation:true,renderedObjectEnvelopeMm:bounds.toArray()});
  model.dispose();
 }
 console.log(JSON.stringify({products:rows,scope:'Node/Three actual local geometry and material checks; image decoding stubbed. Envelopes include derived geometry; this is not independent dimension, GPU/photo or hardware acceptance.'},null,2));
}finally{globalThis.fetch=nativeFetch;TextureLoader.prototype.loadAsync=nativeLoad;}
