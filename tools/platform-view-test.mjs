#!/usr/bin/env node
// Resource-lifecycle regression without a GPU. Browser QA also checks actual WebGL uploads.
import assert from 'node:assert/strict';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import { createSceneView, resizeFramebufferTexture } from '../web/platform/scene-view.js';

const canvas = { width: 466, height: 466 };
let texture = new THREE.CanvasTexture(canvas);
texture.colorSpace = THREE.SRGBColorSpace;
texture.minFilter = THREE.LinearFilter;
texture.magFilter = THREE.NearestFilter;
texture.generateMipmaps = false;
texture.flipY = false;
const material = new THREE.MeshBasicMaterial({ map: texture });
let disposed = 0;
texture.addEventListener('dispose', () => { disposed++; });

const initial = texture;
texture = resizeFramebufferTexture(canvas, texture, 466, 466, material);
assert.equal(texture, initial);
assert.equal(disposed, 0);
console.log('ok same-size frames reuse their texture and do not dispose GPU storage');

texture = resizeFramebufferTexture(canvas, texture, 468, 466, material);
assert.notEqual(texture, initial);
assert.equal(disposed, 1);
assert.equal(material.map, texture);
assert.equal(texture.image, canvas);
assert.deepEqual([canvas.width, canvas.height], [468, 466]);
assert.equal(texture.colorSpace, THREE.SRGBColorSpace);
assert.equal(texture.minFilter, THREE.LinearFilter);
assert.equal(texture.magFilter, THREE.NearestFilter);
assert.equal(texture.generateMipmaps, false);
assert.equal(texture.flipY, false);
console.log('ok first native StopWatch frame replaces storage and preserves texture settings');

const stopwatch = texture;
stopwatch.addEventListener('dispose', () => { disposed++; });
texture = resizeFramebufferTexture(canvas, texture, 480, 800, material);
assert.notEqual(texture, stopwatch);
assert.equal(disposed, 2);
assert.equal(material.map, texture);
assert.deepEqual([canvas.width, canvas.height], [480, 800]);
const paper = texture;
for (let frame = 0; frame < 100; frame++) texture = resizeFramebufferTexture(canvas, texture, 480, 800, material);
assert.equal(texture, paper);
assert.equal(disposed, 2);
console.log('ok PaperMono switching replaces the texture once; following frames reuse it');

paper.addEventListener('dispose', () => { disposed++; });
texture = resizeFramebufferTexture(canvas, texture, 468, 466, material);
assert.notEqual(texture, paper);
assert.equal(material.map, texture);
assert.equal(disposed, 3);
assert.deepEqual([canvas.width, canvas.height], [468, 466]);
texture.dispose(); material.dispose();
console.log('ok returning to StopWatch replaces the larger texture and updates the screen material');
console.log('Passed 4 framebuffer texture lifecycle checks.');

