// Real source triangles and all42 nearest physical-key hit regions. No texture-fit claim.
import test from 'node:test';import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';import {fileURLToPath} from 'node:url';import {join} from 'node:path';import {createHash} from 'node:crypto';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';import {controlInput} from '../web/platform/control-input.js';
const assets=new URL('../web/platform/assets/',import.meta.url),id='m5stack-u215';
test('CardKB2 all42 source hit regions map human control IDs to real guest matrix coordinates',async()=>{
 const native=globalThis.fetch,load=THREE.TextureLoader.prototype.loadAsync;
 try{
  globalThis.fetch=async value=>{const url=new URL(value);url.search='';let bytes;try{bytes=await readFile(url);}catch(e){if(e.code!=='ENOENT'||!process.env.ESP32SIM_MODEL_ASSET_ROOT||!url.href.startsWith(assets.href))throw e;bytes=await readFile(join(process.env.ESP32SIM_MODEL_ASSET_ROOT,fileURLToPath(url).slice(fileURLToPath(assets).length)));}return new Response(bytes);};THREE.TextureLoader.prototype.loadAsync=async()=>new THREE.Texture();
  const product=JSON.parse(await readFile(new URL('../web/platform/catalog/products.json',import.meta.url))).items.find(p=>p.id===id),metadata=JSON.parse(await readFile(new URL(id+'/model.json',assets))),check=JSON.parse(await readFile(new URL(id+'/build-check.json',assets)));
  const model=createManufacturerModel(product,null);try{
   await model.ready;model.group.updateMatrixWorld(true);assert.equal(metadata.presentation.buttons.length,42);
   for(const region of metadata.presentation.buttons){const[x,y]=region.positionMm,ray=new THREE.Raycaster(new THREE.Vector3(x,y,100),new THREE.Vector3(0,0,-1)),hit=ray.intersectObject(model.group,true)[0];assert.equal(hit.object.userData.control,region.id,'nearest key '+region.id);const control=product.controls.find(c=>c.id===region.id);assert.deepEqual(controlInput({type:'button',id:region.id,pressed:true},product),{type:'matrix-key',row:control.row,column:control.column,pressed:true});}
   assert.equal(createHash('sha256').update(await readFile(new URL(id+'/model.json',assets))).digest('hex'),check.modelSha256);
   const stored=new Uint8Array(await(await fetch(new URL(id+'/'+metadata.binary,assets))).arrayBuffer());assert.equal(createHash('sha256').update(stored).digest('hex'),check.storedGeometrySha256,'retained geometry bytes are unchanged');
  }finally{model.dispose();}
 }finally{globalThis.fetch=native;THREE.TextureLoader.prototype.loadAsync=load;}
});
