// Production Three loader/rays/materials; texture pixels are decoded by the
// separate CPU export/render workflow, not this Node contract check.
import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {applyShellAppearance} from '../web/platform/shell-appearance.js';
const root=new URL('../',import.meta.url),ids=process.argv.slice(2),hash=b=>createHash('sha256').update(b).digest('hex');
assert(ids.length,'Pass exact product IDs');
const products=JSON.parse(await readFile(new URL('web/platform/catalog/products.json',root))).items;
const old={fetch:globalThis.fetch,document:globalThis.document,load:THREE.TextureLoader.prototype.loadAsync};
globalThis.fetch=async url=>new Response(await readFile(fileURLToPath(url)));
globalThis.document={createElement:()=>({width:1,height:1,getContext:()=>({fillText(){}})})};
THREE.TextureLoader.prototype.loadAsync=async url=>{await readFile(fileURLToPath(url));return new THREE.Texture();};
const rows=[];
try {
 for(const id of ids){
  const base=new URL(`web/platform/assets/${id}/`,root),bytes=await readFile(new URL('model.json',base)),meta=JSON.parse(bytes),plan=JSON.parse(await readFile(new URL('model-plan.json',base)));
  assert.equal(meta.artifacts['geometry.bin'],plan.appearanceRebuild.nativeGeometrySha256);
  for(const [file,sha256]of Object.entries(meta.artifacts))assert.equal(hash(await readFile(new URL(file,base))),sha256,file);
  assert.deepEqual(meta.derivedParts,plan.derivedParts);
  const model=createManufacturerModel(products.find(p=>p.id===id),new THREE.Texture());await model.ready;model.group.updateMatrixWorld(true);
  assert.equal(model.group.userData.inspectionError,undefined);
  const markerNames=[];model.group.traverse(o=>{if(o.name.includes('-unknown-'))markerNames.push(o.name);});
  assert(markerNames.some(n=>n.endsWith('-back')),'Unknown reverse remains marked');
  if(id==='m5stack-c126-ext'){
   assert(!markerNames.some(n=>n.endsWith('-front')),'Known exposed front has no UNKNOWN overlay');
   const spec=meta.derivedParts.find(p=>p.id==='proto-pcb-photo').geometry,photo=model.parts['proto-pcb-photo'],carrier=model.parts['proto-board-carrier'];
   assert.equal(spec.holesMm.length,30);
   for(const hole of spec.holesMm){const x=hole.reduce((s,p)=>s+p[0],0)/hole.length,y=hole.reduce((s,p)=>s+p[1],0)/hole.length,ray=new THREE.Raycaster(new THREE.Vector3(x,y,100),new THREE.Vector3(0,0,-1));assert.equal(ray.intersectObject(photo).length,0,'Source-photo hole remains open');assert.equal(ray.intersectObject(carrier).length,0,'Estimated carrier hole remains open');}
   assert(new THREE.Raycaster(new THREE.Vector3(-7,0,100),new THREE.Vector3(0,0,-1)).intersectObject(photo).length>0,'PROTO printed region is supported');
  }
  if(id==='m5stack-k125'){
   const paper=model.parts['back-paper'],point=paper.getWorldPosition(new THREE.Vector3());
   const hit=new THREE.Raycaster(point.clone().add(new THREE.Vector3(0,0,-100)),new THREE.Vector3(0,0,1)).intersectObjects(model.group.children,true).find(h=>h.object.userData.inputOccluder);
   assert.equal(hit?.object,paper,'Rear paper covers the exposed center slot');
   const start=paper.position.clone();model.group.userData.inspection.setExpanded(true);
   assert.deepEqual(paper.position.toArray(),start.clone().add(new THREE.Vector3(0,0,-19)).toArray(),'Paper follows the native rear shell');
   model.group.userData.inspection.setExpanded(false);assert(paper.position.equals(start));
  }
  const shells=new Set(model.group.userData.shellMeshes),fixed=[];
  model.group.traverse(o=>{if(o.isMesh&&!shells.has(o))fixed.push([o,o.material.color.getHex(),o.material.opacity,o.material.map]);});
  for(const shell of shells)assert.equal(shell.material.onBeforeCompile,THREE.Material.prototype.onBeforeCompile,'No custom source shader');
  for(const shell of [{color:'#d4af37',opacity:1,finish:'metal'},{color:'#8054b3',opacity:.35,finish:'plastic'}]){
   for(const mesh of shells)applyShellAppearance(mesh,{shell});
   for(const [mesh,color,opacity,map]of fixed){assert.equal(mesh.material.color.getHex(),color);assert.equal(mesh.material.opacity,opacity);assert.equal(mesh.material.map,map);}
  }
  const poses=model.group.children.map(o=>[o,o.position.clone(),o.quaternion.clone()]);model.group.userData.inspection?.setExpanded(true);model.group.userData.inspection?.setExpanded(false);
  for(const [o,position,quaternion]of poses){assert(o.position.equals(position));assert(o.quaternion.equals(quaternion));}
  rows.push({id,modelSha256:hash(bytes),nativeGeometrySha256:meta.artifacts['geometry.bin'],artifactHashesVerified:true,unknownMarkers:markerNames,photoHoles:id==='m5stack-c126-ext'?30:null,fixedMaterialsPreserved:['original','gold','translucent-purple'],inspectionRestoreExact:true});model.dispose();console.log(id+' open face pass');
 }
 const output=new URL('docs/platform/evidence/m5-open-face-20261126/loader-checks.json',root);await mkdir(new URL('.',output),{recursive:true});await writeFile(output,JSON.stringify({scope:'Actual production loader/materials/rays. Pixel appearance is assessed separately by production mesh CPU renders; no browser GPU, physical fit or factory firmware acceptance.',rows},null,2)+'\n');
}finally{globalThis.fetch=old.fetch;globalThis.document=old.document;THREE.TextureLoader.prototype.loadAsync=old.load;}
