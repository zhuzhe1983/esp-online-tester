// Actual manufacturer triangles/ray ordering; textures are stubbed, no visual calibration claim.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {join} from 'node:path';
import {createHash} from 'node:crypto';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
const assetBase=new URL('../web/platform/assets/',import.meta.url);
const products=JSON.parse(await readFile(new URL('../web/platform/catalog/products.json',import.meta.url))).items;
const skuIds=['m5stack-u139','m5stack-u140'];
for(const id of skuIds)test(id+' native cover nearest hit reaches the exact physical button ID',async()=>{
 const originalFetch=globalThis.fetch,textureLoad=THREE.TextureLoader.prototype.loadAsync;
 try{
  globalThis.fetch=async value=>{const url=new URL(value);url.search='';let bytes;try{bytes=await readFile(url);}catch(e){if(e.code!=='ENOENT'||!process.env.ESP32SIM_MODEL_ASSET_ROOT||!url.href.startsWith(assetBase.href))throw e;bytes=await readFile(join(process.env.ESP32SIM_MODEL_ASSET_ROOT,fileURLToPath(url).slice(fileURLToPath(assetBase).length)));}return new Response(bytes);};
  THREE.TextureLoader.prototype.loadAsync=async()=>new THREE.Texture();
  const model=createManufacturerModel(products.find(p=>p.id===id),null);try{
   await model.ready;model.group.updateMatrixWorld(true);const cover=model.parts['button-cover'];assert.equal(cover.userData.control,'button');assert(model.buttons.includes(cover));
   for(const x of[-8,0,8]){const ray=new THREE.Raycaster(new THREE.Vector3(x,0,100),new THREE.Vector3(0,0,-1)),hit=ray.intersectObject(model.group,true)[0];assert.equal(hit?.object,cover,'native cover is the nearest visible surface at x='+x);assert.equal(hit.object.userData.control,'button');}
   const metadata=JSON.parse(await readFile(new URL(id+'/model.json',assetBase))),plan=JSON.parse(await readFile(new URL(id+'/model-plan.json',assetBase))),check=JSON.parse(await readFile(new URL('../docs/platform/evidence/m5-unit-controllers-20261126/'+id+'-build-check.json',import.meta.url)));
   assert.equal(metadata.parts.find(p=>p.name==='button-cover').controlId,'button');assert.equal(plan.parts.find(p=>p.name==='button-cover').controlId,'button');assert.equal(check.metadataSha256,createHash('sha256').update(await readFile(new URL(id+'/model.json',assetBase))).digest('hex'));
   assert.equal(metadata.artifacts[metadata.binary],check.geometrySha256,'geometry is the retained manufacturer export');
   const geometryBytes=new Uint8Array(await (await fetch(new URL(id+'/'+metadata.binary,assetBase))).arrayBuffer());assert.equal(createHash('sha256').update(geometryBytes).digest('hex'),check.geometrySha256,'actual ray triangles retain the verified geometry hash');
  }finally{model.dispose();}
 }finally{globalThis.fetch=originalFetch;THREE.TextureLoader.prototype.loadAsync=textureLoad;}
});
