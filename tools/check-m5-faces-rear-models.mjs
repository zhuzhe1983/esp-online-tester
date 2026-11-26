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
const ids=(process.argv.find(arg=>arg.startsWith('--ids='))?.slice(6)||'m5stack-k005,m5stack-c156').split(',');
assert.ok(ids.every(id=>['m5stack-k005','m5stack-c156'].includes(id)),'Only explicitly supported model IDs');
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
  const product=catalog.find(p=>p.id===id);assert.ok(product);assert.equal(product.runtime.status,'partial');assert.equal(product.runtime.board,id==='m5stack-k005'?'m5stack-basic':'cores3');
  const bytes=await readFile(path.join(assets,id,'model.json'));metadata=JSON.parse(bytes);labels=[];
  for(const [filename,expected] of Object.entries(metadata.artifacts))assert.equal(digest(await readFile(path.join(assets,id,filename))),expected,filename);
  const model=createManufacturerModel(product,new THREE.Texture());await model.ready;model.group.updateMatrixWorld(true);
  assert.equal(model.group.userData.inspectionError,undefined,id+' source inspection groups');
  assert.equal(Boolean(model.screen),Boolean(product.display),id+' actual screen presence');
  for(const part of metadata.parts)assert.ok(model.parts[part.name],id+' native geometry '+part.name);
  assert.ok(labels.includes('UNKNOWN'),id+' unknown board stays identifiable');
  const poses=model.group.children.map(o=>[o,o.position.clone()]);const inspection=model.group.userData.inspection;assert.ok(inspection,id+' inspection');inspection.setExpanded(true);assert.ok(poses.some(([o,p])=>!o.position.equals(p)));inspection.setExpanded(false);for(const [o,p]of poses)assert.ok(o.position.equals(p),id+' exact restore');model.group.updateMatrixWorld(true);
  const rays=[];
  if(model.screen){const normal=new THREE.Vector3(0,0,1).transformDirection(model.screen.matrixWorld),[w,h]=metadata.presentation.screen.sizeMm;for(const u of [-.4,0,.4])for(const v of [-.4,0,.4]){const p=model.screen.localToWorld(new THREE.Vector3(u*w,v*h,0));const hit=new THREE.Raycaster(p.clone().addScaledVector(normal,1000),normal.clone().negate(),0,2000).intersectObjects(model.group.children,true).find(x=>x.object.userData.inputOccluder||x.object.userData.screen||x.object.userData.control);assert.ok(hit?.object===model.screen,id+' exposed display ray '+[u,v]);rays.push([u,v]);}}
  const shells=new Set(model.group.userData.shellMeshes||[]),preserved=[];model.group.traverse(o=>{if(o.isMesh&&!shells.has(o)&&!Array.isArray(o.material))preserved.push([o,o.material.color?.getHexString(),o.material.opacity]);});
  for(const style of [{color:'#d6ad45',opacity:1,finish:'metal'},{color:'#8054b3',opacity:.35,finish:'plastic'}]){for(const shell of shells)applyShellAppearance(shell,{shell:style});for(const [object,color,opacity]of preserved){assert.equal(object.material.color?.getHexString(),color,id+' paper/PCB/connector color isolation');assert.equal(object.material.opacity,opacity,id+' non-shell opacity isolation');}}
  if(id==='m5stack-a172')assert.equal(shells.size,0,'Bare A172 radio board cannot be recolored as an enclosure');
  if(id==='m5stack-k005') {
   const mapping=JSON.parse(await readFile(path.join(assets,id,'panel-key-mapping.json')));
   for(const [panel,count] of [['qwerty',35],['calculator',20],['gamepad',8]])assert.equal(mapping.keys.filter(k=>k.panel===panel).length,count);
   assert.equal(new Set(mapping.keys.map(k=>k.id)).size,63);
   const keyRays=[];
   for(const key of mapping.keys){const part=metadata.parts.find(p=>p.name===key.id);assert.ok(part);const b=part.boundsMm,center=new THREE.Vector3((b[0][0]+b[1][0])/2,(b[0][1]+b[1][1])/2,b[1][2]);const hit=new THREE.Raycaster(center.clone().add(new THREE.Vector3(0,0,1000)),new THREE.Vector3(0,0,-1),0,2000).intersectObjects(model.group.children,true).find(x=>x.object.userData.inputOccluder);assert.ok(hit?.object.name===key.id||hit?.object.name===key.id+'-photo-surface',key.id+' exposed physical key');assert.equal(hit.object.userData.control,undefined,'Panel key is visual only');keyRays.push(key.id);}
   assert.equal(keyRays.length,63);
   assert.equal(product.controls.length,3,'Host control identity unchanged');
   for(const button of metadata.presentation.buttons){const hit=new THREE.Raycaster(new THREE.Vector3(...button.positionMm).add(new THREE.Vector3(0,0,1000)),new THREE.Vector3(0,0,-1),0,2000).intersectObjects(model.group.children,true).find(x=>x.object.userData.inputOccluder||x.object.userData.control);assert.equal(hit?.object.userData.control,button.id,'Host Gray key remains exposed and mapped');}

  }
  inspection.setExpanded(true);model.group.updateMatrixWorld(true);
  const unknownRays=[];
  model.group.traverse(o=>{if(o.name.endsWith('-unknown-front')){const normal=new THREE.Vector3(0,0,1).transformDirection(o.matrixWorld),center=o.getWorldPosition(new THREE.Vector3());const hit=new THREE.Raycaster(center.clone().addScaledVector(normal,1000),normal.clone().negate(),0,2000).intersectObjects(model.group.children,true).find(x=>x.object===o||x.object.userData.inputOccluder);assert.ok(hit?.object===o,id+' visible inspection UNKNOWN '+o.name+' hit '+hit?.object?.name);unknownRays.push(o.name);}});
  assert.ok(unknownRays.length>0);inspection.setExpanded(false);model.group.updateMatrixWorld(true);
  const box=new THREE.Box3().setFromObject(model.group);assert.ok([...box.min.toArray(),...box.max.toArray()].every(Number.isFinite));
  results.push({id,modelSha256:digest(bytes),loader:'pass',artifactHashes:'pass',inspectionRestore:'pass',unknownMarkerConstructed:true,inspectionUnknownRays:unknownRays,physicalPanelKeyRays:id==='m5stack-k005'?63:0,hostKeyRays:id==='m5stack-k005'?3:0,framebufferRays:rays,shellMaterialIsolation:'pass',fullConstructedBoundsMm:[box.min.toArray(),box.max.toArray()],runtimeValidated:false,photoComparison:'not-tested',browserReadability:'not-tested'});
  model.dispose();model.group.traverse(o=>{o.geometry?.dispose();if(o.material)(Array.isArray(o.material)?o.material:[o.material]).forEach(m=>m.dispose());});console.log(id+' pass');
 }
 const output=process.argv[2]||path.join(root,'docs/platform/evidence/m5-faces-rear-20261126/loader-checks.json');await mkdir(path.dirname(output),{recursive:true});await writeFile(output,JSON.stringify({checkedAt:new Date().toISOString(),scope:'Production Three.js loader and CPU raycaster; source hashes and material/inspection checks. No WebGL, image decoding, calibrated photo overlay, input or firmware execution acceptance.',results},null,2)+'\n');
} finally {globalThis.fetch=original.fetch;globalThis.document=original.document;THREE.TextureLoader.prototype.loadAsync=original.texture;}
