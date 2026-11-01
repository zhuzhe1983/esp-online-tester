import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import crypto from 'node:crypto';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {pickVisibleSurface} from '../web/platform/scene-interaction.js';
import {applyShellAppearance} from '../web/platform/shell-appearance.js';
const base=new URL('../web/platform/assets/m5stack-k128/',import.meta.url),meta=JSON.parse(await fs.readFile(new URL('model.json',base))),contract=JSON.parse(await fs.readFile(new URL('front-assembly.json',base))),binary=await fs.readFile(new URL('geometry.bin',base));
const optics=['camera-bezel','camera-window','camera-lens','microphone-left','microphone-right','ltr-window-1','ltr-window-2'];
async function load(){const original=globalThis.fetch;try{globalThis.fetch=async url=>new Response(String(url).includes('model.json')?JSON.stringify(meta):binary);const model=createManufacturerModel({id:'m5stack-k128',model:{url:'./assets/m5stack-k128/model.json'},display:{width:320,height:240}},null);await model.ready;return model;}finally{globalThis.fetch=original;}}
function dispose(m){m.dispose();m.group.traverse(o=>{o.geometry?.dispose();for(const mat of [].concat(o.material||[]))mat.dispose();});}
function hit(m,x,y){return pickVisibleSurface(new THREE.Raycaster(new THREE.Vector3(x,y,100),new THREE.Vector3(0,0,-1)),m.group)?.object;}

test('DXF-defined front optical apertures remain visible and do not cover any active LCD area',async()=>{
 const m=await load();try {
  for(const [x,y,name] of [[0,-21,'camera-lens'],[-14,-20.25,'microphone-left'],[14,-20.25,'microphone-right'],[6.58,-21,'ltr-window-1'],[8.98,-21,'ltr-window-2']])assert.equal(hit(m,x,y)?.name,name);
  for(const x of [-20.31,0,20.31])for(const y of [-15.23,0,15.23])assert.equal(hit(m,x,y),m.screen,`LCD point ${x},${y} must hit the live texture`);
  assert.deepEqual(m.screen.position.toArray(),[0,0,15.85]);
  assert.equal(hit(m,0,-19.3)?.name,'camera-bezel');
 }finally{dispose(m);}
});

test('Gold and transparent cases preserve camera, microphones, LTR and glass materials',async()=>{
 const m=await load();try {
  const names=['front-glass','sensor-window-support',...optics],before=Object.fromEntries(names.map(n=>[n,{color:m.parts[n].material.color.getHex(),opacity:m.parts[n].material.opacity}]));
  for(const shell of [{color:'#caa34e',opacity:1,finish:'metal'},{color:'#8456cf',opacity:.25,finish:'plastic'}]) {
   for(const mesh of m.group.userData.shellMeshes)applyShellAppearance(mesh,{shell});
   for(const name of names){assert.equal(m.parts[name].material.color.getHex(),before[name].color);assert.equal(m.parts[name].material.opacity,before[name].opacity);}
   assert.equal(hit(m,0,-21)?.name,'camera-lens');assert.equal(hit(m,0,0),m.screen);
  }
 }finally{dispose(m);}
});

test('Head reuse contract pins source bytes and excludes DIN hardware, board placeholders and host connectors',()=>{
 assert.equal(contract.binarySha256,crypto.createHash('sha256').update(binary).digest('hex'));
 assert.deepEqual(contract.parts.map(p=>p.name),['shell-front','sensor-window-support','front-glass']);
 assert.deepEqual(contract.assemblyPartNames,['shell-front','sensor-window-support','front-glass',...optics,'live-display']);
 for(const name of ['din-base','foot-a','foot-b','foot-c','foot-d','din-latch','pcb-main','usb-c','grove-a'])assert.ok(contract.excludedParts.includes(name));
 assert.equal(contract.anchors.shellFrontPlane.positionMm[2]-contract.anchors.rearMatingPlane.positionMm[2],15.5);
 const sourcePart=meta.parts.find(p=>p.name==='shell-front'),selected=contract.parts.find(p=>p.name==='shell-front');assert.deepEqual(selected,sourcePart);
});
