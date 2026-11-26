import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {applyShellAppearance} from '../web/platform/shell-appearance.js';
import {applyServoValues} from '../web/platform/servo-view.js';
const ids=process.argv.slice(3);if(!ids.length)ids.push('m5stack-k016-b','m5stack-k016-p2','m5stack-k117','m5stack-k151');
const root=new URL('../web/platform/',import.meta.url),catalog=JSON.parse(await readFile(new URL('catalog/products.json',root))).items,rows=[],sha=b=>createHash('sha256').update(b).digest('hex');
const old={fetch:globalThis.fetch,document:globalThis.document,load:THREE.TextureLoader.prototype.loadAsync};
try {
 globalThis.fetch=async url=>new Response(await readFile(new URL(url)));
 globalThis.document={createElement:()=>({getContext:()=>({fillText(text){assert.equal(text,'UNKNOWN');}})})};
 THREE.TextureLoader.prototype.loadAsync=async url=>{await readFile(new URL(url));return new THREE.Texture();};
 for(const id of ids){
  const base=new URL(`assets/${id}/`,root),bytes=await readFile(new URL('model.json',base)),meta=JSON.parse(bytes),plan=JSON.parse(await readFile(new URL('model-plan.json',base))),product=catalog.find(p=>p.id===id),framebuffer=new THREE.Texture(),model=createManufacturerModel(product,framebuffer);
  try {
   await model.ready;model.group.updateMatrixWorld(true);assert.equal(model.group.userData.inspectionError,undefined);
   for(const [name,hash]of Object.entries(meta.artifacts))assert.equal(sha(await readFile(new URL(name,base))),hash,id+' '+name);
   const meshes=[];model.group.traverse(m=>{if(m.isMesh)meshes.push(m);});
   const nativePockets=id==='m5stack-k016-b'?plan.frontRecessContours:plan.parts.find(p=>p.name===(id==='m5stack-k117'?'shell-front':'button-a-cap'))?.geometry?.subtract?.filter(p=>p.shape==='plate').map(p=>p.outlineMm);
   if(id!=='m5stack-k151'){
    assert.equal(model.parts['front-logo'],undefined,'No substituted font plane');assert.equal(nativePockets.length,2);
    const surface=model.parts[id==='m5stack-k016-b'?'front-bezel':id==='m5stack-k117'?'shell-front':'button-a-cap'],floor=id==='m5stack-k016-b'?6.75:id==='m5stack-k117'?5.075:6.81;
    for(const points of nativePockets){const vertices=points.map(p=>new THREE.Vector2(...p)),face=THREE.ShapeUtils.triangulateShape(vertices,[])[0],point=face.map(i=>vertices[i]).reduce((sum,p)=>sum.add(p),new THREE.Vector2()).multiplyScalar(1/3),ray=new THREE.Raycaster(new THREE.Vector3(point.x,point.y,60),new THREE.Vector3(0,0,-1));assert.ok(Math.abs(ray.intersectObject(surface,false)[0].point.z-floor)<.00002,'Real recessed glyph floor');}
   }
   if(id==='m5stack-k117'){
    const sleeve=model.parts['usb-a-sleeve'];for(const x of [-2.8,2.8])assert.equal(new THREE.Raycaster(new THREE.Vector3(x,-20,60),new THREE.Vector3(0,0,-1)).intersectObject(sleeve,false).length,0,'Lock window open on both sides');
    assert.equal(new THREE.Raycaster(new THREE.Vector3(0,-50,.5),new THREE.Vector3(0,1,0)).intersectObject(sleeve,false).length,0,'USB end is open');assert.equal(model.parts['button-cap'].userData.control,'button');
   }
   const fixed=meshes.filter(m=>!model.group.userData.shellMeshes.includes(m)).map(m=>({m,color:m.material.color.getHex(),opacity:m.material.opacity,map:m.material.map}));
   for(const preset of [{color:'#d4af37',opacity:1,finish:'metal'},{color:'#8054b3',opacity:.35,finish:'plastic'}]){
    for(const shell of model.group.userData.shellMeshes)applyShellAppearance(shell,{shell:preset});for(const row of fixed){assert.equal(row.m.material.color.getHex(),row.color);assert.equal(row.m.material.opacity,row.opacity);assert.equal(row.m.material.map,row.map);}
   }
   let unknownView=null,inspection='not-declared';
   if(id==='m5stack-k151'){
    const photo=model.parts['pcb-power-back-photo'],board=model.parts['pcb-power'];assert.ok(photo&&board.getObjectByName('pcb-power-unknown-front'));assert.equal(board.getObjectByName('pcb-power-unknown-back'),undefined);
    for(const values of [[{id:'pan',positionDegrees:31},{id:'tilt',positionDegrees:-19}],[]]){applyServoValues(model.group.userData.servoJoints,values);model.group.updateMatrixWorld(true);assert.ok(board.localToWorld(new THREE.Vector3(0,0,-.515)).distanceTo(photo.getWorldPosition(new THREE.Vector3()))<1e-8,'Rear photo follows actual power-board joints');}
   }else{
    assert.ok(model.group.userData.inspection);const initial=model.group.children.map(o=>({o,p:o.position.toArray(),q:o.quaternion.toArray()}));model.group.userData.inspection.setProgress(1);model.group.updateMatrixWorld(true);
    const board=model.parts['pcb-main'],marker=board.getObjectByName('pcb-main-unknown-front');assert.ok(marker);const {width,height}=marker.geometry.parameters,points=[[0,0],[width*.4,height*.4],[-width*.4,height*.4],[width*.4,-height*.4],[-width*.4,-height*.4]].map(([x,y])=>marker.localToWorld(new THREE.Vector3(x,y,0)));
    for(const x of [-3,-2,-1,-.85,-.75,-.6,.6,.75,.85,1,2,3])for(const y of [-3,-1,-.5,0,.5,1,3]){const d=new THREE.Vector3(x,y,1).normalize();if(points.every(target=>new THREE.Raycaster(target.clone().addScaledVector(d,200),d.clone().negate()).intersectObjects(meshes,false)[0]?.object===marker))unknownView=d.toArray();}assert.ok(unknownView,id+' complete UNKNOWN plane visible in one inspection direction');
    for(let i=0;i<3;i++){model.group.userData.inspection.setProgress(0);model.group.userData.inspection.setProgress(1);}model.group.userData.inspection.setProgress(0);for(const {o,p,q}of initial){assert.deepEqual(o.position.toArray(),p);assert.deepEqual(o.quaternion.toArray(),q);}inspection='complete-marker-five-rays-and-exact-reset';
   }
   if(product.display)assert.equal(model.screen.material.map,framebuffer);
   rows.push({id,modelSha256:sha(bytes),planSha256:sha(await readFile(new URL('model-plan.json',base))),geometrySha256:meta.artifacts[meta.binary],artifacts:Object.keys(meta.artifacts).length,sourceTracedRecesses:id==='m5stack-k151'?null:2,fixedMaterialIsolation:true,inspection,unknownInspectionDirection:unknownView,photoFollowsBoardServos:id==='m5stack-k151',limits:id==='m5stack-k151'?['Articulated model has no internal inspection controller; inward UNKNOWN preserved, no full disassembly acceptance.']:[]});
  }finally{model.dispose();framebuffer.dispose();}
 }
}finally{globalThis.fetch=old.fetch;THREE.TextureLoader.prototype.loadAsync=old.load;if(old.document===undefined)delete globalThis.document;else globalThis.document=old.document;}
const receipt={scope:'Real production Three geometry/material/joint/inspection checks. Texture decoding is separate in CPU renders; no firmware or physical fit acceptance.',products:rows};if(process.argv[2])await writeFile(process.argv[2],JSON.stringify(receipt,null,2)+'\n');console.log(JSON.stringify(receipt,null,2));
