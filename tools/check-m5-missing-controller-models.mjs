#!/usr/bin/env node
// Actual loader/geometry/material checks. Texture decoding/browser photography
// remain a separate review; this test does not pretend to render a screenshot.
import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {fileURLToPath} from 'node:url';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {applyShellAppearance} from '../web/platform/shell-appearance.js';
const ids=process.argv.length>3?process.argv.slice(3):['m5stack-k140','m5stack-k016-p2','m5stack-k117','m5stack-k126','m5stack-u195'];
const root=new URL('../web/platform/',import.meta.url),catalog=JSON.parse(await readFile(new URL('catalog/products.json',root),'utf8')).items;
const nativeFetch=globalThis.fetch,nativeLoad=THREE.TextureLoader.prototype.loadAsync,nativeDocument=globalThis.document;
const sha=data=>createHash('sha256').update(data).digest('hex'),results=[];
try {
 globalThis.document={createElement:()=>({getContext:()=>({fillText(){}})})};
 globalThis.fetch=async url=>{const u=new URL(url);u.search='';return new Response(await readFile(u));};
 THREE.TextureLoader.prototype.loadAsync=async url=>{const u=new URL(url);u.search='';assert.ok((await readFile(u)).length);return new THREE.Texture();};
 for(const id of ids){
  console.log('checking '+id);const product=catalog.find(p=>p.id===id);assert.ok(product);assert.ok(['unavailable','partial','supported'].includes(product.runtime.status));assert.equal(typeof product.runtime.board,product.runtime.status==='unavailable'?'object':'string');
  const base=new URL(`assets/${id}/`,root),bytes=await readFile(new URL('model.json',base)),metadata=JSON.parse(bytes);
  for(const [file,digest]of Object.entries(metadata.artifacts))assert.equal(sha(await readFile(new URL(file,base))),digest,id+' '+file);
  const binary=await readFile(new URL(metadata.binary,base));
  for(const part of metadata.parts){
   assert.equal(part.vertexCount%3,0);const view=new DataView(binary.buffer,binary.byteOffset+part.byteOffset,part.vertexCount*24);
   const low=[Infinity,Infinity,Infinity],high=[-Infinity,-Infinity,-Infinity];
   for(let i=0;i<part.vertexCount;i++)for(let a=0;a<3;a++){const value=view.getFloat32(i*24+a*4,true);assert.ok(Number.isFinite(value));low[a]=Math.min(low[a],value);high[a]=Math.max(high[a],value);}
   for(let a=0;a<3;a++){assert.ok(Math.abs(low[a]-part.boundsMm[0][a])<1e-5);assert.ok(Math.abs(high[a]-part.boundsMm[1][a])<1e-5);}
  }
  const framebuffer=new THREE.DataTexture(new Uint8Array([0,0,0,255]),1,1),model=createManufacturerModel(product,framebuffer);await model.ready;
  try {
   const papers=metadata.derivedParts.filter(p=>p.kind==='label').map(p=>model.parts[p.id]),paperState=papers.map(p=>({color:p.material.color.getHex(),opacity:p.material.opacity,map:p.material.map}));
   for(const preset of [{color:'#d4ac36',opacity:1,finish:'metal'},{color:'#8054b3',opacity:.35,finish:'plastic'}])for(const shell of model.group.userData.shellMeshes)applyShellAppearance(shell,{shell:preset});
   papers.forEach((p,i)=>{assert.equal(p.material.color.getHex(),paperState[i].color);assert.equal(p.material.opacity,paperState[i].opacity);assert.equal(p.material.map,paperState[i].map);assert.equal(p.material.alphaTest,.5);});
   model.group.updateMatrixWorld(true);const board=model.parts['pcb-main'];assert.ok(board,'Missing explicit unknown board');
   const marker=board.children.find(p=>/unknown/.test(p.name));assert.ok(marker);
   const at=marker.getWorldPosition(new THREE.Vector3()),normal=new THREE.Vector3(0,0,1).applyQuaternion(marker.getWorldQuaternion(new THREE.Quaternion()));
   const clearFrom=offset=>{const origin=at.clone().add(offset);const ray=new THREE.Raycaster(origin,at.clone().sub(origin).normalize());return ray.intersectObject(model.group,true).filter(hit=>hit.object.material?.opacity>=.9)[0]?.object===marker;};
   let unknownView='translucent-shell-front-normal';
   if(!clearFrom(normal.clone().multiplyScalar(100))){
    assert.ok(model.group.userData.inspection,id+' UNKNOWN needs an inspection path');model.group.userData.inspection.setExpanded(true);model.group.updateMatrixWorld(true);
    const offsets=[new THREE.Vector3(60,0,60),new THREE.Vector3(-60,0,60),new THREE.Vector3(0,60,60),new THREE.Vector3(0,-60,60)];
    const offset=offsets.find(clearFrom);assert.ok(offset,id+' UNKNOWN center occluded in expanded oblique views');unknownView='expanded-oblique offset '+JSON.stringify(offset.toArray());
    model.group.userData.inspection.setExpanded(false);model.group.updateMatrixWorld(true);
   }
   if(metadata.presentation.screen){assert.ok(model.screen?.material.map===framebuffer);assert.ok(!metadata.derivedParts.some(p=>p.kind==='label'&&/screen|display/i.test(p.id)));}
   let inspection=false;
   if(model.group.userData.inspection){const initial=model.group.children.map(p=>({p,position:p.position.clone()}));for(let i=0;i<3;i++){model.group.userData.inspection.setExpanded(true);model.group.userData.inspection.setExpanded(false);}for(const {p,position}of initial)assert.ok(p.position.distanceTo(position)<1e-10);inspection=true;}
   const assemblyBox=new THREE.Box3();for(const part of metadata.parts)assemblyBox.union(new THREE.Box3().setFromObject(model.parts[part.name]));for(const part of metadata.derivedParts.filter(p=>!['label','printed-marking','photo-surface'].includes(p.kind))){const object=model.parts[part.id];assemblyBox.union(new THREE.Box3().setFromObject(part.kind==='pcb-placeholder'?object.children[0]:object));}const assemblySize=assemblyBox.getSize(new THREE.Vector3()).toArray();
   results.push({productId:id,modelSha256:sha(bytes),geometrySha256:sha(binary),physicalAssemblyExtentsMm:assemblySize,dimensionAssessment:'comparison only; derived features are photo estimates and do not establish manufacturing accuracy',loader:'passed',paperMaterialIsolation:'passed',unknownMarkerCenterVisibility:{status:'passed',view:unknownView,limit:'center ray only; rendered text legibility requires browser review'},inspectionReset:inspection?'passed':'not-declared',textureDecoding:'not-rendered',browserPhotoComparison:'not-verified'});
  }finally{model.dispose();framebuffer.dispose();}
 }
}finally{globalThis.fetch=nativeFetch;THREE.TextureLoader.prototype.loadAsync=nativeLoad;if(nativeDocument===undefined)delete globalThis.document;else globalThis.document=nativeDocument;}
const result={schemaVersion:1,models:results.length,scope:'Real Three loader with file-backed fetch and texture-byte existence/hash checks; browser image decoding is stubbed and not claimed.',results};
if(process.argv[2])await writeFile(process.argv[2],JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify(result,null,2));
