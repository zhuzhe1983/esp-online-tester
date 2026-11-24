#!/usr/bin/env node
// Additional source-bounded checks on production Three meshes, without WebGL.
import assert from 'node:assert/strict';
import {readFile, writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import path from 'node:path';
import {createHash} from 'node:crypto';
import * as THREE from '../../../web/platform/vendor/three.module.min.js';
import {createManufacturerModel} from '../../../web/platform/manufacturer-model.js';

const root=fileURLToPath(new URL('../../../',import.meta.url));
const folder=path.join(root,'docs/research/waveshare-s3-industrial-2026-11-24/batch-03');
const manifest=JSON.parse(await readFile(path.join(folder,'manifest.json')));
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const original={fetch:globalThis.fetch,document:globalThis.document,texture:THREE.TextureLoader.prototype.loadAsync};
globalThis.document={createElement(){return {getContext(){return {fillText(){}};}};}};
globalThis.fetch=async input=>new Response(await readFile(fileURLToPath(new URL(input))),{status:200});
let metadata;
THREE.TextureLoader.prototype.loadAsync=async function(input){const f=fileURLToPath(new URL(input)),bytes=await readFile(f);assert.equal(sha(bytes),metadata.artifacts[path.basename(f)]);return new THREE.Texture();};
const first=(objects,origin,direction)=>new THREE.Raycaster(origin,direction,0,5000).intersectObjects(objects,true).find(({object})=>{for(let p=object;p;p=p.parent)if(!p.visible)return false;return object.material?.opacity!==0;});
const results=[];
try{
 for(const item of manifest.items)for(const id of item.modelIds){
  const dir=path.join(root,'web/platform/assets',id),raw=await readFile(path.join(dir,'model.json'));
  metadata=JSON.parse(raw);const plan=JSON.parse(await readFile(path.join(dir,'model-plan.json')));
  const model=createManufacturerModel({id,model:{url:`./assets/${id}/model.json`},controls:plan.validation.controls,display:plan.validation.display},new THREE.Texture());
  await model.ready;model.group.updateMatrixWorld(true);
  const unknown=[];
  for(const name of plan.validation.visibleUnknownLabels??[]){
   const label=model.group.getObjectByName(name);assert.ok(label?.isMesh,name);
   const point=label.getWorldPosition(new THREE.Vector3()),normal=new THREE.Vector3(0,0,1).transformDirection(label.matrixWorld);
   const hit=first(model.group.children,point.clone().addScaledVector(normal,1000),normal.clone().negate());
   assert.equal(hit?.object,label,id+' UNKNOWN face must be visible from its own surface normal');
   unknown.push({name,pointMm:point.toArray(),normal:normal.toArray(),firstHit:hit.object.name});
  }
  let hollow=null;
  if(id.includes('8di-8ro')||id.includes('relay-6ch')){
   const walls=Object.values(model.parts).filter(o=>o&&(o.name.startsWith('case-side-')||o.name.startsWith('case-end-')));
   const axial=first(walls,new THREE.Vector3(0,0,1000),new THREE.Vector3(0,0,-1));
   assert.equal(axial,undefined,'Separate side walls leave a real empty cavity');
   const lateral=first(walls,new THREE.Vector3(0,0,10),new THREE.Vector3(1,0,0));
   assert.ok(lateral&&lateral.distance>40,'Cavity has a real inner wall');
   const portCenter=new THREE.Vector3(id.includes('relay-6ch')?-9:24,40,29);
   assert.equal(first(walls,portCenter.clone().add(new THREE.Vector3(0,1000,0)),new THREE.Vector3(0,-1,0)),undefined,'Documented USB route is not obstructed by a solid housing wall');
   hollow={wallParts:walls.map(o=>o.name),axialHits:0,innerWallDistanceMm:lateral.distance,usbPortWallHits:0,scope:'Explicit photo-estimated hollow walls and aperture geometry; no mating or material-strength acceptance.'};
  }
  let ethernetMouth=null;
  if(model.parts['rj45-back']){
   const center=id.includes('8di-8ro')?new THREE.Vector3(41,38,26):new THREE.Vector3(0,28,7.8);
   const shells=Object.values(model.parts).filter(o=>o?.name.startsWith('rj45-'));
   const hit=first(shells,center.clone().add(new THREE.Vector3(0,1000,0)),new THREE.Vector3(0,-1,0));
   assert.equal(hit?.object.name,'rj45-back','RJ45 opening reaches the inner back wall');
   ethernetMouth={firstHit:hit.object.name,pointMm:hit.point.toArray(),scope:'Open photo-estimated shell only; contact geometry and plug fit unknown.'};
  }
  results.push({id,modelSha256:sha(raw),visibleUnknownLabels:unknown,hollow,ethernetMouth});
  model.dispose();
 }
 await writeFile(path.join(folder,'additional-geometry-checks.json'),JSON.stringify({scope:'Production Three.js object construction and CPU ray checks only; no DOM/WebGL rendering or physical hardware acceptance.',results},null,2)+'\n');
 console.log(`${results.length} models: visible UNKNOWN labels, three case cavities, and source-declared RJ45 mouths passed.`);
}finally{globalThis.fetch=original.fetch;globalThis.document=original.document;THREE.TextureLoader.prototype.loadAsync=original.texture;}
