#!/usr/bin/env node
// Geometric/material checks against the production loader; no browser/GPU claims.
import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {applyShellAppearance} from '../web/platform/shell-appearance.js';
const root=resolve(fileURLToPath(new URL('..',import.meta.url))),args=process.argv.slice(2),option=(key,fallback)=>args.includes(key)?args[args.indexOf(key)+1]:fallback;
const ids=option('--ids','m5stack-k162,m5stack-a172,m5stack-m141,m5stack-k140,m5stack-c008').split(','),out=option('--out',null),hash=b=>createHash('sha256').update(b).digest('hex');
const products=JSON.parse(await readFile(resolve(root,'web/platform/catalog/products.json'))).items;
const saved={fetch:globalThis.fetch,document:globalThis.document,load:THREE.TextureLoader.prototype.loadAsync};
const rows=[];
function visible(mesh){for(let p=mesh;p;p=p.parent)if(!p.visible)return false;return mesh.isMesh&&mesh.material?.opacity>.01;}
function appearance(mat){return JSON.stringify({color:mat.color?.toArray(),opacity:mat.opacity,metalness:mat.metalness,roughness:mat.roughness,map:mat.map?.uuid});}
function transforms(group){const result=[];group.traverse(p=>result.push([p.uuid,...p.position.toArray(),...p.quaternion.toArray(),...p.scale.toArray(),p.visible]));return result;}
function first(group,target,direction,ignoreTranslucent=false){const d=new THREE.Vector3(...direction).normalize(),ray=new THREE.Raycaster(target.clone().addScaledVector(d,1000),d.negate());return ray.intersectObjects(group.children,true).find(h=>visible(h.object)&&(!ignoreTranslucent||h.object.material.opacity>.99));}
function readableDirections(group,label){const p=label.geometry.getAttribute('position'),box=new THREE.Box3().setFromBufferAttribute(p),c=box.getCenter(new THREE.Vector3()),s=box.getSize(new THREE.Vector3());const points=[[0,0],[-.42,-.3],[.42,-.3],[.42,.3],[-.42,.3]].map(([x,y])=>label.localToWorld(new THREE.Vector3(c.x+x*s.x,c.y+y*s.y,0)));const directions=[[0,0,1],[0,0,-1],[1.7,1.1,3],[-1.7,1.1,3],[1.7,-1.1,3],[-1.7,-1.1,3],[1.7,1.1,-3],[-1.7,1.1,-3],[3,1,1],[-3,1,1],[3,-1,1],[-3,-1,1],[3,1,-1],[-3,1,-1]];return directions.map(direction=>({direction,visibleProbes:points.filter(t=>first(group,t,direction,true)?.object===label).length}));}
try{
 globalThis.fetch=async url=>{try{return new Response(await readFile(fileURLToPath(url)));}catch{return new Response('',{status:404});}};
 globalThis.document={createElement:()=>({width:0,height:0,getContext:()=>({fillText(){},fillStyle:'#fff',font:'',textAlign:'',textBaseline:''})})};
 THREE.TextureLoader.prototype.loadAsync=async url=>{await readFile(fileURLToPath(url));return new THREE.Texture();};
 for(const id of ids){
  const folder=resolve(root,'web/platform/assets',id),plan=JSON.parse(await readFile(resolve(folder,'model-plan.json'))),bytes=await readFile(resolve(folder,'model.json')),metadata=JSON.parse(bytes),product=products.find(p=>p.id===id);assert.ok(product,id);
  for(const [file,expected]of Object.entries(plan.visibleFaceReview.sourceHashes))assert.equal(hash(await readFile(resolve(folder,file))),expected,file);
  const model=createManufacturerModel(product,new THREE.Texture());
  try{
   await model.ready;assert.equal(model.group.userData.inspectionError,undefined,id);model.group.updateMatrixWorld(true);
   const group=model.group,pcb=model.parts['pcb-main'];assert.equal(pcb.userData.kind,'pcb-placeholder');
   const labels=[];pcb.traverse(p=>{if(p.name.includes('-unknown-'))labels.push(p);});assert.ok(labels.length,'UNKNOWN retained');
   const meshes=[];group.traverse(p=>{if(p.isMesh&&!Array.isArray(p.material))meshes.push(p);});
   const shellSet=new Set(group.userData.shellMeshes||Object.values(model.parts).filter(m=>m?.isMesh&&m.userData.originalAppearance?.shell)),baseline=new Map(meshes.map(m=>[m,appearance(m.material)])),materialChecks=[],originalShellMaterials=new Map([...shellSet].map(m=>[m,m.material.clone()]));
   for(const [name,shell]of Object.entries({gold:{color:'#d4af37',opacity:1,finish:'metal'},purple:{color:'#8054b3',opacity:.35,finish:'plastic'}})){
    for(const mesh of shellSet)applyShellAppearance(mesh,{shell});
    const changed=meshes.filter(m=>appearance(m.material)!==baseline.get(m));assert.ok(changed.every(m=>shellSet.has(m)),'Non-shell material changed');if(shellSet.size)assert.ok(changed.length,'Shell appearance failed');materialChecks.push({name,shellMeshes:shellSet.size,changedMeshes:changed.map(m=>m.name),nonShellUnchanged:meshes.filter(m=>!shellSet.has(m)).length});
   }
   const inspection=group.userData.inspection,assembled=transforms(group),states=[];
   for(const mode of ['original','purple']){
    inspection?.setProgress(0);
    for(const mesh of shellSet){if(mode==='original')mesh.material=originalShellMaterials.get(mesh);else applyShellAppearance(mesh,{shell:{color:'#8054b3',opacity:.35,finish:'plastic'}});}
    for(const progress of inspection?[0,.5,1]:[0]){inspection?.setProgress(progress);group.updateMatrixWorld(true);states.push({appearance:mode,progress,labels:labels.map(label=>({name:label.name,directions:readableDirections(group,label)}))});}
   }
   assert.ok(states.some(s=>s.appearance==='original'&&s.labels.some(l=>l.directions.some(d=>d.visibleProbes===5))),id+' UNKNOWN lacks an unobstructed original-material reachable view: '+JSON.stringify(states));
   if(inspection){for(let i=0;i<8;i++){inspection.setProgress(.5);inspection.setProgress(1);inspection.setProgress(0);}assert.deepEqual(transforms(group),assembled,'Inspection reset drift');}
   const probes=[];
   if(id==='m5stack-k162'){
    // Restore opacity only on the rear closure for this opaque-face check.
    const closure=model.parts['rear-closure-photo-estimate'];closure.material.opacity=1;for(const xy of [[0,0],[12,0],[-12,0],[0,12],[0,-12]]){const target=new THREE.Vector3(...xy,0),h=first(group,target,[0,0,-1]);assert.equal(h?.object.name,closure.name);probes.push({xy,first:h.object.name});}
   }
   if(id==='m5stack-m141'){
    const print=model.parts['front-print'],h=first(group,new THREE.Vector3(0,0,0),[0,0,1]);assert.equal(h?.object,print);probes.push({frontCenterFirst:h.object.name});
   }
   if(id==='m5stack-c008'){
    const back=model.parts['shell-back'],ray=new THREE.Raycaster(new THREE.Vector3(0,0,-100),new THREE.Vector3(0,0,1));assert.equal(ray.intersectObject(back,false).length,0,'Native rear aperture closed');assert.equal(labels.length,1);assert.equal(labels[0].name,'pcb-main-unknown-front');assert.equal(model.parts['usb-c'].userData.kind,'connector-visual');probes.push({rearCenterIntersectsNativeShell:false,unknownOnFront:true,usbIsIndependentRecessedVisual:true});
   }
   if(id==='m5stack-a172'){assert.ok(model.parts['pcb-back-photo']);assert.ok(model.parts['radio-back-marking']);assert.equal(labels[0].name,'pcb-main-unknown-front');assert.equal(plan.derivedParts.find(p=>p.id==='pcb-main').geometry.holesMm.length,1);probes.push({backsidePhotoSurfaces:2,confirmedThroughHoles:1,radioUnknownLayerRetained:true});}
   if(id==='m5stack-k140'){assert.ok(!metadata.derivedParts.some(p=>/pcb.*photo/.test(p.id)));probes.push({pcbPhotographProjected:false,unknownRetained:true,substrateColor:plan.derivedParts.find(p=>p.id==='pcb-main').color});}
   rows.push({id,modelSha256:hash(bytes),binarySha256:hash(await readFile(resolve(folder,metadata.binary))),sourceHashes:plan.visibleFaceReview.sourceHashes,materialChecks,inspectionReset:inspection?'passed-eight-cycles':'no-declared-inspection',unknownVisibility:states,probes,status:'passed'});
  }finally{model.dispose();}
 }
}finally{globalThis.fetch=saved.fetch;globalThis.document=saved.document;THREE.TextureLoader.prototype.loadAsync=saved.load;}
const receipt={schemaVersion:1,scope:'Actual production Three meshes, materials, source hashes, rays and public inspection controller. Rays ignore translucent shell occlusion but respect opaque parts; text sharpness is separately checked in CPU images. No GPU/browser or physical fit/firmware acceptance.',products:rows};if(out){await mkdir(dirname(resolve(out)),{recursive:true});await writeFile(out,JSON.stringify(receipt,null,2)+'\n');}console.log(JSON.stringify({checked:rows.map(r=>r.id),status:'passed',out}));
