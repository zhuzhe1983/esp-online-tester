#!/usr/bin/env node
// Source-bounded production meshes/rays; does not claim browser or hardware QA.
import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import path from 'node:path';
import {createHash} from 'node:crypto';
import * as THREE from '../../../../../web/platform/vendor/three.module.min.js';
import {createManufacturerModel} from '../../../../../web/platform/manufacturer-model.js';

const root=fileURLToPath(new URL('../../../../../',import.meta.url));
const out=fileURLToPath(new URL('./',import.meta.url));
const ids=['waveshare-esp32-s3-nano','waveshare-esp32-s3-nano-m','waveshare-esp32-s3-pico','waveshare-esp32-s3-pico-m'];
const sha=data=>createHash('sha256').update(data).digest('hex');
const originals={fetch:globalThis.fetch,document:globalThis.document,texture:THREE.TextureLoader.prototype.loadAsync};
globalThis.document={createElement(){return {getContext(){return {fillText(){}};}};}};
globalThis.fetch=async input=>new Response(await readFile(fileURLToPath(new URL(input))),{status:200});
let metadata;
THREE.TextureLoader.prototype.loadAsync=async function(input){const file=fileURLToPath(new URL(input)),bytes=await readFile(file);assert.equal(sha(bytes),metadata.artifacts[path.basename(file)]);return new THREE.Texture();};
const first=(objects,origin,direction)=>new THREE.Raycaster(origin,direction,0,5000).intersectObjects(objects,true).find(({object})=>{for(let p=object;p;p=p.parent)if(!p.visible)return false;return object.material?.opacity!==0;});
const results=[];
try{
 for(const id of ids){
  const folder=path.join(root,'web/platform/assets',id),raw=await readFile(path.join(folder,'model.json'));
  metadata=JSON.parse(raw);const plan=JSON.parse(await readFile(path.join(folder,'model-plan.json')));
  const model=createManufacturerModel({id,model:{url:`./assets/${id}/model.json`},controls:plan.validation.controls},new THREE.Texture());
  await model.ready;model.group.updateMatrixWorld(true);
  const label=model.group.getObjectByName('pcb-main-unknown-back');assert.ok(label?.isMesh);
  const center=label.getWorldPosition(new THREE.Vector3()),normal=new THREE.Vector3(0,0,1).transformDirection(label.matrixWorld);
  assert.equal(first(model.group.children,center.clone().addScaledVector(normal,1000),normal.clone().negate())?.object,label);
  const usb=model.parts['usb-open-envelope'],usbCenter=new THREE.Box3().setFromObject(usb).getCenter(new THREE.Vector3());
  assert.equal(first([usb],usbCenter.clone().add(new THREE.Vector3(0,100,0)),new THREE.Vector3(0,-1,0)),undefined);
  const pins=Object.values(model.parts).filter(p=>p?.name.startsWith('header-pin-'));
  const expected=plan.dimensions.preSolderedHeaderRows.reduce((a,b)=>a+b,0);assert.equal(pins.length,expected);
  const pinCenters=[];
  for(const pin of pins){const p=new THREE.Box3().setFromObject(pin).getCenter(new THREE.Vector3());
   assert.equal(first([model.parts['pcb-main']],p.clone().setZ(100),new THREE.Vector3(0,0,-1)),undefined);
   assert.equal(first([model.parts['pcb-front-photo']],p.clone().setZ(100),new THREE.Vector3(0,0,-1)),undefined);
   pinCenters.push(p.toArray());
  }
  const mounting=[];
  if(id.includes('nano'))for(const x of [-7.62,7.62])for(const y of [-20.32,20.32]){
   assert.equal(first([model.parts['pcb-main']],new THREE.Vector3(x,y,100),new THREE.Vector3(0,0,-1)),undefined);
   assert.equal(first([model.parts['pcb-front-photo']],new THREE.Vector3(x,y,100),new THREE.Vector3(0,0,-1)),undefined);
   mounting.push([x,y]);
  }
  assert.ok(model.parts['reset-physical']&&model.parts['reset-cap']);
  assert.equal(model.parts['reset-cap'].userData.control,undefined);
  assert.deepEqual(model.buttons.map(b=>b.userData.control),id.includes('nano')?[]:['boot']);
  results.push({id,modelSha256:sha(raw),planSha256:sha(await readFile(path.join(folder,'model-plan.json'))),
   visibleUnknown:{label:label.name,normal:normal.toArray(),pointMm:center.toArray(),firstHit:'pcb-main-unknown-back'},
   headerPins:{count:pins.length,rows:plan.dimensions.preSolderedHeaderRows,centersMm:pinCenters,pcbAndPhotoThroughHoleRays:'passed'},
   mountingHoleCentersMm:mounting,openUsbShellAxialHits:0,physicalReset:'visible, no fabricated GPIO interaction; platform restart is separate',
   controls:plan.validation.controls,ledIndices:plan.validation.ledIndices,
   scope:'Actual production Three.js geometry and CPU rays; no browser text-readability, physical-fit or real-hardware claim.'});
  model.dispose();console.log(id+' geometry pass');
 }
 await writeFile(path.join(out,'additional-geometry-checks.json'),JSON.stringify({results},null,2)+'\n');
}finally{globalThis.fetch=originals.fetch;globalThis.document=originals.document;THREE.TextureLoader.prototype.loadAsync=originals.texture;}
