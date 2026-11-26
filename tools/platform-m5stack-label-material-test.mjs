import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {applyShellAppearance} from '../web/platform/shell-appearance.js';
const nativeFetch=globalThis.fetch,nativeLoad=THREE.TextureLoader.prototype.loadAsync,nativeDocument=globalThis.document;
const ids=['m5stack-c126','m5stack-c126-echo','m5stack-c008-c','m5stack-c008-b','m5stack-k138'];
try {
 globalThis.fetch=async url=>{try{return new Response(await readFile(fileURLToPath(url)));}catch{return new Response('',{status:404});}};
 THREE.TextureLoader.prototype.loadAsync=async url=>{await readFile(fileURLToPath(url));return new THREE.Texture();};
 globalThis.document={createElement:()=>({getContext:()=>({fillText(){}})})};
 const {items}=JSON.parse(await readFile(new URL('../web/platform/catalog/products.json',import.meta.url)));
 for(const id of ids){
  const model=createManufacturerModel(items.find(p=>p.id===id),new THREE.Texture());await model.ready;
  const shells=new Set(model.group.userData.shellMeshes),fixed=[];
  model.group.traverse(mesh=>{if(mesh.isMesh&&!shells.has(mesh))fixed.push({mesh,color:mesh.material.color.getHex(),opacity:mesh.material.opacity,map:mesh.material.map});});
  for(const appearance of [{shell:{color:'#c4c9c2',opacity:1,finish:'plastic'}},{shell:{color:'#d4af37',opacity:1,finish:'metal'}},{shell:{color:'#8054b3',opacity:.35,finish:'plastic'}}]){
   for(const shell of shells)applyShellAppearance(shell,appearance);
   for(const {mesh,color,opacity,map} of fixed){assert.equal(mesh.material.color.getHex(),color,id+' fixed part tint '+mesh.name);assert.equal(mesh.material.opacity,opacity,id+' fixed part opacity '+mesh.name);assert.equal(mesh.material.map,map,id+' texture identity '+mesh.name);}
  }
  model.dispose();
 }
} finally {globalThis.fetch=nativeFetch;THREE.TextureLoader.prototype.loadAsync=nativeLoad;if(nativeDocument===undefined)delete globalThis.document;else globalThis.document=nativeDocument;}
console.log('ok five actual M5Stack models: original-like, gold and translucent-purple shell presets preserve paper, PCB, keys and connector materials');
