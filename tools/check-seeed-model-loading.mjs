#!/usr/bin/env node
// Actual Three.js object construction and CPU rays. This is not WebGL/browser acceptance.
import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {parseArgs} from 'node:util';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {applyShellAppearance} from '../web/platform/shell-appearance.js';
const root=fileURLToPath(new URL('../',import.meta.url)),assets=path.join(root,'web/platform/assets');
const {values,positionals}=parseArgs({options:{catalog:{type:'string',default:path.join(root,'web/platform/catalog/products.json')},output:{type:'string',default:path.join(root,'docs/platform/evidence/seeed-expanded-models/loader-checks.json')}},allowPositionals:true});
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const catalogBody=await readFile(values.catalog),catalog=JSON.parse(catalogBody),products=catalog.items.filter(p=>p.id.startsWith('seeed-')&&(!positionals.length||positionals.includes(p.id)));
if(positionals.length)assert.equal(products.length,new Set(positionals).size);
const nativeFetch=globalThis.fetch,rows=[];
globalThis.fetch=async input=>{const url=new URL(input);assert.equal(url.protocol,'file:');const file=fileURLToPath(url);assert.ok(path.resolve(file).startsWith(assets+path.sep));const bytes=await readFile(file);if(url.searchParams.has('sha256'))assert.equal(sha(bytes),url.searchParams.get('sha256'),'Loaded geometry cache identity');return new Response(bytes,{status:200});};
const hitFrom=(objects,origin,direction)=>new THREE.Raycaster(origin,direction,0,5000).intersectObjects(objects,true).find(hit=>hit.object.userData.inputOccluder||hit.object.userData.screen||hit.object.userData.control);
try {
 for(const product of products){
  const model=createManufacturerModel(product,new THREE.Texture()),metadata=await model.ready;model.group.updateMatrixWorld(true);
  assert.equal(Boolean(model.screen),Boolean(product.display),product.id+' display presence');assert.equal(model.group.userData.inspectionError,undefined,product.id+' inspection groups');
  for(const part of metadata.parts)assert.ok(model.parts[part.name],product.id+' missing part '+part.name);
  const inspection=model.group.userData.inspection;if(inspection){const poses=model.group.children.map(o=>[o,o.position.clone()]);inspection.setExpanded(true);assert.ok(poses.some(([o,v])=>!o.position.equals(v)),product.id+' inspection movement');inspection.setExpanded(false);for(const [o,v] of poses)assert.ok(o.position.equals(v),product.id+' inspection restore');}
  const bounds=new THREE.Box3().setFromObject(model.group);assert.ok(!bounds.isEmpty());assert.ok([...bounds.min.toArray(),...bounds.max.toArray()].every(Number.isFinite));
  const screenRays=[];
  if(model.screen){const screen=model.screen,normal=new THREE.Vector3(0,0,1).transformDirection(screen.matrixWorld),[w,h]=metadata.presentation.screen.sizeMm;
   for(const u of [-.4,0,.4])for(const v of [-.4,0,.4]){if(metadata.presentation.screen.shape==='circle'&&u*u+v*v>.24)continue;const point=screen.localToWorld(new THREE.Vector3(u*w,v*h,0)),hit=hitFrom(model.group.children,point.clone().addScaledVector(normal,1000),normal.clone().negate());assert.equal(hit?.object,screen,product.id+' framebuffer ray '+[u,v]);screenRays.push({uv:[u,v],firstHit:'live-display'});}
  }
  const controlRays=[];
  for(const button of model.buttons){
   const id=button.userData.control;assert.ok(product.controls.some(c=>c.id===id&&c.type==='button'),product.id+' control '+id);
   const positions=button.geometry.getAttribute('position'),index=button.geometry.index,count=index?index.count:positions.count,candidates=[];
   const vertex=i=>new THREE.Vector3().fromBufferAttribute(positions,index?index.getX(i):i).applyMatrix4(button.matrixWorld);
   for(let at=0;at<count;at+=3){const a=vertex(at),b=vertex(at+1),c=vertex(at+2),n=b.clone().sub(a).cross(c.clone().sub(a)),area=n.length();if(area>1e-8)candidates.push({point:a.add(b).add(c).multiplyScalar(1/3),normal:n.normalize(),area});}
   candidates.sort((a,b)=>b.area-a.area);let found=null;
   // Aim at actual exposed triangles, never through a knob's hollow center.
   for(const q of candidates.slice(0,36)){const hit=hitFrom(model.group.children,q.point.clone().addScaledVector(q.normal,1000),q.normal.clone().negate());if(hit?.object===button){found={controlId:id,part:button.name,visibleFromNormal:q.normal.toArray(),firstHit:button.name};break;}}
   if(!found)for(const axis of [new THREE.Vector3(1,0,0),new THREE.Vector3(-1,0,0),new THREE.Vector3(0,1,0),new THREE.Vector3(0,-1,0),new THREE.Vector3(0,0,1),new THREE.Vector3(0,0,-1)]){
    const exposed=candidates.filter(q=>q.normal.dot(axis)>.1).sort((a,b)=>b.point.dot(axis)-a.point.dot(axis)).slice(0,12);
    for(const q of exposed){const hit=hitFrom(model.group.children,q.point.clone().addScaledVector(axis,1000),axis.clone().negate());if(hit?.object===button){found={controlId:id,part:button.name,visibleFromNormal:axis.toArray(),firstHit:button.name};break;}}if(found)break;
   }
   assert.ok(found,product.id+' button has no tested unobstructed surface: '+id);controlRays.push(found);
  }
  const shells=new Set(model.group.userData.shellMeshes||[]),preserved=[];model.group.traverse(o=>{if(o.isMesh&&!shells.has(o)&&!Array.isArray(o.material))preserved.push([o,o.material.color.getHexString(),o.material.opacity]);});
  for(const shell of shells){applyShellAppearance(shell,{shell:{color:'#d6ad45',opacity:1,finish:'metal'}});assert.equal(shell.material.color.getHexString(),'d6ad45');}
  for(const shell of shells){applyShellAppearance(shell,{shell:{color:'#8054b3',opacity:.35,finish:'plastic'}});assert.equal(shell.material.color.getHexString(),'8054b3');assert.equal(shell.material.opacity,.35);}
  for(const [mesh,color,opacity] of preserved){assert.equal(mesh.material.color.getHexString(),color,product.id+' independent material '+mesh.name);assert.equal(mesh.material.opacity,opacity);}
  if(product.id.startsWith('seeed-xiao-epaper-ee'))assert.equal(shells.size,0,product.id+' PCB/panel is not an enclosure');
  rows.push({productId:product.id,modelSha256:sha(await readFile(path.join(assets,product.id,'model.json'))),geometrySha256:metadata.artifacts['geometry.bin'],loader:'pass',inspectionGroups:'pass',modelBoundsMm:[bounds.min.toArray(),bounds.max.toArray()],framebufferMode:model.screen?'present':'absent-no-integrated-display',framebufferRays:screenRays,visibleButtonRays:controlRays,materialIsolation:{goldAndTranslucentPurple:'pass',shellParts:shells.size,independentMeshes:preserved.length},scope:'Actual Three.js loader and CPU raycaster; no WebGL, DOM screenshot, calibrated photo overlay or firmware/hardware acceptance'});
  model.dispose();model.group.traverse(o=>{o.geometry?.dispose();if(o.material)(Array.isArray(o.material)?o.material:[o.material]).forEach(m=>m.dispose());});console.log(product.id+' pass');
 }
 await mkdir(path.dirname(values.output),{recursive:true});await writeFile(values.output,JSON.stringify({checkedAt:new Date().toISOString(),catalogSha256:sha(catalogBody),environment:'Node.js '+process.versions.node,devices:rows},null,2)+'\n');console.log(JSON.stringify({devices:rows.length,framebufferRays:rows.reduce((n,r)=>n+r.framebufferRays.length,0),visibleButtons:rows.reduce((n,r)=>n+r.visibleButtonRays.length,0),status:'pass'}));
}finally{globalThis.fetch=nativeFetch;}
