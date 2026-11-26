import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {fileURLToPath} from 'node:url';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {applyShellAppearance} from '../web/platform/shell-appearance.js';
const ids=['m5stack-u138','m5stack-u037','m5stack-k115'],rows=[];
const hash=b=>createHash('sha256').update(b).digest('hex');
const originalFetch=globalThis.fetch,originalLoad=THREE.TextureLoader.prototype.loadAsync,originalDocument=globalThis.document;
try{
 globalThis.fetch=async u=>new Response(await readFile(fileURLToPath(u)));
 THREE.TextureLoader.prototype.loadAsync=async u=>{await readFile(fileURLToPath(u));return new THREE.Texture();};
 globalThis.document={createElement:()=>({getContext:()=>({fillText(s){assert.equal(s,'UNKNOWN');}})})};
 const products=JSON.parse(await readFile(new URL('../web/platform/catalog/products.json',import.meta.url))).items;
 const firmwares=JSON.parse(await readFile(new URL('../web/platform/catalog/firmwares.json',import.meta.url))).items,watchlist=JSON.parse(await readFile(new URL('../web/platform/catalog/firmware-watchlist.json',import.meta.url))).entries;
 for(const id of ids){
  const product=products.find(p=>p.id===id);assert.ok(product);assert.equal(product.chip,'esp32');assert.equal(product.runtime.board,{'m5stack-u138':'unit-poesp32','m5stack-u037':'m5stack-u037','m5stack-k115':'atom-psram-display'}[id]);assert.equal(product.runtime.status,'partial');assert.equal(product.display,null);
  assert.equal(product.runtime.flashMb,id==='m5stack-k115'?8:4);assert.equal(product.runtime.psramMb,id==='m5stack-k115'?2:id==='m5stack-u037'?4:0);
  const firmware=firmwares.find(f=>f.id===product.defaultFirmwareId);assert.ok(firmware);assert.deepEqual(firmware.productIds,[id]);assert.equal(firmware.status,'reference');assert.equal(firmware.source,null);assert.equal(firmware.role,id==='m5stack-u138'?'factory':'project');assert.equal(firmware.simulationVerified,false);
  for(const kind of ['factory','uiflow','xiaozhi']){const watch=watchlist.find(w=>w.id===id+'-'+kind+'-reference-watch');assert.ok(watch);assert.equal(watch.metadataProvider,'manual');assert.deepEqual(watch.compatibility.productIds,[id]);assert.equal(watch.compatibility.status,'reference-only');assert.equal(watch.compatibility.simulationVerified,false);assert.deepEqual(watch.catalogTargets,[]);assert.deepEqual(watch.assets,[]);}
  const asset=new URL('../web/platform/'+product.model.url,import.meta.url),plan=JSON.parse(await readFile(new URL('model-plan.json',asset)));
  const model=createManufacturerModel(product,null),meta=await model.ready;assert.equal(model.group.userData.inspectionError,undefined);assert.ok(model.group.userData.inspection);
  for(const [name,digest]of Object.entries(meta.artifacts))assert.equal(hash(await readFile(new URL(name,asset))),digest,id+' '+name);
  const geometry=await readFile(new URL(meta.binary,asset));
  if(plan.cad){
   const source=await readFile(new URL(plan.cad.file,asset)),matrix=new THREE.Matrix4().set(...plan.cad.matrixRows.flat());
   for(const part of plan.cad.parts){
    const exported=meta.parts.find(p=>p.name===part.name);let vertex=0;
    for(const [start,end]of part.sourceFaceRanges)for(let face=start;face<end;face++)for(let corner=0;corner<3;corner++){
     const off=84+face*50+12+corner*12,expected=new THREE.Vector3(source.readFloatLE(off),source.readFloatLE(off+4),source.readFloatLE(off+8)).applyMatrix4(matrix);
     const actual=exported.byteOffset+(vertex++)*24;
     assert.ok(Math.max(...expected.toArray().map((v,i)=>Math.abs(v-geometry.readFloatLE(actual+i*4))))<1e-5,id+' native triangle unchanged');
    }
    assert.equal(vertex,exported.vertexCount);
   }
  }
  model.group.updateMatrixWorld(true);const meshes=[],shells=new Set(model.group.userData.shellMeshes),fixed=[];
  model.group.traverse(m=>{if(!m.isMesh)return;meshes.push(m);if(m.material.map){const uv=m.geometry.getAttribute('uv');if(uv)for(let i=0;i<uv.count;i++)assert.ok(uv.getX(i)>=0&&uv.getX(i)<=1&&uv.getY(i)>=0&&uv.getY(i)<=1,id+' source UV bounded');}if(!shells.has(m))fixed.push({m,color:m.material.color.getHex(),opacity:m.material.opacity,map:m.material.map});});
  assert.equal(model.connectors.grove,undefined,'Uncalibrated Grove cannot attach a cable');
  const before=model.group.children.map(o=>({o,position:o.position.toArray(),quaternion:o.quaternion.toArray()}));
  model.group.userData.inspection.setExpanded(true);model.group.updateMatrixWorld(true);const visible=[];
  for(const key of ['pcb-main',...(id==='m5stack-k115'?['atom-pcb']:[])]){
   const pcb=model.parts[key];assert.equal(pcb.userData.kind,'pcb-placeholder');const marker=pcb.getObjectByName(key+'-unknown-front');assert.ok(marker?.material.map);
   const {width,height}=marker.geometry.parameters,targets=[[0,0],[width*.45,height*.45],[-width*.45,height*.45],[width*.45,-height*.45],[-width*.45,-height*.45]].map(([x,y])=>marker.localToWorld(new THREE.Vector3(x,y,0)));let view;
   for(const x of [-2,-1,-.5,.5,1,2])for(const y of [-1,0,1]){if(view)continue;const d=new THREE.Vector3(x,y,1).normalize();if(targets.every(t=>new THREE.Raycaster(t.clone().addScaledVector(d,200),d.clone().negate(),0,200.02).intersectObjects(meshes,false)[0]?.object===marker))view=d.toArray();}
   assert.ok(view,id+' UNKNOWN center and four corners visible from an oblique inspection view');visible.push({part:key,direction:view});
  }
  for(let n=0;n<3;n++){model.group.userData.inspection.setExpanded(false);model.group.userData.inspection.setExpanded(true);}model.group.userData.inspection.setExpanded(false);
  for(const p of before){assert.deepEqual(p.o.position.toArray(),p.position);assert.deepEqual(p.o.quaternion.toArray(),p.quaternion);}
  for(const shell of [{color:'#bfc2bb',opacity:1,finish:'plastic'},{color:'#d4af37',opacity:1,finish:'metal'},{color:'#8054b3',opacity:.35,finish:'plastic'}]){
   for(const mesh of shells)applyShellAppearance(mesh,{shell});
   for(const {m,color,opacity,map}of fixed){assert.equal(m.material.color.getHex(),color,id+' fixed material '+m.name);assert.equal(m.material.opacity,opacity);assert.equal(m.material.map,map);}
  }
  const body=new THREE.Box3();for(const part of meta.parts.filter(p=>p.sourceFaceRanges)){body.expandByPoint(new THREE.Vector3(...part.boundsMm[0]));body.expandByPoint(new THREE.Vector3(...part.boundsMm[1]));}
  model.group.updateMatrixWorld(true);const assembled=new THREE.Box3();for(const m of meshes)if(!m.material.map&&!m.name.startsWith('cover-key:'))assembled.union(new THREE.Box3().setFromObject(m));
  rows.push({id,modelSha256:hash(await readFile(asset)),planSha256:hash(await readFile(new URL('model-plan.json',asset))),geometrySha256:meta.artifacts[meta.binary],nativeBodyBoundsMm:plan.cad?[body.min.toArray(),body.max.toArray()]:null,nativeBodySizeMm:plan.cad?body.getSize(new THREE.Vector3()).toArray():null,physicalAssembledBoundsMm:[assembled.min.toArray(),assembled.max.toArray()],physicalAssembledSizeMm:assembled.getSize(new THREE.Vector3()).toArray(),physicalBoundsScope:'Native and estimated solid hardware including placeholder board solids; excludes photo/UNKNOWN and invisible input planes.',nativeTrianglesUnchanged:Boolean(plan.cad),opaqueTextureUvBounded:true,materialPresets:['original-like','gold','translucent-purple'],fixedMaterialsUnchanged:true,unknownInspectionSightLines:visible,inspectionResetExact:true,runtime:product.runtime.status,scope:'Actual loader and mesh/ray checks, with image decoding stubbed. No rendered-photo, glyph, firmware or hardware acceptance.'});model.dispose();
 }
}finally{globalThis.fetch=originalFetch;THREE.TextureLoader.prototype.loadAsync=originalLoad;if(originalDocument===undefined)delete globalThis.document;else globalThis.document=originalDocument;}
if(process.argv[2]){const target=new URL('file://'+process.argv[2]);await mkdir(new URL('.',target),{recursive:true});await writeFile(target,JSON.stringify({schemaVersion:'1',checkedAt:new Date().toISOString(),rows,browserAcceptance:'Not run; no rendered WebGL or physical acceptance claimed.'},null,2)+'\n');}
console.log('ok U138/U037/K115: actual loader, native triangles, artifact hashes, UV bounds, fixed materials and UNKNOWN inspection sight lines');
