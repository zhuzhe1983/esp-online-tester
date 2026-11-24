#!/usr/bin/env node
// Additional source-bounded checks on production Three meshes, without WebGL.
import assert from 'node:assert/strict';
import {readFile, writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import path from 'node:path';
import {createHash} from 'node:crypto';
import * as THREE from '../../../../../web/platform/vendor/three.module.min.js';
import {createManufacturerModel} from '../../../../../web/platform/manufacturer-model.js';

const root=fileURLToPath(new URL('../../../../../',import.meta.url));
const folder=path.dirname(fileURLToPath(import.meta.url));
const modelIds=["waveshare-esp32-s3-zero-m", "waveshare-esp32-s3-zero-n8r8", "waveshare-esp32-s3-zero-n8r8-m", "waveshare-esp32-s3-dev-kit-n8r8", "waveshare-esp32-s3-dev-kit-n8r8-m", "waveshare-esp32-s3-dev-kit-n16r8", "waveshare-esp32-s3-dev-kit-n16r8-m", "waveshare-esp32-s3-dev-kit-n32r16v", "waveshare-esp32-s3-dev-kit-n32r16v-m", "waveshare-esp32-s3-dev-kit-n16r8-u", "waveshare-esp32-s3-dev-kit-n16r8-um"];
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const original={fetch:globalThis.fetch,document:globalThis.document,texture:THREE.TextureLoader.prototype.loadAsync};
globalThis.document={createElement(){return {getContext(){return {fillText(){}};}};}};
globalThis.fetch=async input=>new Response(await readFile(fileURLToPath(new URL(input))),{status:200});
let metadata;
THREE.TextureLoader.prototype.loadAsync=async function(input){const f=fileURLToPath(new URL(input)),bytes=await readFile(f);assert.equal(sha(bytes),metadata.artifacts[path.basename(f)]);return new THREE.Texture();};
const first=(objects,origin,direction)=>new THREE.Raycaster(origin,direction,0,5000).intersectObjects(objects,true).find(({object})=>{for(let p=object;p;p=p.parent)if(!p.visible)return false;return object.material?.opacity!==0;});
const results=[];
try{
 for(const id of modelIds){
  const dir=path.join(root,'web/platform/assets',id),raw=await readFile(path.join(dir,'model.json'));
  metadata=JSON.parse(raw);const plan=JSON.parse(await readFile(path.join(dir,'model-plan.json')));
  const model=createManufacturerModel({id,model:{url:`./assets/${id}/model.json`},controls:plan.validation?.controls??(id.endsWith("nano")?[]:[{id:"boot",type:"button"}]),display:plan.validation?.display},new THREE.Texture());
  await model.ready;model.group.updateMatrixWorld(true);
  const unknown=[];
  for(const name of plan.validation?.visibleUnknownLabels??(plan.derivedParts?.some(p=>p.kind==='pcb-placeholder')?['pcb-main-unknown-back']:[])){
   const label=model.group.getObjectByName(name);assert.ok(label?.isMesh,name);
   const point=label.getWorldPosition(new THREE.Vector3()),normal=new THREE.Vector3(0,0,1).transformDirection(label.matrixWorld);
   const hit=first(model.group.children,point.clone().addScaledVector(normal,1000),normal.clone().negate());
   assert.equal(hit?.object,label,id+' UNKNOWN face must be visible from its own surface normal');
   unknown.push({name,pointMm:point.toArray(),normal:normal.toArray(),firstHit:hit.object.name});
  }
  const usb=model.parts['usb-open-envelope']||((id.endsWith('-m')&&!id.includes('zero')&&!id.includes('dev-kit'))?model.parts['usb-shell']:null);
  let usbOpening=null;
  if(usb){const box=new THREE.Box3().setFromObject(usb),center=box.getCenter(new THREE.Vector3());assert.equal(first([usb],center.clone().add(new THREE.Vector3(0,100,0)),new THREE.Vector3(0,-1,0)),undefined,id+' USB envelope is actually open');usbOpening={centerMm:center.toArray(),shellAxialHits:0,scope:'Uncalibrated metal shell envelope only; tongue/contact and mating fit remain separate.'};}
  const headerPins=Object.values(model.parts).filter(o=>o?.name.startsWith('header-pin-'));
  let headerCheck=null;
  if(headerPins.length){const expected=plan.dimensions.preSolderedHeaderRows.reduce((a,b)=>a+b,0);assert.equal(headerPins.length,expected);const pcb=model.parts['pcb-main'];assert.ok(pcb);for(const pin of headerPins){const point=new THREE.Box3().setFromObject(pin).getCenter(new THREE.Vector3());assert.equal(first([pcb],point.clone().setZ(100),new THREE.Vector3(0,0,-1)),undefined,id+' pin is centered in a sourced through hole, not a solid PCB');}headerCheck={pins:headerPins.length,rows:plan.dimensions.preSolderedHeaderRows,throughHoleRays:'passed',scope:'Photo-derived count/pitch and geometry continuity; no pin length/connector fit guarantee.'};}
  let endNotch=null;
  if(id.endsWith('-u')||id.endsWith('-um')){const pcb=model.parts['pcb-main'];assert.equal(first([pcb],new THREE.Vector3(0,-29,100),new THREE.Vector3(0,0,-1)),undefined);assert.ok(first([pcb],new THREE.Vector3(0,-25,100),new THREE.Vector3(0,0,-1)));endNotch={openRayMm:[0,-29],solidRayMm:[0,-25],scope:'Photo-estimated U-shaped board-end cutout is physically open in the model; exact notch dimensions unknown.'};}
  results.push({id,modelSha256:sha(raw),visibleUnknownLabels:unknown,usbOpening,headerCheck,endNotch});
  model.dispose();
 }
 await writeFile(path.join(folder,'additional-geometry-checks.json'),JSON.stringify({scope:'Production Three.js object construction and CPU ray checks only; no DOM/WebGL rendering or physical hardware acceptance.',results},null,2)+'\n');
 console.log(`${results.length} models: visible UNKNOWN faces, open USB envelopes and header through-hole rays passed.`);
}finally{globalThis.fetch=original.fetch;globalThis.document=original.document;THREE.TextureLoader.prototype.loadAsync=original.texture;}
