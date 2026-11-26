#!/usr/bin/env node
// Actual production-loader integration, with CPU rays and material isolation.
// Node substitutes texture decoding/canvas; this is not WebGL or photo acceptance.
import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import path from 'node:path';
import {createHash} from 'node:crypto';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {applyShellAppearance} from '../web/platform/shell-appearance.js';

const root=fileURLToPath(new URL('../',import.meta.url));
const assets=path.join(root,'web/platform/assets');
const ids=process.argv.length>2?process.argv.slice(2):['m5stack-k159','m5stack-k162','m5stack-k016-p-se','m5stack-c156','m5stack-a172'];
const catalog=JSON.parse(await readFile(path.join(root,'web/platform/catalog/products.json'))).items;
const digest=bytes=>createHash('sha256').update(bytes).digest('hex');
const original={fetch:globalThis.fetch,document:globalThis.document,texture:THREE.TextureLoader.prototype.loadAsync};
let metadata,labels=[];
globalThis.document={createElement(){return {width:0,height:0,getContext(){return {fillText(text){labels.push(text);}};}};}};
globalThis.fetch=async input=>{const url=new URL(input);assert.equal(url.protocol,'file:');const filename=fileURLToPath(url);assert.ok(filename.startsWith(assets+path.sep));const bytes=await readFile(filename);if(url.searchParams.has('sha256'))assert.equal(digest(bytes),url.searchParams.get('sha256'));return new Response(bytes);};
THREE.TextureLoader.prototype.loadAsync=async function(input){const filename=fileURLToPath(new URL(input));const bytes=await readFile(filename);assert.equal(digest(bytes),metadata.artifacts[path.basename(filename)],'Texture source identity');return new THREE.Texture({width:512,height:512});};
const results=[];
try {
 for(const id of ids){
  const product=catalog.find(p=>p.id===id);assert.ok(product);
  const sharedHost={"m5stack-k159":"paperdiy","m5stack-k162":"m5-tough-c5","m5stack-k016-p-se":"stickc-plus-se","m5stack-c156":"cores3","m5stack-a172":"m5-stamp-addon-c6","m5stack-u017":"m5stack-u017","m5stack-u109":"m5stack-u109"}[id];
  assert.equal(product.runtime.status,sharedHost?'partial':'unavailable');assert.equal(product.runtime.board,sharedHost||null);
  const bytes=await readFile(path.join(assets,id,'model.json'));metadata=JSON.parse(bytes);labels=[];
  for(const [filename,expected] of Object.entries(metadata.artifacts))assert.equal(digest(await readFile(path.join(assets,id,filename))),expected,filename);
  const model=createManufacturerModel(product,new THREE.Texture());await model.ready;model.group.updateMatrixWorld(true);
  assert.equal(model.group.userData.inspectionError,undefined,id+' source inspection groups');
  assert.equal(Boolean(model.screen),Boolean(product.display),id+' actual screen presence');
  for(const part of metadata.parts)assert.ok(model.parts[part.name],id+' native geometry '+part.name);
  assert.ok(labels.includes('UNKNOWN'),id+' unknown board stays identifiable');
  const poses=model.group.children.map(o=>[o,o.position.clone()]);const inspection=model.group.userData.inspection;assert.ok(inspection,id+' inspection');inspection.setExpanded(true);assert.ok(poses.some(([o,p])=>!o.position.equals(p)));inspection.setExpanded(false);for(const [o,p]of poses)assert.ok(o.position.equals(p),id+' exact restore');model.group.updateMatrixWorld(true);
  const rays=[],keyRays=[],unknownVisibility=[];
  if(model.screen){const normal=new THREE.Vector3(0,0,1).transformDirection(model.screen.matrixWorld),[w,h]=metadata.presentation.screen.sizeMm;for(const u of [-.4,0,.4])for(const v of [-.4,0,.4]){const p=model.screen.localToWorld(new THREE.Vector3(u*w,v*h,0));const hit=new THREE.Raycaster(p.clone().addScaledVector(normal,1000),normal.clone().negate(),0,2000).intersectObjects(model.group.children,true).find(x=>x.object.userData.inputOccluder||x.object.userData.screen||x.object.userData.control);assert.equal(hit?.object,model.screen,id+' exposed display ray '+[u,v]);rays.push([u,v]);}}
  const shells=new Set(model.group.userData.shellMeshes||[]),preserved=[];model.group.traverse(o=>{if(o.isMesh&&!shells.has(o)&&!Array.isArray(o.material))preserved.push([o,o.material.color?.getHexString(),o.material.opacity]);});
  for(const style of [{color:'#d6ad45',opacity:1,finish:'metal'},{color:'#8054b3',opacity:.35,finish:'plastic'}]){for(const shell of shells)applyShellAppearance(shell,{shell:style});for(const [object,color,opacity]of preserved){assert.equal(object.material.color?.getHexString(),color,id+' paper/PCB/connector color isolation');assert.equal(object.material.opacity,opacity,id+' non-shell opacity isolation');}}
  if(['m5stack-a172','m5stack-u215','m5stack-u109'].includes(id))assert.equal(shells.size,0,'Bare board cannot be recolored as an enclosure');
  if(id==='m5stack-k159')for(const key of ['button-a','button-b']){
   const mesh=model.parts['cover-key:'+key];assert.ok(mesh,key+' exposed physical actuator');const point=mesh.getWorldPosition(new THREE.Vector3()),normal=new THREE.Vector3(0,0,1).transformDirection(mesh.matrixWorld);
   const hit=new THREE.Raycaster(point.clone().addScaledVector(normal,100),normal.clone().negate(),0,200).intersectObjects(model.group.children,true).find(x=>x.object.userData.inputOccluder||x.object.userData.control);
   assert.equal(hit?.object.userData.control,key,key+' outward side face nearest ray');keyRays.push({id:key,ray:'actuator-outward-normal'});
  }
  if(id==='m5stack-u215'){
   const mapping=JSON.parse(await readFile(path.join(assets,id,'keyboard-mapping.json')));
   assert.equal(mapping.keys.length,42);assert.deepEqual(mapping.unpopulated,[{row:0,column:10},{row:3,column:10}]);
   for(const key of mapping.keys){const mesh=model.parts['cover-key:'+key.id];assert.ok(mesh,key.id+' physical cover');const point=mesh.getWorldPosition(new THREE.Vector3());const hit=new THREE.Raycaster(point.clone().add(new THREE.Vector3(0,0,1000)),new THREE.Vector3(0,0,-1),0,2000).intersectObjects(model.group.children,true).find(x=>x.object.userData.inputOccluder||x.object.userData.control);assert.equal(hit?.object.userData.control,key.id,key.id+' visible key has its own nearest hit');assert.equal(key.scanIndex,key.row*11+key.column);keyRays.push({id:key.id,row:key.row,column:key.column,scanIndex:key.scanIndex});}
  }
  if(['m5stack-u215','m5stack-u213','m5stack-m141','m5stack-u017','m5stack-u109'].includes(id)){
   inspection.setExpanded(true);model.group.updateMatrixWorld(true);
   const markers=[];model.group.traverse(o=>{if(o.name.includes('-unknown-'))markers.push(o);});
   assert.ok(markers.length);
   for(const marker of markers){const point=marker.getWorldPosition(new THREE.Vector3()),normal=new THREE.Vector3(0,0,1).transformDirection(marker.matrixWorld);const hit=new THREE.Raycaster(point.clone().addScaledVector(normal,1000),normal.clone().negate(),0,2000).intersectObjects(model.group.children,true).find(x=>x.object.visible&&x.object.material?.opacity!==0);assert.equal(hit?.object,marker,id+' expanded UNKNOWN is geometrically visible');unknownVisibility.push({marker:marker.name,mode:'inspect-parts',ray:'unobstructed'});}
   inspection.setExpanded(false);model.group.updateMatrixWorld(true);
  }
  const box=new THREE.Box3().setFromObject(model.group);assert.ok([...box.min.toArray(),...box.max.toArray()].every(Number.isFinite));
  results.push({id,modelSha256:digest(bytes),loader:'pass',artifactHashes:'pass',inspectionRestore:'pass',unknownMarkerConstructed:true,unknownVisibility,framebufferRays:rays,keyRays,shellMaterialIsolation:'pass',fullConstructedBoundsMm:[box.min.toArray(),box.max.toArray()],runtimeValidated:false,photoComparison:'not-tested',browserReadability:'not-tested'});
  model.dispose();model.group.traverse(o=>{o.geometry?.dispose();if(o.material)(Array.isArray(o.material)?o.material:[o.material]).forEach(m=>m.dispose());});console.log(id+' pass');
 }
 const output=path.join(root,process.argv.length>2?'docs/platform/evidence/m5-module-products-20261126/loader-checks.json':'docs/platform/evidence/m5-missing-products-20261126/loader-checks.json');await mkdir(path.dirname(output),{recursive:true});await writeFile(output,JSON.stringify({checkedAt:new Date().toISOString(),scope:'Production Three.js loader and CPU raycaster; source hashes and material/inspection checks. No WebGL, image decoding, calibrated photo overlay, input or firmware execution acceptance.',results},null,2)+'\n');
} finally {globalThis.fetch=original.fetch;globalThis.document=original.document;THREE.TextureLoader.prototype.loadAsync=original.texture;}
