import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {Texture,TextureLoader} from '../web/platform/vendor/three.module.min.js';
import {applyShellAppearance} from '../web/platform/shell-appearance.js';
const root=new URL('../web/platform/',import.meta.url),products=JSON.parse(await readFile(new URL('catalog/products.json',root))).items;
const ids=['m5stack-u007','m5stack-u082','m5stack-u109-x','m5stack-u133'];
const nativeFetch=globalThis.fetch,nativeLoad=TextureLoader.prototype.loadAsync;
try {
 globalThis.fetch=async url=>new Response(await readFile(fileURLToPath(new URL(url))));TextureLoader.prototype.loadAsync=async()=>new Texture();
 for(const id of ids){
  const p=products.find(p=>p.id===id);assert.ok(p);assert.equal(p.runtime.board,id==='m5stack-u133'?'m5-kmeter':id);assert.equal(p.runtime.status,'partial');
  const url=new URL(p.model.url,root),meta=JSON.parse(await readFile(url));
  for(const [name,hash]of Object.entries(meta.artifacts))assert.equal(createHash('sha256').update(await readFile(new URL(name,url))).digest('hex'),hash,id+' '+name);
  const model=createManufacturerModel(p,null);await model.ready;assert.equal(model.group.userData.inspectionError,undefined);assert.equal(model.parts['pcb-main'].userData.kind,'pcb-placeholder');
  const independent=id==='m5stack-u109-x'?model.parts['rf-shield']:id==='m5stack-u133'?model.parts['thermocouple-socket']:model.parts['camera-lens'];const color=independent.material.color.getHexString();
  for(const shell of model.group.userData.shellMeshes)applyShellAppearance(shell,{shell:{color:'#8054b3',opacity:.35,finish:'plastic'}});
  assert.equal(independent.material.color.getHexString(),color,'Source component material stays independent of shell colour');
  const original=model.parts['pcb-main'].position.toArray();model.group.userData.inspection?.setProgress(1);model.group.userData.inspection?.setProgress(0);assert.deepEqual(model.parts['pcb-main'].position.toArray(),original);model.dispose();
 }
 const scope=JSON.parse(await readFile(new URL('../docs/platform/research/m5-full-store-scope-20261126.json',import.meta.url)));const k024=scope.products.find(p=>p.skus.includes('K024'));
 assert.equal(k024.classification,'requires-external-host-accessory');assert.ok(!scope.missingConfirmedDeviceSkus.includes('K024'));assert.ok(!products.some(p=>p.sku==='K024'),'No invented K024 controller');
 assert.equal(products.find(p=>p.id==='m5stack-u133').chip,'esp32c3');assert.equal(products.find(p=>p.id==='m5stack-u082').runtime.psramMb,8);
 console.log('ok four legacy previews: actual model loads, all asset hashes, UNKNOWN boards, material isolation, inspection reset, exact U133 identity and K024 external-host correction');
}finally{globalThis.fetch=nativeFetch;TextureLoader.prototype.loadAsync=nativeLoad;}