// Exercise the real async model loader with an exposed shell and a pending label.
// Only DOM/GPU plumbing and asset I/O are replaced; appearance and materials are real.
const globals = ['document','window','ResizeObserver','requestAnimationFrame','fetch'];
const saved = Object.fromEntries(globals.map(key=>[key,globalThis[key]]));
const originalAdd = THREE.Group.prototype.add, originalLoad = THREE.TextureLoader.prototype.loadAsync;
const element = () => ({style:{},dataset:{},classList:{toggle(){}},setAttribute(){},removeAttribute(){},addEventListener(){},removeEventListener(){},append(){},replaceChildren(){},getContext(){return {fillRect(){}};},clientWidth:500,clientHeight:450});
const vertices = new Float32Array([-1,-1,0,0,0,1,1,-1,0,0,0,1,0,1,0,0,0,1]);
const fixture = {unit:'mm',strideFloats:6,binary:'geometry.bin',parts:[
  {name:'shell',role:'shell',color:'#b8bcba',byteOffset:0,vertexCount:3},
  {name:'bottom-hook',role:'shell',color:'#a5aaa7',byteOffset:0,vertexCount:3},
],derivedParts:[{kind:'label',id:'paper-label',texture:'label.png',geometry:{sizeMm:[1,1]}}]};
const product = {id:'test-delayed-shell',model:{url:'./assets/test-delayed-shell/model.json'}};
let view;
try {
  globalThis.document = {createElement:element,createElementNS(){throw Error('No GPU in this lifecycle test');}};
  globalThis.window = {devicePixelRatio:1,addEventListener(){},removeEventListener(){}};
  globalThis.ResizeObserver = class {observe(){} disconnect(){}};
  globalThis.requestAnimationFrame = () => 1;
  globalThis.fetch = async url => new Response(new URL(url).pathname.endsWith('model.json')?JSON.stringify(fixture):vertices.buffer);
  let model, releaseTexture, textureStarted;
  THREE.Group.prototype.add = function(...objects){for(const object of objects)if(object.name===product.id)model=object;return originalAdd.apply(this,objects);};
  const mount=element(); view=createSceneView(mount);
  async function loading(){
    const started=new Promise(resolve=>textureStarted=resolve);
    THREE.TextureLoader.prototype.loadAsync=()=>new Promise(resolve=>{releaseTexture=()=>resolve(new THREE.Texture());textureStarted();});
    view.setProduct(product);await started;assert.equal(mount.dataset.modelStatus,'loading');return model.getObjectByName('shell');
  }
  async function ready(){releaseTexture();for(let i=0;i<20&&mount.dataset.modelStatus==='loading';i++)await new Promise(resolve=>setImmediate(resolve));assert.equal(mount.dataset.modelStatus,'ready');}
  let shell=await loading(); view.setAppearance(null);
  assert.equal(shell.material.color.getHexString(),'b8bcba','Rendering default appearance during label loading must not replace manufacturer color');
  await ready();assert.equal(view.getAppearance().shell.color,'#b8bcba');
  assert.equal(shell.userData.originalAppearance.shell.color,'#b8bcba');
  view.setAppearance({shell:{color:'#cc8844',opacity:.35,finish:'metal'}});
  assert.equal(shell.material.color.getHexString(),'cc8844');assert.equal(shell.material.opacity,.35);
  view.setAppearance(null);assert.equal(shell.material.color.getHexString(),'b8bcba');assert.equal(model.getObjectByName('bottom-hook').material.color.getHexString(),'a5aaa7');
  console.log('ok delayed original appearance stays untouched, customizes after ready and resets each shell to its own color');
  shell=await loading();view.setAppearance({shell:{color:'#663399',opacity:.4,finish:'glossy-plastic'}});await ready();
  assert.equal(shell.material.color.getHexString(),'663399');assert.equal(shell.material.opacity,.4);assert.equal(shell.material.roughness,.14);assert.equal(shell.material.clearcoat,.7);
  view.setAppearance(null);assert.equal(shell.material.color.getHexString(),'b8bcba');assert.equal(shell.material.roughness,.66);assert.equal(shell.material.clearcoat,0);
  shell=await loading();view.setAppearance({shell:{opacity:.25}});await ready();
  assert.equal(shell.material.color.getHexString(),'b8bcba','Partial preferences must resolve against loaded model defaults');assert.equal(shell.material.opacity,.25);
  shell=await loading();view.setAppearance({shell:{color:'#663399'}});view.setAppearance(null);await ready();assert.equal(shell.material.color.getHexString(),'b8bcba');
  console.log('ok custom, partial and reset requests made while loading resolve after manufacturer defaults arrive');
} finally {
  view?.dispose();THREE.Group.prototype.add=originalAdd;THREE.TextureLoader.prototype.loadAsync=originalLoad;
  for(const key of globals)if(saved[key]===undefined)delete globalThis[key];else globalThis[key]=saved[key];
}
