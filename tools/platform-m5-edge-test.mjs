import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {chipForBoard,validateAppImage} from '../web/platform/app-image.js';
import {applyShellAppearance} from '../web/platform/shell-appearance.js';
const platform=new URL('../web/platform/',import.meta.url);
const catalog=JSON.parse(await readFile(new URL('catalog/products.json',platform))).items;
test('NanoH2 retains H2 SRAM image restrictions, never silently selecting C6/S3',async()=>{
 assert.equal(chipForBoard('nanoh2'),'esp32h2');
 const {waveshareH2CoreProbe}=await import('./fixtures/waveshare-h2-core-probe.mjs');
 const bytes=waveshareH2CoreProbe();await validateAppImage(bytes,'nanoh2',4<<20);
 const other=Buffer.from(bytes);other.writeUInt16LE(13,12);await assert.rejects(validateAppImage(other,'nanoh2',4<<20),e=>e.code==='CHIP_MISMATCH');
 await assert.rejects(validateAppImage(bytes,'nanoh2',8<<20),e=>e.code==='FLASH_CAPACITY_UNSUPPORTED');
});
test('real NanoH2/PowerHub/Tab5 assets keep physical input, paper and inspection relationships',async()=>{
 const nativeFetch=globalThis.fetch,load=THREE.TextureLoader.prototype.loadAsync;
 try {
  globalThis.fetch=async value=>{const u=new URL(value);u.search='';return new Response(await readFile(u));};
  THREE.TextureLoader.prototype.loadAsync=async()=>new THREE.Texture();
  for(const id of ['m5stack-c149','m5stack-c148','m5stack-c145','m5stack-k148','m5stack-u202']){
   const product=catalog.find(x=>x.id===id);assert(product);
   const model=createManufacturerModel(product,null);try{
    await model.ready;assert.equal(model.group.userData.inspectionError,undefined);
    const board=model.parts['pcb-main'],base=board.position.clone();
    const inspection=model.group.userData.inspection;
    assert(inspection,id+' exposes UNKNOWN board via declared inspection');
    inspection.setExpanded(true);assert.deepEqual(board.position.toArray(),base.toArray());inspection.setExpanded(false);
    if(id==='m5stack-c149'){
     const hit=model.parts['cover-key:button-a'];model.group.updateMatrixWorld(true);
     const ray=new THREE.Raycaster(new THREE.Vector3(0,-1,100),new THREE.Vector3(0,0,-1));
     assert.equal(ray.intersectObject(model.group,true)[0].object,hit,'front flexure receives the nearest visible hit');
     const led=model.group.userData.rgbLedMeshes[0];assert.equal(led.name,'rgb-window');
     const slitRay=new THREE.Raycaster(new THREE.Vector3(0,-2.8,100),new THREE.Vector3(0,0,-1));
     assert.equal(slitRay.intersectObject(model.group,true).find(h=>h.object!==hit).object,led,'the emitted pixels are actually visible through the source CAD slit');
     const color=led.material.color.getHexString();for(const shell of model.group.userData.shellMeshes)applyShellAppearance(shell,{shell:{color:'#8054b3',opacity:.35,finish:'plastic'}});
     assert.equal(led.material.color.getHexString(),color);assert.equal(led.material.opacity,1);
    }
    if(id==='m5stack-c148'||id==='m5stack-k148'){
     assert.equal(model.parts['user-button-carrier'].userData.control,'button-user');
     assert(!model.buttons.some(x=>['power-top','power-side'].includes(x.userData.control)),'STM32 PMU buttons have no fabricated GPIO hit mapping');
    }
    if(id==='m5stack-u202')assert.equal(model.parts['side-reset'].userData.control,'button','the native actuator addresses the P0 semantic input');
   }finally{model.dispose();}
  }
 }finally{globalThis.fetch=nativeFetch;THREE.TextureLoader.prototype.loadAsync=load;}
});
