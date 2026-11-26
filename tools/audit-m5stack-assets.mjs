// Headless asset/geometry audit, deliberately separate from image/GPU acceptance.
import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {fileURLToPath} from 'node:url';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {applyShellAppearance} from '../web/platform/shell-appearance.js';

const output=process.argv[2];
if(!output)throw Error('Usage: node tools/audit-m5stack-assets.mjs <receipt.json>');
const platform=new URL('../web/platform/',import.meta.url);
const products=JSON.parse(await readFile(new URL('catalog/products.json',platform))).items.filter(p=>p.manufacturer==='M5Stack');
const originalFetch=globalThis.fetch,originalDocument=globalThis.document,originalTextureLoad=THREE.TextureLoader.prototype.loadAsync;
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const rows=[];
try{
 globalThis.fetch=async value=>{try{return new Response(await readFile(fileURLToPath(value)));}catch{return new Response('',{status:404});}};
 // Retained images must exist. Decoding, visual alignment and GPU upload are not
 // exercised here; consumers must not interpret this stub as rendered evidence.
 THREE.TextureLoader.prototype.loadAsync=async function(value){assert((await readFile(fileURLToPath(value))).length>0);return new THREE.Texture();};
 globalThis.document={createElement:()=>({getContext:()=>({fillText(){}})})};
 for(const product of products){
  const row={id:product.id,runtime:product.runtime.status,board:product.runtime.board,errors:[]};rows.push(row);
  if(!product.model?.url||product.id==='m5stack-papermono'){
   row.status='dedicated-loader';row.scope='Catalog identity inspected; dedicated renderer not exercised by this audit.';continue;
  }
  let model;
  try{
   const url=new URL(product.model.url,platform),bytes=await readFile(url),metadata=JSON.parse(bytes),root=new URL('.',url);
   row.modelSha256=hash(bytes);row.artifacts=[];
   for(const [name,digest] of Object.entries(metadata.artifacts||{})){
    const data=await readFile(new URL(name,root)),actual=hash(data);
    assert.equal(actual,digest,`${product.id}: ${name}`);row.artifacts.push({file:name,sha256:actual,bytes:data.length});
   }
   model=createManufacturerModel(product,new THREE.Texture());await model.ready;model.group.updateMatrixWorld(true);
   row.meshes=0;row.texturedMeshes=0;
   const shells=new Set(model.group.userData.shellMeshes||[]),fixed=[];
   model.group.traverse(object=>{if(!object.isMesh)return;row.meshes++;const materials=[].concat(object.material||[]);for(const material of materials){if(material.map)row.texturedMeshes++;if(!shells.has(object))fixed.push({material,color:material.color?.getHexString(),opacity:material.opacity,map:material.map});}});
   for(const appearance of [{color:'#c5a45b',opacity:1,finish:'metal'},{color:'#8054b3',opacity:.35,finish:'plastic'}]){
    for(const shell of shells)applyShellAppearance(shell,{shell:appearance});
    for(const before of fixed){assert.equal(before.material.color?.getHexString(),before.color);assert.equal(before.material.opacity,before.opacity);assert.equal(before.material.map,before.map);}
   }
   row.status='passed';row.materialIsolation=true;
   if(model.group.userData.inspectionError)row.inspectionWarning=model.group.userData.inspectionError;
  }catch(error){row.status='failed';row.errors.push(error.message);}finally{model?.dispose();}
 }
}finally{
 globalThis.fetch=originalFetch;THREE.TextureLoader.prototype.loadAsync=originalTextureLoad;
 if(originalDocument===undefined)delete globalThis.document;else globalThis.document=originalDocument;
}
const result={schemaVersion:1,checkedAt:new Date().toISOString(),scope:'All current M5Stack product entries; generic loader, declared asset hashes and material isolation only. Image decoding is stubbed. No browser, visual-accuracy, firmware or physical-hardware acceptance.',counts:{products:rows.length,genericPassed:rows.filter(r=>r.status==='passed').length,dedicatedNotExercised:rows.filter(r=>r.status==='dedicated-loader').length,failed:rows.filter(r=>r.status==='failed').length},products:rows};
await writeFile(output,JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify(result.counts));
if(result.counts.failed)process.exitCode=1;
