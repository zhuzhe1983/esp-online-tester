// Real production loader checks; image decoding and CPU rendering are separate.
import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {applyShellAppearance} from '../web/platform/shell-appearance.js';
const root=new URL('../web/platform/',import.meta.url),id='m5stack-c148';
const sha=b=>createHash('sha256').update(b).digest('hex');
const product=JSON.parse(await readFile(new URL('catalog/products.json',root))).items.find(p=>p.id===id);
const base=new URL(`assets/${id}/`,root),bytes=await readFile(new URL('model.json',base)),meta=JSON.parse(bytes),plan=JSON.parse(await readFile(new URL('model-plan.json',base)));
const old={fetch:globalThis.fetch,document:globalThis.document,load:THREE.TextureLoader.prototype.loadAsync};
let model;
try {
 globalThis.fetch=async url=>new Response(await readFile(new URL(url)));
 globalThis.document={createElement:()=>({getContext:()=>({fillText(text){assert.equal(text,'UNKNOWN');}})})};
 THREE.TextureLoader.prototype.loadAsync=async url=>{await readFile(new URL(url));return new THREE.Texture();};
 for(const [name,hash]of Object.entries(meta.artifacts))assert.equal(sha(await readFile(new URL(name,base))),hash,name);
 model=createManufacturerModel(product,null);await model.ready;model.group.updateMatrixWorld(true);
 assert.equal(model.group.userData.inspectionError,undefined);assert.ok(model.group.userData.inspection);
 const label=model.parts['rear-paper'],shell=model.parts['back-shell'];assert.ok(label&&shell);
 const bounds=new THREE.Box3().setFromObject(label);assert.ok(bounds.min.x>-15.0105&&bounds.max.x<15.0105&&bounds.min.y>23.7895&&bounds.max.y<42.5105);
 assert.equal(label.material.alphaTest,.5);assert.equal(label.material.opacity,1);
 const meshes=[];model.group.traverse(o=>{if(o.isMesh)meshes.push(o);});
 for(const x of [-14,-7,0,7,14])for(const y of [24.5,28,33,38,41.5]){
  const origin=new THREE.Vector3(x,y,-70),ray=new THREE.Raycaster(origin,new THREE.Vector3(0,0,1));
  assert.equal(ray.intersectObjects(meshes,false)[0]?.object,label,'Rear paper unobscured at '+[x,y]);
  const hit=ray.intersectObject(shell,false)[0];assert.ok(hit&&Math.abs(hit.point.z+16.2)<.00001,'Native recess supports paper');
 }
 const ext=plan.derivedParts.filter(p=>p.id.startsWith('ext-header-'));
 assert.equal(ext.length,16);assert.ok(ext.every(p=>p.geometry.centerMm[1]+p.geometry.sizeMm[1]/2<23.7895));
 const pads=plan.derivedParts.filter(p=>p.id.startsWith('rear-rubber-pad-')).map(p=>model.parts[p.id]);assert.equal(pads.length,4);
 for(const pad of pads){
  assert.ok(!model.group.userData.shellMeshes.includes(pad));assert.equal(pad.material.color.getHexString(),'222522');
  const at=pad.getWorldPosition(new THREE.Vector3()),ray=new THREE.Raycaster(new THREE.Vector3(at.x,at.y,-70),new THREE.Vector3(0,0,1));
  assert.equal(ray.intersectObjects(meshes,false)[0]?.object,pad);assert.ok(Math.abs(ray.intersectObject(shell,false)[0].point.z+16)<.00001,'Native pad seat');
 }
 const pcb=model.parts['pcb-main'],marker=pcb.getObjectByName('pcb-main-unknown-front');assert.ok(marker);assert.equal(pcb.getObjectByName('pcb-main-unknown-back'),undefined);
 const fixed=meshes.filter(m=>!model.group.userData.shellMeshes.includes(m)).map(m=>({m,map:m.material.map,color:m.material.color.getHex(),opacity:m.material.opacity}));
 for(const preset of [{color:'#d4af37',opacity:1,finish:'metal'},{color:'#8054b3',opacity:.35,finish:'plastic'}]){
  for(const m of model.group.userData.shellMeshes)applyShellAppearance(m,{shell:preset});
  for(const row of fixed){assert.equal(row.m.material.map,row.map);assert.equal(row.m.material.color.getHex(),row.color);assert.equal(row.m.material.opacity,row.opacity);}
 }
 const initial=model.group.children.map(o=>({o,p:o.position.toArray(),q:o.quaternion.toArray()})),labelAt=label.getWorldPosition(new THREE.Vector3()),shellAt=shell.getWorldPosition(new THREE.Vector3());
 model.group.userData.inspection.setExpanded(true);model.group.updateMatrixWorld(true);
 const delta=label.getWorldPosition(new THREE.Vector3()).sub(labelAt);assert.deepEqual(delta.toArray(),shell.getWorldPosition(new THREE.Vector3()).sub(shellAt).toArray());assert.ok(delta.length()>0);
 const unknownTargets=[[0,0],[14,2.8],[-14,2.8],[14,-2.8],[-14,-2.8]].map(([x,y])=>marker.localToWorld(new THREE.Vector3(x,y,0))),unknownDirection=[2,3,1],d=new THREE.Vector3(...unknownDirection).normalize();
 assert.ok(unknownTargets.every(target=>new THREE.Raycaster(target.clone().addScaledVector(d,200),d.clone().negate()).intersectObjects(meshes,false)[0]?.object===marker),'Complete inward UNKNOWN plane stays visible in one inspection direction');
 for(let i=0;i<3;i++){model.group.userData.inspection.setExpanded(false);model.group.userData.inspection.setExpanded(true);}model.group.userData.inspection.setExpanded(false);
 for(const {o,p,q}of initial){assert.deepEqual(o.position.toArray(),p);assert.deepEqual(o.quaternion.toArray(),q);}
 const receipt={productId:id,modelSha256:sha(bytes),planSha256:sha(await readFile(new URL('model-plan.json',base))),artifacts:meta.artifacts,nativeRecessMm:{x:[-15.0105,15.0105],y:[23.7895,42.5105],z:-16.2},paperBoundsMm:[bounds.min.toArray(),bounds.max.toArray()],supportAndVisibilityRays:25,nativeSeatSupportedBlackPads:4,unknownOnlyOnInwardFace:true,unknownInspectionDirection:unknownDirection,unknownInspectionVisiblePoints:5,extBlocksBelowLabel:true,inspectionPaperFollowsRearShell:true,inspectionResetExact:true,fixedMaterialIsolation:['gold','translucent-purple'],limitations:['Image decoding is stubbed here; see independent Cycles CPU render receipts.','EXT socket contacts/depth and photogrammetric alignment remain estimates; no physical fit or firmware acceptance.']};
 if(process.argv[2])await writeFile(process.argv[2],JSON.stringify(receipt,null,2)+'\n');console.log(JSON.stringify(receipt,null,2));
} finally {model?.dispose();globalThis.fetch=old.fetch;THREE.TextureLoader.prototype.loadAsync=old.load;if(old.document===undefined)delete globalThis.document;else globalThis.document=old.document;}
