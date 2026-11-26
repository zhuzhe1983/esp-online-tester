import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {applyShellAppearance} from '../web/platform/shell-appearance.js';
const ids=['m5stack-u121','m5stack-u121-b','m5stack-u121-b-v11','m5stack-u174','m5stack-u174-b'];
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const nativeFetch=globalThis.fetch,nativeLoad=THREE.TextureLoader.prototype.loadAsync,nativeDocument=globalThis.document;
const output=process.argv[2],rows=[];
try {
 globalThis.fetch=async url=>{try{return new Response(await readFile(fileURLToPath(url)));}catch{return new Response('',{status:404});}};
 THREE.TextureLoader.prototype.loadAsync=async url=>{await readFile(fileURLToPath(url));const map=new THREE.Texture();map.userData.sourceUrl=url;return map;};
 globalThis.document={createElement:()=>({getContext:()=>({fillText(text){assert.equal(text,'UNKNOWN');}})})};
 const {items}=JSON.parse(await readFile(new URL('../web/platform/catalog/products.json',import.meta.url)));
 for(const id of ids){
  const product=items.find(p=>p.id===id);assert.ok(product);assert.ok(product.runtime.board ? product.runtime.status==='partial' : product.runtime.status==='unavailable');assert.equal(product.display,null);
  const model=createManufacturerModel(product,null),metadata=await model.ready;model.group.updateMatrixWorld(true);assert.equal(model.group.userData.inspectionError,undefined,id+' inspection contract');assert.ok(model.group.userData.inspection);
  const assetBase=new URL('../web/platform/'+product.model.url,import.meta.url),artifactRows=[];
  for(const [name,digest]of Object.entries(metadata.artifacts)){const data=await readFile(new URL(name,assetBase));assert.equal(hash(data),digest,id+' artifact '+name);artifactRows.push({file:name,bytes:data.length,sha256:digest});}
  const physical=new THREE.Box3(),shells=new Set(model.group.userData.shellMeshes),fixed=[],meshes=[];
  model.group.traverse(mesh=>{if(!mesh.isMesh)return;if(mesh.material.map){const uv=mesh.geometry.getAttribute('uv');if(uv)for(let i=0;i<uv.count;i++)assert.ok(uv.getX(i)>=0&&uv.getX(i)<=1&&uv.getY(i)>=0&&uv.getY(i)<=1,id+' bounded source texture UVs');}meshes.push(mesh);if(!shells.has(mesh))fixed.push({mesh,color:mesh.material.color.getHex(),opacity:mesh.material.opacity,map:mesh.material.map});if(!mesh.material.map&&!mesh.name.startsWith('cover-key:'))physical.union(new THREE.Box3().setFromObject(mesh));});
  const before=model.group.children.map(o=>({o,position:o.position.toArray(),rotation:o.quaternion.toArray()}));
  const pcb=model.parts['pcb-main'],anchorBefore=model.parts.grove.getWorldPosition(new THREE.Vector3()).toArray();assert.equal(model.connectors.grove,undefined,id+' uncalibrated Grove cannot create attached cable');
  const marker=pcb.getObjectByName('pcb-main-unknown-front');assert.ok(marker?.material.map,id+' visible UNKNOWN canvas surface exists');
  model.group.userData.inspection.setExpanded(true);model.group.updateMatrixWorld(true);assert.deepEqual(model.parts.grove.getWorldPosition(new THREE.Vector3()).toArray(),anchorBefore,id+' Grove stays with electronics while covers move');
  const size=marker.geometry.parameters,positions=[[0,0],[size.width*.48,size.height*.48],[-size.width*.48,size.height*.48],[size.width*.48,-size.height*.48],[-size.width*.48,-size.height*.48]],targets=positions.map(([x,y])=>marker.localToWorld(new THREE.Vector3(x,y,0)));
  let visibleView=null;
  for(const x of [-2,-1.5,-1,-.5,.5,1,1.5,2])for(const y of [-1,-.5,0,.5,1]){
   if(visibleView)continue;const direction=new THREE.Vector3(x,y,1).normalize(),visible=targets.every(target=>{const ray=new THREE.Raycaster(target.clone().addScaledVector(direction,200),direction.clone().negate(),0,200.02),hit=ray.intersectObjects(meshes,false)[0];return hit?.object===marker;});
   if(visible)visibleView=direction.toArray();
  }
  assert.ok(visibleView,id+' a single oblique inspect view exposes center and four inset UNKNOWN corners geometrically');
  for(let i=0;i<3;i++){model.group.userData.inspection.setExpanded(false);model.group.userData.inspection.setExpanded(true);}model.group.userData.inspection.setExpanded(false);
  for(const {o,position,rotation}of before){assert.deepEqual(o.position.toArray(),position,id+' exact inspection reset');assert.deepEqual(o.quaternion.toArray(),rotation);}
  for(const appearance of [{shell:{color:'#bfc2bb',opacity:1,finish:'plastic'}},{shell:{color:'#d4af37',opacity:1,finish:'metal'}},{shell:{color:'#8054b3',opacity:.35,finish:'plastic'}}]){
   for(const shell of shells)applyShellAppearance(shell,appearance);
   for(const {mesh,color,opacity,map}of fixed){assert.equal(mesh.material.color.getHex(),color,id+' fixed tint '+mesh.name);assert.equal(mesh.material.opacity,opacity,id+' fixed opacity '+mesh.name);assert.equal(mesh.material.map,map,id+' fixed texture '+mesh.name);}
  }
  const record={id,modelSha256:hash(await readFile(assetBase)),planSha256:hash(await readFile(new URL('model-plan.json',assetBase))),geometrySha256:metadata.artifacts['geometry.bin'],nativeSourceSha256:metadata.artifacts['source.stl'],nativeSourceCommit:metadata.provenance.commit,physicalAssembledBoundsMm:[physical.min.toArray(),physical.max.toArray()],physicalAssembledSizeMm:physical.getSize(new THREE.Vector3()).toArray(),physicalBoundsScope:'Native plus derived solid meshes, excluding paper/photo/UNKNOWN planes and invisible input helpers. Approximate connectors/PCB remain estimates.',inspectUnknownGeometricLineOfSight:{singleObliqueDirection:visibleView,points:'Center plus four inset corners',scope:'Actual model ray intersections; not rendered glyph readability or browser acceptance.'},inspectionResetExact:true,groveAnchorStaysWithBoard:true,uncalibratedCableDisabled:true,materialIsolation:{presets:['original-like','gold','translucent-purple'],fixedPartMaterialsUnchanged:true},runtime:{board:product.runtime.board,status:product.runtime.status},artifacts:artifactRows};rows.push(record);model.dispose();
 }
}finally{globalThis.fetch=nativeFetch;THREE.TextureLoader.prototype.loadAsync=nativeLoad;if(nativeDocument===undefined)delete globalThis.document;else globalThis.document=nativeDocument;}
if(output){await mkdir(new URL('.',new URL('file://'+output)),{recursive:true});await writeFile(output,JSON.stringify({schemaVersion:'1',checkedAt:new Date().toISOString(),rows,browserAcceptance:'Not run: cloud Chrome cannot create a WebGL context. No hardware or firmware execution validation claimed.'},null,2)+'\n');}
console.log('ok five camera models: exact artifact hashes, honest runtime boundary, complete assembly bounds, material isolation, inspect visibility geometry and exact reset');
