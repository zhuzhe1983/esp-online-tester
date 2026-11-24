#!/usr/bin/env node
// Actual production-loader regression: optical pixels and their cut-out cannot drift during inspection.
import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import path from 'node:path';
import {createHash} from 'node:crypto';
import * as THREE from '../../../web/platform/vendor/three.module.min.js';
import {createManufacturerModel} from '../../../web/platform/manufacturer-model.js';
const root=fileURLToPath(new URL('../../../',import.meta.url)),assets=path.join(root,'web/platform/assets');
const ids=process.argv.slice(2),sha=b=>createHash('sha256').update(b).digest('hex'),results=[];
const fetchOriginal=globalThis.fetch,docOriginal=globalThis.document,textureOriginal=THREE.TextureLoader.prototype.loadAsync;
globalThis.document={createElement(tag){assert.equal(tag,'canvas');return {width:0,height:0,getContext(){return {fillText(){}};}};}};
globalThis.fetch=async input=>{const u=new URL(input),p=fileURLToPath(u);assert.equal(u.protocol,'file:');assert.ok(path.resolve(p).startsWith(assets+path.sep));const b=await readFile(p);if(u.searchParams.has('sha256'))assert.equal(sha(b),u.searchParams.get('sha256'));return new Response(b);};
THREE.TextureLoader.prototype.loadAsync=async()=>new THREE.Texture();
try{
 for(const id of ids){
  const dir=path.join(assets,id),raw=await readFile(path.join(dir,'model.json')),meta=JSON.parse(raw),plan=JSON.parse(await readFile(path.join(dir,'model-plan.json'))),check=plan.validation.rigidOpticalInspection;
  if(!check){assert.ok(id.endsWith('lcd-1-3-b')||id.endsWith('lcd-1-3-c'));results.push({id,modelSha256:sha(raw),classification:'Independent removable open-front base/prism stand; native case is not the optical panel retaining frame.',affected:false});continue;}
  const group=meta.presentation.inspectionGroups.find(g=>g.id===check.group);assert.ok(group);assert.deepEqual(group.parts,check.parts);assert.deepEqual(group.offsetMm,check.offsetMm);assert.ok(group.parts.includes('live-display')&&group.parts.includes('estimated-front-frame'));
  const product={id,model:{url:`./assets/${id}/model.json`},display:plan.validation.display,controls:plan.validation.controls};
  const model=createManufacturerModel(product,new THREE.Texture());await model.ready;assert.equal(model.group.userData.inspectionError,undefined);model.group.updateMatrixWorld(true);
  const objects=check.parts.map(n=>[n,n==='live-display'?model.screen:model.parts[n]]);assert.ok(objects.every(([,o])=>o));const before=objects.map(([n,o])=>[n,o,[...o.matrixWorld.elements]]);
  model.group.userData.inspection.setExpanded(true);model.group.updateMatrixWorld(true);
  for(const [n,o,b] of before){const expected=[...b];for(let a=0;a<3;a++)expected[12+a]+=check.offsetMm[a];assert.ok(o.matrixWorld.elements.every((v,a)=>Math.abs(v-expected[a])<1e-10),`${id} ${n} must have the shared optical transform`);}
  const screen=model.screen,cap=model.parts['estimated-front-frame'],[w,h]=meta.presentation.screen.sizeMm,samples=meta.presentation.screen.shape==='circle'?[-.3,0,.3]:[-.4,0,.4];let rays=0;
  for(const x of samples)for(const y of samples){const p=screen.localToWorld(new THREE.Vector3(x*w,y*h,0)),ray=new THREE.Raycaster(p.clone().add(new THREE.Vector3(0,0,1000)),new THREE.Vector3(0,0,-1),0,2000);assert.equal(ray.intersectObject(cap,true).length,0,`${id} expanded frame preserves the actual optical aperture`);const first=ray.intersectObjects(model.group.children,true).find(h=>h.object.userData.inputOccluder||h.object.userData.screen||h.object.userData.control);assert.ok(first?.object===screen,`${id} expanded optical pixels remain unobstructed`);rays++;}
  model.group.userData.inspection.setExpanded(false);model.group.updateMatrixWorld(true);for(const [n,o,b]of before)assert.deepEqual(o.matrixWorld.elements,b,`${id} ${n} restores exactly`);
  results.push({id,modelSha256:sha(raw),group:check.group,rigidParts:check.parts,offsetMm:check.offsetMm,expandedOpticalRays:rays,exactRestore:true});model.dispose();console.log(id+' optical inspection pass');
 }
 await writeFile(path.join(root,'docs/research/waveshare-s3-20261123/seventh-verification/optical-inspection-regression.json'),JSON.stringify({checkedAt:new Date().toISOString(),scope:'Production Three.js objects, actual expanded world transforms and aperture/screen rays. No browser/WebGL or manufactured fit acceptance.',results},null,2)+'\n');
}finally{globalThis.fetch=fetchOriginal;globalThis.document=docOriginal;THREE.TextureLoader.prototype.loadAsync=textureOriginal;}
